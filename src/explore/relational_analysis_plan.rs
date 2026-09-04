//! Canonical analysis-layer DAG planning for checked relational Explore.
//!
//! This layer owns no parser names, spans, source paths, runtime resources, or
//! scheduling state. It resolves the normalized positional IR against the
//! producer-minted identities carried by [`CheckedExploreQueryView`], validates
//! every prior-node reference, cross-checks the producer's analysis-graph
//! digest, and emits canonical layer registrations for journal bootstrap.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::choice_relation::ChoiceRelationSpec;
use super::mechanism::{MechanismObservationIr, MechanismSiteId};
use super::relation::{ChoiceId, MechanismRequestId, QuestionId, RelationId, ViewId};
use super::relational_endpoint_totality::{
    RelationalEndpointTotalityCertificate, RelationalEndpointTotalityCertificateId,
};
use super::relational_ir::{
    ExploreAggregateReducerIr, ExploreAnalysisNodeIr, ExploreChoicePartitionIr,
    ExploreChoiceRelationIr, ExploreMechanismTargetIr, ExploreResultChoiceIr, ExploreResultGrainIr,
    ExploreResultHavingIr, ExploreResultInputIr, ExploreResultViewIr,
};
use crate::{
    CheckedExploreAnalysisIdentity, CheckedExploreQueryView, ExploreChooseCardinality,
    ExploreOptimizeDirection,
};

pub(crate) const RELATIONAL_ANALYSIS_PLAN_VERSION: u32 = 6;

const ANALYSIS_PLAN_ROOT_V6: &[u8] = b"futuruna.explore.relational-analysis.plan-root.v6";
const CHOICE_SPEC_DIGEST_V1: &[u8] = b"futuruna.explore.relational-analysis.choice-spec.v1";
const RESULT_SPEC_DIGEST_V2: &[u8] = b"futuruna.explore.relational-analysis.result-spec.v2";
const OBSERVATION_ID_V1: &[u8] = b"futuruna.explore.relational-analysis.observation-id.v1";
const OBSERVATION_DIGEST_V2: &[u8] = b"futuruna.explore.relational-analysis.observation-digest.v2";
const CHECKED_ANALYSIS_GRAPH_V4: &[u8] = b"futuruna.checked-explore-analysis-graph.v4\0";
const CHECKED_ANALYSIS_MECHANISM_NODE_V2: &[u8] =
    b"futuruna.checked-explore-analysis-mechanism-node.v2\0";

/// Typed copy of the producer's canonical checked-analysis graph digest.
/// Keeping this distinct from every other 32-byte commitment prevents journal
/// bootstrap code from accidentally substituting a plan or layer root.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalCheckedAnalysisGraphDigest([u8; 32]);

