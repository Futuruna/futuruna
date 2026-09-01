//! Producer-owned lowering from one checked Explore query into the reusable
//! relational classification graph.
//!
//! V1 is intentionally small and total. It lowers exact scalar expressions,
//! checked constructors and singular projections, conservative first-match
//! control flow, checked query/callable/pattern binders, and closed acyclic
//! pure functions. Every other checked shape becomes one lane-local residual
//! for the concrete evaluator; no runtime spelling lookup or native-backend
//! AST is used.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::explore::relational_classification_capsule::{
    ClassificationAdmissionScope, ClassificationBinaryOp, ClassificationCallableDefinition,
    ClassificationCallableId, ClassificationConstant, ClassificationInputSlot,
    ClassificationInterner, ClassificationLaneRoot, ClassificationNodeId, ClassificationNodeKey,
    ClassificationNodeKind, ClassificationResidual, ClassificationResidualDependency,
    ClassificationResidualReason, ClassificationRuntimeLayout, ClassificationSemanticLane,
    ClassificationTypeId, ClassificationUnaryOp, FrozenClassificationProgram,
    FrozenClassificationRuntimeShapes, RelationalClassificationCapsuleError,
    RuntimeConstructorShape,
};
use crate::explore::{
    ExploreFindIr, ExploreSourceBindingKindIr, ExploreSourceBindingRoleIr, ExploreSuccessorKindIr,
};
use crate::{
    checked_explore_projection_binder_digest, checked_explore_projection_constructor_digest,
    checked_explore_projection_owner_digest, checked_explore_semantic_binders,
    checked_explore_semantic_dependency_digest, checked_explore_semantic_dependency_root_digest,
    hash_checked_explore_type_schema, CheckedBinderSiteId, CheckedCallTarget, CheckedCallableId,
    CheckedConstructorIdentity, CheckedConstructorLayout, CheckedDataFieldId, CheckedDataTypeId,
    CheckedExploreQuerySites, CheckedExploreSemanticDependency, CheckedExploreSemanticIndex,
    CheckedExpressionResolution, CheckedExpressionType, CheckedFieldResolution,
    CheckedPatternSiteId, CheckedResolutionArtifacts, CheckedResolutionRecorder,
    CheckedValueBinding, ExploreAdmissionScope, ExprKind, ExprSiteId, Literal, MatchArm, Pat, Stmt,
    Ty,
};

const CLASSIFICATION_TYPE_DIGEST_V1: &[u8] = b"futuruna.checked-explore-classification-type.v1\0";
const CLASSIFICATION_SYNTHETIC_FIND_SITE_V1: &[u8] =
    b"futuruna.checked-explore-classification-find-all-site.v1\0";
const CLASSIFICATION_FIELD_DEPENDENCY_V1: &[u8] =
    b"futuruna.checked-explore-classification-field-dependency.v1\0";

#[derive(Debug)]
pub(crate) enum CheckedExploreClassificationError {
    CheckedBoundary(Box<str>),
    Capsule(RelationalClassificationCapsuleError),
}

/// Producer-owned executable graph and the separately sealed runtime
/// presentation metadata needed by name-free constructor nodes.
///
/// Runtime spellings deliberately do not participate in the reusable graph
/// root. The checked query artifact nevertheless retains and rebuilds both
/// siblings together, and capsule binding commits both roots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CheckedExploreClassification {
    pub(crate) program: FrozenClassificationProgram,
    pub(crate) runtime_shapes: FrozenClassificationRuntimeShapes,
}

