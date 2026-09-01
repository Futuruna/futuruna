//! Closed relational query descriptors for Explore.
//!
//! This IR preserves the dependency order of the finite source relation and
//! gives `context`, `before`, and each per-source `after` value semantic roles.
//! It deliberately contains no Cartesian axes, boundary hints, output mode,
//! probe plan, scheduling policy, or rank-derived identity.

use std::collections::BTreeSet;

use super::{
    ExploreExactDomain, FindPolarity, StructuralEdgeId, StructuralMechanismId, StructuralNodeId,
};
use crate::{
    ExploreAdmissionScope, ExploreChooseCardinality, ExploreOptimizeDirection,
    ExploreRelationMultiplicity, ExploreStarterProjectionFacet, Expr, ExprKind, Span, Ty,
    TypedExploreStarterProjection, TypedExploreStarterProjectionSubject,
    EXPLORE_RELATION_NORMALIZATION_VERSION,
};

/// Compare the checked type shapes carried into relational IR without relying
/// on [`Ty`] implementing global equality. Optional syntax is normalized to
/// its explicit unary `Option` application, matching Explore type checking.
pub(crate) fn relational_tys_equivalent(left: &Ty, right: &Ty) -> bool {
    match (left, right) {
        (Ty::Name(left), Ty::Name(right)) | (Ty::Var(left), Ty::Var(right)) => left == right,
        (
            Ty::App(left_constructor, left_arguments),
            Ty::App(right_constructor, right_arguments),
        ) => {
            relational_tys_equivalent(left_constructor, right_constructor)
                && left_arguments.len() == right_arguments.len()
                && left_arguments
                    .iter()
                    .zip(right_arguments)
                    .all(|(left, right)| relational_tys_equivalent(left, right))
        }
        (Ty::Arrow(left_input, left_output), Ty::Arrow(right_input, right_output)) => {
            relational_tys_equivalent(left_input, right_input)
                && relational_tys_equivalent(left_output, right_output)
        }
        (Ty::Ref(left), Ty::Ref(right))
        | (Ty::MutRef(left), Ty::MutRef(right))
        | (Ty::Shared(left), Ty::Shared(right))
        | (Ty::Optional(left), Ty::Optional(right)) => relational_tys_equivalent(left, right),
        (Ty::Optional(inner), Ty::App(constructor, arguments))
        | (Ty::App(constructor, arguments), Ty::Optional(inner)) => {
            matches!(constructor.as_ref(), Ty::Name(name) if name == "Option")
                && arguments.len() == 1
                && relational_tys_equivalent(inner, &arguments[0])
        }
        (Ty::Unit, Ty::Name(name)) | (Ty::Name(name), Ty::Unit) => name == "Unit",
        (Ty::Unit, Ty::Unit) | (Ty::Hole, Ty::Hole) => true,
        _ => false,
    }
}

/// Version of the canonical relational IR shape, independent of run and view
/// serialization versions.
pub const EXPLORE_RELATIONAL_IR_VERSION: u32 = 1;

/// One already-checked finite-domain plan.
///
/// `Exact` is source-independent and can be enumerated directly. The other
/// variants are evaluated inside the environment identified by explicit
/// binding dependencies; their typing and finiteness proofs are producer
/// obligations, not executor guesses.
#[derive(Debug, Clone)]
pub enum ExploreFiniteDomainIr {
    Exact(ExploreExactDomain),
    Collection {
        expression: Expr,
        collection_ty: Ty,
        element_ty: Ty,
    },
    IntRange {
        start: Expr,
        end_exclusive: Expr,
    },
}

/// A resolved edge from one source binding to an earlier source binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreSourceDependencyIr {
    pub binding_index: usize,
    pub binding_name: String,
}

/// Semantic participation of a source binding in the canonical source row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreSourceBindingRoleIr {
    /// A dependent-construction input. It contributes lineage/support, but the
    /// semantic source key remains the typed `(Context, Before)` pair.
    Auxiliary,
    Context,
    Before,
}

