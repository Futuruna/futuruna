//! Closed relational query descriptors for Explore.
//!
//! This IR preserves the dependency order of the finite source relation and
//! preserves the authored `given`/`vary`/`let` producer roles, and gives
//! `context`, `before`, and each per-source `after` value semantic roles. It
//! deliberately contains no Cartesian axes, boundary hints, output mode,
//! probe plan, scheduling policy, or rank-derived identity.

use std::collections::BTreeSet;

use super::{
    ExploreExactDomain, FindPolarity, StructuralEdgeId, StructuralMechanismId, StructuralNodeId,
};
use crate::{
    ExploreAdmissionScope, ExploreChooseCardinality, ExploreMechanismSupportFacet,
    ExploreOptimizeDirection, ExploreRelationMultiplicity, Expr, ExprKind, Span, Ty,
    TypedExploreMechanismSupportSubject, TypedExploreStarterProjection,
    TypedExploreSupportObservationDemand, TypedExploreTransitionGraph,
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
pub const EXPLORE_RELATIONAL_IR_VERSION: u32 = 3;

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

/// How an authored source binding produces its value.
///
/// This is deliberately separate from [`ExploreSourceBindingRoleIr`]: a
/// producer role records whether the author conditioned, varied, or derived a
/// binding, while the binding role records how the resulting value
/// participates in the canonical `(Context, Before)` source row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreSourceProducerRoleIr {
    Given,
    Vary,
    Let,
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
    pub producer_role: ExploreSourceProducerRoleIr,
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

/// One authored address for a closed FIND question.
///
/// `name` is deliberately kept outside [`ExploreFindIr`]: it resolves
/// references within the declaration but is not part of the question's
/// semantic identity.
#[derive(Debug, Clone)]
pub struct ExploreNamedFindIr {
    pub name: String,
    pub find: ExploreFindIr,
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
    /// Cases admitted and classified by one explicitly addressed FIND
    /// question. The positional reference is closed during type checking;
    /// the authored name remains presentation metadata and never enters the
    /// semantic [`super::ViewId`].
    Find {
        find_name: String,
        find_index: usize,
    },
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

/// Canonical partition of a semantic choice relation. The transitional
/// nested result spelling maps both `each case` and `group all` to one global
/// partition; display grain remains a property of the downstream view.
#[derive(Debug, Clone)]
pub enum ExploreChoicePartitionIr {
    All {
        span: Span,
    },
    By {
        fields: Box<[ExploreResultFieldIr]>,
        span: Span,
    },
}

/// Typed semantic choice relation lowered from the current nested `choose`
/// spelling. It deliberately owns no aggregate, SELECT, display, or privacy
/// fields. The existing concrete result reducer may execute this relation and
/// its display view as one fused physical stage without creating a second
/// evaluator.
#[derive(Debug, Clone)]
pub struct ExploreChoiceRelationIr {
    pub find_name: String,
    pub find_index: usize,
    pub partition: ExploreChoicePartitionIr,
    pub measures: Box<[ExploreResultFieldIr]>,
    pub having: Option<ExploreResultHavingIr>,
    pub policy: ExploreResultChoiceIr,
    pub span: Span,
}

/// One named result node over source rows, an explicitly addressed FIND case
/// relation, or a prior mechanism-incidence relation.
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

impl ExploreResultViewIr {
    /// Lower the transitional nested spelling directly to the canonical
    /// semantic choice relation consumed by identity and planning. Keeping
    /// this derivation on the typed IR prevents publication or the runtime
    /// from reconstructing choice semantics from display rows.
    pub fn canonical_choice_relation(&self) -> Result<Option<ExploreChoiceRelationIr>, String> {
        let Some(policy) = self.choose.clone() else {
            return Ok(None);
        };
        let ExploreResultInputIr::Find {
            find_name,
            find_index,
        } = &self.input
        else {
            return Err(format!(
                "result view `{}` may choose only from a FIND case relation",
                self.name
            ));
        };
        let partition = match &self.grain {
            ExploreResultGrainIr::EachCase { span } | ExploreResultGrainIr::GroupAll { span } => {
                ExploreChoicePartitionIr::All { span: *span }
            }
            ExploreResultGrainIr::GroupBy { fields, span } => ExploreChoicePartitionIr::By {
                fields: fields.clone(),
                span: *span,
            },
            ExploreResultGrainIr::EachIncidence { .. } => {
                return Err(format!(
                    "result view `{}` cannot choose from incidence grain",
                    self.name
                ));
            }
        };
        Ok(Some(ExploreChoiceRelationIr {
            find_name: find_name.clone(),
            find_index: *find_index,
            partition,
            measures: self.measures.clone(),
            having: self.having.clone(),
            policy,
            span: self.span,
        }))
    }
}

/// Resolved case population consumed by a mechanism request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExploreMechanismTargetIr {
    /// Cases classified by one explicitly addressed FIND question.
    Find { find_index: usize },
    /// The closed reference is positional and strictly prior. The view's name
    /// remains only on its descriptor and cannot enter target identity.
    ViewChosen { view_node_index: usize },
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
pub(crate) enum ExploreMechanismSupportFacetIr {
    Activation,
    DifferentialParticipation,
}

impl From<ExploreMechanismSupportFacet> for ExploreMechanismSupportFacetIr {
    fn from(facet: ExploreMechanismSupportFacet) -> Self {
        match facet {
            ExploreMechanismSupportFacet::Activation => Self::Activation,
            ExploreMechanismSupportFacet::DifferentialParticipation => {
                Self::DifferentialParticipation
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ExploreMechanismSupportSubjectIr {
    Mechanism(StructuralMechanismId),
    Node {
        facet: ExploreMechanismSupportFacetIr,
        node_id: StructuralNodeId,
    },
    Edge {
        facet: ExploreMechanismSupportFacetIr,
        edge_id: StructuralEdgeId,
    },
}

impl From<TypedExploreMechanismSupportSubject> for ExploreMechanismSupportSubjectIr {
    fn from(subject: TypedExploreMechanismSupportSubject) -> Self {
        match subject {
            TypedExploreMechanismSupportSubject::Mechanism(mechanism_id) => {
                Self::Mechanism(mechanism_id)
            }
            TypedExploreMechanismSupportSubject::Node { facet, node_id } => Self::Node {
                facet: facet.into(),
                node_id,
            },
            TypedExploreMechanismSupportSubject::Edge { facet, edge_id } => Self::Edge {
                facet: facet.into(),
                edge_id,
            },
        }
    }
}

/// One checked compact support-observation demand. The declaration addresses
/// an earlier mechanism request but remains outside its semantic analysis DAG.
#[derive(Debug, Clone)]
pub(crate) struct ExploreSupportObservationDemandIr {
    pub(crate) name: String,
    pub(crate) request_node_index: usize,
    pub(crate) subject: ExploreMechanismSupportSubjectIr,
    pub(crate) within_mechanism: Option<StructuralMechanismId>,
    pub(crate) span: Span,
}

impl ExploreSupportObservationDemandIr {
    pub(crate) fn lower(demand: &TypedExploreSupportObservationDemand) -> Self {
        Self {
            name: demand.name.clone(),
            request_node_index: demand.request_node_index,
            subject: demand.subject.into(),
            within_mechanism: demand.within_mechanism,
            span: demand.span,
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
    pub(crate) subject: ExploreMechanismSupportSubjectIr,
    pub(crate) within_mechanism: Option<StructuralMechanismId>,
    pub(crate) value_view_node_index: usize,
    pub(crate) span: Span,
}

impl ExploreStarterProjectionIr {
    pub(crate) fn lower(projection: &TypedExploreStarterProjection) -> Self {
        Self {
            name: projection.name.clone(),
            request_node_index: projection.request_node_index,
            subject: projection.subject.into(),
            within_mechanism: projection.within_mechanism,
            value_view_node_index: projection.value_view_node_index,
            span: projection.span,
        }
    }
}

/// One explicitly named, identity-only publication of the complete semantic
/// transition relation. It remains outside the analysis DAG so attaching,
/// removing, or renaming it cannot perturb any upstream semantic identity.
#[derive(Debug, Clone)]
pub(crate) struct ExploreTransitionGraphIr {
    pub(crate) name: String,
    pub(crate) span: Span,
}

impl ExploreTransitionGraphIr {
    pub(crate) fn lower(graph: &TypedExploreTransitionGraph) -> Self {
        Self {
            name: graph.name.clone(),
            span: graph.span,
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
    pub finds: Box<[ExploreNamedFindIr]>,
    pub analysis: Box<[ExploreAnalysisNodeIr]>,
    pub(crate) observation_demands: Box<[ExploreSupportObservationDemandIr]>,
    pub(crate) starter_projections: Box<[ExploreStarterProjectionIr]>,
    pub(crate) transition_graphs: Box<[ExploreTransitionGraphIr]>,
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

        self.validate_declaration_names()?;

        self.validate_analysis()?;
        self.validate_observation_demands()?;
        self.validate_starter_projections()?;
        self.validate_transition_graphs()?;

        Ok(())
    }

    fn validate_declaration_names(&self) -> Result<(), String> {
        let mut names = BTreeSet::new();
        for find in self.finds.iter() {
            if !names.insert(find.name.as_str()) {
                return Err(format!("duplicate exploration declaration `{}`", find.name));
            }
        }
        for name in self
            .analysis
            .iter()
            .map(ExploreAnalysisNodeIr::name)
            .chain(
                self.observation_demands
                    .iter()
                    .map(|demand| demand.name.as_str()),
            )
            .chain(
                self.starter_projections
                    .iter()
                    .map(|projection| projection.name.as_str()),
            )
            .chain(
                self.transition_graphs
                    .iter()
                    .map(|graph| graph.name.as_str()),
            )
        {
            if !names.insert(name) {
                return Err(format!("duplicate exploration declaration `{name}`"));
            }
        }
        Ok(())
    }

    fn validate_observation_demands(&self) -> Result<(), String> {
        let mut names = self
            .analysis
            .iter()
            .map(ExploreAnalysisNodeIr::name)
            .collect::<BTreeSet<_>>();
        for demand in self.observation_demands.iter() {
            if !names.insert(demand.name.as_str()) {
                return Err(format!(
                    "duplicate exploration declaration `{}`",
                    demand.name
                ));
            }
            if !matches!(
                self.analysis.get(demand.request_node_index),
                Some(ExploreAnalysisNodeIr::Mechanisms(_))
            ) {
                return Err(format!(
                    "support observation `{}` does not resolve mechanism node index {}",
                    demand.name, demand.request_node_index
                ));
            }
            if demand.within_mechanism.is_some()
                && matches!(
                    demand.subject,
                    ExploreMechanismSupportSubjectIr::Mechanism(_)
                )
            {
                return Err(format!(
                    "support observation `{}` cannot refine a whole mechanism within another mechanism",
                    demand.name
                ));
            }
        }
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
                ExploreAnalysisNodeIr::Mechanisms(request) => match &request.target {
                    ExploreMechanismTargetIr::Find { find_index } => {
                        if *find_index >= self.finds.len() {
                            return Err(format!(
                                "mechanism request `{}` targets absent FIND question index {}",
                                request.name, find_index
                            ));
                        }
                    }
                    ExploreMechanismTargetIr::ViewChosen { view_node_index } => {
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
                        if view.canonical_choice_relation()?.is_none() {
                            return Err(format!(
                                "mechanism request `{}` must target a semantic choice relation",
                                request.name
                            ));
                        }
                    }
                },
            }
        }
        Ok(())
    }

    fn validate_starter_projections(&self) -> Result<(), String> {
        let mut names = self
            .analysis
            .iter()
            .map(ExploreAnalysisNodeIr::name)
            .chain(
                self.observation_demands
                    .iter()
                    .map(|demand| demand.name.as_str()),
            )
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
            if projection.within_mechanism.is_some()
                && matches!(
                    projection.subject,
                    ExploreMechanismSupportSubjectIr::Mechanism(_)
                )
            {
                return Err(format!(
                    "starter projection `{}` cannot refine a whole mechanism within another mechanism",
                    projection.name
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
                    "starter projection `{}` requires a lossless FIND-backed each-case value view",
                    projection.name
                ));
            }
        }
        Ok(())
    }

    fn validate_transition_graphs(&self) -> Result<(), String> {
        let mut names = self
            .analysis
            .iter()
            .map(ExploreAnalysisNodeIr::name)
            .chain(
                self.observation_demands
                    .iter()
                    .map(|demand| demand.name.as_str()),
            )
            .chain(
                self.starter_projections
                    .iter()
                    .map(|projection| projection.name.as_str()),
            )
            .collect::<BTreeSet<_>>();
        for graph in self.transition_graphs.iter() {
            if !names.insert(graph.name.as_str()) {
                return Err(format!(
                    "duplicate exploration declaration `{}`",
                    graph.name
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
        // This is also the fail-closed lowering boundary for the transitional
        // nested syntax: every authored `choose` must denote one canonical
        // FIND-backed semantic choice relation.
        view.canonical_choice_relation()?;
        match &view.input {
            ExploreResultInputIr::Find {
                find_name,
                find_index,
            } => {
                let Some(find) = self.finds.get(*find_index) else {
                    return Err(format!(
                        "result view `{}` consumes absent FIND question index {}",
                        view.name, find_index
                    ));
                };
                if find.name != *find_name {
                    return Err(format!(
                        "result view `{}` addresses FIND `{find_name}` but index {find_index} resolves `{}`",
                        view.name, find.name
                    ));
                }
            }
            ExploreResultInputIr::MechanismIncidence { request_node_index } => {
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
            ExploreResultInputIr::Sources => {}
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
            (ExploreResultInputIr::Find { .. }, ExploreResultGrainIr::EachIncidence { .. }) => {
                return Err(format!(
                    "result view `{}` uses each-incidence grain over FIND cases",
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
    if !matches!(&view.input, ExploreResultInputIr::Find { .. })
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

            match (binding.producer_role, &binding.kind) {
                (
                    ExploreSourceProducerRoleIr::Given | ExploreSourceProducerRoleIr::Let,
                    ExploreSourceBindingKindIr::Singleton { .. },
                )
                | (ExploreSourceProducerRoleIr::Vary, ExploreSourceBindingKindIr::Finite { .. }) => {
                }
                (producer_role, kind) => {
                    let expected_kind = match producer_role {
                        ExploreSourceProducerRoleIr::Given | ExploreSourceProducerRoleIr::Let => {
                            "a singleton value"
                        }
                        ExploreSourceProducerRoleIr::Vary => "a finite domain",
                    };
                    let actual_kind = match kind {
                        ExploreSourceBindingKindIr::Singleton { .. } => "a singleton value",
                        ExploreSourceBindingKindIr::Finite { .. } => "a finite domain",
                    };
                    return Err(format!(
                        "source binding `{}` has producer role {producer_role:?}, which requires {expected_kind}, but carries {actual_kind}",
                        binding.name
                    ));
                }
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
            if binding.producer_role == ExploreSourceProducerRoleIr::Given {
                if let Some(dependency) = binding.dependencies.first() {
                    return Err(format!(
                        "source `given {}` depends on earlier source binding `{}`; use `let` for values derived from source bindings",
                        binding.name, dependency.binding_name
                    ));
                }
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
