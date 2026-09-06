//! Public invocation/report boundary for the resumable relational Explore engine.
//!
//! The durable journal is the source of truth.  This adapter checks and binds
//! one query, advances its semantic stream under the resource envelope, and
//! projects only compact counters, including how many result records this
//! invocation appended. It deliberately does not clone the complete relation
//! merely to print a checkpoint.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
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
    CheckedExploreSourceCoverageManifest, CheckedExploreSourceProjectionFactorKind, Diagnostic,
    ExploreAdmissionScope, Expr, OwnedCheckedExploreQuery, RuleDispatchKey, Stmt, Ty,
    TypeCheckArtifacts, TypeChecker,
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
use super::relational_classified_population::RelationalClassificationProgressCounts;
use super::relational_durable_journal::{RelationalDurableJournal, RelationalDurableJournalLimits};
use super::relational_interpreter_mechanism::{
    checked_ground_definitions, RelationalInterpreterMechanismReplayRuntime,
};
use super::relational_journal::{RelationalJournal, RelationalJournalContract};
use super::relational_mechanism_step_driver::RelationalStructuralArtifactCache;
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
    relational_tys_equivalent, ExploreAnalysisNodeIr, ExploreExactDomain, ExploreFindIr,
    ExploreFiniteDomainIr, ExploreFiniteTypePlan, ExploreMechanismTargetIr, ExploreResultGrainIr,
    ExploreResultInputIr, ExploreResultViewIr, ExploreSourceBindingKindIr, ExploreSuccessorKindIr,
    RelationalInterpreterExpressionRuntime, RelationalSupportPlan, RelationalSupportPlanner,
};

pub const EXPLORE_RELATIONAL_STREAM_REPORT_VERSION: u32 = 11;

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
/// containment. The private fields prevent safe construction from merely
/// plausible numbers; ordinary library callers leave `outer_containment` as
/// `None`.
///
/// ```compile_fail
/// use futuruna::explore::ExploreStreamOuterContainment;
/// use std::num::NonZeroU64;
///
/// let gib = NonZeroU64::new(1024 * 1024 * 1024).unwrap();
/// let _unattested = ExploreStreamOuterContainment {
///     rust_heap_limit_bytes: gib,
///     untracked_memory_reserve_bytes: gib,
///     group_rss_limit_bytes: gib,
///     available_memory_floor_bytes: gib,
/// };
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExploreStreamOuterContainment {
    rust_heap_limit_bytes: NonZeroU64,
    untracked_memory_reserve_bytes: NonZeroU64,
    group_rss_limit_bytes: NonZeroU64,
    available_memory_floor_bytes: NonZeroU64,
}

impl ExploreStreamOuterContainment {
    /// Attest that the current process is already inside the described live
    /// containment boundary.
    ///
    /// # Safety
    ///
    /// Before calling this function, the caller must have installed the exact
    /// Rust-heap cap and arranged for an independently runnable supervisor to
    /// continuously contain the complete process group at the supplied RSS
    /// limit, available-memory floor, critical memory pressure, throttled-page
    /// signal, and telemetry-loss boundaries. It must also continuously sample
    /// total host CPU, carry and repay host-CPU budget debt, and pace the
    /// complete process group so cumulative use stays within the authorized
    /// 80%-of-installed-CPU boundary. That supervisor must outlive every
    /// Explore epoch receiving this process-local value. The attestation must
    /// never be persisted or transferred to another process.
    pub unsafe fn attest_current_process_is_supervised(
        rust_heap_limit_bytes: NonZeroU64,
        untracked_memory_reserve_bytes: NonZeroU64,
        group_rss_limit_bytes: NonZeroU64,
        available_memory_floor_bytes: NonZeroU64,
    ) -> Self {
        Self {
            rust_heap_limit_bytes,
            untracked_memory_reserve_bytes,
            group_rss_limit_bytes,
            available_memory_floor_bytes,
        }
    }

    pub fn rust_heap_limit_bytes(self) -> NonZeroU64 {
        self.rust_heap_limit_bytes
    }

    pub fn untracked_memory_reserve_bytes(self) -> NonZeroU64 {
        self.untracked_memory_reserve_bytes
    }

    pub fn group_rss_limit_bytes(self) -> NonZeroU64 {
        self.group_rss_limit_bytes
    }

    pub fn available_memory_floor_bytes(self) -> NonZeroU64 {
        self.available_memory_floor_bytes
    }
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
    /// Query-bound deterministic structural producer retained across warm
    /// slice-driver reconstruction. Durable journal evidence remains the
    /// authority for every resume and cold invocation.
    structural_artifact_cache: Arc<RelationalStructuralArtifactCache>,
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
/// Independent finite integer ranges and one bounded exact finite-type factor
/// become scalar function inputs. The structured factor is represented by its
/// producer-issued canonical ordinal and reconstructed inside the classifier.
/// Singleton bindings retain their checked expression and are replayed in
/// authored source order, so derived records such as a profile and composite
/// `Before` state retain exactly the checked relation semantics.
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
    ExactFiniteOrdinalInput {
        plan: ExploreFiniteTypePlan,
        exact_cardinality: u128,
        plan_digest: [u8; 32],
    },
    Singleton {
        value: Expr,
    },
}

/// Producer-minted RuleDispatch type contracts for the frozen classifier slice.
///
/// Rechecking a pruned declaration graph can lose whole-program type evidence
/// even though every retained occurrence came from the same checked program.
/// These contracts deliberately grant no universal dispatch-totality claim:
/// the native classifier turns an actual partial miss into process failure and
/// the coordinator retries the whole batch through the checked interpreter.
#[doc(hidden)]
#[derive(Clone, Debug)]
pub struct ExploreNativeClassifierRuleMetadataV2 {
    pub checked_program: [u8; 32],
    pub return_types: BTreeMap<RuleDispatchKey, String>,
    pub return_issues: BTreeMap<RuleDispatchKey, String>,
    pub parameter_types: BTreeMap<RuleDispatchKey, Vec<Option<String>>>,
    pub parameter_names: BTreeMap<RuleDispatchKey, Vec<Option<String>>>,
    pub parameter_issues: BTreeSet<RuleDispatchKey>,
    pub boolean_miss_safe_keys: BTreeSet<RuleDispatchKey>,
    pub runtime_irrefutable_keys: BTreeSet<RuleDispatchKey>,
}

/// Classification-only compiler input for native classifier V2.
///
/// V2 accepts one or more independent, statically bounded `Int` ranges and at
/// most one bounded exact finite-type factor, mixed with ordered
/// singleton/derived source bindings, followed by a singleton successor,
/// scoped admissions, and All/Matches/Violations FIND. Finite scalars are
/// operational accelerator inputs only: they never become CaseIds, source
/// identities, transcript coordinates, or journal evidence.
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
    pub rule_metadata: ExploreNativeClassifierRuleMetadataV2,
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
            driver_limits: RelationalStreamDriverLimits::default(),
        })
    }
}