#[derive(Debug, Clone)]
pub enum ExploreSourceBindingKindIr {
    Singleton { value: Expr },
    Finite { domain: ExploreFiniteDomainIr },
}

/// One binding in the ordered, dependent finite source relation.
#[derive(Debug, Clone)]
pub struct ExploreSourceBindingIr {
    pub binding_index: usize,
    pub name: String,
    pub value_ty: Ty,
    pub role: ExploreSourceBindingRoleIr,
    /// Canonical, index-sorted dependencies. Every edge must point strictly to
    /// an earlier binding.
    pub dependencies: Box<[ExploreSourceDependencyIr]>,
    pub kind: ExploreSourceBindingKindIr,
    pub span: Span,
}

/// Producer-closed description of the finite source relation.
#[derive(Debug, Clone)]
pub struct ExploreSourceRelationIr {
    pub normalization_version: u32,
    pub multiplicity: ExploreRelationMultiplicity,
    pub bindings: Box<[ExploreSourceBindingIr]>,
    pub context_binding_index: usize,
    pub before_binding_index: usize,
    pub context_ty: Ty,
    pub before_ty: Ty,
}

#[derive(Debug, Clone)]
pub enum ExploreSuccessorKindIr {
    Singleton { value: Expr },
    Finite { domain: ExploreFiniteDomainIr },
}

/// The finite successor relation evaluated separately for every source row.
/// Its expression environment contains only semantic `context` and `before`.
#[derive(Debug, Clone)]
pub struct ExploreSuccessorRelationIr {
    pub multiplicity: ExploreRelationMultiplicity,
    pub after_ty: Ty,
    pub kind: ExploreSuccessorKindIr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ExploreAdmissionIr {
    pub admission_index: usize,
    pub scope: ExploreAdmissionScope,
    pub predicate: Expr,
    pub span: Span,
}

/// A closed FIND question. The enum prevents `all` from accidentally carrying
/// a predicate and keeps matches/violations distinct without a mode flag.
#[derive(Debug, Clone)]
pub enum ExploreFindIr {
    All { span: Span },
    Matches { predicate: Expr, span: Span },
    Violations { predicate: Expr, span: Span },
}

impl ExploreFindIr {
    pub(crate) const fn polarity(&self) -> FindPolarity {
        match self {
            Self::All { .. } => FindPolarity::All,
            Self::Matches { .. } => FindPolarity::Matches,
            Self::Violations { .. } => FindPolarity::Violations,
        }
    }

    pub fn predicate(&self) -> Option<&Expr> {
        match self {
            Self::All { .. } => None,
            Self::Matches { predicate, .. } | Self::Violations { predicate, .. } => Some(predicate),
        }
    }

