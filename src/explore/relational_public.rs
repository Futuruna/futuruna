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
    CheckedExploreCoverageRootRole, CheckedExploreCoverageSubject, CheckedExploreQueryAccessError,
    CheckedExploreQueryArtifactIssue, CheckedExploreQueryView,
    CheckedExploreSourceCoverageManifest, Diagnostic, ExploreAdmissionScope, Expr,
    OwnedCheckedExploreQuery, Stmt, Ty, TypeCheckArtifacts, TypeChecker,
};

use super::mechanism_incidence::MechanismCountEvidence;
use super::relation::AdmissionDecision;
use super::relational_analysis_catalog::{
    RelationalAnalysisLayerSnapshot, RelationalAnalysisLayerStatus,
    RelationalResultLayerSnapshotState, RelationalResultPublication,
};
use super::relational_analysis_plan::{
    RelationalAnalysisLayerId, RelationalAnalysisPlan, RelationalAnalysisPlanRoot,
};
use super::relational_classification_capsule::{
    ClassificationProvenanceRoot, ClassificationSpecializationRoot,
    FrozenClassificationQuestionSet, RelationalClassificationCapsule,
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
    ExploreResultGrainIr, ExploreResultInputIr, ExploreResultViewIr, ExploreSourceBindingKindIr,
    ExploreSuccessorKindIr, RelationalInterpreterExpressionRuntime, RelationalSupportPlan,
    RelationalSupportPlanner,
};

pub const EXPLORE_RELATIONAL_STREAM_REPORT_VERSION: u32 = 9;

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
    region_replay_authority: Option<Arc<RelationalRegionReplayAuthority>>,
    contract: RelationalJournalContract,
    analysis_plan_root: RelationalAnalysisPlanRoot,
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
        let durable = match &self.region_replay_authority {
            Some(authority) => {
                RelationalDurableJournal::open_or_create_with_region_replay_authority(
                    &options.run_state,
                    self.contract.clone(),
                    self.analysis_plan_root,
                    RelationalDurableJournalLimits::default(),
                    Arc::clone(authority),
                )
            }
            None => RelationalDurableJournal::open_or_create(
                &options.run_state,
                self.contract.clone(),
                self.analysis_plan_root,
                RelationalDurableJournalLimits::default(),
            ),
        }
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

    // Wire protocol V2 carries one QuestionId, one predicate program, and one
    // outcome. Require exactly one authored FIND; aliases use the checked
    // interpreter until the wire format can address them without nominating a
    // primary alias.
    let [question_id] = checked.find_question_ids() else {
        return None;
    };
    let [named_find] = query.finds.as_ref() else {
        return None;
    };
    let find = match &named_find.find {
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
            question_id: question_id.bytes(),
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
        FrozenClassificationQuestionSet::freeze(checked.question_ids().iter().copied()).map_err(
            |error| {
                ExploreStreamPreparationError::Execution(format!(
                    "checked Explore question set is incoherent: {error}"
                ))
            },
        )?,
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
    Find {
        name: String,
        question_id: String,
    },
    ChosenView {
        name: String,
        question_id: String,
        view_id: String,
    },
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
    /// Sorted, duplicate-free semantic question identities. Authored FIND
    /// names and order intentionally remain outside resumable identity.
    pub question_ids: Vec<String>,
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
    AwaitingMechanismSupport {
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
}

/// Progress for one authored FIND address. Equivalent aliases have distinct
/// names but the same QuestionId and therefore the same durable counts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExploreStreamFind {
    pub name: String,
    pub question_id: String,
    pub closed: bool,
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

/// One checked, ordered column in a named result's public schema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExploreStreamResultColumn {
    pub name: String,
    pub ty: String,
}

/// Resolved upstream address and semantic identity for one result view.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExploreStreamResultInput {
    Sources { relation_id: String },
    Find { name: String, question_id: String },
    MechanismIncidence { name: String, request_id: String },
}

