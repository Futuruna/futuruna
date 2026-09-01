//! Public invocation/report boundary for the resumable relational Explore engine.
//!
//! The durable journal is the source of truth.  This adapter checks and binds
//! one query, advances its semantic stream under the resource envelope, and
//! projects only compact counters, including how many result records this
//! invocation appended. It deliberately does not clone the complete relation
//! merely to print a checkpoint.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::num::{NonZeroU64, NonZeroUsize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::{
    calculate, walk_ast_stmt, AstChild, CheckedConstructorLayout, CheckedExploreAnalysisIdentity,
    CheckedExploreCoverageBindingRole, CheckedExploreCoverageClassification,
    CheckedExploreCoverageGapReason, CheckedExploreCoverageLiteralKind,
    CheckedExploreCoverageRootRole, CheckedExploreCoverageSubject, CheckedExploreQueryView,
    CheckedExploreSourceCoverageManifest, Diagnostic, ExploreAdmissionScope, Expr,
    OwnedCheckedExploreQuery, Stmt, Ty, TypeCheckArtifacts, TypeChecker,
};

use super::mechanism_incidence::MechanismCountEvidence;
use super::relation::AdmissionDecision;
use super::relational_analysis_catalog::{
    RelationalAnalysisLayerSnapshot, RelationalAnalysisLayerStatus,
    RelationalResultLayerSnapshotState, RelationalResultPublication,
};
use super::relational_analysis_plan::{RelationalAnalysisLayerId, RelationalAnalysisPlan};
use super::relational_classification_capsule::{
    ClassificationProvenanceRoot, ClassificationSpecializationRoot, RelationalClassificationCapsule,
};
use super::relational_classification_evaluator::RelationalClassificationEvaluatorBackend;
use super::relational_durable_journal::{RelationalDurableJournal, RelationalDurableJournalLimits};
use super::relational_interpreter_mechanism::{
    checked_ground_definitions, RelationalInterpreterMechanismReplayRuntime,
};
use super::relational_journal::{RelationalJournal, RelationalJournalContract};
use super::relational_native_classifier::{
    RelationalNativeClassifierProtocolV2, RelationalNativeClassifierV2,
};
use super::relational_region_proof::RelationalRegionReplayAuthority;
use super::relational_result_publication::{
    publish_relational_result_artifacts, RelationalPublicationLimits, RelationalPublicationPlan,
};
use super::relational_stream_driver::{
    RelationalStreamDriver, RelationalStreamDriverLimits, RelationalStreamQuiescence,
};
use super::relational_stream_run_loop::{
    run_relational_stream_slice_with_resources, ExactStreamOuterContainmentReceipt,
    RelationalBaseQuantumController, RelationalStreamSliceBudget, RelationalStreamSliceOutcome,
    RelationalStreamSlicePauseReason,
};
use super::relational_support_planner::statically_evaluate_checked_int_range;
use super::result_projection::{IndexedResultProjectionRecord, ResultProjectionRecord};
use super::result_view::{ResultGroupDisposition, ResultValue, ResultViewCount, ResultViewSpec};
use super::stream_resource::ExactStreamOneWorkerEnvelope;
use super::{
    ExploreAnalysisNodeIr, ExploreFindIr, ExploreFiniteDomainIr, ExploreMechanismTargetIr,
    ExploreSourceBindingKindIr, ExploreSuccessorKindIr, RelationalInterpreterExpressionRuntime,
    RelationalSupportPlan, RelationalSupportPlanner,
};

pub const EXPLORE_RELATIONAL_STREAM_REPORT_VERSION: u32 = 5;

const RESULT_PREVIEW_ROWS_PER_VIEW: usize = 16;
const RESULT_PREVIEW_ROWS_PER_REPORT: usize = 64;
const RESULT_PREVIEW_RECORDS_PER_VIEW: u128 = 256;
const RESULT_PREVIEW_RECORDS_PER_REPORT: u128 = 1_024;
const RESULT_PREVIEW_VALUE_NODES_PER_REPORT: usize = 4_096;
const RESULT_PREVIEW_VALUE_BYTES_PER_REPORT: usize = 256 * 1024;
/// Operational only. V1 caches complete pure-call results and never admits a
/// partial call, so this fixed cap bounds retained cross-chunk state without
/// entering query, support, capsule, journal, or result identity.
const CLASSIFICATION_CALL_CACHE_ENTRIES: usize = 16_384;

/// Operational proof carried by a CLI slice that is already enclosed by the
/// validated process-group supervisor.
///
/// These limits do not change query identity or durable semantics. They let
/// the inner one-worker governor charge a bounded stream quantum while the
/// outer layer independently enforces the epoch-wide Rust-heap ceiling and
/// retains room for stacks, FFI/mappings, host memory and process-group
/// containment. Supplying
/// this value without actually installing and monitoring the described
/// envelope violates the execution contract; ordinary library callers should
/// leave `outer_containment` as `None`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExploreStreamOuterContainment {
    pub rust_heap_limit_bytes: NonZeroU64,
    pub untracked_memory_reserve_bytes: NonZeroU64,
    pub group_rss_limit_bytes: NonZeroU64,
    pub available_memory_floor_bytes: NonZeroU64,
}

/// Operational controls for one append-only Explore invocation.
///
/// Neither field participates in semantic identity.  A later invocation may
/// resume the same run directory with a different time budget.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExploreStreamSliceOptions {
    pub run_state: PathBuf,
    /// Optional crash-resumable public materialization. This must be a
    /// separate directory from the authoritative run-state tree.
    pub output_directory: Option<PathBuf>,
    pub max_runtime: Option<Duration>,
    /// Fresh operational authority for this process invocation only. It is
    /// reacquired on every resume and is never written to the semantic journal.
    pub outer_containment: Option<ExploreStreamOuterContainment>,
}

/// Paths and process-local authority retained by one warm Explore epoch.
///
/// An epoch owns the durable writer fence until it is dropped. Logical slice
/// budgets remain per-call so a prepared worker can pause and continue without
/// repeating checking or replay-catalog construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExploreStreamEpochOptions {
    pub run_state: PathBuf,
    pub output_directory: Option<PathBuf>,
    pub outer_containment: Option<ExploreStreamOuterContainment>,
}

/// Immutable checked preparation plus process-local evaluator caches.
///
/// This is intentionally an in-memory artifact. Durable resumption is still
/// authorized by the journal; serializing compiler heap layout is not part of
/// this contract.
pub struct PreparedRelationalExplore {
    checked: Arc<OwnedCheckedExploreQuery>,
    support_plan: RelationalSupportPlan,
    region_replay_authority: Arc<RelationalRegionReplayAuthority>,
    contract: RelationalJournalContract,
    publication_plan: RelationalPublicationPlan,
    expression_runtime: RelationalInterpreterExpressionRuntime,
    mechanism_runtime: RelationalInterpreterMechanismReplayRuntime,
    /// Canonical checked classification graph and its bounded warm call cache.
    /// The one-worker `RefCell` supplies interior mutability to the otherwise
    /// immutable semantic driver and retains that cache across resumable epoch
    /// slices.
    classification_evaluator: RefCell<RelationalClassificationEvaluatorBackend>,
    native_classifier_plan: Option<ExploreNativeClassifierPlanV2>,
    native_classifier_shape_v2: bool,
    native_classifier: Option<RelationalNativeClassifierV2>,
    preparation_wall_time: Duration,
}

/// Producer-bound semantic identity embedded in a native classifier.
///
/// A classifier may return only ordered outcome tags for this exact identity;
/// the relational coordinator remains the sole producer of source/case IDs,
/// transcripts, evidence, and journal events.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExploreNativeClassifierIdentityV2 {
    pub checked_program: [u8; 32],
    pub relation_id: [u8; 32],
    pub admission_id: [u8; 32],
    pub question_id: [u8; 32],
}

/// One normalized scoped-WHERE predicate in authored/canonical order.
#[doc(hidden)]
#[derive(Clone, Debug)]
pub struct ExploreNativeClassifierAdmissionV2 {
    pub scope: ExploreAdmissionScope,
    pub predicate: Expr,
}

/// The normalized FIND operation supported by native classifier V2.
#[doc(hidden)]
#[derive(Clone, Debug)]
pub enum ExploreNativeClassifierFindV2 {
    All,
    Matches { predicate: Expr },
    Violations { predicate: Expr },
}

/// One checked source binding reconstructed by native classifier V2.
///
/// Independent finite integer ranges become function inputs. Singleton
/// bindings retain their checked expression and are replayed in authored
/// source order, so derived records such as a profile and composite `Before`
/// state retain exactly the checked relation semantics.
#[doc(hidden)]
#[derive(Clone, Debug)]
pub struct ExploreNativeClassifierSourceBindingV2 {
    pub binding_index: usize,
    pub name: String,
    pub ty: Ty,
    pub kind: ExploreNativeClassifierSourceBindingKindV2,
}

#[doc(hidden)]
#[derive(Clone, Debug)]
pub enum ExploreNativeClassifierSourceBindingKindV2 {
    FiniteIntInput,
    Singleton { value: Expr },
}

/// Classification-only compiler input for native classifier V2.
///
/// V2 accepts one or more independent, statically bounded `Int` ranges mixed
/// with ordered singleton/derived source bindings, followed by a singleton
/// successor, scoped admissions, and All/Matches/Violations FIND. Finite
/// values are operational accelerator inputs only: they never become CaseIds,
/// source identities, transcript coordinates, or journal evidence.
#[doc(hidden)]
#[derive(Clone, Debug)]
pub struct ExploreNativeClassifierPlanV2 {
    pub identity: ExploreNativeClassifierIdentityV2,
    pub source_bindings: Box<[ExploreNativeClassifierSourceBindingV2]>,
    pub finite_input_binding_indices: Box<[usize]>,
    pub finite_coordinate_count: u128,
    pub after_binding_name: String,
    pub after_ty: Ty,
    pub successor_value: Expr,
    pub admissions: Box<[ExploreNativeClassifierAdmissionV2]>,
    pub find: ExploreNativeClassifierFindV2,
    /// Frozen constructor-normalized declarations accepted by the same
    /// checked-program boundary that minted `identity`. The sidecar compiler
    /// must consume this snapshot and must not resolve authored imports again.
    pub checked_declarations: Box<[Stmt]>,
    pub compile_time_metadata_bindings: BTreeSet<String>,
}

impl PreparedRelationalExplore {
    pub const fn preparation_wall_time(&self) -> Duration {
        self.preparation_wall_time
    }

    /// Return query-bound native-classifier input only when the checked,
    /// normalized relation has the exact V2 shape.
    ///
    /// `None` disables the optimization. Exact stream execution remains
    /// available through the checked interpreter and must never approximate an
    /// unsupported query into this plan.
    #[doc(hidden)]
    pub fn take_native_classifier_plan_v2(&mut self) -> Option<ExploreNativeClassifierPlanV2> {
        self.native_classifier_plan.take()
    }

    /// Install a query-bound operational classifier for subsequent epoch
    /// slices.
    ///
    /// The executable receives no direct evidence-writing authority. Protocol
    /// validation and the interpreter canary are fail-closed checks for
    /// accidental incompatibility; they do not authenticate the executable or
    /// prove that its classifications implement the checked query.
    ///
    /// # Safety
    ///
    /// `executable` must be a trusted artifact compiled from the frozen
    /// [`ExploreNativeClassifierPlanV2`] produced by this prepared query. For
    /// every accepted V2 request it must return exactly the ordered admission
    /// and FIND outcomes of Futuruna's checked evaluator for the identity in
    /// that plan. The caller must also ensure that the path continues to name
    /// those exact executable bytes for the lifetime of every epoch opened
    /// from this preparation; it must not be replaced or have a symlink
    /// retargeted after installation. The process executes with the caller's
    /// operating-system privileges.
    ///
    /// Violating this contract can mint false exact exploration evidence. The
    /// echoed identity and first-batch parity canary are not substitutes for
    /// this trust requirement.
    #[doc(hidden)]
    pub unsafe fn install_native_classifier_executable_v2(
        &mut self,
        executable: PathBuf,
    ) -> Result<(), ExploreStreamPreparationError> {
        if !self.native_classifier_shape_v2 {
            return Err(ExploreStreamPreparationError::Execution(
                "selected exploration does not have the native classifier V2 shape".into(),
            ));
        }
        let native =
            RelationalNativeClassifierV2::for_checked_query(executable, &self.checked.view())
                .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?;
        self.native_classifier = Some(native);
        Ok(())
    }

    pub fn open_epoch(
        self,
        options: ExploreStreamEpochOptions,
    ) -> Result<RelationalExploreEpoch, ExploreStreamPreparationError> {
        validate_epoch_options(&options)?;
        let outer_containment = exact_stream_outer_containment(options.outer_containment)?;
        let resources = ExactStreamOneWorkerEnvelope::new_with_outer_containment(outer_containment)
            .map_err(|reason| {
                ExploreStreamPreparationError::Execution(format!(
                    "cannot initialize Explore resource envelope: {}",
                    reason.code()
                ))
            })?;
        let durable = RelationalDurableJournal::open_or_create_with_region_replay_authority(
            &options.run_state,
            self.contract,
            RelationalDurableJournalLimits::default(),
            Arc::clone(&self.region_replay_authority),
        )
        .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?;
        Ok(RelationalExploreEpoch {
            prepared: self,
            durable,
            options,
            base_quantum_controller: RelationalBaseQuantumController::default(),
            resources,
        })
    }
}