    pub fn span(&self) -> Span {
        match self {
            Self::All { span } | Self::Matches { span, .. } | Self::Violations { span, .. } => {
                *span
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExploreResultFieldIr {
    pub name: String,
    pub value: Expr,
    pub ty: Ty,
    pub span: Span,
}

/// The already-resolved row population consumed by a result node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExploreResultInputIr {
    /// Canonical `(Context, Before)` rows, independently of successor,
    /// admission, and FIND progress.
    Sources,
    /// Cases admitted and classified by this query's FIND question.
    Selected,
    /// Incidences produced by one strictly earlier mechanism node.
    MechanismIncidence { request_node_index: usize },
}

#[derive(Debug, Clone)]
pub enum ExploreAggregateReducerIr {
    CountDistinct { value: Expr, value_ty: Ty },
}

#[derive(Debug, Clone)]
pub struct ExploreAggregateFieldIr {
    pub name: String,
    pub reducer: ExploreAggregateReducerIr,
    pub ty: Ty,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ExploreResultGrainIr {
    EachCase {
        span: Span,
    },
    EachIncidence {
        span: Span,
    },
    GroupAll {
        span: Span,
    },
    GroupBy {
        fields: Box<[ExploreResultFieldIr]>,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub enum ExploreResultHavingIr {
    Varies {
        measure_name: String,
        measure_index: usize,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub struct ExploreParetoObjectiveIr {
    pub direction: ExploreOptimizeDirection,
    pub value: Expr,
    pub ty: Ty,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ExploreResultChoiceIr {
    Optimize {
        cardinality: ExploreChooseCardinality,
        direction: ExploreOptimizeDirection,
        objective: Expr,
        objective_ty: Ty,
        span: Span,
    },
    Pareto {
        objectives: Box<[ExploreParetoObjectiveIr]>,
        span: Span,
    },
}

/// One named result node over selected cases or a prior mechanism-incidence
/// relation.
#[derive(Debug, Clone)]
pub struct ExploreResultViewIr {
    pub node_index: usize,
    pub name: String,
    pub input: ExploreResultInputIr,
    pub grain: ExploreResultGrainIr,
    pub measures: Box<[ExploreResultFieldIr]>,
    pub aggregates: Box<[ExploreAggregateFieldIr]>,
    pub having: Option<ExploreResultHavingIr>,
    pub select: Box<[ExploreResultFieldIr]>,
    pub choose: Option<ExploreResultChoiceIr>,
    pub span: Span,
}

/// Resolved case population consumed by a mechanism request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExploreMechanismTargetIr {
    SelectedCases,
    /// The closed reference is positional and strictly prior. The view's name
    /// remains only on its descriptor and cannot enter target identity.
    ViewChosen {
        view_node_index: usize,
    },
}

/// One named differential mechanism observation request.
#[derive(Debug, Clone)]
pub struct ExploreMechanismRequestIr {
    pub node_index: usize,
    pub name: String,
    pub target: ExploreMechanismTargetIr,
    pub callable_name: String,
    /// Canonical endpoint template `CALLABLE(state, context)`.
    pub endpoint_template: Expr,
    pub observation_ty: Ty,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ExploreStarterProjectionFacetIr {
    Activation,
    DifferentialParticipation,
}

impl From<ExploreStarterProjectionFacet> for ExploreStarterProjectionFacetIr {
    fn from(facet: ExploreStarterProjectionFacet) -> Self {
        match facet {
            ExploreStarterProjectionFacet::Activation => Self::Activation,
            ExploreStarterProjectionFacet::DifferentialParticipation => {
                Self::DifferentialParticipation
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ExploreStarterProjectionSubjectIr {
    Mechanism(StructuralMechanismId),
    Node {
        facet: ExploreStarterProjectionFacetIr,
        node_id: StructuralNodeId,
    },
    Edge {
        facet: ExploreStarterProjectionFacetIr,
        edge_id: StructuralEdgeId,
    },
}

impl From<TypedExploreStarterProjectionSubject> for ExploreStarterProjectionSubjectIr {
    fn from(subject: TypedExploreStarterProjectionSubject) -> Self {
        match subject {
            TypedExploreStarterProjectionSubject::Mechanism(mechanism_id) => {
                Self::Mechanism(mechanism_id)
            }
            TypedExploreStarterProjectionSubject::Node { facet, node_id } => Self::Node {
                facet: facet.into(),
                node_id,
            },
            TypedExploreStarterProjectionSubject::Edge { facet, edge_id } => Self::Edge {
                facet: facet.into(),
                edge_id,
            },
        }
    }
}

/// One checked, single-subject starter projection consumer. It references the
/// core analysis DAG but is stored outside that DAG so attaching a publication
/// consumer cannot rename its upstream semantics.
#[derive(Debug, Clone)]
pub(crate) struct ExploreStarterProjectionIr {
    pub(crate) name: String,
    pub(crate) request_node_index: usize,
    pub(crate) subject: ExploreStarterProjectionSubjectIr,
    pub(crate) value_view_node_index: usize,
    pub(crate) span: Span,
}

impl ExploreStarterProjectionIr {
    pub(crate) fn lower(projection: &TypedExploreStarterProjection) -> Self {
        Self {
            name: projection.name.clone(),
            request_node_index: projection.request_node_index,
            subject: projection.subject.into(),
            value_view_node_index: projection.value_view_node_index,
            span: projection.span,
        }
    }
}

/// One node in declaration order. Positional references form a closed DAG:
/// every input or target edge must point to a strictly earlier compatible
/// node.
#[derive(Debug, Clone)]
pub enum ExploreAnalysisNodeIr {
    Result(ExploreResultViewIr),
    Mechanisms(ExploreMechanismRequestIr),
}

impl ExploreAnalysisNodeIr {
    pub fn node_index(&self) -> usize {
        match self {
            Self::Result(view) => view.node_index,
            Self::Mechanisms(request) => request.node_index,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Result(view) => &view.name,
            Self::Mechanisms(request) => &request.name,
        }
    }
}

/// Canonical relational Explore query descriptor.
///
/// Relation, admission, question, view, and mechanism identities are minted
/// by the checked-artifact layer from these normalized semantic descriptors;
/// declaration names remain addresses rather than identity inputs.
#[derive(Debug, Clone)]
pub struct ExploreQueryIr {
    pub name: String,
    pub source: ExploreSourceRelationIr,
    pub successor: ExploreSuccessorRelationIr,
    pub admissions: Box<[ExploreAdmissionIr]>,
    pub find: ExploreFindIr,
    pub analysis: Box<[ExploreAnalysisNodeIr]>,
    pub(crate) starter_projections: Box<[ExploreStarterProjectionIr]>,
    pub span: Span,
}

impl ExploreQueryIr {
    /// Check the closed structural invariants that downstream enumeration and
    /// identity derivation may rely on without reinterpreting source syntax.
    pub fn validate(&self) -> Result<(), String> {
        self.source.validate()?;

        if !relational_tys_equivalent(&self.source.before_ty, &self.successor.after_ty) {
            return Err("successor After type does not match source Before type".to_string());
        }

        for (expected, admission) in self.admissions.iter().enumerate() {
            if admission.admission_index != expected {
                return Err(format!(
                    "admission has canonical index {}, expected {}",
                    admission.admission_index, expected
                ));
            }
        }

        self.validate_analysis()?;
        self.validate_starter_projections()?;

        Ok(())
    }

    fn validate_analysis(&self) -> Result<(), String> {
        let mut names = BTreeSet::new();
        for (expected, node) in self.analysis.iter().enumerate() {
            if node.node_index() != expected {
                return Err(format!(
                    "analysis node `{}` has canonical index {}, expected {}",
                    node.name(),
                    node.node_index(),
                    expected
                ));
            }
            if !names.insert(node.name()) {
                return Err(format!("duplicate analysis node name `{}`", node.name()));
            }

            match node {
                ExploreAnalysisNodeIr::Result(view) => self.validate_result_view(view, expected)?,
                ExploreAnalysisNodeIr::Mechanisms(request) => {
                    if let ExploreMechanismTargetIr::ViewChosen { view_node_index } =
                        &request.target
                    {
                        if *view_node_index >= expected {
                            return Err(format!(
                                "mechanism request `{}` targets non-prior result node index {}",
                                request.name, view_node_index
                            ));
                        }
                        let Some(ExploreAnalysisNodeIr::Result(view)) =
                            self.analysis.get(*view_node_index)
                        else {
                            return Err(format!(
                                "mechanism request `{}` targets non-result node index {}",
                                request.name, view_node_index
                            ));
                        };
                        if !matches!(&view.input, ExploreResultInputIr::Selected)
                            || view.choose.is_none()
                        {
                            return Err(format!(
                                "mechanism request `{}` must target a chosen selected-case view",
                                request.name
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_starter_projections(&self) -> Result<(), String> {
        let mut names = self
            .analysis
            .iter()
            .map(ExploreAnalysisNodeIr::name)
            .collect::<BTreeSet<_>>();
        for projection in self.starter_projections.iter() {
            if !names.insert(projection.name.as_str()) {
                return Err(format!(
                    "duplicate exploration declaration `{}`",
                    projection.name
                ));
            }
            if !matches!(
                self.analysis.get(projection.request_node_index),
                Some(ExploreAnalysisNodeIr::Mechanisms(_))
            ) {
                return Err(format!(
                    "starter projection `{}` does not resolve mechanism node index {}",
                    projection.name, projection.request_node_index
                ));
            }
            let Some(ExploreAnalysisNodeIr::Result(value_view)) =
                self.analysis.get(projection.value_view_node_index)
            else {
                return Err(format!(
                    "starter projection `{}` does not resolve value-view node index {}",
                    projection.name, projection.value_view_node_index
                ));
            };
            if !starter_value_view_is_compatible(
                value_view,
                &self.source.context_ty,
                &self.source.before_ty,
                &self.successor.after_ty,
            ) {
                return Err(format!(
                    "starter projection `{}` requires a lossless selected-input each-case value view",
                    projection.name
                ));
            }
        }
        Ok(())
    }

    fn validate_result_view(
        &self,
        view: &ExploreResultViewIr,
        node_index: usize,
    ) -> Result<(), String> {
        if let ExploreResultInputIr::MechanismIncidence { request_node_index } = &view.input {
            if *request_node_index >= node_index {
                return Err(format!(
                    "result view `{}` consumes non-prior mechanism node index {}",
                    view.name, request_node_index
                ));
            }
            if !matches!(
                self.analysis.get(*request_node_index),
                Some(ExploreAnalysisNodeIr::Mechanisms(_))
            ) {
                return Err(format!(
                    "result view `{}` consumes non-mechanism node index {}",
                    view.name, request_node_index
                ));
            }
        }

        match (&view.input, &view.grain) {
            (
                ExploreResultInputIr::Sources,
                ExploreResultGrainIr::EachCase { .. } | ExploreResultGrainIr::EachIncidence { .. },
            ) => {
                return Err(format!(
                    "source result view `{}` requires grouped grain",
                    view.name
                ));
            }
            (ExploreResultInputIr::Selected, ExploreResultGrainIr::EachIncidence { .. }) => {
                return Err(format!(
                    "result view `{}` uses each-incidence grain over selected cases",
                    view.name
                ));
            }
            (
                ExploreResultInputIr::MechanismIncidence { .. },
                ExploreResultGrainIr::EachCase { .. },
            ) => {
                return Err(format!(
                    "result view `{}` uses each-case grain over mechanism incidences",
                    view.name
                ));
            }
            _ => {}
        }

        let grouped = matches!(
            &view.grain,
            ExploreResultGrainIr::GroupAll { .. } | ExploreResultGrainIr::GroupBy { .. }
        );
        if !view.aggregates.is_empty() && !grouped {
            return Err(format!(
                "result view `{}` uses aggregates without grouped grain",
                view.name
            ));
        }
        if view.having.is_some() && !grouped {
            return Err(format!(
                "result view `{}` uses having without grouped grain",
                view.name
            ));
        }

        if let Some(ExploreResultHavingIr::Varies {
            measure_name,
            measure_index,
            ..
        }) = &view.having
        {
            let Some(measure) = view.measures.get(*measure_index) else {
                return Err(format!(
                    "result view `{}` has an absent measure index {}",
                    view.name, measure_index
                ));
            };
            if measure.name != *measure_name {
                return Err(format!(
                    "result view `{}` resolves having measure `{}` to `{}`",
                    view.name, measure_name, measure.name
                ));
            }
        }

        Ok(())
    }
}

fn starter_value_view_is_compatible(
    view: &ExploreResultViewIr,
    context_ty: &Ty,
    before_ty: &Ty,
    after_ty: &Ty,
) -> bool {
    if !matches!(&view.input, ExploreResultInputIr::Selected)
        || !matches!(&view.grain, ExploreResultGrainIr::EachCase { .. })
        || !view.aggregates.is_empty()
        || view.having.is_some()
        || view.choose.is_some()
    {
        return false;
    }

    let mut roles = [false; 4];
    for field in view.select.iter() {
        let ExprKind::Var(binding) = &field.value.kind else {
            continue;
        };
        let role = match binding.as_str() {
            "case_id" if matches!(&field.ty, Ty::Name(name) if name == "CaseId") => Some(0),
            "context" if relational_tys_equivalent(&field.ty, context_ty) => Some(1),
            "before" if relational_tys_equivalent(&field.ty, before_ty) => Some(2),
            "after" if relational_tys_equivalent(&field.ty, after_ty) => Some(3),
            "case_id" | "context" | "before" | "after" => return false,
            _ => None,
        };
        if let Some(role) = role {
            if roles[role] {
                return false;
            }
            roles[role] = true;
        }
    }
    roles.into_iter().all(|present| present)
}

impl ExploreSourceRelationIr {
    fn validate(&self) -> Result<(), String> {
        if self.normalization_version != EXPLORE_RELATION_NORMALIZATION_VERSION {
            return Err(format!(
                "source relation normalization version {} is unsupported; expected {}",
                self.normalization_version, EXPLORE_RELATION_NORMALIZATION_VERSION
            ));
        }

        let mut context_count = 0usize;
        let mut before_count = 0usize;
        let mut binding_names = BTreeSet::new();

        for (expected, binding) in self.bindings.iter().enumerate() {
            if binding.binding_index != expected {
                return Err(format!(
                    "source binding `{}` has canonical index {}, expected {}",
                    binding.name, binding.binding_index, expected
                ));
            }
            if !binding_names.insert(binding.name.as_str()) {
                return Err(format!("duplicate source binding name `{}`", binding.name));
            }

            let mut previous_dependency = None;
            for dependency in binding.dependencies.iter() {
                if dependency.binding_index >= expected {
                    return Err(format!(
                        "source binding `{}` depends on non-earlier binding index {}",
                        binding.name, dependency.binding_index
                    ));
                }
                if previous_dependency.is_some_and(|index| dependency.binding_index <= index) {
                    return Err(format!(
                        "source binding `{}` dependencies are not strictly index-sorted",
                        binding.name
                    ));
                }
                let referenced = &self.bindings[dependency.binding_index];
                if referenced.name != dependency.binding_name {
                    return Err(format!(
                        "source binding `{}` resolves dependency `{}` to `{}`",
                        binding.name, dependency.binding_name, referenced.name
                    ));
                }
                previous_dependency = Some(dependency.binding_index);
            }

            match binding.role {
                ExploreSourceBindingRoleIr::Auxiliary => {}
                ExploreSourceBindingRoleIr::Context => {
                    context_count += 1;
                    if binding.binding_index != self.context_binding_index
                        || !relational_tys_equivalent(&binding.value_ty, &self.context_ty)
                    {
                        return Err(
                            "source Context role does not match its canonical index/type"
                                .to_string(),
                        );
                    }
                }
                ExploreSourceBindingRoleIr::Before => {
                    before_count += 1;
                    if binding.binding_index != self.before_binding_index
                        || !relational_tys_equivalent(&binding.value_ty, &self.before_ty)
                    {
                        return Err("source Before role does not match its canonical index/type"
                            .to_string());
                    }
                }
            }
        }

        if context_count != 1 || before_count != 1 {
            return Err(format!(
                "source relation requires exactly one Context and one Before role; found {context_count} and {before_count}"
            ));
        }

        Ok(())
    }
}