/// Checked output grain of one named result view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExploreStreamResultGrain {
    EachCase,
    EachIncidence,
    GroupAll,
    GroupBy,
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
    pub input: ExploreStreamResultInput,
    pub grain: ExploreStreamResultGrain,
    pub columns: Vec<ExploreStreamResultColumn>,
    pub group_keys: Vec<ExploreStreamResultColumn>,
    pub status: ExploreStreamLayerStatus,
    pub input_rows: ExploreStreamCount,
    pub output_rows: ExploreStreamCount,
    pub projection_records: ExploreStreamCount,
    /// Number of bounded records appended during this invocation. Their
    /// values remain in the journal-owned projection and can be copied to
    /// NDJSON by a separate cursor without materializing one report array.
    pub projection_records_appended: u128,
    /// Exact authenticated roots for any published result, independently of
    /// whether a bounded grouped preview is applicable.
    pub evidence: Option<ExploreStreamResultEvidence>,
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
    /// Total append-only prefix observations across the automatic whole-
    /// mechanism lane and explicitly requested node/edge lanes. These are
    /// stream facts, not a fabricated estimate of final case count.
    pub total_support_observation_points: u128,
    pub total_support_observation_chain_root: Option<String>,
    /// Automatic whole-mechanism coverage used to authorize structural
    /// support closure. Explicit readers never contribute to these fields.
    pub automatic_support_observation_points: u128,
    pub automatic_registered_support_slices: u128,
    pub automatic_dirty_support_slices: u128,
    pub automatic_observed_support_slices: u128,
    pub automatic_sealed_support_slices: u128,
    pub automatic_support_observation_chain_root: Option<String>,
    pub initial_automatic_support_observation_point_id: Option<String>,
    /// Durable observation declarations include automatic whole-mechanism
    /// aliases, so this count is intentionally independent of the explicit
    /// node/edge scheduler's registered-slice count.
    pub explicit_support_observation_demand_registrations: u128,
    pub explicit_support_observation_points: u128,
    pub explicit_registered_support_slices: u128,
    pub explicit_ready_support_slices: u128,
    pub explicit_pending_backfill_support_slices: u128,
    pub explicit_dirty_support_slices: u128,
    pub explicit_unsealed_support_slices: u128,
    pub explicit_observed_support_slices: u128,
    pub explicit_sealed_support_slices: u128,
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
    pub analysis_closed: bool,
    pub counts: ExploreStreamPopulationCounts,
    pub finds: Vec<ExploreStreamFind>,
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
            let detail = match &error {
                CheckedExploreQueryAccessError::Producer(
                    CheckedExploreQueryArtifactIssue::EndpointTotality(issue),
                ) => issue.to_string(),
                _ => format!("{error:?}"),
            };
            ExploreStreamPreparationError::Execution(format!(
                "checked exploration boundary is unavailable: {detail}"
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
    let region_replay_authority = if checked.question_ids().len() == 1 {
        Some(Arc::new(
            RelationalRegionReplayAuthority::new(
                Arc::clone(&owned_checked),
                support_plan.clone(),
                Arc::clone(&classification_capsule),
            )
            .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?,
        ))
    } else {
        None
    };
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
        checked.question_ids().iter().copied(),
        checked.transition_schemas().state_schema_id(),
        checked.transition_schemas().context_schema_id(),
        checked.transition_schemas().transition_type_id(),
        analysis_plan.producer_graph_digest().bytes(),
    );
    let publication_plan = RelationalPublicationPlan::from_checked(&checked, contract.clone())
        .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?;
    trace_preparation_phase(started, "prepared publication");
    Ok(PreparedRelationalExplore {
        checked: owned_checked,
        support_plan,
        region_replay_authority,
        contract,
        analysis_plan_root: analysis_plan.root(),
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
            analysis_plan_root: _,
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
    definitions.rule_dispatch_return_types = artifacts.rule_dispatch_backend_return_types.clone();
    definitions.rule_dispatch_return_issues = artifacts.rule_dispatch_backend_return_issues.clone();
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
    // The current classified-support accelerator proves one semantic question
    // at a time. Use it only when the checked set is exactly singular; plural
    // queries share the concrete traversal and never nominate a primary FIND.
    let classification_progress = match checked.question_ids() {
        [question_id] => scheduler
            .classification_progress_counts(*question_id)
            .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?,
        _ => None,
    };
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
    let support_complete = classification_progress.is_some_and(|counts| counts.is_complete());
    let relation_closed = relation_enumeration_closed || support_complete;
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
    let mut finds = Vec::with_capacity(checked.closed_query.finds.len());
    for (find_index, named_find) in checked.closed_query.finds.iter().enumerate() {
        let question_id = checked.find_question_id(find_index).ok_or_else(|| {
            ExploreStreamPreparationError::Execution(format!(
                "checked FIND {find_index} has no aligned QuestionId"
            ))
        })?;
        let find_classified = scheduler
            .question_decision_count(question_id)
            .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?
            as u128;
        let selected_observed = scheduler
            .selected_count(question_id)
            .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?
            as u128;
        let not_selected_observed =
            find_classified
                .checked_sub(selected_observed)
                .ok_or_else(|| {
                    ExploreStreamPreparationError::Execution(format!(
                        "selected case count exceeds the classified FIND population for {}",
                        named_find.name
                    ))
                })?;
        let find_closed_extensional = admission_closed_extensional && find_classified == admitted;
        let selected_seal = journal
            .analysis_state()
            .and_then(|analysis| analysis.selected_question(question_id));
        let certified_selected_count = selected_seal.map(|seal| seal.mechanism_target().count());
        let question_progress =
            classification_progress.filter(|_| checked.question_ids() == [question_id]);
        let question_support_complete =
            question_progress.is_some_and(|counts| counts.is_complete());
        let closed =
            selected_seal.is_some() || find_closed_extensional || question_support_complete;
        let find_classified_count = merge_population_count(
            &format!("FIND-classified cases for {}", named_find.name),
            question_progress.map(|counts| (counts.admitted(), counts.is_complete())),
            find_classified,
            find_closed_extensional,
            None,
        )?;
        let selected_count = merge_population_count(
            &format!("selected cases for {}", named_find.name),
            question_progress.map(|counts| (counts.admitted_selected(), counts.is_complete())),
            selected_observed,
            find_closed_extensional,
            certified_selected_count,
        )?;
        let not_selected_count = merge_population_count(
            &format!("not-selected cases for {}", named_find.name),
            question_progress.map(|counts| (counts.admitted_not_selected(), counts.is_complete())),
            not_selected_observed,
            find_closed_extensional,
            None,
        )?;
        finds.push(ExploreStreamFind {
            name: named_find.name.clone(),
            question_id: hex(question_id.bytes()),
            closed,
            find_classified: find_classified_count,
            selected: selected_count,
            not_selected: not_selected_count,
        });
    }

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
            question_ids: journal
                .contract()
                .question_ids()
                .iter()
                .map(|question_id| hex(question_id.bytes()))
                .collect(),
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
        analysis_closed: scheduler.analysis_is_closed(),
        counts: ExploreStreamPopulationCounts {
            sources,
            cases,
            admission_classified: admission_classified_count,
            admitted: admitted_count,
            rejected: rejected_count,
        },
        finds,
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
                ExploreAnalysisNodeIr::Result(view),
                CheckedExploreAnalysisIdentity::View { view_id },
            ) => ExploreStreamLayer::Result(result_layer(
                analysis,
                checked,
                index,
                view,
                *view_id,
                projection_starts.get(index).copied().unwrap_or(0),
                &mut preview_budget,
            )?),
            (
                ExploreAnalysisNodeIr::Mechanisms(request),
                CheckedExploreAnalysisIdentity::Mechanisms { request_id, .. },
            ) => ExploreStreamLayer::Mechanisms(mechanism_layer(
                journal,
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
        ExploreMechanismTargetIr::Find { find_index } => {
            let named_find = checked.closed_query.finds.get(*find_index).ok_or_else(|| {
                ExploreStreamPreparationError::Execution(format!(
                    "mechanism target names missing FIND {find_index}"
                ))
            })?;
            let question_id = checked.find_question_id(*find_index).ok_or_else(|| {
                ExploreStreamPreparationError::Execution(format!(
                    "mechanism target FIND {find_index} has no aligned QuestionId"
                ))
            })?;
            Ok(ExploreStreamMechanismTarget::Find {
                name: named_find.name.clone(),
                question_id: hex(question_id.bytes()),
            })
        }
        ExploreMechanismTargetIr::ViewChosen { view_node_index } => {
            let (node, identity) =
                checked
                    .analysis_nodes()
                    .nth(*view_node_index)
                    .ok_or_else(|| {
                        ExploreStreamPreparationError::Execution(format!(
                            "mechanism target names missing analysis node {view_node_index}"
                        ))
                    })?;
            let ExploreAnalysisNodeIr::Result(view) = node else {
                return Err(ExploreStreamPreparationError::Execution(format!(
                    "mechanism target analysis node {view_node_index} is not a result view"
                )));
            };
            let CheckedExploreAnalysisIdentity::View { view_id } = identity else {
                return Err(ExploreStreamPreparationError::Execution(format!(
                    "mechanism target analysis node {view_node_index} is not a result view"
                )));
            };
            let ExploreResultInputIr::Find { find_index, .. } = &view.input else {
                return Err(ExploreStreamPreparationError::Execution(format!(
                    "mechanism target analysis node {view_node_index} is not FIND-backed"
                )));
            };
            let question_id = checked.find_question_id(*find_index).ok_or_else(|| {
                ExploreStreamPreparationError::Execution(format!(
                    "mechanism target view {view_node_index} has no aligned QuestionId"
                ))
            })?;
            Ok(ExploreStreamMechanismTarget::ChosenView {
                name: view.name.clone(),
                question_id: hex(question_id.bytes()),
                view_id: hex(view_id.bytes()),
            })
        }
    }
}

fn public_result_input(
    checked: &CheckedExploreQueryView<'_>,
    node_index: usize,
    input: &ExploreResultInputIr,
) -> Result<ExploreStreamResultInput, ExploreStreamPreparationError> {
    match input {
        ExploreResultInputIr::Sources => Ok(ExploreStreamResultInput::Sources {
            relation_id: hex(checked.relation_id().bytes()),
        }),
        ExploreResultInputIr::Find {
            find_name,
            find_index,
        } => {
            let find = checked.closed_query.finds.get(*find_index).ok_or_else(|| {
                ExploreStreamPreparationError::Execution(format!(
                    "result node {node_index} names missing FIND {find_index}"
                ))
            })?;
            if find.name != *find_name {
                return Err(ExploreStreamPreparationError::Execution(format!(
                    "result node {node_index} FIND address diverged from its checked input"
                )));
            }
            let question_id = checked.find_question_id(*find_index).ok_or_else(|| {
                ExploreStreamPreparationError::Execution(format!(
                    "result node {node_index} FIND {find_index} has no aligned QuestionId"
                ))
            })?;
            Ok(ExploreStreamResultInput::Find {
                name: find_name.clone(),
                question_id: hex(question_id.bytes()),
            })
        }
        ExploreResultInputIr::MechanismIncidence { request_node_index } => {
            if *request_node_index >= node_index {
                return Err(ExploreStreamPreparationError::Execution(format!(
                    "result node {node_index} does not reference a prior mechanism node"
                )));
            }
            let node = checked
                .closed_query
                .analysis
                .get(*request_node_index)
                .ok_or_else(|| {
                    ExploreStreamPreparationError::Execution(format!(
                        "result node {node_index} names missing mechanism node {request_node_index}"
                    ))
                })?;
            let identity = checked
                .artifact
                .analysis
                .get(*request_node_index)
                .ok_or_else(|| {
                    ExploreStreamPreparationError::Execution(format!(
                        "result node {node_index} input node {request_node_index} has no checked identity"
                    ))
                })?;
            let ExploreAnalysisNodeIr::Mechanisms(request) = node else {
                return Err(ExploreStreamPreparationError::Execution(format!(
                    "result node {node_index} input node {request_node_index} is not a mechanism request"
                )));
            };
            let CheckedExploreAnalysisIdentity::Mechanisms { request_id, .. } = identity else {
                return Err(ExploreStreamPreparationError::Execution(format!(
                    "result node {node_index} input node {request_node_index} has no mechanism identity"
                )));
            };
            Ok(ExploreStreamResultInput::MechanismIncidence {
                name: request.name.clone(),
                request_id: hex(request_id.bytes()),
            })
        }
    }
}

const fn public_result_grain(grain: &ExploreResultGrainIr) -> ExploreStreamResultGrain {
    match grain {
        ExploreResultGrainIr::EachCase { .. } => ExploreStreamResultGrain::EachCase,
        ExploreResultGrainIr::EachIncidence { .. } => ExploreStreamResultGrain::EachIncidence,
        ExploreResultGrainIr::GroupAll { .. } => ExploreStreamResultGrain::GroupAll,
        ExploreResultGrainIr::GroupBy { .. } => ExploreStreamResultGrain::GroupBy,
    }
}

fn public_result_columns(fields: &[super::ExploreResultFieldIr]) -> Vec<ExploreStreamResultColumn> {
    fields
        .iter()
        .map(|field| ExploreStreamResultColumn {
            name: field.name.clone(),
            ty: field.ty.to_string(),
        })
        .collect()
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
    checked: &CheckedExploreQueryView<'_>,
    node_index: usize,
    view: &ExploreResultViewIr,
    view_id: super::ViewId,
    start: usize,
    preview_budget: &mut ExploreStreamPreviewBudget,
) -> Result<ExploreStreamResultLayer, ExploreStreamPreparationError> {
    let input = public_result_input(checked, node_index, &view.input)?;
    let grain = public_result_grain(&view.grain);
    let columns = public_result_columns(&view.select);
    let group_keys = match &view.grain {
        ExploreResultGrainIr::GroupBy { fields, .. } => public_result_columns(fields),
        ExploreResultGrainIr::EachCase { .. }
        | ExploreResultGrainIr::EachIncidence { .. }
        | ExploreResultGrainIr::GroupAll { .. } => Vec::new(),
    };
    let absent = || ExploreStreamResultLayer {
        name: view.name.clone(),
        view_id: hex(view_id.bytes()),
        input: input.clone(),
        grain,
        columns: columns.clone(),
        group_keys: group_keys.clone(),
        status: ExploreStreamLayerStatus::ResultUnregistered,
        input_rows: ExploreStreamCount::LowerBound(0),
        output_rows: ExploreStreamCount::LowerBound(0),
        projection_records: ExploreStreamCount::LowerBound(0),
        projection_records_appended: 0,
        evidence: None,
        grouped_preview: None,
    };
    let Some(analysis) = analysis else {
        return Ok(absent());
    };
    let published_counts = analysis
        .result_projection_closure(view_id)
        .map(|closure| closure.counts());
    let published_output_groups = published_counts.and_then(|counts| counts.output_groups());
    let published_output_rows = published_counts.map(|counts| counts.output_rows());
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
            let projection_record_count = projection.len() as u128;
            let output_rows =
                result_output_rows(status, spec, projection_record_count, published_output_rows)?;
            let result_evidence = public_result_evidence(
                status,
                publication,
                projection_record_count,
                projection.root().bytes(),
            )?;
            let grouped_preview = grouped_result_preview(
                spec,
                projection_record_count,
                projection.records(),
                result_evidence.clone(),
                published_output_groups,
                preview_budget,
            )?;
            Ok(ExploreStreamResultLayer {
                name: view.name.clone(),
                view_id: hex(view_id.bytes()),
                input: input.clone(),
                grain,
                columns: columns.clone(),
                group_keys: group_keys.clone(),
                status: layer_status(status),
                input_rows: relation_count(evidence.logical_len(), evidence.input_is_sealed()),
                output_rows,
                projection_records: relation_count(
                    projection_record_count,
                    status == RelationalAnalysisLayerStatus::ResultPublished,
                ),
                projection_records_appended: projection_suffix_len(projection.len(), start)?,
                evidence: result_evidence,
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
            let projection_record_count = projection.records().len() as u128;
            let output_rows = result_output_rows(
                result.status(),
                spec,
                projection_record_count,
                published_output_rows,
            )?;
            let result_evidence = public_result_evidence(
                result.status(),
                *publication,
                projection_record_count,
                projection.root().bytes(),
            )?;
            let grouped_preview = grouped_result_preview(
                spec,
                projection_record_count,
                projection.records().iter(),
                result_evidence.clone(),
                published_output_groups,
                preview_budget,
            )?;
            Ok(ExploreStreamResultLayer {
                name: view.name.clone(),
                view_id: hex(view_id.bytes()),
                input: input.clone(),
                grain,
                columns: columns.clone(),
                group_keys: group_keys.clone(),
                status: layer_status(result.status()),
                input_rows: relation_count(evidence.logical_len(), evidence.input_is_sealed()),
                output_rows,
                projection_records: relation_count(projection_record_count, publication.is_some()),
                projection_records_appended: projection_suffix_len(
                    projection.records().len(),
                    start,
                )?,
                evidence: result_evidence,
                grouped_preview,
            })
        }
        _ => Err(ExploreStreamPreparationError::Execution(
            "analysis state does not own exactly one catalog".into(),
        )),
    }
}

fn result_output_rows(
    status: RelationalAnalysisLayerStatus,
    spec: &ResultViewSpec,
    projection_record_count: u128,
    published_count: Option<ResultViewCount>,
) -> Result<ExploreStreamCount, ExploreStreamPreparationError> {
    if status == RelationalAnalysisLayerStatus::ResultPublished {
        return match published_count {
            Some(ResultViewCount::Exact(value)) => Ok(ExploreStreamCount::Exact(value)),
            Some(ResultViewCount::LowerBound(_) | ResultViewCount::Provisional(_)) => Err(
                preview_error("published result retained a non-exact output-row count"),
            ),
            None => Err(preview_error(
                "published result omitted its authenticated output-row count",
            )),
        };
    }

    // Every ungrouped projection record is one output row. Grouped prefixes
    // interleave headers and chosen rows, so their record count is not a row
    // count and the compact report deliberately avoids a population scan.
    Ok(ExploreStreamCount::LowerBound(
        if spec.grain().is_grouped() {
            0
        } else {
            projection_record_count
        },
    ))
}

fn public_result_evidence(
    status: RelationalAnalysisLayerStatus,
    publication: Option<RelationalResultPublication>,
    projection_record_count: u128,
    projection_root: [u8; 32],
) -> Result<Option<ExploreStreamResultEvidence>, ExploreStreamPreparationError> {
    let Some(publication) = publication else {
        return if status == RelationalAnalysisLayerStatus::ResultPublished {
            Err(preview_error(
                "published result omitted its authenticated publication receipt",
            ))
        } else {
            Ok(None)
        };
    };
    if status != RelationalAnalysisLayerStatus::ResultPublished {
        return Err(preview_error(
            "open result unexpectedly retained a terminal publication receipt",
        ));
    }
    Ok(Some(ExploreStreamResultEvidence {
        spec_root: hex(publication.spec_root().bytes()),
        projection_root: hex(projection_root),
        projection_record_count,
        publication_id: hex(publication.id().bytes()),
        evidence_root: hex(publication.evidence_root().bytes()),
        result_root: hex(publication.result_root().bytes()),
    }))
}

fn grouped_result_preview<'a>(
    spec: &ResultViewSpec,
    projection_record_count: u128,
    records: impl Iterator<Item = &'a IndexedResultProjectionRecord>,
    evidence: Option<ExploreStreamResultEvidence>,
    published_output_groups: Option<ResultViewCount>,
    budget: &mut ExploreStreamPreviewBudget,
) -> Result<Option<ExploreStreamGroupedResultPreview>, ExploreStreamPreparationError> {
    if !spec.grain().is_grouped() || spec.choice().is_some() {
        return Ok(None);
    }
    let Some(evidence) = evidence else {
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
        evidence,
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
    journal: &RelationalJournal,
    name: String,
    request_id: super::MechanismRequestId,
    target: ExploreStreamMechanismTarget,
) -> Result<ExploreStreamMechanismLayer, ExploreStreamPreparationError> {
    let analysis = journal.analysis_state();
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
        total_support_observation_points: 0,
        total_support_observation_chain_root: None,
        automatic_support_observation_points: 0,
        automatic_registered_support_slices: 0,
        automatic_dirty_support_slices: 0,
        automatic_observed_support_slices: 0,
        automatic_sealed_support_slices: 0,
        automatic_support_observation_chain_root: None,
        initial_automatic_support_observation_point_id: None,
        explicit_support_observation_demand_registrations: 0,
        explicit_support_observation_points: 0,
        explicit_registered_support_slices: 0,
        explicit_ready_support_slices: 0,
        explicit_pending_backfill_support_slices: 0,
        explicit_dirty_support_slices: 0,
        explicit_unsealed_support_slices: 0,
        explicit_observed_support_slices: 0,
        explicit_sealed_support_slices: 0,
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
    let total_support_observation_points = journal.mechanism_support_observation_count(request_id);
    let automatic_support_observation_points =
        journal.mechanism_support_automatic_observation_count(request_id);
    let explicit_support_observation_points = total_support_observation_points
        .checked_sub(automatic_support_observation_points)
        .ok_or_else(|| {
            ExploreStreamPreparationError::Execution(
                "automatic support observations exceeded the shared observation log".into(),
            )
        })?;
    let explicit_scheduler =
        journal.durable_explicit_mechanism_support_scheduler_summary(request_id);
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
        total_support_observation_points,
        total_support_observation_chain_root: journal
            .mechanism_support_observation_chain_root(request_id)
            .map(|root| hex(root.bytes())),
        automatic_support_observation_points,
        automatic_registered_support_slices: journal
            .mechanism_support_registered_slice_count(request_id),
        automatic_dirty_support_slices: journal.mechanism_support_dirty_slice_count(request_id),
        automatic_observed_support_slices: journal
            .mechanism_support_observed_slice_count(request_id),
        automatic_sealed_support_slices: journal.mechanism_support_sealed_slice_count(request_id),
        automatic_support_observation_chain_root: journal
            .mechanism_support_automatic_observation_chain_root(request_id)
            .map(|root| hex(root.bytes())),
        initial_automatic_support_observation_point_id: journal
            .mechanism_support_initial_observation_point_id(request_id)
            .map(|point_id| hex(point_id.bytes())),
        explicit_support_observation_demand_registrations: journal
            .mechanism_support_observation_demand_count(request_id),
        explicit_support_observation_points,
        explicit_registered_support_slices: explicit_scheduler
            .map_or(0, |scheduler| scheduler.registry().slice_count()),
        explicit_ready_support_slices: explicit_scheduler
            .map_or(0, |scheduler| scheduler.registry().ready_slice_count()),
        explicit_pending_backfill_support_slices: explicit_scheduler
            .map_or(0, |scheduler| scheduler.pending_backfill().slice_count()),
        explicit_dirty_support_slices: explicit_scheduler
            .map_or(0, |scheduler| scheduler.dirty().slice_count()),
        explicit_unsealed_support_slices: explicit_scheduler
            .map_or(0, |scheduler| scheduler.unsealed().slice_count()),
        explicit_observed_support_slices: journal
            .mechanism_support_explicit_observed_slice_count(request_id),
        explicit_sealed_support_slices: journal
            .mechanism_support_explicit_sealed_slice_count(request_id),
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
            RelationalStreamQuiescence::AwaitingMechanismSupport { request_id } => {
                ExploreStreamPauseReason::AwaitingMechanismSupport {
                    request_id: hex(request_id.bytes()),
                }
            }
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
    use std::time::Duration;

    use super::super::relational_analysis_catalog::RelationalAnalysisCatalogError;
    use super::super::relational_analysis_journal::{
        RelationalAnalysisEvidenceEvent, RelationalAnalysisJournalError,
    };
    use super::super::relational_analysis_plan::{
        RelationalAnalysisLayerRegistration, RelationalAnalysisPlan,
        RelationalMechanismLayerRegistration, RelationalMechanismObservationId,
    };
    use super::super::relational_case_support_projection::{
        derive_relational_case_support_projection, RelationalCaseSupportClosureAuthority,
        RelationalCaseSupportCount, RelationalCaseSupportProjectionFrontier,
        RelationalCaseSupportProjectionRecord,
    };
    use super::super::relational_classified_sweep_step_driver::{
        RelationalClassifiedSweepStepDriver, RelationalClassifiedSweepStepOutcome,
    };
    use super::super::relational_durable_journal::{
        RelationalDurableJournal, RelationalDurableJournalError, RelationalDurableJournalLimits,
    };
    use super::super::relational_frontier::{WorkCompletionRef, WorkNodeSpec};
    use super::super::relational_journal::{
        RelationalCheckpointEvent, RelationalClassifiedSupportFragment, RelationalEvidenceEvent,
        RelationalJournal, RelationalJournalError, RelationalJournalEvent,
    };
    use super::super::relational_mechanism_step_driver::RelationalMechanismStepQuantum;
    use super::super::relational_step_driver::{
        RelationalConcreteQuiescence, RelationalStepDriver, RelationalStepOutcome,
        RelationalStepQuantum,
    };
    use super::super::relational_stream_driver::{
        RelationalStreamDriver, RelationalStreamDriverLimits, RelationalStreamQuantum,
        RelationalStreamStepOutcome,
    };
    use super::super::result_projection::ResultProjectionError;
    use super::super::stream_resource::{
        ExactStreamOneWorkerEnvelope, ExactStreamResourceAction, ExactStreamWorkSubject,
    };
    use super::*;
    use crate::{Lexer, Parser};

    const EXACT_EMPTY: &str = r#"
? explore regional_exact_empty {
    from {
        vary before in range(0, 300)
        given context = ()
    }

    transition after = before + 1
    find cases = matches of after < before
}
"#;

    const UNIFORMLY_SELECTED: &str = r#"
? explore regional_uniformly_selected {
    from {
        vary before in range(0, 300)
        given context = ()
    }

    transition after = before + 1
    find cases = matches of after > before
}
"#;

    const MIXED_FIRST_CHILD: &str = r#"
? explore regional_mixed_first_child {
    from {
        vary before in range(0, 300)
        given context = ()
    }

    transition after = before + 1
    find cases = matches of before >= 128
}
"#;

    const UNSUPPORTED_NONLINEAR: &str = r#"
? explore regional_unsupported_nonlinear {
    from {
        vary before in range(0, 300)
        given context = ()
    }

    transition after = before + 1
    find cases = matches of before * before < 0
}
"#;

    const HYBRID: &str = r#"
? explore regional_hybrid {
    from {
        vary before in range(0, 300)
        given context = ()
    }

    transition after = before + 1
    find cases = matches of before >= 280
}
"#;

    const PLURAL_PUBLICATION: &str = r#"
? explore plural_publication {
    from {
        vary before in range(0, 2)
        given context = ()
    }

    transition after = before + 1
    find all_cases = all
    find upper_case = matches of before >= 1
}
"#;

    const CHOSEN_MECHANISM_PUBLICATION: &str = r#"
> chosen_target_observer(state: Int, context: Unit) -> Int {
    if state < 2 { 0 } else { 1 }
}

? explore chosen_mechanism_publication {
    from {
        vary before in range(0, 4)
        given context = ()
    }

    transition after = before + 1
    find all_cases = all
    results winner from find all_cases {
        group all
        measure [score = before / 2]
        select [case_id, before, after, score]
        choose all maximizing score
    }
    mechanisms winner_path from view winner chosen using chosen_target_observer
}
"#;

    const SELF_DESCRIBING_ANSWER_PUBLICATION: &str = r#"
? explore self_describing_answer_publication {
    from {
        vary before in range(0, 2)
        given context = ()
    }

    transition after = before + 1
    find all_cases = all
    find impossible = matches of before < 0

    results all_rows from find all_cases {
        each case
        select [before, after]
    }

    results empty_rows from find impossible {
        each case
        select [before, after]
    }

    results before_bins from find all_cases {
        group by [bin = before]
        aggregate [cases = count_distinct(case_id)]
        select [bin, cases]
    }
}
"#;

    const PLURAL_FIND_COMPLETION_PREFIX: &str = r#"
? explore plural_find_completion_prefix {
    from {
        vary before in [0]
        given context = ()
    }

    transition after in [before + 1]
    find all_cases = all
    find increasing = matches of after > before
}
"#;

    const ZERO_QUESTION_PUBLICATION: &str = r#"
? explore zero_question_publication {
    from {
        vary before in range(0, 2)
        given context = ()
    }

    transition after = before + 1
    transitions case_graph from all cases
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

    fn exact_one_region_replay_authority(
        prepared: &PreparedRelationalExplore,
    ) -> Arc<RelationalRegionReplayAuthority> {
        assert_eq!(
            prepared.checked.view().question_ids().len(),
            1,
            "regional replay authority is an exact-one-FIND accelerator"
        );
        Arc::clone(
            prepared
                .region_replay_authority
                .as_ref()
                .expect("exact-one fixture must prepare regional replay authority"),
        )
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
            prepared.contract.clone(),
            exact_one_region_replay_authority(&prepared),
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
        let question_id = checked.question_ids()[0];
        let analysis_plan =
            RelationalAnalysisPlan::from_checked(&checked).expect("plan partial fixture analysis");
        let mut journal = RelationalJournal::new_with_region_replay_authority(
            prepared.contract.clone(),
            exact_one_region_replay_authority(&prepared),
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
            .classified_chunk_accumulator(question_id)
            .unwrap()
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

    #[test]
    fn plural_find_completion_prefix_keeps_the_next_question_runnable() {
        let mut prepared = prepare(PLURAL_FIND_COMPLETION_PREFIX);
        let checked = prepared.checked.view();
        let question_ids = checked.question_ids().to_vec();
        assert_eq!(question_ids.len(), 2);
        let analysis_plan =
            RelationalAnalysisPlan::from_checked(&checked).expect("plan plural prefix fixture");
        let mut journal = RelationalJournal::new(prepared.contract.clone());
        journal
            .append(RelationalJournalEvent::analysis_plan_registered(
                analysis_plan,
            ))
            .expect("register plural prefix analysis plan");
        let driver = RelationalStepDriver::from_checked_with_max_members_per_quantum_and_classification_backends(
            &checked,
            &prepared.support_plan,
            NonZeroU16::new(1).unwrap(),
            None,
            Some(&prepared.classification_evaluator),
        )
        .expect("build plural prefix base scheduler");

        let mut resumed_after_find_completion = false;
        let mut reached_concrete_close = false;
        for _ in 0..128 {
            match driver
                .step_with_max_members_per_quantum(
                    &journal,
                    &mut prepared.expression_runtime,
                    NonZeroU16::new(1).unwrap(),
                )
                .expect("advance plural prefix base scheduler")
            {
                RelationalStepOutcome::Emitted(batch) => {
                    let quantum = batch.quantum();
                    if !resumed_after_find_completion {
                        if let RelationalStepQuantum::Find {
                            question_id,
                            case_id,
                            ..
                        } = quantum
                        {
                            let events = batch.into_events().into_vec();
                            let completion_index = events
                                .iter()
                                .position(|event| {
                                    matches!(
                                        event,
                                        RelationalJournalEvent::Checkpoint(
                                            RelationalCheckpointEvent::WorkNodeCompleted {
                                                completion:
                                                    WorkCompletionRef::FindDecided {
                                                        question_id: completed_question_id,
                                                        case_id: completed_case_id,
                                                        ..
                                                    },
                                                    ..
                                            }
                                        ) if *completed_question_id == question_id
                                            && *completed_case_id == case_id
                                    )
                                })
                                .expect("FIND batch completion event");
                            let next_question_insertion_index = events
                                .iter()
                                .position(|event| {
                                    matches!(
                                        event,
                                        RelationalJournalEvent::Checkpoint(
                                            RelationalCheckpointEvent::WorkNodeInserted {
                                                spec:
                                                    WorkNodeSpec::EvaluateFind {
                                                        question_id: pending_question_id,
                                                        case_id: pending_case_id,
                                                    },
                                                ..
                                            }
                                        ) if *pending_question_id != question_id
                                            && *pending_case_id == case_id
                                    )
                                })
                                .expect("next plural FIND work insertion");
                            assert!(
                                next_question_insertion_index < completion_index,
                                "the next FIND must be durable before the current FIND disappears"
                            );

                            for event in events.into_iter().take(completion_index + 1) {
                                journal
                                    .append(event)
                                    .expect("append crash prefix through FIND completion");
                            }
                            let retained_prefix = journal.entries().to_vec();
                            journal = RelationalJournal::replay(
                                prepared.contract.clone(),
                                retained_prefix,
                            )
                            .expect("cold-replay plural FIND completion prefix");
                            resumed_after_find_completion = true;
                            continue;
                        }
                    }
                    append_base_batch(&mut journal, batch);
                }
                RelationalStepOutcome::Quiescent(
                    RelationalConcreteQuiescence::ConcreteBaseClassified {
                        cases,
                        admitted,
                        question_counts,
                    },
                ) => {
                    assert!(resumed_after_find_completion);
                    assert_eq!(cases, 1);
                    assert_eq!(admitted, 1);
                    assert_eq!(question_counts.len(), 2);
                    for question_id in question_ids.iter().copied() {
                        let count = question_counts
                            .iter()
                            .find(|count| count.question_id == question_id)
                            .expect("classified count for each plural question");
                        assert_eq!(count.classified_cases, 1);
                        assert_eq!(
                            journal
                                .scheduler_view()
                                .expect("inspect plural completion prefix replay")
                                .question_decision_count(question_id)
                                .expect("known plural question"),
                            1
                        );
                    }
                    reached_concrete_close = true;
                    break;
                }
                RelationalStepOutcome::Quiescent(
                    RelationalConcreteQuiescence::SupportEvidenceClosed { .. },
                ) => panic!("plural concrete fixture must not use exact-one support closure"),
            }
        }
        assert!(
            reached_concrete_close,
            "plural prefix fixture did not reach concrete base closure"
        );
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
    fn plural_find_report_and_publication_keep_each_named_question_explicit() {
        let temp = TestDirectory::new();
        let run_state = temp.path().join("run-state");
        let output = temp.path().join("output");
        let prepared = prepare(PLURAL_PUBLICATION);
        assert_eq!(prepared.checked.view().question_ids().len(), 2);
        assert!(
            prepared.region_replay_authority.is_none(),
            "plural FIND execution must not choose one question for regional replay"
        );

        let mut epoch = prepared
            .open_epoch(ExploreStreamEpochOptions {
                run_state,
                output_directory: Some(output.clone()),
                outer_containment: None,
            })
            .expect("open plural publication epoch");
        epoch.resources = ExactStreamOneWorkerEnvelope::new_unmetered_for_test()
            .expect("create deterministic plural publication resource envelope");
        let report = epoch
            .run_slice(None)
            .expect("complete tiny plural publication fixture");

        assert_eq!(report.schema_version, 9);
        assert_eq!(report.lifecycle, ExploreStreamLifecycle::Complete);
        assert_eq!(report.counts.cases, ExploreStreamCount::Exact(2));
        assert_eq!(report.counts.admitted, ExploreStreamCount::Exact(2));
        assert_eq!(report.identity.question_ids.len(), 2);
        assert_eq!(report.finds.len(), 2);
        assert_eq!(report.finds[0].name, "all_cases");
        assert_eq!(report.finds[0].selected, ExploreStreamCount::Exact(2));
        assert_eq!(report.finds[1].name, "upper_case");
        assert_eq!(report.finds[1].selected, ExploreStreamCount::Exact(1));
        assert_ne!(report.finds[0].question_id, report.finds[1].question_id);
        assert!(report
            .finds
            .iter()
            .all(|find| report.identity.question_ids.contains(&find.question_id)));

        let publication = report
            .publication
            .as_ref()
            .expect("plural run must refresh its publication manifest");
        assert!(publication.artifacts.iter().all(|artifact| {
            artifact.key != "graph:case-support" && artifact.key != "graph:case-transitions"
        }));
        let manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&publication.manifest_path)
                .expect("read plural publication manifest"),
        )
        .expect("parse plural publication manifest");
        assert_eq!(manifest["schema_version"].as_u64(), Some(16));
        assert_eq!(
            manifest["identity"]["question_ids"]
                .as_array()
                .expect("manifest question IDs")
                .len(),
            2
        );
        assert_eq!(
            manifest["finds"]
                .as_array()
                .expect("manifest named FIND entries")
                .iter()
                .map(|find| find["name"].as_str().expect("manifest FIND name"))
                .collect::<Vec<_>>(),
            vec!["all_cases", "upper_case"]
        );
    }

    #[test]
    fn answer_index_describes_ungrouped_grouped_and_exact_empty_results() {
        let temp = TestDirectory::new();
        let run_state = temp.path().join("run-state");
        let output = temp.path().join("output");
        let prepared = prepare(SELF_DESCRIBING_ANSWER_PUBLICATION);

        let cold_journal = RelationalJournal::new(prepared.contract.clone());
        let cold_layers = analysis_layers(
            &cold_journal,
            &prepared.checked.view(),
            &vec![0; prepared.checked.view().closed_query.analysis.len()],
        )
        .expect("describe result declarations before analysis registration");
        let cold_results = cold_layers
            .iter()
            .filter_map(|layer| match layer {
                ExploreStreamLayer::Result(result) => Some(result),
                ExploreStreamLayer::Mechanisms(_) => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(cold_results.len(), 3);
        assert!(cold_results.iter().all(|result| {
            result.status == ExploreStreamLayerStatus::ResultUnregistered
                && result.output_rows == ExploreStreamCount::LowerBound(0)
                && result.evidence.is_none()
                && !result.columns.is_empty()
        }));
        assert_eq!(cold_results[0].grain, ExploreStreamResultGrain::EachCase);
        assert_eq!(cold_results[2].grain, ExploreStreamResultGrain::GroupBy);
        assert_eq!(cold_results[2].group_keys[0].name, "bin");
        assert_eq!(cold_results[2].group_keys[0].ty, "Int");

        let mut epoch = prepared
            .open_epoch(ExploreStreamEpochOptions {
                run_state,
                output_directory: Some(output.clone()),
                outer_containment: None,
            })
            .expect("open self-describing answer publication epoch");
        epoch.resources = ExactStreamOneWorkerEnvelope::new_unmetered_for_test()
            .expect("create deterministic answer publication resource envelope");
        let report = epoch
            .run_slice(None)
            .expect("complete tiny self-describing answer fixture");
        assert_eq!(report.schema_version, 9);
        assert_eq!(report.lifecycle, ExploreStreamLifecycle::Complete);

        let results = report
            .layers
            .iter()
            .filter_map(|layer| match layer {
                ExploreStreamLayer::Result(result) => Some(result),
                ExploreStreamLayer::Mechanisms(_) => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].name, "all_rows");
        assert_eq!(results[0].output_rows, ExploreStreamCount::Exact(2));
        assert_eq!(results[1].name, "empty_rows");
        assert_eq!(results[1].output_rows, ExploreStreamCount::Exact(0));
        assert_eq!(results[2].name, "before_bins");
        assert_eq!(results[2].output_rows, ExploreStreamCount::Exact(2));
        assert!(results.iter().all(|result| result.evidence.is_some()));

        let resumed = epoch
            .run_slice(Some(Duration::from_nanos(1)))
            .expect("recognize a completed journal before the runtime deadline");
        assert_eq!(resumed.lifecycle, ExploreStreamLifecycle::Complete);
        assert_eq!(resumed.semantic_batches_appended, 0);
        assert_eq!(resumed.semantic_events_appended, 0);
        assert!(resumed
            .publication
            .as_ref()
            .is_some_and(ExploreStreamPublication::is_caught_up));

        let publication = report.publication.expect("answer publication summary");
        let manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&publication.manifest_path).expect("read answer manifest"),
        )
        .expect("parse answer manifest");
        assert_eq!(manifest["schema_version"].as_u64(), Some(16));
        assert_eq!(manifest["answer"]["rows_inlined"].as_bool(), Some(false));
        let views = manifest["answer"]["result_views"]
            .as_array()
            .expect("manifest answer result views");
        assert_eq!(views.len(), 3);
        assert_eq!(views[0]["name"].as_str(), Some("all_rows"));
        assert_eq!(views[0]["grain"].as_str(), Some("each_case"));
        assert_eq!(
            views[0]["counts"]["output_rows"]["status"].as_str(),
            Some("exact")
        );
        assert_eq!(
            views[0]["counts"]["output_rows"]["value"].as_str(),
            Some("2")
        );
        assert_eq!(views[1]["name"].as_str(), Some("empty_rows"));
        assert_eq!(
            views[1]["counts"]["output_rows"]["value"].as_str(),
            Some("0")
        );
        assert_eq!(views[2]["grain"].as_str(), Some("group_by"));
        assert_eq!(
            views[2]["group_key_columns"][0]["name"].as_str(),
            Some("bin")
        );
        assert!(views.iter().all(|view| {
            view["artifact"]["caught_up_to_journal_prefix"].as_bool() == Some(true)
                && view["artifact"]["path"].as_str().is_some()
                && view["artifact"]["layer_roots"].is_object()
        }));
        let empty_artifact = output.join(
            views[1]["artifact"]["path"]
                .as_str()
                .expect("exact-empty result artifact path"),
        );
        assert_eq!(
            fs::read(&empty_artifact).expect("read materialized exact-empty result artifact"),
            b""
        );
    }

    #[test]
    fn chosen_view_mechanism_target_resumes_from_its_exact_published_case_set() {
        let temp = TestDirectory::new();
        let run_state = temp.path().join("run-state");
        let output = temp.path().join("output");

        // Stop halfway through the two-row chosen-target batch. Reopening
        // below must recover the durable proper prefix and replay exactly the
        // two winners rather than rebuilding an ambient selected population.
        {
            let mut prepared = prepare(CHOSEN_MECHANISM_PUBLICATION);
            let checked = prepared.checked.view();
            let limits = RelationalStreamDriverLimits::new(
                NonZeroU16::new(4).unwrap(),
                NonZeroU16::new(4).unwrap(),
                NonZeroU16::new(2).unwrap(),
            );
            let driver =
                RelationalStreamDriver::from_checked_with_limits_and_classification_backends(
                    &checked,
                    &prepared.support_plan,
                    limits,
                    None,
                    Some(&prepared.classification_evaluator),
                )
                .expect("build chosen-target stream scheduler");
            let mut durable =
                RelationalDurableJournal::open_or_create_with_region_replay_authority(
                    &run_state,
                    prepared.contract.clone(),
                    prepared.analysis_plan_root,
                    RelationalDurableJournalLimits::default(),
                    exact_one_region_replay_authority(&prepared),
                )
                .expect("open chosen-target durable journal");
            let mut admitted_chosen_target = false;
            for _ in 0..512 {
                let outcome = driver
                    .step_with_base_member_limit(
                        durable
                            .journal_mut_for_event_planning()
                            .expect("borrow chosen-target planning journal"),
                        &mut prepared.expression_runtime,
                        &mut prepared.mechanism_runtime,
                        NonZeroU16::new(4).unwrap(),
                    )
                    .expect("advance chosen-target prefix");
                let RelationalStreamStepOutcome::Emitted(batch) = outcome else {
                    panic!("chosen-target stream quiesced before target admission");
                };
                let quantum = batch.quantum();
                let chosen_batch = matches!(
                    quantum,
                    RelationalStreamQuantum::Mechanism(
                        RelationalMechanismStepQuantum::AdmitChosenTargetCases {
                            case_count,
                            ..
                        }
                    ) if case_count.get() == 2
                );
                let expected_sequence = batch.expected_sequence();
                let expected_head = batch.expected_head();
                let events = batch.into_events().into_vec();
                let append_count = if chosen_batch {
                    assert_eq!(events.len(), 2);
                    1
                } else {
                    events.len()
                };
                durable
                    .append_events(
                        expected_sequence,
                        expected_head,
                        events.into_iter().take(append_count),
                    )
                    .expect("append chosen-target prefix batch");
                if chosen_batch {
                    admitted_chosen_target = true;
                    durable
                        .flush_for_pause()
                        .expect("flush chosen-target crash prefix");
                    break;
                }
            }
            assert!(
                admitted_chosen_target,
                "fixture did not stop inside its two-case chosen target batch"
            );
        }

        let mut epoch = prepare(CHOSEN_MECHANISM_PUBLICATION)
            .open_epoch(ExploreStreamEpochOptions {
                run_state,
                output_directory: Some(output),
                outer_containment: None,
            })
            .expect("reopen chosen-target stream epoch");
        epoch.resources = ExactStreamOneWorkerEnvelope::new_unmetered_for_test()
            .expect("create deterministic chosen-target resource envelope");
        let report = epoch
            .run_slice(None)
            .expect("resume and complete chosen-target stream");

        assert_eq!(report.lifecycle, ExploreStreamLifecycle::Complete);
        assert!(report.analysis_closed);
        assert_eq!(report.counts.cases, ExploreStreamCount::Exact(4));
        assert_eq!(report.finds.len(), 1);
        assert_eq!(report.finds[0].selected, ExploreStreamCount::Exact(4));

        let mechanism = report
            .layers
            .iter()
            .find_map(|layer| match layer {
                ExploreStreamLayer::Mechanisms(mechanism) if mechanism.name == "winner_path" => {
                    Some(mechanism)
                }
                _ => None,
            })
            .expect("chosen mechanism layer");
        assert_eq!(mechanism.status, ExploreStreamLayerStatus::MechanismClosed);
        assert!(matches!(
            &mechanism.target,
            ExploreStreamMechanismTarget::ChosenView {
                name,
                question_id,
                ..
            } if name == "winner" && question_id == &report.finds[0].question_id
        ));
        assert_eq!(mechanism.target_cases, ExploreStreamCount::Exact(2));
        assert_eq!(mechanism.terminal_cases, ExploreStreamCount::Exact(2));
        assert_eq!(mechanism.incidence_cases, ExploreStreamCount::Exact(2));
        assert_eq!(mechanism.unavailable_cases, ExploreStreamCount::Exact(0));
        assert_eq!(mechanism.raw_signatures, ExploreStreamCount::Exact(1));
        assert_eq!(
            mechanism.structural_mechanisms,
            ExploreStreamCount::Exact(1)
        );
        assert!(mechanism.raw_closure_root.is_some());
        assert!(mechanism.structural_closure_root.is_some());
    }

    #[test]
    fn chosen_view_mechanism_target_rejects_tampered_projection_provenance() {
        let temp = TestDirectory::new();
        let run_state = temp.path().join("run-state");
        let mut prepared = prepare(CHOSEN_MECHANISM_PUBLICATION);
        let checked = prepared.checked.view();
        let limits = RelationalStreamDriverLimits::new(
            NonZeroU16::new(4).unwrap(),
            NonZeroU16::new(4).unwrap(),
            NonZeroU16::new(2).unwrap(),
        );
        let driver = RelationalStreamDriver::from_checked_with_limits_and_classification_backends(
            &checked,
            &prepared.support_plan,
            limits,
            None,
            Some(&prepared.classification_evaluator),
        )
        .expect("build chosen-target provenance scheduler");
        let mut durable = RelationalDurableJournal::open_or_create_with_region_replay_authority(
            &run_state,
            prepared.contract.clone(),
            prepared.analysis_plan_root,
            RelationalDurableJournalLimits::default(),
            exact_one_region_replay_authority(&prepared),
        )
        .expect("open chosen-target provenance journal");

        let mut rejected_tamper = false;
        for _ in 0..512 {
            let outcome = driver
                .step_with_base_member_limit(
                    durable
                        .journal_mut_for_event_planning()
                        .expect("borrow chosen-target provenance journal"),
                    &mut prepared.expression_runtime,
                    &mut prepared.mechanism_runtime,
                    NonZeroU16::new(4).unwrap(),
                )
                .expect("advance chosen-target provenance prefix");
            let RelationalStreamStepOutcome::Emitted(batch) = outcome else {
                panic!("chosen-target stream quiesced before its provenance batch");
            };
            let expected_sequence = batch.expected_sequence();
            let expected_head = batch.expected_head();
            let is_chosen_batch = matches!(
                batch.quantum(),
                RelationalStreamQuantum::Mechanism(
                    RelationalMechanismStepQuantum::AdmitChosenTargetCases {
                        case_count,
                        ..
                    }
                ) if case_count.get() == 2
            );
            let events = batch.into_events().into_vec();
            if !is_chosen_batch {
                durable
                    .append_events(expected_sequence, expected_head, events)
                    .expect("append chosen-target provenance setup batch");
                continue;
            }

            assert_eq!(events.len(), 2);
            let chosen_claim = |event: &RelationalJournalEvent| match event {
                RelationalJournalEvent::Evidence(RelationalEvidenceEvent::Analysis(
                    RelationalAnalysisEvidenceEvent::MechanismChosenTargetCaseAccepted {
                        request_id,
                        view_id,
                        projection_ordinal,
                        case_id,
                    },
                )) => (*request_id, *view_id, *projection_ordinal, *case_id),
                _ => panic!("chosen-target batch contained a non-provenance event"),
            };
            let (request_id, view_id, projection_ordinal, first_case_id) = chosen_claim(&events[0]);
            let (second_request_id, second_view_id, second_ordinal, second_case_id) =
                chosen_claim(&events[1]);
            assert_eq!(second_request_id, request_id);
            assert_eq!(second_view_id, view_id);
            assert_ne!(second_ordinal, projection_ordinal);
            assert_ne!(second_case_id, first_case_id);

            let tampered = RelationalJournalEvent::analysis(
                RelationalAnalysisEvidenceEvent::mechanism_chosen_target_case_accepted(
                    request_id,
                    view_id,
                    projection_ordinal,
                    second_case_id,
                ),
            );
            let error = durable
                .append_events(expected_sequence, expected_head, [tampered])
                .expect_err("a CaseId from another projection ordinal must be rejected");
            assert!(matches!(
                error,
                RelationalDurableJournalError::Journal(RelationalJournalError::Analysis(
                    RelationalAnalysisJournalError::Catalog(
                        RelationalAnalysisCatalogError::ResultProjection(
                            ResultProjectionError::ExpectedRecordMismatch { ordinal }
                        )
                    )
                )) if ordinal == projection_ordinal
            ));
            rejected_tamper = true;
            break;
        }
        assert!(
            rejected_tamper,
            "fixture did not reach its chosen-target provenance batch"
        );
    }

    #[test]
    fn zero_question_stream_closes_with_the_canonical_empty_scope() {
        let temp = TestDirectory::new();
        let run_state = temp.path().join("run-state");
        let output = temp.path().join("output");
        let prepared = prepare(ZERO_QUESTION_PUBLICATION);
        assert!(prepared.checked.view().question_ids().is_empty());
        assert!(prepared.region_replay_authority.is_none());

        let mut epoch = prepared
            .open_epoch(ExploreStreamEpochOptions {
                run_state,
                output_directory: Some(output),
                outer_containment: None,
            })
            .expect("open zero-question publication epoch");
        epoch.resources = ExactStreamOneWorkerEnvelope::new_unmetered_for_test()
            .expect("create deterministic zero-question resource envelope");
        let report = epoch
            .run_slice(None)
            .expect("complete tiny zero-question publication fixture");

        assert_eq!(report.lifecycle, ExploreStreamLifecycle::Complete);
        assert_eq!(report.counts.cases, ExploreStreamCount::Exact(2));
        assert_eq!(report.counts.admitted, ExploreStreamCount::Exact(2));
        assert!(report.identity.question_ids.is_empty());
        assert!(report.finds.is_empty());
        assert!(report.analysis_scope_root.is_some());
        let publication = report.publication.expect("publish zero-question graph");
        assert!(publication
            .artifacts
            .iter()
            .any(|artifact| artifact.key.starts_with("semantic-transition-graph:")));
    }

    #[test]
    fn pre_step_pause_rejects_a_mismatched_registered_analysis_plan_before_publication() {
        let source = r#"
> pause_guard_observer(state: Int, context: Unit) -> Int { state }

? explore pause_guard {
    from {
        vary before in [0, 1]
        given context = ()
    }
    transition after = before
    find all_cases = all
    mechanisms paths from find all_cases using pause_guard_observer
}
"#;
        let prepared = prepare(source);
        let checked = prepared.checked.view();
        let fresh_plan =
            RelationalAnalysisPlan::from_checked(&checked).expect("build fresh checked plan");
        let mut changed_observation = false;
        let registrations = fresh_plan
            .layer_registrations()
            .iter()
            .cloned()
            .map(|registration| match registration {
                RelationalAnalysisLayerRegistration::Mechanisms(mechanism)
                    if !changed_observation =>
                {
                    changed_observation = true;
                    let mut bytes = mechanism.observation_id().bytes();
                    bytes[0] ^= 1;
                    RelationalAnalysisLayerRegistration::Mechanisms(
                        RelationalMechanismLayerRegistration::restore_from_journal_codec(
                            mechanism.request_id(),
                            mechanism.target(),
                            RelationalMechanismObservationId::from_journal_codec_bytes(bytes),
                            mechanism.endpoint_totality_certificate_id(),
                            mechanism.dependencies().to_vec().into_boxed_slice(),
                        ),
                    )
                }
                registration => registration,
            })
            .collect::<Vec<_>>();
        assert!(changed_observation, "fixture must have one mechanism layer");
        let alternate_plan = RelationalAnalysisPlan::restore_from_journal_codec(
            fresh_plan.question_ids().to_vec().into_boxed_slice(),
            fresh_plan.producer_graph_digest(),
            registrations,
        )
        .expect("restore same-graph alternate plan");
        assert_eq!(
            alternate_plan.producer_graph_digest(),
            fresh_plan.producer_graph_digest(),
            "the fixture isolates plan authority from producer-graph authority"
        );
        assert_ne!(alternate_plan.root(), fresh_plan.root());

        let temp = TestDirectory::new();
        let run_state = temp.path().join("run-state");
        let output = temp.path().join("output");
        {
            let alternate_root = alternate_plan.root();
            let mut durable =
                RelationalDurableJournal::open_or_create_with_region_replay_authority(
                    &run_state,
                    prepared.contract.clone(),
                    alternate_root,
                    RelationalDurableJournalLimits::default(),
                    exact_one_region_replay_authority(&prepared),
                )
                .expect("open alternate-plan seed journal under its own authority");
            let (sequence, head) = {
                let journal = durable.journal().expect("inspect seed journal");
                (journal.next_sequence(), journal.head())
            };
            durable
                .append_events(
                    sequence,
                    head,
                    [RelationalJournalEvent::analysis_plan_registered(
                        alternate_plan,
                    )],
                )
                .expect("seed alternate registered plan");
            durable
                .flush_for_pause()
                .expect("flush alternate registered plan");
        }

        let error = match prepare(source).open_epoch(ExploreStreamEpochOptions {
            run_state,
            output_directory: Some(output.clone()),
            outer_containment: None,
        }) {
            Err(error) => error,
            Ok(_) => panic!("mismatched registered plan must fail while opening the journal"),
        };
        assert_eq!(
            error.to_string(),
            "relational journal analysis plan differs from the freshly checked plan"
        );
        assert!(
            !output.join("manifest.json").exists(),
            "mismatched plan state must not reach publication"
        );
    }

    #[test]
    fn hybrid_stream_resumes_materializes_sparse_selected_and_projects_exact_public_closure() {
        let temp = TestDirectory::new();
        let run_state = temp.path().join("run-state");
        let mut prepared = prepare(HYBRID);
        let question_id = prepared.checked.view().question_ids()[0];
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
                    prepared.contract.clone(),
                    prepared.analysis_plan_root,
                    RelationalDurableJournalLimits::default(),
                    exact_one_region_replay_authority(&prepared),
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
                view.classified_support_fragments(question_id).unwrap(),
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
            prepared.contract.clone(),
            prepared.analysis_plan_root,
            RelationalDurableJournalLimits::default(),
            exact_one_region_replay_authority(&prepared),
        )
        .expect("reopen hybrid durable journal");
        let reopened = durable.journal().expect("inspect reopened hybrid journal");
        assert_eq!(reopened.next_sequence(), paused_checkpoint.next_sequence());
        assert_eq!(reopened.head(), paused_checkpoint.head());
        assert!(matches!(
            reopened
                .scheduler_view()
                .expect("inspect replayed hybrid prefix")
                .classified_support_fragments(question_id)
                .unwrap(),
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
        let fragments = view.classified_support_fragments(question_id).unwrap();
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
        assert_eq!(
            view.selected_run_materializations(question_id)
                .unwrap()
                .count(),
            1
        );
        assert!(view
            .selected_run_materializations_cover_classified_prefix(question_id)
            .unwrap());
        assert!(view.support_catalog_is_sealed());
        assert!(journal
            .analysis_state()
            .is_some_and(|analysis| analysis.is_closed()));

        let selected_question = journal
            .analysis_state()
            .and_then(|analysis| analysis.selected_question(question_id))
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
            .verified_case_chunk_partition(question_id)
            .unwrap()
            .expect("hybrid canonical partition");
        let projection = derive_relational_case_support_projection(
            partition,
            fragments,
            |cell_id| {
                view.selected_run_materialization(question_id, cell_id)
                    .unwrap()
            },
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
    fn checked_shared_namespace_query_replays_crash_prefix_with_fresh_runtime() {
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
        let epoch_options = || ExploreStreamEpochOptions {
            run_state: run_state.clone(),
            output_directory: None,
            outer_containment: None,
        };

        // Evaluate the transitive target under the ordinary head-bound permit
        // protocol, then simulate a crash after installing only the durable
        // case consequence. Admission, FIND, and the source cursor remain
        // absent, so a fresh runtime must replan and validate this exact case.
        {
            let mut epoch = prepare_fixture()
                .open_epoch(epoch_options())
                .expect("open shared namespace epoch");
            epoch.resources = ExactStreamOneWorkerEnvelope::new_unmetered_for_test()
                .expect("create deterministic shared namespace resource envelope");
            let RelationalExploreEpoch {
                prepared,
                durable,
                resources,
                ..
            } = &mut epoch;
            let checked = prepared.checked.view();
            let question_id = checked.question_ids()[0];
            let driver =
                RelationalStreamDriver::from_checked_with_limits_and_classification_backends(
                    &checked,
                    &prepared.support_plan,
                    RelationalStreamDriverLimits::default(),
                    None,
                    Some(&prepared.classification_evaluator),
                )
                .expect("build shared namespace stream scheduler");
            let mut crash_case_id = None;
            for _ in 0..32 {
                let (expected_sequence, expected_head) = {
                    let journal = durable
                        .journal()
                        .expect("inspect shared namespace permit subject");
                    (journal.next_sequence(), journal.head())
                };
                let subject = ExactStreamWorkSubject::RelationalJournalQuantum {
                    expected_sequence,
                    expected_head: expected_head.bytes(),
                };
                let owned = resources.conservative_in_process_owned_snapshot();
                let permit = match resources.poll(owned, None, Some(subject)).action {
                    ExactStreamResourceAction::Dispatch(permit) => permit,
                    action => {
                        panic!("unmetered shared namespace quantum was not dispatched: {action:?}")
                    }
                };
                assert_eq!(permit.subject(), subject);
                let in_flight = resources
                    .begin_work(permit)
                    .expect("begin shared namespace relational quantum");
                assert_eq!(in_flight.subject(), subject);

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
                    panic!("shared namespace stream quiesced before evaluating its successor");
                };
                assert_eq!(batch.expected_sequence(), expected_sequence);
                assert_eq!(batch.expected_head(), expected_head);
                let quantum = batch.quantum();
                let mut events = batch.into_events().into_vec();
                let successor = events.iter().enumerate().find_map(|(index, event)| {
                    let RelationalJournalEvent::Evidence(
                        RelationalEvidenceEvent::SuccessorDiscovered { case_id, row, .. },
                    ) = event
                    else {
                        return None;
                    };
                    Some((index, *case_id, row.after().clone()))
                });
                if let Some((successor_index, case_id, after)) = successor {
                    assert!(matches!(
                        quantum,
                        RelationalStreamQuantum::Base(
                            RelationalStepQuantum::SourceMembers { .. }
                                | RelationalStepQuantum::SourceMembersAndBindingExhaustion { .. }
                        )
                    ));
                    assert_eq!(after, super::super::ExploreValue::Int(36));
                    let admission_index = events
                        .iter()
                        .position(|event| {
                            matches!(
                                event,
                                RelationalJournalEvent::Evidence(
                                    RelationalEvidenceEvent::AdmissionClassified { .. }
                                )
                            )
                        })
                        .expect("fused shared namespace admission event");
                    let question_index = events
                        .iter()
                        .position(|event| {
                            matches!(
                                event,
                                RelationalJournalEvent::Evidence(
                                    RelationalEvidenceEvent::QuestionClassified { .. }
                                )
                            )
                        })
                        .expect("fused shared namespace FIND event");
                    let cursor_index = events
                        .iter()
                        .position(|event| {
                            matches!(
                                event,
                                RelationalJournalEvent::Checkpoint(
                                    RelationalCheckpointEvent::WorkCursorAdvanced { .. }
                                )
                            )
                        })
                        .expect("shared namespace source cursor event");
                    assert!(successor_index < admission_index);
                    assert!(admission_index < question_index);
                    assert!(question_index < cursor_index);

                    let append = durable
                        .append_events(
                            expected_sequence,
                            expected_head,
                            events.drain(..=successor_index),
                        )
                        .expect("append crash prefix through shared namespace successor");
                    assert_eq!(
                        append.semantic_event_count().get(),
                        u64::try_from(successor_index + 1).expect("small crash prefix")
                    );
                    resources
                        .finish_or_abandon_work(in_flight)
                        .expect("finish crash-prefix relational quantum");
                    crash_case_id = Some(case_id);
                    break;
                }

                durable
                    .append_events(expected_sequence, expected_head, events)
                    .expect("append shared namespace setup quantum");
                resources
                    .finish_or_abandon_work(in_flight)
                    .expect("finish shared namespace setup quantum");
            }
            let crash_case_id = crash_case_id
                .expect("shared namespace fixture did not emit its singleton successor");
            let journal = durable
                .journal()
                .expect("inspect durable shared namespace crash prefix");
            let scheduler = journal
                .scheduler_view()
                .expect("inspect shared namespace crash-prefix scheduler");
            assert_eq!(scheduler.source_count(), 1);
            assert_eq!(scheduler.case_count(), 1);
            assert_eq!(scheduler.admission_decision_count(), 0);
            assert_eq!(scheduler.question_decision_count(question_id).unwrap(), 0);
            assert_eq!(
                scheduler
                    .case(crash_case_id)
                    .expect("durable shared namespace crash-prefix case")
                    .after(),
                &super::super::ExploreValue::Int(36)
            );
            durable
                .flush_for_pause()
                .expect("flush shared namespace successor crash prefix");
        }

        // Rechecking constructs a fresh interpreter/import closure. Because
        // the source cursor was not installed, normal active execution must
        // recompute TO and validate the candidate against durable after=36.
        let mut epoch = prepare_fixture()
            .open_epoch(epoch_options())
            .expect("reopen shared namespace epoch");
        epoch.resources = ExactStreamOneWorkerEnvelope::new_unmetered_for_test()
            .expect("create deterministic resumed resource envelope");
        let report = epoch
            .run_slice(None)
            .expect("resume shared namespace through the normal active slice");
        assert_eq!(report.lifecycle, ExploreStreamLifecycle::Complete);
        assert_eq!(report.counts.sources, ExploreStreamCount::Exact(1));
        assert_eq!(report.counts.cases, ExploreStreamCount::Exact(1));
        assert_eq!(report.counts.admitted, ExploreStreamCount::Exact(1));
        assert_eq!(report.finds.len(), 1);
        assert_eq!(report.finds[0].selected, ExploreStreamCount::Exact(1));

        let checked = epoch.prepared.checked.view();
        let journal = epoch
            .durable
            .journal()
            .expect("inspect completed shared namespace journal");
        let scheduler = journal
            .scheduler_view()
            .expect("inspect completed shared namespace scheduler");
        let question_id = checked.question_ids()[0];
        assert_eq!(scheduler.case_count(), 1);
        assert_eq!(scheduler.selected_count(question_id).unwrap(), 1);
        let analysis = journal
            .analysis_state()
            .expect("completed shared namespace analysis");
        assert!(analysis.is_closed());
        assert_eq!(
            analysis
                .selected_question(question_id)
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