fn native_classifier_plan_v2_from_checked(
    checked: &CheckedExploreQueryView<'_>,
    checked_declarations: Box<[Stmt]>,
    compile_time_metadata_bindings: BTreeSet<String>,
    rule_metadata: ExploreNativeClassifierRuleMetadataV2,
) -> Option<ExploreNativeClassifierPlanV2> {
    let query = checked.closed_query;
    query.validate().ok()?;
    if query.source.bindings.is_empty() {
        return None;
    }

    let mut source_bindings = Vec::with_capacity(query.source.bindings.len());
    let mut finite_input_binding_indices = Vec::new();
    let mut finite_coordinate_count = 1u128;
    let source_projection = checked.source_image_projection();
    let mut structured_factor_count = 0usize;
    for (position, binding) in query.source.bindings.iter().enumerate() {
        if binding.binding_index != position || binding.name.is_empty() {
            return None;
        }
        let kind = match &binding.kind {
            ExploreSourceBindingKindIr::Finite { domain } => match domain {
                ExploreFiniteDomainIr::IntRange { .. } => {
                    if !binding.dependencies.is_empty()
                        || !native_classifier_int_ty(&binding.value_ty)
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
                ExploreFiniteDomainIr::Exact(ExploreExactDomain::FiniteType { ty, plan }) => {
                    if !binding.dependencies.is_empty()
                        || !relational_tys_equivalent(ty, &binding.value_ty)
                        || structured_factor_count != 0
                    {
                        return None;
                    }
                    let exact_cardinality = plan.cardinality().exact()?;
                    if exact_cardinality == 0 || exact_cardinality > i64::MAX as u128 {
                        return None;
                    }
                    let plan_digest = crate::checked_explore_finite_plan_digest(plan);
                    let projection_factor = source_projection?.factors.iter().find(|factor| {
                        usize::try_from(factor.binding_index).ok() == Some(binding.binding_index)
                    })?;
                    if projection_factor.exact_cardinality != exact_cardinality
                        || !matches!(
                            projection_factor.kind,
                            CheckedExploreSourceProjectionFactorKind::ExactFinite {
                                plan_digest: certified_plan_digest,
                            } if certified_plan_digest == plan_digest
                        )
                    {
                        return None;
                    }
                    structured_factor_count += 1;
                    finite_coordinate_count =
                        finite_coordinate_count.checked_mul(exact_cardinality)?;
                    finite_input_binding_indices.push(binding.binding_index);
                    ExploreNativeClassifierSourceBindingKindV2::ExactFiniteOrdinalInput {
                        plan: plan.clone(),
                        exact_cardinality,
                        plan_digest,
                    }
                }
                _ => return None,
            },
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
    if rule_metadata.checked_program != checked_program {
        return None;
    }

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
        rule_metadata,
    })
}

fn native_classifier_rule_metadata_v2(
    artifacts: &TypeCheckArtifacts,
    checked_program: [u8; 32],
) -> ExploreNativeClassifierRuleMetadataV2 {
    let return_types = artifacts
        .checked_resolutions
        .rule_dispatch_type_contracts
        .iter()
        .map(|(key, contract)| (key.clone(), contract.result_type.to_string()))
        .collect::<BTreeMap<_, _>>();
    let parameter_types = artifacts
        .checked_resolutions
        .rule_dispatch_type_contracts
        .iter()
        .map(|(key, contract)| {
            (
                key.clone(),
                contract
                    .parameter_types
                    .iter()
                    .map(|parameter| parameter.as_ref().map(ToString::to_string))
                    .collect(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let return_issues = artifacts
        .checked_resolutions
        .rule_families
        .keys()
        .filter(|key| !return_types.contains_key(*key))
        .cloned()
        .map(|key| {
            (
                key,
                "checked RuleDispatch family has no conflict-free type contract".to_string(),
            )
        })
        .collect();
    ExploreNativeClassifierRuleMetadataV2 {
        checked_program,
        return_types,
        return_issues,
        parameter_types,
        parameter_names: artifacts.rule_dispatch_parameter_names.clone(),
        parameter_issues: artifacts.rule_dispatch_parameter_issues.clone(),
        boolean_miss_safe_keys: artifacts.rule_dispatch_boolean_miss_safe_keys.clone(),
        runtime_irrefutable_keys: artifacts.rule_dispatch_runtime_irrefutable_keys.clone(),
    }
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
    /// Operational only. Production epochs use the stable default; focused
    /// protocol tests may shrink artifact chunks without changing identity.
    driver_limits: RelationalStreamDriverLimits,
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
    Choice {
        name: String,
        question_id: String,
        choice_id: String,
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
    AwaitingChoiceMechanisms {
        request_id: String,
        choice_id: String,
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
    ChoiceInputOpen,
    ChoiceMembersOpen,
    ChoiceClosed,
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
    Choice { choice_id: String },
    MechanismIncidence { name: String, request_id: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExploreStreamChoiceLayer {
    pub name: String,
    pub choice_id: String,
    pub question_id: String,
    pub status: ExploreStreamLayerStatus,
    pub candidates: ExploreStreamCount,
    pub members: ExploreStreamCount,
    pub frontier_root: Option<String>,
    pub content_root: Option<String>,
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
    /// Stable membership identity when this display view materializes a
    /// canonical choice relation. Display-only SELECT/privacy changes may
    /// change `view_id` without changing this identity.
    pub choice_id: Option<String>,
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
    Choice(ExploreStreamChoiceLayer),
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
            let checked_program = decode_lowercase_sha256(checked.program_hash())?;
            native_classifier_plan_v2_from_checked(
                &checked,
                checked_declarations,
                artifacts.compile_time_metadata_bindings.clone(),
                native_classifier_rule_metadata_v2(&artifacts, checked_program),
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
        structural_artifact_cache: Arc::new(RelationalStructuralArtifactCache::default()),
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
        self.run_slice_with_budget(budget)
    }

    #[cfg(test)]
    fn run_one_batch_slice_for_test(
        &mut self,
    ) -> Result<ExploreStreamSliceReport, ExploreStreamPreparationError> {
        let budget = RelationalStreamSliceBudget::new(None)
            .expect("unlimited test slice budget is valid")
            .with_max_semantic_batches(NonZeroU64::MIN);
        self.run_slice_with_budget(budget)
    }

    fn run_slice_with_budget(
        &mut self,
        budget: RelationalStreamSliceBudget,
    ) -> Result<ExploreStreamSliceReport, ExploreStreamPreparationError> {
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
            structural_artifact_cache,
            preparation_wall_time: _,
        } = &mut self.prepared;
        let checked = checked.view();
        let driver = RelationalStreamDriver::from_checked_with_limits_classification_backends_and_structural_cache(
                &checked,
                support_plan,
                self.driver_limits,
                native_classifier.clone(),
                Some(classification_evaluator),
                Arc::clone(structural_artifact_cache),
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
            CheckedExploreAnalysisIdentity::View { view_id, .. } => match analysis {
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
                            RelationalAnalysisLayerSnapshot::Choice(_) => None,
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
    // A shared classified sweep has one admission partition and one selection
    // partition per canonical QuestionId. Derive every question explicitly:
    // this keeps named FIND reports independent without nominating a semantic
    // primary, while also checking that their shared admission accounting is
    // identical.
    let mut classification_progress = BTreeMap::new();
    let mut classification_progress_available = None;
    let mut classification_admission_progress: Option<RelationalClassificationProgressCounts> =
        None;
    for question_id in checked.question_ids().iter().copied() {
        let question_progress = scheduler
            .classification_progress_counts(question_id)
            .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?;
        match classification_progress_available {
            Some(expected) if expected != question_progress.is_some() => {
                return Err(ExploreStreamPreparationError::Execution(
                    "named FINDs disagree on classified-support availability".into(),
                ));
            }
            None => classification_progress_available = Some(question_progress.is_some()),
            Some(_) => {}
        }
        if let Some(classified) = question_progress {
            if let Some(certified) = certified_case_cardinality {
                if classified.candidates() != certified {
                    return Err(ExploreStreamPreparationError::Execution(format!(
                        "classified case candidate count {} does not match certified root cardinality {certified}",
                        classified.candidates()
                    )));
                }
            }
            if let Some(shared) = classification_admission_progress {
                if classified.candidates() != shared.candidates()
                    || classified.classified() != shared.classified()
                    || classified.admitted() != shared.admitted()
                    || classified.rejected() != shared.rejected()
                    || classified.is_complete() != shared.is_complete()
                {
                    return Err(ExploreStreamPreparationError::Execution(
                        "named FINDs disagree on their shared admission classification progress"
                            .into(),
                    ));
                }
            } else {
                classification_admission_progress = Some(classified);
            }
        }
        if classification_progress
            .insert(question_id, question_progress)
            .is_some()
        {
            return Err(ExploreStreamPreparationError::Execution(
                "checked question identity set contains a duplicate QuestionId".into(),
            ));
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
    let support_complete =
        classification_admission_progress.is_some_and(|counts| counts.is_complete());
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
        classification_admission_progress.map(|counts| (counts.classified(), counts.is_complete())),
        admission_classified,
        admission_closed_extensional,
        certified_admission_counts.map(|(classified, _, _)| classified),
    )?;
    let admitted_count = merge_population_count(
        "admitted cases",
        classification_admission_progress.map(|counts| (counts.admitted(), counts.is_complete())),
        admitted,
        admission_closed_extensional,
        certified_admission_counts.map(|(_, admitted, _)| admitted),
    )?;
    let rejected_count = merge_population_count(
        "rejected cases",
        classification_admission_progress.map(|counts| (counts.rejected(), counts.is_complete())),
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
        let question_progress = classification_progress.get(&question_id).copied().flatten();
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
    let mut emitted_choices = BTreeSet::new();
    let mut preview_budget = ExploreStreamPreviewBudget::default();
    for (index, (node, identity)) in checked.analysis_nodes().enumerate() {
        let name = node.name().to_string();
        match (node, identity) {
            (
                ExploreAnalysisNodeIr::Result(view),
                CheckedExploreAnalysisIdentity::View { view_id, choice_id },
            ) => {
                if let Some(choice_id) = choice_id {
                    if emitted_choices.insert(*choice_id) {
                        layers.push(ExploreStreamLayer::Choice(choice_layer(
                            analysis, checked, view, *choice_id,
                        )?));
                    }
                }
                layers.push(ExploreStreamLayer::Result(result_layer(
                    analysis,
                    checked,
                    index,
                    view,
                    *view_id,
                    *choice_id,
                    projection_starts.get(index).copied().unwrap_or(0),
                    &mut preview_budget,
                )?));
            }
            (
                ExploreAnalysisNodeIr::Mechanisms(request),
                CheckedExploreAnalysisIdentity::Mechanisms { request_id, .. },
            ) => layers.push(ExploreStreamLayer::Mechanisms(mechanism_layer(
                journal,
                name,
                *request_id,
                public_mechanism_target(checked, &request.target)?,
            )?)),
            _ => {
                return Err(ExploreStreamPreparationError::Execution(format!(
                    "checked analysis identity kind diverged at node {index}"
                )))
            }
        }
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
            let CheckedExploreAnalysisIdentity::View {
                view_id: _,
                choice_id,
            } = identity
            else {
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
            let choice_id = choice_id.ok_or_else(|| {
                ExploreStreamPreparationError::Execution(format!(
                    "mechanism target analysis node {view_node_index} has no canonical ChoiceId"
                ))
            })?;
            Ok(ExploreStreamMechanismTarget::Choice {
                name: view.name.clone(),
                question_id: hex(question_id.bytes()),
                choice_id: hex(choice_id.bytes()),
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

fn choice_layer(
    analysis: Option<&super::RelationalAnalysisJournalState>,
    checked: &CheckedExploreQueryView<'_>,
    view: &ExploreResultViewIr,
    choice_id: super::ChoiceId,
) -> Result<ExploreStreamChoiceLayer, ExploreStreamPreparationError> {
    let ExploreResultInputIr::Find {
        find_name: _,
        find_index,
    } = &view.input
    else {
        return Err(ExploreStreamPreparationError::Execution(
            "a checked Choice relation is not FIND-backed".into(),
        ));
    };
    let question_id = checked.find_question_id(*find_index).ok_or_else(|| {
        ExploreStreamPreparationError::Execution(format!(
            "choice for result {} has no aligned QuestionId",
            view.name
        ))
    })?;
    let absent = || ExploreStreamChoiceLayer {
        name: view.name.clone(),
        choice_id: hex(choice_id.bytes()),
        question_id: hex(question_id.bytes()),
        status: ExploreStreamLayerStatus::ChoiceInputOpen,
        candidates: ExploreStreamCount::LowerBound(0),
        members: ExploreStreamCount::Unknown {
            confirmed_lower_bound: 0,
        },
        frontier_root: None,
        content_root: None,
    };
    let Some(analysis) = analysis else {
        return Ok(absent());
    };
    match (analysis.open_catalog(), analysis.closed_catalog()) {
        (Some(open), None) => {
            let relation = open
                .choice_relation(choice_id)
                .map_err(|error| ExploreStreamPreparationError::Execution(error.to_string()))?;
            let counts = relation.counts();
            Ok(ExploreStreamChoiceLayer {
                name: view.name.clone(),
                choice_id: hex(choice_id.bytes()),
                question_id: hex(question_id.bytes()),
                status: layer_status(
                    open.layer_status(RelationalAnalysisLayerId::Choice(choice_id))
                        .ok_or_else(|| {
                            ExploreStreamPreparationError::Execution(
                                "analysis omitted a declared choice layer".into(),
                            )
                        })?,
                ),
                candidates: public_choice_count(counts.candidates()),
                members: public_choice_count(counts.members()),
                frontier_root: Some(hex(relation.frontier_root().bytes())),
                content_root: relation.content_root().map(|root| hex(root.bytes())),
            })
        }
        (None, Some(closed)) => {
            let layer = closed
                .snapshot()
                .layer(RelationalAnalysisLayerId::Choice(choice_id))
                .ok_or_else(|| {
                    ExploreStreamPreparationError::Execution(
                        "closed analysis omitted a declared choice layer".into(),
                    )
                })?;
            let RelationalAnalysisLayerSnapshot::Choice(choice) = layer else {
                return Err(ExploreStreamPreparationError::Execution(
                    "closed analysis choice identity names another layer kind".into(),
                ));
            };
            let relation = choice.relation();
            Ok(ExploreStreamChoiceLayer {
                name: view.name.clone(),
                choice_id: hex(choice_id.bytes()),
                question_id: hex(question_id.bytes()),
                status: layer_status(choice.status()),
                candidates: ExploreStreamCount::Exact(relation.candidates().len() as u128),
                members: ExploreStreamCount::Exact(relation.members().len() as u128),
                frontier_root: Some(hex(relation.frontier_root().bytes())),
                content_root: relation.content_root().map(|root| hex(root.bytes())),
            })
        }
        _ => Err(ExploreStreamPreparationError::Execution(
            "analysis state does not own exactly one catalog".into(),
        )),
    }
}

const fn public_choice_count(count: super::ChoiceCount) -> ExploreStreamCount {
    match count {
        super::ChoiceCount::LowerBound(value) => ExploreStreamCount::LowerBound(value),
        super::ChoiceCount::Provisional(value) => ExploreStreamCount::Unknown {
            confirmed_lower_bound: value,
        },
        super::ChoiceCount::Exact(value) => ExploreStreamCount::Exact(value),
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
    choice_id: Option<super::ChoiceId>,
    start: usize,
    preview_budget: &mut ExploreStreamPreviewBudget,
) -> Result<ExploreStreamResultLayer, ExploreStreamPreparationError> {
    let input = match choice_id {
        Some(choice_id) => ExploreStreamResultInput::Choice {
            choice_id: hex(choice_id.bytes()),
        },
        None => public_result_input(checked, node_index, &view.input)?,
    };
    let grain = if choice_id.is_some() {
        ExploreStreamResultGrain::EachCase
    } else {
        public_result_grain(&view.grain)
    };
    let columns = public_result_columns(&view.select);
    let group_keys = match (choice_id, &view.grain) {
        (Some(_), _) => Vec::new(),
        (None, ExploreResultGrainIr::GroupBy { fields, .. }) => public_result_columns(fields),
        (
            None,
            ExploreResultGrainIr::EachCase { .. }
            | ExploreResultGrainIr::EachIncidence { .. }
            | ExploreResultGrainIr::GroupAll { .. },
        ) => Vec::new(),
    };
    let absent = || ExploreStreamResultLayer {
        name: view.name.clone(),
        view_id: hex(view_id.bytes()),
        choice_id: choice_id.map(|choice_id| hex(choice_id.bytes())),
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
                choice_id: choice_id.map(|choice_id| hex(choice_id.bytes())),
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
                choice_id: choice_id.map(|choice_id| hex(choice_id.bytes())),
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
            RelationalStreamQuiescence::AwaitingChoiceMechanisms {
                request_id,
                choice_id,
            } => ExploreStreamPauseReason::AwaitingChoiceMechanisms {
                request_id: hex(request_id.bytes()),
                choice_id: hex(choice_id.bytes()),
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
        RelationalAnalysisLayerStatus::ChoiceInputOpen => ExploreStreamLayerStatus::ChoiceInputOpen,
        RelationalAnalysisLayerStatus::ChoiceMembersOpen => {
            ExploreStreamLayerStatus::ChoiceMembersOpen
        }
        RelationalAnalysisLayerStatus::ChoiceClosed => ExploreStreamLayerStatus::ChoiceClosed,
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
    use std::num::{NonZeroU16, NonZeroU32};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    use super::super::mechanism_incidence::MechanismTargetCaseSetCommitment;
    use super::super::relation::{AdmissionDecision, RelationalCaseId, SelectionDecision};
    use super::super::relational_analysis_catalog::RelationalAnalysisCatalogError;
    use super::super::relational_analysis_journal::{
        RelationalAnalysisEvidenceEvent, RelationalAnalysisJournalError,
        RelationalMechanismArtifactClaim,
    };
    use super::super::relational_analysis_plan::{
        RelationalAnalysisLayerRegistration, RelationalAnalysisPlan,
        RelationalMechanismLayerRegistration, RelationalMechanismObservationId,
    };
    use super::super::relational_candidate_schedule::{
        RelationalCandidateNominationRoot, RelationalCandidateScheduleReason,
    };
    use super::super::relational_case_executor::RelationalCaseExecutor;
    use super::super::relational_case_support_projection::{
        derive_relational_case_support_projection, relational_case_support_active_set_root,
        RelationalCaseSupportClosureAuthority, RelationalCaseSupportCount,
        RelationalCaseSupportOpenReason, RelationalCaseSupportProjection,
        RelationalCaseSupportProjectionFrontier, RelationalCaseSupportProjectionRecord,
        RelationalCaseSupportRecordKey, RelationalCaseSupportRow, RelationalCaseSupportRowHash,
    };
    use super::super::relational_classified_sweep_step_driver::{
        RelationalClassifiedSweepStepDriver, RelationalClassifiedSweepStepOutcome,
    };
    use super::super::relational_durable_journal::{
        RelationalDurableJournal, RelationalDurableJournalError, RelationalDurableJournalLimits,
    };
    use super::super::relational_executor::RelationalSourceEnumerator;
    use super::super::relational_frontier::{WorkCompletionRef, WorkNodeSpec};
    use super::super::relational_journal::{
        RelationalCheckpointEvent, RelationalClassifiedSupportFragment, RelationalEvidenceEvent,
        RelationalJournal, RelationalJournalError, RelationalJournalEvent,
        RelationalJournalSnapshot, RelationalSchedulerDecision,
    };
    use super::super::relational_mechanism_step_driver::RelationalMechanismStepQuantum;
    use super::super::relational_proof_strategy::{
        RelationalGuardOrigin, RelationalProofStrategyInventory,
    };
    use super::super::relational_region_proof::{
        derive_relational_lifted_affine_guard_atoms, derive_relational_source_event_guard_atoms,
    };
    use super::super::relational_result_step_driver::RelationalResultStepQuantum;
    use super::super::relational_step_driver::{
        RelationalConcreteQuiescence, RelationalStepDriver, RelationalStepOutcome,
        RelationalStepQuantum,
    };
    use super::super::relational_stream_driver::{
        RelationalStreamDriver, RelationalStreamDriverLimits, RelationalStreamQuantum,
        RelationalStreamStepOutcome,
    };
    use super::super::relational_support_step_driver::RelationalSupportStepQuantum;
    use super::super::result_projection::ResultProjectionError;
    use super::super::stream_resource::{
        ExactStreamOneWorkerEnvelope, ExactStreamResourceAction, ExactStreamWorkSubject,
    };
    use super::super::support_journal::SupportJournalEvent;
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

    const CANDIDATE_RESULT_CONTENTION: &str = r#"
? explore candidate_result_contention {
    from {
        vary before in range(0, 300)
        given context = ()
    }

    transition after = before + 1
    find cases = matches of before >= 280
    results selected_rows from find cases {
        each case
        select [before, after]
    }
}
"#;

    const THREE_CHUNK_CANDIDATE_RESIDUAL: &str = r#"
> above_threshold(x: Int) -> Bool { x >= 680 }

> three_chunk_observer(state: Int, context: Unit) -> Int {
    if state < 690 { 0 } else { 1 }
}

? explore three_chunk_candidate_residual {
    from {
        vary before in range(0, 700)
        given context = ()
    }

    transition after = before + 1
    find cases = matches of above_threshold(before)
    mechanisms threshold_path from find cases using three_chunk_observer
}
"#;

    const THREE_CHUNK_SOURCE_EVENT_RESIDUAL: &str = r#"
| source_event_rate(value: Int) -> 0
| exception threshold source_event_rate(value: Int) -> 1 under value >= 680

> three_chunk_observer(state: Int, context: Unit) -> Int {
    if state < 690 { 0 } else { 1 }
}

? explore three_chunk_source_event_residual {
    from {
        vary before in range(0, 700)
        given context = ()
    }

    transition after = before + 1
    find cases = matches of source_event_rate(before) == 1
    mechanisms threshold_path from find cases using three_chunk_observer
}
"#;

    const SHARED_PLURAL_CLASSIFIED_SWEEP: &str = r#"
? explore shared_plural_classified_sweep {
    from {
        vary before in range(0, 300)
        given context = ()
    }

    transition after = before + 1
    find final_twenty = matches of before >= 280
    find final_ten = matches of before >= 290
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
    find all_cases = matches of before * before >= 0
    results winner from find all_cases {
        group all
        measure [score = before / 2]
        select [case_id, before, after, score]
        choose all maximizing score
    }
    mechanisms winner_path from view winner chosen using chosen_target_observer
}
"#;

    const SHARED_CHOICE_DISPLAY_FAILURE: &str = r#"
> chosen_target_observer(state: Int, context: Unit) -> Int {
    if state < 2 { 0 } else { 1 }
}

> fail_display(value: Int) -> Int {
    100 / value
}

? explore shared_choice_display_failure {
    from {
        vary before in range(0, 4)
        given context = ()
    }

    transition after = before + 1
    find all_cases = all
    results good_display from find all_cases {
        group all
        measure [score = before / 2]
        select [case_id, shown = before]
        choose all maximizing score
    }
    results bad_display from find all_cases {
        group all
        measure [score = before / 2]
        select [case_id, shown = fail_display(before)]
        choose all maximizing score
    }
    mechanisms winner_path from view good_display chosen using chosen_target_observer
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

    fn case_support_add_sequence(
        projection: &RelationalCaseSupportProjection<'_>,
    ) -> Vec<(RelationalCaseSupportRecordKey, RelationalCaseSupportRowHash)> {
        (0..projection.available_source_record_count())
            .filter_map(|ordinal| {
                match projection
                    .record_at(ordinal)
                    .expect("read case/support projection record")
                    .expect("case/support projection ordinal exists")
                {
                    RelationalCaseSupportProjectionRecord::Add { key, row_hash, .. } => {
                        Some((key, row_hash))
                    }
                    RelationalCaseSupportProjectionRecord::Seal(_) => None,
                }
            })
            .collect()
    }

    #[derive(Debug, Eq, PartialEq)]
    struct CanonicalExhaustiveCounts {
        candidates: u128,
        rejected: u128,
        admitted: u128,
        admitted_not_selected: u128,
        admitted_selected: u128,
        selected_case_ids: BTreeSet<RelationalCaseId>,
    }

    fn canonical_exhaustive_three_chunk_oracle(source_text: &str) -> CanonicalExhaustiveCounts {
        canonical_exhaustive_product_oracle(source_text, &[700])
    }

    fn canonical_exhaustive_product_oracle(
        source_text: &str,
        radices: &[u128],
    ) -> CanonicalExhaustiveCounts {
        let mut prepared = prepare(source_text);
        let checked = prepared.checked.view();
        let question_id = checked.question_ids()[0];
        let source =
            RelationalSourceEnumerator::new(checked.relation_id(), &checked.closed_query.source)
                .expect("build independent canonical source enumerator");
        let cases = RelationalCaseExecutor::new(checked.relation_id(), checked.closed_query)
            .expect("build independent canonical case executor");
        let questions = cases
            .checked_question_evaluation_plan(&checked)
            .expect("bind independent canonical FIND evaluator");
        let mut oracle = CanonicalExhaustiveCounts {
            candidates: 0,
            rejected: 0,
            admitted: 0,
            admitted_not_selected: 0,
            admitted_selected: 0,
            selected_case_ids: BTreeSet::new(),
        };

        for rank in 0..radices.iter().product::<u128>() {
            let mut remaining = rank;
            let mut coordinates = vec![0; radices.len()];
            for (coordinate, radix) in coordinates.iter_mut().zip(radices).rev() {
                *coordinate = remaining % radix;
                remaining /= radix;
            }
            let completed = source
                .completed_source_at_independent_finite_ordinals(
                    &coordinates,
                    &mut prepared.expression_runtime,
                )
                .expect("enumerate independent canonical source coordinate");
            let transition = cases
                .statically_singleton_transition(
                    completed.source_key(),
                    completed.row(),
                    &mut prepared.expression_runtime,
                )
                .expect("evaluate independent canonical transition")
                .expect("fixture transition is statically singleton");
            let (case, _) = transition.into_parts();
            let classification = cases
                .classify(
                    completed.row(),
                    &case,
                    &questions,
                    &mut prepared.expression_runtime,
                )
                .expect("classify independent canonical case");
            oracle.candidates += 1;
            match classification.admission() {
                AdmissionDecision::Rejected => {
                    oracle.rejected += 1;
                    assert_eq!(classification.question_decision(question_id), None);
                }
                AdmissionDecision::Admitted => {
                    oracle.admitted += 1;
                    match classification
                        .question_decision(question_id)
                        .expect("admitted oracle case has FIND evidence")
                    {
                        SelectionDecision::NotSelected => oracle.admitted_not_selected += 1,
                        SelectionDecision::Selected => {
                            oracle.admitted_selected += 1;
                            assert!(oracle.selected_case_ids.insert(classification.case_id()));
                        }
                    }
                }
            }
        }
        oracle
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
        let target = concrete
            .next_target(
                journal
                    .scheduler_view()
                    .expect("inspect direct classifier prefix"),
            )
            .expect("select direct concrete target");
        let RelationalClassifiedSweepStepOutcome::Emitted(partial) = concrete
            .step(
                &journal,
                target.as_ref(),
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

        assert_eq!(report.schema_version, 11);
        assert_eq!(report.lifecycle, ExploreStreamLifecycle::Complete);
        assert_eq!(report.counts.cases, ExploreStreamCount::Exact(2));
        assert_eq!(report.counts.admitted, ExploreStreamCount::Exact(2));
        assert_eq!(report.identity.question_ids.len(), 2);
        assert_eq!(report.finds.len(), 2);
        assert_eq!(report.finds[0].name, "all_cases");
        assert_eq!(
            report.finds[0].find_classified,
            ExploreStreamCount::Exact(2)
        );
        assert_eq!(report.finds[0].selected, ExploreStreamCount::Exact(2));
        assert_eq!(report.finds[0].not_selected, ExploreStreamCount::Exact(0));
        assert_eq!(report.finds[1].name, "upper_case");
        assert_eq!(
            report.finds[1].find_classified,
            ExploreStreamCount::Exact(2)
        );
        assert_eq!(report.finds[1].selected, ExploreStreamCount::Exact(1));
        assert_eq!(report.finds[1].not_selected, ExploreStreamCount::Exact(1));
        assert_ne!(report.finds[0].question_id, report.finds[1].question_id);
        assert!(report
            .finds
            .iter()
            .all(|find| report.identity.question_ids.contains(&find.question_id)));

        let publication = report
            .publication
            .as_ref()
            .expect("plural run must refresh its publication manifest");
        let case_support_artifacts = publication
            .artifacts
            .iter()
            .filter(|artifact| artifact.kind == "case_support_graph")
            .collect::<Vec<_>>();
        assert_eq!(case_support_artifacts.len(), 2);
        assert!(case_support_artifacts.iter().all(|artifact| {
            artifact.key.starts_with("graph:case-support:")
                && artifact.key != "graph:case-support"
                && artifact.relative_path.starts_with("graphs")
        }));
        assert_eq!(
            case_support_artifacts
                .iter()
                .map(|artifact| artifact.key.as_str())
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            2
        );
        assert!(publication
            .artifacts
            .iter()
            .all(|artifact| artifact.key != "graph:case-transitions"));
        let manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&publication.manifest_path)
                .expect("read plural publication manifest"),
        )
        .expect("parse plural publication manifest");
        assert_eq!(manifest["schema_version"].as_u64(), Some(20));
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
        let manifest_case_support = manifest["artifacts"]
            .as_array()
            .expect("manifest artifact descriptors")
            .iter()
            .filter(|artifact| artifact["kind"] == "case_support_graph")
            .collect::<Vec<_>>();
        assert_eq!(manifest_case_support.len(), 2);
        let expected_question_ids = report
            .identity
            .question_ids
            .iter()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>();
        let published_question_ids = manifest_case_support
            .iter()
            .map(|artifact| {
                artifact["question_id"]
                    .as_str()
                    .expect("case-support question identity")
            })
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(published_question_ids, expected_question_ids);
        let selected_counts = manifest_case_support
            .iter()
            .map(|artifact| {
                (
                    artifact["question_id"]
                        .as_str()
                        .expect("case-support question identity"),
                    artifact["graph_projection"]["counts"]["selected_cases"]["value"]
                        .as_str()
                        .expect("exact case-support selected count"),
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(
            selected_counts.get(report.finds[0].question_id.as_str()),
            Some(&"2")
        );
        assert_eq!(
            selected_counts.get(report.finds[1].question_id.as_str()),
            Some(&"1")
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
                ExploreStreamLayer::Choice(_) => None,
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
        assert_eq!(report.schema_version, 11);
        assert_eq!(report.lifecycle, ExploreStreamLifecycle::Complete);

        let results = report
            .layers
            .iter()
            .filter_map(|layer| match layer {
                ExploreStreamLayer::Choice(_) => None,
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
        assert_eq!(manifest["schema_version"].as_u64(), Some(20));
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
    fn warm_epoch_reuses_one_structural_derivation_across_chunk_slices() {
        let temp = TestDirectory::new();
        let run_state = temp.path().join("run-state");
        let options = || ExploreStreamEpochOptions {
            run_state: run_state.clone(),
            output_directory: None,
            outer_containment: None,
        };
        let configure = |epoch: &mut RelationalExploreEpoch| {
            epoch.resources = ExactStreamOneWorkerEnvelope::new_unmetered_for_test()
                .expect("create deterministic structural-cache resource envelope");
            epoch.driver_limits = RelationalStreamDriverLimits::default()
                .with_mechanism_artifact_chunk_bytes(NonZeroU32::MIN);
        };

        let mut epoch = prepare(CHOSEN_MECHANISM_PUBLICATION)
            .open_epoch(options())
            .expect("open warm structural-cache epoch");
        configure(&mut epoch);
        let cache = Arc::clone(&epoch.prepared.structural_artifact_cache);
        let mut structural_sequences = Vec::new();
        for _ in 0..128 {
            let report = epoch
                .run_one_batch_slice_for_test()
                .expect("advance one warm structural-cache batch");
            let pending_structural = epoch
                .durable
                .journal()
                .expect("inspect warm structural-cache journal")
                .analysis_state()
                .and_then(|analysis| analysis.pending_mechanism_artifact_claim())
                .is_some_and(|claim| {
                    matches!(
                        claim,
                        RelationalMechanismArtifactClaim::StructuralQuotient { .. }
                    )
                });
            if pending_structural && cache.successful_derivations() == 1 {
                assert_eq!(report.semantic_batches_appended, 1);
                assert_eq!(report.semantic_events_appended, 2);
                structural_sequences.push(report.checkpoint.next_sequence);
                if structural_sequences.len() == 3 {
                    break;
                }
            }
        }
        assert_eq!(
            structural_sequences.len(),
            3,
            "fixture must span the structural open and at least two chunk slices"
        );
        assert_eq!(structural_sequences[1], structural_sequences[0] + 2);
        assert_eq!(structural_sequences[2], structural_sequences[1] + 2);
        assert_eq!(cache.successful_derivations(), 1);

        let warm_next_sequence = structural_sequences[2];
        drop(epoch);

        let mut cold_epoch = prepare(CHOSEN_MECHANISM_PUBLICATION)
            .open_epoch(options())
            .expect("reopen cold structural-cache epoch");
        configure(&mut cold_epoch);
        let cold_cache = Arc::clone(&cold_epoch.prepared.structural_artifact_cache);
        assert_eq!(cold_cache.successful_derivations(), 0);
        let first_cold = cold_epoch
            .run_one_batch_slice_for_test()
            .expect("authenticate and continue the cold pending artifact");
        assert_eq!(first_cold.semantic_events_appended, 2);
        assert_eq!(first_cold.checkpoint.next_sequence, warm_next_sequence + 2);
        assert_eq!(cold_cache.successful_derivations(), 1);
        let second_cold = cold_epoch
            .run_one_batch_slice_for_test()
            .expect("reuse the cold epoch's authenticated structural artifact");
        assert_eq!(second_cold.semantic_events_appended, 2);
        assert_eq!(
            second_cold.checkpoint.next_sequence,
            first_cold.checkpoint.next_sequence + 2
        );
        assert_eq!(cold_cache.successful_derivations(), 1);
    }

    #[test]
    fn analysis_registration_decision_prefix_replays_and_resumes_plan_registration() {
        let temp = TestDirectory::new();
        let run_state = temp.path().join("run-state");
        let mut prepared = prepare(UNIFORMLY_SELECTED);
        let checked = prepared.checked.view();
        let driver = RelationalStreamDriver::from_checked_with_limits_and_classification_backends(
            &checked,
            &prepared.support_plan,
            RelationalStreamDriverLimits::default(),
            None,
            Some(&prepared.classification_evaluator),
        )
        .expect("build registration-prefix scheduler");

        {
            let mut durable =
                RelationalDurableJournal::open_or_create_with_region_replay_authority(
                    &run_state,
                    prepared.contract.clone(),
                    prepared.analysis_plan_root,
                    RelationalDurableJournalLimits::default(),
                    exact_one_region_replay_authority(&prepared),
                )
                .expect("open registration-prefix journal");
            let outcome = driver
                .step_with_base_member_limit(
                    durable
                        .journal_mut_for_event_planning()
                        .expect("borrow registration-prefix journal"),
                    &mut prepared.expression_runtime,
                    &mut prepared.mechanism_runtime,
                    NonZeroU16::new(1).unwrap(),
                )
                .expect("plan initial analysis registration");
            let RelationalStreamStepOutcome::Emitted(batch) = outcome else {
                panic!("fresh stream must emit its analysis registration");
            };
            assert_eq!(
                batch.quantum(),
                RelationalStreamQuantum::RegisterAnalysisPlan
            );
            let expected_sequence = batch.expected_sequence();
            let expected_head = batch.expected_head();
            let events = batch.into_events().into_vec();
            assert!(matches!(
                events.as_slice(),
                [
                    RelationalJournalEvent::Checkpoint(
                        RelationalCheckpointEvent::SchedulerDecisionRecorded {
                            decision: RelationalSchedulerDecision::AnalysisRegistration,
                            ..
                        }
                    ),
                    RelationalJournalEvent::Evidence(
                        RelationalEvidenceEvent::AnalysisPlanRegistered { .. }
                    )
                ]
            ));

            let wrong_plan_root = prepare(EXACT_EMPTY).analysis_plan_root;
            assert_ne!(wrong_plan_root, prepared.analysis_plan_root);
            let mut mismatched =
                RelationalDurableJournal::open_or_create_with_region_replay_authority(
                    temp.path().join("poisoned-run-state"),
                    prepared.contract.clone(),
                    wrong_plan_root,
                    RelationalDurableJournalLimits::default(),
                    exact_one_region_replay_authority(&prepared),
                )
                .expect("open mismatched registration journal");
            let error = mismatched
                .append_events(expected_sequence, expected_head, events.clone())
                .expect_err("a wrong plan after an applied decision must fail");
            assert!(matches!(
                error,
                RelationalDurableJournalError::ExpectedAnalysisPlanRootMismatch {
                    expected,
                    actual,
                } if expected == wrong_plan_root && actual == prepared.analysis_plan_root
            ));
            assert!(
                mismatched.is_poisoned(),
                "a later event validation failure must poison a partially advanced owner"
            );

            durable
                .append_events(expected_sequence, expected_head, [events[0].clone()])
                .expect("append only the crash-safe scheduling prefix");
            assert!(durable
                .journal()
                .expect("inspect scheduling prefix")
                .analysis_state()
                .is_none());
            durable
                .flush_for_pause()
                .expect("flush scheduling-only prefix");
        }

        let mut durable = RelationalDurableJournal::open_or_create_with_region_replay_authority(
            &run_state,
            prepared.contract.clone(),
            prepared.analysis_plan_root,
            RelationalDurableJournalLimits::default(),
            exact_one_region_replay_authority(&prepared),
        )
        .expect("replay scheduling-only prefix");
        assert_eq!(
            durable
                .journal()
                .expect("inspect replayed scheduling prefix")
                .next_sequence(),
            1
        );
        let outcome = driver
            .step_with_base_member_limit(
                durable
                    .journal_mut_for_event_planning()
                    .expect("borrow replayed registration-prefix journal"),
                &mut prepared.expression_runtime,
                &mut prepared.mechanism_runtime,
                NonZeroU16::new(1).unwrap(),
            )
            .expect("resume analysis registration");
        let RelationalStreamStepOutcome::Emitted(batch) = outcome else {
            panic!("registration-prefix resume must re-offer the uncommitted plan");
        };
        durable
            .append_events(
                batch.expected_sequence(),
                batch.expected_head(),
                batch.into_events(),
            )
            .expect("append resumed analysis registration");
        let journal = durable.journal().expect("inspect resumed registration");
        assert_eq!(journal.next_sequence(), 3);
        assert_eq!(
            journal
                .scheduler_view()
                .expect("inspect registered analysis plan")
                .analysis_plan_root(),
            Some(prepared.analysis_plan_root)
        );
    }

    #[test]
    fn choice_candidate_prefix_resumes_before_question_seal_and_mechanisms_consume_closed_choice() {
        let temp = TestDirectory::new();
        let run_state = temp.path().join("run-state");
        let output = temp.path().join("output");

        let (choice_id, question_id, paused_sequence, paused_head, paused_frontier) = {
            let mut prepared = prepare(CHOSEN_MECHANISM_PUBLICATION);
            let checked = prepared.checked.view();
            let question_id = checked.question_ids()[0];
            let choice_id = checked
                .analysis_nodes()
                .find_map(|(_, identity)| match identity {
                    CheckedExploreAnalysisIdentity::View {
                        choice_id: Some(choice_id),
                        ..
                    } => Some(*choice_id),
                    _ => None,
                })
                .expect("fixture choice identity");
            let limits = RelationalStreamDriverLimits::new(
                NonZeroU16::new(1).unwrap(),
                NonZeroU16::new(1).unwrap(),
                NonZeroU16::new(1).unwrap(),
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
            let mut paused_prefix = None;
            for _ in 0..512 {
                let outcome = driver
                    .step_with_base_member_limit(
                        durable
                            .journal_mut_for_event_planning()
                            .expect("borrow chosen-target planning journal"),
                        &mut prepared.expression_runtime,
                        &mut prepared.mechanism_runtime,
                        NonZeroU16::new(1).unwrap(),
                    )
                    .expect("advance choice-candidate prefix");
                let RelationalStreamStepOutcome::Emitted(batch) = outcome else {
                    panic!("choice stream quiesced before candidate admission");
                };
                let quantum = batch.quantum();
                let candidate_batch = matches!(
                    quantum,
                    RelationalStreamQuantum::Result(
                        super::super::relational_result_step_driver::RelationalResultStepQuantum::EvaluateChoiceCandidates {
                            choice_id: actual,
                            row_count,
                            ..
                        }
                    ) if actual == choice_id && row_count.get() == 1
                );
                let expected_sequence = batch.expected_sequence();
                let expected_head = batch.expected_head();
                let events = batch.into_events().into_vec();
                durable
                    .append_events(expected_sequence, expected_head, events)
                    .expect("append choice-candidate prefix batch");
                if candidate_batch {
                    let prefix = {
                        let journal = durable.journal().expect("inspect candidate prefix");
                        let analysis = journal.analysis_state().expect("analysis state");
                        assert!(
                            analysis.selected_question(question_id).is_none(),
                            "candidate evidence must be resumable before exact question closure"
                        );
                        let choice = analysis
                            .open_catalog()
                            .expect("open analysis catalog")
                            .choice_relation(choice_id)
                            .expect("choice relation");
                        assert_eq!(
                            choice.status(),
                            super::super::choice_relation::ChoiceRelationStatus::InputOpen
                        );
                        assert_eq!(
                            choice.counts().candidates(),
                            super::super::choice_relation::ChoiceCount::LowerBound(1)
                        );
                        assert_eq!(
                            choice.counts().members(),
                            super::super::choice_relation::ChoiceCount::Provisional(0)
                        );
                        assert!(choice.content_root().is_none());
                        (
                            journal.next_sequence(),
                            journal.head(),
                            choice.frontier_root(),
                        )
                    };
                    durable
                        .flush_for_pause()
                        .expect("flush choice-candidate crash prefix");
                    paused_prefix = Some(prefix);
                    break;
                }
            }
            let (paused_sequence, paused_head, paused_frontier) =
                paused_prefix.expect("fixture did not stop after its first choice candidate");
            (
                choice_id,
                question_id,
                paused_sequence,
                paused_head,
                paused_frontier,
            )
        };

        {
            let prepared = prepare(CHOSEN_MECHANISM_PUBLICATION);
            let durable = RelationalDurableJournal::open_or_create_with_region_replay_authority(
                &run_state,
                prepared.contract.clone(),
                prepared.analysis_plan_root,
                RelationalDurableJournalLimits::default(),
                exact_one_region_replay_authority(&prepared),
            )
            .expect("reopen choice-candidate journal");
            let journal = durable
                .journal()
                .expect("inspect reopened candidate prefix");
            assert_eq!(journal.next_sequence(), paused_sequence);
            assert_eq!(journal.head(), paused_head);
            let analysis = journal.analysis_state().expect("reopened analysis state");
            assert!(analysis.selected_question(question_id).is_none());
            let choice = analysis
                .open_catalog()
                .expect("reopened analysis catalog")
                .choice_relation(choice_id)
                .expect("reopened choice relation");
            assert_eq!(choice.frontier_root(), paused_frontier);
            assert_eq!(
                choice.counts().candidates(),
                super::super::choice_relation::ChoiceCount::LowerBound(1)
            );
            assert_eq!(
                choice.counts().members(),
                super::super::choice_relation::ChoiceCount::Provisional(0)
            );
            assert!(choice.content_root().is_none());
        }

        let mut epoch = prepare(CHOSEN_MECHANISM_PUBLICATION)
            .open_epoch(ExploreStreamEpochOptions {
                run_state,
                output_directory: Some(output.clone()),
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

        let winner = report
            .layers
            .iter()
            .find_map(|layer| match layer {
                ExploreStreamLayer::Result(result) if result.name == "winner" => Some(result),
                _ => None,
            })
            .expect("choice materializing result layer");
        let winner_choice_id = winner
            .choice_id
            .as_ref()
            .expect("choice materializer exposes ChoiceId");
        let choice = report
            .layers
            .iter()
            .find_map(|layer| match layer {
                ExploreStreamLayer::Choice(choice) => Some(choice),
                _ => None,
            })
            .expect("independent choice layer");
        assert_eq!(choice.choice_id, *winner_choice_id);
        assert_eq!(choice.status, ExploreStreamLayerStatus::ChoiceClosed);
        assert_eq!(choice.candidates, ExploreStreamCount::Exact(4));
        assert_eq!(choice.members, ExploreStreamCount::Exact(2));
        assert!(choice.content_root.is_some());

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
            ExploreStreamMechanismTarget::Choice {
                name,
                question_id,
                choice_id,
            } if name == "winner"
                && question_id == &report.finds[0].question_id
                && choice_id == winner_choice_id
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

        let publication = report
            .publication
            .as_ref()
            .expect("choice run publication summary");
        let manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&publication.manifest_path).expect("read choice manifest"),
        )
        .expect("parse choice manifest");
        assert_eq!(manifest["schema_version"].as_u64(), Some(20));
        assert_eq!(manifest["answer"]["schema_version"].as_u64(), Some(2));
        let choices = manifest["answer"]["choices"]
            .as_array()
            .expect("choice answer index");
        assert_eq!(choices.len(), 1);
        assert_eq!(
            choices[0]["choice_id"].as_str(),
            Some(winner_choice_id.as_str())
        );
        assert_eq!(
            choices[0]["result_artifact_keys"]
                .as_array()
                .expect("choice display consumers")
                .len(),
            1
        );
        assert_eq!(
            choices[0]["mechanism_request_ids"][0].as_str(),
            Some(mechanism.request_id.as_str())
        );
        let published_target = &manifest["answer"]["mechanisms"][0]["target"];
        assert_eq!(
            published_target["choice_id"].as_str(),
            Some(winner_choice_id.as_str())
        );
        assert!(published_target.get("materializing_view_id").is_none());
    }

    #[test]
    fn excluded_candidates_never_reach_shared_choice_display_select() {
        let temp = TestDirectory::new();
        let run_state = temp.path().join("run-state");
        let output = temp.path().join("output");
        let mut epoch = prepare(SHARED_CHOICE_DISPLAY_FAILURE)
            .open_epoch(ExploreStreamEpochOptions {
                run_state,
                output_directory: Some(output.clone()),
                outer_containment: None,
            })
            .expect("open shared-choice display epoch");
        epoch.resources = ExactStreamOneWorkerEnvelope::new_unmetered_for_test()
            .expect("create deterministic shared-choice resource envelope");
        let report = epoch
            .run_slice(None)
            .expect("excluded candidate must not execute either display SELECT");
        assert_eq!(report.lifecycle, ExploreStreamLifecycle::Complete);
        assert!(report.analysis_closed);

        let choice = report
            .layers
            .iter()
            .find_map(|layer| match layer {
                ExploreStreamLayer::Choice(choice) => Some(choice),
                _ => None,
            })
            .expect("shared choice layer");
        assert_eq!(choice.status, ExploreStreamLayerStatus::ChoiceClosed);
        assert_eq!(choice.candidates, ExploreStreamCount::Exact(4));
        assert_eq!(choice.members, ExploreStreamCount::Exact(2));

        for name in ["good_display", "bad_display"] {
            let result = report
                .layers
                .iter()
                .find_map(|layer| match layer {
                    ExploreStreamLayer::Result(result) if result.name == name => Some(result),
                    _ => None,
                })
                .expect("shared choice display layer");
            assert_eq!(result.status, ExploreStreamLayerStatus::ResultPublished);
            assert_eq!(result.grain, ExploreStreamResultGrain::EachCase);
            assert_eq!(result.choice_id.as_deref(), Some(choice.choice_id.as_str()));
            assert_eq!(result.input_rows, ExploreStreamCount::Exact(2));
            assert_eq!(result.output_rows, ExploreStreamCount::Exact(2));
            assert_eq!(result.projection_records, ExploreStreamCount::Exact(2));
        }

        let mechanism = report
            .layers
            .iter()
            .find_map(|layer| match layer {
                ExploreStreamLayer::Mechanisms(mechanism) => Some(mechanism),
                _ => None,
            })
            .expect("shared choice mechanism layer");
        assert_eq!(mechanism.status, ExploreStreamLayerStatus::MechanismClosed);
        assert_eq!(mechanism.target_cases, ExploreStreamCount::Exact(2));

        let publication = report
            .publication
            .as_ref()
            .expect("shared choice publication summary");
        let manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&publication.manifest_path).expect("read shared choice manifest"),
        )
        .expect("parse shared choice manifest");
        let views = manifest["answer"]["result_views"]
            .as_array()
            .expect("shared choice result views");
        for (name, expected_shown) in [
            ("good_display", [2_i64, 3_i64]),
            ("bad_display", [33_i64, 50_i64]),
        ] {
            let descriptor = views
                .iter()
                .find(|view| view["name"].as_str() == Some(name))
                .expect("shared choice view descriptor");
            assert_eq!(descriptor["grain"].as_str(), Some("each_case"));
            assert_eq!(
                descriptor["counts"]["input_rows"]["value"].as_str(),
                Some("2")
            );
            let path = output.join(
                descriptor["artifact"]["path"]
                    .as_str()
                    .expect("shared choice artifact path"),
            );
            let rows = fs::read_to_string(path)
                .expect("read shared choice display")
                .lines()
                .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("parse row"))
                .filter(|line| line["record"]["kind"] == "result_row")
                .collect::<Vec<_>>();
            assert_eq!(rows.len(), 2);
            let shown = rows
                .iter()
                .map(|row| {
                    row["record"]["values"]["shown"]
                        .as_i64()
                        .expect("integer shown value")
                })
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(shown, expected_shown.into_iter().collect());
        }
    }

    #[test]
    fn choice_and_mechanisms_remain_closed_when_member_display_select_fails() {
        let source = SHARED_CHOICE_DISPLAY_FAILURE.replacen("100 / value", "100 / (value - 2)", 1);
        let temp = TestDirectory::new();
        let run_state = temp.path().join("run-state");
        let prepared = prepare(&source);
        let checked = prepared.checked.view();
        let choice_id = checked
            .analysis_nodes()
            .find_map(|(_, identity)| match identity {
                CheckedExploreAnalysisIdentity::View {
                    choice_id: Some(choice_id),
                    ..
                } => Some(*choice_id),
                _ => None,
            })
            .expect("member-failure choice identity");
        let request_id = checked
            .analysis_nodes()
            .find_map(|(_, identity)| match identity {
                CheckedExploreAnalysisIdentity::Mechanisms { request_id, .. } => Some(*request_id),
                _ => None,
            })
            .expect("member-failure mechanism identity");
        let mut epoch = prepared
            .open_epoch(ExploreStreamEpochOptions {
                run_state,
                output_directory: None,
                outer_containment: None,
            })
            .expect("open member-failure choice epoch");
        epoch.resources = ExactStreamOneWorkerEnvelope::new_unmetered_for_test()
            .expect("create deterministic member-failure resource envelope");
        let error = epoch
            .run_slice(None)
            .expect_err("SELECT must fail on the chosen before=2 member")
            .to_string();
        assert!(
            error.contains("division by zero") || error.contains("choice display selected field"),
            "unexpected member display failure: {error}"
        );

        let journal = epoch
            .durable
            .journal()
            .expect("inspect member-failure choice journal");
        let analysis = journal
            .analysis_state()
            .expect("member-failure analysis state");
        assert!(!analysis.is_closed());
        let catalog = analysis
            .open_catalog()
            .expect("open member-failure catalog");
        assert_eq!(
            catalog.layer_status(RelationalAnalysisLayerId::Choice(choice_id)),
            Some(RelationalAnalysisLayerStatus::ChoiceClosed)
        );
        assert_eq!(
            catalog.layer_status(RelationalAnalysisLayerId::Mechanisms(request_id)),
            Some(RelationalAnalysisLayerStatus::MechanismClosed)
        );
        let content_root = catalog
            .choice_content_root(choice_id)
            .expect("choice root lookup")
            .expect("closed choice content root");
        assert_eq!(
            catalog
                .mechanism_incidence(request_id)
                .expect("closed choice mechanism")
                .target_seal()
                .expect("choice mechanism target seal")
                .upstream(),
            super::super::mechanism_incidence::MechanismTargetSealUpstream::Choice {
                choice_id,
                content_root,
            }
        );
    }

    #[test]
    fn choice_mechanism_target_rejects_tampered_member_provenance() {
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
                    RelationalMechanismStepQuantum::AdmitChoiceTargetCases {
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

            assert_eq!(events.len(), 3);
            assert!(matches!(
                &events[0],
                RelationalJournalEvent::Checkpoint(
                    RelationalCheckpointEvent::SchedulerDecisionRecorded {
                        decision: RelationalSchedulerDecision::ReadyMechanism,
                        ..
                    }
                )
            ));
            let chosen_claim = |event: &RelationalJournalEvent| match event {
                RelationalJournalEvent::Evidence(RelationalEvidenceEvent::Analysis(
                    RelationalAnalysisEvidenceEvent::MechanismChoiceTargetCaseAccepted {
                        request_id,
                        choice_id,
                        member_ordinal,
                        case_id,
                    },
                )) => (*request_id, *choice_id, *member_ordinal, *case_id),
                _ => panic!("chosen-target batch contained a non-provenance event"),
            };
            let (request_id, choice_id, member_ordinal, first_case_id) = chosen_claim(&events[1]);
            let (second_request_id, second_choice_id, second_ordinal, second_case_id) =
                chosen_claim(&events[2]);
            assert_eq!(second_request_id, request_id);
            assert_eq!(second_choice_id, choice_id);
            assert_ne!(second_ordinal, member_ordinal);
            assert_ne!(second_case_id, first_case_id);

            let tampered = RelationalJournalEvent::analysis(
                RelationalAnalysisEvidenceEvent::mechanism_choice_target_case_accepted(
                    request_id,
                    choice_id,
                    member_ordinal,
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
                        RelationalAnalysisCatalogError::ChoiceMemberMismatch {
                            choice_id: rejected_choice_id,
                            member_ordinal: ordinal,
                        }
                    )
                )) if rejected_choice_id == choice_id && ordinal == member_ordinal
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
            fresh_plan.choice_registrations().to_vec(),
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
        let paused_adds;

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
            let mut selected_candidate_materialized = false;

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
                if let RelationalStreamQuantum::Base(RelationalStepQuantum::ClassifiedSweep(
                    classified,
                )) = quantum
                {
                    preceding_base_classifications += 1;
                    assert_eq!(classified.chunk_ordinal(), 1);
                    assert_eq!(
                        classified.schedule_reason(),
                        RelationalCandidateScheduleReason::CheckedGuardBoundary
                    );
                }
                if matches!(
                    quantum,
                    RelationalStreamQuantum::Base(
                        RelationalStepQuantum::SelectedRunMaterialization(_)
                    )
                ) {
                    selected_candidate_materialized = true;
                }
                durable
                    .append_events(
                        batch.expected_sequence(),
                        batch.expected_head(),
                        batch.into_events(),
                    )
                    .expect("append hybrid prefix batch");
                if selected_candidate_materialized {
                    break;
                }
            }
            assert_eq!(preceding_base_classifications, 1);
            assert!(selected_candidate_materialized);
            paused_adds = {
                let view = durable
                    .journal()
                    .expect("inspect durable prefix")
                    .scheduler_view()
                    .expect("inspect hybrid prefix");
                assert!(view.classified_support_fragments().unwrap().is_empty());
                assert_eq!(view.accepted_classified_fragment_count(), 1);
                assert!(view.classified_support_fragment_at(0).unwrap().is_none());
                assert!(matches!(
                    view.classified_support_fragment_at(1).unwrap(),
                    Some(RelationalClassifiedSupportFragment::Concrete(_))
                ));
                assert_eq!(
                    view.selected_run_materializations(question_id)
                        .unwrap()
                        .count(),
                    1
                );
                let partition = view
                    .verified_case_chunk_partition()
                    .unwrap()
                    .expect("hybrid canonical partition before pause");
                let projection = derive_relational_case_support_projection(
                    question_id,
                    partition,
                    view,
                    None,
                    None,
                )
                .expect("derive sparse public hybrid prefix");
                let metadata = projection.metadata();
                assert_eq!(
                    metadata.classified_case_count,
                    RelationalCaseSupportCount::LowerBound(44)
                );
                assert_eq!(
                    metadata.selected_case_count,
                    RelationalCaseSupportCount::LowerBound(20)
                );
                assert_eq!(
                    metadata.materialized_selected_case_count,
                    RelationalCaseSupportCount::LowerBound(20)
                );
                assert!(matches!(
                    metadata.frontier,
                    RelationalCaseSupportProjectionFrontier::Open(
                        RelationalCaseSupportOpenReason::AwaitingClassifiedFragments {
                            missing_chunk_count: 1,
                            first_missing_chunk_ordinal: 0,
                        }
                    )
                ));
                let adds = case_support_add_sequence(&projection);
                assert_eq!(
                    projection.available_source_record_count(),
                    u128::try_from(adds.len()).expect("bounded sparse add count")
                );
                assert!(matches!(
                    adds.first(),
                    Some((RelationalCaseSupportRecordKey::Root, _))
                ));
                assert!(matches!(
                    adds.last(),
                    Some((
                        RelationalCaseSupportRecordKey::SelectedMaterialization {
                            chunk_ordinal: 1,
                            ..
                        },
                        _
                    ))
                ));
                adds
            };
            paused_checkpoint = durable
                .flush_for_pause()
                .expect("flush sparse hybrid prefix");
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
        {
            let reopened = durable.journal().expect("inspect reopened hybrid journal");
            assert_eq!(reopened.next_sequence(), paused_checkpoint.next_sequence());
            assert_eq!(reopened.head(), paused_checkpoint.head());
            let reopened_view = reopened
                .scheduler_view()
                .expect("inspect replayed hybrid prefix");
            assert!(reopened_view
                .classified_support_fragments()
                .unwrap()
                .is_empty());
            assert_eq!(reopened_view.accepted_classified_fragment_count(), 1);
            assert!(reopened_view
                .classified_support_fragment_at(0)
                .unwrap()
                .is_none());
            let reopened_partition = reopened_view
                .verified_case_chunk_partition()
                .unwrap()
                .expect("replayed hybrid canonical partition");
            let reopened_projection = derive_relational_case_support_projection(
                question_id,
                reopened_partition,
                reopened_view,
                None,
                None,
            )
            .expect("rederive sparse hybrid prefix after reopen");
            assert_eq!(
                case_support_add_sequence(&reopened_projection),
                paused_adds,
                "replay must preserve the byte-semantic add sequence"
            );
        }

        let mut accepted_late_chunk_zero = false;
        for _ in 0..64 {
            let outcome = driver
                .step_with_base_member_limit(
                    durable
                        .journal_mut_for_event_planning()
                        .expect("borrow reopened planning journal"),
                    &mut prepared.expression_runtime,
                    &mut prepared.mechanism_runtime,
                    NonZeroU16::new(256).unwrap(),
                )
                .expect("advance hybrid stream to late chunk zero");
            let RelationalStreamStepOutcome::Emitted(batch) = outcome else {
                panic!("hybrid stream quiesced before accepting late chunk zero");
            };
            let accepts_chunk_zero = matches!(
                batch.quantum(),
                RelationalStreamQuantum::Base(RelationalStepQuantum::CertifiedRegion {
                    chunk_ordinal: 0,
                    ..
                })
            );
            durable
                .append_events(
                    batch.expected_sequence(),
                    batch.expected_head(),
                    batch.into_events(),
                )
                .expect("append hybrid batch through late chunk zero");
            if accepts_chunk_zero {
                accepted_late_chunk_zero = true;
                break;
            }
        }
        assert!(accepted_late_chunk_zero);

        let adds_after_chunk_zero = {
            let view = durable
                .journal()
                .expect("inspect late-chunk hybrid journal")
                .scheduler_view()
                .expect("inspect late-chunk hybrid view");
            assert!(view.classified_support_fragments().unwrap().is_empty());
            assert_eq!(view.accepted_classified_fragment_count(), 2);
            assert!(matches!(
                view.classified_support_fragment_at(0).unwrap(),
                Some(RelationalClassifiedSupportFragment::CertifiedZeroSelected(
                    _
                ))
            ));
            let partition = view
                .verified_case_chunk_partition()
                .unwrap()
                .expect("late-chunk hybrid canonical partition");
            let projection =
                derive_relational_case_support_projection(question_id, partition, view, None, None)
                    .expect("derive hybrid projection after late chunk zero");
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
                RelationalCaseSupportProjectionFrontier::Open(
                    RelationalCaseSupportOpenReason::AwaitingClosureAuthority
                )
            ));
            let adds = case_support_add_sequence(&projection);
            assert!(adds.starts_with(&paused_adds));
            let appended = &adds[paused_adds.len()..];
            assert_eq!(appended.len(), 2);
            assert!(matches!(
                appended[0].0,
                RelationalCaseSupportRecordKey::Chunk { chunk_ordinal: 0 }
            ));
            assert!(matches!(
                appended[1].0,
                RelationalCaseSupportRecordKey::Region {
                    chunk_ordinal: 0,
                    run_ordinal: 0,
                }
            ));
            adds
        };

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
        let fragments = view.classified_support_fragments().unwrap();
        assert_eq!(fragments.len(), 2);
        assert!(matches!(
            fragments[0],
            RelationalClassifiedSupportFragment::CertifiedZeroSelected(_)
        ));
        assert!(matches!(
            fragments[1],
            RelationalClassifiedSupportFragment::Concrete(_)
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
                .map(|fragment| {
                    fragment
                        .admitted_selected_count(question_id)
                        .expect("hybrid fragment contains the query question")
                })
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
            .selected_run_materializations_cover_classified_slots(question_id)
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
            .verified_case_chunk_partition()
            .unwrap()
            .expect("hybrid canonical partition");
        let projection = derive_relational_case_support_projection(
            question_id,
            partition,
            view,
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
                RelationalCaseSupportProjectionRecord::Add {
                    row:
                        RelationalCaseSupportRow::Chunk {
                            classification_authority,
                            ..
                        },
                    ..
                } => Some(classification_authority.kind()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            chunk_authorities,
            vec!["concrete_sweep", "regional_certificate"]
        );
        assert!(records.iter().any(|record| matches!(
            record,
            RelationalCaseSupportProjectionRecord::Add {
                row: RelationalCaseSupportRow::Region {
                    exact_case_count: 256,
                    correlated_starter_region_id: Some(_),
                    ..
                },
                ..
            }
        )));
        assert_eq!(
            records
                .iter()
                .filter(|record| matches!(
                    record,
                    RelationalCaseSupportProjectionRecord::Add {
                        row: RelationalCaseSupportRow::SelectedMaterialization { .. },
                        ..
                    }
                ))
                .count(),
            1
        );
        assert_eq!(
            records
                .iter()
                .filter(|record| matches!(record, RelationalCaseSupportProjectionRecord::Seal(_)))
                .count(),
            1
        );
        assert!(matches!(
            records.last(),
            Some(RelationalCaseSupportProjectionRecord::Seal(closure))
                if closure.exact_logical_case_count == 300
                    && closure.exact_selected_case_count == 20
        ));

        let final_adds = case_support_add_sequence(&projection);
        assert_eq!(final_adds, adds_after_chunk_zero);
        let mut canonical_adds = final_adds;
        canonical_adds.sort_unstable_by_key(|(key, _)| *key);
        let closure = match metadata.frontier {
            RelationalCaseSupportProjectionFrontier::Exact(closure) => closure,
            RelationalCaseSupportProjectionFrontier::Open(reason) => {
                panic!("completed hybrid projection remained open: {reason:?}")
            }
        };
        assert_eq!(
            closure.active_record_count,
            u128::try_from(canonical_adds.len()).expect("bounded active add count")
        );
        let expected_active_set_root = relational_case_support_active_set_root(
            projection.projection_id(),
            closure.active_record_count,
            canonical_adds,
        )
        .expect("fold canonical hybrid active set");
        assert_eq!(closure.active_set_root, expected_active_set_root);
    }

    #[test]
    fn classified_target_compaction_does_not_reseed_accepted_sparse_work() {
        let mut prepared = prepare(HYBRID);
        let checked = prepared.checked.view();
        let analysis_plan =
            RelationalAnalysisPlan::from_checked(&checked).expect("plan compaction fixture");
        let mut journal = RelationalJournal::new_with_region_replay_authority(
            prepared.contract.clone(),
            exact_one_region_replay_authority(&prepared),
        );
        journal
            .append(RelationalJournalEvent::analysis_plan_registered(
                analysis_plan,
            ))
            .expect("register compaction fixture analysis plan");
        let member_limit = NonZeroU16::new(256).unwrap();
        let driver =
            RelationalStepDriver::from_checked_with_operational_limits_and_classification_backends(
                &checked,
                &prepared.support_plan,
                member_limit,
                NonZeroU32::MIN,
                NonZeroU32::MIN,
                None,
                Some(&prepared.classification_evaluator),
            )
            .expect("build low-trigger classified scheduler");

        let mut seeded = [false; 2];
        let mut compactions = 0usize;
        let mut closed = false;
        for _ in 0..128 {
            match driver
                .step_with_max_members_per_quantum(
                    &journal,
                    &mut prepared.expression_runtime,
                    member_limit,
                )
                .expect("advance low-trigger classified scheduler")
            {
                RelationalStepOutcome::Emitted(batch) => {
                    let partition_just_accepted = matches!(
                        batch.quantum(),
                        RelationalStepQuantum::Support(
                            RelationalSupportStepQuantum::AcceptCaseChunkPartition { .. }
                        )
                    );
                    match batch.quantum() {
                        RelationalStepQuantum::SeedClassifiedTargetWork {
                            chunk_ordinal, ..
                        } => {
                            let index = usize::try_from(chunk_ordinal)
                                .expect("fixture chunk ordinal fits usize");
                            assert!(index < seeded.len(), "fixture has exactly two chunks");
                            assert!(
                                !std::mem::replace(&mut seeded[index], true),
                                "an accepted classified target must never be reseeded after compaction"
                            );
                        }
                        RelationalStepQuantum::CompactWorkFrontier { removed_nodes } => {
                            assert_eq!(removed_nodes, 1);
                            compactions += 1;
                        }
                        _ => {}
                    }
                    append_base_batch(&mut journal, batch);
                    if partition_just_accepted {
                        let before = (journal.next_sequence(), journal.head());
                        for seal in [
                            SupportJournalEvent::ObligationFrontierSealed,
                            SupportJournalEvent::CatalogSealed,
                        ] {
                            assert!(matches!(
                                journal.append(RelationalJournalEvent::support(seal)),
                                Err(RelationalJournalError::ClassifiedSupportCoveragePending)
                            ));
                            assert_eq!((journal.next_sequence(), journal.head()), before);
                        }
                    }
                    let retained_work_nodes = journal.scheduler_view().unwrap().work_node_count();
                    assert!(
                        retained_work_nodes <= 4,
                        "the fixed four-node support frontier or one live target pair bounds retained work; retained {retained_work_nodes} nodes"
                    );
                }
                RelationalStepOutcome::Quiescent(
                    RelationalConcreteQuiescence::SupportEvidenceClosed { .. },
                ) => {
                    closed = true;
                    break;
                }
                RelationalStepOutcome::Quiescent(other) => {
                    panic!("unexpected classified quiescence: {other:?}");
                }
            }
        }

        assert!(closed, "low-trigger classified fixture must close");
        assert_eq!(seeded, [true, true]);
        assert!(compactions >= 4, "both two-node target pairs were peeled");
        assert_eq!(journal.scheduler_view().unwrap().work_node_count(), 0);
    }

    #[test]
    fn lifted_callable_candidate_endpoint_and_residual_match_canonical_exhaustive_oracle() {
        assert_three_chunk_candidate_matches_canonical_exhaustive_oracle(
            THREE_CHUNK_CANDIDATE_RESIDUAL,
            RelationalCandidateScheduleReason::LiftedCandidate,
        );
    }

    #[test]
    fn checked_source_event_leads_live_order_and_preserves_exhaustive_semantics() {
        let prepared = prepare(THREE_CHUNK_SOURCE_EVENT_RESIDUAL);
        let checked = prepared.checked.view();
        let inventory =
            RelationalProofStrategyInventory::from_checked(&checked, &prepared.support_plan)
                .expect("derive source-event fixture strategy inventory");
        let [axis] = inventory.axes() else {
            panic!("source-event fixture must expose one exact integer axis")
        };
        assert!(
            inventory.guard_atoms().is_empty(),
            "the direct query AST must not claim the rule condition as its own guard"
        );
        assert!(
            derive_relational_lifted_affine_guard_atoms(
                &checked,
                checked.classification_program().as_ref(),
                axis,
            )
            .is_empty(),
            "the frozen pure-callable graph must fail closed at a rule dispatch"
        );
        let source_event_atoms = derive_relational_source_event_guard_atoms(&checked, axis);
        let [source_event_atom] = source_event_atoms.as_ref() else {
            panic!("the checked Clause+Exception inventory must yield one scheduler atom")
        };
        let checked_source_events = checked.source_event_inventory();
        let [checked_source_event] = checked_source_events.events() else {
            panic!("the checked Clause+Exception inventory must retain one source event")
        };
        assert!(matches!(
            source_event_atom.origin(),
            RelationalGuardOrigin::SourceEvent {
                inventory_root,
                source_event_id,
                occurrence_id,
            } if *inventory_root == checked_source_events.inventory_root()
                && *source_event_id == checked_source_event.source_event_id.bytes()
                && *occurrence_id == checked_source_event.occurrence_id.bytes()
        ));

        assert_three_chunk_candidate_matches_canonical_exhaustive_oracle(
            THREE_CHUNK_SOURCE_EVENT_RESIDUAL,
            RelationalCandidateScheduleReason::SourceEvent,
        );
    }

    fn assert_three_chunk_candidate_matches_canonical_exhaustive_oracle(
        source_text: &str,
        expected_first_reason: RelationalCandidateScheduleReason,
    ) {
        let oracle = canonical_exhaustive_three_chunk_oracle(source_text);
        assert_eq!(
            (
                oracle.candidates,
                oracle.rejected,
                oracle.admitted,
                oracle.admitted_not_selected,
                oracle.admitted_selected,
            ),
            (700, 0, 700, 680, 20),
            "the independent exhaustive oracle anchors the fixture semantics"
        );

        let candidate = execute_three_chunk_schedule(source_text, false);
        let canonical = execute_three_chunk_schedule(source_text, true);
        assert_eq!(
            candidate
                .scheduled
                .iter()
                .map(|(chunk_ordinal, reason, _)| (*chunk_ordinal, *reason))
                .collect::<Vec<_>>(),
            vec![
                (2, expected_first_reason),
                (0, RelationalCandidateScheduleReason::LowerRangeEndpoint),
                (1, RelationalCandidateScheduleReason::ResidualFallback),
            ],
            "live scheduling must exercise candidate, endpoint, then residual work"
        );
        assert_eq!(
            candidate
                .scheduled
                .iter()
                .map(|(_, _, root)| *root)
                .collect::<BTreeSet<_>>()
                .len(),
            candidate.scheduled.len(),
            "candidate, endpoint, and residual targets need distinct authenticated roots"
        );
        assert_eq!(
            canonical
                .scheduled
                .iter()
                .map(|(chunk_ordinal, reason, _)| (*chunk_ordinal, *reason))
                .collect::<Vec<_>>(),
            vec![
                (0, RelationalCandidateScheduleReason::ResidualFallback),
                (1, RelationalCandidateScheduleReason::ResidualFallback),
                (2, RelationalCandidateScheduleReason::ResidualFallback),
            ],
            "the test control must execute the canonical chunk order"
        );

        let oracle_counts = (
            oracle.candidates,
            oracle.rejected,
            oracle.admitted,
            oracle.admitted_not_selected,
            oracle.admitted_selected,
        );
        let oracle_commitment =
            MechanismTargetCaseSetCommitment::from_cases(oracle.selected_case_ids.iter().copied());
        for run in [&candidate, &canonical] {
            assert_eq!(run.chunk_count, 3);
            assert_eq!(run.accepted_fragment_count, 3);
            assert_eq!(run.counts, oracle_counts);
            assert_eq!(run.selected_case_ids, oracle.selected_case_ids);
            assert_eq!(run.target_commitment, oracle_commitment);
        }
        assert_eq!(
            candidate.snapshot.core_evidence_root(),
            canonical.snapshot.core_evidence_root(),
            "candidate order must not change the closed core evidence root"
        );
        assert_eq!(
            candidate.snapshot.exploration_evidence_root(),
            canonical.snapshot.exploration_evidence_root(),
            "candidate order must not change the whole-exploration evidence root"
        );
        assert_eq!(
            candidate.snapshot.analysis_terminal_root(),
            canonical.snapshot.analysis_terminal_root(),
            "candidate order must not change the closed analysis catalog root"
        );
        assert_eq!(
            candidate.snapshot.analysis_closure_set_root(),
            canonical.snapshot.analysis_closure_set_root(),
            "candidate order must not change the closed analysis set root"
        );
        assert_eq!(
            candidate.target_commitment, canonical.target_commitment,
            "candidate order must not change the selected mechanism-target root"
        );
        assert_eq!(
            candidate.mechanism_closure_roots, canonical.mechanism_closure_roots,
            "candidate order must not change incidence, structural, or support closure roots"
        );
    }

    #[test]
    fn income_distance_unit_edges_match_independent_exhaustion_in_both_directions() {
        let source = r#"
# Starter(income: Int, distance: Int)
# Step(income: Int, distance: Int)
> net(s: Starter, c: Step) -> Int {
    s.income +
    (if s.income < 13 { 10 } else { 0 }) +
    (if s.distance < 17 { 3 } else { 0 })
}
> advance(s: Starter, c: Step) -> Starter { Starter(s.income + c.income, s.distance + c.distance) }
? explore income_distance_edges {
    from {
        vary income in range(0, 20)
        vary distance in range(0, 30)
        vary direction in range(0, 2)
        let before = Starter(income, distance)
        let context = Step(1 - direction, direction)
    }
    transition after = advance(before, context)
    where after after.income <= 19 && after.distance <= 29
    find cliffs = violations of net(after, context) >= net(before, context)
    mechanisms paths from find cliffs using net
}
"#;
        assert_product_stream_matches_oracle(source, &[20, 30, 2], (1200, 50, 1150, 1100, 50));
    }

    #[test]
    fn affine_income_distance_closure_matches_exhaustion_without_point_classification() {
        let source = r#"
# Starter(income: Int, distance: Int)
# Step(income: Int, distance: Int)
> net(s: Starter, c: Step) -> Int { s.income * 3 + s.distance * 2 }
> advance(s: Starter, c: Step) -> Starter { Starter(s.income + c.income, s.distance + c.distance) }
? explore affine_income_distance_edges {
    from {
        vary income in range(100, 120)
        vary distance in range(30, 50)
        vary direction in range(0, 2)
        let before = Starter(income, distance)
        let context = Step(1 - direction, direction)
    }


    transition after = advance(before, context)
    find cliffs = violations of net(after, context) >= net(before, context)
    mechanisms paths from find cliffs using net
}
"#;
        let run = assert_product_stream_matches_oracle(source, &[20, 20, 2], (800, 0, 800, 800, 0));
        assert_eq!(
            run.certified_fragment_count, 4,
            "all four product chunks use verified closure"
        );
    }

    #[test]
    fn isolated_unit_cliffs_and_integer_rounding_remain_exact_product_residuals() {
        let template = r#"
# Starter(income: Int, distance: Int)
# Step(income: Int, distance: Int)
> net(s: Starter, c: Step) -> Int { METRIC }
> advance(s: Starter, c: Step) -> Starter { Starter(s.income + c.income, s.distance + c.distance) }
? explore narrow_product_cliffs {
    from {
        vary income in range(0, 20)
        vary distance in range(0, 7)
        vary direction in range(0, 2)
        let before = Starter(income, distance)
        let context = Step(1 - direction, direction)
    }
    transition after = advance(before, context)
    find cliffs = violations of net(after, context) >= net(before, context)
    mechanisms paths from find cliffs using net
}
"#;
        let isolated = template.replace(
            "METRIC",
            "s.income + s.distance + (if s.income == 7 && s.distance == 3 { 4 } else { 0 })",
        );
        assert_product_stream_matches_oracle(&isolated, &[20, 7, 2], (280, 0, 280, 278, 2));
        let rounding = template.replace("METRIC", "s.income - (s.income / 3) * 4 + s.distance");
        assert_product_stream_matches_oracle(&rounding, &[20, 7, 2], (280, 0, 280, 238, 42));
    }

    fn assert_product_stream_matches_oracle(
        source: &str,
        radices: &[u128],
        expected: (u128, u128, u128, u128, u128),
    ) -> ThreeChunkScheduleRun {
        let oracle = canonical_exhaustive_product_oracle(source, radices);
        assert_eq!(
            (
                oracle.candidates,
                oracle.rejected,
                oracle.admitted,
                oracle.admitted_not_selected,
                oracle.admitted_selected
            ),
            expected
        );
        let candidate = execute_three_chunk_schedule(source, false);
        let canonical = execute_three_chunk_schedule(source, true);
        for run in [&candidate, &canonical] {
            assert_eq!(run.counts, expected);
            assert_eq!(run.selected_case_ids, oracle.selected_case_ids);
        }
        assert_eq!(
            candidate.snapshot.core_evidence_root(),
            canonical.snapshot.core_evidence_root()
        );
        assert_eq!(
            candidate.snapshot.exploration_evidence_root(),
            canonical.snapshot.exploration_evidence_root()
        );
        assert_eq!(candidate.target_commitment, canonical.target_commitment);
        assert_eq!(
            candidate.mechanism_closure_roots,
            canonical.mechanism_closure_roots
        );
        candidate
    }

    #[test]
    fn authored_income_distance_demo_closes_harmless_regions_and_finds_both_cliffs() {
        let source = include_str!("../../examples/relational-explore-income-distance.runa");
        let run = assert_product_stream_matches_oracle(source, &[20, 20, 2], (800, 0, 800, 798, 2));
        assert_eq!(run.certified_fragment_count, 3);
    }

    struct ThreeChunkScheduleRun {
        scheduled: Vec<(
            u128,
            RelationalCandidateScheduleReason,
            RelationalCandidateNominationRoot,
        )>,
        chunk_count: usize,
        accepted_fragment_count: usize,
        certified_fragment_count: usize,
        counts: (u128, u128, u128, u128, u128),
        selected_case_ids: BTreeSet<RelationalCaseId>,
        target_commitment: MechanismTargetCaseSetCommitment,
        mechanism_closure_roots: ([u8; 32], [u8; 32], [u8; 32]),
        snapshot: RelationalJournalSnapshot,
    }

    fn execute_three_chunk_schedule(
        source_text: &str,
        force_canonical_order: bool,
    ) -> ThreeChunkScheduleRun {
        let mut prepared = prepare(source_text);
        let mut checked = prepared.checked.view();
        let question_id = checked.question_ids()[0];
        let mechanism_request_id = RelationalAnalysisPlan::from_checked(&checked)
            .expect("plan three-chunk mechanism consumer")
            .layer_registrations()
            .iter()
            .find_map(|registration| match registration {
                RelationalAnalysisLayerRegistration::Mechanisms(registration) => {
                    Some(registration.request_id())
                }
                _ => None,
            })
            .expect("three-chunk fixture retains one mechanism request");
        let mut driver =
            RelationalStreamDriver::from_checked_with_limits_and_classification_backends(
                &checked,
                &prepared.support_plan,
                RelationalStreamDriverLimits::default(),
                None,
                Some(&prepared.classification_evaluator),
            )
            .expect("build three-chunk candidate stream scheduler");
        if force_canonical_order {
            driver.force_canonical_chunk_order_for_test();
        }
        let mut journal = RelationalJournal::new_with_region_replay_authority(
            prepared.contract.clone(),
            exact_one_region_replay_authority(&prepared),
        );
        let mut scheduled = Vec::new();
        let mut completed = false;
        let mut reopened_product_prefix = false;

        for _ in 0..1024 {
            match driver
                .step_with_base_member_limit(
                    &mut journal,
                    &mut prepared.expression_runtime,
                    &mut prepared.mechanism_runtime,
                    NonZeroU16::new(256).unwrap(),
                )
                .expect("advance three-chunk candidate stream")
            {
                RelationalStreamStepOutcome::Emitted(batch) => {
                    if let RelationalStreamQuantum::Base(
                        RelationalStepQuantum::SeedClassifiedTargetWork {
                            chunk_ordinal,
                            schedule_reason,
                            nomination_root,
                            ..
                        },
                    ) = batch.quantum()
                    {
                        let expected_decision = match schedule_reason {
                            RelationalCandidateScheduleReason::CheckedGuardBoundary => {
                                RelationalSchedulerDecision::BaseCandidateCheckedGuard
                            }
                            RelationalCandidateScheduleReason::SourceEvent => {
                                RelationalSchedulerDecision::BaseCandidateSourceEvent
                            }
                            RelationalCandidateScheduleReason::LiftedCandidate => {
                                RelationalSchedulerDecision::BaseCandidateLifted
                            }
                            RelationalCandidateScheduleReason::LowerRangeEndpoint => {
                                RelationalSchedulerDecision::BaseCandidateLowerRangeEndpoint
                            }
                            RelationalCandidateScheduleReason::UpperRangeEndpoint => {
                                RelationalSchedulerDecision::BaseCandidateUpperRangeEndpoint
                            }
                            RelationalCandidateScheduleReason::ResidualFallback => {
                                RelationalSchedulerDecision::BaseCandidateResidual
                            }
                            other => panic!(
                                "three-chunk fixture emitted unexpected candidate reason: {other:?}"
                            ),
                        };
                        assert!(matches!(
                            batch.events().first(),
                            Some(RelationalJournalEvent::Checkpoint(
                                RelationalCheckpointEvent::SchedulerDecisionRecorded {
                                    decision,
                                    nomination_root: checkpoint_root,
                                    ..
                                }
                            )) if *decision == expected_decision
                                && *checkpoint_root == Some(nomination_root)
                        ));
                        scheduled.push((chunk_ordinal, schedule_reason, nomination_root));
                    }
                    for event in batch.into_events() {
                        journal
                            .append(event)
                            .expect("append three-chunk candidate batch event");
                    }
                    if !reopened_product_prefix
                        && source_text.contains("# Starter(")
                        && journal
                            .scheduler_view()
                            .unwrap()
                            .accepted_classified_fragment_count()
                            > 0
                    {
                        use super::super::relational_journal_codec::{
                            decode_relational_journal_entry, encode_relational_journal_entry,
                            RelationalJournalCodecLimits,
                        };
                        let limits = RelationalJournalCodecLimits::default();
                        let entries = journal
                            .entries()
                            .iter()
                            .map(|entry| {
                                let bytes = encode_relational_journal_entry(entry, limits).unwrap();
                                decode_relational_journal_entry(
                                    prepared.contract.clone(),
                                    entry.sequence(),
                                    entry.previous(),
                                    &bytes,
                                    limits,
                                )
                                .unwrap()
                            })
                            .collect::<Vec<_>>();
                        let previous_head = journal.head();
                        drop(driver);
                        prepared = prepare(source_text);
                        checked = prepared.checked.view();
                        journal = RelationalJournal::replay_with_region_replay_authority(
                            prepared.contract.clone(),
                            entries,
                            exact_one_region_replay_authority(&prepared),
                        )
                        .expect("cold product-prefix replay re-proves regional certificates");
                        assert_eq!(journal.head(), previous_head);
                        driver = RelationalStreamDriver::from_checked_with_limits_and_classification_backends(
                            &checked, &prepared.support_plan,
                            RelationalStreamDriverLimits::default(), None,
                            Some(&prepared.classification_evaluator),
                        ).unwrap();
                        if force_canonical_order {
                            driver.force_canonical_chunk_order_for_test();
                        }
                        reopened_product_prefix = true;
                    }
                }
                RelationalStreamStepOutcome::Complete => {
                    completed = true;
                    break;
                }
                RelationalStreamStepOutcome::Quiescent(quiescence) => {
                    panic!("three-chunk candidate stream quiesced before closure: {quiescence:?}");
                }
            }
        }
        assert!(
            completed,
            "three-chunk candidate stream exceeded its compact fixture bound"
        );

        let view = journal
            .scheduler_view()
            .expect("inspect completed three-chunk candidate journal");
        let chunk_count = view
            .verified_case_chunk_partition()
            .unwrap()
            .expect("three-chunk canonical partition")
            .partition()
            .chunks()
            .len();
        let accepted_fragment_count = view.accepted_classified_fragment_count();
        let certified_fragment_count = (0..chunk_count)
            .filter(|ordinal| {
                matches!(
                    view.classified_support_fragment_at(*ordinal).unwrap(),
                    Some(RelationalClassifiedSupportFragment::CertifiedZeroSelected(
                        _
                    ))
                )
            })
            .count();
        let counts = view
            .classification_progress_counts(question_id)
            .expect("derive candidate-run classification counts")
            .expect("candidate-run root cardinality is exact");
        assert!(counts.is_complete());
        let counts = (
            counts.candidates(),
            counts.rejected(),
            counts.admitted(),
            counts.admitted_not_selected(),
            counts.admitted_selected(),
        );
        let selected_case_ids = view
            .materialized_selected_case_ids(question_id)
            .expect("read candidate-run selected CaseIds")
            .collect::<BTreeSet<_>>();
        let target_commitment = journal
            .analysis_state()
            .and_then(|analysis| analysis.selected_question(question_id))
            .expect("completed candidate run has an exact selected-question seal")
            .mechanism_target();
        let analysis = journal
            .analysis_state()
            .expect("completed candidate run retains closed analysis state");
        let incidence_closure = analysis
            .mechanism_closure(mechanism_request_id)
            .expect("three-chunk incidence is closed");
        let structural_closure = analysis
            .structural_quotient_closure(mechanism_request_id)
            .expect("three-chunk structural quotient is closed");
        let support_closure = analysis
            .mechanism_support_closure(mechanism_request_id)
            .expect("three-chunk mechanism support is closed");
        assert_eq!(
            incidence_closure.counts().target_cases(),
            MechanismCountEvidence::Exact(counts.4)
        );
        assert_eq!(
            incidence_closure.counts().incidence_cases(),
            MechanismCountEvidence::Exact(counts.4)
        );
        assert_eq!(structural_closure.counts().mechanisms() > 0, counts.4 > 0);
        assert_eq!(support_closure.successful_case_count(), counts.4);
        assert_eq!(support_closure.unavailable_case_count(), 0);
        let mechanism_closure_roots = (
            incidence_closure.incidence_root().bytes(),
            structural_closure.root().bytes(),
            support_closure.root().bytes(),
        );
        let snapshot = journal
            .snapshot()
            .expect("snapshot completed three-chunk semantic evidence");
        ThreeChunkScheduleRun {
            scheduled,
            chunk_count,
            accepted_fragment_count,
            certified_fragment_count,
            counts,
            selected_case_ids,
            target_commitment,
            mechanism_closure_roots,
            snapshot,
        }
    }

    #[test]
    fn candidate_sparse_stream_prioritizes_ready_result_over_base_and_replays_the_choice() {
        let temp = TestDirectory::new();
        let run_state = temp.path().join("candidate-contention-run-state");
        let member_limit = NonZeroU16::new(256).unwrap();

        let (
            paused_checkpoint,
            expected_sequence,
            expected_head,
            expected_quantum,
            expected_events,
        ) = {
            let mut prepared = prepare(CANDIDATE_RESULT_CONTENTION);
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
                .expect("build candidate contention stream scheduler");
            let mut durable =
                RelationalDurableJournal::open_or_create_with_region_replay_authority(
                    &run_state,
                    prepared.contract.clone(),
                    prepared.analysis_plan_root,
                    RelationalDurableJournalLimits::default(),
                    exact_one_region_replay_authority(&prepared),
                )
                .expect("open candidate contention journal");

            let mut selected_run_materialized = false;
            for _ in 0..64 {
                let outcome = driver
                    .step_with_base_member_limit(
                        durable
                            .journal_mut_for_event_planning()
                            .expect("borrow candidate contention journal"),
                        &mut prepared.expression_runtime,
                        &mut prepared.mechanism_runtime,
                        member_limit,
                    )
                    .expect("advance candidate contention prefix");
                let RelationalStreamStepOutcome::Emitted(batch) = outcome else {
                    panic!("candidate contention stream quiesced before selected materialization");
                };
                let quantum = batch.quantum();
                durable
                    .append_events(
                        batch.expected_sequence(),
                        batch.expected_head(),
                        batch.into_events(),
                    )
                    .expect("append candidate contention prefix");
                if matches!(
                    quantum,
                    RelationalStreamQuantum::Base(
                        RelationalStepQuantum::SelectedRunMaterialization(_)
                    )
                ) {
                    selected_run_materialized = true;
                    break;
                }
            }
            assert!(
                selected_run_materialized,
                "candidate contention fixture did not materialize its sparse selected run"
            );

            {
                let journal = durable.journal().expect("inspect contention prefix");
                let view = journal
                    .scheduler_view()
                    .expect("inspect contention scheduler state");
                assert!(view.classified_support_fragments().unwrap().is_empty());
                assert_eq!(view.accepted_classified_fragment_count(), 1);
                assert_eq!(
                    view.selected_run_materializations(question_id)
                        .unwrap()
                        .count(),
                    1
                );
            }

            // The base scheduler is independently ready to seed chunk 0, but
            // the stream coordinator must first consume the selected rows
            // exposed by candidate chunk 1.
            let base = RelationalStepDriver::from_checked_with_max_members_per_quantum_and_classification_backends(
                &checked,
                &prepared.support_plan,
                member_limit,
                None,
                Some(&prepared.classification_evaluator),
            )
            .expect("build contention base scheduler");
            let RelationalStepOutcome::Emitted(base_batch) = base
                .step_with_max_members_per_quantum(
                    durable.journal().expect("inspect ready base work"),
                    &mut prepared.expression_runtime,
                    member_limit,
                )
                .expect("plan ready lower-priority base work")
            else {
                panic!("candidate contention base scheduler was not ready");
            };
            assert!(matches!(
                base_batch.quantum(),
                RelationalStepQuantum::SeedClassifiedTargetWork {
                    chunk_ordinal: 0,
                    schedule_reason: RelationalCandidateScheduleReason::LowerRangeEndpoint,
                    ..
                }
            ));

            let outcome = driver
                .step_with_base_member_limit(
                    durable
                        .journal_mut_for_event_planning()
                        .expect("borrow contention decision prefix"),
                    &mut prepared.expression_runtime,
                    &mut prepared.mechanism_runtime,
                    member_limit,
                )
                .expect("schedule contention winner");
            let RelationalStreamStepOutcome::Emitted(batch) = outcome else {
                panic!("candidate contention winner was not emitted");
            };
            assert!(matches!(
                batch.quantum(),
                RelationalStreamQuantum::Result(
                    RelationalResultStepQuantum::EvaluateSelectedRows {
                        row_count,
                        seals_input: false,
                        ..
                    }
                ) if row_count.get() == 20
            ));
            assert!(matches!(
                batch.events().first(),
                Some(RelationalJournalEvent::Checkpoint(
                    RelationalCheckpointEvent::SchedulerDecisionRecorded {
                        decision: RelationalSchedulerDecision::ReadyResult,
                        ..
                    }
                ))
            ));

            let expected_sequence = batch.expected_sequence();
            let expected_head = batch.expected_head();
            let expected_quantum = batch.quantum();
            let expected_events = batch.events().to_vec();
            let paused_checkpoint = durable
                .flush_for_pause()
                .expect("flush unadvanced contention prefix");
            (
                paused_checkpoint,
                expected_sequence,
                expected_head,
                expected_quantum,
                expected_events,
            )
        };

        let mut prepared = prepare(CANDIDATE_RESULT_CONTENTION);
        let checked = prepared.checked.view();
        let driver = RelationalStreamDriver::from_checked_with_limits_and_classification_backends(
            &checked,
            &prepared.support_plan,
            RelationalStreamDriverLimits::default(),
            None,
            Some(&prepared.classification_evaluator),
        )
        .expect("rebuild candidate contention stream scheduler");
        let mut durable = RelationalDurableJournal::open_or_create_with_region_replay_authority(
            &run_state,
            prepared.contract.clone(),
            prepared.analysis_plan_root,
            RelationalDurableJournalLimits::default(),
            exact_one_region_replay_authority(&prepared),
        )
        .expect("reopen candidate contention journal");
        assert_eq!(
            durable
                .journal()
                .expect("inspect reopened contention journal")
                .next_sequence(),
            paused_checkpoint.next_sequence()
        );
        assert_eq!(
            durable
                .journal()
                .expect("inspect reopened contention journal")
                .head(),
            paused_checkpoint.head()
        );

        let outcome = driver
            .step_with_base_member_limit(
                durable
                    .journal_mut_for_event_planning()
                    .expect("borrow reopened contention journal"),
                &mut prepared.expression_runtime,
                &mut prepared.mechanism_runtime,
                member_limit,
            )
            .expect("replay contention winner");
        let RelationalStreamStepOutcome::Emitted(batch) = outcome else {
            panic!("replayed candidate contention winner was not emitted");
        };
        assert_eq!(batch.expected_sequence(), expected_sequence);
        assert_eq!(batch.expected_head(), expected_head);
        assert_eq!(batch.quantum(), expected_quantum);
        assert_eq!(batch.events(), expected_events.as_slice());
        durable
            .append_events(
                batch.expected_sequence(),
                batch.expected_head(),
                batch.into_events(),
            )
            .expect("append replayed contention winner");
    }

    #[test]
    fn paged_uniform_selection_materializes_bounded_runs_independent_of_slice_timing() {
        use super::super::relational_bounded_chunk_partition::{
            plan_relational_bounded_case_chunks, reverify_relational_case_chunk_partition_artifact,
            RelationalCaseChunkPlanningOutcome,
        };
        use super::super::relational_classified_sweep::{
            classify_relational_case_chunk, classify_relational_case_chunk_slice,
            finalize_relational_classified_case_chunk,
        };
        use super::super::relational_selected_run_materialization::materialize_relational_selected_run;
        use super::super::relational_support_planner::prove_relational_case_image_injectivity;
        use super::super::support_cell::relational_case_chunk_partition_gateway;
        let source = r#"
? explore paged_selected {
    from {
        vary before in range(0, 1048577)
        given context = ()
    }
    transition after = before + 1
    find cases = all
}
"#;
        let mut prepared = prepare(source);
        let checked = prepared.checked.view();
        let plan = &prepared.support_plan;
        let image = prove_relational_case_image_injectivity(plan).unwrap();
        let RelationalCaseChunkPlanningOutcome::Partitioned(partition) =
            plan_relational_bounded_case_chunks(plan, &image).unwrap()
        else {
            panic!("expected pages");
        };
        let verified = reverify_relational_case_chunk_partition_artifact(
            partition.artifact(),
            plan,
            image.injectivity(),
        )
        .unwrap();
        let injectivity =
            relational_case_chunk_partition_gateway::injectivity(&verified, 0).unwrap();
        let direct = classify_relational_case_chunk(
            &checked,
            plan,
            &verified,
            0,
            &injectivity,
            &mut prepared.expression_runtime,
        )
        .unwrap();
        assert_eq!(direct.artifact().runs().len(), 2);
        for run in direct.artifact().runs() {
            assert_eq!(run.cardinality(), 256);
            assert!(run.outcome().any_selected());
        }
        let mut prior = None;
        loop {
            let slice = classify_relational_case_chunk_slice(
                &checked,
                plan,
                &verified,
                0,
                &injectivity,
                prior.as_ref(),
                NonZeroU16::new(61).unwrap(),
                &mut prepared.expression_runtime,
            )
            .unwrap();
            if slice.accumulator().is_complete() {
                let sliced = finalize_relational_classified_case_chunk(
                    plan,
                    &verified,
                    0,
                    &injectivity,
                    slice.accumulator(),
                )
                .unwrap();
                assert_eq!(sliced.artifact(), direct.artifact());
                break;
            }
            prior = Some(slice.accumulator().clone());
        }
        let mut ids = BTreeSet::new();
        for ordinal in 0..2 {
            let materialized = materialize_relational_selected_run(
                &checked,
                plan,
                &verified,
                direct.verified(),
                ordinal,
                &mut prepared.expression_runtime,
            )
            .unwrap();
            assert_eq!(materialized.artifact().materialized_case_count(), 256);
            for case in materialized.artifact().cases() {
                assert!(ids.insert(case.case_id()));
            }
        }
        assert_eq!(ids.len(), 512);
    }

    #[test]
    fn paged_classification_cold_resume_preserves_every_unit_and_slice_bounds() {
        use super::super::relational_classified_sweep::classify_relational_case_chunk;
        use super::super::support_cell::relational_case_chunk_partition_gateway;

        let source = r#"
? explore paged_classification {
    from {
        vary before in range(0, 1048577)
        given context = ()
    }
    transition after = before + 1
    where before before % 2 == 0
    find cliffs = matches of before % 4 == 0
}
"#;
        let temp = TestDirectory::new();
        let path = temp.path().join("paged-classification");
        let mut completed_artifact = None;
        for epoch in 0..2 {
            // A new checked program, runtime, driver and disk fold each time.
            let mut prepared = prepare(source);
            let checked = prepared.checked.view();
            let mut driver =
                RelationalStreamDriver::from_checked_with_limits_and_classification_backends(
                    &checked,
                    &prepared.support_plan,
                    RelationalStreamDriverLimits::default(),
                    None,
                    Some(&prepared.classification_evaluator),
                )
                .unwrap();
            driver.force_canonical_chunk_order_for_test();
            let mut durable =
                RelationalDurableJournal::open_or_create_with_region_replay_authority(
                    &path,
                    prepared.contract.clone(),
                    prepared.analysis_plan_root,
                    RelationalDurableJournalLimits::default(),
                    exact_one_region_replay_authority(&prepared),
                )
                .unwrap();
            if epoch == 1 {
                let view = durable.journal().unwrap().scheduler_view().unwrap();
                let prior = view.classified_chunk_accumulator().unwrap().unwrap();
                assert_eq!(prior.next_coordinate(), 17);
                assert_eq!(prior.interval_end_exclusive(), 512);
            }
            let mut reached_checkpoint = false;
            for _ in 0..64 {
                let limit = if epoch == 0 { 17 } else { u16::MAX };
                let outcome = driver
                    .step_with_base_member_limit(
                        durable.journal_mut_for_event_planning().unwrap(),
                        &mut prepared.expression_runtime,
                        &mut prepared.mechanism_runtime,
                        NonZeroU16::new(limit).unwrap(),
                    )
                    .unwrap();
                let RelationalStreamStepOutcome::Emitted(batch) = outcome else {
                    panic!("page prefix unexpectedly quiesced");
                };
                let quantum = batch.quantum();
                durable
                    .append_events(
                        batch.expected_sequence(),
                        batch.expected_head(),
                        batch.into_events(),
                    )
                    .unwrap();
                if let RelationalStreamQuantum::Base(RelationalStepQuantum::ClassifiedSweep(
                    slice,
                )) = quantum
                {
                    assert!(slice
                        .evaluated_member_count()
                        .map_or(true, |count| count.get() <= 256));
                    let view = durable.journal().unwrap().scheduler_view().unwrap();
                    if epoch == 0 {
                        assert_eq!(
                            view.classified_chunk_accumulator()
                                .unwrap()
                                .unwrap()
                                .next_coordinate(),
                            17
                        );
                        assert_eq!(view.accepted_classified_fragment_count(), 0);
                        reached_checkpoint = true;
                        break;
                    }
                    if let Some(fragment) = view.classified_support_fragment_at(0).unwrap() {
                        let artifact = fragment.concrete().unwrap();
                        assert_eq!(artifact.evaluated_case_count(), 512);
                        assert_eq!(artifact.rejected_count(), 256);
                        assert_eq!(artifact.admitted_count(), 256);
                        assert_eq!(artifact.admitted_selected_counts(), &[128]);
                        assert_eq!(artifact.runs().len(), 512);
                        for (index, run) in artifact.runs().iter().enumerate() {
                            assert_eq!(run.interval_start(), index as u128);
                            assert_eq!(run.interval_end_exclusive(), index as u128 + 1);
                            let expected_admission = if index % 2 == 0 {
                                AdmissionDecision::Admitted
                            } else {
                                AdmissionDecision::Rejected
                            };
                            assert_eq!(run.outcome().admission(), expected_admission);
                            if index % 2 == 0 {
                                let expected_selection = if index % 4 == 0 {
                                    SelectionDecision::Selected
                                } else {
                                    SelectionDecision::NotSelected
                                };
                                assert_eq!(run.outcome().selection(0), Some(expected_selection));
                            }
                        }
                        let verified = view.verified_case_chunk_partition().unwrap().unwrap();
                        let injectivity =
                            relational_case_chunk_partition_gateway::injectivity(verified, 0)
                                .unwrap();
                        let direct = classify_relational_case_chunk(
                            &checked,
                            &prepared.support_plan,
                            verified,
                            0,
                            &injectivity,
                            &mut prepared.expression_runtime,
                        )
                        .unwrap();
                        assert_eq!(
                            direct.artifact(),
                            artifact,
                            "slice scheduling must not change exact evidence"
                        );
                        completed_artifact = Some(artifact.clone());
                        reached_checkpoint = true;
                        break;
                    }
                }
            }
            assert!(reached_checkpoint);
            durable.flush_for_pause().unwrap();
        }
        let prepared = prepare(source);
        let durable = RelationalDurableJournal::open_or_create_with_region_replay_authority(
            &path,
            prepared.contract.clone(),
            prepared.analysis_plan_root,
            RelationalDurableJournalLimits::default(),
            exact_one_region_replay_authority(&prepared),
        )
        .unwrap();
        let view = durable.journal().unwrap().scheduler_view().unwrap();
        assert_eq!(
            view.classified_support_fragment_at(0)
                .unwrap()
                .unwrap()
                .concrete(),
            completed_artifact.as_ref()
        );
        assert_eq!(view.accepted_classified_fragment_count(), 1);
    }

    #[test]
    fn candidate_partial_slice_reopens_on_the_same_canonical_chunk() {
        let temp = TestDirectory::new();
        let run_state = temp.path().join("candidate-partial-run-state");
        let mut prepared = prepare(HYBRID);
        let mut paused_nomination_root = None;

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
                .expect("build candidate partial scheduler");
            let mut durable =
                RelationalDurableJournal::open_or_create_with_region_replay_authority(
                    &run_state,
                    prepared.contract.clone(),
                    prepared.analysis_plan_root,
                    RelationalDurableJournalLimits::default(),
                    exact_one_region_replay_authority(&prepared),
                )
                .expect("open candidate partial journal");
            let mut paused = false;
            for _ in 0..32 {
                let outcome = driver
                    .step_with_base_member_limit(
                        durable
                            .journal_mut_for_event_planning()
                            .expect("borrow candidate partial journal"),
                        &mut prepared.expression_runtime,
                        &mut prepared.mechanism_runtime,
                        NonZeroU16::new(17).unwrap(),
                    )
                    .expect("advance to candidate partial slice");
                let RelationalStreamStepOutcome::Emitted(batch) = outcome else {
                    panic!("candidate partial fixture quiesced before its first slice");
                };
                let quantum = batch.quantum();
                let checkpoint_nomination_root = match batch.events().first() {
                    Some(RelationalJournalEvent::Checkpoint(
                        RelationalCheckpointEvent::SchedulerDecisionRecorded {
                            nomination_root,
                            ..
                        },
                    )) => *nomination_root,
                    _ => None,
                };
                durable
                    .append_events(
                        batch.expected_sequence(),
                        batch.expected_head(),
                        batch.into_events(),
                    )
                    .expect("append candidate partial prefix");
                if let RelationalStreamQuantum::Base(RelationalStepQuantum::ClassifiedSweep(
                    classified,
                )) = quantum
                {
                    assert_eq!(classified.chunk_ordinal(), 1);
                    assert_eq!(
                        classified.schedule_reason(),
                        RelationalCandidateScheduleReason::CheckedGuardBoundary
                    );
                    assert!(classified.classified_artifact_id().is_none());
                    assert_eq!(
                        checkpoint_nomination_root,
                        Some(classified.nomination_root())
                    );
                    paused_nomination_root = Some(classified.nomination_root());
                    paused = true;
                    break;
                }
            }
            assert!(
                paused,
                "candidate partial fixture never reached its first slice"
            );
            let view = durable
                .journal()
                .expect("inspect candidate partial prefix")
                .scheduler_view()
                .expect("inspect candidate partial scheduler state");
            assert_eq!(
                view.classified_chunk_accumulator()
                    .unwrap()
                    .expect("candidate slice accumulator")
                    .chunk_ordinal(),
                1
            );
            assert_eq!(view.accepted_classified_fragment_count(), 0);
            durable
                .flush_for_pause()
                .expect("flush candidate partial prefix");
        }
        let paused_nomination_root =
            paused_nomination_root.expect("paused candidate slice has nomination provenance");

        let checked = prepared.checked.view();
        let driver = RelationalStreamDriver::from_checked_with_limits_and_classification_backends(
            &checked,
            &prepared.support_plan,
            RelationalStreamDriverLimits::default(),
            None,
            Some(&prepared.classification_evaluator),
        )
        .expect("rebuild candidate partial scheduler");
        let mut durable = RelationalDurableJournal::open_or_create_with_region_replay_authority(
            &run_state,
            prepared.contract.clone(),
            prepared.analysis_plan_root,
            RelationalDurableJournalLimits::default(),
            exact_one_region_replay_authority(&prepared),
        )
        .expect("reopen candidate partial journal");
        let outcome = driver
            .step_with_base_member_limit(
                durable
                    .journal_mut_for_event_planning()
                    .expect("borrow reopened candidate partial journal"),
                &mut prepared.expression_runtime,
                &mut prepared.mechanism_runtime,
                NonZeroU16::new(17).unwrap(),
            )
            .expect("resume candidate partial slice");
        let RelationalStreamStepOutcome::Emitted(batch) = outcome else {
            panic!("candidate partial resume did not emit its owned chunk");
        };
        assert!(matches!(
            batch.quantum(),
            RelationalStreamQuantum::Base(RelationalStepQuantum::ClassifiedSweep(classified))
                if classified.chunk_ordinal() == 1
                    && classified.schedule_reason()
                        == RelationalCandidateScheduleReason::CheckedGuardBoundary
                    && classified.nomination_root() == paused_nomination_root
        ));
        assert!(matches!(
            batch.events().first(),
            Some(RelationalJournalEvent::Checkpoint(
                RelationalCheckpointEvent::SchedulerDecisionRecorded {
                    nomination_root,
                    ..
                }
            )) if *nomination_root == Some(paused_nomination_root)
        ));
    }

    #[test]
    fn plural_classified_sweep_resumes_once_and_shares_joint_question_runs() {
        let temp = TestDirectory::new();
        let run_state = temp.path().join("run-state");
        let mut classified_member_evaluations = 0u128;

        let (question_twenty_id, question_ten_id, paused_checkpoint) = {
            let mut prepared = prepare(SHARED_PLURAL_CLASSIFIED_SWEEP);
            let checked = prepared.checked.view();
            let [question_twenty_id, question_ten_id] = checked.find_question_ids() else {
                panic!("shared plural fixture must have exactly two authored FIND questions");
            };
            assert!(prepared.region_replay_authority.is_none());
            assert!(prepared.native_classifier_plan.is_none());
            assert!(!prepared.native_classifier_shape_v2);

            let driver =
                RelationalStreamDriver::from_checked_with_limits_and_classification_backends(
                    &checked,
                    &prepared.support_plan,
                    RelationalStreamDriverLimits::default(),
                    None,
                    Some(&prepared.classification_evaluator),
                )
                .expect("build shared plural stream scheduler");
            let mut durable = RelationalDurableJournal::open_or_create(
                &run_state,
                prepared.contract.clone(),
                prepared.analysis_plan_root,
                RelationalDurableJournalLimits::default(),
            )
            .expect("open shared plural durable journal");

            let mut paused_checkpoint = None;
            for _ in 0..64 {
                let outcome = driver
                    .step_with_base_member_limit(
                        durable
                            .journal_mut_for_event_planning()
                            .expect("borrow shared plural planning journal"),
                        &mut prepared.expression_runtime,
                        &mut prepared.mechanism_runtime,
                        NonZeroU16::new(17).unwrap(),
                    )
                    .expect("advance shared plural prefix");
                let RelationalStreamStepOutcome::Emitted(batch) = outcome else {
                    panic!("shared plural stream quiesced before its first classified slice");
                };
                let quantum = batch.quantum();
                let pause_after_batch = match quantum {
                    RelationalStreamQuantum::Base(RelationalStepQuantum::ClassifiedSweep(
                        sweep,
                    )) => {
                        let evaluated = sweep
                            .evaluated_member_count()
                            .expect("the first shared plural slice evaluates members");
                        classified_member_evaluations += u128::from(evaluated.get());
                        assert_eq!(evaluated.get(), 17);
                        assert_eq!(sweep.chunk_ordinal(), 0);
                        assert_eq!(sweep.interval_start(), 0);
                        assert_eq!(sweep.interval_end_exclusive(), 256);
                        assert!(sweep.slice_artifact_id().is_some());
                        assert!(sweep.classified_artifact_id().is_none());
                        true
                    }
                    _ => false,
                };
                durable
                    .append_events(
                        batch.expected_sequence(),
                        batch.expected_head(),
                        batch.into_events(),
                    )
                    .expect("append shared plural prefix batch");
                if pause_after_batch {
                    let view = durable
                        .journal()
                        .expect("inspect shared plural prefix")
                        .scheduler_view()
                        .expect("inspect shared plural prefix scheduler");
                    let accumulator = view
                        .classified_chunk_accumulator()
                        .expect("inspect shared plural accumulator")
                        .expect("the incomplete shared plural slice is retained");
                    assert_eq!(accumulator.interval_start(), 0);
                    assert_eq!(accumulator.interval_end_exclusive(), 256);
                    assert_eq!(accumulator.next_coordinate(), 17);
                    assert_eq!(accumulator.evaluated_case_count(), 17);
                    assert!(view
                        .classified_support_fragments()
                        .expect("inspect shared plural classified prefix")
                        .is_empty());
                    paused_checkpoint = Some(
                        durable
                            .flush_for_pause()
                            .expect("flush incomplete shared plural slice"),
                    );
                    break;
                }
            }

            (
                *question_twenty_id,
                *question_ten_id,
                paused_checkpoint.expect("fixture did not reach a 17-member classified slice"),
            )
        };

        let mut prepared = prepare(SHARED_PLURAL_CLASSIFIED_SWEEP);
        let checked = prepared.checked.view();
        assert_eq!(
            checked.find_question_ids(),
            [question_twenty_id, question_ten_id]
        );
        assert!(prepared.region_replay_authority.is_none());
        assert!(prepared.native_classifier_plan.is_none());
        let driver = RelationalStreamDriver::from_checked_with_limits_and_classification_backends(
            &checked,
            &prepared.support_plan,
            RelationalStreamDriverLimits::default(),
            None,
            Some(&prepared.classification_evaluator),
        )
        .expect("rebuild shared plural stream scheduler after pause");
        let mut durable = RelationalDurableJournal::open_or_create(
            &run_state,
            prepared.contract.clone(),
            prepared.analysis_plan_root,
            RelationalDurableJournalLimits::default(),
        )
        .expect("reopen shared plural durable journal");
        {
            let reopened = durable
                .journal()
                .expect("inspect reopened shared plural journal");
            assert_eq!(reopened.next_sequence(), paused_checkpoint.next_sequence());
            assert_eq!(reopened.head(), paused_checkpoint.head());
            let accumulator = reopened
                .scheduler_view()
                .expect("inspect reopened shared plural scheduler")
                .classified_chunk_accumulator()
                .expect("inspect reopened shared plural accumulator")
                .expect("the incomplete shared plural slice survives replay");
            assert_eq!(accumulator.next_coordinate(), 17);
            assert_eq!(accumulator.evaluated_case_count(), 17);
        }

        let mut completed = false;
        for _ in 0..256 {
            match driver
                .step_with_base_member_limit(
                    durable
                        .journal_mut_for_event_planning()
                        .expect("borrow reopened shared plural planning journal"),
                    &mut prepared.expression_runtime,
                    &mut prepared.mechanism_runtime,
                    NonZeroU16::new(17).unwrap(),
                )
                .expect("resume shared plural stream")
            {
                RelationalStreamStepOutcome::Emitted(batch) => {
                    if let RelationalStreamQuantum::Base(RelationalStepQuantum::ClassifiedSweep(
                        sweep,
                    )) = batch.quantum()
                    {
                        classified_member_evaluations += sweep
                            .evaluated_member_count()
                            .map_or(0, |evaluated| u128::from(evaluated.get()));
                    }
                    durable
                        .append_events(
                            batch.expected_sequence(),
                            batch.expected_head(),
                            batch.into_events(),
                        )
                        .expect("append resumed shared plural batch");
                }
                RelationalStreamStepOutcome::Complete => {
                    completed = true;
                    break;
                }
                RelationalStreamStepOutcome::Quiescent(quiescence) => {
                    panic!("shared plural stream quiesced before closure: {quiescence:?}");
                }
            }
        }
        assert!(
            completed,
            "shared plural fixture exceeded its compact bound"
        );
        assert_eq!(
            classified_member_evaluations, 300,
            "two FIND predicates share one 300-member classified sweep"
        );
        durable
            .flush_for_pause()
            .expect("flush completed shared plural journal");

        let journal = durable
            .journal()
            .expect("inspect completed shared plural journal");
        let view = journal
            .scheduler_view()
            .expect("inspect completed shared plural scheduler");
        let fragments = view
            .classified_support_fragments()
            .expect("inspect shared plural classified fragments");
        assert_eq!(fragments.len(), 2);
        assert!(fragments
            .iter()
            .all(|fragment| fragment.certificate().is_none()));
        assert_eq!(
            fragments
                .iter()
                .map(RelationalClassifiedSupportFragment::exact_case_count)
                .sum::<u128>(),
            300
        );
        assert_eq!(
            fragments
                .iter()
                .map(|fragment| {
                    fragment
                        .admitted_selected_count(question_twenty_id)
                        .expect("shared fragment contains the final-twenty question")
                })
                .sum::<u128>(),
            20
        );
        assert_eq!(
            fragments
                .iter()
                .map(|fragment| {
                    fragment
                        .admitted_selected_count(question_ten_id)
                        .expect("shared fragment contains the final-ten question")
                })
                .sum::<u128>(),
            10
        );

        let joint_runs = fragments
            .iter()
            .flat_map(|fragment| {
                let artifact = fragment
                    .concrete()
                    .expect("plural execution has no exact-one regional fragment");
                let twenty_index = artifact
                    .question_index(question_twenty_id)
                    .expect("shared artifact indexes the final-twenty question");
                let ten_index = artifact
                    .question_index(question_ten_id)
                    .expect("shared artifact indexes the final-ten question");
                artifact.runs().iter().map(move |run| {
                    assert_eq!(
                        run.outcome().selection(artifact.question_ids().len()),
                        None,
                        "the packed mask rejects indexes outside its logical question set",
                    );
                    (
                        run.interval_start(),
                        run.interval_end_exclusive(),
                        run.outcome().admission(),
                        run.outcome()
                            .selection(twenty_index)
                            .expect("admitted run has a final-twenty decision"),
                        run.outcome()
                            .selection(ten_index)
                            .expect("admitted run has a final-ten decision"),
                    )
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            joint_runs,
            vec![
                (
                    0,
                    256,
                    AdmissionDecision::Admitted,
                    SelectionDecision::NotSelected,
                    SelectionDecision::NotSelected,
                ),
                (
                    256,
                    280,
                    AdmissionDecision::Admitted,
                    SelectionDecision::NotSelected,
                    SelectionDecision::NotSelected,
                ),
                (
                    280,
                    290,
                    AdmissionDecision::Admitted,
                    SelectionDecision::Selected,
                    SelectionDecision::NotSelected,
                ),
                (
                    290,
                    300,
                    AdmissionDecision::Admitted,
                    SelectionDecision::Selected,
                    SelectionDecision::Selected,
                ),
            ]
        );

        assert_eq!(
            view.selected_run_materializations(question_twenty_id)
                .expect("inspect final-twenty materializations")
                .count(),
            2
        );
        assert_eq!(
            view.selected_run_materializations(question_ten_id)
                .expect("inspect final-ten materializations")
                .count(),
            1
        );
        let twenty_cases = view
            .materialized_selected_case_ids(question_twenty_id)
            .expect("inspect final-twenty concrete cases")
            .collect::<std::collections::BTreeSet<_>>();
        let ten_cases = view
            .materialized_selected_case_ids(question_ten_id)
            .expect("inspect final-ten concrete cases")
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(twenty_cases.len(), 20);
        assert_eq!(ten_cases.len(), 10);
        assert!(ten_cases.is_subset(&twenty_cases));
        assert_eq!(twenty_cases.union(&ten_cases).count(), 20);
        assert!(view
            .selected_run_materializations_cover_classified_slots(question_twenty_id)
            .expect("verify final-twenty materialization cover"));
        assert!(view
            .selected_run_materializations_cover_classified_slots(question_ten_id)
            .expect("verify final-ten materialization cover"));
        assert!(view.support_catalog_is_sealed());
        assert!(journal
            .analysis_state()
            .is_some_and(|analysis| analysis.is_closed()));
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

        let [(
            ExploreAnalysisNodeIr::Result(_),
            CheckedExploreAnalysisIdentity::View { view_id, .. },
        )] = checked.analysis_nodes().collect::<Vec<_>>().as_slice()
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