fn native_classifier_plan_v2_from_checked(
    checked: &CheckedExploreQueryView<'_>,
    checked_declarations: Box<[Stmt]>,
    compile_time_metadata_bindings: BTreeSet<String>,
) -> Option<ExploreNativeClassifierPlanV2> {
    let query = checked.closed_query;
    query.validate().ok()?;
    if query.source.bindings.is_empty() {
        return None;
    }

    let mut source_bindings = Vec::with_capacity(query.source.bindings.len());
    let mut finite_input_binding_indices = Vec::new();
    let mut finite_coordinate_count = 1u128;
    for (position, binding) in query.source.bindings.iter().enumerate() {
        if binding.binding_index != position || binding.name.is_empty() {
            return None;
        }
        let kind = match &binding.kind {
            ExploreSourceBindingKindIr::Finite { domain } => {
                if !binding.dependencies.is_empty()
                    || !native_classifier_int_ty(&binding.value_ty)
                    || !matches!(domain, ExploreFiniteDomainIr::IntRange { .. })
                {
                    return None;
                }
                let cardinality = statically_evaluate_checked_int_range(domain)
                    .ok()
                    .flatten()?
                    .cardinality();
                if cardinality == 0 {
                    return None;
                }
                finite_coordinate_count = finite_coordinate_count.checked_mul(cardinality)?;
                finite_input_binding_indices.push(binding.binding_index);
                ExploreNativeClassifierSourceBindingKindV2::FiniteIntInput
            }
            ExploreSourceBindingKindIr::Singleton { value } => {
                ExploreNativeClassifierSourceBindingKindV2::Singleton {
                    value: value.clone(),
                }
            }
        };
        source_bindings.push(ExploreNativeClassifierSourceBindingV2 {
            binding_index: binding.binding_index,
            name: binding.name.clone(),
            ty: binding.value_ty.clone(),
            kind,
        });
    }
    if finite_input_binding_indices.is_empty()
        || finite_input_binding_indices.len()
            > RelationalNativeClassifierProtocolV2::MAX_FACTORS_PER_SUBJECT
    {
        return None;
    }
    let ExploreSuccessorKindIr::Singleton {
        value: successor_value,
    } = &query.successor.kind
    else {
        return None;
    };

    let find = match &query.find {
        ExploreFindIr::All { .. } => ExploreNativeClassifierFindV2::All,
        ExploreFindIr::Matches { predicate, .. } => ExploreNativeClassifierFindV2::Matches {
            predicate: predicate.clone(),
        },
        ExploreFindIr::Violations { predicate, .. } => ExploreNativeClassifierFindV2::Violations {
            predicate: predicate.clone(),
        },
    };
    let checked_program = decode_lowercase_sha256(checked.program_hash())?;

    Some(ExploreNativeClassifierPlanV2 {
        identity: ExploreNativeClassifierIdentityV2 {
            checked_program,
            relation_id: checked.relation_id().bytes(),
            admission_id: checked.admission_id().bytes(),
            question_id: checked.question_id().bytes(),
        },
        source_bindings: source_bindings.into_boxed_slice(),
        finite_input_binding_indices: finite_input_binding_indices.into_boxed_slice(),
        finite_coordinate_count,
        after_binding_name: "after".to_string(),
        after_ty: query.successor.after_ty.clone(),
        successor_value: successor_value.clone(),
        admissions: query
            .admissions
            .iter()
            .map(|admission| ExploreNativeClassifierAdmissionV2 {
                scope: admission.scope,
                predicate: admission.predicate.clone(),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        find,
        checked_declarations,
        compile_time_metadata_bindings,
    })
}

/// Clone only the compiler-proven declaration occurrences reachable from this
/// query's FROM/TO, WHERE, and FIND layers. Imports have already been
/// normalized into canonical checked-program order. Any import left inside a
/// retained nested declaration would make codegen capable of rereading the
/// filesystem, so V2 declines that optimization. Raw Rust and external
/// dependencies are also outside this exact classifier boundary.
fn native_classifier_checked_declarations(
    artifacts: &TypeCheckArtifacts,
    checked: &CheckedExploreQueryView<'_>,
) -> Option<Box<[Stmt]>> {
    let declarations = match artifacts.checked_explore_classifier_declarations_v1(checked) {
        Ok(declarations) => declarations,
        Err(error) => {
            if std::env::var_os("FUTURUNA_EXPLORE_TRACE").is_some() {
                eprintln!("Explore native classifier declaration slice unavailable: {error}");
            }
            return None;
        }
    };
    if std::env::var_os("FUTURUNA_EXPLORE_TRACE").is_some() {
        eprintln!(
            "Explore native classifier declaration slice: {} statements",
            declarations.len()
        );
    }
    for statement in declarations.iter() {
        let mut unsupported = false;
        walk_ast_stmt(statement, &mut |child| {
            let AstChild::Stmt(statement) = child else {
                return;
            };
            if matches!(
                statement,
                Stmt::Import(_)
                    | Stmt::QualifiedImport(_, _)
                    | Stmt::HashImport(_, _)
                    | Stmt::Depend(_, _)
                    | Stmt::RustBlock(_)
            ) {
                unsupported = true;
            }
        });
        if unsupported {
            return None;
        }
    }
    Some(declarations)
}

fn native_classifier_int_ty(ty: &Ty) -> bool {
    matches!(ty, Ty::Name(name) if matches!(name.as_str(), "Int" | "Heltal"))
}

fn decode_lowercase_sha256(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64
        || value
            .as_bytes()
            .iter()
            .any(|byte| !matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return None;
    }
    let mut digest = [0u8; 32];
    for (index, output) in digest.iter_mut().enumerate() {
        let offset = index * 2;
        *output = u8::from_str_radix(&value[offset..offset + 2], 16).ok()?;
    }
    Some(digest)
}

fn bind_relational_classification_capsule(
    checked: &CheckedExploreQueryView<'_>,
    support_plan: &RelationalSupportPlan,
) -> Result<Arc<RelationalClassificationCapsule>, ExploreStreamPreparationError> {
    let checked_program = decode_lowercase_sha256(checked.program_hash()).ok_or_else(|| {
        ExploreStreamPreparationError::Execution(
            "checked Explore program identity is not canonical lowercase SHA-256".into(),
        )
    })?;
    let provenance_digest = decode_lowercase_sha256(
        checked.source_coverage().manifest_digest.as_ref(),
    )
    .ok_or_else(|| {
        ExploreStreamPreparationError::Execution(
            "checked Explore source-coverage identity is not canonical lowercase SHA-256".into(),
        )
    })?;
    let capsule = RelationalClassificationCapsule::bind(
        checked.classification_program(),
        checked.classification_runtime_shapes(),
        checked_program,
        checked.relation_id(),
        checked.admission_id(),
        checked.question_id(),
        support_plan.root(),
        support_plan.root_cell_id(),
        ClassificationSpecializationRoot::none(),
        ClassificationProvenanceRoot::from_checked_source_coverage_digest(provenance_digest),
    )
    .map_err(|error| {
        ExploreStreamPreparationError::Execution(format!(
            "checked Explore classification capsule is incoherent: {error}"
        ))
    })?;
    Ok(Arc::new(capsule))
}

/// One open, exclusively owned durable stream that can execute many slices.
pub struct RelationalExploreEpoch {
    prepared: PreparedRelationalExplore,
    durable: RelationalDurableJournal,
    options: ExploreStreamEpochOptions,
    base_quantum_controller: RelationalBaseQuantumController,
    /// One operational governor for the whole warm process epoch. Recreating
    /// it at each observable slice would discard the checked stable window and
    /// repeatedly spend semantic time re-establishing the same host facts.
    resources: ExactStreamOneWorkerEnvelope,
}

/// A cardinality at the current durable frontier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExploreStreamCount {
    Unknown {
        confirmed_lower_bound: u128,
    },
    LowerBound(u128),
    Interval {
        lower_bound: u128,
        upper_bound: u128,
    },
    Exact(u128),
}

impl ExploreStreamCount {
    pub const fn current(self) -> u128 {
        match self {
            Self::Unknown {
                confirmed_lower_bound,
            } => confirmed_lower_bound,
            Self::LowerBound(value) | Self::Exact(value) => value,
            Self::Interval { lower_bound, .. } => lower_bound,
        }
    }

    pub const fn is_exact(self) -> bool {
        matches!(self, Self::Exact(_))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExploreStreamMechanismTarget {
    Selected,
    ChosenView { view_id: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExploreStreamMechanismSupportTotals {
    pub target_cases: u128,
    pub successful_cases: u128,
    pub unavailable_cases: u128,
    pub signature_fibers: u128,
    /// Exact size of the sealed target's starter projection. This is only a
    /// conservative upper bound for any individual structural subject.
    pub target_starters: u128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExploreStreamIdentity {
    pub checked_program: String,
    pub relation_id: String,
    pub admission_id: String,
    pub question_id: String,
    pub analysis_graph_digest: String,
    pub journal_id: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExploreStreamCoverageRootRole {
    Context,
    Before,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExploreStreamCoverageBindingRole {
    Auxiliary,
    Context,
    Before,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExploreStreamCoverageLiteralKind {
    Integer,
    FloatBits,
    String,
    Character,
    Boolean,
    Unit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExploreStreamCoverageGapReason {
    SchemaNotDeclaredRecord,
    SchemaCompositionUnavailable,
    InterproceduralFieldProvenance,
    ConstructorFieldMappingUnavailable,
    ConstructorChoiceProvenanceUnavailable,
    UpstreamCoverageGap,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExploreStreamCoverageFieldPathSegment {
    pub owner_type_name: String,
    pub variant_index: usize,
    pub field_index: usize,
    pub variant_name: String,
    pub field_name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExploreStreamCoverageConstructorLayout {
    Positional,
    Named,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExploreStreamCoverageSubject {
    SourceBinding {
        binding_index: usize,
        binding_name: String,
        role: ExploreStreamCoverageBindingRole,
    },
    SchemaRoot {
        role: ExploreStreamCoverageRootRole,
        type_name: String,
    },
    SchemaField {
        role: ExploreStreamCoverageRootRole,
        path: Vec<ExploreStreamCoverageFieldPathSegment>,
    },
    Literal {
        kind: ExploreStreamCoverageLiteralKind,
        value: String,
    },
    TopLevelConstant {
        dependency_digest: String,
        addresses: Vec<String>,
    },
    ConstructorChoice {
        owner_digest: String,
        owner_name: String,
        variant_name: String,
        variant_index: usize,
        layout: ExploreStreamCoverageConstructorLayout,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExploreStreamCoverageClassification {
    VariedFiniteDimension {
        dimension_id: String,
    },
    DerivedFromDeclaredDimensions {
        dimension_ids: Vec<String>,
    },
    ConditionedSingletonOrSourceRestriction,
    ExactIrrelevanceCertificate {
        certificate_digest: String,
    },
    CoverageGap {
        reason: ExploreStreamCoverageGapReason,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExploreStreamCoverageEntry {
    pub subject_id: String,
    pub subject: ExploreStreamCoverageSubject,
    pub classification: ExploreStreamCoverageClassification,
}

/// Producer-owned account of which source/profile dimensions this query
/// varies, conditions, derives, proves irrelevant, or cannot yet cover.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExploreStreamSourceCoverage {
    pub version: u32,
    pub manifest_digest: String,
    pub semantic_dependency_digest: String,
    pub has_gaps: bool,
    pub entries: Vec<ExploreStreamCoverageEntry>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExploreStreamLifecycle {
    Paused,
    Complete,
}

/// Honest reason why a resumable invocation returned before semantic closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExploreStreamPauseReason {
    RuntimeLimit,
    ResourceAdmission {
        code: String,
    },
    MechanismReplay {
        request_id: String,
        case_id: String,
        endpoint: String,
        reason: String,
    },
    AwaitingChosenViewMechanisms {
        request_id: String,
        view_id: String,
    },
    AwaitingSourceResult {
        view_id: String,
    },
    AwaitingMechanismIncidenceResult {
        view_id: String,
        request_id: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExploreStreamCheckpoint {
    pub next_sequence: u64,
    /// Append-only commitment to the complete durable evidence prefix.
    pub journal_head: String,
    pub durable_segment_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExploreStreamPopulationCounts {
    pub sources: ExploreStreamCount,
    pub cases: ExploreStreamCount,
    pub admission_classified: ExploreStreamCount,
    pub admitted: ExploreStreamCount,
    pub rejected: ExploreStreamCount,
    pub find_classified: ExploreStreamCount,
    pub selected: ExploreStreamCount,
    pub not_selected: ExploreStreamCount,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExploreStreamLayerStatus {
    ResultUnregistered,
    ResultInputOpen,
    ResultAwaitingPublication,
    ResultPublished,
    MechanismUnregistered,
    MechanismTargetOpen,
    MechanismTerminalOpen,
    MechanismClosed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExploreStreamProjectedValue {
    Value(super::ExploreValue),
    CaseId(String),
    TransitionId(String),
    SignatureId(String),
    StructuralMechanismId(String),
    ExecutionProfileId(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExploreStreamResultField {
    pub name: String,
    pub value: ExploreStreamProjectedValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExploreStreamResultGroupRow {
    pub projection_ordinal: u128,
    pub fields: Vec<ExploreStreamResultField>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExploreStreamPreviewLimit {
    RowsPerView,
    RowsPerReport,
    RecordsPerView,
    RecordsPerReport,
    ValueNodesPerReport,
    ValueBytesPerReport,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExploreStreamPreviewStatus {
    Complete,
    Truncated {
        reason: ExploreStreamPreviewLimit,
        next_projection_ordinal: u128,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExploreStreamResultEvidence {
    pub spec_root: String,
    pub projection_root: String,
    pub projection_record_count: u128,
    pub publication_id: String,
    pub evidence_root: String,
    pub result_root: String,
}

/// A bounded, SELECT-authorized sample of one exact grouped result.
///
/// The full rows remain in the independently resumable NDJSON artifact. The
/// preview is presentation only: its fixed caps never participate in query,
/// journal, projection, or publication identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExploreStreamGroupedResultPreview {
    pub columns: Vec<String>,
    pub raw_groups: ExploreStreamCount,
    pub output_groups: ExploreStreamCount,
    pub rows: Vec<ExploreStreamResultGroupRow>,
    pub scanned_projection_records: u128,
    pub status: ExploreStreamPreviewStatus,
    pub evidence: ExploreStreamResultEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExploreStreamResultLayer {
    pub name: String,
    pub view_id: String,
    pub status: ExploreStreamLayerStatus,
    pub input_rows: ExploreStreamCount,
    pub projection_records: ExploreStreamCount,
    /// Number of bounded records appended during this invocation. Their
    /// values remain in the journal-owned projection and can be copied to
    /// NDJSON by a separate cursor without materializing one report array.
    pub projection_records_appended: u128,
    pub grouped_preview: Option<ExploreStreamGroupedResultPreview>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExploreStreamMechanismLayer {
    pub name: String,
    pub request_id: String,
    pub target: ExploreStreamMechanismTarget,
    pub status: ExploreStreamLayerStatus,
    pub target_cases: ExploreStreamCount,
    pub terminal_cases: ExploreStreamCount,
    pub incidence_cases: ExploreStreamCount,
    pub unavailable_cases: ExploreStreamCount,
    pub raw_signatures: ExploreStreamCount,
    pub structural_assignments: ExploreStreamCount,
    pub structural_mechanisms: ExploreStreamCount,
    pub execution_profiles: ExploreStreamCount,
    pub raw_closure_root: Option<String>,
    pub structural_closure_root: Option<String>,
    pub support_closure_root: Option<String>,
    pub support_closure_totals: Option<ExploreStreamMechanismSupportTotals>,
}

/// Bounded materialization progress for the optional public result directory.
///
/// Semantic completion and publication catch-up are deliberately separate:
/// the journal may already be exact while a large NDJSON view is still being
/// copied in resumable batches.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExploreStreamPublicationArtifact {
    pub key: String,
    pub name: String,
    pub kind: String,
    pub relative_path: PathBuf,
    pub published_lines: u128,
    pub published_bytes: u64,
    pub caught_up_to_journal_prefix: bool,
    pub prefix_digest: String,
    pub layer_roots: serde_json::Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExploreStreamPublication {
    pub output_directory: PathBuf,
    pub manifest_path: PathBuf,
    pub lines_appended: u64,
    pub source_ordinals_advanced: u64,
    pub artifacts_caught_up: usize,
    pub artifact_count: usize,
    pub artifacts: Vec<ExploreStreamPublicationArtifact>,
}

impl ExploreStreamPublication {
    pub const fn is_caught_up(&self) -> bool {
        self.artifacts_caught_up == self.artifact_count
    }
}

/// Invocation-local acceleration telemetry. The observer memo is never part
/// of the semantic journal, checkpoint, or result identity; these counters
/// only make it possible to verify that a large exact run is avoiding
/// redundant adjacent endpoint evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExploreStreamObserverMemoStats {
    pub enabled: bool,
    pub hits: u64,
    pub misses: u64,
    pub inserts: u64,
    pub evictions: u64,
    pub entries: usize,
    pub retained_canonical_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExploreStreamLayer {
    Result(ExploreStreamResultLayer),
    Mechanisms(ExploreStreamMechanismLayer),
}

/// Compact observation of one durable Explore prefix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExploreStreamSliceReport {
    pub schema_version: u32,
    pub query_name: String,
    pub identity: ExploreStreamIdentity,
    pub source_coverage: ExploreStreamSourceCoverage,
    pub lifecycle: ExploreStreamLifecycle,
    pub pause_reason: Option<ExploreStreamPauseReason>,
    pub checkpoint: ExploreStreamCheckpoint,
    pub semantic_batches_appended: u64,
    pub semantic_events_appended: u64,
    pub observer_memo: ExploreStreamObserverMemoStats,
    pub relation_closed: bool,
    pub find_closed: bool,
    pub analysis_closed: bool,
    pub counts: ExploreStreamPopulationCounts,
    pub analysis_scope_root: Option<String>,
    pub analysis_terminal_root: Option<String>,
    pub analysis_closure_set_root: Option<String>,
    pub layers: Vec<ExploreStreamLayer>,
    pub publication: Option<ExploreStreamPublication>,
}

#[derive(Debug, Clone)]
pub enum ExploreStreamPreparationError {
    Diagnostics(Vec<Diagnostic>),
    Selection(String),
    Execution(String),
}

impl std::fmt::Display for ExploreStreamPreparationError {
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

impl std::error::Error for ExploreStreamPreparationError {}

/// Compatibility wrapper for one cold preparation and one durable slice.
pub fn execute_checked_relational_stream_slice(
    statements: &[Stmt],
    source_dir: Option<String>,
    source: &str,
    query_name: Option<&str>,
    options: ExploreStreamSliceOptions,
) -> Result<ExploreStreamSliceReport, ExploreStreamPreparationError> {
    let ExploreStreamSliceOptions {
        run_state,
        output_directory,
        max_runtime,
        outer_containment,
    } = options;
    let epoch_options = ExploreStreamEpochOptions {
        run_state,
        output_directory,
        outer_containment,
    };
    validate_epoch_options(&epoch_options)?;
    let prepared = prepare_checked_relational_stream(statements, source_dir, source, query_name)?;
    let mut epoch = prepared.open_epoch(epoch_options)?;
    epoch.run_slice(max_runtime)
}

/// Check and lower one selected query into a reusable in-memory worker epoch.
pub fn prepare_checked_relational_stream(
    statements: &[Stmt],
    source_dir: Option<String>,
    source: &str,
    query_name: Option<&str>,
) -> Result<PreparedRelationalExplore, ExploreStreamPreparationError> {
    let started = Instant::now();
    trace_preparation_phase(started, "begin");
    let artifacts =
        TypeChecker::check_with_explore_artifacts(statements, source_dir.clone(), source);
    trace_preparation_phase(started, "checked program");
    if !artifacts.diagnostics.is_empty() {
        return Err(ExploreStreamPreparationError::Diagnostics(
            artifacts.diagnostics,
        ));
    }
    artifacts
        .validate_checked_runtime_entry_v1(statements, source_dir.as_deref())
        .map_err(ExploreStreamPreparationError::Execution)?;
    trace_preparation_phase(started, "validated runtime snapshot");

    let selected = select_checked_relational_query_index(&artifacts, query_name)?;
    let (checked, observer_memo_plan, mechanism_memo_plan) = artifacts
        .checked_exploration_query_with_memo_plans(selected)
        .map_err(|error| {
            ExploreStreamPreparationError::Execution(format!(
                "checked exploration boundary is unavailable: {error:?}"
            ))
        })?;
    trace_preparation_phase(started, "validated query and memo plan");
    // Heap allocation makes every checked-query expression address stable for
    // the lifetime of the prepared epoch. The expression runtime uses those
    // addresses only as process-local lookup keys.
    let owned_checked = Arc::new(checked.to_owned_checked_query());
    trace_preparation_phase(started, "owned selected query");
    let (catalog, definitions) = checked_expression_runtime_inputs(&artifacts)?;
    trace_preparation_phase(started, "rebuilt interpreter inputs");
    let catalog = Arc::new(catalog);
    let definitions = Arc::new(definitions);
    let mechanism_runtime = RelationalInterpreterMechanismReplayRuntime::from_checked_definitions(
        &artifacts,
        &checked,
        Arc::clone(&definitions),
        mechanism_memo_plan,
    )
    .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?;
    trace_preparation_phase(started, "built request mechanism catalog");
    let native_classifier_plan = native_classifier_checked_declarations(&artifacts, &checked)
        .and_then(|checked_declarations| {
            native_classifier_plan_v2_from_checked(
                &checked,
                checked_declarations,
                artifacts.compile_time_metadata_bindings.clone(),
            )
        });
    let native_classifier_shape_v2 = native_classifier_plan.is_some();
    trace_preparation_phase(started, "froze native classifier input");
    drop(artifacts);
    trace_preparation_phase(started, "released checking artifacts");

    let checked = owned_checked.view();
    let support_plan = RelationalSupportPlanner::from_checked(&checked)
        .and_then(|planner| planner.plan())
        .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?;
    trace_preparation_phase(started, "planned support");
    let classification_capsule = bind_relational_classification_capsule(&checked, &support_plan)?;
    let classification_call_cache_capacity = NonZeroUsize::new(CLASSIFICATION_CALL_CACHE_ENTRIES)
        .ok_or_else(|| {
        ExploreStreamPreparationError::Execution(
            "classification call-cache capacity must be positive".into(),
        )
    })?;
    let region_replay_authority = Arc::new(
        RelationalRegionReplayAuthority::new(
            Arc::clone(&owned_checked),
            support_plan.clone(),
            Arc::clone(&classification_capsule),
        )
        .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?,
    );
    let classification_evaluator = RefCell::new(RelationalClassificationEvaluatorBackend::new(
        classification_capsule,
        classification_call_cache_capacity,
    ));
    trace_preparation_phase(started, "bound classification capsule");
    let analysis_plan = RelationalAnalysisPlan::from_checked(&checked)
        .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?;
    trace_preparation_phase(started, "planned analysis");
    let expression_runtime = RelationalInterpreterExpressionRuntime::new(
        Arc::clone(&catalog),
        definitions.as_ref(),
        checked.closed_query,
        observer_memo_plan,
    )
    .map_err(ExploreStreamPreparationError::Execution)?;
    trace_preparation_phase(started, "built warm expression runtime");
    let contract = RelationalJournalContract::new(
        checked.relation_id(),
        checked.admission_id(),
        checked.question_id(),
        checked.transition_schemas().state_schema_id(),
        checked.transition_schemas().context_schema_id(),
        checked.transition_schemas().transition_type_id(),
        analysis_plan.producer_graph_digest().bytes(),
    );
    let publication_plan = RelationalPublicationPlan::from_checked(&checked, contract)
        .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?;
    trace_preparation_phase(started, "prepared publication");
    Ok(PreparedRelationalExplore {
        checked: owned_checked,
        support_plan,
        region_replay_authority,
        contract,
        publication_plan,
        expression_runtime,
        mechanism_runtime,
        classification_evaluator,
        native_classifier_plan,
        native_classifier_shape_v2,
        native_classifier: None,
        preparation_wall_time: started.elapsed(),
    })
}

fn trace_preparation_phase(started: Instant, phase: &str) {
    if std::env::var_os("FUTURUNA_EXPLORE_TRACE").is_some() {
        eprintln!(
            "Explore preparation: {phase}; elapsed={}ms",
            started.elapsed().as_millis()
        );
    }
}

impl RelationalExploreEpoch {
    /// Advance one logical slice while retaining preparation, evaluator memo,
    /// mechanism caches, and the durable writer fence for the next call.
    pub fn run_slice(
        &mut self,
        max_runtime: Option<Duration>,
    ) -> Result<ExploreStreamSliceReport, ExploreStreamPreparationError> {
        let budget = RelationalStreamSliceBudget::new(max_runtime)
            .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?;

        let PreparedRelationalExplore {
            checked,
            support_plan,
            region_replay_authority: _,
            contract: _,
            publication_plan,
            expression_runtime,
            mechanism_runtime,
            classification_evaluator,
            native_classifier_plan: _,
            native_classifier_shape_v2: _,
            native_classifier,
            preparation_wall_time: _,
        } = &mut self.prepared;
        let checked = checked.view();
        let driver = RelationalStreamDriver::from_checked_with_limits_and_classification_backends(
            &checked,
            support_plan,
            RelationalStreamDriverLimits::default(),
            native_classifier.clone(),
            Some(classification_evaluator),
        )
        .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?;
        let projection_starts = projection_lengths(
            self.durable
                .journal()
                .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?,
            &checked,
        )?;
        let outcome = run_relational_stream_slice_with_resources(
            &mut self.durable,
            &driver,
            expression_runtime,
            mechanism_runtime,
            &mut self.resources,
            &mut self.base_quantum_controller,
            budget,
        )
        .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?;

        let (observer_memo_enabled, observer_memo_stats) = expression_runtime.observer_memo_stats();
        let mut report = build_report(
            &self.durable,
            &checked,
            projection_starts,
            &outcome,
            ExploreStreamObserverMemoStats {
                enabled: observer_memo_enabled,
                hits: observer_memo_stats.hits,
                misses: observer_memo_stats.misses,
                inserts: observer_memo_stats.inserts,
                evictions: observer_memo_stats.evictions,
                entries: observer_memo_stats.entries,
                retained_canonical_bytes: observer_memo_stats.retained_canonical_bytes,
            },
        )?;
        if let Some(output_directory) = self.options.output_directory.as_ref() {
            let publication = publish_relational_result_artifacts(
                output_directory,
                &self.durable,
                publication_plan,
                &report,
                RelationalPublicationLimits::default(),
            )
            .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?;
            report.publication = Some(ExploreStreamPublication {
                output_directory: output_directory.clone(),
                manifest_path: publication.manifest_path().to_path_buf(),
                lines_appended: publication.lines_appended(),
                source_ordinals_advanced: publication.source_ordinals_advanced(),
                artifacts_caught_up: publication.artifacts_caught_up(),
                artifact_count: publication.artifact_count(),
                artifacts: publication
                    .artifacts()
                    .iter()
                    .map(|artifact| ExploreStreamPublicationArtifact {
                        key: artifact.key().to_string(),
                        name: artifact.name().to_string(),
                        kind: artifact.kind().to_string(),
                        relative_path: PathBuf::from(artifact.relative_path()),
                        published_lines: artifact.published_lines(),
                        published_bytes: artifact.published_bytes(),
                        caught_up_to_journal_prefix: artifact.caught_up_to_journal_prefix(),
                        prefix_digest: artifact.prefix_digest().to_string(),
                        layer_roots: artifact.layer_roots().clone(),
                    })
                    .collect(),
            });
        }
        Ok(report)
    }
}

fn exact_stream_outer_containment(
    receipt: Option<ExploreStreamOuterContainment>,
) -> Result<Option<ExactStreamOuterContainmentReceipt>, ExploreStreamPreparationError> {
    receipt
        .map(|receipt| {
            ExactStreamOuterContainmentReceipt::new(
                receipt.rust_heap_limit_bytes.get(),
                receipt.untracked_memory_reserve_bytes.get(),
                receipt.group_rss_limit_bytes.get(),
                receipt.available_memory_floor_bytes.get(),
            )
        })
        .transpose()
        .map_err(|reason| {
            ExploreStreamPreparationError::Execution(format!(
                "invalid outer resource-containment receipt: {}",
                reason.code()
            ))
        })
}

fn validate_epoch_options(
    options: &ExploreStreamEpochOptions,
) -> Result<(), ExploreStreamPreparationError> {
    if options.run_state.as_os_str().is_empty() {
        return Err(ExploreStreamPreparationError::Execution(
            "relational Explore run_state path must not be empty".into(),
        ));
    }
    if let Some(output_directory) = options.output_directory.as_deref() {
        validate_separate_output_directory(&options.run_state, output_directory)?;
    }
    Ok(())
}

fn validate_separate_output_directory(
    run_state: &std::path::Path,
    output_directory: &std::path::Path,
) -> Result<(), ExploreStreamPreparationError> {
    if output_directory.as_os_str().is_empty() {
        return Err(ExploreStreamPreparationError::Execution(
            "relational Explore output path must not be empty".into(),
        ));
    }
    let run_state = resolved_effective_path(run_state)?;
    let output_directory = resolved_effective_path(output_directory)?;
    if run_state == output_directory
        || run_state.starts_with(&output_directory)
        || output_directory.starts_with(&run_state)
    {
        return Err(ExploreStreamPreparationError::Execution(
            "relational Explore --output and --run-state must be separate directory trees".into(),
        ));
    }
    Ok(())
}

/// Resolve every existing ancestor (including symlinks) and then reattach the
/// not-yet-created suffix. This catches an output path routed back into the
/// run-state tree through `/tmp`-style or user-created aliases without
/// requiring either final directory to exist yet.
fn resolved_effective_path(
    path: &std::path::Path,
) -> Result<PathBuf, ExploreStreamPreparationError> {
    let normalized = normalized_absolute_path(path)?;
    let mut ancestor = normalized.clone();
    let mut suffix = Vec::new();
    loop {
        match std::fs::canonicalize(&ancestor) {
            Ok(mut resolved) => {
                for component in suffix.iter().rev() {
                    resolved.push(component);
                }
                return normalized_absolute_path(&resolved);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let component = ancestor.file_name().ok_or_else(|| {
                    ExploreStreamPreparationError::Execution(format!(
                        "cannot resolve relational Explore path `{}`",
                        normalized.display()
                    ))
                })?;
                suffix.push(component.to_os_string());
                if !ancestor.pop() {
                    return Err(ExploreStreamPreparationError::Execution(format!(
                        "cannot resolve relational Explore path `{}`",
                        normalized.display()
                    )));
                }
            }
            Err(error) => {
                return Err(ExploreStreamPreparationError::Execution(format!(
                    "cannot resolve relational Explore path `{}`: {error}",
                    normalized.display()
                )));
            }
        }
    }
}

fn normalized_absolute_path(
    path: &std::path::Path,
) -> Result<PathBuf, ExploreStreamPreparationError> {
    use std::path::Component;

    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(ExploreStreamPreparationError::Execution(
                        "relational Explore path escapes its filesystem root".into(),
                    ));
                }
            }
            Component::Normal(component) => normalized.push(component),
        }
    }
    Ok(normalized)
}

fn select_checked_relational_query_index(
    artifacts: &TypeCheckArtifacts,
    query_name: Option<&str>,
) -> Result<usize, ExploreStreamPreparationError> {
    if let Some(query_name) = query_name {
        return artifacts
            .exploration_universes
            .iter()
            .position(|candidate| candidate.name == query_name)
            .ok_or_else(|| {
                ExploreStreamPreparationError::Selection(format!(
                    "exploration `{query_name}` was not found"
                ))
            });
    }
    match artifacts.exploration_universes.as_slice() {
        [_] => Ok(0),
        [] => Err(ExploreStreamPreparationError::Selection(
            "the program contains no selectable exploration".into(),
        )),
        queries => Err(ExploreStreamPreparationError::Selection(format!(
            "the program contains multiple explorations; select one with --query ({})",
            queries
                .iter()
                .map(|query| query.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

fn checked_expression_runtime_inputs(
    artifacts: &TypeCheckArtifacts,
) -> Result<(calculate::TypeCatalog, super::GroundDefinitions), ExploreStreamPreparationError> {
    artifacts
        .checked_runtime_root_program_v1()
        .map_err(ExploreStreamPreparationError::Execution)?;
    let catalog =
        calculate::TypeCatalog::collect_checked_analysis_program(&artifacts.analysis_program)
            .map_err(|errors| ExploreStreamPreparationError::Execution(errors.join("; ")))?;
    let mut definitions = checked_ground_definitions(&artifacts.analysis_program)
        .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?;
    definitions.rule_dispatch_return_types = artifacts.rule_dispatch_return_types.clone();
    definitions.rule_dispatch_return_issues = artifacts.rule_dispatch_return_issues.clone();
    definitions.rule_dispatch_boolean_miss_safe_keys =
        artifacts.rule_dispatch_boolean_miss_safe_keys.clone();
    Ok((catalog, definitions))
}

fn projection_lengths(
    journal: &RelationalJournal,
    checked: &CheckedExploreQueryView<'_>,
) -> Result<Vec<usize>, ExploreStreamPreparationError> {
    let analysis = journal.analysis_state();
    checked
        .analysis_nodes()
        .map(|(_, identity)| match identity {
            CheckedExploreAnalysisIdentity::View { view_id } => match analysis {
                None => Ok(0),
                Some(state) => match (state.open_catalog(), state.closed_catalog()) {
                    (Some(open), None) => open
                        .result_projection(*view_id)
                        .map(|projection| projection.len())
                        .or_else(|error| {
                            match open.layer_status(RelationalAnalysisLayerId::Result(*view_id)) {
                                Some(RelationalAnalysisLayerStatus::ResultUnregistered) => Ok(0),
                                _ => Err(error),
                            }
                        })
                        .map_err(|error| {
                            ExploreStreamPreparationError::Execution(error.to_string())
                        }),
                    (None, Some(closed)) => closed
                        .snapshot()
                        .layer(RelationalAnalysisLayerId::Result(*view_id))
                        .and_then(|layer| match layer {
                            RelationalAnalysisLayerSnapshot::Result(result) => result
                                .state()
                                .projection()
                                .map(|projection| projection.records().len()),
                            RelationalAnalysisLayerSnapshot::Mechanisms(_) => None,
                        })
                        .ok_or_else(|| {
                            ExploreStreamPreparationError::Execution(
                                "closed analysis omitted a declared result layer".into(),
                            )
                        }),
                    _ => Err(ExploreStreamPreparationError::Execution(
                        "analysis state does not own exactly one catalog".into(),
                    )),
                },
            },
            CheckedExploreAnalysisIdentity::Mechanisms { .. } => Ok(0),
        })
        .collect()
}

fn build_report(
    durable: &RelationalDurableJournal,
    checked: &CheckedExploreQueryView<'_>,
    projection_starts: Vec<usize>,
    outcome: &RelationalStreamSliceOutcome,
    observer_memo: ExploreStreamObserverMemoStats,
) -> Result<ExploreStreamSliceReport, ExploreStreamPreparationError> {
    let journal = durable
        .journal()
        .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?;
    let scheduler = journal
        .scheduler_view()
        .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?;
    let progress = outcome.progress();
    let checkpoint = progress.checkpoint();
    let head = hex(checkpoint.head().bytes());
    let relation_enumeration_closed = scheduler.relation_enumeration_is_complete();
    let case_count = scheduler.case_count() as u128;
    let certified_case_cardinality = scheduler.certified_root_case_cardinality();
    let cases = match certified_case_cardinality {
        Some(certified) if case_count > certified => {
            return Err(ExploreStreamPreparationError::Execution(format!(
                "observed case count {case_count} exceeds certified root cardinality {certified}"
            )));
        }
        Some(certified) if relation_enumeration_closed && case_count != certified => {
            return Err(ExploreStreamPreparationError::Execution(format!(
                "closed relation case count {case_count} does not match certified root cardinality {certified}"
            )));
        }
        Some(certified) => ExploreStreamCount::Exact(certified),
        None => relation_count(case_count, relation_enumeration_closed),
    };
    let classification_progress = scheduler
        .classification_progress_counts()
        .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?;
    if let (Some(certified), Some(classified)) =
        (certified_case_cardinality, classification_progress)
    {
        if classified.candidates() != certified {
            return Err(ExploreStreamPreparationError::Execution(format!(
                "classified case candidate count {} does not match certified root cardinality {certified}",
                classified.candidates()
            )));
        }
    }
    let admission_classified = scheduler.admission_decision_count() as u128;
    let admitted = scheduler.admitted_count() as u128;
    let rejected = admission_classified.checked_sub(admitted).ok_or_else(|| {
        ExploreStreamPreparationError::Execution(
            "admitted case count exceeds the classified admission population".into(),
        )
    })?;
    let certified_admission_counts = match (
        certified_case_cardinality,
        scheduler.certified_root_admission_decision(),
    ) {
        (Some(total), Some(AdmissionDecision::Admitted)) => {
            if admission_classified > total || rejected != 0 {
                return Err(ExploreStreamPreparationError::Execution(
                    "concrete admission decisions contradict the certified uniformly admitted root"
                        .into(),
                ));
            }
            Some((total, total, 0))
        }
        (Some(total), Some(AdmissionDecision::Rejected)) => {
            if admission_classified > total || admitted != 0 {
                return Err(ExploreStreamPreparationError::Execution(
                    "concrete admission decisions contradict the certified uniformly rejected root"
                        .into(),
                ));
            }
            Some((total, 0, total))
        }
        _ => None,
    };
    let admission_closed_extensional =
        relation_enumeration_closed && admission_classified == case_count;
    let find_classified = scheduler.question_decision_count() as u128;
    let selected_observed = scheduler.selected_count() as u128;
    let not_selected_observed =
        find_classified
            .checked_sub(selected_observed)
            .ok_or_else(|| {
                ExploreStreamPreparationError::Execution(
                    "selected case count exceeds the classified FIND population".into(),
                )
            })?;
    let find_closed_extensional = admission_closed_extensional && find_classified == admitted;
    let selected_seal = journal
        .analysis_state()
        .and_then(|analysis| analysis.selected_question());
    let certified_selected_count = selected_seal.map(|seal| seal.mechanism_target().count());
    let support_complete = classification_progress.is_some_and(|counts| counts.is_complete());
    let relation_closed = relation_enumeration_closed || support_complete;
    let find_closed = selected_seal.is_some() || find_closed_extensional || support_complete;
    let sources_observed = scheduler.source_count() as u128;
    let source_enumeration_closed = scheduler.source_enumeration_is_closed();
    let certified_source_cardinality = scheduler
        .certified_source_population()
        .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?
        .map(|binding| binding.exact_cardinality());
    let sources = merge_population_count(
        "source candidates",
        None,
        sources_observed,
        source_enumeration_closed,
        certified_source_cardinality,
    )?;

    let admission_classified_count = merge_population_count(
        "admission-classified cases",
        classification_progress.map(|counts| (counts.classified(), counts.is_complete())),
        admission_classified,
        admission_closed_extensional,
        certified_admission_counts.map(|(classified, _, _)| classified),
    )?;
    let admitted_count = merge_population_count(
        "admitted cases",
        classification_progress.map(|counts| (counts.admitted(), counts.is_complete())),
        admitted,
        admission_closed_extensional,
        certified_admission_counts.map(|(_, admitted, _)| admitted),
    )?;
    let rejected_count = merge_population_count(
        "rejected cases",
        classification_progress.map(|counts| (counts.rejected(), counts.is_complete())),
        rejected,
        admission_closed_extensional,
        certified_admission_counts.map(|(_, _, rejected)| rejected),
    )?;
    let find_classified_count = merge_population_count(
        "FIND-classified cases",
        classification_progress.map(|counts| (counts.admitted(), counts.is_complete())),
        find_classified,
        find_closed_extensional,
        None,
    )?;
    let selected_count = merge_population_count(
        "selected cases",
        classification_progress.map(|counts| (counts.admitted_selected(), counts.is_complete())),
        selected_observed,
        find_closed_extensional,
        certified_selected_count,
    )?;
    let not_selected_count = merge_population_count(
        "not-selected cases",
        classification_progress
            .map(|counts| (counts.admitted_not_selected(), counts.is_complete())),
        not_selected_observed,
        find_closed_extensional,
        None,
    )?;

    let analysis_scope_root = scheduler
        .analysis_scope_root()
        .map(|root| hex(root.bytes()));
    let analysis_terminal_root = journal
        .analysis_state()
        .and_then(|analysis| analysis.closed_catalog())
        .map(|catalog| hex(catalog.root().bytes()));
    let analysis_closure_set_root = scheduler
        .analysis_closure_set_root()
        .map(|root| hex(root.bytes()));
    let layers = analysis_layers(journal, checked, &projection_starts)?;
    let pause_reason = outcome.pause_reason().map(public_pause_reason);
    let lifecycle = if matches!(outcome, RelationalStreamSliceOutcome::Complete { .. }) {
        ExploreStreamLifecycle::Complete
    } else {
        ExploreStreamLifecycle::Paused
    };
    Ok(ExploreStreamSliceReport {
        schema_version: EXPLORE_RELATIONAL_STREAM_REPORT_VERSION,
        query_name: checked.closed_query.name.clone(),
        identity: ExploreStreamIdentity {
            checked_program: checked.program_hash().to_string(),
            relation_id: hex(checked.relation_id().bytes()),
            admission_id: hex(checked.admission_id().bytes()),
            question_id: hex(checked.question_id().bytes()),
            analysis_graph_digest: checked.analysis_graph_hash().to_string(),
            journal_id: hex(journal.contract().id().bytes()),
        },
        source_coverage: public_source_coverage(checked.source_coverage()),
        lifecycle,
        pause_reason,
        checkpoint: ExploreStreamCheckpoint {
            next_sequence: checkpoint.next_sequence(),
            journal_head: head,
            durable_segment_count: checkpoint.durable_segment_count(),
        },
        semantic_batches_appended: progress.semantic_batches_appended(),
        semantic_events_appended: progress.semantic_events_appended(),
        observer_memo,
        relation_closed,
        find_closed,
        analysis_closed: scheduler.analysis_is_closed(),
        counts: ExploreStreamPopulationCounts {
            sources,
            cases,
            admission_classified: admission_classified_count,
            admitted: admitted_count,
            rejected: rejected_count,
            find_classified: find_classified_count,
            selected: selected_count,
            not_selected: not_selected_count,
        },
        analysis_scope_root,
        analysis_terminal_root,
        analysis_closure_set_root,
        layers,
        publication: None,
    })
}

fn public_source_coverage(
    manifest: &CheckedExploreSourceCoverageManifest,
) -> ExploreStreamSourceCoverage {
    ExploreStreamSourceCoverage {
        version: manifest.version,
        manifest_digest: manifest.manifest_digest.to_string(),
        semantic_dependency_digest: hex(manifest.semantic_dependency_digest),
        has_gaps: manifest.has_coverage_gaps(),
        entries: manifest
            .entries
            .iter()
            .map(|entry| ExploreStreamCoverageEntry {
                subject_id: hex(entry.subject_id),
                subject: public_coverage_subject(&entry.subject),
                classification: public_coverage_classification(&entry.classification),
            })
            .collect(),
    }
}

fn public_coverage_subject(
    subject: &CheckedExploreCoverageSubject,
) -> ExploreStreamCoverageSubject {
    match subject {
        CheckedExploreCoverageSubject::SourceBinding {
            binding_index,
            binding_name,
            role,
        } => ExploreStreamCoverageSubject::SourceBinding {
            binding_index: *binding_index,
            binding_name: binding_name.to_string(),
            role: public_coverage_binding_role(*role),
        },
        CheckedExploreCoverageSubject::SchemaRoot { role, type_name } => {
            ExploreStreamCoverageSubject::SchemaRoot {
                role: public_coverage_root_role(*role),
                type_name: type_name.to_string(),
            }
        }
        CheckedExploreCoverageSubject::SchemaField { role, path } => {
            ExploreStreamCoverageSubject::SchemaField {
                role: public_coverage_root_role(*role),
                path: path
                    .iter()
                    .map(|segment| ExploreStreamCoverageFieldPathSegment {
                        owner_type_name: segment.owner_type_name.to_string(),
                        variant_index: segment.variant_index,
                        field_index: segment.field_index,
                        variant_name: segment.variant_name.to_string(),
                        field_name: segment.field_name.to_string(),
                    })
                    .collect(),
            }
        }
        CheckedExploreCoverageSubject::Literal { kind, value } => {
            ExploreStreamCoverageSubject::Literal {
                kind: public_coverage_literal_kind(*kind),
                value: value.to_string(),
            }
        }
        CheckedExploreCoverageSubject::TopLevelConstant {
            dependency_digest,
            addresses,
        } => ExploreStreamCoverageSubject::TopLevelConstant {
            dependency_digest: hex(*dependency_digest),
            addresses: addresses
                .iter()
                .map(|address| address.to_string())
                .collect(),
        },
        CheckedExploreCoverageSubject::ConstructorChoice {
            owner_digest,
            owner_name,
            variant_name,
            variant_index,
            layout,
        } => ExploreStreamCoverageSubject::ConstructorChoice {
            owner_digest: hex(*owner_digest),
            owner_name: owner_name.to_string(),
            variant_name: variant_name.to_string(),
            variant_index: *variant_index,
            layout: match layout {
                CheckedConstructorLayout::Positional => {
                    ExploreStreamCoverageConstructorLayout::Positional
                }
                CheckedConstructorLayout::Named => ExploreStreamCoverageConstructorLayout::Named,
            },
        },
    }
}

fn public_coverage_classification(
    classification: &CheckedExploreCoverageClassification,
) -> ExploreStreamCoverageClassification {
    match classification {
        CheckedExploreCoverageClassification::VariedFiniteDimension { dimension_id } => {
            ExploreStreamCoverageClassification::VariedFiniteDimension {
                dimension_id: hex(*dimension_id),
            }
        }
        CheckedExploreCoverageClassification::DerivedFromDeclaredDimensions { dimension_ids } => {
            ExploreStreamCoverageClassification::DerivedFromDeclaredDimensions {
                dimension_ids: dimension_ids.iter().map(|id| hex(*id)).collect(),
            }
        }
        CheckedExploreCoverageClassification::ConditionedSingletonOrSourceRestriction => {
            ExploreStreamCoverageClassification::ConditionedSingletonOrSourceRestriction
        }
        CheckedExploreCoverageClassification::ExactIrrelevanceCertificate {
            certificate_digest,
        } => ExploreStreamCoverageClassification::ExactIrrelevanceCertificate {
            certificate_digest: hex(*certificate_digest),
        },
        CheckedExploreCoverageClassification::CoverageGap { reason } => {
            ExploreStreamCoverageClassification::CoverageGap {
                reason: public_coverage_gap_reason(*reason),
            }
        }
    }
}

const fn public_coverage_root_role(
    role: CheckedExploreCoverageRootRole,
) -> ExploreStreamCoverageRootRole {
    match role {
        CheckedExploreCoverageRootRole::Context => ExploreStreamCoverageRootRole::Context,
        CheckedExploreCoverageRootRole::Before => ExploreStreamCoverageRootRole::Before,
    }
}

const fn public_coverage_binding_role(
    role: CheckedExploreCoverageBindingRole,
) -> ExploreStreamCoverageBindingRole {
    match role {
        CheckedExploreCoverageBindingRole::Auxiliary => ExploreStreamCoverageBindingRole::Auxiliary,
        CheckedExploreCoverageBindingRole::Context => ExploreStreamCoverageBindingRole::Context,
        CheckedExploreCoverageBindingRole::Before => ExploreStreamCoverageBindingRole::Before,
    }
}

const fn public_coverage_literal_kind(
    kind: CheckedExploreCoverageLiteralKind,
) -> ExploreStreamCoverageLiteralKind {
    match kind {
        CheckedExploreCoverageLiteralKind::Integer => ExploreStreamCoverageLiteralKind::Integer,
        CheckedExploreCoverageLiteralKind::FloatBits => ExploreStreamCoverageLiteralKind::FloatBits,
        CheckedExploreCoverageLiteralKind::String => ExploreStreamCoverageLiteralKind::String,
        CheckedExploreCoverageLiteralKind::Character => ExploreStreamCoverageLiteralKind::Character,
        CheckedExploreCoverageLiteralKind::Boolean => ExploreStreamCoverageLiteralKind::Boolean,
        CheckedExploreCoverageLiteralKind::Unit => ExploreStreamCoverageLiteralKind::Unit,
    }
}

const fn public_coverage_gap_reason(
    reason: CheckedExploreCoverageGapReason,
) -> ExploreStreamCoverageGapReason {
    match reason {
        CheckedExploreCoverageGapReason::SchemaNotDeclaredRecord => {
            ExploreStreamCoverageGapReason::SchemaNotDeclaredRecord
        }
        CheckedExploreCoverageGapReason::SchemaCompositionUnavailable => {
            ExploreStreamCoverageGapReason::SchemaCompositionUnavailable
        }
        CheckedExploreCoverageGapReason::InterproceduralFieldProvenance => {
            ExploreStreamCoverageGapReason::InterproceduralFieldProvenance
        }
        CheckedExploreCoverageGapReason::ConstructorFieldMappingUnavailable => {
            ExploreStreamCoverageGapReason::ConstructorFieldMappingUnavailable
        }
        CheckedExploreCoverageGapReason::ConstructorChoiceProvenanceUnavailable => {
            ExploreStreamCoverageGapReason::ConstructorChoiceProvenanceUnavailable
        }
        CheckedExploreCoverageGapReason::UpstreamCoverageGap => {
            ExploreStreamCoverageGapReason::UpstreamCoverageGap
        }
    }
}

fn analysis_layers(
    journal: &RelationalJournal,
    checked: &CheckedExploreQueryView<'_>,
    projection_starts: &[usize],
) -> Result<Vec<ExploreStreamLayer>, ExploreStreamPreparationError> {
    let analysis = journal.analysis_state();
    let mut layers = Vec::new();
    let mut preview_budget = ExploreStreamPreviewBudget::default();
    for (index, (node, identity)) in checked.analysis_nodes().enumerate() {
        let name = node.name().to_string();
        let layer = match (node, identity) {
            (
                ExploreAnalysisNodeIr::Result(_),
                CheckedExploreAnalysisIdentity::View { view_id },
            ) => ExploreStreamLayer::Result(result_layer(
                analysis,
                name,
                *view_id,
                projection_starts.get(index).copied().unwrap_or(0),
                &mut preview_budget,
            )?),
            (
                ExploreAnalysisNodeIr::Mechanisms(request),
                CheckedExploreAnalysisIdentity::Mechanisms { request_id, .. },
            ) => ExploreStreamLayer::Mechanisms(mechanism_layer(
                analysis,
                name,
                *request_id,
                public_mechanism_target(checked, &request.target)?,
            )?),
            _ => {
                return Err(ExploreStreamPreparationError::Execution(format!(
                    "checked analysis identity kind diverged at node {index}"
                )))
            }
        };
        layers.push(layer);
    }
    Ok(layers)
}

fn public_mechanism_target(
    checked: &CheckedExploreQueryView<'_>,
    target: &ExploreMechanismTargetIr,
) -> Result<ExploreStreamMechanismTarget, ExploreStreamPreparationError> {
    match target {
        ExploreMechanismTargetIr::SelectedCases => Ok(ExploreStreamMechanismTarget::Selected),
        ExploreMechanismTargetIr::ViewChosen { view_node_index } => {
            let (_, identity) =
                checked
                    .analysis_nodes()
                    .nth(*view_node_index)
                    .ok_or_else(|| {
                        ExploreStreamPreparationError::Execution(format!(
                            "mechanism target names missing analysis node {view_node_index}"
                        ))
                    })?;
            let CheckedExploreAnalysisIdentity::View { view_id } = identity else {
                return Err(ExploreStreamPreparationError::Execution(format!(
                    "mechanism target analysis node {view_node_index} is not a result view"
                )));
            };
            Ok(ExploreStreamMechanismTarget::ChosenView {
                view_id: hex(view_id.bytes()),
            })
        }
    }
}

#[derive(Default)]
struct ExploreStreamPreviewBudget {
    rows: usize,
    records: u128,
    value_nodes: usize,
    value_bytes: usize,
}

fn result_layer(
    analysis: Option<&super::RelationalAnalysisJournalState>,
    name: String,
    view_id: super::ViewId,
    start: usize,
    preview_budget: &mut ExploreStreamPreviewBudget,
) -> Result<ExploreStreamResultLayer, ExploreStreamPreparationError> {
    let absent = || ExploreStreamResultLayer {
        name: name.clone(),
        view_id: hex(view_id.bytes()),
        status: ExploreStreamLayerStatus::ResultUnregistered,
        input_rows: ExploreStreamCount::LowerBound(0),
        projection_records: ExploreStreamCount::LowerBound(0),
        projection_records_appended: 0,
        grouped_preview: None,
    };
    let Some(analysis) = analysis else {
        return Ok(absent());
    };
    let published_output_groups = analysis
        .result_projection_closure(view_id)
        .and_then(|closure| closure.counts().output_groups());
    match (analysis.open_catalog(), analysis.closed_catalog()) {
        (Some(open), None) => {
            let status = open
                .layer_status(RelationalAnalysisLayerId::Result(view_id))
                .ok_or_else(|| {
                    ExploreStreamPreparationError::Execution(
                        "analysis omitted a declared result layer".into(),
                    )
                })?;
            if status == RelationalAnalysisLayerStatus::ResultUnregistered {
                return Ok(absent());
            }
            let evidence = open
                .result_evidence(view_id)
                .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?;
            let projection = open
                .result_projection(view_id)
                .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?;
            let spec = open
                .result_spec(view_id)
                .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?;
            let publication = open
                .result_publication(view_id)
                .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?;
            let grouped_preview = grouped_result_preview(
                spec,
                projection.len() as u128,
                projection.root().bytes(),
                projection.records(),
                publication,
                published_output_groups,
                preview_budget,
            )?;
            Ok(ExploreStreamResultLayer {
                name,
                view_id: hex(view_id.bytes()),
                status: layer_status(status),
                input_rows: relation_count(evidence.logical_len(), evidence.input_is_sealed()),
                projection_records: relation_count(
                    projection.len() as u128,
                    status == RelationalAnalysisLayerStatus::ResultPublished,
                ),
                projection_records_appended: projection_suffix_len(projection.len(), start)?,
                grouped_preview,
            })
        }
        (None, Some(closed)) => {
            let layer = closed
                .snapshot()
                .layer(RelationalAnalysisLayerId::Result(view_id))
                .ok_or_else(|| {
                    ExploreStreamPreparationError::Execution(
                        "closed analysis omitted a declared result layer".into(),
                    )
                })?;
            let RelationalAnalysisLayerSnapshot::Result(result) = layer else {
                return Err(ExploreStreamPreparationError::Execution(
                    "closed analysis result identity names a mechanism layer".into(),
                ));
            };
            let RelationalResultLayerSnapshotState::Registered {
                spec,
                evidence,
                projection,
                publication,
                ..
            } = result.state()
            else {
                return Ok(absent());
            };
            let grouped_preview = grouped_result_preview(
                spec,
                projection.records().len() as u128,
                projection.root().bytes(),
                projection.records().iter(),
                *publication,
                published_output_groups,
                preview_budget,
            )?;
            Ok(ExploreStreamResultLayer {
                name,
                view_id: hex(view_id.bytes()),
                status: layer_status(result.status()),
                input_rows: relation_count(evidence.logical_len(), evidence.input_is_sealed()),
                projection_records: relation_count(
                    projection.records().len() as u128,
                    publication.is_some(),
                ),
                projection_records_appended: projection_suffix_len(
                    projection.records().len(),
                    start,
                )?,
                grouped_preview,
            })
        }
        _ => Err(ExploreStreamPreparationError::Execution(
            "analysis state does not own exactly one catalog".into(),
        )),
    }
}

fn grouped_result_preview<'a>(
    spec: &ResultViewSpec,
    projection_record_count: u128,
    projection_root: [u8; 32],
    records: impl Iterator<Item = &'a IndexedResultProjectionRecord>,
    publication: Option<RelationalResultPublication>,
    published_output_groups: Option<ResultViewCount>,
    budget: &mut ExploreStreamPreviewBudget,
) -> Result<Option<ExploreStreamGroupedResultPreview>, ExploreStreamPreparationError> {
    if !spec.grain().is_grouped() || spec.choice().is_some() {
        return Ok(None);
    }
    let Some(publication) = publication else {
        return Ok(None);
    };
    let exact_output_groups = match published_output_groups {
        Some(ResultViewCount::Exact(value)) => value,
        Some(ResultViewCount::LowerBound(_) | ResultViewCount::Provisional(_)) => {
            return Err(preview_error(
                "published grouped result retained a non-exact output-group count",
            ))
        }
        None => {
            return Err(preview_error(
                "published grouped result omitted its authenticated closure count",
            ))
        }
    };

    let columns = spec
        .projection_names()
        .iter()
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    let mut scanned_projection_records = 0_u128;
    let mut status = ExploreStreamPreviewStatus::Complete;

    for indexed in records {
        let next_projection_ordinal = indexed.ordinal();
        let limit = if scanned_projection_records >= RESULT_PREVIEW_RECORDS_PER_VIEW {
            Some(ExploreStreamPreviewLimit::RecordsPerView)
        } else if budget.records >= RESULT_PREVIEW_RECORDS_PER_REPORT {
            Some(ExploreStreamPreviewLimit::RecordsPerReport)
        } else {
            None
        };
        if let Some(reason) = limit {
            status = ExploreStreamPreviewStatus::Truncated {
                reason,
                next_projection_ordinal,
            };
            break;
        }
        scanned_projection_records = scanned_projection_records
            .checked_add(1)
            .ok_or_else(|| preview_error("projection scan count overflow"))?;
        budget.records = budget
            .records
            .checked_add(1)
            .ok_or_else(|| preview_error("report projection scan count overflow"))?;

        let ResultProjectionRecord::Group(group) = indexed.record() else {
            return Err(preview_error(
                "published grouped no-choice view contains a non-group projection record",
            ));
        };
        match group.disposition() {
            ResultGroupDisposition::ExactExcluded => continue,
            ResultGroupDisposition::Provisional { .. } => {
                return Err(preview_error(
                    "published grouped result contains a provisional group",
                ))
            }
            ResultGroupDisposition::ExactIncluded => {}
        }
        let values = group.projected_values().ok_or_else(|| {
            preview_error("published grouped no-choice result omitted SELECT values")
        })?;
        if values.len() != columns.len() {
            return Err(preview_error(
                "published grouped result SELECT names and values diverged",
            ));
        }

        let reason = if rows.len() >= RESULT_PREVIEW_ROWS_PER_VIEW {
            Some(ExploreStreamPreviewLimit::RowsPerView)
        } else if budget.rows >= RESULT_PREVIEW_ROWS_PER_REPORT {
            Some(ExploreStreamPreviewLimit::RowsPerReport)
        } else {
            let (value_nodes, value_bytes) = result_values_weight(values)
                .ok_or_else(|| preview_error("grouped result preview weight overflow"))?;
            if budget
                .value_nodes
                .checked_add(value_nodes)
                .is_none_or(|total| total > RESULT_PREVIEW_VALUE_NODES_PER_REPORT)
            {
                Some(ExploreStreamPreviewLimit::ValueNodesPerReport)
            } else if budget
                .value_bytes
                .checked_add(value_bytes)
                .is_none_or(|total| total > RESULT_PREVIEW_VALUE_BYTES_PER_REPORT)
            {
                Some(ExploreStreamPreviewLimit::ValueBytesPerReport)
            } else {
                budget.value_nodes += value_nodes;
                budget.value_bytes += value_bytes;
                None
            }
        };
        if let Some(reason) = reason {
            status = ExploreStreamPreviewStatus::Truncated {
                reason,
                next_projection_ordinal,
            };
            break;
        }

        let fields = columns
            .iter()
            .zip(values)
            .map(|(name, value)| ExploreStreamResultField {
                name: name.clone(),
                value: public_projected_value(value),
            })
            .collect();
        rows.push(ExploreStreamResultGroupRow {
            projection_ordinal: indexed.ordinal(),
            fields,
        });
        budget.rows += 1;
    }

    if matches!(status, ExploreStreamPreviewStatus::Complete)
        && scanned_projection_records != projection_record_count
    {
        return Err(preview_error(
            "published grouped result projection length diverged during reporting",
        ));
    }
    if matches!(status, ExploreStreamPreviewStatus::Complete)
        && rows.len() as u128 != exact_output_groups
    {
        return Err(preview_error(
            "published grouped result closure count diverged from its projection",
        ));
    }
    Ok(Some(ExploreStreamGroupedResultPreview {
        columns,
        raw_groups: ExploreStreamCount::Exact(projection_record_count),
        output_groups: ExploreStreamCount::Exact(exact_output_groups),
        rows,
        scanned_projection_records,
        status,
        evidence: ExploreStreamResultEvidence {
            spec_root: hex(publication.spec_root().bytes()),
            projection_root: hex(projection_root),
            projection_record_count,
            publication_id: hex(publication.id().bytes()),
            evidence_root: hex(publication.evidence_root().bytes()),
            result_root: hex(publication.result_root().bytes()),
        },
    }))
}

fn preview_error(message: impl Into<String>) -> ExploreStreamPreparationError {
    ExploreStreamPreparationError::Execution(message.into())
}

fn public_projected_value(value: &ResultValue) -> ExploreStreamProjectedValue {
    match value {
        ResultValue::Value(value) => ExploreStreamProjectedValue::Value(value.clone()),
        ResultValue::CaseId(id) => ExploreStreamProjectedValue::CaseId(hex(id.bytes())),
        ResultValue::TransitionId(id) => ExploreStreamProjectedValue::TransitionId(hex(id.bytes())),
        ResultValue::SignatureId(id) => ExploreStreamProjectedValue::SignatureId(hex(id.bytes())),
        ResultValue::StructuralMechanismId(id) => {
            ExploreStreamProjectedValue::StructuralMechanismId(hex(id.bytes()))
        }
        ResultValue::ExecutionProfileId(id) => {
            ExploreStreamProjectedValue::ExecutionProfileId(hex(id.bytes()))
        }
    }
}

fn result_values_weight(values: &[ResultValue]) -> Option<(usize, usize)> {
    values.iter().try_fold((0_usize, 0_usize), |total, value| {
        let weight = match value {
            ResultValue::Value(value) => explore_value_weight(value)?,
            ResultValue::CaseId(_)
            | ResultValue::TransitionId(_)
            | ResultValue::SignatureId(_)
            | ResultValue::StructuralMechanismId(_)
            | ResultValue::ExecutionProfileId(_) => (1, 64),
        };
        Some((
            total.0.checked_add(weight.0)?,
            total.1.checked_add(weight.1)?,
        ))
    })
}

fn explore_value_weight(value: &super::ExploreValue) -> Option<(usize, usize)> {
    use super::ExploreValue;

    match value {
        ExploreValue::Int(_) | ExploreValue::FloatBits(_) => Some((1, 8)),
        ExploreValue::String(value) => Some((1, value.len())),
        ExploreValue::Character(_) => Some((1, 4)),
        ExploreValue::Boolean(_) => Some((1, 1)),
        ExploreValue::Unit => Some((1, 0)),
        ExploreValue::List(values) | ExploreValue::Set(values) | ExploreValue::Tuple(values) => {
            values.iter().try_fold((1_usize, 0_usize), |total, value| {
                let weight = explore_value_weight(value)?;
                Some((
                    total.0.checked_add(weight.0)?,
                    total.1.checked_add(weight.1)?,
                ))
            })
        }
        ExploreValue::Constructor {
            type_name,
            variant,
            fields,
            ..
        } => fields.iter().try_fold(
            (1_usize, type_name.len().checked_add(variant.len())?),
            |total, (name, value)| {
                let weight = explore_value_weight(value)?;
                Some((
                    total.0.checked_add(weight.0)?,
                    total.1.checked_add(name.len())?.checked_add(weight.1)?,
                ))
            },
        ),
    }
}

fn mechanism_layer(
    analysis: Option<&super::RelationalAnalysisJournalState>,
    name: String,
    request_id: super::MechanismRequestId,
    target: ExploreStreamMechanismTarget,
) -> Result<ExploreStreamMechanismLayer, ExploreStreamPreparationError> {
    let absent = || ExploreStreamMechanismLayer {
        name: name.clone(),
        request_id: hex(request_id.bytes()),
        target: target.clone(),
        status: ExploreStreamLayerStatus::MechanismUnregistered,
        target_cases: ExploreStreamCount::Unknown {
            confirmed_lower_bound: 0,
        },
        terminal_cases: ExploreStreamCount::LowerBound(0),
        incidence_cases: ExploreStreamCount::LowerBound(0),
        unavailable_cases: ExploreStreamCount::LowerBound(0),
        raw_signatures: ExploreStreamCount::Unknown {
            confirmed_lower_bound: 0,
        },
        structural_assignments: ExploreStreamCount::LowerBound(0),
        structural_mechanisms: ExploreStreamCount::LowerBound(0),
        execution_profiles: ExploreStreamCount::LowerBound(0),
        raw_closure_root: None,
        structural_closure_root: None,
        support_closure_root: None,
        support_closure_totals: None,
    };
    let Some(analysis) = analysis else {
        return Ok(absent());
    };
    let (status, counts, known_raw_signatures) =
        match (analysis.open_catalog(), analysis.closed_catalog()) {
            (Some(open), None) => {
                let status = open
                    .layer_status(RelationalAnalysisLayerId::Mechanisms(request_id))
                    .ok_or_else(|| {
                        ExploreStreamPreparationError::Execution(
                            "analysis omitted a declared mechanism layer".into(),
                        )
                    })?;
                let incidence = open
                    .mechanism_incidence(request_id)
                    .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?;
                (
                    status,
                    incidence.counts(),
                    incidence.signature_definition_count() as u128,
                )
            }
            (None, Some(closed)) => {
                let layer = closed
                    .snapshot()
                    .layer(RelationalAnalysisLayerId::Mechanisms(request_id))
                    .ok_or_else(|| {
                        ExploreStreamPreparationError::Execution(
                            "closed analysis omitted a declared mechanism layer".into(),
                        )
                    })?;
                let RelationalAnalysisLayerSnapshot::Mechanisms(mechanism) = layer else {
                    return Err(ExploreStreamPreparationError::Execution(
                        "closed mechanism identity names a result layer".into(),
                    ));
                };
                (
                    mechanism.status(),
                    mechanism.incidence().counts(),
                    mechanism.incidence().signature_definitions().len() as u128,
                )
            }
            _ => {
                return Err(ExploreStreamPreparationError::Execution(
                    "analysis state does not own exactly one catalog".into(),
                ));
            }
        };

    let raw_closure = analysis.mechanism_closure(request_id);
    let structural_catalog = analysis.structural_mechanism_catalog(request_id);
    let structural_closure = analysis.structural_quotient_closure(request_id);
    let support_closure = analysis.mechanism_support_closure(request_id);
    let (known_assignments, known_structural_mechanisms, known_execution_profiles) =
        match structural_closure {
            Some(closure) => {
                let counts = closure.counts();
                (
                    counts.assignments(),
                    counts.mechanisms(),
                    counts.execution_profiles(),
                )
            }
            None => structural_catalog.map_or((0, 0, 0), |catalog| {
                (
                    catalog.assignment_count() as u128,
                    catalog.structural_mechanism_count() as u128,
                    catalog.execution_profile_count() as u128,
                )
            }),
        };
    let raw_unavailable = counts.unavailable_cases();
    Ok(ExploreStreamMechanismLayer {
        name,
        request_id: hex(request_id.bytes()),
        target,
        status: layer_status(status),
        target_cases: mechanism_count(counts.target_cases()),
        terminal_cases: mechanism_count(counts.terminal_cases()),
        incidence_cases: mechanism_count(counts.incidence_cases()),
        unavailable_cases: mechanism_count(counts.unavailable_cases()),
        raw_signatures: raw_signature_count(known_raw_signatures, status, raw_unavailable),
        structural_assignments: structural_count(
            known_assignments,
            structural_closure.is_some(),
            raw_unavailable,
        ),
        structural_mechanisms: structural_count(
            known_structural_mechanisms,
            structural_closure.is_some(),
            raw_unavailable,
        ),
        execution_profiles: structural_count(
            known_execution_profiles,
            structural_closure.is_some(),
            raw_unavailable,
        ),
        raw_closure_root: raw_closure.map(|closure| hex(closure.incidence_root().bytes())),
        structural_closure_root: structural_closure.map(|closure| hex(closure.root().bytes())),
        support_closure_root: support_closure.map(|closure| hex(closure.root().bytes())),
        support_closure_totals: support_closure.map(|closure| {
            ExploreStreamMechanismSupportTotals {
                target_cases: closure.target_case_count(),
                successful_cases: closure.successful_case_count(),
                unavailable_cases: closure.unavailable_case_count(),
                signature_fibers: closure.signature_fiber_count(),
                target_starters: closure.target_starter_count(),
            }
        }),
    })
}

fn projection_suffix_len(
    current: usize,
    invocation_start: usize,
) -> Result<u128, ExploreStreamPreparationError> {
    let appended = current.checked_sub(invocation_start).ok_or_else(|| {
        ExploreStreamPreparationError::Execution(
            "durable result projection moved behind its invocation cursor".into(),
        )
    })?;
    Ok(appended as u128)
}

fn public_pause_reason(reason: RelationalStreamSlicePauseReason) -> ExploreStreamPauseReason {
    match reason {
        RelationalStreamSlicePauseReason::RuntimeLimit => ExploreStreamPauseReason::RuntimeLimit,
        RelationalStreamSlicePauseReason::ResourceAdmission { code } => {
            ExploreStreamPauseReason::ResourceAdmission { code: code.into() }
        }
        RelationalStreamSlicePauseReason::Semantic(reason) => match reason {
            RelationalStreamQuiescence::MechanismReplayPaused {
                request_id,
                case_id,
                endpoint,
                reason,
            } => ExploreStreamPauseReason::MechanismReplay {
                request_id: hex(request_id.bytes()),
                case_id: hex(case_id.bytes()),
                endpoint: format!("{endpoint:?}"),
                reason: format!("{reason:?}"),
            },
            RelationalStreamQuiescence::AwaitingChosenViewMechanisms {
                request_id,
                view_id,
            } => ExploreStreamPauseReason::AwaitingChosenViewMechanisms {
                request_id: hex(request_id.bytes()),
                view_id: hex(view_id.bytes()),
            },
            RelationalStreamQuiescence::AwaitingSourceResult { view_id } => {
                ExploreStreamPauseReason::AwaitingSourceResult {
                    view_id: hex(view_id.bytes()),
                }
            }
            RelationalStreamQuiescence::AwaitingMechanismIncidenceResult {
                view_id,
                request_id,
            } => ExploreStreamPauseReason::AwaitingMechanismIncidenceResult {
                view_id: hex(view_id.bytes()),
                request_id: hex(request_id.bytes()),
            },
        },
    }
}

const fn layer_status(status: RelationalAnalysisLayerStatus) -> ExploreStreamLayerStatus {
    match status {
        RelationalAnalysisLayerStatus::ResultUnregistered => {
            ExploreStreamLayerStatus::ResultUnregistered
        }
        RelationalAnalysisLayerStatus::ResultInputOpen => ExploreStreamLayerStatus::ResultInputOpen,
        RelationalAnalysisLayerStatus::ResultAwaitingPublication => {
            ExploreStreamLayerStatus::ResultAwaitingPublication
        }
        RelationalAnalysisLayerStatus::ResultPublished => ExploreStreamLayerStatus::ResultPublished,
        RelationalAnalysisLayerStatus::MechanismTargetOpen => {
            ExploreStreamLayerStatus::MechanismTargetOpen
        }
        RelationalAnalysisLayerStatus::MechanismTerminalOpen => {
            ExploreStreamLayerStatus::MechanismTerminalOpen
        }
        RelationalAnalysisLayerStatus::MechanismClosed => ExploreStreamLayerStatus::MechanismClosed,
    }
}

const fn relation_count(value: u128, exact: bool) -> ExploreStreamCount {
    if exact {
        ExploreStreamCount::Exact(value)
    } else {
        ExploreStreamCount::LowerBound(value)
    }
}

/// Merge independent counts over the same logical population without adding
/// overlapping observations. Sealed support and concrete catalogs each supply
/// a lower bound until one path proves completeness; an independent exact
/// certificate, when present, is the final authority. Every stronger claim is
/// cross-checked against the weaker observations before it is reported.
fn merge_population_count(
    label: &str,
    support: Option<(u128, bool)>,
    extensional: u128,
    extensional_complete: bool,
    certified_exact: Option<u128>,
) -> Result<ExploreStreamCount, ExploreStreamPreparationError> {
    let support_lower_bound = support.map_or(0, |(value, _)| value);
    let confirmed_lower_bound = support_lower_bound.max(extensional);

    if let Some(exact) = certified_exact {
        let support_exact_conflict =
            support.is_some_and(|(value, complete)| complete && value != exact);
        if confirmed_lower_bound > exact
            || support_exact_conflict
            || (extensional_complete && extensional != exact)
        {
            return Err(ExploreStreamPreparationError::Execution(format!(
                "{label} observations conflict with certified exact count {exact}"
            )));
        }
        return Ok(ExploreStreamCount::Exact(exact));
    }

    if let Some((support_exact, true)) = support {
        if extensional > support_exact || (extensional_complete && extensional != support_exact) {
            return Err(ExploreStreamPreparationError::Execution(format!(
                "extensional {label} count {extensional} conflicts with exact classified-support count {support_exact}"
            )));
        }
        return Ok(ExploreStreamCount::Exact(support_exact));
    }

    if extensional_complete {
        if support_lower_bound > extensional {
            return Err(ExploreStreamPreparationError::Execution(format!(
                "classified-support {label} lower bound {support_lower_bound} exceeds exact extensional count {extensional}"
            )));
        }
        return Ok(ExploreStreamCount::Exact(extensional));
    }

    Ok(ExploreStreamCount::LowerBound(confirmed_lower_bound))
}

const fn mechanism_count(count: MechanismCountEvidence) -> ExploreStreamCount {
    match count {
        MechanismCountEvidence::Unknown {
            confirmed_lower_bound,
        } => ExploreStreamCount::Unknown {
            confirmed_lower_bound,
        },
        MechanismCountEvidence::LowerBound(value) => ExploreStreamCount::LowerBound(value),
        MechanismCountEvidence::Exact(value) => ExploreStreamCount::Exact(value),
    }
}

const fn raw_signature_count(
    known: u128,
    status: RelationalAnalysisLayerStatus,
    unavailable: MechanismCountEvidence,
) -> ExploreStreamCount {
    if !matches!(status, RelationalAnalysisLayerStatus::MechanismClosed) {
        return if known == 0 {
            ExploreStreamCount::Unknown {
                confirmed_lower_bound: 0,
            }
        } else {
            ExploreStreamCount::LowerBound(known)
        };
    }
    closed_count_with_unavailable_residual(known, unavailable)
}

const fn structural_count(
    known: u128,
    structural_closed: bool,
    raw_unavailable: MechanismCountEvidence,
) -> ExploreStreamCount {
    if !structural_closed {
        return ExploreStreamCount::LowerBound(known);
    }
    closed_count_with_unavailable_residual(known, raw_unavailable)
}

const fn closed_count_with_unavailable_residual(
    known: u128,
    unavailable: MechanismCountEvidence,
) -> ExploreStreamCount {
    match unavailable {
        MechanismCountEvidence::Exact(0) => ExploreStreamCount::Exact(known),
        MechanismCountEvidence::Exact(unavailable) => match known.checked_add(unavailable) {
            Some(upper_bound) => ExploreStreamCount::Interval {
                lower_bound: known,
                upper_bound,
            },
            None => ExploreStreamCount::LowerBound(known),
        },
        MechanismCountEvidence::Unknown { .. } | MechanismCountEvidence::LowerBound(_) => {
            ExploreStreamCount::LowerBound(known)
        }
    }
}

fn hex(bytes: [u8; 32]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(64);
    for byte in bytes {
        encoded.push(DIGITS[(byte >> 4) as usize] as char);
        encoded.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    encoded
}

#[cfg(test)]
mod regional_stream_acceptance_tests {
    use std::fs;
    use std::num::NonZeroU16;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::super::relational_analysis_plan::RelationalAnalysisPlan;
    use super::super::relational_case_support_projection::{
        derive_relational_case_support_projection, RelationalCaseSupportClosureAuthority,
        RelationalCaseSupportCount, RelationalCaseSupportProjectionFrontier,
        RelationalCaseSupportProjectionRecord,
    };
    use super::super::relational_classified_sweep_step_driver::{
        RelationalClassifiedSweepStepDriver, RelationalClassifiedSweepStepOutcome,
    };
    use super::super::relational_durable_journal::{
        RelationalDurableJournal, RelationalDurableJournalLimits,
    };
    use super::super::relational_journal::{
        RelationalClassifiedSupportFragment, RelationalJournal, RelationalJournalEvent,
    };
    use super::super::relational_step_driver::{
        RelationalStepDriver, RelationalStepOutcome, RelationalStepQuantum,
    };
    use super::super::relational_stream_driver::{
        RelationalStreamDriver, RelationalStreamDriverLimits, RelationalStreamQuantum,
        RelationalStreamStepOutcome,
    };
    use super::*;
    use crate::{Lexer, Parser};

    const EXACT_EMPTY: &str = r#"
? explore regional_exact_empty {
    from {
        before in range(0, 300)
        context = ()
    }

    to after = before + 1
    find matches of after < before
}
"#;

    const UNIFORMLY_SELECTED: &str = r#"
? explore regional_uniformly_selected {
    from {
        before in range(0, 300)
        context = ()
    }

    to after = before + 1
    find matches of after > before
}
"#;

    const MIXED_FIRST_CHILD: &str = r#"
? explore regional_mixed_first_child {
    from {
        before in range(0, 300)
        context = ()
    }

    to after = before + 1
    find matches of before >= 128
}
"#;

    const UNSUPPORTED_NONLINEAR: &str = r#"
? explore regional_unsupported_nonlinear {
    from {
        before in range(0, 300)
        context = ()
    }

    to after = before + 1
    find matches of before * before < 0
}
"#;

    const HYBRID: &str = r#"
? explore regional_hybrid {
    from {
        before in range(0, 300)
        context = ()
    }

    to after = before + 1
    find matches of before >= 280
}
"#;

    fn parse(source: &str) -> Vec<Stmt> {
        let mut lexer = Lexer::new(source);
        Parser::new(lexer.tokenize(), source)
            .parse_program()
            .expect("parse regional stream fixture")
    }

    fn prepare(source: &str) -> PreparedRelationalExplore {
        let statements = parse(source);
        prepare_checked_relational_stream(&statements, None, source, None)
            .expect("prepare regional stream fixture")
    }

    fn append_base_batch(
        journal: &mut RelationalJournal,
        batch: super::super::relational_step_driver::RelationalStepBatch,
    ) {
        assert_eq!(journal.next_sequence(), batch.expected_sequence());
        assert_eq!(journal.head(), batch.expected_head());
        for event in batch.into_events() {
            journal.append(event).expect("append base scheduler event");
        }
    }

    fn first_classification_quantum(source: &str) -> RelationalStepQuantum {
        let mut prepared = prepare(source);
        let checked = prepared.checked.view();
        let analysis_plan =
            RelationalAnalysisPlan::from_checked(&checked).expect("plan fixture analysis");
        let mut journal = RelationalJournal::new_with_region_replay_authority(
            prepared.contract,
            Arc::clone(&prepared.region_replay_authority),
        );
        journal
            .append(RelationalJournalEvent::analysis_plan_registered(
                analysis_plan,
            ))
            .expect("register fixture analysis plan");
        let driver = RelationalStepDriver::from_checked_with_max_members_per_quantum_and_classification_backends(
            &checked,
            &prepared.support_plan,
            NonZeroU16::new(1).unwrap(),
            None,
            Some(&prepared.classification_evaluator),
        )
        .expect("build fixture base scheduler");

        for _ in 0..64 {
            let outcome = driver
                .step_with_max_members_per_quantum(
                    &journal,
                    &mut prepared.expression_runtime,
                    NonZeroU16::new(1).unwrap(),
                )
                .expect("advance fixture base scheduler");
            let RelationalStepOutcome::Emitted(batch) = outcome else {
                panic!("fixture quiesced before classifying its first child");
            };
            let quantum = batch.quantum();
            if matches!(
                quantum,
                RelationalStepQuantum::CertifiedRegion { .. }
                    | RelationalStepQuantum::ClassifiedSweep(_)
            ) {
                return quantum;
            }
            append_base_batch(&mut journal, batch);
        }
        panic!("fixture did not reach its first classified child");
    }

    #[test]
    fn scheduler_proves_before_concrete_and_falls_back_for_nonempty_unsupported_and_partial_children(
    ) {
        assert!(matches!(
            first_classification_quantum(EXACT_EMPTY),
            RelationalStepQuantum::CertifiedRegion {
                chunk_ordinal: 0,
                ..
            }
        ));
        for source in [UNIFORMLY_SELECTED, MIXED_FIRST_CHILD, UNSUPPORTED_NONLINEAR] {
            assert!(matches!(
                first_classification_quantum(source),
                RelationalStepQuantum::ClassifiedSweep(_)
            ));
        }

        let mut prepared = prepare(EXACT_EMPTY);
        let checked = prepared.checked.view();
        let analysis_plan =
            RelationalAnalysisPlan::from_checked(&checked).expect("plan partial fixture analysis");
        let mut journal = RelationalJournal::new_with_region_replay_authority(
            prepared.contract,
            Arc::clone(&prepared.region_replay_authority),
        );
        journal
            .append(RelationalJournalEvent::analysis_plan_registered(
                analysis_plan,
            ))
            .expect("register partial fixture analysis plan");
        let base = RelationalStepDriver::from_checked_with_max_members_per_quantum_and_classification_backends(
            &checked,
            &prepared.support_plan,
            NonZeroU16::new(1).unwrap(),
            None,
            Some(&prepared.classification_evaluator),
        )
        .expect("build partial fixture base scheduler");
        loop {
            let outcome = base
                .step_with_max_members_per_quantum(
                    &journal,
                    &mut prepared.expression_runtime,
                    NonZeroU16::new(1).unwrap(),
                )
                .expect("advance partial fixture setup");
            let RelationalStepOutcome::Emitted(batch) = outcome else {
                panic!("partial fixture quiesced before its proof opportunity");
            };
            if matches!(
                batch.quantum(),
                RelationalStepQuantum::CertifiedRegion { .. }
            ) {
                break;
            }
            append_base_batch(&mut journal, batch);
        }

        let concrete =
            RelationalClassifiedSweepStepDriver::from_checked_with_classification_backends(
                &checked,
                &prepared.support_plan,
                None,
                Some(&prepared.classification_evaluator),
            )
            .expect("build direct concrete classifier");
        let RelationalClassifiedSweepStepOutcome::Emitted(partial) = concrete
            .step(
                &journal,
                NonZeroU16::new(1).unwrap(),
                &mut prepared.expression_runtime,
            )
            .expect("checkpoint one concrete case")
        else {
            panic!("first concrete member unexpectedly exhausted the partition");
        };
        assert!(partial.quantum().classified_artifact_id().is_none());
        let (expected_sequence, expected_head) =
            (partial.expected_sequence(), partial.expected_head());
        assert_eq!(journal.next_sequence(), expected_sequence);
        assert_eq!(journal.head(), expected_head);
        for event in partial.into_events() {
            journal
                .append(event)
                .expect("append partial concrete checkpoint");
        }
        assert!(journal
            .scheduler_view()
            .expect("inspect partial fixture")
            .classified_chunk_accumulator()
            .is_some());

        let RelationalStepOutcome::Emitted(resumed) = base
            .step_with_max_members_per_quantum(
                &journal,
                &mut prepared.expression_runtime,
                NonZeroU16::new(1).unwrap(),
            )
            .expect("resume active concrete child")
        else {
            panic!("partial fixture quiesced before resuming its child");
        };
        assert!(matches!(
            resumed.quantum(),
            RelationalStepQuantum::ClassifiedSweep(_)
        ));
    }

    static TEMP_NONCE: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "futuruna-regional-stream-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("clock")
                    .as_nanos(),
                TEMP_NONCE.fetch_add(1, Ordering::Relaxed),
            ));
            fs::create_dir(&path).expect("create regional stream test directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            self.0.as_path()
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).ok();
        }
    }

    #[test]
    fn hybrid_stream_resumes_materializes_sparse_selected_and_projects_exact_public_closure() {
        let temp = TestDirectory::new();
        let run_state = temp.path().join("run-state");
        let mut prepared = prepare(HYBRID);
        let paused_checkpoint;

        {
            let checked = prepared.checked.view();
            let driver =
                RelationalStreamDriver::from_checked_with_limits_and_classification_backends(
                    &checked,
                    &prepared.support_plan,
                    RelationalStreamDriverLimits::default(),
                    None,
                    Some(&prepared.classification_evaluator),
                )
                .expect("build hybrid stream scheduler");
            let mut durable =
                RelationalDurableJournal::open_or_create_with_region_replay_authority(
                    &run_state,
                    prepared.contract,
                    RelationalDurableJournalLimits::default(),
                    Arc::clone(&prepared.region_replay_authority),
                )
                .expect("open hybrid durable journal");
            let mut preceding_base_classifications = 0usize;

            for _ in 0..64 {
                let outcome = driver
                    .step_with_base_member_limit(
                        durable
                            .journal_mut_for_event_planning()
                            .expect("borrow durable planning journal"),
                        &mut prepared.expression_runtime,
                        &mut prepared.mechanism_runtime,
                        NonZeroU16::new(256).unwrap(),
                    )
                    .expect("advance hybrid prefix");
                let RelationalStreamStepOutcome::Emitted(batch) = outcome else {
                    panic!("hybrid stream quiesced before its first certified child");
                };
                let quantum = batch.quantum();
                if matches!(
                    quantum,
                    RelationalStreamQuantum::Base(RelationalStepQuantum::ClassifiedSweep(_))
                ) {
                    preceding_base_classifications += 1;
                }
                durable
                    .append_events(
                        batch.expected_sequence(),
                        batch.expected_head(),
                        batch.into_events(),
                    )
                    .expect("append hybrid prefix batch");
                if matches!(
                    quantum,
                    RelationalStreamQuantum::Base(RelationalStepQuantum::CertifiedRegion {
                        chunk_ordinal: 0,
                        ..
                    })
                ) {
                    break;
                }
            }
            assert_eq!(preceding_base_classifications, 0);
            let view = durable
                .journal()
                .expect("inspect durable prefix")
                .scheduler_view()
                .expect("inspect hybrid prefix");
            assert!(matches!(
                view.classified_support_fragments(),
                [RelationalClassifiedSupportFragment::CertifiedZeroSelected(
                    _
                )]
            ));
            paused_checkpoint = durable
                .flush_for_pause()
                .expect("flush certified hybrid prefix");
        }

        let checked = prepared.checked.view();
        let driver = RelationalStreamDriver::from_checked_with_limits_and_classification_backends(
            &checked,
            &prepared.support_plan,
            RelationalStreamDriverLimits::default(),
            None,
            Some(&prepared.classification_evaluator),
        )
        .expect("rebuild hybrid stream scheduler after pause");
        let mut durable = RelationalDurableJournal::open_or_create_with_region_replay_authority(
            &run_state,
            prepared.contract,
            RelationalDurableJournalLimits::default(),
            Arc::clone(&prepared.region_replay_authority),
        )
        .expect("reopen hybrid durable journal");
        let reopened = durable.journal().expect("inspect reopened hybrid journal");
        assert_eq!(reopened.next_sequence(), paused_checkpoint.next_sequence());
        assert_eq!(reopened.head(), paused_checkpoint.head());
        assert!(matches!(
            reopened
                .scheduler_view()
                .expect("inspect replayed hybrid prefix")
                .classified_support_fragments(),
            [RelationalClassifiedSupportFragment::CertifiedZeroSelected(
                _
            )]
        ));

        let mut completed = false;
        for _ in 0..128 {
            match driver
                .step_with_base_member_limit(
                    durable
                        .journal_mut_for_event_planning()
                        .expect("borrow reopened planning journal"),
                    &mut prepared.expression_runtime,
                    &mut prepared.mechanism_runtime,
                    NonZeroU16::new(256).unwrap(),
                )
                .expect("resume hybrid stream")
            {
                RelationalStreamStepOutcome::Emitted(batch) => {
                    durable
                        .append_events(
                            batch.expected_sequence(),
                            batch.expected_head(),
                            batch.into_events(),
                        )
                        .expect("append resumed hybrid batch");
                }
                RelationalStreamStepOutcome::Complete => {
                    completed = true;
                    break;
                }
                RelationalStreamStepOutcome::Quiescent(quiescence) => {
                    panic!("hybrid stream quiesced before closure: {quiescence:?}");
                }
            }
        }
        assert!(
            completed,
            "hybrid stream exceeded its compact fixture bound"
        );
        durable
            .flush_for_pause()
            .expect("flush completed hybrid journal");

        let journal = durable.journal().expect("inspect completed hybrid journal");
        let view = journal
            .scheduler_view()
            .expect("inspect completed hybrid view");
        let fragments = view.classified_support_fragments();
        assert!(matches!(
            fragments,
            [
                RelationalClassifiedSupportFragment::CertifiedZeroSelected(_),
                RelationalClassifiedSupportFragment::Concrete(_)
            ]
        ));
        assert_eq!(
            fragments
                .iter()
                .map(|fragment| fragment.exact_case_count())
                .sum::<u128>(),
            300
        );
        assert_eq!(
            fragments
                .iter()
                .map(|fragment| fragment.admitted_selected_count())
                .sum::<u128>(),
            20
        );
        assert_eq!(view.selected_run_materializations().count(), 1);
        assert!(view.selected_run_materializations_cover_classified_prefix());
        assert!(view.support_catalog_is_sealed());
        assert!(journal
            .analysis_state()
            .is_some_and(|analysis| analysis.is_closed()));

        let selected_question = journal
            .analysis_state()
            .and_then(|analysis| analysis.selected_question())
            .expect("hybrid selected-question seal");
        let closure_authority =
            RelationalCaseSupportClosureAuthority::from_authenticated_certified_support(
                view.support_catalog_is_sealed(),
                view.certified_root_case_cardinality()
                    .expect("hybrid exact root cardinality"),
                view.support_evidence_root()
                    .expect("hybrid support evidence root"),
                selected_question,
            )
            .expect("authorize exact public hybrid closure");
        let partition = view
            .verified_case_chunk_partition()
            .expect("hybrid canonical partition");
        let projection = derive_relational_case_support_projection(
            partition,
            fragments,
            |cell_id| view.selected_run_materialization(cell_id),
            None,
            Some(closure_authority),
        )
        .expect("derive exact public hybrid projection");
        let metadata = projection.metadata();
        assert_eq!(
            metadata.classified_case_count,
            RelationalCaseSupportCount::Exact(300)
        );
        assert_eq!(
            metadata.selected_case_count,
            RelationalCaseSupportCount::Exact(20)
        );
        assert_eq!(
            metadata.materialized_selected_case_count,
            RelationalCaseSupportCount::Exact(20)
        );
        assert!(matches!(
            metadata.frontier,
            RelationalCaseSupportProjectionFrontier::Exact(closure)
                if closure.exact_logical_case_count == 300
                    && closure.exact_selected_case_count == 20
                    && closure.classified_chunk_count == 2
                    && closure.selected_materialization_count == 1
        ));

        let records = (0..projection.available_source_record_count())
            .map(|ordinal| {
                projection
                    .record_at(ordinal)
                    .expect("read public hybrid record")
                    .expect("public hybrid ordinal exists")
            })
            .collect::<Vec<_>>();
        let chunk_authorities = records
            .iter()
            .filter_map(|record| match record {
                RelationalCaseSupportProjectionRecord::Chunk {
                    classification_authority,
                    ..
                } => Some(classification_authority.kind()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            chunk_authorities,
            vec!["regional_certificate", "concrete_sweep"]
        );
        assert!(records.iter().any(|record| matches!(
            record,
            RelationalCaseSupportProjectionRecord::Region {
                exact_case_count: 256,
                correlated_starter_region_id: Some(_),
                ..
            }
        )));
        assert_eq!(
            records
                .iter()
                .filter(|record| matches!(
                    record,
                    RelationalCaseSupportProjectionRecord::SelectedMaterialization { .. }
                ))
                .count(),
            1
        );
        assert!(matches!(
            records.last(),
            Some(RelationalCaseSupportProjectionRecord::Closure(closure))
                if closure.exact_logical_case_count == 300
                    && closure.exact_selected_case_count == 20
        ));
    }

    #[test]
    fn checked_shared_namespace_query_resumes_before_successor_with_fresh_runtime() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/verify/qualified_namespace_parity.runa");
        let fixture_dir = fixture.parent().expect("fixture directory");
        let source = fs::read_to_string(&fixture).expect("read shared namespace fixture");
        let statements = parse(&source);
        let prepare_fixture = || {
            prepare_checked_relational_stream(
                &statements,
                Some(fixture_dir.to_string_lossy().into_owned()),
                &source,
                Some("qualified_namespace_transitive"),
            )
            .expect("prepare checked shared namespace exploration")
        };
        let temp = TestDirectory::new();
        let run_state = temp.path().join("run-state");

        // Stop at an explicit source-only quantum. Dropping this preparation
        // guarantees that successor evaluation (including the transitive root
        // call) happens only after the same checked source is prepared again.
        {
            let mut prepared = prepare_fixture();
            let checked = prepared.checked.view();
            let driver =
                RelationalStreamDriver::from_checked_with_limits_and_classification_backends(
                    &checked,
                    &prepared.support_plan,
                    RelationalStreamDriverLimits::default(),
                    None,
                    Some(&prepared.classification_evaluator),
                )
                .expect("build shared namespace stream scheduler");
            let mut durable =
                RelationalDurableJournal::open_or_create_with_region_replay_authority(
                    &run_state,
                    prepared.contract,
                    RelationalDurableJournalLimits::default(),
                    Arc::clone(&prepared.region_replay_authority),
                )
                .expect("open shared namespace durable journal");
            let mut stopped_at_source_members = false;
            for _ in 0..32 {
                let outcome = driver
                    .step_with_base_member_limit(
                        durable
                            .journal_mut_for_event_planning()
                            .expect("borrow shared namespace planning journal"),
                        &mut prepared.expression_runtime,
                        &mut prepared.mechanism_runtime,
                        NonZeroU16::new(1).unwrap(),
                    )
                    .expect("advance shared namespace source prefix");
                let RelationalStreamStepOutcome::Emitted(batch) = outcome else {
                    panic!("shared namespace stream quiesced before binding its source");
                };
                let quantum = batch.quantum();
                durable
                    .append_events(
                        batch.expected_sequence(),
                        batch.expected_head(),
                        batch.into_events(),
                    )
                    .expect("append shared namespace source prefix");
                if matches!(
                    quantum,
                    RelationalStreamQuantum::Base(
                        RelationalStepQuantum::SourceMembers { .. }
                            | RelationalStepQuantum::SourceMembersAndBindingExhaustion { .. }
                    )
                ) {
                    stopped_at_source_members = true;
                    break;
                }
            }
            assert!(
                stopped_at_source_members,
                "shared namespace fixture did not reach its source-only boundary"
            );
            assert_eq!(
                durable
                    .journal()
                    .expect("inspect source-only shared namespace prefix")
                    .scheduler_view()
                    .expect("inspect source-only shared namespace scheduler")
                    .case_count(),
                0,
                "the pause boundary must precede successor evaluation"
            );
            durable
                .flush_for_pause()
                .expect("flush source-only shared namespace prefix");
        }

        // Rechecking the identical fixture constructs a fresh expression
        // runtime and import closure, while reopening the journal replays the
        // durable source prefix through the active relational architecture.
        let mut prepared = prepare_fixture();
        let checked = prepared.checked.view();
        let driver = RelationalStreamDriver::from_checked_with_limits_and_classification_backends(
            &checked,
            &prepared.support_plan,
            RelationalStreamDriverLimits::default(),
            None,
            Some(&prepared.classification_evaluator),
        )
        .expect("rebuild shared namespace stream scheduler");
        let mut durable = RelationalDurableJournal::open_or_create_with_region_replay_authority(
            &run_state,
            prepared.contract,
            RelationalDurableJournalLimits::default(),
            Arc::clone(&prepared.region_replay_authority),
        )
        .expect("reopen shared namespace durable journal");
        let mut completed = false;
        for _ in 0..64 {
            match driver
                .step_with_base_member_limit(
                    durable
                        .journal_mut_for_event_planning()
                        .expect("borrow reopened shared namespace planning journal"),
                    &mut prepared.expression_runtime,
                    &mut prepared.mechanism_runtime,
                    NonZeroU16::new(1).unwrap(),
                )
                .expect("resume shared namespace stream")
            {
                RelationalStreamStepOutcome::Emitted(batch) => {
                    durable
                        .append_events(
                            batch.expected_sequence(),
                            batch.expected_head(),
                            batch.into_events(),
                        )
                        .expect("append resumed shared namespace batch");
                }
                RelationalStreamStepOutcome::Complete => {
                    completed = true;
                    break;
                }
                RelationalStreamStepOutcome::Quiescent(quiescence) => {
                    panic!("shared namespace stream quiesced before closure: {quiescence:?}");
                }
            }
        }
        assert!(completed, "shared namespace fixture did not close");

        let journal = durable
            .journal()
            .expect("inspect completed shared namespace journal");
        let scheduler = journal
            .scheduler_view()
            .expect("inspect completed shared namespace scheduler");
        assert_eq!(scheduler.case_count(), 1);
        assert_eq!(scheduler.selected_count(), 1);
        let analysis = journal
            .analysis_state()
            .expect("completed shared namespace analysis");
        assert!(analysis.is_closed());
        assert_eq!(
            analysis
                .selected_question()
                .expect("exact shared namespace selected-question seal")
                .mechanism_target()
                .count(),
            1,
            "the closed selected population must be exactly one case"
        );

        let [(ExploreAnalysisNodeIr::Result(_), CheckedExploreAnalysisIdentity::View { view_id })] =
            checked.analysis_nodes().collect::<Vec<_>>().as_slice()
        else {
            panic!("shared namespace exploration must have one result layer");
        };
        let closed = analysis
            .closed_catalog()
            .expect("completed shared namespace analysis catalog");
        let layer = closed
            .snapshot()
            .layer(RelationalAnalysisLayerId::Result(*view_id))
            .expect("completed shared namespace result layer");
        let RelationalAnalysisLayerSnapshot::Result(result) = layer else {
            panic!("shared namespace analysis identity must name a result layer");
        };
        let spec = result
            .state()
            .spec()
            .expect("registered shared namespace result specification");
        let projection = result
            .state()
            .projection()
            .expect("completed shared namespace result projection");
        assert_eq!(
            spec.projection_names(),
            &[Box::<str>::from("before"), Box::<str>::from("score")]
        );
        let [record] = projection.records() else {
            panic!("shared namespace result must contain exactly one projected row");
        };
        let row = record
            .record()
            .row()
            .expect("each-case shared namespace projection row");
        assert_eq!(
            row.values(),
            &[
                ResultValue::Value(super::super::ExploreValue::Int(1)),
                ResultValue::Value(super::super::ExploreValue::Int(36)),
            ],
            "root callable isolation must produce 36, not Policy leakage's 405"
        );
    }
}