impl RelationalCheckedAnalysisGraphDigest {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Content identity of a complete, canonical analysis registration DAG.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalAnalysisPlanRoot([u8; 32]);

impl RelationalAnalysisPlanRoot {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Opaque semantic seal for a normalized result specification. The producer's
/// `ViewId` seals expressions and checked types; this digest additionally
/// commits the name/span-free IR shape, resolved input, and direct dependency
/// IDs used by the analysis planner.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalResultSpecDigest([u8; 32]);

impl RelationalResultSpecDigest {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Opaque planner seal for one canonical semantic choice relation. ChoiceId
/// already binds checked expressions and types; this digest additionally
/// commits the name/span-free IR shape used by the journaled Choice reducer.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalChoiceSpecDigest([u8; 32]);

impl RelationalChoiceSpecDigest {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Stable semantic identity of the checked mechanism endpoint template.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalMechanismObservationId([u8; 32]);

impl RelationalMechanismObservationId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Request-scoped semantic seal for an authorized observation. V2 binds both
/// the producer-minted request identity and the endpoint-totality certificate
/// which must authorize replay under that request.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalMechanismObservationDigest([u8; 32]);

impl RelationalMechanismObservationDigest {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum RelationalAnalysisLayerId {
    Choice(ChoiceId),
    Result(ViewId),
    Mechanisms(MechanismRequestId),
}

impl RelationalAnalysisLayerId {
    pub(crate) const fn identity_bytes(self) -> [u8; 32] {
        match self {
            Self::Choice(choice_id) => choice_id.bytes(),
            Self::Result(view_id) => view_id.bytes(),
            Self::Mechanisms(request_id) => request_id.bytes(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum RelationalAnalysisDependencyId {
    Relation(RelationId),
    Question(QuestionId),
    Choice(ChoiceId),
    Result(ViewId),
    Mechanisms(MechanismRequestId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalResolvedResultInput {
    Sources(RelationId),
    Selected(QuestionId),
    Choice(ChoiceId),
    MechanismIncidence(MechanismRequestId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalResolvedMechanismTarget {
    Selected(QuestionId),
    Choice(ChoiceId),
}

/// One first-class semantic choice relation in the analysis plan. The
/// relation is independently identified even though the current concrete
/// executor fuses it with one downstream display view.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalChoiceRegistration {
    choice_id: ChoiceId,
    input_question_id: QuestionId,
    semantic_spec_digest: RelationalChoiceSpecDigest,
    spec: ChoiceRelationSpec,
    dependencies: Box<[RelationalAnalysisDependencyId]>,
}

impl RelationalChoiceRegistration {
    pub(super) fn restore_from_journal_codec(
        choice_id: ChoiceId,
        input_question_id: QuestionId,
        semantic_spec_digest: RelationalChoiceSpecDigest,
        spec: ChoiceRelationSpec,
        dependencies: Box<[RelationalAnalysisDependencyId]>,
    ) -> Self {
        Self {
            choice_id,
            input_question_id,
            semantic_spec_digest,
            spec,
            dependencies,
        }
    }

    pub(crate) const fn choice_id(&self) -> ChoiceId {
        self.choice_id
    }

    pub(crate) const fn input_question_id(&self) -> QuestionId {
        self.input_question_id
    }

    pub(crate) const fn semantic_spec_digest(&self) -> RelationalChoiceSpecDigest {
        self.semantic_spec_digest
    }

    pub(crate) const fn spec(&self) -> &ChoiceRelationSpec {
        &self.spec
    }

    pub(crate) fn dependencies(&self) -> &[RelationalAnalysisDependencyId] {
        &self.dependencies
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalResultLayerRegistration {
    view_id: ViewId,
    choice_id: Option<ChoiceId>,
    input: RelationalResolvedResultInput,
    semantic_spec_digest: RelationalResultSpecDigest,
    dependencies: Box<[RelationalAnalysisDependencyId]>,
}

impl RelationalResultLayerRegistration {
    pub(super) fn restore_from_journal_codec(
        view_id: ViewId,
        choice_id: Option<ChoiceId>,
        input: RelationalResolvedResultInput,
        semantic_spec_digest: RelationalResultSpecDigest,
        dependencies: Box<[RelationalAnalysisDependencyId]>,
    ) -> Self {
        Self {
            view_id,
            choice_id,
            input,
            semantic_spec_digest,
            dependencies,
        }
    }

    pub(crate) const fn view_id(&self) -> ViewId {
        self.view_id
    }

    pub(crate) const fn choice_id(&self) -> Option<ChoiceId> {
        self.choice_id
    }

    pub(crate) const fn input(&self) -> RelationalResolvedResultInput {
        self.input
    }

    pub(crate) const fn semantic_spec_digest(&self) -> RelationalResultSpecDigest {
        self.semantic_spec_digest
    }

    pub(crate) fn dependencies(&self) -> &[RelationalAnalysisDependencyId] {
        &self.dependencies
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalMechanismLayerRegistration {
    request_id: MechanismRequestId,
    target: RelationalResolvedMechanismTarget,
    observation_id: RelationalMechanismObservationId,
    endpoint_totality_certificate_id: RelationalEndpointTotalityCertificateId,
    observation_digest: RelationalMechanismObservationDigest,
    dependencies: Box<[RelationalAnalysisDependencyId]>,
}

impl RelationalMechanismLayerRegistration {
    pub(super) fn restore_from_journal_codec(
        request_id: MechanismRequestId,
        target: RelationalResolvedMechanismTarget,
        observation_id: RelationalMechanismObservationId,
        endpoint_totality_certificate_id: RelationalEndpointTotalityCertificateId,
        dependencies: Box<[RelationalAnalysisDependencyId]>,
    ) -> Self {
        let observation_digest = derive_observation_digest(
            request_id,
            target,
            observation_id,
            endpoint_totality_certificate_id,
            &dependencies,
        );
        Self {
            request_id,
            target,
            observation_id,
            endpoint_totality_certificate_id,
            observation_digest,
            dependencies,
        }
    }

    pub(crate) const fn request_id(&self) -> MechanismRequestId {
        self.request_id
    }

    pub(crate) const fn target(&self) -> RelationalResolvedMechanismTarget {
        self.target
    }

    pub(crate) const fn observation_id(&self) -> RelationalMechanismObservationId {
        self.observation_id
    }

    pub(crate) const fn endpoint_totality_certificate_id(
        &self,
    ) -> RelationalEndpointTotalityCertificateId {
        self.endpoint_totality_certificate_id
    }

    pub(crate) const fn observation_digest(&self) -> RelationalMechanismObservationDigest {
        self.observation_digest
    }

    pub(crate) fn dependencies(&self) -> &[RelationalAnalysisDependencyId] {
        &self.dependencies
    }
}

/// One journal-bootstrap registration. The slice stored by the plan is sorted
/// by semantic layer ID, not authored declaration order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalAnalysisLayerRegistration {
    Result(RelationalResultLayerRegistration),
    Mechanisms(RelationalMechanismLayerRegistration),
}

impl RelationalAnalysisLayerRegistration {
    pub(crate) const fn layer_id(&self) -> RelationalAnalysisLayerId {
        match self {
            Self::Result(result) => RelationalAnalysisLayerId::Result(result.view_id),
            Self::Mechanisms(request) => RelationalAnalysisLayerId::Mechanisms(request.request_id),
        }
    }

    pub(crate) fn dependencies(&self) -> &[RelationalAnalysisDependencyId] {
        match self {
            Self::Result(result) => result.dependencies(),
            Self::Mechanisms(request) => request.dependencies(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalAnalysisPlan {
    root: RelationalAnalysisPlanRoot,
    payload: RelationalAnalysisPlanPayload,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RelationalAnalysisPlanPayload {
    question_ids: Box<[QuestionId]>,
    producer_graph_digest: RelationalCheckedAnalysisGraphDigest,
    choices: Box<[RelationalChoiceRegistration]>,
    registrations: Box<[RelationalAnalysisLayerRegistration]>,
}

impl RelationalAnalysisPlan {
    /// Construct only from the immutable joined checked-query boundary. The
    /// producer graph digest is recomputed over the resolved layer identities
    /// and compared before an owned plan is returned.
    pub(crate) fn from_checked(
        checked: &CheckedExploreQueryView<'_>,
    ) -> Result<Self, RelationalAnalysisPlanError> {
        checked
            .closed_query
            .validate()
            .map_err(RelationalAnalysisPlanError::InvalidQuery)?;
        let analysis_nodes = checked.analysis_nodes();
        if checked.closed_query.analysis.len() != analysis_nodes.len() {
            return Err(RelationalAnalysisPlanError::IdentityCountMismatch {
                nodes: checked.closed_query.analysis.len(),
                identities: analysis_nodes.len(),
            });
        }

        let question_ids = checked.question_ids();
        let find_question_ids = checked.find_question_ids();
        let relation_id = checked.relation_id();
        let mut resolved_by_position =
            Vec::<RelationalAnalysisLayerId>::with_capacity(checked.closed_query.analysis.len());
        let mut resolved_choice_by_position =
            Vec::<Option<ChoiceId>>::with_capacity(checked.closed_query.analysis.len());
        let mut choices = BTreeMap::<ChoiceId, RelationalChoiceRegistration>::new();
        let mut registrations =
            BTreeMap::<RelationalAnalysisLayerId, RelationalAnalysisLayerRegistration>::new();

        for (node_index, (node, identity)) in checked.analysis_nodes().enumerate() {
            if node.node_index() != node_index {
                return Err(RelationalAnalysisPlanError::NonCanonicalNodeIndex {
                    actual: node.node_index(),
                    expected: node_index,
                });
            }
            let registration = match (node, identity) {
                (
                    ExploreAnalysisNodeIr::Result(view),
                    CheckedExploreAnalysisIdentity::View { view_id, choice_id },
                ) => {
                    if let Some(choice_id) = choice_id {
                        let choice = build_choice_registration(
                            node_index,
                            view,
                            *choice_id,
                            find_question_ids,
                        )?;
                        match choices.get(choice_id) {
                            Some(existing) if existing == &choice => {}
                            Some(_) => {
                                return Err(RelationalAnalysisPlanError::ChoiceIdentityCollision(
                                    *choice_id,
                                ));
                            }
                            None => {
                                choices.insert(*choice_id, choice);
                            }
                        }
                    }
                    build_result_registration(
                        node_index,
                        view,
                        *view_id,
                        *choice_id,
                        relation_id,
                        find_question_ids,
                        &resolved_by_position,
                    )?
                }
                (
                    ExploreAnalysisNodeIr::Mechanisms(request),
                    CheckedExploreAnalysisIdentity::Mechanisms {
                        request_id,
                        observation,
                        endpoint_totality,
                    },
                ) => build_mechanism_registration(
                    node_index,
                    request.target.clone(),
                    *request_id,
                    observation,
                    endpoint_totality,
                    relation_id,
                    find_question_ids,
                    &checked.closed_query.analysis,
                    &resolved_by_position,
                    &resolved_choice_by_position,
                )?,
                _ => {
                    return Err(RelationalAnalysisPlanError::IdentityKindMismatch { node_index });
                }
            };
            let layer_id = registration.layer_id();
            match registrations.get(&layer_id) {
                Some(existing) if existing == &registration => {}
                Some(_) => {
                    return Err(RelationalAnalysisPlanError::LayerIdentityCollision(
                        layer_id,
                    ));
                }
                None => {
                    registrations.insert(layer_id, registration);
                }
            }
            resolved_by_position.push(layer_id);
            resolved_choice_by_position.push(match identity {
                CheckedExploreAnalysisIdentity::View { choice_id, .. } => *choice_id,
                CheckedExploreAnalysisIdentity::Mechanisms { .. } => None,
            });
        }

        let choices = choices.into_values().collect::<Vec<_>>();
        let registrations = registrations.into_values().collect::<Vec<_>>();
        assemble_plan(
            question_ids,
            checked.analysis_graph_hash(),
            choices,
            registrations,
        )
    }

    fn from_payload(payload: RelationalAnalysisPlanPayload) -> Self {
        let root = derive_analysis_plan_root(&payload);
        Self { root, payload }
    }

    pub(super) fn restore_from_journal_codec(
        question_ids: Box<[QuestionId]>,
        producer_graph_digest: RelationalCheckedAnalysisGraphDigest,
        choices: Vec<RelationalChoiceRegistration>,
        registrations: Vec<RelationalAnalysisLayerRegistration>,
    ) -> Result<Self, RelationalAnalysisPlanError> {
        let choices = canonicalize_choices(choices)?;
        let registrations = canonicalize_registrations(registrations)?;
        validate_question_ids(&question_ids)?;
        validate_choice_dependencies(&question_ids, &choices)?;
        validate_registration_dependencies(&question_ids, &choices, &registrations)?;
        let derived_graph_digest = RelationalCheckedAnalysisGraphDigest(
            derive_checked_analysis_graph_digest(&choices, &registrations),
        );
        if producer_graph_digest != derived_graph_digest {
            return Err(RelationalAnalysisPlanError::AnalysisGraphDigestMismatch {
                producer: producer_graph_digest,
                derived: derived_graph_digest,
            });
        }
        Ok(Self::from_payload(RelationalAnalysisPlanPayload {
            question_ids,
            producer_graph_digest,
            choices,
            registrations,
        }))
    }

    pub(crate) const fn root(&self) -> RelationalAnalysisPlanRoot {
        self.root
    }

    pub(crate) fn validate_root(&self) -> bool {
        self.root == derive_analysis_plan_root(&self.payload)
    }

    pub(crate) fn question_ids(&self) -> &[QuestionId] {
        &self.payload.question_ids
    }

    pub(crate) const fn producer_graph_digest(&self) -> RelationalCheckedAnalysisGraphDigest {
        self.payload.producer_graph_digest
    }

    pub(crate) fn layer_registrations(&self) -> &[RelationalAnalysisLayerRegistration] {
        &self.payload.registrations
    }

    pub(crate) fn choice_registrations(&self) -> &[RelationalChoiceRegistration] {
        &self.payload.choices
    }

    pub(crate) fn choice_registration(
        &self,
        choice_id: ChoiceId,
    ) -> Option<&RelationalChoiceRegistration> {
        self.payload
            .choices
            .binary_search_by_key(&choice_id, RelationalChoiceRegistration::choice_id)
            .ok()
            .map(|index| &self.payload.choices[index])
    }

    pub(crate) fn registration(
        &self,
        layer_id: RelationalAnalysisLayerId,
    ) -> Option<&RelationalAnalysisLayerRegistration> {
        self.payload
            .registrations
            .binary_search_by_key(&layer_id, RelationalAnalysisLayerRegistration::layer_id)
            .ok()
            .map(|index| &self.payload.registrations[index])
    }
}

fn build_choice_registration(
    node_index: usize,
    view: &ExploreResultViewIr,
    choice_id: ChoiceId,
    find_question_ids: &[QuestionId],
) -> Result<RelationalChoiceRegistration, RelationalAnalysisPlanError> {
    let choice = view
        .canonical_choice_relation()
        .map_err(RelationalAnalysisPlanError::InvalidQuery)?
        .ok_or(RelationalAnalysisPlanError::MissingChoiceRelation { node_index })?;
    let input_question_id = find_question_ids.get(choice.find_index).copied().ok_or(
        RelationalAnalysisPlanError::UnknownFindIndex {
            node_index,
            find_index: choice.find_index,
        },
    )?;
    let dependencies =
        canonical_dependencies([RelationalAnalysisDependencyId::Question(input_question_id)]);
    let semantic_spec_digest =
        derive_choice_spec_digest(choice_id, input_question_id, &dependencies, &choice);
    let partition_value_count = match &choice.partition {
        ExploreChoicePartitionIr::All { .. } => 0,
        ExploreChoicePartitionIr::By { fields, .. } => fields.len(),
    };
    let having = choice.having.as_ref().map(|having| match having {
        ExploreResultHavingIr::Varies { measure_index, .. } => {
            super::result_view::ResultViewHaving::Varies {
                measure_index: *measure_index,
            }
        }
    });
    let policy = match &choice.policy {
        ExploreResultChoiceIr::Optimize {
            cardinality,
            direction,
            ..
        } => super::result_view::ResultViewChoice::Optimize {
            cardinality: *cardinality,
            direction: *direction,
        },
        ExploreResultChoiceIr::Pareto { objectives, .. } => {
            super::result_view::ResultViewChoice::Pareto {
                directions: objectives
                    .iter()
                    .map(|objective| objective.direction)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            }
        }
    };
    let spec = ChoiceRelationSpec::new(
        choice_id,
        partition_value_count,
        choice.measures.len(),
        having,
        policy,
    )
    .map_err(|error| RelationalAnalysisPlanError::InvalidChoiceSpec(error.to_string()))?;
    Ok(RelationalChoiceRegistration {
        choice_id,
        input_question_id,
        semantic_spec_digest,
        spec,
        dependencies,
    })
}

fn build_result_registration(
    node_index: usize,
    view: &ExploreResultViewIr,
    view_id: ViewId,
    choice_id: Option<ChoiceId>,
    relation_id: RelationId,
    find_question_ids: &[QuestionId],
    resolved_by_position: &[RelationalAnalysisLayerId],
) -> Result<RelationalAnalysisLayerRegistration, RelationalAnalysisPlanError> {
    let authored_input = match &view.input {
        ExploreResultInputIr::Sources => RelationalResolvedResultInput::Sources(relation_id),
        ExploreResultInputIr::Find { find_index, .. } => RelationalResolvedResultInput::Selected(
            find_question_ids.get(*find_index).copied().ok_or(
                RelationalAnalysisPlanError::UnknownFindIndex {
                    node_index,
                    find_index: *find_index,
                },
            )?,
        ),
        ExploreResultInputIr::MechanismIncidence { request_node_index } => {
            let request_node_index = *request_node_index;
            require_prior_reference(node_index, request_node_index)?;
            match resolved_by_position.get(request_node_index).copied() {
                Some(RelationalAnalysisLayerId::Mechanisms(request_id)) => {
                    RelationalResolvedResultInput::MechanismIncidence(request_id)
                }
                Some(_) => {
                    return Err(RelationalAnalysisPlanError::ReferenceKindMismatch {
                        node_index,
                        referenced_index: request_node_index,
                        expected: "mechanism request",
                    });
                }
                None => {
                    return Err(RelationalAnalysisPlanError::ReferenceMissing {
                        node_index,
                        referenced_index: request_node_index,
                    });
                }
            }
        }
    };
    let input = choice_id.map_or(authored_input, RelationalResolvedResultInput::Choice);
    let dependencies = canonical_dependencies([match choice_id {
        Some(choice_id) => RelationalAnalysisDependencyId::Choice(choice_id),
        None => match input {
            RelationalResolvedResultInput::Sources(relation_id) => {
                RelationalAnalysisDependencyId::Relation(relation_id)
            }
            RelationalResolvedResultInput::Selected(question_id) => {
                RelationalAnalysisDependencyId::Question(question_id)
            }
            RelationalResolvedResultInput::Choice(choice_id) => {
                RelationalAnalysisDependencyId::Choice(choice_id)
            }
            RelationalResolvedResultInput::MechanismIncidence(request_id) => {
                RelationalAnalysisDependencyId::Mechanisms(request_id)
            }
        },
    }]);
    let semantic_spec_digest = derive_result_spec_digest(view_id, input, &dependencies, view);
    Ok(RelationalAnalysisLayerRegistration::Result(
        RelationalResultLayerRegistration {
            view_id,
            choice_id,
            input,
            semantic_spec_digest,
            dependencies,
        },
    ))
}

fn build_mechanism_registration(
    node_index: usize,
    target: ExploreMechanismTargetIr,
    request_id: MechanismRequestId,
    observation: &MechanismObservationIr,
    endpoint_totality: &RelationalEndpointTotalityCertificate,
    relation_id: RelationId,
    find_question_ids: &[QuestionId],
    analysis: &[ExploreAnalysisNodeIr],
    resolved_by_position: &[RelationalAnalysisLayerId],
    resolved_choice_by_position: &[Option<ChoiceId>],
) -> Result<RelationalAnalysisLayerRegistration, RelationalAnalysisPlanError> {
    endpoint_totality.validate_identity().map_err(|error| {
        RelationalAnalysisPlanError::InvalidEndpointTotalityCertificate {
            node_index,
            message: error.to_string(),
        }
    })?;
    if endpoint_totality.request_id() != request_id {
        return Err(
            RelationalAnalysisPlanError::EndpointTotalityRequestScopeMismatch {
                node_index,
                expected: request_id,
                actual: endpoint_totality.request_id(),
            },
        );
    }
    if endpoint_totality.relation_id() != relation_id {
        return Err(
            RelationalAnalysisPlanError::EndpointTotalityRelationScopeMismatch {
                node_index,
                expected: relation_id,
                actual: endpoint_totality.relation_id(),
            },
        );
    }
    let endpoint_totality_certificate_id = endpoint_totality.certificate_id();
    let target = match target {
        ExploreMechanismTargetIr::Find { find_index } => {
            RelationalResolvedMechanismTarget::Selected(
                find_question_ids.get(find_index).copied().ok_or(
                    RelationalAnalysisPlanError::UnknownFindIndex {
                        node_index,
                        find_index,
                    },
                )?,
            )
        }
        ExploreMechanismTargetIr::ViewChosen { view_node_index } => {
            require_prior_reference(node_index, view_node_index)?;
            let Some(ExploreAnalysisNodeIr::Result(view)) = analysis.get(view_node_index) else {
                return Err(RelationalAnalysisPlanError::ReferenceKindMismatch {
                    node_index,
                    referenced_index: view_node_index,
                    expected: "chosen selected-case result",
                });
            };
            if !matches!(&view.input, ExploreResultInputIr::Find { .. }) || view.choose.is_none() {
                return Err(
                    RelationalAnalysisPlanError::TargetViewNotChosenSelectedCases {
                        node_index,
                        view_node_index,
                    },
                );
            }
            match resolved_by_position.get(view_node_index).copied() {
                Some(RelationalAnalysisLayerId::Result(_view_id)) => {
                    let choice_id = resolved_choice_by_position
                        .get(view_node_index)
                        .copied()
                        .flatten()
                        .ok_or(RelationalAnalysisPlanError::MissingChoiceRelation {
                            node_index: view_node_index,
                        })?;
                    RelationalResolvedMechanismTarget::Choice(choice_id)
                }
                Some(_) => {
                    return Err(RelationalAnalysisPlanError::ReferenceKindMismatch {
                        node_index,
                        referenced_index: view_node_index,
                        expected: "result view",
                    });
                }
                None => {
                    return Err(RelationalAnalysisPlanError::ReferenceMissing {
                        node_index,
                        referenced_index: view_node_index,
                    });
                }
            }
        }
    };
    let dependencies = canonical_dependencies([match target {
        RelationalResolvedMechanismTarget::Selected(question_id) => {
            RelationalAnalysisDependencyId::Question(question_id)
        }
        RelationalResolvedMechanismTarget::Choice(choice_id) => {
            RelationalAnalysisDependencyId::Choice(choice_id)
        }
    }]);
    let (observation_id, observation_digest) = derive_observation_identity(
        request_id,
        target,
        observation,
        endpoint_totality_certificate_id,
        &dependencies,
    )?;
    Ok(RelationalAnalysisLayerRegistration::Mechanisms(
        RelationalMechanismLayerRegistration {
            request_id,
            target,
            observation_id,
            endpoint_totality_certificate_id,
            observation_digest,
            dependencies,
        },
    ))
}

fn require_prior_reference(
    node_index: usize,
    referenced_index: usize,
) -> Result<(), RelationalAnalysisPlanError> {
    if referenced_index < node_index {
        Ok(())
    } else {
        Err(RelationalAnalysisPlanError::ReferenceNotPrior {
            node_index,
            referenced_index,
        })
    }
}

fn canonical_dependencies(
    dependencies: impl IntoIterator<Item = RelationalAnalysisDependencyId>,
) -> Box<[RelationalAnalysisDependencyId]> {
    dependencies
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

fn derive_choice_spec_digest(
    choice_id: ChoiceId,
    input_question_id: QuestionId,
    dependencies: &[RelationalAnalysisDependencyId],
    choice: &ExploreChoiceRelationIr,
) -> RelationalChoiceSpecDigest {
    let mut hasher = AnalysisHasher::new(CHOICE_SPEC_DIGEST_V1);
    hasher.digest(choice_id.bytes());
    hasher.digest(input_question_id.bytes());
    hash_dependencies(&mut hasher, dependencies);
    match &choice.partition {
        ExploreChoicePartitionIr::All { .. } => hasher.u8(0x01),
        ExploreChoicePartitionIr::By { fields, .. } => {
            hasher.u8(0x02);
            hasher.u128(fields.len() as u128);
        }
    }
    hasher.u128(choice.measures.len() as u128);
    match &choice.having {
        None => hasher.u8(0x01),
        Some(ExploreResultHavingIr::Varies { measure_index, .. }) => {
            hasher.u8(0x02);
            hasher.u128(*measure_index as u128);
        }
    }
    match &choice.policy {
        ExploreResultChoiceIr::Optimize {
            cardinality,
            direction,
            ..
        } => {
            hasher.u8(0x01);
            hash_choose_cardinality(&mut hasher, *cardinality);
            hash_optimize_direction(&mut hasher, *direction);
        }
        ExploreResultChoiceIr::Pareto { objectives, .. } => {
            hasher.u8(0x02);
            hasher.u128(objectives.len() as u128);
            for objective in objectives {
                hash_optimize_direction(&mut hasher, objective.direction);
            }
        }
    }
    RelationalChoiceSpecDigest(hasher.finish())
}

fn derive_result_spec_digest(
    view_id: ViewId,
    input: RelationalResolvedResultInput,
    dependencies: &[RelationalAnalysisDependencyId],
    view: &ExploreResultViewIr,
) -> RelationalResultSpecDigest {
    let mut hasher = AnalysisHasher::new(RESULT_SPEC_DIGEST_V2);
    hasher.digest(view_id.bytes());
    hash_result_input(&mut hasher, input);
    hash_dependencies(&mut hasher, dependencies);
    if matches!(input, RelationalResolvedResultInput::Choice(_)) {
        // GROUP/MEASURE/HAVING/CHOOSE are committed by the ChoiceId input.
        // The downstream display is an each-member public projection only.
        hasher.u8(0x01);
        hasher.u128(0);
        hasher.u128(0);
        hasher.u8(0x01);
        hasher.u128(view.select.len() as u128);
        hasher.u8(0x01);
        return RelationalResultSpecDigest(hasher.finish());
    }
    match &view.grain {
        ExploreResultGrainIr::EachCase { .. } => hasher.u8(0x01),
        ExploreResultGrainIr::EachIncidence { .. } => hasher.u8(0x02),
        ExploreResultGrainIr::GroupAll { .. } => hasher.u8(0x03),
        ExploreResultGrainIr::GroupBy { fields, .. } => {
            hasher.u8(0x04);
            hasher.u128(fields.len() as u128);
        }
    }
    hasher.u128(view.measures.len() as u128);
    hasher.u128(view.aggregates.len() as u128);
    for aggregate in &view.aggregates {
        hasher.u8(match &aggregate.reducer {
            ExploreAggregateReducerIr::CountDistinct { .. } => 0x01,
        });
    }
    match &view.having {
        None => hasher.u8(0x01),
        Some(ExploreResultHavingIr::Varies { measure_index, .. }) => {
            hasher.u8(0x02);
            hasher.u128(*measure_index as u128);
        }
    }
    hasher.u128(view.select.len() as u128);
    match &view.choose {
        None => hasher.u8(0x01),
        Some(ExploreResultChoiceIr::Optimize {
            cardinality,
            direction,
            ..
        }) => {
            hasher.u8(0x02);
            hash_choose_cardinality(&mut hasher, *cardinality);
            hash_optimize_direction(&mut hasher, *direction);
        }
        Some(ExploreResultChoiceIr::Pareto { objectives, .. }) => {
            hasher.u8(0x03);
            hasher.u128(objectives.len() as u128);
            for objective in objectives {
                hash_optimize_direction(&mut hasher, objective.direction);
            }
        }
    }
    RelationalResultSpecDigest(hasher.finish())
}

fn derive_observation_identity(
    request_id: MechanismRequestId,
    target: RelationalResolvedMechanismTarget,
    observation: &MechanismObservationIr,
    endpoint_totality_certificate_id: RelationalEndpointTotalityCertificateId,
    dependencies: &[RelationalAnalysisDependencyId],
) -> Result<
    (
        RelationalMechanismObservationId,
        RelationalMechanismObservationDigest,
    ),
    RelationalAnalysisPlanError,
> {
    let template_site = MechanismSiteId::from_expression_site(&observation.template_site)
        .map_err(|error| RelationalAnalysisPlanError::Observation(error.to_string()))?;
    let mut identity_hasher = AnalysisHasher::new(OBSERVATION_ID_V1);
    identity_hasher.digest(template_site.digest_bytes());
    identity_hasher.u32(observation.normalization_version);
    let observation_id = RelationalMechanismObservationId(identity_hasher.finish());

    let observation_digest = derive_observation_digest(
        request_id,
        target,
        observation_id,
        endpoint_totality_certificate_id,
        dependencies,
    );
    Ok((observation_id, observation_digest))
}

fn derive_observation_digest(
    request_id: MechanismRequestId,
    target: RelationalResolvedMechanismTarget,
    observation_id: RelationalMechanismObservationId,
    endpoint_totality_certificate_id: RelationalEndpointTotalityCertificateId,
    dependencies: &[RelationalAnalysisDependencyId],
) -> RelationalMechanismObservationDigest {
    // Every input is persisted in the mechanism registration so journal
    // decoding can rederive this certificate binding instead of trusting an
    // opaque digest. The request ID already commits the checked observation
    // closure; repeating an unpersisted dependency-root count added no
    // independent authority.
    let mut hasher = AnalysisHasher::new(OBSERVATION_DIGEST_V2);
    hasher.digest(request_id.bytes());
    hasher.digest(observation_id.bytes());
    hasher.digest(endpoint_totality_certificate_id.bytes());
    hash_mechanism_target(&mut hasher, target);
    hash_dependencies(&mut hasher, dependencies);
    RelationalMechanismObservationDigest(hasher.finish())
}

fn assemble_plan(
    question_ids: &[QuestionId],
    producer_graph_hash: &str,
    choices: Vec<RelationalChoiceRegistration>,
    registrations: Vec<RelationalAnalysisLayerRegistration>,
) -> Result<RelationalAnalysisPlan, RelationalAnalysisPlanError> {
    let choices = canonicalize_choices(choices)?;
    let registrations = canonicalize_registrations(registrations)?;
    validate_question_ids(question_ids)?;
    validate_choice_dependencies(question_ids, &choices)?;
    validate_registration_dependencies(question_ids, &choices, &registrations)?;
    let producer_graph_digest =
        RelationalCheckedAnalysisGraphDigest(parse_lowercase_sha256(producer_graph_hash)?);
    let derived_graph_digest = RelationalCheckedAnalysisGraphDigest(
        derive_checked_analysis_graph_digest(&choices, &registrations),
    );
    if producer_graph_digest != derived_graph_digest {
        return Err(RelationalAnalysisPlanError::AnalysisGraphDigestMismatch {
            producer: producer_graph_digest,
            derived: derived_graph_digest,
        });
    }
    Ok(RelationalAnalysisPlan::from_payload(
        RelationalAnalysisPlanPayload {
            question_ids: question_ids.to_vec().into_boxed_slice(),
            producer_graph_digest,
            choices,
            registrations,
        },
    ))
}

fn canonicalize_choices(
    choices: Vec<RelationalChoiceRegistration>,
) -> Result<Box<[RelationalChoiceRegistration]>, RelationalAnalysisPlanError> {
    let mut canonical = BTreeMap::<ChoiceId, RelationalChoiceRegistration>::new();
    for choice in choices {
        match canonical.get(&choice.choice_id) {
            Some(existing) if existing == &choice => {}
            Some(_) => {
                return Err(RelationalAnalysisPlanError::ChoiceIdentityCollision(
                    choice.choice_id,
                ));
            }
            None => {
                canonical.insert(choice.choice_id, choice);
            }
        }
    }
    Ok(canonical
        .into_values()
        .collect::<Vec<_>>()
        .into_boxed_slice())
}

fn canonicalize_registrations(
    registrations: Vec<RelationalAnalysisLayerRegistration>,
) -> Result<Box<[RelationalAnalysisLayerRegistration]>, RelationalAnalysisPlanError> {
    let mut canonical =
        BTreeMap::<RelationalAnalysisLayerId, RelationalAnalysisLayerRegistration>::new();
    for registration in registrations {
        if let RelationalAnalysisLayerRegistration::Mechanisms(request) = &registration {
            let expected = derive_observation_digest(
                request.request_id,
                request.target,
                request.observation_id,
                request.endpoint_totality_certificate_id,
                &request.dependencies,
            );
            if request.observation_digest != expected {
                return Err(RelationalAnalysisPlanError::ObservationDigestMismatch {
                    request_id: request.request_id,
                    expected,
                    actual: request.observation_digest,
                });
            }
        }
        let layer_id = registration.layer_id();
        match canonical.get(&layer_id) {
            Some(existing) if existing == &registration => {}
            Some(_) => {
                return Err(RelationalAnalysisPlanError::LayerIdentityCollision(
                    layer_id,
                ));
            }
            None => {
                canonical.insert(layer_id, registration);
            }
        }
    }
    Ok(canonical
        .into_values()
        .collect::<Vec<_>>()
        .into_boxed_slice())
}

fn validate_choice_dependencies(
    question_ids: &[QuestionId],
    choices: &[RelationalChoiceRegistration],
) -> Result<(), RelationalAnalysisPlanError> {
    for choice in choices {
        let expected = RelationalAnalysisDependencyId::Question(choice.input_question_id);
        if choice.dependencies() != std::slice::from_ref(&expected) {
            return Err(RelationalAnalysisPlanError::ChoiceDependencyRecipeMismatch(
                choice.choice_id,
            ));
        }
        if question_ids
            .binary_search(&choice.input_question_id)
            .is_err()
        {
            return Err(
                RelationalAnalysisPlanError::ForeignChoiceQuestionDependency {
                    choice_id: choice.choice_id,
                    actual: choice.input_question_id,
                },
            );
        }
    }
    Ok(())
}

fn validate_registration_dependencies(
    question_ids: &[QuestionId],
    choices: &[RelationalChoiceRegistration],
    registrations: &[RelationalAnalysisLayerRegistration],
) -> Result<(), RelationalAnalysisPlanError> {
    let layers = registrations
        .iter()
        .map(RelationalAnalysisLayerRegistration::layer_id)
        .collect::<BTreeSet<_>>();
    for registration in registrations {
        let expected = match registration {
            RelationalAnalysisLayerRegistration::Result(result) => match result.choice_id {
                Some(choice_id) => RelationalAnalysisDependencyId::Choice(choice_id),
                None => match result.input {
                    RelationalResolvedResultInput::Sources(relation_id) => {
                        RelationalAnalysisDependencyId::Relation(relation_id)
                    }
                    RelationalResolvedResultInput::Selected(question_id) => {
                        RelationalAnalysisDependencyId::Question(question_id)
                    }
                    RelationalResolvedResultInput::Choice(choice_id) => {
                        RelationalAnalysisDependencyId::Choice(choice_id)
                    }
                    RelationalResolvedResultInput::MechanismIncidence(request_id) => {
                        RelationalAnalysisDependencyId::Mechanisms(request_id)
                    }
                },
            },
            RelationalAnalysisLayerRegistration::Mechanisms(request) => match request.target {
                RelationalResolvedMechanismTarget::Selected(question_id) => {
                    RelationalAnalysisDependencyId::Question(question_id)
                }
                RelationalResolvedMechanismTarget::Choice(choice_id) => {
                    RelationalAnalysisDependencyId::Choice(choice_id)
                }
            },
        };
        if registration.dependencies() != std::slice::from_ref(&expected) {
            return Err(RelationalAnalysisPlanError::DependencyRecipeMismatch(
                registration.layer_id(),
            ));
        }
        match expected {
            RelationalAnalysisDependencyId::Relation(_) => {}
            RelationalAnalysisDependencyId::Question(actual)
                if question_ids.binary_search(&actual).is_err() =>
            {
                return Err(RelationalAnalysisPlanError::ForeignQuestionDependency {
                    layer_id: registration.layer_id(),
                    actual,
                });
            }
            RelationalAnalysisDependencyId::Question(_) => {}
            RelationalAnalysisDependencyId::Choice(choice_id) => {
                if choices
                    .binary_search_by_key(&choice_id, RelationalChoiceRegistration::choice_id)
                    .is_err()
                {
                    return Err(RelationalAnalysisPlanError::DanglingChoiceDependency {
                        layer_id: registration.layer_id(),
                        choice_id,
                    });
                }
            }
            RelationalAnalysisDependencyId::Result(view_id) => {
                let dependency = RelationalAnalysisLayerId::Result(view_id);
                if !layers.contains(&dependency) {
                    return Err(RelationalAnalysisPlanError::DanglingDependency {
                        layer_id: registration.layer_id(),
                        dependency,
                    });
                }
            }
            RelationalAnalysisDependencyId::Mechanisms(request_id) => {
                let dependency = RelationalAnalysisLayerId::Mechanisms(request_id);
                if !layers.contains(&dependency) {
                    return Err(RelationalAnalysisPlanError::DanglingDependency {
                        layer_id: registration.layer_id(),
                        dependency,
                    });
                }
            }
        }
        if let RelationalAnalysisLayerRegistration::Result(result) = registration {
            match result.choice_id {
                Some(choice_id) => {
                    let Some(choice) = choices
                        .binary_search_by_key(&choice_id, RelationalChoiceRegistration::choice_id)
                        .ok()
                        .map(|index| &choices[index])
                    else {
                        return Err(RelationalAnalysisPlanError::DanglingChoiceDependency {
                            layer_id: registration.layer_id(),
                            choice_id,
                        });
                    };
                    if result.input != RelationalResolvedResultInput::Choice(choice_id) {
                        return Err(
                            RelationalAnalysisPlanError::ChoiceMaterializerInputMismatch {
                                view_id: result.view_id,
                                choice_id,
                            },
                        );
                    }
                }
                None => {}
            }
        }
    }
    Ok(())
}

fn validate_question_ids(question_ids: &[QuestionId]) -> Result<(), RelationalAnalysisPlanError> {
    if question_ids.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(RelationalAnalysisPlanError::NonCanonicalQuestionSet);
    }
    Ok(())
}

fn derive_checked_analysis_graph_digest(
    choices: &[RelationalChoiceRegistration],
    registrations: &[RelationalAnalysisLayerRegistration],
) -> [u8; 32] {
    let mut semantic_nodes = choices
        .iter()
        .map(|choice| (0x02, choice.choice_id.bytes()))
        .collect::<BTreeSet<_>>();
    semantic_nodes.extend(registrations.iter().map(|registration| match registration {
        RelationalAnalysisLayerRegistration::Result(result) => (0x01, result.view_id.bytes()),
        RelationalAnalysisLayerRegistration::Mechanisms(request) => {
            (0x03, derive_checked_mechanism_node_digest(request))
        }
    }));
    let mut hasher = Sha256::new();
    hasher.update(CHECKED_ANALYSIS_GRAPH_V4);
    checked_hash_component(
        &mut hasher,
        "semantic-node-count",
        &semantic_nodes.len().to_string(),
    );
    for (kind, identity) in semantic_nodes {
        hasher.update([kind]);
        hasher.update(identity);
    }
    hasher.finalize().into()
}

fn derive_checked_mechanism_node_digest(
    registration: &RelationalMechanismLayerRegistration,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(CHECKED_ANALYSIS_MECHANISM_NODE_V2);
    hasher.update(registration.request_id.bytes());
    hasher.update(registration.endpoint_totality_certificate_id.bytes());
    hasher.finalize().into()
}

fn checked_hash_component(hasher: &mut Sha256, label: &str, value: &str) {
    hasher.update((label.len() as u64).to_le_bytes());
    hasher.update(label.as_bytes());
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
}

fn parse_lowercase_sha256(value: &str) -> Result<[u8; 32], RelationalAnalysisPlanError> {
    if value.len() != 64
        || !value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(RelationalAnalysisPlanError::MalformedAnalysisGraphDigest);
    }
    let mut digest = [0_u8; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        let offset = index * 2;
        *byte =
            (hex_nibble(value.as_bytes()[offset]) << 4) | hex_nibble(value.as_bytes()[offset + 1]);
    }
    Ok(digest)
}

fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => unreachable!("lowercase hexadecimal validated"),
    }
}

fn derive_analysis_plan_root(
    payload: &RelationalAnalysisPlanPayload,
) -> RelationalAnalysisPlanRoot {
    let mut hasher = AnalysisHasher::new(ANALYSIS_PLAN_ROOT_V6);
    hasher.u32(RELATIONAL_ANALYSIS_PLAN_VERSION);
    hasher.u128(payload.question_ids.len() as u128);
    for question_id in &payload.question_ids {
        hasher.digest(question_id.bytes());
    }
    hasher.digest(payload.producer_graph_digest.bytes());
    hasher.u128(payload.choices.len() as u128);
    for choice in &payload.choices {
        hash_choice_registration(&mut hasher, choice);
    }
    hasher.u128(payload.registrations.len() as u128);
    for registration in &payload.registrations {
        hash_registration(&mut hasher, registration);
    }
    RelationalAnalysisPlanRoot(hasher.finish())
}

fn hash_choice_registration(hasher: &mut AnalysisHasher, choice: &RelationalChoiceRegistration) {
    hasher.digest(choice.choice_id.bytes());
    hasher.digest(choice.input_question_id.bytes());
    hasher.digest(choice.semantic_spec_digest.bytes());
    hasher.u128(choice.spec.partition_value_count() as u128);
    hasher.u128(choice.spec.measure_count() as u128);
    match choice.spec.having() {
        None => hasher.u8(0x00),
        Some(super::result_view::ResultViewHaving::Varies { measure_index }) => {
            hasher.u8(0x01);
            hasher.u128(measure_index as u128);
        }
    }
    match choice.spec.policy() {
        super::result_view::ResultViewChoice::Optimize {
            cardinality,
            direction,
        } => {
            hasher.u8(0x01);
            hash_choose_cardinality(hasher, *cardinality);
            hash_optimize_direction(hasher, *direction);
        }
        super::result_view::ResultViewChoice::Pareto { directions } => {
            hasher.u8(0x02);
            hasher.u128(directions.len() as u128);
            for direction in directions.iter().copied() {
                hash_optimize_direction(hasher, direction);
            }
        }
    }
    hash_dependencies(hasher, &choice.dependencies);
}

fn hash_registration(
    hasher: &mut AnalysisHasher,
    registration: &RelationalAnalysisLayerRegistration,
) {
    match registration {
        RelationalAnalysisLayerRegistration::Result(result) => {
            hasher.u8(0x01);
            hasher.digest(result.view_id.bytes());
            match result.choice_id {
                None => hasher.u8(0x00),
                Some(choice_id) => {
                    hasher.u8(0x01);
                    hasher.digest(choice_id.bytes());
                }
            }
            hash_result_input(hasher, result.input);
            hasher.digest(result.semantic_spec_digest.bytes());
            hash_dependencies(hasher, &result.dependencies);
        }
        RelationalAnalysisLayerRegistration::Mechanisms(request) => {
            hasher.u8(0x02);
            hasher.digest(request.request_id.bytes());
            hash_mechanism_target(hasher, request.target);
            hasher.digest(request.observation_id.bytes());
            hasher.digest(request.endpoint_totality_certificate_id.bytes());
            hasher.digest(request.observation_digest.bytes());
            hash_dependencies(hasher, &request.dependencies);
        }
    }
}

fn hash_result_input(hasher: &mut AnalysisHasher, input: RelationalResolvedResultInput) {
    match input {
        RelationalResolvedResultInput::Sources(relation_id) => {
            hasher.u8(0x03);
            hasher.digest(relation_id.bytes());
        }
        RelationalResolvedResultInput::Selected(question_id) => {
            hasher.u8(0x01);
            hasher.digest(question_id.bytes());
        }
        RelationalResolvedResultInput::Choice(choice_id) => {
            hasher.u8(0x04);
            hasher.digest(choice_id.bytes());
        }
        RelationalResolvedResultInput::MechanismIncidence(request_id) => {
            hasher.u8(0x02);
            hasher.digest(request_id.bytes());
        }
    }
}

fn hash_mechanism_target(hasher: &mut AnalysisHasher, target: RelationalResolvedMechanismTarget) {
    match target {
        RelationalResolvedMechanismTarget::Selected(question_id) => {
            hasher.u8(0x01);
            hasher.digest(question_id.bytes());
        }
        RelationalResolvedMechanismTarget::Choice(choice_id) => {
            hasher.u8(0x02);
            hasher.digest(choice_id.bytes());
        }
    }
}

fn hash_dependencies(hasher: &mut AnalysisHasher, dependencies: &[RelationalAnalysisDependencyId]) {
    hasher.u128(dependencies.len() as u128);
    for dependency in dependencies {
        match dependency {
            RelationalAnalysisDependencyId::Relation(relation_id) => {
                hasher.u8(0x04);
                hasher.digest(relation_id.bytes());
            }
            RelationalAnalysisDependencyId::Question(question_id) => {
                hasher.u8(0x01);
                hasher.digest(question_id.bytes());
            }
            RelationalAnalysisDependencyId::Choice(choice_id) => {
                hasher.u8(0x05);
                hasher.digest(choice_id.bytes());
            }
            RelationalAnalysisDependencyId::Result(view_id) => {
                hasher.u8(0x02);
                hasher.digest(view_id.bytes());
            }
            RelationalAnalysisDependencyId::Mechanisms(request_id) => {
                hasher.u8(0x03);
                hasher.digest(request_id.bytes());
            }
        }
    }
}

fn hash_choose_cardinality(hasher: &mut AnalysisHasher, cardinality: ExploreChooseCardinality) {
    hasher.u8(match cardinality {
        ExploreChooseCardinality::One => 0x01,
        ExploreChooseCardinality::All => 0x02,
    });
}

fn hash_optimize_direction(hasher: &mut AnalysisHasher, direction: ExploreOptimizeDirection) {
    hasher.u8(match direction {
        ExploreOptimizeDirection::Minimize => 0x01,
        ExploreOptimizeDirection::Maximize => 0x02,
    });
}

struct AnalysisHasher(Sha256);

impl AnalysisHasher {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update((domain.len() as u64).to_be_bytes());
        hasher.update(domain);
        Self(hasher)
    }

    fn u8(&mut self, value: u8) {
        self.0.update([value]);
    }

    fn u32(&mut self, value: u32) {
        self.0.update(value.to_be_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.0.update(value.to_be_bytes());
    }

    fn digest(&mut self, digest: [u8; 32]) {
        self.0.update(digest);
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalAnalysisPlanError {
    InvalidQuery(String),
    IdentityCountMismatch {
        nodes: usize,
        identities: usize,
    },
    NonCanonicalNodeIndex {
        actual: usize,
        expected: usize,
    },
    IdentityKindMismatch {
        node_index: usize,
    },
    UnknownFindIndex {
        node_index: usize,
        find_index: usize,
    },
    ReferenceNotPrior {
        node_index: usize,
        referenced_index: usize,
    },
    ReferenceMissing {
        node_index: usize,
        referenced_index: usize,
    },
    ReferenceKindMismatch {
        node_index: usize,
        referenced_index: usize,
        expected: &'static str,
    },
    TargetViewNotChosenSelectedCases {
        node_index: usize,
        view_node_index: usize,
    },
    InvalidEndpointTotalityCertificate {
        node_index: usize,
        message: String,
    },
    EndpointTotalityRequestScopeMismatch {
        node_index: usize,
        expected: MechanismRequestId,
        actual: MechanismRequestId,
    },
    EndpointTotalityRelationScopeMismatch {
        node_index: usize,
        expected: RelationId,
        actual: RelationId,
    },
    ObservationDigestMismatch {
        request_id: MechanismRequestId,
        expected: RelationalMechanismObservationDigest,
        actual: RelationalMechanismObservationDigest,
    },
    Observation(String),
    MissingChoiceRelation {
        node_index: usize,
    },
    InvalidChoiceSpec(String),
    ChoiceIdentityCollision(ChoiceId),
    ChoiceDependencyRecipeMismatch(ChoiceId),
    ForeignChoiceQuestionDependency {
        choice_id: ChoiceId,
        actual: QuestionId,
    },
    DanglingChoiceDependency {
        layer_id: RelationalAnalysisLayerId,
        choice_id: ChoiceId,
    },
    ChoiceMaterializerInputMismatch {
        view_id: ViewId,
        choice_id: ChoiceId,
    },
    LayerIdentityCollision(RelationalAnalysisLayerId),
    DependencyRecipeMismatch(RelationalAnalysisLayerId),
    ForeignQuestionDependency {
        layer_id: RelationalAnalysisLayerId,
        actual: QuestionId,
    },
    NonCanonicalQuestionSet,
    DanglingDependency {
        layer_id: RelationalAnalysisLayerId,
        dependency: RelationalAnalysisLayerId,
    },
    MalformedAnalysisGraphDigest,
    AnalysisGraphDigestMismatch {
        producer: RelationalCheckedAnalysisGraphDigest,
        derived: RelationalCheckedAnalysisGraphDigest,
    },
}

impl fmt::Display for RelationalAnalysisPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidQuery(message) => {
                write!(formatter, "invalid checked analysis query: {message}")
            }
            Self::IdentityCountMismatch { nodes, identities } => write!(
                formatter,
                "analysis query has {nodes} nodes but {identities} producer identities"
            ),
            Self::NonCanonicalNodeIndex { actual, expected } => write!(
                formatter,
                "analysis node has canonical index {actual}, expected {expected}"
            ),
            Self::IdentityKindMismatch { node_index } => write!(
                formatter,
                "analysis node {node_index} and producer identity have different kinds"
            ),
            Self::UnknownFindIndex {
                node_index,
                find_index,
            } => write!(
                formatter,
                "analysis node {node_index} references absent FIND index {find_index}"
            ),
            Self::ReferenceNotPrior {
                node_index,
                referenced_index,
            } => write!(
                formatter,
                "analysis node {node_index} references non-prior node {referenced_index}"
            ),
            Self::ReferenceMissing {
                node_index,
                referenced_index,
            } => write!(
                formatter,
                "analysis node {node_index} references absent node {referenced_index}"
            ),
            Self::ReferenceKindMismatch {
                node_index,
                referenced_index,
                expected,
            } => write!(
                formatter,
                "analysis node {node_index} references node {referenced_index}, expected {expected}"
            ),
            Self::TargetViewNotChosenSelectedCases {
                node_index,
                view_node_index,
            } => write!(
                formatter,
                "mechanism node {node_index} targets result {view_node_index}, which is not a chosen selected-case view"
            ),
            Self::InvalidEndpointTotalityCertificate {
                node_index,
                message,
            } => write!(
                formatter,
                "mechanism node {node_index} has an invalid endpoint-totality certificate: {message}"
            ),
            Self::EndpointTotalityRequestScopeMismatch {
                node_index,
                expected,
                actual,
            } => write!(
                formatter,
                "mechanism node {node_index} endpoint-totality certificate belongs to request {actual:?}, expected {expected:?}"
            ),
            Self::EndpointTotalityRelationScopeMismatch {
                node_index,
                expected,
                actual,
            } => write!(
                formatter,
                "mechanism node {node_index} endpoint-totality certificate belongs to relation {actual:?}, expected {expected:?}"
            ),
            Self::ObservationDigestMismatch {
                request_id,
                expected,
                actual,
            } => write!(
                formatter,
                "mechanism request {request_id:?} observation digest does not match its certificate-bound registration: expected {expected:?}, actual {actual:?}"
            ),
            Self::Observation(message) => {
                write!(
                    formatter,
                    "invalid checked mechanism observation: {message}"
                )
            }
            Self::MissingChoiceRelation { node_index } => write!(
                formatter,
                "analysis result node {node_index} has a ChoiceId without a canonical choice relation"
            ),
            Self::InvalidChoiceSpec(message) => {
                write!(formatter, "invalid semantic choice relation: {message}")
            }
            Self::ChoiceIdentityCollision(choice_id) => write!(
                formatter,
                "different choice registrations share ChoiceId {choice_id:?}"
            ),
            Self::ChoiceDependencyRecipeMismatch(choice_id) => write!(
                formatter,
                "choice relation {choice_id:?} has dependencies inconsistent with its FIND input"
            ),
            Self::ForeignChoiceQuestionDependency { choice_id, actual } => write!(
                formatter,
                "choice relation {choice_id:?} depends on foreign question {actual:?}"
            ),
            Self::DanglingChoiceDependency {
                layer_id,
                choice_id,
            } => write!(
                formatter,
                "analysis layer {layer_id:?} has dangling choice dependency {choice_id:?}"
            ),
            Self::ChoiceMaterializerInputMismatch { view_id, choice_id } => write!(
                formatter,
                "result view {view_id:?} does not materialize its declared choice relation {choice_id:?}"
            ),
            Self::LayerIdentityCollision(layer_id) => write!(
                formatter,
                "different analysis registrations share layer identity {layer_id:?}"
            ),
            Self::DependencyRecipeMismatch(layer_id) => write!(
                formatter,
                "analysis layer {layer_id:?} has dependencies inconsistent with its resolved input"
            ),
            Self::ForeignQuestionDependency {
                layer_id,
                actual,
            } => write!(
                formatter,
                "analysis layer {layer_id:?} depends on foreign question {actual:?}"
            ),
            Self::NonCanonicalQuestionSet => formatter.write_str(
                "analysis plan question IDs must be strictly sorted and unique",
            ),
            Self::DanglingDependency {
                layer_id,
                dependency,
            } => write!(
                formatter,
                "analysis layer {layer_id:?} has dangling dependency {dependency:?}"
            ),
            Self::MalformedAnalysisGraphDigest => formatter
                .write_str("checked analysis graph digest is not lowercase SHA-256 hexadecimal"),
            Self::AnalysisGraphDigestMismatch { producer, derived } => write!(
                formatter,
                "checked analysis graph digest mismatch: producer={producer:?}, derived={derived:?}"
            ),
        }
    }
}

impl Error for RelationalAnalysisPlanError {}

#[cfg(test)]
mod tests {
    use super::super::relation::{
        AdmissionId, ChoiceId, MechanismTargetId, RelationId, ViewInputId,
    };
    use super::*;

    fn question() -> QuestionId {
        let relation = RelationId::from_canonical_semantic_preimage(b"analysis-plan relation");
        let admission = AdmissionId::from_canonical_admission_preimage(relation, b"admission");
        QuestionId::from_canonical_find_preimage(
            admission,
            b"question",
            super::super::relation::FindPolarity::All,
        )
    }

    fn registrations(
        question_id: QuestionId,
    ) -> (
        RelationalChoiceRegistration,
        RelationalAnalysisLayerRegistration,
        RelationalAnalysisLayerRegistration,
    ) {
        let choice_id = ChoiceId::from_canonical_choice_preimage(question_id, b"selected choice");
        let view_id =
            ViewId::from_canonical_view_preimage(ViewInputId::Choice(choice_id), b"selected view");
        let request_id = MechanismRequestId::from_canonical_request_preimages(
            question_id,
            MechanismTargetId::Choice(choice_id),
            b"observation",
            b"normalization",
        );
        let target = RelationalResolvedMechanismTarget::Choice(choice_id);
        let observation_id = RelationalMechanismObservationId([0x22; 32]);
        let endpoint_totality_certificate_id =
            RelationalEndpointTotalityCertificateId::from_canonical_bytes([0x23; 32]);
        let dependencies =
            vec![RelationalAnalysisDependencyId::Choice(choice_id)].into_boxed_slice();
        let observation_digest = derive_observation_digest(
            request_id,
            target,
            observation_id,
            endpoint_totality_certificate_id,
            &dependencies,
        );
        let result =
            RelationalAnalysisLayerRegistration::Result(RelationalResultLayerRegistration {
                view_id,
                choice_id: Some(choice_id),
                input: RelationalResolvedResultInput::Choice(choice_id),
                semantic_spec_digest: RelationalResultSpecDigest([0x11; 32]),
                dependencies: vec![RelationalAnalysisDependencyId::Choice(choice_id)]
                    .into_boxed_slice(),
            });
        let mechanisms =
            RelationalAnalysisLayerRegistration::Mechanisms(RelationalMechanismLayerRegistration {
                request_id,
                target,
                observation_id,
                endpoint_totality_certificate_id,
                observation_digest,
                dependencies,
            });
        let choice = RelationalChoiceRegistration {
            choice_id,
            input_question_id: question_id,
            semantic_spec_digest: RelationalChoiceSpecDigest([0x10; 32]),
            spec: ChoiceRelationSpec::new(
                choice_id,
                0,
                0,
                None,
                super::super::result_view::ResultViewChoice::Optimize {
                    cardinality: ExploreChooseCardinality::One,
                    direction: ExploreOptimizeDirection::Minimize,
                },
            )
            .unwrap(),
            dependencies: vec![RelationalAnalysisDependencyId::Question(question_id)]
                .into_boxed_slice(),
        };
        (choice, result, mechanisms)
    }

    fn producer_hash(
        choices: Vec<RelationalChoiceRegistration>,
        registrations: Vec<RelationalAnalysisLayerRegistration>,
    ) -> String {
        let choices = canonicalize_choices(choices).unwrap();
        let canonical = canonicalize_registrations(registrations).unwrap();
        let digest = derive_checked_analysis_graph_digest(&choices, &canonical);
        digest.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn canonical_registration_order_does_not_rename_plan() {
        let question_id = question();
        let (choice, result, mechanisms) = registrations(question_id);
        let graph_hash = producer_hash(
            vec![choice.clone()],
            vec![result.clone(), mechanisms.clone()],
        );
        let forward = assemble_plan(
            &[question_id],
            &graph_hash,
            vec![choice.clone()],
            vec![result.clone(), mechanisms.clone()],
        )
        .unwrap();
        let reverse = assemble_plan(
            &[question_id],
            &graph_hash,
            vec![choice],
            vec![mechanisms, result],
        )
        .unwrap();

        assert!(forward.validate_root());
        assert_eq!(forward.root(), reverse.root());
        assert_eq!(forward.layer_registrations(), reverse.layer_registrations());
    }

    #[test]
    fn plan_root_commits_spec_content_beyond_producer_layer_ids() {
        let question_id = question();
        let (choice, result, mechanisms) = registrations(question_id);
        let graph_hash = producer_hash(
            vec![choice.clone()],
            vec![result.clone(), mechanisms.clone()],
        );
        let original = assemble_plan(
            &[question_id],
            &graph_hash,
            vec![choice.clone()],
            vec![result.clone(), mechanisms.clone()],
        )
        .unwrap();
        let mut changed_result = result;
        let RelationalAnalysisLayerRegistration::Result(result) = &mut changed_result else {
            unreachable!()
        };
        result.semantic_spec_digest = RelationalResultSpecDigest([0x44; 32]);
        let changed = assemble_plan(
            &[question_id],
            &graph_hash,
            vec![choice],
            vec![changed_result, mechanisms],
        )
        .unwrap();

        assert_ne!(original.root(), changed.root());
    }

    #[test]
    fn endpoint_totality_authorization_commits_observation_digest_and_plan_root() {
        let question_id = question();
        let (choice, result, mechanisms) = registrations(question_id);
        let graph_hash = producer_hash(
            vec![choice.clone()],
            vec![result.clone(), mechanisms.clone()],
        );
        let original = assemble_plan(
            &[question_id],
            &graph_hash,
            vec![choice.clone()],
            vec![result.clone(), mechanisms.clone()],
        )
        .unwrap();
        let RelationalAnalysisLayerRegistration::Mechanisms(mut changed_request) = mechanisms
        else {
            unreachable!()
        };
        let first_certificate = changed_request.endpoint_totality_certificate_id;
        let second_certificate =
            RelationalEndpointTotalityCertificateId::from_canonical_bytes([0x24; 32]);
        let first_observation_digest = derive_observation_digest(
            changed_request.request_id,
            changed_request.target,
            changed_request.observation_id,
            first_certificate,
            &changed_request.dependencies,
        );
        let second_observation_digest = derive_observation_digest(
            changed_request.request_id,
            changed_request.target,
            changed_request.observation_id,
            second_certificate,
            &changed_request.dependencies,
        );
        assert_eq!(changed_request.observation_digest, first_observation_digest);
        assert_ne!(first_observation_digest, second_observation_digest);

        changed_request.endpoint_totality_certificate_id = second_certificate;
        changed_request.observation_digest = second_observation_digest;
        let changed_registration = RelationalAnalysisLayerRegistration::Mechanisms(changed_request);
        let changed_graph_hash = producer_hash(
            vec![choice.clone()],
            vec![result.clone(), changed_registration.clone()],
        );
        assert_ne!(graph_hash, changed_graph_hash);
        let changed = assemble_plan(
            &[question_id],
            &changed_graph_hash,
            vec![choice],
            vec![result, changed_registration],
        )
        .unwrap();
        assert_ne!(original.root(), changed.root());
    }

    #[test]
    fn endpoint_totality_stale_certificate_bound_observation_digest_fails_canonicalization() {
        let question_id = question();
        let (_, _, mechanisms) = registrations(question_id);
        let RelationalAnalysisLayerRegistration::Mechanisms(mut request) = mechanisms else {
            unreachable!()
        };
        request.endpoint_totality_certificate_id =
            RelationalEndpointTotalityCertificateId::from_canonical_bytes([0x25; 32]);

        assert!(matches!(
            canonicalize_registrations(vec![RelationalAnalysisLayerRegistration::Mechanisms(
                request
            )]),
            Err(RelationalAnalysisPlanError::ObservationDigestMismatch { .. })
        ));
    }

    #[test]
    fn dangling_dependency_and_graph_digest_mismatch_fail_closed() {
        let question_id = question();
        let (choice, result, mechanisms) = registrations(question_id);
        let graph_hash = producer_hash(
            vec![choice.clone()],
            vec![result.clone(), mechanisms.clone()],
        );
        let RelationalAnalysisLayerRegistration::Mechanisms(mut request) = mechanisms else {
            unreachable!()
        };
        let unrelated_choice =
            ChoiceId::from_canonical_choice_preimage(question_id, b"unrelated choice");
        request.target = RelationalResolvedMechanismTarget::Choice(unrelated_choice);
        request.dependencies =
            vec![RelationalAnalysisDependencyId::Choice(unrelated_choice)].into_boxed_slice();
        request.observation_digest = derive_observation_digest(
            request.request_id,
            request.target,
            request.observation_id,
            request.endpoint_totality_certificate_id,
            &request.dependencies,
        );
        assert!(matches!(
            assemble_plan(
                &[question_id],
                &graph_hash,
                vec![choice.clone()],
                vec![
                    result.clone(),
                    RelationalAnalysisLayerRegistration::Mechanisms(request)
                ],
            ),
            Err(RelationalAnalysisPlanError::DanglingChoiceDependency { .. })
        ));

        assert!(matches!(
            assemble_plan(&[question_id], &"00".repeat(32), vec![choice], vec![result]),
            Err(RelationalAnalysisPlanError::AnalysisGraphDigestMismatch { .. })
        ));
    }
}