impl fmt::Display for CheckedExploreClassificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CheckedBoundary(message) => formatter.write_str(message),
            Self::Capsule(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl Error for CheckedExploreClassificationError {}

impl From<RelationalClassificationCapsuleError> for CheckedExploreClassificationError {
    fn from(error: RelationalClassificationCapsuleError) -> Self {
        Self::Capsule(error)
    }
}

/// Lower one already-joined checked query boundary into the strict executable
/// V1 classification subset.
///
/// The caller must supply the exact analysis program, resolution artifact,
/// closed query, semantic sites, and question identity that are being joined
/// into one checked artifact. Structural divergence is a producer error. An
/// unsupported checked expression is represented by exactly one residual for
/// its semantic lane.
pub(crate) fn checked_explore_classification_program(
    program: &crate::CheckedAnalysisProgram,
    resolutions: &CheckedResolutionArtifacts,
    closed_query: &crate::explore::ExploreQueryIr,
    sites: &CheckedExploreQuerySites,
    question_id: crate::explore::QuestionId,
) -> Result<CheckedExploreClassification, CheckedExploreClassificationError> {
    if program.id != resolutions.analysis_program {
        return Err(CheckedExploreClassificationError::CheckedBoundary(
            "classification producer inputs belong to different checked programs".into(),
        ));
    }
    closed_query
        .validate()
        .map_err(|message| CheckedExploreClassificationError::CheckedBoundary(message.into()))?;
    validate_query_site_shape(program, closed_query, sites)?;

    let semantic_binders =
        checked_explore_semantic_binders(closed_query, sites).map_err(|issue| {
            CheckedExploreClassificationError::CheckedBoundary(
                format!("checked classification binder boundary is incoherent: {issue:?}")
                    .into_boxed_str(),
            )
        })?;

    CheckedClassificationProducer {
        query: closed_query,
        sites,
        question_id,
        index: CheckedExploreSemanticIndex::build(program),
        resolutions,
        semantic_binders,
        interner: ClassificationInterner::default(),
        expected_lanes: Vec::new(),
        roots: Vec::new(),
        residuals: Vec::new(),
        callable_states: BTreeMap::new(),
        callable_definitions: BTreeMap::new(),
        runtime_shapes: BTreeMap::new(),
        deferred_binder_types: BTreeMap::new(),
    }
    .produce()
}

fn validate_query_site_shape(
    program: &crate::CheckedAnalysisProgram,
    query: &crate::explore::ExploreQueryIr,
    sites: &CheckedExploreQuerySites,
) -> Result<(), CheckedExploreClassificationError> {
    if query.source.bindings.len() != sites.source_bindings.len()
        || query.admissions.len() != sites.admissions.len()
        || query.find.predicate().is_some() != sites.selection.is_some()
    {
        return Err(CheckedExploreClassificationError::CheckedBoundary(
            "checked classification query and semantic sites diverged".into(),
        ));
    }
    let sites_belong_to_program = sites
        .source_bindings
        .iter()
        .map(|binding| &binding.expression)
        .chain(std::iter::once(&sites.successor))
        .chain(sites.admissions.iter())
        .chain(sites.selection.iter())
        .all(|site| site.analysis_program == program.id);
    if !sites_belong_to_program {
        return Err(CheckedExploreClassificationError::CheckedBoundary(
            "classification semantic sites belong to a different checked program".into(),
        ));
    }
    for (ordinal, binding) in query.source.bindings.iter().enumerate() {
        if binding.binding_index != ordinal {
            return Err(CheckedExploreClassificationError::CheckedBoundary(
                "checked classification source binding order is not canonical".into(),
            ));
        }
    }
    for (ordinal, admission) in query.admissions.iter().enumerate() {
        if admission.admission_index != ordinal {
            return Err(CheckedExploreClassificationError::CheckedBoundary(
                "checked classification admission order is not canonical".into(),
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScalarKind {
    Integer,
    Float,
    Boolean,
    String,
    Character,
    Unit,
}

#[derive(Clone, Copy, Debug)]
struct LoweredValue {
    node: ClassificationNodeId,
    ty: ClassificationTypeId,
    scalar: Option<ScalarKind>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DeferredFieldProjection {
    owner_id: [u8; 32],
    variant_ordinal: u32,
    field_ordinal: u32,
    base: ClassificationNodeId,
    dependency: [u8; 32],
}

#[derive(Clone, Copy, Debug)]
enum BinderValue {
    Lowered(LoweredValue),
    /// A one-level match field binder has no retained field type of its own.
    /// The checked type at an actual binder-use site supplies the result type;
    /// repeated uses must agree before a typed Project node is interned.
    DeferredProjection(DeferredFieldProjection),
    UnsupportedType,
}

type BinderEnvironment = BTreeMap<CheckedBinderSiteId, BinderValue>;

#[derive(Clone, Debug)]
struct LoweringFailure {
    reason: ClassificationResidualReason,
    dependencies: Vec<ClassificationResidualDependency>,
}

impl LoweringFailure {
    fn with_dependency(mut self, dependency: ClassificationResidualDependency) -> Self {
        self.dependencies.push(dependency);
        self
    }

    fn without_node_dependencies(mut self) -> Self {
        self.dependencies
            .retain(|dependency| !matches!(dependency, ClassificationResidualDependency::Node(_)));
        self
    }
}

#[derive(Debug)]
enum LoweringError {
    Residual(LoweringFailure),
    Capsule(RelationalClassificationCapsuleError),
}

impl From<RelationalClassificationCapsuleError> for LoweringError {
    fn from(error: RelationalClassificationCapsuleError) -> Self {
        Self::Capsule(error)
    }
}

type LoweringResult = Result<LoweredValue, LoweringError>;

#[derive(Clone, Debug)]
enum ResidualIdentityRoot {
    Expression(ExprSiteId),
    Type(Ty),
    Synthetic([u8; 32]),
}

#[derive(Clone, Debug)]
enum CallableLoweringState {
    Visiting,
    Lowered(ClassificationCallableId),
    Residual(LoweringFailure),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct RuntimeShapeKey {
    owner_id: [u8; 32],
    variant_ordinal: u32,
}

#[derive(Clone, Copy, Debug)]
struct RegisteredRuntimeShape {
    key: RuntimeShapeKey,
    constructor_id: [u8; 32],
}

struct CheckedClassificationProducer<'program, 'query> {
    query: &'query crate::explore::ExploreQueryIr,
    sites: &'query CheckedExploreQuerySites,
    question_id: crate::explore::QuestionId,
    index: CheckedExploreSemanticIndex<'program>,
    resolutions: &'program CheckedResolutionArtifacts,
    semantic_binders: BTreeMap<CheckedBinderSiteId, Box<str>>,
    interner: ClassificationInterner,
    expected_lanes: Vec<ClassificationSemanticLane>,
    roots: Vec<ClassificationLaneRoot>,
    residuals: Vec<ClassificationResidual>,
    callable_states: BTreeMap<CheckedCallableId, CallableLoweringState>,
    callable_definitions: BTreeMap<ClassificationCallableId, ClassificationCallableDefinition>,
    runtime_shapes: BTreeMap<RuntimeShapeKey, CheckedConstructorIdentity>,
    deferred_binder_types: BTreeMap<CheckedBinderSiteId, ClassificationTypeId>,
}

impl<'program, 'query> CheckedClassificationProducer<'program, 'query> {
    fn produce(
        mut self,
    ) -> Result<CheckedExploreClassification, CheckedExploreClassificationError> {
        let mut source_environment = BinderEnvironment::new();
        for ordinal in 0..self.query.source.bindings.len() {
            let ordinal_u32 = u32::try_from(ordinal).map_err(|_| {
                CheckedExploreClassificationError::CheckedBoundary(
                    "classification source binding ordinal exceeds the V1 graph ABI".into(),
                )
            })?;
            let binding = self.query.source.bindings[ordinal].clone();
            let checked_sites = self.sites.source_bindings[ordinal].clone();
            let parameter =
                self.source_parameter(ordinal_u32, &binding.value_ty, &checked_sites.expression)?;
            let residual_identity = match &binding.kind {
                ExploreSourceBindingKindIr::Singleton { .. } => {
                    ResidualIdentityRoot::Expression(checked_sites.expression.clone())
                }
                ExploreSourceBindingKindIr::Finite { .. } => {
                    ResidualIdentityRoot::Type(binding.value_ty.clone())
                }
            };
            let outcome = match &binding.kind {
                ExploreSourceBindingKindIr::Singleton { .. } => {
                    self.lower_expression(&checked_sites.expression, &source_environment)
                }
                ExploreSourceBindingKindIr::Finite { .. } => self.binder_value_result(
                    parameter,
                    &checked_sites.expression,
                    &binding.value_ty,
                ),
            };
            self.record_lane(
                ClassificationSemanticLane::SourceBinding(ordinal_u32),
                residual_identity,
                outcome,
            )?;

            // Later source constructors consume the exact already-enumerated
            // member, not a duplicated copy of the earlier construction DAG.
            source_environment.insert(checked_sites.binder.clone(), parameter);
        }

        let successor_environment = self.endpoint_environment(true, false)?;
        let successor_kind = self.query.successor.kind.clone();
        let successor_site = self.sites.successor.clone();
        let successor_ty = self.query.successor.after_ty.clone();
        let successor_residual_identity = match &successor_kind {
            ExploreSuccessorKindIr::Singleton { .. } => {
                ResidualIdentityRoot::Expression(successor_site.clone())
            }
            ExploreSuccessorKindIr::Finite { .. } => {
                ResidualIdentityRoot::Type(successor_ty.clone())
            }
        };
        let successor = match successor_kind {
            ExploreSuccessorKindIr::Singleton { .. } => {
                self.lower_expression(&successor_site, &successor_environment)
            }
            ExploreSuccessorKindIr::Finite { .. } => {
                let after = self.endpoint_input(ClassificationInputSlot::AFTER, &successor_ty)?;
                self.binder_value_result(after, &successor_site, &successor_ty)
            }
        };
        self.record_lane(
            ClassificationSemanticLane::Successor,
            successor_residual_identity,
            successor,
        )?;

        for ordinal in 0..self.query.admissions.len() {
            let ordinal_u32 = u32::try_from(ordinal).map_err(|_| {
                CheckedExploreClassificationError::CheckedBoundary(
                    "classification admission ordinal exceeds the V1 graph ABI".into(),
                )
            })?;
            let admission = self.query.admissions[ordinal].clone();
            let admission_site = self.sites.admissions[ordinal].clone();
            let lane = ClassificationSemanticLane::Admission {
                ordinal: ordinal_u32,
                scope: match admission.scope {
                    ExploreAdmissionScope::Before => ClassificationAdmissionScope::Before,
                    ExploreAdmissionScope::After => ClassificationAdmissionScope::After,
                    ExploreAdmissionScope::Transition => ClassificationAdmissionScope::Transition,
                },
            };
            let admission_environment = match admission.scope {
                ExploreAdmissionScope::Before => self.endpoint_environment(true, false)?,
                ExploreAdmissionScope::After => self.endpoint_environment(false, true)?,
                ExploreAdmissionScope::Transition => self.endpoint_environment(true, true)?,
            };
            let outcome = self
                .lower_expression(&admission_site, &admission_environment)
                .and_then(|value| {
                    if value.scalar == Some(ScalarKind::Boolean) {
                        Ok(value)
                    } else {
                        Err(self.residual_error(
                            &admission_site,
                            ClassificationResidualReason::UnsupportedType,
                            [],
                        ))
                    }
                });
            self.record_lane(
                lane,
                ResidualIdentityRoot::Expression(admission_site),
                outcome,
            )?;
        }

        let find_environment = self.endpoint_environment(true, true)?;
        let find_kind = self.query.find.clone();
        let selection_site = self.sites.selection.clone();
        let find_residual_identity = match &find_kind {
            ExploreFindIr::All { .. } => {
                ResidualIdentityRoot::Synthetic(self.synthetic_find_site_digest())
            }
            ExploreFindIr::Matches { .. } | ExploreFindIr::Violations { .. } => {
                ResidualIdentityRoot::Expression(
                    selection_site
                        .as_ref()
                        .expect("validated checked FIND predicate site")
                        .clone(),
                )
            }
        };
        let find = match find_kind {
            ExploreFindIr::All { .. } => self.lower_find_all(),
            ExploreFindIr::Matches { .. } => self
                .lower_expression(
                    selection_site
                        .as_ref()
                        .expect("validated checked FIND predicate site"),
                    &find_environment,
                )
                .and_then(|value| {
                    if value.scalar == Some(ScalarKind::Boolean) {
                        Ok(value)
                    } else {
                        Err(self.residual_error(
                            selection_site
                                .as_ref()
                                .expect("validated checked FIND predicate site"),
                            ClassificationResidualReason::UnsupportedType,
                            [],
                        ))
                    }
                }),
            ExploreFindIr::Violations { .. } => {
                let selection_site = selection_site
                    .as_ref()
                    .expect("validated checked FIND predicate site");
                self.lower_expression(selection_site, &find_environment)
                    .and_then(|predicate| {
                        if predicate.scalar != Some(ScalarKind::Boolean) {
                            return Err(self.residual_error(
                                selection_site,
                                ClassificationResidualReason::UnsupportedType,
                                [],
                            ));
                        }
                        self.intern(
                            predicate.ty,
                            Some(ScalarKind::Boolean),
                            ClassificationNodeKind::Unary {
                                op: ClassificationUnaryOp::BooleanNot,
                                operand: predicate.node,
                            },
                        )
                    })
            }
        };
        self.record_lane(
            ClassificationSemanticLane::Find,
            find_residual_identity,
            find,
        )?;

        let program = FrozenClassificationProgram::freeze_with_callables(
            std::mem::take(&mut self.interner),
            std::mem::take(&mut self.callable_definitions).into_values(),
            std::mem::take(&mut self.expected_lanes),
            std::mem::take(&mut self.roots),
            std::mem::take(&mut self.residuals),
        )
        .map_err(CheckedExploreClassificationError::Capsule)?;
        let runtime_shapes = self.freeze_reachable_runtime_shapes(&program)?;
        runtime_shapes
            .validate_for_program(&program)
            .map_err(CheckedExploreClassificationError::Capsule)?;
        Ok(CheckedExploreClassification {
            program,
            runtime_shapes,
        })
    }

    fn freeze_reachable_runtime_shapes(
        &self,
        program: &FrozenClassificationProgram,
    ) -> Result<FrozenClassificationRuntimeShapes, CheckedExploreClassificationError> {
        let mut semantic_keys = BTreeSet::new();
        let mut constructor_ids = BTreeSet::new();
        for (_, node) in program.nodes() {
            match &node.kind {
                ClassificationNodeKind::Construct { constructor_id, .. } => {
                    constructor_ids.insert(*constructor_id);
                }
                ClassificationNodeKind::Project {
                    owner_id,
                    variant_ordinal,
                    ..
                }
                | ClassificationNodeKind::IsVariant {
                    owner_id,
                    variant_ordinal,
                    ..
                } => {
                    semantic_keys.insert(RuntimeShapeKey {
                        owner_id: *owner_id,
                        variant_ordinal: *variant_ordinal,
                    });
                }
                ClassificationNodeKind::Constant(_)
                | ClassificationNodeKind::Input(_)
                | ClassificationNodeKind::SourceParameter(_)
                | ClassificationNodeKind::CallableParameter { .. }
                | ClassificationNodeKind::Unary { .. }
                | ClassificationNodeKind::Binary { .. }
                | ClassificationNodeKind::If { .. }
                | ClassificationNodeKind::Call { .. } => {}
            }
        }

        let shapes = self.runtime_shapes.iter().filter_map(|(key, identity)| {
            let constructor_id = checked_explore_projection_constructor_digest(identity);
            (semantic_keys.contains(key) || constructor_ids.contains(&constructor_id)).then(|| {
                RuntimeConstructorShape::new(
                    key.owner_id,
                    key.variant_ordinal,
                    constructor_id,
                    identity.owner_type.clone(),
                    identity.variant.clone(),
                    match identity.layout {
                        CheckedConstructorLayout::Positional => {
                            ClassificationRuntimeLayout::Positional
                        }
                        CheckedConstructorLayout::Named => ClassificationRuntimeLayout::Named,
                    },
                    identity
                        .fields
                        .iter()
                        .map(|field| field.name.clone())
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                )
            })
        });
        FrozenClassificationRuntimeShapes::freeze(shapes)
            .map_err(CheckedExploreClassificationError::Capsule)
    }

    fn record_lane(
        &mut self,
        lane: ClassificationSemanticLane,
        residual_identity: ResidualIdentityRoot,
        outcome: LoweringResult,
    ) -> Result<(), CheckedExploreClassificationError> {
        self.expected_lanes.push(lane);
        match outcome {
            Ok(value) => self.roots.push(ClassificationLaneRoot {
                lane,
                node: value.node,
            }),
            Err(LoweringError::Residual(failure)) => {
                let site_digest = self.residual_identity_digest(&residual_identity)?;
                self.residuals.push(ClassificationResidual::new(
                    failure.reason,
                    lane,
                    site_digest,
                    failure.dependencies,
                ));
            }
            Err(LoweringError::Capsule(error)) => {
                return Err(CheckedExploreClassificationError::Capsule(error));
            }
        }
        Ok(())
    }

    fn endpoint_environment(
        &mut self,
        include_before: bool,
        include_after: bool,
    ) -> Result<BinderEnvironment, CheckedExploreClassificationError> {
        let context_ty = self.query.source.context_ty.clone();
        let before_ty = self.query.source.before_ty.clone();
        let after_ty = self.query.successor.after_ty.clone();
        let context = self.endpoint_input(ClassificationInputSlot::CONTEXT, &context_ty)?;
        let before = if include_before {
            Some(self.endpoint_input(ClassificationInputSlot::BEFORE, &before_ty)?)
        } else {
            None
        };
        let after = if include_after {
            Some(self.endpoint_input(ClassificationInputSlot::AFTER, &after_ty)?)
        } else {
            None
        };
        let mut environment = BinderEnvironment::new();
        for ordinal in 0..self.query.source.bindings.len() {
            let binding = &self.query.source.bindings[ordinal];
            let binder = self.sites.source_bindings[ordinal].binder.clone();
            let value = match binding.role {
                ExploreSourceBindingRoleIr::Context => Some(context),
                ExploreSourceBindingRoleIr::Before => before,
                ExploreSourceBindingRoleIr::Auxiliary => None,
            };
            if let Some(value) = value {
                environment.insert(binder, value);
            }
        }

        if let Some(after) = after {
            let successor = &self.sites.successor;
            let after_binder = CheckedBinderSiteId::Structural {
                analysis_program: successor.analysis_program.clone(),
                declaration: successor.declaration.clone(),
                normalized_declaration_ordinal: successor.normalized_declaration_ordinal,
                ast_path: successor.ast_path.clone(),
                binder_path: vec![CheckedResolutionRecorder::BINDER_EXPLORE_ROLE, 2]
                    .into_boxed_slice(),
            };
            environment.insert(after_binder, after);
        }
        Ok(environment)
    }

    fn source_parameter(
        &mut self,
        ordinal: u32,
        ty: &Ty,
        _site: &ExprSiteId,
    ) -> Result<BinderValue, CheckedExploreClassificationError> {
        let Some((ty, scalar)) = self.classification_type(ty) else {
            return Ok(BinderValue::UnsupportedType);
        };
        let node = self
            .interner
            .intern(ClassificationNodeKey {
                ty,
                kind: ClassificationNodeKind::SourceParameter(ordinal),
            })
            .map_err(CheckedExploreClassificationError::Capsule)?;
        Ok(BinderValue::Lowered(LoweredValue { node, ty, scalar }))
    }

    fn endpoint_input(
        &mut self,
        slot: ClassificationInputSlot,
        ty: &Ty,
    ) -> Result<BinderValue, CheckedExploreClassificationError> {
        let Some((ty, scalar)) = self.classification_type(ty) else {
            return Ok(BinderValue::UnsupportedType);
        };
        let node = self
            .interner
            .intern(ClassificationNodeKey {
                ty,
                kind: ClassificationNodeKind::Input(slot),
            })
            .map_err(CheckedExploreClassificationError::Capsule)?;
        Ok(BinderValue::Lowered(LoweredValue { node, ty, scalar }))
    }

    fn binder_value_result(
        &self,
        value: BinderValue,
        site: &ExprSiteId,
        ty: &Ty,
    ) -> LoweringResult {
        match value {
            BinderValue::Lowered(value) => Ok(value),
            BinderValue::DeferredProjection(projection) => Err(self.residual_error(
                site,
                ClassificationResidualReason::UnsupportedType,
                [ClassificationResidualDependency::Field(
                    projection.dependency,
                )],
            )),
            BinderValue::UnsupportedType => Err(self.residual_error(
                site,
                ClassificationResidualReason::UnsupportedType,
                self.type_dependencies(ty),
            )),
        }
    }

    fn lower_find_all(&mut self) -> LoweringResult {
        let bool_ty = Ty::Name("Bool".to_string());
        let Some((ty, scalar @ Some(ScalarKind::Boolean))) = self.classification_type(&bool_ty)
        else {
            return Err(LoweringError::Residual(LoweringFailure {
                reason: ClassificationResidualReason::UnsupportedType,
                dependencies: self.type_dependencies(&bool_ty),
            }));
        };
        self.intern(
            ty,
            scalar,
            ClassificationNodeKind::Constant(ClassificationConstant::Boolean(true)),
        )
    }

    fn lower_expression(
        &mut self,
        site: &ExprSiteId,
        environment: &BinderEnvironment,
    ) -> LoweringResult {
        let expression = self.index.expression(site).cloned().ok_or_else(|| {
            self.residual_error(
                site,
                ClassificationResidualReason::UnsupportedExpression,
                [],
            )
        })?;
        let resolution = self
            .resolutions
            .expressions
            .get(site)
            .cloned()
            .ok_or_else(|| {
                self.residual_error(site, ClassificationResidualReason::UnsupportedType, [])
            })?;
        if self.resolutions.unsupported_sites.contains_key(site) {
            return Err(self.residual_error(
                site,
                ClassificationResidualReason::UnsupportedExpression,
                self.resolution_dependencies(&resolution),
            ));
        }

        match &expression.kind {
            ExprKind::Lambda(_, _) => {
                return Err(self.residual_error(
                    site,
                    ClassificationResidualReason::HigherOrderCall,
                    self.resolution_dependencies(&resolution),
                ));
            }
            ExprKind::List(_) | ExprKind::Tuple(_) | ExprKind::Index(_, _) => {
                return Err(self.residual_error(
                    site,
                    ClassificationResidualReason::CollectionTraversal,
                    self.resolution_dependencies(&resolution),
                ));
            }
            ExprKind::Effect(_, _) | ExprKind::Handle { .. } | ExprKind::Try(_) => {
                return Err(self.residual_error(
                    site,
                    ClassificationResidualReason::EffectfulExpression,
                    self.resolution_dependencies(&resolution),
                ));
            }
            ExprKind::Pipe(_, _) => {
                return Err(self.residual_error(
                    site,
                    ClassificationResidualReason::DynamicDispatch,
                    self.resolution_dependencies(&resolution),
                ));
            }
            ExprKind::Conjunction(_) | ExprKind::Disjunction(_) => {
                return Err(self.residual_error(
                    site,
                    ClassificationResidualReason::UnsupportedExpression,
                    self.resolution_dependencies(&resolution),
                ));
            }
            _ => {}
        }

        let resolved_ty = match &resolution.resolved_type {
            CheckedExpressionType::Resolved(ty) => ty,
            CheckedExpressionType::Callable { .. } | CheckedExpressionType::CallableReference => {
                return Err(self.residual_error(
                    site,
                    ClassificationResidualReason::HigherOrderCall,
                    self.resolution_dependencies(&resolution),
                ));
            }
            CheckedExpressionType::PolymorphicEmptyList => {
                return Err(self.residual_error(
                    site,
                    ClassificationResidualReason::CollectionTraversal,
                    self.resolution_dependencies(&resolution),
                ));
            }
            CheckedExpressionType::Unsupported => {
                return Err(self.residual_error(
                    site,
                    ClassificationResidualReason::UnsupportedType,
                    self.resolution_dependencies(&resolution),
                ));
            }
        };
        let Some((ty, scalar)) = self.classification_type(resolved_ty) else {
            return Err(self.residual_error(
                site,
                ClassificationResidualReason::UnsupportedType,
                self.type_dependencies(resolved_ty),
            ));
        };

        match expression.kind {
            ExprKind::Var(_) => self.lower_variable(site, &resolution, ty, scalar, environment),
            ExprKind::Lit(literal) => self.lower_literal(site, literal, ty, scalar),
            ExprKind::App(_, arguments) => {
                self.lower_application(site, &arguments, &resolution, ty, scalar, environment)
            }
            ExprKind::BinOp(operator, _, _) => {
                self.lower_binary(site, &operator, ty, scalar, environment)
            }
            ExprKind::UnOp(operator, _) => {
                self.lower_unary(site, &operator, ty, scalar, environment)
            }
            ExprKind::If(_, _, _) => self.lower_if(site, ty, scalar, environment),
            ExprKind::Match(_, arms) => self.lower_match(site, &arms, ty, scalar, environment),
            ExprKind::Field(_, _) => self.lower_field(site, &resolution, ty, scalar, environment),
            ExprKind::Block(statements) => {
                if matches!(statements.as_slice(), [Stmt::Expr(_)]) {
                    let inner = child_site(&child_site(site, 0), 0);
                    self.lower_expression(&inner, environment)
                        .and_then(|value| {
                            if value.ty == ty {
                                Ok(value)
                            } else {
                                Err(self.residual_error(
                                    site,
                                    ClassificationResidualReason::UnsupportedType,
                                    [ClassificationResidualDependency::Node(value.node)],
                                ))
                            }
                        })
                } else {
                    Err(self.residual_error(
                        site,
                        ClassificationResidualReason::UnsupportedExpression,
                        self.resolution_dependencies(&resolution),
                    ))
                }
            }
            ExprKind::Unit => {
                if scalar == Some(ScalarKind::Unit) {
                    self.intern(
                        ty,
                        scalar,
                        ClassificationNodeKind::Constant(ClassificationConstant::Unit),
                    )
                } else {
                    Err(self.residual_error(
                        site,
                        ClassificationResidualReason::UnsupportedType,
                        [],
                    ))
                }
            }
            ExprKind::Lambda(_, _)
            | ExprKind::Index(_, _)
            | ExprKind::List(_)
            | ExprKind::Tuple(_)
            | ExprKind::Effect(_, _)
            | ExprKind::Handle { .. }
            | ExprKind::Try(_)
            | ExprKind::Conjunction(_)
            | ExprKind::Disjunction(_)
            | ExprKind::Pipe(_, _) => unreachable!("unsupported expression returned above"),
        }
    }

    fn lower_variable(
        &mut self,
        site: &ExprSiteId,
        resolution: &CheckedExpressionResolution,
        ty: ClassificationTypeId,
        scalar: Option<ScalarKind>,
        environment: &BinderEnvironment,
    ) -> LoweringResult {
        match resolution.value_binding.as_ref() {
            Some(CheckedValueBinding::Binder { site: binder, .. }) => {
                match environment.get(binder).copied() {
                    Some(BinderValue::Lowered(value)) if value.ty == ty => Ok(value),
                    Some(BinderValue::Lowered(value)) => Err(self.residual_error(
                        site,
                        ClassificationResidualReason::UnsupportedType,
                        [ClassificationResidualDependency::Node(value.node)],
                    )),
                    Some(BinderValue::DeferredProjection(projection)) => {
                        if let Some(previous) = self.deferred_binder_types.get(binder) {
                            if *previous != ty {
                                return Err(self.residual_error(
                                    site,
                                    ClassificationResidualReason::UnsupportedType,
                                    [ClassificationResidualDependency::Field(
                                        projection.dependency,
                                    )],
                                ));
                            }
                        } else {
                            self.deferred_binder_types.insert(binder.clone(), ty);
                        }
                        self.intern(
                            ty,
                            scalar,
                            ClassificationNodeKind::Project {
                                owner_id: projection.owner_id,
                                variant_ordinal: projection.variant_ordinal,
                                field_ordinal: projection.field_ordinal,
                                base: projection.base,
                            },
                        )
                    }
                    Some(BinderValue::UnsupportedType) => Err(self.residual_error(
                        site,
                        ClassificationResidualReason::UnsupportedType,
                        [ClassificationResidualDependency::Capture(
                            checked_explore_projection_binder_digest(binder),
                        )],
                    )),
                    None => Err(self.residual_error(
                        site,
                        ClassificationResidualReason::OpenCapture,
                        [ClassificationResidualDependency::Capture(
                            checked_explore_projection_binder_digest(binder),
                        )],
                    )),
                }
            }
            Some(CheckedValueBinding::TopLevel(binding)) => {
                let mut dependencies = Vec::new();
                if let Some(digest) = self.semantic_dependency_digest(
                    CheckedExploreSemanticDependency::TopLevel(binding.clone()),
                ) {
                    dependencies.push(ClassificationResidualDependency::TopLevelConstant(digest));
                }
                Err(self.residual_error(
                    site,
                    ClassificationResidualReason::OpenCapture,
                    dependencies,
                ))
            }
            Some(CheckedValueBinding::Callable(callable)) => {
                let dependencies = self
                    .classification_callable_id(callable)
                    .map(ClassificationResidualDependency::Callable)
                    .into_iter();
                Err(self.residual_error(
                    site,
                    ClassificationResidualReason::HigherOrderCall,
                    dependencies,
                ))
            }
            Some(CheckedValueBinding::RuleFamily(family)) => {
                let mut dependencies = Vec::new();
                if let Some(digest) = self.semantic_dependency_digest(
                    CheckedExploreSemanticDependency::RuleFamily(family.clone()),
                ) {
                    dependencies.push(ClassificationResidualDependency::RuleFamily(digest));
                }
                Err(self.residual_error(
                    site,
                    ClassificationResidualReason::DynamicDispatch,
                    dependencies,
                ))
            }
            Some(CheckedValueBinding::Constructor {
                owner_type,
                variant,
                variant_index,
                ..
            }) => {
                let Some(constructor) = resolution.exact_constructor.as_ref() else {
                    return Err(self.residual_error(
                        site,
                        ClassificationResidualReason::UnsupportedExpression,
                        self.resolution_dependencies(resolution),
                    ));
                };
                if !legacy_constructor_metadata_matches(
                    owner_type,
                    variant,
                    *variant_index,
                    constructor,
                ) {
                    return Err(self.residual_error(
                        site,
                        ClassificationResidualReason::UnsupportedExpression,
                        self.resolution_dependencies(resolution),
                    ));
                }
                if !constructor.fields.is_empty() {
                    return Err(self.residual_error(
                        site,
                        ClassificationResidualReason::UnsupportedExpression,
                        [ClassificationResidualDependency::Constructor(
                            checked_explore_projection_constructor_digest(constructor),
                        )],
                    ));
                }
                if scalar == Some(ScalarKind::Boolean)
                    && matches!(
                        &constructor.owner,
                        CheckedDataTypeId::Intrinsic { canonical_name }
                            if canonical_name.as_ref() == "Bool"
                    )
                {
                    let value = match constructor.variant.as_ref() {
                        "True" => true,
                        "False" => false,
                        _ => {
                            return Err(self.residual_error(
                                site,
                                ClassificationResidualReason::UnsupportedExpression,
                                [ClassificationResidualDependency::Constructor(
                                    checked_explore_projection_constructor_digest(constructor),
                                )],
                            ));
                        }
                    };
                    return self.intern(
                        ty,
                        scalar,
                        ClassificationNodeKind::Constant(ClassificationConstant::Boolean(value)),
                    );
                }
                let shape = self.register_runtime_shape(site, constructor)?;
                self.intern(
                    ty,
                    scalar,
                    ClassificationNodeKind::Construct {
                        constructor_id: shape.constructor_id,
                        fields: Box::new([]),
                    },
                )
            }
            Some(CheckedValueBinding::OpaqueQualifiedOwner(_)) => Err(self.residual_error(
                site,
                ClassificationResidualReason::DynamicDispatch,
                self.resolution_dependencies(resolution),
            )),
            None => Err(self.residual_error(
                site,
                ClassificationResidualReason::UnresolvedMember,
                self.resolution_dependencies(resolution),
            )),
        }
    }

    fn lower_literal(
        &mut self,
        site: &ExprSiteId,
        literal: Literal,
        ty: ClassificationTypeId,
        scalar: Option<ScalarKind>,
    ) -> LoweringResult {
        let constant = match (literal, scalar) {
            (Literal::Int(value), Some(ScalarKind::Integer)) => {
                ClassificationConstant::Integer(value)
            }
            (Literal::Bool(value), Some(ScalarKind::Boolean)) => {
                ClassificationConstant::Boolean(value)
            }
            (Literal::Str(value), Some(ScalarKind::String)) => {
                ClassificationConstant::String(value.into_boxed_str())
            }
            (Literal::Char(value), Some(ScalarKind::Character)) => {
                ClassificationConstant::Character(value)
            }
            (Literal::Float(_), Some(ScalarKind::Float)) => {
                return Err(self.residual_error(
                    site,
                    ClassificationResidualReason::UnsupportedType,
                    [],
                ));
            }
            _ => {
                return Err(self.residual_error(
                    site,
                    ClassificationResidualReason::UnsupportedType,
                    [],
                ));
            }
        };
        self.intern(ty, scalar, ClassificationNodeKind::Constant(constant))
    }

    fn lower_unary(
        &mut self,
        site: &ExprSiteId,
        operator: &str,
        ty: ClassificationTypeId,
        scalar: Option<ScalarKind>,
        environment: &BinderEnvironment,
    ) -> LoweringResult {
        let operand_site = child_site(site, 0);
        let operand = self.lower_expression(&operand_site, environment)?;
        match (operator, operand.scalar, scalar) {
            ("!", Some(ScalarKind::Boolean), Some(ScalarKind::Boolean)) => self.intern(
                ty,
                scalar,
                ClassificationNodeKind::Unary {
                    op: ClassificationUnaryOp::BooleanNot,
                    operand: operand.node,
                },
            ),
            ("-", Some(ScalarKind::Integer), Some(ScalarKind::Integer)) => self.intern(
                ty,
                scalar,
                ClassificationNodeKind::Unary {
                    op: ClassificationUnaryOp::IntegerNegateChecked,
                    operand: operand.node,
                },
            ),
            ("+", Some(ScalarKind::Integer), Some(ScalarKind::Integer)) if operand.ty == ty => {
                Ok(operand)
            }
            ("+" | "-", Some(ScalarKind::Float), Some(ScalarKind::Float)) => Err(self
                .residual_error(
                    site,
                    ClassificationResidualReason::UncertainArithmetic,
                    [ClassificationResidualDependency::Node(operand.node)],
                )),
            _ => Err(self.residual_error(
                site,
                ClassificationResidualReason::UnsupportedExpression,
                [ClassificationResidualDependency::Node(operand.node)],
            )),
        }
    }

    fn lower_binary(
        &mut self,
        site: &ExprSiteId,
        operator: &str,
        ty: ClassificationTypeId,
        scalar: Option<ScalarKind>,
        environment: &BinderEnvironment,
    ) -> LoweringResult {
        let left = self.lower_expression(&child_site(site, 0), environment)?;
        let right = self.lower_expression(&child_site(site, 1), environment)?;
        let op = match (operator, left.scalar, right.scalar, scalar) {
            (
                "+",
                Some(ScalarKind::Integer),
                Some(ScalarKind::Integer),
                Some(ScalarKind::Integer),
            ) => ClassificationBinaryOp::IntegerAddChecked,
            (
                "-",
                Some(ScalarKind::Integer),
                Some(ScalarKind::Integer),
                Some(ScalarKind::Integer),
            ) => ClassificationBinaryOp::IntegerSubtractChecked,
            (
                "*",
                Some(ScalarKind::Integer),
                Some(ScalarKind::Integer),
                Some(ScalarKind::Integer),
            ) => ClassificationBinaryOp::IntegerMultiplyChecked,
            (
                "/",
                Some(ScalarKind::Integer),
                Some(ScalarKind::Integer),
                Some(ScalarKind::Integer),
            ) => ClassificationBinaryOp::IntegerDivideChecked,
            (
                "%",
                Some(ScalarKind::Integer),
                Some(ScalarKind::Integer),
                Some(ScalarKind::Integer),
            ) => ClassificationBinaryOp::IntegerRemainderChecked,
            (
                "==",
                Some(ScalarKind::Integer),
                Some(ScalarKind::Integer),
                Some(ScalarKind::Boolean),
            )
            | (
                "==",
                Some(ScalarKind::Boolean),
                Some(ScalarKind::Boolean),
                Some(ScalarKind::Boolean),
            ) => ClassificationBinaryOp::Equal,
            (
                "!=",
                Some(ScalarKind::Integer),
                Some(ScalarKind::Integer),
                Some(ScalarKind::Boolean),
            )
            | (
                "!=",
                Some(ScalarKind::Boolean),
                Some(ScalarKind::Boolean),
                Some(ScalarKind::Boolean),
            ) => ClassificationBinaryOp::NotEqual,
            (
                "<",
                Some(ScalarKind::Integer),
                Some(ScalarKind::Integer),
                Some(ScalarKind::Boolean),
            ) => ClassificationBinaryOp::LessThan,
            (
                "<=",
                Some(ScalarKind::Integer),
                Some(ScalarKind::Integer),
                Some(ScalarKind::Boolean),
            ) => ClassificationBinaryOp::LessThanOrEqual,
            (
                ">",
                Some(ScalarKind::Integer),
                Some(ScalarKind::Integer),
                Some(ScalarKind::Boolean),
            ) => ClassificationBinaryOp::GreaterThan,
            (
                ">=",
                Some(ScalarKind::Integer),
                Some(ScalarKind::Integer),
                Some(ScalarKind::Boolean),
            ) => ClassificationBinaryOp::GreaterThanOrEqual,
            (
                "&&",
                Some(ScalarKind::Boolean),
                Some(ScalarKind::Boolean),
                Some(ScalarKind::Boolean),
            ) => ClassificationBinaryOp::BooleanAndShortCircuit,
            (
                "||",
                Some(ScalarKind::Boolean),
                Some(ScalarKind::Boolean),
                Some(ScalarKind::Boolean),
            ) => ClassificationBinaryOp::BooleanOrShortCircuit,
            (_, Some(ScalarKind::Float), _, _) | (_, _, Some(ScalarKind::Float), _) => {
                return Err(self.residual_error(
                    site,
                    ClassificationResidualReason::UncertainArithmetic,
                    [
                        ClassificationResidualDependency::Node(left.node),
                        ClassificationResidualDependency::Node(right.node),
                    ],
                ));
            }
            _ => {
                return Err(self.residual_error(
                    site,
                    ClassificationResidualReason::UnsupportedExpression,
                    [
                        ClassificationResidualDependency::Node(left.node),
                        ClassificationResidualDependency::Node(right.node),
                    ],
                ));
            }
        };
        self.intern(
            ty,
            scalar,
            ClassificationNodeKind::Binary {
                op,
                left: left.node,
                right: right.node,
            },
        )
    }

    fn lower_if(
        &mut self,
        site: &ExprSiteId,
        ty: ClassificationTypeId,
        scalar: Option<ScalarKind>,
        environment: &BinderEnvironment,
    ) -> LoweringResult {
        let condition = self.lower_expression(&child_site(site, 0), environment)?;
        let then_node = self.lower_expression(&child_site(site, 1), environment)?;
        let else_node = self.lower_expression(&child_site(site, 2), environment)?;
        if condition.scalar != Some(ScalarKind::Boolean) || then_node.ty != ty || else_node.ty != ty
        {
            return Err(self.residual_error(
                site,
                ClassificationResidualReason::UnsupportedType,
                [
                    ClassificationResidualDependency::Node(condition.node),
                    ClassificationResidualDependency::Node(then_node.node),
                    ClassificationResidualDependency::Node(else_node.node),
                ],
            ));
        }
        self.intern(
            ty,
            scalar,
            ClassificationNodeKind::If {
                condition: condition.node,
                then_node: then_node.node,
                else_node: else_node.node,
            },
        )
    }

    fn lower_field(
        &mut self,
        site: &ExprSiteId,
        resolution: &CheckedExpressionResolution,
        ty: ClassificationTypeId,
        scalar: Option<ScalarKind>,
        environment: &BinderEnvironment,
    ) -> LoweringResult {
        let Some(CheckedFieldResolution::Data {
            owner_type: _,
            fields,
        }) = resolution.field.as_ref()
        else {
            return Err(self.residual_error(
                site,
                ClassificationResidualReason::UnresolvedMember,
                self.resolution_dependencies(resolution),
            ));
        };
        let [field] = fields.as_ref() else {
            return Err(self.residual_error(
                site,
                ClassificationResidualReason::UnresolvedMember,
                fields.iter().map(|field| {
                    ClassificationResidualDependency::Field(field_dependency_digest(
                        &field.identity,
                    ))
                }),
            ));
        };
        let Some(constructor) = self.exact_constructor_for_field(field) else {
            return Err(self.residual_error(
                site,
                ClassificationResidualReason::UnresolvedMember,
                [ClassificationResidualDependency::Field(
                    field_dependency_digest(&field.identity),
                )],
            ));
        };
        let shape = self.register_runtime_shape(site, &constructor)?;
        let field_ordinal = u32::try_from(field.field_index).map_err(|_| {
            self.residual_error(
                site,
                ClassificationResidualReason::UnsupportedType,
                [ClassificationResidualDependency::Field(
                    field_dependency_digest(&field.identity),
                )],
            )
        })?;
        let base = self.lower_expression(&child_site(site, 0), environment)?;
        self.intern(
            ty,
            scalar,
            ClassificationNodeKind::Project {
                owner_id: shape.key.owner_id,
                variant_ordinal: shape.key.variant_ordinal,
                field_ordinal,
                base: base.node,
            },
        )
    }

    fn lower_match(
        &mut self,
        site: &ExprSiteId,
        arms: &[MatchArm],
        ty: ClassificationTypeId,
        scalar: Option<ScalarKind>,
        environment: &BinderEnvironment,
    ) -> LoweringResult {
        let Some((last_index, last_arm)) =
            arms.len().checked_sub(1).map(|index| (index, &arms[index]))
        else {
            return Err(self.residual_error(
                site,
                ClassificationResidualReason::MatchNormalizationRequired,
                [],
            ));
        };
        if last_arm.guard.is_some() || !pattern_is_irrefutable(&last_arm.pat) {
            return Err(self.residual_error(
                site,
                ClassificationResidualReason::MatchNormalizationRequired,
                [],
            ));
        }

        let scrutinee_site = child_site(site, 0);
        let allow_bare_fielded_tag = self
            .index
            .expression(&scrutinee_site)
            .is_some_and(|expression| matches!(&expression.kind, ExprKind::Var(_)))
            && arms.iter().all(|arm| pattern_is_tag_only(&arm.pat));
        let scrutinee = self.lower_expression(&scrutinee_site, environment)?;
        let arm_sites = match_arm_sites(site, arms);

        let mut fallback_environment = environment.clone();
        let fallback_test = self.lower_pattern(
            site,
            &last_arm.pat,
            vec![u32::try_from(last_index).map_err(|_| {
                self.residual_error(
                    site,
                    ClassificationResidualReason::MatchNormalizationRequired,
                    [],
                )
            })?],
            scrutinee,
            &mut fallback_environment,
            allow_bare_fielded_tag,
        )?;
        if fallback_test.scalar != Some(ScalarKind::Boolean) {
            return Err(self.residual_error(
                site,
                ClassificationResidualReason::MatchNormalizationRequired,
                [ClassificationResidualDependency::Node(fallback_test.node)],
            ));
        }
        let mut next = self.lower_expression(&arm_sites[last_index].1, &fallback_environment)?;
        if next.ty != ty {
            return Err(self.residual_error(
                &arm_sites[last_index].1,
                ClassificationResidualReason::UnsupportedType,
                [ClassificationResidualDependency::Node(next.node)],
            ));
        }

        for arm_index in (0..last_index).rev() {
            let arm = &arms[arm_index];
            let mut arm_environment = environment.clone();
            let mut condition = self.lower_pattern(
                site,
                &arm.pat,
                vec![u32::try_from(arm_index).map_err(|_| {
                    self.residual_error(
                        site,
                        ClassificationResidualReason::MatchNormalizationRequired,
                        [],
                    )
                })?],
                scrutinee,
                &mut arm_environment,
                allow_bare_fielded_tag,
            )?;
            if let Some(guard_site) = arm_sites[arm_index].0.as_ref() {
                let guard = self.lower_expression(guard_site, &arm_environment)?;
                if guard.scalar != Some(ScalarKind::Boolean) {
                    return Err(self.residual_error(
                        guard_site,
                        ClassificationResidualReason::UnsupportedType,
                        [ClassificationResidualDependency::Node(guard.node)],
                    ));
                }
                condition = self.boolean_and(site, condition, guard)?;
            }
            let body = self.lower_expression(&arm_sites[arm_index].1, &arm_environment)?;
            if condition.scalar != Some(ScalarKind::Boolean) || body.ty != ty || next.ty != ty {
                return Err(self.residual_error(
                    &arm_sites[arm_index].1,
                    ClassificationResidualReason::UnsupportedType,
                    [
                        ClassificationResidualDependency::Node(condition.node),
                        ClassificationResidualDependency::Node(body.node),
                        ClassificationResidualDependency::Node(next.node),
                    ],
                ));
            }
            next = self.intern(
                ty,
                scalar,
                ClassificationNodeKind::If {
                    condition: condition.node,
                    then_node: body.node,
                    else_node: next.node,
                },
            )?;
        }
        Ok(next)
    }

    fn lower_pattern(
        &mut self,
        match_site: &ExprSiteId,
        pattern: &Pat,
        pattern_path: Vec<u32>,
        value: LoweredValue,
        environment: &mut BinderEnvironment,
        allow_bare_fielded_tag: bool,
    ) -> LoweringResult {
        match pattern {
            Pat::Wild => self.boolean_constant(match_site, true),
            Pat::Var(_) => {
                environment.insert(
                    pattern_binder_site(match_site, &pattern_path, false),
                    BinderValue::Lowered(value),
                );
                self.boolean_constant(match_site, true)
            }
            Pat::Lit(literal) => self.lower_pattern_literal(match_site, literal, value),
            Pat::As(inner, _) => {
                environment.insert(
                    pattern_binder_site(match_site, &pattern_path, true),
                    BinderValue::Lowered(value),
                );
                self.lower_pattern(
                    match_site,
                    inner,
                    pattern_path,
                    value,
                    environment,
                    allow_bare_fielded_tag,
                )
            }
            Pat::Con(_, children) => {
                let children = children.iter().collect::<Vec<_>>();
                self.lower_constructor_pattern(
                    match_site,
                    pattern,
                    &children,
                    &pattern_path,
                    value,
                    environment,
                    allow_bare_fielded_tag,
                )
            }
            Pat::NamedCon(_, fields) => {
                let children = fields.iter().map(|(_, child)| child).collect::<Vec<_>>();
                self.lower_constructor_pattern(
                    match_site,
                    pattern,
                    &children,
                    &pattern_path,
                    value,
                    environment,
                    allow_bare_fielded_tag,
                )
            }
        }
    }

    fn lower_constructor_pattern(
        &mut self,
        match_site: &ExprSiteId,
        pattern: &Pat,
        children: &[&Pat],
        pattern_path: &[u32],
        value: LoweredValue,
        environment: &mut BinderEnvironment,
        allow_bare_fielded_tag: bool,
    ) -> LoweringResult {
        let pattern_site = checked_pattern_site(match_site, pattern_path);
        let Some(resolution) = self
            .resolutions
            .constructor_patterns
            .get(&pattern_site)
            .cloned()
        else {
            return Err(self.residual_error(
                match_site,
                ClassificationResidualReason::MatchNormalizationRequired,
                [],
            ));
        };
        if resolution.source_fields.len() != children.len()
            || matches!(pattern, Pat::NamedCon(_, _))
                && resolution.constructor.layout != CheckedConstructorLayout::Named
            || matches!(pattern, Pat::Con(_, children) if children.is_empty())
                && !resolution.constructor.fields.is_empty()
                && !allow_bare_fielded_tag
        {
            return Err(self.residual_error(
                match_site,
                ClassificationResidualReason::MatchNormalizationRequired,
                [ClassificationResidualDependency::Constructor(
                    checked_explore_projection_constructor_digest(&resolution.constructor),
                )],
            ));
        }

        if resolution.constructor.fields.is_empty()
            && matches!(
                &resolution.constructor.owner,
                CheckedDataTypeId::Intrinsic { canonical_name }
                    if canonical_name.as_ref() == "Bool"
            )
        {
            let literal = match resolution.constructor.variant.as_ref() {
                "True" => Literal::Bool(true),
                "False" => Literal::Bool(false),
                _ => {
                    return Err(self.residual_error(
                        match_site,
                        ClassificationResidualReason::MatchNormalizationRequired,
                        [ClassificationResidualDependency::Constructor(
                            checked_explore_projection_constructor_digest(&resolution.constructor),
                        )],
                    ));
                }
            };
            return self.lower_pattern_literal(match_site, &literal, value);
        }

        let shape = self.register_runtime_shape(match_site, &resolution.constructor)?;
        let (bool_ty, bool_scalar) = self.boolean_type(match_site)?;
        let mut condition = self.intern(
            bool_ty,
            bool_scalar,
            ClassificationNodeKind::IsVariant {
                owner_id: shape.key.owner_id,
                variant_ordinal: shape.key.variant_ordinal,
                base: value.node,
            },
        )?;

        for (source_index, (child, field)) in children
            .iter()
            .zip(resolution.source_fields.iter())
            .enumerate()
        {
            if field.owner != resolution.constructor.owner
                || field.variant_index != resolution.constructor.variant_index
                || resolution.constructor.fields.get(field.field_index) != Some(field)
            {
                return Err(self.residual_error(
                    match_site,
                    ClassificationResidualReason::MatchNormalizationRequired,
                    [ClassificationResidualDependency::Constructor(
                        shape.constructor_id,
                    )],
                ));
            }
            let mut child_path = pattern_path.to_vec();
            child_path.push(u32::try_from(source_index).map_err(|_| {
                self.residual_error(
                    match_site,
                    ClassificationResidualReason::MatchNormalizationRequired,
                    [ClassificationResidualDependency::Constructor(
                        shape.constructor_id,
                    )],
                )
            })?);
            if let Some(child_condition) = self.lower_one_level_field_pattern(
                match_site,
                child,
                &child_path,
                value,
                field,
                shape,
                environment,
            )? {
                // IsVariant remains the left-most condition, so no projection
                // can execute against the wrong runtime variant.
                condition = self.boolean_and(match_site, condition, child_condition)?;
            }
        }
        Ok(condition)
    }

    fn lower_one_level_field_pattern(
        &mut self,
        match_site: &ExprSiteId,
        pattern: &Pat,
        pattern_path: &[u32],
        base: LoweredValue,
        field: &CheckedDataFieldId,
        shape: RegisteredRuntimeShape,
        environment: &mut BinderEnvironment,
    ) -> Result<Option<LoweredValue>, LoweringError> {
        let field_ordinal = u32::try_from(field.field_index).map_err(|_| {
            self.residual_error(
                match_site,
                ClassificationResidualReason::MatchNormalizationRequired,
                [ClassificationResidualDependency::Field(
                    field_dependency_digest(field),
                )],
            )
        })?;
        let projection = DeferredFieldProjection {
            owner_id: shape.key.owner_id,
            variant_ordinal: shape.key.variant_ordinal,
            field_ordinal,
            base: base.node,
            dependency: field_dependency_digest(field),
        };
        match pattern {
            Pat::Wild => Ok(None),
            Pat::Var(_) => {
                environment.insert(
                    pattern_binder_site(match_site, pattern_path, false),
                    BinderValue::DeferredProjection(projection),
                );
                Ok(None)
            }
            // The checked constructor identity proves this field's ordinal, but it does not
            // retain the field's checked result type. Do not infer that type from the literal:
            // aliases and other scalar-compatible source types would make the Project node
            // ill-typed. Whole-value literals remain lowerable because the scrutinee carries
            // its checked type; field literals stay residual until checked field types are
            // available here.
            Pat::Lit(_) => Err(self.residual_error(
                match_site,
                ClassificationResidualReason::MatchNormalizationRequired,
                [ClassificationResidualDependency::Field(
                    projection.dependency,
                )],
            )),
            Pat::Con(_, _) | Pat::NamedCon(_, _) | Pat::As(_, _) => Err(self.residual_error(
                match_site,
                ClassificationResidualReason::MatchNormalizationRequired,
                [ClassificationResidualDependency::Field(
                    projection.dependency,
                )],
            )),
        }
    }

    fn lower_pattern_literal(
        &mut self,
        site: &ExprSiteId,
        literal: &Literal,
        value: LoweredValue,
    ) -> LoweringResult {
        let (literal_ty, literal_scalar, constant) =
            self.pattern_literal_constant(site, literal)?;
        if value.ty != literal_ty || value.scalar != Some(literal_scalar) {
            return Err(self.residual_error(
                site,
                ClassificationResidualReason::UnsupportedType,
                [ClassificationResidualDependency::Node(value.node)],
            ));
        }
        let literal = self.intern(
            literal_ty,
            Some(literal_scalar),
            ClassificationNodeKind::Constant(constant),
        )?;
        let (bool_ty, bool_scalar) = self.boolean_type(site)?;
        self.intern(
            bool_ty,
            bool_scalar,
            ClassificationNodeKind::Binary {
                op: ClassificationBinaryOp::Equal,
                left: value.node,
                right: literal.node,
            },
        )
    }

    fn pattern_literal_constant(
        &self,
        site: &ExprSiteId,
        literal: &Literal,
    ) -> Result<(ClassificationTypeId, ScalarKind, ClassificationConstant), LoweringError> {
        let (ty, scalar, constant) = match literal {
            Literal::Int(value) => (
                Ty::Name("Int".to_string()),
                ScalarKind::Integer,
                ClassificationConstant::Integer(*value),
            ),
            Literal::Bool(value) => (
                Ty::Name("Bool".to_string()),
                ScalarKind::Boolean,
                ClassificationConstant::Boolean(*value),
            ),
            Literal::Str(value) => (
                Ty::Name("String".to_string()),
                ScalarKind::String,
                ClassificationConstant::String(value.clone().into_boxed_str()),
            ),
            Literal::Char(value) => (
                Ty::Name("Char".to_string()),
                ScalarKind::Character,
                ClassificationConstant::Character(*value),
            ),
            // Floating equality remains outside the exact scalar ABI.
            Literal::Float(_) => {
                return Err(self.residual_error(
                    site,
                    ClassificationResidualReason::MatchNormalizationRequired,
                    [],
                ));
            }
        };
        let Some((ty, Some(actual_scalar))) = self.classification_type(&ty) else {
            return Err(self.residual_error(
                site,
                ClassificationResidualReason::UnsupportedType,
                [],
            ));
        };
        if actual_scalar != scalar {
            return Err(self.residual_error(
                site,
                ClassificationResidualReason::UnsupportedType,
                [],
            ));
        }
        Ok((ty, scalar, constant))
    }

    fn boolean_type(
        &self,
        site: &ExprSiteId,
    ) -> Result<(ClassificationTypeId, Option<ScalarKind>), LoweringError> {
        let bool_ty = Ty::Name("Bool".to_string());
        match self.classification_type(&bool_ty) {
            Some((ty, scalar @ Some(ScalarKind::Boolean))) => Ok((ty, scalar)),
            _ => Err(self.residual_error(
                site,
                ClassificationResidualReason::UnsupportedType,
                self.type_dependencies(&bool_ty),
            )),
        }
    }

    fn boolean_constant(&mut self, site: &ExprSiteId, value: bool) -> LoweringResult {
        let (ty, scalar) = self.boolean_type(site)?;
        self.intern(
            ty,
            scalar,
            ClassificationNodeKind::Constant(ClassificationConstant::Boolean(value)),
        )
    }

    fn boolean_and(
        &mut self,
        site: &ExprSiteId,
        left: LoweredValue,
        right: LoweredValue,
    ) -> LoweringResult {
        let (ty, scalar) = self.boolean_type(site)?;
        if left.ty != ty
            || right.ty != ty
            || left.scalar != Some(ScalarKind::Boolean)
            || right.scalar != Some(ScalarKind::Boolean)
        {
            return Err(self.residual_error(
                site,
                ClassificationResidualReason::UnsupportedType,
                [
                    ClassificationResidualDependency::Node(left.node),
                    ClassificationResidualDependency::Node(right.node),
                ],
            ));
        }
        self.intern(
            ty,
            scalar,
            ClassificationNodeKind::Binary {
                op: ClassificationBinaryOp::BooleanAndShortCircuit,
                left: left.node,
                right: right.node,
            },
        )
    }

    fn lower_application(
        &mut self,
        site: &ExprSiteId,
        arguments: &[crate::Expr],
        resolution: &CheckedExpressionResolution,
        ty: ClassificationTypeId,
        scalar: Option<ScalarKind>,
        environment: &BinderEnvironment,
    ) -> LoweringResult {
        if let Some(CheckedCallTarget::Constructor { arity, .. }) = resolution.call_target.as_ref()
        {
            return self.lower_constructor_application(
                site,
                arguments,
                *arity,
                resolution,
                ty,
                scalar,
                environment,
            );
        }
        let (callable, arity) = match resolution.call_target.as_ref() {
            Some(CheckedCallTarget::Function { callable, arity }) => (callable, *arity),
            Some(CheckedCallTarget::BoundCallable { binder, .. }) => {
                return Err(self.residual_error(
                    site,
                    ClassificationResidualReason::HigherOrderCall,
                    [ClassificationResidualDependency::Capture(
                        checked_explore_projection_binder_digest(binder),
                    )],
                ));
            }
            Some(CheckedCallTarget::RuleFamily(family)) => {
                let mut dependencies = Vec::new();
                if let Some(digest) = self.semantic_dependency_digest(
                    CheckedExploreSemanticDependency::RuleFamily(family.clone()),
                ) {
                    dependencies.push(ClassificationResidualDependency::RuleFamily(digest));
                }
                return Err(self.residual_error(
                    site,
                    ClassificationResidualReason::DynamicDispatch,
                    dependencies,
                ));
            }
            Some(CheckedCallTarget::ScopedMember { rule_family, .. }) => {
                let mut dependencies = Vec::new();
                if let Some(family) = rule_family {
                    if let Some(digest) = self.semantic_dependency_digest(
                        CheckedExploreSemanticDependency::RuleFamily(family.clone()),
                    ) {
                        dependencies.push(ClassificationResidualDependency::RuleFamily(digest));
                    }
                }
                return Err(self.residual_error(
                    site,
                    ClassificationResidualReason::DynamicDispatch,
                    dependencies,
                ));
            }
            Some(CheckedCallTarget::Constructor { .. }) => unreachable!("handled above"),
            Some(CheckedCallTarget::Builtin { .. }) | None => {
                return Err(self.residual_error(
                    site,
                    ClassificationResidualReason::DynamicDispatch,
                    self.resolution_dependencies(resolution),
                ));
            }
        };
        if arity != arguments.len() {
            return Err(self.residual_error(
                site,
                ClassificationResidualReason::DynamicDispatch,
                self.resolution_dependencies(resolution),
            ));
        }

        let callable_id = self.ensure_callable(callable, arity, site)?;
        let definition = self
            .callable_definitions
            .get(&callable_id)
            .cloned()
            .expect("lowered callable has a frozen definition");
        if definition.return_type != ty {
            return Err(self.residual_error(
                site,
                ClassificationResidualReason::UnsupportedType,
                [ClassificationResidualDependency::Callable(callable_id)],
            ));
        }
        let argument_sites = self.canonical_argument_sites(site, arguments, resolution)?;
        if argument_sites.len() != definition.parameter_types.len() {
            return Err(self.residual_error(
                site,
                ClassificationResidualReason::DynamicDispatch,
                [ClassificationResidualDependency::Callable(callable_id)],
            ));
        }
        let mut lowered_arguments = Vec::with_capacity(argument_sites.len());
        for (argument_site, parameter_type) in
            argument_sites.iter().zip(definition.parameter_types.iter())
        {
            let argument = self.lower_expression(argument_site, environment)?;
            if argument.ty != *parameter_type {
                return Err(self.residual_error(
                    argument_site,
                    ClassificationResidualReason::UnsupportedType,
                    [
                        ClassificationResidualDependency::Node(argument.node),
                        ClassificationResidualDependency::Callable(callable_id),
                    ],
                ));
            }
            lowered_arguments.push(argument.node);
        }
        self.intern(
            ty,
            scalar,
            ClassificationNodeKind::Call {
                callable_id,
                arguments: lowered_arguments.into_boxed_slice(),
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_constructor_application(
        &mut self,
        site: &ExprSiteId,
        arguments: &[crate::Expr],
        arity: usize,
        resolution: &CheckedExpressionResolution,
        ty: ClassificationTypeId,
        scalar: Option<ScalarKind>,
        environment: &BinderEnvironment,
    ) -> LoweringResult {
        let Some(constructor) = resolution.exact_constructor.as_ref() else {
            return Err(self.residual_error(
                site,
                ClassificationResidualReason::UnsupportedExpression,
                self.resolution_dependencies(resolution),
            ));
        };
        let Some(CheckedCallTarget::Constructor {
            owner_type,
            variant,
            variant_index,
            arity: target_arity,
            ..
        }) = resolution.call_target.as_ref()
        else {
            return Err(self.residual_error(
                site,
                ClassificationResidualReason::UnsupportedExpression,
                self.resolution_dependencies(resolution),
            ));
        };
        if *target_arity != arity
            || !legacy_constructor_metadata_matches(
                owner_type,
                variant,
                *variant_index,
                constructor,
            )
        {
            return Err(self.residual_error(
                site,
                ClassificationResidualReason::UnsupportedExpression,
                self.resolution_dependencies(resolution),
            ));
        }
        if arity != arguments.len() || arity != constructor.fields.len() {
            return Err(self.residual_error(
                site,
                ClassificationResidualReason::UnsupportedExpression,
                [ClassificationResidualDependency::Constructor(
                    checked_explore_projection_constructor_digest(constructor),
                )],
            ));
        }
        if scalar == Some(ScalarKind::Boolean)
            && arity == 0
            && matches!(
                &constructor.owner,
                CheckedDataTypeId::Intrinsic { canonical_name }
                    if canonical_name.as_ref() == "Bool"
            )
        {
            let value = match constructor.variant.as_ref() {
                "True" => true,
                "False" => false,
                _ => {
                    return Err(self.residual_error(
                        site,
                        ClassificationResidualReason::UnsupportedExpression,
                        [ClassificationResidualDependency::Constructor(
                            checked_explore_projection_constructor_digest(constructor),
                        )],
                    ));
                }
            };
            return self.intern(
                ty,
                scalar,
                ClassificationNodeKind::Constant(ClassificationConstant::Boolean(value)),
            );
        }

        let argument_sites = self.canonical_argument_sites(site, arguments, resolution)?;
        if let Some(order) = resolution.named_arguments.as_ref() {
            if order.parameter_names.len() != constructor.fields.len()
                || !order
                    .parameter_names
                    .iter()
                    .zip(constructor.fields.iter())
                    .all(|(parameter, field)| parameter.as_ref() == field.name.as_ref())
            {
                return Err(self.residual_error(
                    site,
                    ClassificationResidualReason::UnsupportedExpression,
                    [ClassificationResidualDependency::Constructor(
                        checked_explore_projection_constructor_digest(constructor),
                    )],
                ));
            }
        }
        if argument_sites.len() != constructor.fields.len() {
            return Err(self.residual_error(
                site,
                ClassificationResidualReason::UnsupportedExpression,
                [ClassificationResidualDependency::Constructor(
                    checked_explore_projection_constructor_digest(constructor),
                )],
            ));
        }
        let mut fields = Vec::with_capacity(argument_sites.len());
        for argument_site in &argument_sites {
            fields.push(self.lower_expression(argument_site, environment)?.node);
        }
        let shape = self.register_runtime_shape(site, constructor)?;
        self.intern(
            ty,
            scalar,
            ClassificationNodeKind::Construct {
                constructor_id: shape.constructor_id,
                fields: fields.into_boxed_slice(),
            },
        )
    }

    fn canonical_argument_sites(
        &self,
        application_site: &ExprSiteId,
        arguments: &[crate::Expr],
        resolution: &CheckedExpressionResolution,
    ) -> Result<Vec<ExprSiteId>, LoweringError> {
        let Some(order) = resolution.named_arguments.as_ref() else {
            return Ok((0..arguments.len())
                .map(|source_index| child_site(application_site, source_index + 1))
                .collect());
        };
        if order.canonical_source_indices.len() != arguments.len() {
            return Err(self.residual_error(
                application_site,
                ClassificationResidualReason::DynamicDispatch,
                self.resolution_dependencies(resolution),
            ));
        }
        let mut seen = BTreeSet::new();
        let mut sites = Vec::with_capacity(arguments.len());
        for source_index in order.canonical_source_indices.iter().copied() {
            if source_index >= arguments.len() || !seen.insert(source_index) {
                return Err(self.residual_error(
                    application_site,
                    ClassificationResidualReason::DynamicDispatch,
                    self.resolution_dependencies(resolution),
                ));
            }
            let source_site = child_site(application_site, source_index + 1);
            let ExprKind::App(_, wrapper_arguments) = &arguments[source_index].kind else {
                return Err(self.residual_error(
                    &source_site,
                    ClassificationResidualReason::DynamicDispatch,
                    self.resolution_dependencies(resolution),
                ));
            };
            if wrapper_arguments.len() != 2 {
                return Err(self.residual_error(
                    &source_site,
                    ClassificationResidualReason::DynamicDispatch,
                    self.resolution_dependencies(resolution),
                ));
            }
            // CheckedNamedArgumentOrder already authenticated the wrapper and
            // permutation. Child 2 is structurally the value; no marker/name
            // is re-resolved here.
            sites.push(child_site(&source_site, 2));
        }
        Ok(sites)
    }

    fn ensure_callable(
        &mut self,
        callable: &CheckedCallableId,
        arity: usize,
        call_site: &ExprSiteId,
    ) -> Result<ClassificationCallableId, LoweringError> {
        let callable_id = self.classification_callable_id(callable).ok_or_else(|| {
            self.residual_error(call_site, ClassificationResidualReason::DynamicDispatch, [])
        })?;
        match self.callable_states.get(callable).cloned() {
            Some(CallableLoweringState::Lowered(lowered)) => return Ok(lowered),
            Some(CallableLoweringState::Visiting) => {
                return Err(self.residual_error(
                    call_site,
                    ClassificationResidualReason::RecursiveCall,
                    [ClassificationResidualDependency::Callable(callable_id)],
                ));
            }
            Some(CallableLoweringState::Residual(failure)) => {
                return Err(LoweringError::Residual(failure));
            }
            None => {}
        }

        self.callable_states
            .insert(callable.clone(), CallableLoweringState::Visiting);
        let result = self.lower_callable_definition(callable, callable_id, arity, call_site);
        match result {
            Ok(definition) => {
                self.callable_definitions.insert(callable_id, definition);
                self.callable_states.insert(
                    callable.clone(),
                    CallableLoweringState::Lowered(callable_id),
                );
                Ok(callable_id)
            }
            Err(LoweringError::Residual(failure)) => {
                let failure = failure
                    // Callable-local nodes (especially CallableParameter)
                    // have no lane-root ownership once the callable itself is
                    // residualized. The callable semantic identity and the
                    // lane site seal the fallback without dangling graph
                    // references.
                    .without_node_dependencies()
                    .with_dependency(ClassificationResidualDependency::Callable(callable_id));
                self.callable_states.insert(
                    callable.clone(),
                    CallableLoweringState::Residual(failure.clone()),
                );
                Err(LoweringError::Residual(failure))
            }
            Err(LoweringError::Capsule(error)) => Err(LoweringError::Capsule(error)),
        }
    }

    fn lower_callable_definition(
        &mut self,
        callable: &CheckedCallableId,
        callable_id: ClassificationCallableId,
        arity: usize,
        call_site: &ExprSiteId,
    ) -> Result<ClassificationCallableDefinition, LoweringError> {
        let descriptor = self.index.callables.get(callable).cloned().ok_or_else(|| {
            self.residual_error(
                call_site,
                ClassificationResidualReason::DynamicDispatch,
                [ClassificationResidualDependency::Callable(callable_id)],
            )
        })?;
        if descriptor.parameters.len() != arity || descriptor.parameter_sites.len() != arity {
            return Err(self.residual_error(
                call_site,
                ClassificationResidualReason::DynamicDispatch,
                [ClassificationResidualDependency::Callable(callable_id)],
            ));
        }
        if !descriptor.effects.is_empty() || descriptor.parameters.iter().any(|param| param.inout) {
            return Err(self.residual_error(
                call_site,
                ClassificationResidualReason::EffectfulExpression,
                [ClassificationResidualDependency::Callable(callable_id)],
            ));
        }

        let mut parameter_types = Vec::with_capacity(arity);
        let mut environment = BinderEnvironment::new();
        for (ordinal, (parameter, binder)) in descriptor
            .parameters
            .iter()
            .zip(descriptor.parameter_sites.iter())
            .enumerate()
        {
            let Some(parameter_ty) = parameter.ty.as_ref() else {
                return Err(self.residual_error(
                    call_site,
                    ClassificationResidualReason::UnsupportedType,
                    [ClassificationResidualDependency::Callable(callable_id)],
                ));
            };
            let Some((ty, scalar)) = self.classification_type(parameter_ty) else {
                return Err(self.residual_error(
                    call_site,
                    ClassificationResidualReason::UnsupportedType,
                    self.type_dependencies(parameter_ty),
                ));
            };
            let ordinal = u32::try_from(ordinal).map_err(|_| {
                self.residual_error(
                    call_site,
                    ClassificationResidualReason::UnsupportedType,
                    [ClassificationResidualDependency::Callable(callable_id)],
                )
            })?;
            let value = self.intern(
                ty,
                scalar,
                ClassificationNodeKind::CallableParameter {
                    callable_id,
                    ordinal,
                },
            )?;
            parameter_types.push(ty);
            environment.insert(binder.clone(), BinderValue::Lowered(value));
        }

        let Some(return_ty) = descriptor.return_type else {
            return Err(self.residual_error(
                call_site,
                ClassificationResidualReason::UnsupportedType,
                [ClassificationResidualDependency::Callable(callable_id)],
            ));
        };
        let Some((return_type, _)) = self.classification_type(return_ty) else {
            return Err(self.residual_error(
                call_site,
                ClassificationResidualReason::UnsupportedType,
                self.type_dependencies(return_ty),
            ));
        };
        let body = self.lower_expression(&descriptor.body_site, &environment)?;
        if body.ty != return_type {
            return Err(self.residual_error(
                &descriptor.body_site,
                ClassificationResidualReason::UnsupportedType,
                [ClassificationResidualDependency::Node(body.node)],
            ));
        }
        Ok(ClassificationCallableDefinition {
            callable_id,
            parameter_types: parameter_types.into_boxed_slice(),
            return_type,
            body: body.node,
        })
    }

    fn classification_callable_id(
        &self,
        callable: &CheckedCallableId,
    ) -> Option<ClassificationCallableId> {
        let digest = checked_explore_semantic_dependency_root_digest(
            &self.index,
            self.resolutions,
            &self.semantic_binders,
            CheckedExploreSemanticDependency::Callable(callable.clone()),
        )
        .ok()?;
        Some(ClassificationCallableId::from_checked_callable_digest(
            digest,
        ))
    }

    fn classification_type(&self, ty: &Ty) -> Option<(ClassificationTypeId, Option<ScalarKind>)> {
        if !classification_type_shape_supported(ty) {
            return None;
        }
        let mut hasher = Sha256::new();
        hasher.update(CLASSIFICATION_TYPE_DIGEST_V1);
        hash_checked_explore_type_schema(&mut hasher, self.resolutions, ty).ok()?;
        let id = ClassificationTypeId::from_checked_type_digest(hasher.finalize().into());
        Some((id, self.scalar_kind(ty)))
    }

    fn register_runtime_shape(
        &mut self,
        site: &ExprSiteId,
        identity: &CheckedConstructorIdentity,
    ) -> Result<RegisteredRuntimeShape, LoweringError> {
        let variant_ordinal = u32::try_from(identity.variant_index).map_err(|_| {
            self.residual_error(
                site,
                ClassificationResidualReason::UnsupportedType,
                [ClassificationResidualDependency::Constructor(
                    checked_explore_projection_constructor_digest(identity),
                )],
            )
        })?;
        let canonical_fields = identity.fields.iter().enumerate().all(|(index, field)| {
            field.owner == identity.owner
                && field.variant_index == identity.variant_index
                && field.field_index == index
        });
        if !canonical_fields
            || (identity.fields.is_empty()
                && identity.layout != CheckedConstructorLayout::Positional)
        {
            return Err(self.residual_error(
                site,
                ClassificationResidualReason::UnsupportedExpression,
                [ClassificationResidualDependency::Constructor(
                    checked_explore_projection_constructor_digest(identity),
                )],
            ));
        }
        let key = RuntimeShapeKey {
            owner_id: checked_explore_projection_owner_digest(&identity.owner),
            variant_ordinal,
        };
        if let Some(previous) = self.runtime_shapes.get(&key) {
            if previous != identity {
                return Err(self.residual_error(
                    site,
                    ClassificationResidualReason::UnsupportedExpression,
                    [
                        ClassificationResidualDependency::Constructor(
                            checked_explore_projection_constructor_digest(previous),
                        ),
                        ClassificationResidualDependency::Constructor(
                            checked_explore_projection_constructor_digest(identity),
                        ),
                    ],
                ));
            }
        } else {
            self.runtime_shapes.insert(key, identity.clone());
        }
        Ok(RegisteredRuntimeShape {
            key,
            constructor_id: checked_explore_projection_constructor_digest(identity),
        })
    }

    fn exact_constructor_for_field(
        &self,
        field: &crate::CheckedVariantField,
    ) -> Option<CheckedConstructorIdentity> {
        let mut candidates = self
            .resolutions
            .constructor_identities
            .values()
            .filter(|identity| {
                identity.owner == field.identity.owner
                    && identity.variant_index == field.variant_index
            });
        let identity = candidates.next()?.as_ref().clone();
        if candidates.next().is_some() {
            return None;
        }
        let exact_field = identity.fields.get(field.field_index)?;
        (identity.owner == field.identity.owner
            && identity.variant_index == field.variant_index
            && identity.layout == field.layout
            && exact_field == &field.identity)
            .then_some(identity)
    }

    fn scalar_kind(&self, ty: &Ty) -> Option<ScalarKind> {
        let canonical = match ty {
            Ty::Unit => return Some(ScalarKind::Unit),
            Ty::Name(name) => match self.resolutions.data_type_identities.get(name.as_str())? {
                CheckedDataTypeId::Intrinsic { canonical_name } => canonical_name.as_ref(),
                CheckedDataTypeId::Declared(_) => return None,
            },
            _ => return None,
        };
        match canonical {
            "Int" => Some(ScalarKind::Integer),
            "Float" => Some(ScalarKind::Float),
            "Bool" => Some(ScalarKind::Boolean),
            "String" => Some(ScalarKind::String),
            "Char" => Some(ScalarKind::Character),
            "Unit" => Some(ScalarKind::Unit),
            _ => None,
        }
    }

    fn intern(
        &mut self,
        ty: ClassificationTypeId,
        scalar: Option<ScalarKind>,
        kind: ClassificationNodeKind,
    ) -> LoweringResult {
        let node = self.interner.intern(ClassificationNodeKey { ty, kind })?;
        Ok(LoweredValue { node, ty, scalar })
    }

    fn residual_error(
        &self,
        _site: &ExprSiteId,
        reason: ClassificationResidualReason,
        dependencies: impl IntoIterator<Item = ClassificationResidualDependency>,
    ) -> LoweringError {
        LoweringError::Residual(LoweringFailure {
            reason,
            dependencies: dependencies.into_iter().collect(),
        })
    }

    fn residual_identity_digest(
        &self,
        root: &ResidualIdentityRoot,
    ) -> Result<[u8; 32], CheckedExploreClassificationError> {
        let result = match root {
            ResidualIdentityRoot::Expression(site) => checked_explore_semantic_dependency_digest(
                &self.index,
                self.resolutions,
                &self.semantic_binders,
                "classification lane residual",
                std::slice::from_ref(site),
                &[],
            ),
            ResidualIdentityRoot::Type(ty) => checked_explore_semantic_dependency_digest(
                &self.index,
                self.resolutions,
                &self.semantic_binders,
                "classification typed identity lane residual",
                &[],
                std::slice::from_ref(ty),
            ),
            ResidualIdentityRoot::Synthetic(digest) => return Ok(*digest),
        };
        result.map_err(|issue| {
            CheckedExploreClassificationError::CheckedBoundary(
                format!("classification lane residual has no sealed semantic identity: {issue:?}")
                    .into_boxed_str(),
            )
        })
    }

    fn synthetic_find_site_digest(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(CLASSIFICATION_SYNTHETIC_FIND_SITE_V1);
        hasher.update(self.question_id.bytes());
        hasher.finalize().into()
    }

    fn semantic_dependency_digest(
        &self,
        dependency: CheckedExploreSemanticDependency,
    ) -> Option<[u8; 32]> {
        checked_explore_semantic_dependency_root_digest(
            &self.index,
            self.resolutions,
            &self.semantic_binders,
            dependency,
        )
        .ok()
    }

    fn type_dependencies(&self, ty: &Ty) -> Vec<ClassificationResidualDependency> {
        let mut owners = BTreeSet::new();
        collect_declared_type_owners(self.resolutions, ty, &mut owners);
        owners
            .into_iter()
            .filter_map(|owner| {
                self.semantic_dependency_digest(CheckedExploreSemanticDependency::DeclaredType(
                    owner,
                ))
                .map(ClassificationResidualDependency::DeclaredType)
            })
            .collect()
    }

    fn resolution_dependencies(
        &self,
        resolution: &CheckedExpressionResolution,
    ) -> Vec<ClassificationResidualDependency> {
        let mut dependencies = Vec::new();
        match resolution.value_binding.as_ref() {
            Some(CheckedValueBinding::Binder { site, .. }) => {
                dependencies.push(ClassificationResidualDependency::Capture(
                    checked_explore_projection_binder_digest(site),
                ))
            }
            Some(CheckedValueBinding::TopLevel(binding)) => {
                if let Some(digest) = self.semantic_dependency_digest(
                    CheckedExploreSemanticDependency::TopLevel(binding.clone()),
                ) {
                    dependencies.push(ClassificationResidualDependency::TopLevelConstant(digest));
                }
            }
            Some(CheckedValueBinding::Callable(callable)) => {
                if let Some(callable) = self.classification_callable_id(callable) {
                    dependencies.push(ClassificationResidualDependency::Callable(callable));
                }
            }
            Some(CheckedValueBinding::RuleFamily(family)) => {
                if let Some(digest) = self.semantic_dependency_digest(
                    CheckedExploreSemanticDependency::RuleFamily(family.clone()),
                ) {
                    dependencies.push(ClassificationResidualDependency::RuleFamily(digest));
                }
            }
            Some(CheckedValueBinding::Constructor { .. })
            | Some(CheckedValueBinding::OpaqueQualifiedOwner(_))
            | None => {}
        }
        match resolution.call_target.as_ref() {
            Some(CheckedCallTarget::Function { callable, .. }) => {
                if let Some(callable) = self.classification_callable_id(callable) {
                    dependencies.push(ClassificationResidualDependency::Callable(callable));
                }
            }
            Some(CheckedCallTarget::BoundCallable { binder, .. }) => {
                dependencies.push(ClassificationResidualDependency::Capture(
                    checked_explore_projection_binder_digest(binder),
                ))
            }
            Some(CheckedCallTarget::RuleFamily(family)) => {
                if let Some(digest) = self.semantic_dependency_digest(
                    CheckedExploreSemanticDependency::RuleFamily(family.clone()),
                ) {
                    dependencies.push(ClassificationResidualDependency::RuleFamily(digest));
                }
            }
            Some(CheckedCallTarget::ScopedMember { rule_family, .. }) => {
                if let Some(family) = rule_family {
                    if let Some(digest) = self.semantic_dependency_digest(
                        CheckedExploreSemanticDependency::RuleFamily(family.clone()),
                    ) {
                        dependencies.push(ClassificationResidualDependency::RuleFamily(digest));
                    }
                }
            }
            Some(CheckedCallTarget::Builtin { .. })
            | Some(CheckedCallTarget::Constructor { .. })
            | None => {}
        }
        if let Some(constructor) = resolution.exact_constructor.as_ref() {
            dependencies.push(ClassificationResidualDependency::Constructor(
                checked_explore_projection_constructor_digest(constructor),
            ));
        }
        if let Some(CheckedFieldResolution::Data { fields, .. }) = resolution.field.as_ref() {
            dependencies.extend(fields.iter().map(|field| {
                ClassificationResidualDependency::Field(field_dependency_digest(&field.identity))
            }));
        }
        if let Some(CheckedFieldResolution::ScopedMember {
            rule_family: Some(family),
            ..
        }) = resolution.field.as_ref()
        {
            if let Some(digest) = self.semantic_dependency_digest(
                CheckedExploreSemanticDependency::RuleFamily(family.clone()),
            ) {
                dependencies.push(ClassificationResidualDependency::RuleFamily(digest));
            }
        }
        dependencies
    }
}

fn classification_type_shape_supported(ty: &Ty) -> bool {
    match ty {
        Ty::Name(_) | Ty::Unit => true,
        Ty::App(constructor, arguments) => {
            classification_type_shape_supported(constructor)
                && arguments.iter().all(classification_type_shape_supported)
        }
        Ty::Optional(inner) => classification_type_shape_supported(inner),
        Ty::Arrow(_, _) | Ty::Ref(_) | Ty::MutRef(_) | Ty::Shared(_) | Ty::Var(_) | Ty::Hole => {
            false
        }
    }
}

fn collect_declared_type_owners(
    resolutions: &CheckedResolutionArtifacts,
    ty: &Ty,
    owners: &mut BTreeSet<CheckedDataTypeId>,
) {
    match ty {
        Ty::Name(name) => {
            if let Some(owner @ CheckedDataTypeId::Declared(_)) =
                resolutions.data_type_identities.get(name.as_str())
            {
                owners.insert(owner.clone());
            }
        }
        Ty::App(constructor, arguments) => {
            collect_declared_type_owners(resolutions, constructor, owners);
            for argument in arguments {
                collect_declared_type_owners(resolutions, argument, owners);
            }
        }
        Ty::Arrow(parameter, result) => {
            collect_declared_type_owners(resolutions, parameter, owners);
            collect_declared_type_owners(resolutions, result, owners);
        }
        Ty::Ref(inner) | Ty::MutRef(inner) | Ty::Shared(inner) | Ty::Optional(inner) => {
            collect_declared_type_owners(resolutions, inner, owners);
        }
        Ty::Var(_) | Ty::Unit | Ty::Hole => {}
    }
}

fn child_site(site: &ExprSiteId, child: usize) -> ExprSiteId {
    let mut result = site.clone();
    let mut path = result.ast_path.to_vec();
    path.push(child as u32);
    result.ast_path = path.into_boxed_slice();
    result
}

fn checked_pattern_site(match_site: &ExprSiteId, pattern_path: &[u32]) -> CheckedPatternSiteId {
    CheckedPatternSiteId {
        analysis_program: match_site.analysis_program.clone(),
        declaration: match_site.declaration.clone(),
        normalized_declaration_ordinal: match_site.normalized_declaration_ordinal,
        ast_path: match_site.ast_path.clone(),
        pattern_path: pattern_path.to_vec().into_boxed_slice(),
    }
}

fn pattern_binder_site(
    match_site: &ExprSiteId,
    pattern_path: &[u32],
    as_binder: bool,
) -> CheckedBinderSiteId {
    let mut binder_path =
        Vec::with_capacity(1 + pattern_path.len() + if as_binder { 1 } else { 0 });
    binder_path.push(CheckedResolutionRecorder::BINDER_PATTERN);
    binder_path.extend_from_slice(pattern_path);
    if as_binder {
        binder_path.push(u32::MAX);
    }
    CheckedBinderSiteId::Structural {
        analysis_program: match_site.analysis_program.clone(),
        declaration: match_site.declaration.clone(),
        normalized_declaration_ordinal: match_site.normalized_declaration_ordinal,
        ast_path: match_site.ast_path.clone(),
        binder_path: binder_path.into_boxed_slice(),
    }
}

fn pattern_is_irrefutable(pattern: &Pat) -> bool {
    match pattern {
        Pat::Wild | Pat::Var(_) => true,
        Pat::As(inner, _) => pattern_is_irrefutable(inner),
        Pat::Lit(_) | Pat::Con(_, _) | Pat::NamedCon(_, _) => false,
    }
}

fn pattern_is_tag_only(pattern: &Pat) -> bool {
    match pattern {
        Pat::Wild | Pat::Lit(_) => true,
        Pat::Con(_, children) => children.is_empty(),
        Pat::Var(_) | Pat::NamedCon(_, _) | Pat::As(_, _) => false,
    }
}

fn legacy_constructor_metadata_matches(
    owner_type: &str,
    variant: &str,
    variant_index: Option<usize>,
    exact: &CheckedConstructorIdentity,
) -> bool {
    owner_type == exact.owner_type.as_ref()
        && variant == exact.variant.as_ref()
        && variant_index.is_none_or(|variant_index| variant_index == exact.variant_index)
}

fn match_arm_sites(
    match_site: &ExprSiteId,
    arms: &[MatchArm],
) -> Vec<(Option<ExprSiteId>, ExprSiteId)> {
    let mut sites = Vec::with_capacity(arms.len());
    let mut child_index = 1;
    for arm in arms {
        let guard = arm.guard.as_ref().map(|_| {
            let site = child_site(match_site, child_index);
            child_index += 1;
            site
        });
        let body = child_site(match_site, child_index);
        child_index += 1;
        sites.push((guard, body));
    }
    sites
}

fn field_dependency_digest(field: &CheckedDataFieldId) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(CLASSIFICATION_FIELD_DEPENDENCY_V1);
    hasher.update(checked_explore_projection_owner_digest(&field.owner));
    hasher.update((field.variant_index as u64).to_le_bytes());
    hasher.update((field.field_index as u64).to_le_bytes());
    hasher.finalize().into()
}
