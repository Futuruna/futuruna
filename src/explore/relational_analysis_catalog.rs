//! Plan-bound evidence for relational Explore analysis layers.
//!
//! [`RelationalAnalysisPlan`] is the semantic DAG. This module gives each
//! declared node one arrival-order-independent evidence frontier without
//! inventing another query syntax or an execution scheduler. Result nodes own
//! evaluated row evidence and an explicit projection-publication receipt;
//! mechanism nodes own their exact target/incidence catalogs. Only immutable,
//! closed upstream catalogs can seal an edge in the analysis DAG.
//!
//! Closing result evidence is deliberately not the same operation as
//! publishing a [`ClosedResultView`]. The former proves exact input coverage;
//! the latter additionally commits the checked reducer/projector output. This
//! distinction keeps an empty open frontier, an exact empty result, and a
//! projected exact empty result observably different.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::mechanism_incidence::{
    MechanismIncidenceCatalogBuilder, MechanismIncidenceCounts, MechanismIncidenceError,
    MechanismIncidenceInsert, MechanismIncidenceRoot, MechanismIncidenceSnapshot,
    MechanismPublicationDiscovery, MechanismPublicationDiscoveryRef, MechanismRequestScope,
    MechanismSignatureDefinition, MechanismTargetCaseSetCommitment, MechanismTargetSealUpstream,
    MechanismUnavailableReasonDefinition,
};
use super::relation::{
    MechanismRequestId, MechanismTargetId, QuestionCatalog, QuestionContentRoot, QuestionId,
    RelationalCaseId, ViewId,
};
use super::relational_analysis_plan::{
    RelationalAnalysisDependencyId, RelationalAnalysisLayerId, RelationalAnalysisLayerRegistration,
    RelationalAnalysisPlan, RelationalAnalysisPlanRoot, RelationalMechanismLayerRegistration,
    RelationalMechanismObservationDigest, RelationalMechanismObservationId,
    RelationalResolvedMechanismTarget, RelationalResolvedResultInput,
    RelationalResultLayerRegistration, RelationalResultSpecDigest,
};
use super::relational_certified_source_summary::{
    RelationalCertifiedSourceSummaryArtifact, RelationalCertifiedSourceSummaryArtifactId,
};
use super::result_evidence::{
    RelationalResultEvidenceCatalog, RelationalResultEvidenceCatalogBuilder,
    RelationalResultEvidenceId, RelationalResultEvidenceRecord, RelationalResultEvidenceRoot,
    RelationalResultEvidenceSnapshot, RelationalResultInputSeal, ResultEvidenceError,
    ResultEvidenceUpstreamRoot,
};
use super::result_projection::{
    IndexedResultProjectionRecord, ResultProjectionCatalogBuilder, ResultProjectionClosure,
    ResultProjectionError, ResultProjectionRoot, ResultProjectionSnapshot,
};
use super::result_view::{
    ClosedResultView, CompactClosedResultView, ResultViewCount, ResultViewInputKind,
    ResultViewRoot, ResultViewSpec, ResultViewSpecRoot,
};
use super::structural_mechanism::StructuralQuotientClosureRoot;
use super::transition::TransitionId;

pub(crate) const RELATIONAL_ANALYSIS_CATALOG_SNAPSHOT_VERSION: u32 = 4;
pub(crate) const RELATIONAL_RESULT_PUBLICATION_VERSION: u32 = 1;

const ANALYSIS_CATALOG_ROOT_V4: &[u8] = b"futuruna.explore.relational-analysis.catalog-root.v4";
const RESULT_PUBLICATION_ID_V1: &[u8] =
    b"futuruna.explore.relational-analysis.result-publication-id.v1";

/// Arrival-order-independent commitment to every declared analysis layer,
/// including layers whose evidence has not yet been registered or sealed.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalAnalysisCatalogRoot([u8; 32]);

impl RelationalAnalysisCatalogRoot {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Checked plan binding required before mechanism replay evidence may enter
/// the analysis journal. Construction is private to the plan-backed catalog;
/// callers can inspect and pass the contract onward but cannot mint a binding
/// for an undeclared request or a different observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalMechanismEvidenceContract {
    scope: MechanismRequestScope,
    observation_id: RelationalMechanismObservationId,
    observation_digest: RelationalMechanismObservationDigest,
}

impl RelationalMechanismEvidenceContract {
    pub(crate) const fn scope(self) -> MechanismRequestScope {
        self.scope
    }

    pub(crate) const fn observation_id(self) -> RelationalMechanismObservationId {
        self.observation_id
    }

    pub(crate) const fn observation_digest(self) -> RelationalMechanismObservationDigest {
        self.observation_digest
    }
}

/// Compact replay-derived authority that closes one mechanism request while
/// its semantic payload remains in the live catalog builder. It is retained
/// across final analysis closure, but contains no cases, definitions,
/// terminals, state values, or context values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalMechanismClosureReceipt {
    request_id: MechanismRequestId,
    incidence_root: MechanismIncidenceRoot,
    counts: MechanismIncidenceCounts,
    result_input_seal: RelationalResultInputSeal,
    publication_event_end: u128,
}

impl RelationalMechanismClosureReceipt {
    pub(crate) const fn request_id(self) -> MechanismRequestId {
        self.request_id
    }

    pub(crate) const fn incidence_root(self) -> MechanismIncidenceRoot {
        self.incidence_root
    }

    pub(crate) const fn counts(self) -> MechanismIncidenceCounts {
        self.counts
    }

    pub(crate) const fn result_input_seal(self) -> RelationalResultInputSeal {
        self.result_input_seal
    }

    pub(crate) const fn publication_event_end(self) -> u128 {
        self.publication_event_end
    }
}

/// Content identity of one checked result-view publication.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalResultPublicationId([u8; 32]);

impl RelationalResultPublicationId {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Receipt binding a projected result root to the exact plan, spec, and row
/// evidence from which it was produced. Construction is private to
/// [`RelationalAnalysisCatalogBuilder::publish_result_view`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalResultPublication {
    version: u32,
    id: RelationalResultPublicationId,
    plan_root: RelationalAnalysisPlanRoot,
    view_id: ViewId,
    spec_root: ResultViewSpecRoot,
    evidence_root: RelationalResultEvidenceRoot,
    result_root: ResultViewRoot,
}

impl RelationalResultPublication {
    fn issue(
        plan_root: RelationalAnalysisPlanRoot,
        view_id: ViewId,
        spec_root: ResultViewSpecRoot,
        evidence_root: RelationalResultEvidenceRoot,
        result_root: ResultViewRoot,
    ) -> Self {
        let version = RELATIONAL_RESULT_PUBLICATION_VERSION;
        let id = derive_result_publication_id(
            version,
            plan_root,
            view_id,
            spec_root,
            evidence_root,
            result_root,
        );
        Self {
            version,
            id,
            plan_root,
            view_id,
            spec_root,
            evidence_root,
            result_root,
        }
    }

    pub(crate) const fn version(self) -> u32 {
        self.version
    }

    pub(crate) const fn id(self) -> RelationalResultPublicationId {
        self.id
    }

    pub(crate) const fn plan_root(self) -> RelationalAnalysisPlanRoot {
        self.plan_root
    }

    pub(crate) const fn view_id(self) -> ViewId {
        self.view_id
    }

    pub(crate) const fn spec_root(self) -> ResultViewSpecRoot {
        self.spec_root
    }

    pub(crate) const fn evidence_root(self) -> RelationalResultEvidenceRoot {
        self.evidence_root
    }

    pub(crate) const fn result_root(self) -> ResultViewRoot {
        self.result_root
    }

    fn validate_for(
        self,
        plan_root: RelationalAnalysisPlanRoot,
        view_id: ViewId,
        spec_root: ResultViewSpecRoot,
        evidence_root: RelationalResultEvidenceRoot,
    ) -> Result<(), RelationalAnalysisCatalogError> {
        if self.version != RELATIONAL_RESULT_PUBLICATION_VERSION {
            return Err(
                RelationalAnalysisCatalogError::UnsupportedResultPublicationVersion {
                    actual: self.version,
                    expected: RELATIONAL_RESULT_PUBLICATION_VERSION,
                },
            );
        }
        if self.plan_root != plan_root
            || self.view_id != view_id
            || self.spec_root != spec_root
            || self.evidence_root != evidence_root
        {
            return Err(RelationalAnalysisCatalogError::ResultPublicationScopeMismatch { view_id });
        }
        let derived = derive_result_publication_id(
            self.version,
            self.plan_root,
            self.view_id,
            self.spec_root,
            self.evidence_root,
            self.result_root,
        );
        if derived != self.id {
            return Err(
                RelationalAnalysisCatalogError::ResultPublicationIdMismatch {
                    claimed: self.id,
                    derived,
                },
            );
        }
        Ok(())
    }
}

/// Honest state of one declared analysis layer. In particular, an open empty
/// layer does not report the same status as an exact empty layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalAnalysisLayerStatus {
    ResultUnregistered,
    ResultInputOpen,
    ResultAwaitingPublication,
    ResultPublished,
    MechanismTargetOpen,
    MechanismTerminalOpen,
    MechanismClosed,
}

impl RelationalAnalysisLayerStatus {
    pub(crate) const fn is_exact(self) -> bool {
        matches!(self, Self::ResultPublished | Self::MechanismClosed)
    }
}

/// Canonical result-layer state retained in an analysis snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalResultLayerSnapshotState {
    Unregistered,
    Registered {
        spec: ResultViewSpec,
        evidence: RelationalResultEvidenceSnapshot,
        projection: ResultProjectionSnapshot,
        certified_source_summary: Option<RelationalCertifiedSourceSummaryArtifact>,
        publication: Option<RelationalResultPublication>,
    },
}

impl RelationalResultLayerSnapshotState {
    pub(crate) const fn spec(&self) -> Option<&ResultViewSpec> {
        match self {
            Self::Unregistered => None,
            Self::Registered { spec, .. } => Some(spec),
        }
    }

    pub(crate) const fn evidence(&self) -> Option<&RelationalResultEvidenceSnapshot> {
        match self {
            Self::Unregistered => None,
            Self::Registered { evidence, .. } => Some(evidence),
        }
    }

    pub(crate) const fn projection(&self) -> Option<&ResultProjectionSnapshot> {
        match self {
            Self::Unregistered => None,
            Self::Registered { projection, .. } => Some(projection),
        }
    }

    pub(crate) const fn certified_source_summary(
        &self,
    ) -> Option<&RelationalCertifiedSourceSummaryArtifact> {
        match self {
            Self::Unregistered => None,
            Self::Registered {
                certified_source_summary,
                ..
            } => certified_source_summary.as_ref(),
        }
    }

    pub(crate) const fn publication(&self) -> Option<RelationalResultPublication> {
        match self {
            Self::Unregistered => None,
            Self::Registered { publication, .. } => *publication,
        }
    }

    pub(crate) const fn status(&self) -> RelationalAnalysisLayerStatus {
        match self {
            Self::Unregistered => RelationalAnalysisLayerStatus::ResultUnregistered,
            Self::Registered {
                evidence,
                publication: None,
                ..
            } if evidence.input_is_sealed() => {
                RelationalAnalysisLayerStatus::ResultAwaitingPublication
            }
            Self::Registered {
                publication: Some(_),
                ..
            } => RelationalAnalysisLayerStatus::ResultPublished,
            Self::Registered { .. } => RelationalAnalysisLayerStatus::ResultInputOpen,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalResultLayerSnapshot {
    view_id: ViewId,
    input: RelationalResolvedResultInput,
    semantic_spec_digest: RelationalResultSpecDigest,
    state: RelationalResultLayerSnapshotState,
}

impl RelationalResultLayerSnapshot {
    pub(crate) const fn view_id(&self) -> ViewId {
        self.view_id
    }

    pub(crate) const fn input(&self) -> RelationalResolvedResultInput {
        self.input
    }

    pub(crate) const fn semantic_spec_digest(&self) -> RelationalResultSpecDigest {
        self.semantic_spec_digest
    }

    pub(crate) const fn state(&self) -> &RelationalResultLayerSnapshotState {
        &self.state
    }

    pub(crate) const fn status(&self) -> RelationalAnalysisLayerStatus {
        self.state.status()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalMechanismLayerSnapshot {
    request_id: MechanismRequestId,
    target: RelationalResolvedMechanismTarget,
    observation_id: RelationalMechanismObservationId,
    observation_digest: RelationalMechanismObservationDigest,
    incidence: MechanismIncidenceSnapshot,
}

impl RelationalMechanismLayerSnapshot {
    pub(crate) const fn request_id(&self) -> MechanismRequestId {
        self.request_id
    }

    pub(crate) const fn target(&self) -> RelationalResolvedMechanismTarget {
        self.target
    }

    pub(crate) const fn observation_id(&self) -> RelationalMechanismObservationId {
        self.observation_id
    }

    pub(crate) const fn observation_digest(&self) -> RelationalMechanismObservationDigest {
        self.observation_digest
    }

    pub(crate) const fn incidence(&self) -> &MechanismIncidenceSnapshot {
        &self.incidence
    }

    pub(crate) const fn status(&self) -> RelationalAnalysisLayerStatus {
        if self.incidence.frontier_is_complete() {
            RelationalAnalysisLayerStatus::MechanismClosed
        } else if self.incidence.target_is_sealed() {
            RelationalAnalysisLayerStatus::MechanismTerminalOpen
        } else {
            RelationalAnalysisLayerStatus::MechanismTargetOpen
        }
    }
}

/// One canonical layer checkpoint. Analysis snapshots sort these by
/// [`RelationalAnalysisLayerId`], never by event arrival or declaration name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalAnalysisLayerSnapshot {
    Result(RelationalResultLayerSnapshot),
    Mechanisms(RelationalMechanismLayerSnapshot),
}

impl RelationalAnalysisLayerSnapshot {
    pub(crate) const fn layer_id(&self) -> RelationalAnalysisLayerId {
        match self {
            Self::Result(result) => RelationalAnalysisLayerId::Result(result.view_id),
            Self::Mechanisms(mechanism) => {
                RelationalAnalysisLayerId::Mechanisms(mechanism.request_id)
            }
        }
    }

    pub(crate) const fn status(&self) -> RelationalAnalysisLayerStatus {
        match self {
            Self::Result(result) => result.status(),
            Self::Mechanisms(mechanism) => mechanism.status(),
        }
    }
}

/// Owned, canonical checkpoint for the whole analysis evidence DAG.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalAnalysisCatalogSnapshot {
    pub(crate) version: u32,
    plan_root: RelationalAnalysisPlanRoot,
    root: RelationalAnalysisCatalogRoot,
    layers: Box<[RelationalAnalysisLayerSnapshot]>,
}

impl RelationalAnalysisCatalogSnapshot {
    pub(crate) const fn plan_root(&self) -> RelationalAnalysisPlanRoot {
        self.plan_root
    }

    pub(crate) const fn root(&self) -> RelationalAnalysisCatalogRoot {
        self.root
    }

    pub(crate) fn layers(&self) -> &[RelationalAnalysisLayerSnapshot] {
        &self.layers
    }

    pub(crate) fn layer(
        &self,
        layer_id: RelationalAnalysisLayerId,
    ) -> Option<&RelationalAnalysisLayerSnapshot> {
        self.layers
            .binary_search_by_key(&layer_id, RelationalAnalysisLayerSnapshot::layer_id)
            .ok()
            .map(|index| &self.layers[index])
    }

    /// Verify only the catalog-level canonical order and composite root. Deep
    /// restoration additionally rebuilds each subordinate catalog through its
    /// checked snapshot boundary.
    pub(crate) fn validate_root(&self) -> bool {
        self.version == RELATIONAL_ANALYSIS_CATALOG_SNAPSHOT_VERSION
            && strictly_sorted_layer_ids(&self.layers)
            && self.root == derive_analysis_catalog_root(self.plan_root, &self.layers)
    }
}

#[derive(Clone, Debug)]
struct RegisteredResultLayer {
    spec: ResultViewSpec,
    evidence: RelationalResultEvidenceCatalogBuilder,
    projection: ResultProjectionCatalogBuilder,
    certified_source_summary: Option<RelationalCertifiedSourceSummaryArtifact>,
    publication: Option<RelationalResultPublication>,
    /// Process-local proof that this exact immutable evidence/projection
    /// prefix passed full result reconstruction when its publication was
    /// first accepted. The semantic snapshot and every identity omit this
    /// cache; cold typed-snapshot restoration remints it through the same
    /// full validation boundary once.
    validated_publication: Option<ValidatedResultPublication>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ValidatedResultPublication {
    publication_id: RelationalResultPublicationId,
    closure: ResultProjectionClosure,
}

impl ValidatedResultPublication {
    fn after_full_validation(
        publication: RelationalResultPublication,
        closure: ResultProjectionClosure,
    ) -> Result<Self, RelationalAnalysisCatalogError> {
        if publication.view_id() != closure.view_id()
            || publication.spec_root() != closure.spec_root()
        {
            return Err(
                RelationalAnalysisCatalogError::ResultPublicationScopeMismatch {
                    view_id: closure.view_id(),
                },
            );
        }
        if publication.result_root() != closure.result_root() {
            return Err(
                RelationalAnalysisCatalogError::PublishedResultRootMismatch {
                    view_id: closure.view_id(),
                },
            );
        }
        Ok(Self {
            publication_id: publication.id(),
            closure,
        })
    }

    fn validate_unchanged(
        self,
        publication: RelationalResultPublication,
        spec: &ResultViewSpec,
        evidence: &RelationalResultEvidenceCatalogBuilder,
        projection: &ResultProjectionCatalogBuilder,
    ) -> Result<(), RelationalAnalysisCatalogError> {
        spec.validate_spec_root()
            .map_err(RelationalAnalysisCatalogError::ResultSpec)?;
        let view_id = spec.view_id();
        if self.publication_id != publication.id() {
            return Err(RelationalAnalysisCatalogError::ResultPublicationConflict { view_id });
        }
        if self.closure.view_id() != view_id
            || self.closure.spec_root() != spec.spec_root()
            || self.closure.projection_root() != projection.root()
            || self.closure.record_count() != projection.len() as u128
        {
            return Err(RelationalAnalysisCatalogError::ResultProjection(
                ResultProjectionError::ClosureMismatch,
            ));
        }
        if self.closure.result_root() != publication.result_root() {
            return Err(RelationalAnalysisCatalogError::PublishedResultRootMismatch { view_id });
        }

        let counts = self.closure.counts();
        if counts.input_rows() != ResultViewCount::Exact(evidence.logical_len()) {
            return Err(
                RelationalAnalysisCatalogError::PublishedResultEvidenceMismatch { view_id },
            );
        }
        if !counts.output_rows().is_exact()
            || counts.groups().is_some_and(|count| !count.is_exact())
            || counts
                .output_groups()
                .is_some_and(|count| !count.is_exact())
        {
            return Err(RelationalAnalysisCatalogError::ResultProjection(
                ResultProjectionError::ClosureMismatch,
            ));
        }

        let grouped = spec.grain().is_grouped();
        if grouped != counts.groups().is_some() || grouped != counts.output_groups().is_some() {
            return Err(RelationalAnalysisCatalogError::ResultProjection(
                ResultProjectionError::ClosureMismatch,
            ));
        }
        let output_rows = counts.output_rows().current();
        let expected_records = if grouped {
            let groups = counts
                .groups()
                .expect("grouped result counts include the exact group count")
                .current();
            let output_groups = counts
                .output_groups()
                .expect("grouped result counts include the exact output-group count")
                .current();
            if output_groups > groups {
                return Err(RelationalAnalysisCatalogError::ResultProjection(
                    ResultProjectionError::ClosureMismatch,
                ));
            }
            if spec.choice().is_some() {
                groups.checked_add(output_rows)
            } else {
                Some(groups)
            }
        } else {
            Some(output_rows)
        };
        if expected_records != Some(self.closure.record_count()) {
            return Err(RelationalAnalysisCatalogError::ResultProjection(
                ResultProjectionError::ClosureMismatch,
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct ResultLayerBuilder {
    registration: RelationalResultLayerRegistration,
    registered: Option<RegisteredResultLayer>,
}

#[derive(Clone, Debug)]
struct MechanismLayerBuilder {
    registration: RelationalMechanismLayerRegistration,
    incidence: MechanismIncidenceCatalogBuilder,
}

#[derive(Clone, Debug)]
enum AnalysisLayerBuilder {
    Result(ResultLayerBuilder),
    Mechanisms(MechanismLayerBuilder),
}

impl AnalysisLayerBuilder {
    const fn layer_id(&self) -> RelationalAnalysisLayerId {
        match self {
            Self::Result(result) => {
                RelationalAnalysisLayerId::Result(result.registration.view_id())
            }
            Self::Mechanisms(mechanism) => {
                RelationalAnalysisLayerId::Mechanisms(mechanism.registration.request_id())
            }
        }
    }

    fn status(&self) -> RelationalAnalysisLayerStatus {
        match self {
            Self::Result(result) => match result.registered.as_ref() {
                None => RelationalAnalysisLayerStatus::ResultUnregistered,
                Some(registered) if registered.publication.is_some() => {
                    RelationalAnalysisLayerStatus::ResultPublished
                }
                Some(registered) if registered.evidence.input_is_sealed() => {
                    RelationalAnalysisLayerStatus::ResultAwaitingPublication
                }
                Some(_) => RelationalAnalysisLayerStatus::ResultInputOpen,
            },
            Self::Mechanisms(mechanism) if mechanism.incidence.frontier_is_complete() => {
                RelationalAnalysisLayerStatus::MechanismClosed
            }
            Self::Mechanisms(mechanism) if mechanism.incidence.target_is_sealed() => {
                RelationalAnalysisLayerStatus::MechanismTerminalOpen
            }
            Self::Mechanisms(_) => RelationalAnalysisLayerStatus::MechanismTargetOpen,
        }
    }

    fn snapshot(&self) -> RelationalAnalysisLayerSnapshot {
        match self {
            Self::Result(result) => {
                RelationalAnalysisLayerSnapshot::Result(RelationalResultLayerSnapshot {
                    view_id: result.registration.view_id(),
                    input: result.registration.input(),
                    semantic_spec_digest: result.registration.semantic_spec_digest(),
                    state: match &result.registered {
                        None => RelationalResultLayerSnapshotState::Unregistered,
                        Some(registered) => RelationalResultLayerSnapshotState::Registered {
                            spec: registered.spec.clone(),
                            evidence: registered.evidence.snapshot(),
                            projection: registered.projection.snapshot(),
                            certified_source_summary: registered.certified_source_summary.clone(),
                            publication: registered.publication,
                        },
                    },
                })
            }
            Self::Mechanisms(mechanism) => {
                RelationalAnalysisLayerSnapshot::Mechanisms(RelationalMechanismLayerSnapshot {
                    request_id: mechanism.registration.request_id(),
                    target: mechanism.registration.target(),
                    observation_id: mechanism.registration.observation_id(),
                    observation_digest: mechanism.registration.observation_digest(),
                    incidence: mechanism.incidence.snapshot(),
                })
            }
        }
    }

    fn into_snapshot_with_mechanism_publication_discovery(
        self,
    ) -> (
        RelationalAnalysisLayerSnapshot,
        Option<(MechanismRequestId, MechanismPublicationDiscovery)>,
    ) {
        match self {
            Self::Result(result) => {
                let view_id = result.registration.view_id();
                let input = result.registration.input();
                let semantic_spec_digest = result.registration.semantic_spec_digest();
                let state = match result.registered {
                    None => RelationalResultLayerSnapshotState::Unregistered,
                    Some(registered) => RelationalResultLayerSnapshotState::Registered {
                        spec: registered.spec,
                        evidence: registered.evidence.into_snapshot(),
                        projection: registered.projection.into_snapshot(),
                        certified_source_summary: registered.certified_source_summary,
                        publication: registered.publication,
                    },
                };
                (
                    RelationalAnalysisLayerSnapshot::Result(RelationalResultLayerSnapshot {
                        view_id,
                        input,
                        semantic_spec_digest,
                        state,
                    }),
                    None,
                )
            }
            Self::Mechanisms(mechanism) => {
                let request_id = mechanism.registration.request_id();
                let target = mechanism.registration.target();
                let observation_id = mechanism.registration.observation_id();
                let observation_digest = mechanism.registration.observation_digest();
                let (incidence, publication_discovery) = mechanism
                    .incidence
                    .into_snapshot_with_publication_discovery();
                (
                    RelationalAnalysisLayerSnapshot::Mechanisms(RelationalMechanismLayerSnapshot {
                        request_id,
                        target,
                        observation_id,
                        observation_digest,
                        incidence,
                    }),
                    Some((request_id, publication_discovery)),
                )
            }
        }
    }

    fn into_snapshot(self) -> RelationalAnalysisLayerSnapshot {
        self.into_snapshot_with_mechanism_publication_discovery().0
    }
}

/// Mutable evidence registry for exactly one validated analysis plan.
#[derive(Clone, Debug)]
pub(crate) struct RelationalAnalysisCatalogBuilder {
    plan: RelationalAnalysisPlan,
    layers: BTreeMap<RelationalAnalysisLayerId, AnalysisLayerBuilder>,
}

impl RelationalAnalysisCatalogBuilder {
    pub(crate) fn new(
        plan: &RelationalAnalysisPlan,
    ) -> Result<Self, RelationalAnalysisCatalogError> {
        if !plan.validate_root() {
            return Err(RelationalAnalysisCatalogError::InvalidPlanRoot);
        }

        let mut layers = BTreeMap::new();
        for registration in plan.layer_registrations() {
            validate_registration(plan.question_id(), registration)?;
            let state = match registration {
                RelationalAnalysisLayerRegistration::Result(result) => {
                    AnalysisLayerBuilder::Result(ResultLayerBuilder {
                        registration: result.clone(),
                        registered: None,
                    })
                }
                RelationalAnalysisLayerRegistration::Mechanisms(mechanism) => {
                    let scope = mechanism_scope(plan.question_id(), mechanism);
                    AnalysisLayerBuilder::Mechanisms(MechanismLayerBuilder {
                        registration: mechanism.clone(),
                        incidence: MechanismIncidenceCatalogBuilder::new(scope),
                    })
                }
            };
            let layer_id = state.layer_id();
            if layers.insert(layer_id, state).is_some() {
                return Err(RelationalAnalysisCatalogError::DuplicatePlanLayer { layer_id });
            }
        }

        Ok(Self {
            plan: plan.clone(),
            layers,
        })
    }

    pub(crate) const fn plan(&self) -> &RelationalAnalysisPlan {
        &self.plan
    }

    pub(crate) const fn plan_root(&self) -> RelationalAnalysisPlanRoot {
        self.plan.root()
    }

    pub(crate) fn layer_status(
        &self,
        layer_id: RelationalAnalysisLayerId,
    ) -> Option<RelationalAnalysisLayerStatus> {
        self.layers.get(&layer_id).map(AnalysisLayerBuilder::status)
    }

    /// Install the checked reducer spec for one declared result layer. The
    /// caller must repeat the already resolved input identity so a spec cannot
    /// be accidentally attached through an equal-typed but different DAG
    /// edge. Equal replay is idempotent; replacement is forbidden.
    pub(crate) fn register_result_spec(
        &mut self,
        view_id: ViewId,
        resolved_input: RelationalResolvedResultInput,
        spec: ResultViewSpec,
    ) -> Result<bool, RelationalAnalysisCatalogError> {
        spec.validate_spec_root()
            .map_err(RelationalAnalysisCatalogError::ResultSpec)?;
        if spec.view_id() != view_id {
            return Err(RelationalAnalysisCatalogError::ResultViewIdMismatch {
                expected: view_id,
                actual: spec.view_id(),
            });
        }

        let result = self.result_layer_mut(view_id)?;
        if result.registration.input() != resolved_input {
            return Err(RelationalAnalysisCatalogError::ResultInputDependencyMismatch { view_id });
        }
        let expected_kind = result_input_kind(resolved_input);
        if spec.input_kind() != expected_kind {
            return Err(RelationalAnalysisCatalogError::ResultInputKindMismatch {
                view_id,
                expected: expected_kind,
                actual: spec.input_kind(),
            });
        }
        match &result.registered {
            Some(existing) if existing.spec == spec => Ok(false),
            Some(_) => Err(RelationalAnalysisCatalogError::ResultSpecReplacement { view_id }),
            None => {
                let evidence = RelationalResultEvidenceCatalogBuilder::new(&spec)
                    .map_err(RelationalAnalysisCatalogError::ResultEvidence)?;
                let projection = ResultProjectionCatalogBuilder::new(&spec)
                    .map_err(RelationalAnalysisCatalogError::ResultProjection)?;
                result.registered = Some(RegisteredResultLayer {
                    spec,
                    evidence,
                    projection,
                    certified_source_summary: None,
                    publication: None,
                    validated_publication: None,
                });
                Ok(true)
            }
        }
    }

    pub(crate) fn result_spec(
        &self,
        view_id: ViewId,
    ) -> Result<&ResultViewSpec, RelationalAnalysisCatalogError> {
        Ok(&self.registered_result(view_id)?.spec)
    }

    pub(crate) fn certified_source_summary(
        &self,
        view_id: ViewId,
    ) -> Result<Option<&RelationalCertifiedSourceSummaryArtifact>, RelationalAnalysisCatalogError>
    {
        Ok(self
            .registered_result(view_id)?
            .certified_source_summary
            .as_ref())
    }

    /// Atomically bind one proof-specialized source artifact to its result
    /// layer and seal the exact logical input population. The artifact remains
    /// part of the layer snapshot so terminal validation and cold restoration
    /// cannot accidentally reinterpret zero physical rows as an empty result.
    pub(crate) fn accept_certified_source_summary(
        &mut self,
        artifact: &RelationalCertifiedSourceSummaryArtifact,
    ) -> Result<bool, RelationalAnalysisCatalogError> {
        let view_id = artifact.view_id();
        if !artifact.validate_identity() || artifact.analysis_plan_root() != self.plan.root() {
            return Err(
                RelationalAnalysisCatalogError::CertifiedSourceSummaryScopeMismatch { view_id },
            );
        }
        let result = self.result_layer(view_id)?;
        if result.registration.input()
            != RelationalResolvedResultInput::Sources(artifact.relation_id())
            || result.registration.semantic_spec_digest() != artifact.semantic_spec_digest()
        {
            return Err(
                RelationalAnalysisCatalogError::CertifiedSourceSummaryScopeMismatch { view_id },
            );
        }
        let registered = result
            .registered
            .as_ref()
            .ok_or(RelationalAnalysisCatalogError::ResultSpecNotRegistered { view_id })?;
        if registered.spec.spec_root() != artifact.spec_root() {
            return Err(
                RelationalAnalysisCatalogError::CertifiedSourceSummaryScopeMismatch { view_id },
            );
        }
        if let Some(existing) = &registered.certified_source_summary {
            return if existing == artifact {
                Ok(false)
            } else {
                Err(RelationalAnalysisCatalogError::CertifiedSourceSummaryConflict { view_id })
            };
        }
        if !registered.evidence.is_empty()
            || registered.evidence.input_is_sealed()
            || registered.projection.len() != 0
            || registered.publication.is_some()
        {
            return Err(RelationalAnalysisCatalogError::CertifiedSourceSummaryConflict { view_id });
        }

        let seal = RelationalResultInputSeal::from_certified_source_summary(artifact);
        let registered = self.registered_result_mut(view_id)?;
        registered
            .evidence
            .seal_input(seal)
            .map_err(RelationalAnalysisCatalogError::ResultEvidence)?;
        registered.certified_source_summary = Some(artifact.clone());
        Ok(true)
    }

    /// Borrow the journal-owned row evidence for incremental scheduling and
    /// terminal reducer replay. This deliberately avoids materializing a
    /// second closed catalog; the returned builder remains open/closed exactly
    /// as recorded by the authenticated analysis prefix.
    pub(crate) fn result_evidence(
        &self,
        view_id: ViewId,
    ) -> Result<&RelationalResultEvidenceCatalogBuilder, RelationalAnalysisCatalogError> {
        Ok(&self.registered_result(view_id)?.evidence)
    }

    pub(crate) fn insert_result_evidence(
        &mut self,
        view_id: ViewId,
        record: RelationalResultEvidenceRecord,
    ) -> Result<(RelationalResultEvidenceId, bool), RelationalAnalysisCatalogError> {
        self.registered_result_mut(view_id)?
            .evidence
            .insert(record)
            .map_err(RelationalAnalysisCatalogError::ResultEvidence)
    }

    /// Seal a selected-case result only from the exact closed FIND catalog
    /// named by the plan edge.
    pub(crate) fn seal_result_input_from_selected(
        &mut self,
        view_id: ViewId,
        question: &QuestionCatalog,
    ) -> Result<bool, RelationalAnalysisCatalogError> {
        let expected = self.result_registration(view_id)?.input();
        let RelationalResolvedResultInput::Selected(expected_question_id) = expected else {
            return Err(RelationalAnalysisCatalogError::ResultInputDependencyMismatch { view_id });
        };
        validate_question(expected_question_id, question)?;
        let seal = RelationalResultInputSeal::from_selected(question)
            .map_err(RelationalAnalysisCatalogError::ResultEvidence)?;
        self.registered_result_mut(view_id)?
            .evidence
            .seal_input(seal)
            .map_err(RelationalAnalysisCatalogError::ResultEvidence)
    }

    /// Seal an incidence result only when the supplied compact closure receipt
    /// names exactly the completed local plan layer. The receipt already
    /// commits the canonical successful-row set derived at mechanism closure.
    pub(crate) fn seal_result_input_from_mechanisms(
        &mut self,
        view_id: ViewId,
        closure: RelationalMechanismClosureReceipt,
        structural_root: StructuralQuotientClosureRoot,
    ) -> Result<bool, RelationalAnalysisCatalogError> {
        let expected = self.result_registration(view_id)?.input();
        let RelationalResolvedResultInput::MechanismIncidence(expected_request_id) = expected
        else {
            return Err(RelationalAnalysisCatalogError::ResultInputDependencyMismatch { view_id });
        };
        if closure.request_id() != expected_request_id {
            return Err(RelationalAnalysisCatalogError::MechanismRequestMismatch {
                expected: expected_request_id,
                actual: closure.request_id(),
            });
        }
        let local = self.mechanism_layer(expected_request_id)?;
        if !local.incidence.frontier_is_complete()
            || local.incidence.root() != closure.incidence_root()
        {
            return Err(RelationalAnalysisCatalogError::MechanismUpstreamMismatch {
                request_id: expected_request_id,
            });
        }
        let seal = closure
            .result_input_seal()
            .with_structural_quotient(structural_root)
            .map_err(RelationalAnalysisCatalogError::ResultEvidence)?;
        self.registered_result_mut(view_id)?
            .evidence
            .seal_input(seal)
            .map_err(RelationalAnalysisCatalogError::ResultEvidence)
    }

    /// Install a compact source- or selected-input receipt during durable
    /// replay. Mechanism incidence must use
    /// [`Self::seal_result_input_from_mechanisms`], after the outer journal has
    /// authenticated both the incidence and structural-quotient roots.
    pub(crate) fn seal_result_input_with_receipt(
        &mut self,
        view_id: ViewId,
        seal: RelationalResultInputSeal,
    ) -> Result<bool, RelationalAnalysisCatalogError> {
        let expected = self.result_registration(view_id)?.input();
        reject_generic_mechanism_result_seal(view_id, seal.upstream())?;
        let valid = match (expected, seal.upstream()) {
            (
                RelationalResolvedResultInput::Sources(expected_relation_id),
                ResultEvidenceUpstreamRoot::Sources {
                    relation_id: actual_relation_id,
                    ..
                }
                | ResultEvidenceUpstreamRoot::CertifiedSources {
                    relation_id: actual_relation_id,
                    ..
                },
            ) => expected_relation_id == actual_relation_id,
            (
                RelationalResolvedResultInput::Selected(expected_question_id),
                ResultEvidenceUpstreamRoot::Selected {
                    question_id: actual_question_id,
                    ..
                }
                | ResultEvidenceUpstreamRoot::CertifiedSelectedSupport {
                    question_id: actual_question_id,
                    ..
                },
            ) => expected_question_id == actual_question_id,
            _ => false,
        };
        if !valid {
            return Err(RelationalAnalysisCatalogError::ResultInputDependencyMismatch { view_id });
        }
        self.registered_result_mut(view_id)?
            .evidence
            .seal_input(seal)
            .map_err(RelationalAnalysisCatalogError::ResultEvidence)
    }

    /// Produce the immutable exact row-evidence input for the projection
    /// runtime. This does not publish a result view.
    pub(crate) fn close_result_evidence(
        &self,
        view_id: ViewId,
    ) -> Result<RelationalResultEvidenceCatalog, RelationalAnalysisCatalogError> {
        self.registered_result(view_id)?
            .evidence
            .materialize_closed()
            .map_err(RelationalAnalysisCatalogError::ResultEvidence)
    }

    /// Borrow the bounded durable projection prefix for scheduling and
    /// reporting. It remains distinct from row evidence: the latter proves
    /// reducer input, while this catalog records checked SELECT/choice output.
    pub(crate) fn result_projection(
        &self,
        view_id: ViewId,
    ) -> Result<&ResultProjectionCatalogBuilder, RelationalAnalysisCatalogError> {
        Ok(&self.registered_result(view_id)?.projection)
    }

    /// Accept exactly one canonical output record. Equal rediscovery is
    /// idempotent. A published projection may replay an existing record but
    /// can never grow a new suffix.
    pub(crate) fn insert_result_projection_record(
        &mut self,
        view_id: ViewId,
        record: IndexedResultProjectionRecord,
    ) -> Result<bool, RelationalAnalysisCatalogError> {
        let registered = self.registered_result_mut(view_id)?;
        if !registered.evidence.input_is_sealed() {
            return Err(RelationalAnalysisCatalogError::ResultEvidenceFrontierOpen { view_id });
        }
        if registered.publication.is_some()
            && record.ordinal() >= registered.projection.len() as u128
        {
            return Err(
                RelationalAnalysisCatalogError::ResultProjectionAlreadyPublished { view_id },
            );
        }
        registered
            .projection
            .insert(record)
            .map_err(RelationalAnalysisCatalogError::ResultProjection)
    }

    /// Derive the compact terminal claim only after every bounded output
    /// record matches a freshly evaluated exact view.
    pub(crate) fn prepare_result_projection_closure(
        &self,
        view: &ClosedResultView,
    ) -> Result<ResultProjectionClosure, RelationalAnalysisCatalogError> {
        let registered = self.registered_result(view.view_id())?;
        registered
            .projection
            .closure_for(view, &registered.evidence)
            .map_err(RelationalAnalysisCatalogError::ResultProjection)
    }

    pub(crate) fn prepare_durable_result_projection_closure(
        &self,
        view_id: ViewId,
    ) -> Result<ResultProjectionClosure, RelationalAnalysisCatalogError> {
        let registered = self.registered_result(view_id)?;
        registered
            .projection
            .closure_from_durable(&registered.spec, &registered.evidence)
            .map_err(RelationalAnalysisCatalogError::ResultProjection)
    }

    pub(crate) fn prepare_certified_source_projection_closure(
        &self,
        artifact: &RelationalCertifiedSourceSummaryArtifact,
    ) -> Result<ResultProjectionClosure, RelationalAnalysisCatalogError> {
        let registered = self.registered_result(artifact.view_id())?;
        let expected_seal = RelationalResultInputSeal::from_certified_source_summary(artifact);
        if registered.certified_source_summary.as_ref() != Some(artifact)
            || registered.evidence.input_seal() != Some(expected_seal)
        {
            return Err(
                RelationalAnalysisCatalogError::ResultInputDependencyMismatch {
                    view_id: artifact.view_id(),
                },
            );
        }
        registered
            .projection
            .closure_from_certified_source_groups(
                &registered.spec,
                artifact.certified_input_root(),
                artifact.exact_cardinality(),
                artifact.groups(),
            )
            .map_err(RelationalAnalysisCatalogError::ResultProjection)
    }

    /// Validate and publish a compact closure against the exact spec, row
    /// evidence, and bounded projection prefix already owned by this layer.
    /// Validation hashes borrowed evidence plus the materialized output; it
    /// neither needs an expression runtime nor reconstructs an owned view.
    pub(crate) fn publish_result_projection(
        &mut self,
        closure: ResultProjectionClosure,
    ) -> Result<(RelationalResultPublicationId, bool), RelationalAnalysisCatalogError> {
        let view_id = closure.view_id();
        let plan_root = self.plan.root();
        {
            let registered = self.registered_result(view_id)?;
            registered
                .spec
                .validate_spec_root()
                .map_err(RelationalAnalysisCatalogError::ResultSpec)?;
            if !registered.evidence.input_is_sealed() {
                return Err(RelationalAnalysisCatalogError::ResultEvidenceFrontierOpen { view_id });
            }
            if let Some(publication) = registered.publication {
                publication.validate_for(
                    plan_root,
                    view_id,
                    registered.spec.spec_root(),
                    registered.evidence.root(),
                )?;
                let validated = registered.validated_publication.ok_or(
                    RelationalAnalysisCatalogError::PublishedResultValidationMissing { view_id },
                )?;
                validated.validate_unchanged(
                    publication,
                    &registered.spec,
                    &registered.evidence,
                    &registered.projection,
                )?;
                if validated.closure != closure {
                    return Err(RelationalAnalysisCatalogError::ResultProjection(
                        ResultProjectionError::ClosureMismatch,
                    ));
                }
                return Ok((publication.id(), false));
            }
            if registered.validated_publication.is_some() {
                return Err(
                    RelationalAnalysisCatalogError::PublishedResultValidationMissing { view_id },
                );
            }
        }

        let (publication, validated) = {
            let registered = self.registered_result(view_id)?;
            let closed = registered
                .projection
                .validate_closure(closure, &registered.spec, &registered.evidence)
                .map_err(RelationalAnalysisCatalogError::ResultProjection)?;
            let publication = RelationalResultPublication::issue(
                plan_root,
                view_id,
                registered.spec.spec_root(),
                registered.evidence.root(),
                closed.root(),
            );
            let validated =
                ValidatedResultPublication::after_full_validation(publication, closure)?;
            (publication, validated)
        };
        let registered = self.registered_result_mut(view_id)?;
        registered.publication = Some(publication);
        registered.validated_publication = Some(validated);
        Ok((publication.id(), true))
    }

    /// Publish one proof-specialized source result. Its evidence catalog has
    /// zero physical singleton rows by construction; reducer identity is
    /// reconstructed from the certified population root, N, uniform group,
    /// and bounded durable projection.
    pub(crate) fn publish_certified_source_projection(
        &mut self,
        closure: ResultProjectionClosure,
        artifact: &RelationalCertifiedSourceSummaryArtifact,
    ) -> Result<(RelationalResultPublicationId, bool), RelationalAnalysisCatalogError> {
        let view_id = closure.view_id();
        if view_id != artifact.view_id() {
            return Err(RelationalAnalysisCatalogError::ResultInputDependencyMismatch { view_id });
        }
        let plan_root = self.plan.root();
        {
            let registered = self.registered_result(view_id)?;
            let expected_seal = RelationalResultInputSeal::from_certified_source_summary(artifact);
            if registered.certified_source_summary.as_ref() != Some(artifact)
                || registered.evidence.input_seal() != Some(expected_seal)
            {
                return Err(
                    RelationalAnalysisCatalogError::ResultInputDependencyMismatch { view_id },
                );
            }
            if let Some(publication) = registered.publication {
                publication.validate_for(
                    plan_root,
                    view_id,
                    registered.spec.spec_root(),
                    registered.evidence.root(),
                )?;
                let validated = registered.validated_publication.ok_or(
                    RelationalAnalysisCatalogError::PublishedResultValidationMissing { view_id },
                )?;
                validated.validate_unchanged(
                    publication,
                    &registered.spec,
                    &registered.evidence,
                    &registered.projection,
                )?;
                if validated.closure != closure {
                    return Err(RelationalAnalysisCatalogError::ResultProjection(
                        ResultProjectionError::ClosureMismatch,
                    ));
                }
                return Ok((publication.id(), false));
            }
            if registered.validated_publication.is_some() {
                return Err(
                    RelationalAnalysisCatalogError::PublishedResultValidationMissing { view_id },
                );
            }
        }

        let (publication, validated) = {
            let registered = self.registered_result(view_id)?;
            let closed = registered
                .projection
                .validate_certified_source_groups_closure(
                    closure,
                    &registered.spec,
                    artifact.certified_input_root(),
                    artifact.exact_cardinality(),
                    artifact.groups(),
                )
                .map_err(RelationalAnalysisCatalogError::ResultProjection)?;
            let publication = RelationalResultPublication::issue(
                plan_root,
                view_id,
                registered.spec.spec_root(),
                registered.evidence.root(),
                closed.root(),
            );
            let validated =
                ValidatedResultPublication::after_full_validation(publication, closure)?;
            (publication, validated)
        };
        let registered = self.registered_result_mut(view_id)?;
        registered.publication = Some(publication);
        registered.validated_publication = Some(validated);
        Ok((publication.id(), true))
    }

    pub(crate) fn materialize_certified_source_result(
        &self,
        artifact: &RelationalCertifiedSourceSummaryArtifact,
    ) -> Result<CompactClosedResultView, RelationalAnalysisCatalogError> {
        let registered = self.registered_result(artifact.view_id())?;
        let publication =
            registered
                .publication
                .ok_or(RelationalAnalysisCatalogError::ResultNotPublished {
                    view_id: artifact.view_id(),
                })?;
        let expected_seal = RelationalResultInputSeal::from_certified_source_summary(artifact);
        if registered.certified_source_summary.as_ref() != Some(artifact)
            || registered.evidence.input_seal() != Some(expected_seal)
        {
            return Err(
                RelationalAnalysisCatalogError::ResultInputDependencyMismatch {
                    view_id: artifact.view_id(),
                },
            );
        }
        publication.validate_for(
            self.plan.root(),
            artifact.view_id(),
            registered.spec.spec_root(),
            registered.evidence.root(),
        )?;
        let closed = registered
            .projection
            .compact_from_certified_source_groups(
                &registered.spec,
                artifact.certified_input_root(),
                artifact.exact_cardinality(),
                artifact.groups(),
            )
            .map_err(RelationalAnalysisCatalogError::ResultProjection)?;
        if closed.root() != publication.result_root() {
            return Err(
                RelationalAnalysisCatalogError::PublishedResultRootMismatch {
                    view_id: artifact.view_id(),
                },
            );
        }
        Ok(closed)
    }

    /// Compatibility seam for in-process producers: the full view never
    /// enters a journal event. Its already-streamed projection is checked and
    /// only the compact closure is installed.
    pub(crate) fn publish_result_view(
        &mut self,
        view: &ClosedResultView,
    ) -> Result<(RelationalResultPublicationId, bool), RelationalAnalysisCatalogError> {
        let closure = self.prepare_result_projection_closure(view)?;
        self.publish_result_projection(closure)
    }

    /// Deterministically reconstruct a published view for chosen-view
    /// targeting or final reporting, without rerunning the checked runtime.
    pub(crate) fn materialize_published_result(
        &self,
        view_id: ViewId,
    ) -> Result<ClosedResultView, RelationalAnalysisCatalogError> {
        let registered = self.registered_result(view_id)?;
        if registered.certified_source_summary.is_some() {
            return Err(
                RelationalAnalysisCatalogError::CertifiedSourceResultRequiresCompactMaterialization {
                    view_id,
                },
            );
        }
        let publication = registered
            .publication
            .ok_or(RelationalAnalysisCatalogError::ResultNotPublished { view_id })?;
        publication.validate_for(
            self.plan.root(),
            view_id,
            registered.spec.spec_root(),
            registered.evidence.root(),
        )?;
        let closed = registered
            .projection
            .materialize_closed(&registered.spec, &registered.evidence)
            .map_err(RelationalAnalysisCatalogError::ResultProjection)?;
        if closed.root() != publication.result_root() {
            return Err(RelationalAnalysisCatalogError::PublishedResultRootMismatch { view_id });
        }
        Ok(closed)
    }

    pub(crate) fn result_publication(
        &self,
        view_id: ViewId,
    ) -> Result<Option<RelationalResultPublication>, RelationalAnalysisCatalogError> {
        Ok(self.registered_result(view_id)?.publication)
    }

    pub(crate) fn result_evidence_root(
        &self,
        view_id: ViewId,
    ) -> Result<RelationalResultEvidenceRoot, RelationalAnalysisCatalogError> {
        Ok(self.registered_result(view_id)?.evidence.root())
    }

    pub(crate) fn result_projection_root(
        &self,
        view_id: ViewId,
    ) -> Result<ResultProjectionRoot, RelationalAnalysisCatalogError> {
        Ok(self.registered_result(view_id)?.projection.root())
    }

    pub(crate) fn insert_mechanism_target_case(
        &mut self,
        request_id: MechanismRequestId,
        case_id: RelationalCaseId,
    ) -> Result<bool, RelationalAnalysisCatalogError> {
        self.mechanism_layer_mut(request_id)?
            .incidence
            .insert_target_case(case_id)
            .map_err(RelationalAnalysisCatalogError::Mechanism)
    }

    /// Borrow the journal-owned request frontier for bounded scheduling. The
    /// returned builder exposes ordered indexes and seal state, never private
    /// seal construction.
    pub(crate) fn mechanism_incidence(
        &self,
        request_id: MechanismRequestId,
    ) -> Result<&MechanismIncidenceCatalogBuilder, RelationalAnalysisCatalogError> {
        Ok(&self.mechanism_layer(request_id)?.incidence)
    }

    /// Borrow the replay-derived mechanism publication order while the
    /// semantic catalog remains open. This is operational addressing state;
    /// it is intentionally absent from snapshots and catalog roots.
    pub(crate) fn mechanism_publication_discovery(
        &self,
        request_id: MechanismRequestId,
    ) -> Result<MechanismPublicationDiscoveryRef<'_>, RelationalAnalysisCatalogError> {
        Ok(self
            .mechanism_layer(request_id)?
            .incidence
            .publication_discovery())
    }

    /// Resolve the immutable request/observation contract that producer
    /// evidence and its journal event must both name. This is deliberately a
    /// read-only plan lookup, not a scheduling decision.
    pub(crate) fn mechanism_evidence_contract(
        &self,
        request_id: MechanismRequestId,
    ) -> Result<RelationalMechanismEvidenceContract, RelationalAnalysisCatalogError> {
        let registration = self.mechanism_registration(request_id)?;
        Ok(RelationalMechanismEvidenceContract {
            scope: mechanism_scope(self.plan.question_id(), registration),
            observation_id: registration.observation_id(),
            observation_digest: registration.observation_digest(),
        })
    }

    pub(crate) fn intern_mechanism_signature(
        &mut self,
        request_id: MechanismRequestId,
        definition: &MechanismSignatureDefinition,
    ) -> Result<bool, RelationalAnalysisCatalogError> {
        let layer = self.mechanism_layer_mut(request_id)?;
        if layer.incidence.frontier_is_complete() {
            return match layer.incidence.signature_definition(definition.id()) {
                Some(existing) if existing == definition => Ok(false),
                _ => {
                    Err(RelationalAnalysisCatalogError::MechanismLayerAlreadyClosed { request_id })
                }
            };
        }
        layer
            .incidence
            .intern_signature(definition)
            .map_err(RelationalAnalysisCatalogError::Mechanism)
    }

    pub(crate) fn record_mechanism_incidence(
        &mut self,
        request_id: MechanismRequestId,
        case_id: RelationalCaseId,
        transition_id: TransitionId,
        definition: &MechanismSignatureDefinition,
    ) -> Result<MechanismIncidenceInsert, RelationalAnalysisCatalogError> {
        self.mechanism_layer_mut(request_id)?
            .incidence
            .record_incidence(case_id, transition_id, definition)
            .map_err(RelationalAnalysisCatalogError::Mechanism)
    }

    pub(crate) fn record_mechanism_unavailable(
        &mut self,
        request_id: MechanismRequestId,
        case_id: RelationalCaseId,
        definition: &MechanismUnavailableReasonDefinition,
    ) -> Result<bool, RelationalAnalysisCatalogError> {
        self.mechanism_layer_mut(request_id)?
            .incidence
            .record_unavailable(case_id, definition)
            .map_err(RelationalAnalysisCatalogError::Mechanism)
    }

    /// Seal a selected mechanism target only from the exact closed FIND
    /// catalog named by the plan.
    pub(crate) fn seal_mechanism_target_from_selected(
        &mut self,
        request_id: MechanismRequestId,
        question: &QuestionCatalog,
    ) -> Result<bool, RelationalAnalysisCatalogError> {
        let target = self.mechanism_registration(request_id)?.target();
        let RelationalResolvedMechanismTarget::Selected(expected_question_id) = target else {
            return Err(
                RelationalAnalysisCatalogError::MechanismTargetDependencyMismatch { request_id },
            );
        };
        validate_question(expected_question_id, question)?;
        self.mechanism_layer_mut(request_id)?
            .incidence
            .seal_selected_target(question)
            .map_err(RelationalAnalysisCatalogError::Mechanism)
    }

    /// Replay a selected-target seal from the compact receipt minted while
    /// the exact closed FIND catalog was available. Both the resolved
    /// question identity and the independently accumulated target case set
    /// are checked before the subordinate catalog mints its private seal.
    pub(crate) fn seal_mechanism_target_from_selected_commitment(
        &mut self,
        request_id: MechanismRequestId,
        question_id: QuestionId,
        content_root: QuestionContentRoot,
        target: MechanismTargetCaseSetCommitment,
    ) -> Result<bool, RelationalAnalysisCatalogError> {
        let resolved = self.mechanism_registration(request_id)?.target();
        let RelationalResolvedMechanismTarget::Selected(expected_question_id) = resolved else {
            return Err(
                RelationalAnalysisCatalogError::MechanismTargetDependencyMismatch { request_id },
            );
        };
        if question_id != expected_question_id {
            return Err(RelationalAnalysisCatalogError::QuestionScopeMismatch {
                expected: expected_question_id,
                actual: question_id,
            });
        }
        self.mechanism_layer_mut(request_id)?
            .incidence
            .seal_selected_target_commitment(content_root, target)
            .map_err(RelationalAnalysisCatalogError::Mechanism)
    }

    /// Replay a support-certified selected target without substituting an
    /// extensional question-content root. The subordinate catalog compares
    /// the certified cardinality and set commitment with its local target
    /// CaseIds before minting the typed target seal.
    pub(crate) fn seal_mechanism_target_from_certified_selected_commitment(
        &mut self,
        request_id: MechanismRequestId,
        question_id: QuestionId,
        population_root: super::relational_population::CertifiedSelectedPopulationRoot,
        exact_cardinality: u128,
        target: MechanismTargetCaseSetCommitment,
    ) -> Result<bool, RelationalAnalysisCatalogError> {
        let resolved = self.mechanism_registration(request_id)?.target();
        let RelationalResolvedMechanismTarget::Selected(expected_question_id) = resolved else {
            return Err(
                RelationalAnalysisCatalogError::MechanismTargetDependencyMismatch { request_id },
            );
        };
        if question_id != expected_question_id {
            return Err(RelationalAnalysisCatalogError::QuestionScopeMismatch {
                expected: expected_question_id,
                actual: question_id,
            });
        }
        self.mechanism_layer_mut(request_id)?
            .incidence
            .seal_certified_selected_target_commitment(population_root, exact_cardinality, target)
            .map_err(RelationalAnalysisCatalogError::Mechanism)
    }

    /// Seal a chosen-view mechanism target only from the exact result already
    /// published by the corresponding local plan layer.
    pub(crate) fn seal_mechanism_target_from_result(
        &mut self,
        request_id: MechanismRequestId,
        view: &ClosedResultView,
    ) -> Result<bool, RelationalAnalysisCatalogError> {
        let target = self.mechanism_registration(request_id)?.target();
        let RelationalResolvedMechanismTarget::ChosenView(expected_view_id) = target else {
            return Err(
                RelationalAnalysisCatalogError::MechanismTargetDependencyMismatch { request_id },
            );
        };
        if view.view_id() != expected_view_id {
            return Err(RelationalAnalysisCatalogError::ResultViewIdMismatch {
                expected: expected_view_id,
                actual: view.view_id(),
            });
        }
        self.require_matching_publication(view)?;
        self.mechanism_layer_mut(request_id)?
            .incidence
            .seal_chosen_view_target(view)
            .map_err(RelationalAnalysisCatalogError::Mechanism)
    }

    /// Validate one complete mechanism frontier and mint only its compact
    /// replay authority. The accumulated semantic maps remain borrowed in the
    /// live builder until final analysis closure moves them into the snapshot.
    pub(crate) fn mechanism_closure_receipt(
        &self,
        request_id: MechanismRequestId,
    ) -> Result<RelationalMechanismClosureReceipt, RelationalAnalysisCatalogError> {
        let incidence = &self.mechanism_layer(request_id)?.incidence;
        let closed_incidence = incidence
            .closed_ref()
            .map_err(RelationalAnalysisCatalogError::Mechanism)?;
        let incidence_root = closed_incidence.root();
        let result_input_seal = RelationalResultInputSeal::from_canonical_mechanism_terminals(
            request_id,
            incidence_root,
            closed_incidence.incidence_case_count() as u128,
            closed_incidence.canonical_terminal_records(),
        )
        .map_err(RelationalAnalysisCatalogError::ResultEvidence)?;
        Ok(RelationalMechanismClosureReceipt {
            request_id,
            incidence_root,
            counts: incidence.counts(),
            result_input_seal,
            publication_event_end: incidence.publication_event_count() as u128,
        })
    }

    pub(crate) fn snapshot(&self) -> RelationalAnalysisCatalogSnapshot {
        let layers = self
            .layers
            .values()
            .map(AnalysisLayerBuilder::snapshot)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let plan_root = self.plan.root();
        let root = derive_analysis_catalog_root(plan_root, &layers);
        RelationalAnalysisCatalogSnapshot {
            version: RELATIONAL_ANALYSIS_CATALOG_SNAPSHOT_VERSION,
            plan_root,
            root,
            layers,
        }
    }

    pub(crate) fn root(&self) -> RelationalAnalysisCatalogRoot {
        derive_analysis_catalog_builder_root(self.plan.root(), &self.layers)
    }

    /// Consume every layer builder into the canonical checkpoint after a
    /// caller has completed any required preflight. Semantic payloads move out
    /// of their maps and vectors; only the small layer container is rebuilt.
    fn into_snapshot(self) -> RelationalAnalysisCatalogSnapshot {
        let plan_root = self.plan.root();
        let layers = self
            .layers
            .into_values()
            .map(AnalysisLayerBuilder::into_snapshot)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let root = derive_analysis_catalog_root(plan_root, &layers);
        RelationalAnalysisCatalogSnapshot {
            version: RELATIONAL_ANALYSIS_CATALOG_SNAPSHOT_VERSION,
            plan_root,
            root,
            layers,
        }
    }

    /// Consume the canonical analysis payload and the replay-derived
    /// publication indexes in one pass. The returned discovery table is kept
    /// beside the semantic snapshot and therefore cannot affect its root.
    fn into_snapshot_with_mechanism_publication_discovery(
        self,
    ) -> (
        RelationalAnalysisCatalogSnapshot,
        Box<[(MechanismRequestId, MechanismPublicationDiscovery)]>,
    ) {
        let plan_root = self.plan.root();
        let mut publication_discoveries = Vec::new();
        let layers = self
            .layers
            .into_values()
            .map(|layer| {
                let (snapshot, publication_discovery) =
                    layer.into_snapshot_with_mechanism_publication_discovery();
                if let Some(publication_discovery) = publication_discovery {
                    publication_discoveries.push(publication_discovery);
                }
                snapshot
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        publication_discoveries.sort_unstable_by_key(|(request_id, _)| *request_id);
        let root = derive_analysis_catalog_root(plan_root, &layers);
        (
            RelationalAnalysisCatalogSnapshot {
                version: RELATIONAL_ANALYSIS_CATALOG_SNAPSHOT_VERSION,
                plan_root,
                root,
                layers,
            },
            publication_discoveries.into_boxed_slice(),
        )
    }

    /// Restore by rebuilding every subordinate catalog through its own
    /// checked snapshot path and rechecking the plan-derived layer metadata.
    /// Structural-incidence result seals deliberately require outer-journal
    /// replay because this standalone snapshot has no structural catalog from
    /// which to authenticate their claimed quotient root.
    pub(crate) fn from_snapshot(
        plan: &RelationalAnalysisPlan,
        snapshot: RelationalAnalysisCatalogSnapshot,
    ) -> Result<Self, RelationalAnalysisCatalogError> {
        if snapshot.version != RELATIONAL_ANALYSIS_CATALOG_SNAPSHOT_VERSION {
            return Err(RelationalAnalysisCatalogError::UnsupportedSnapshotVersion {
                actual: snapshot.version,
                expected: RELATIONAL_ANALYSIS_CATALOG_SNAPSHOT_VERSION,
            });
        }
        if snapshot.plan_root != plan.root() {
            return Err(RelationalAnalysisCatalogError::SnapshotPlanMismatch);
        }
        if !strictly_sorted_layer_ids(&snapshot.layers) {
            return Err(RelationalAnalysisCatalogError::NonCanonicalSnapshotOrder);
        }

        let expected_root = snapshot.root;
        let snapshot_layers = snapshot.layers.into_vec();
        let mut restored = Self::new(plan)?;
        if snapshot_layers.len() != restored.layers.len() {
            return Err(RelationalAnalysisCatalogError::SnapshotLayerSetMismatch);
        }

        for layer_snapshot in snapshot_layers {
            let layer_id = layer_snapshot.layer_id();
            match layer_snapshot {
                RelationalAnalysisLayerSnapshot::Result(result_snapshot) => {
                    restored.restore_result_layer(result_snapshot)?;
                }
                RelationalAnalysisLayerSnapshot::Mechanisms(mechanism_snapshot) => {
                    restored.restore_mechanism_layer(mechanism_snapshot)?;
                }
            }
            if !restored.layers.contains_key(&layer_id) {
                return Err(RelationalAnalysisCatalogError::SnapshotLayerSetMismatch);
            }
        }
        restored.validate_analysis_edges()?;
        if restored.root() != expected_root {
            return Err(RelationalAnalysisCatalogError::SnapshotRootMismatch);
        }
        Ok(restored)
    }

    /// Exact analysis closure is all-layer closure, not "no events happened".
    /// A plan with no analysis layers therefore closes cleanly, while every
    /// declared result must be published and every mechanism target must have
    /// one terminal per exact target case.
    pub(crate) fn finish(
        self,
    ) -> Result<ClosedRelationalAnalysisCatalog, RelationalAnalysisCatalogError> {
        self.validate_complete()?;
        Ok(ClosedRelationalAnalysisCatalog {
            snapshot: self.into_snapshot(),
        })
    }

    /// Finish semantic closure while moving each request's operational
    /// publication discovery sequence into the journal's closed state. No
    /// mechanism payload or discovery lane is cloned at this boundary.
    pub(crate) fn finish_with_mechanism_publication_discovery(
        self,
    ) -> Result<
        (
            ClosedRelationalAnalysisCatalog,
            Box<[(MechanismRequestId, MechanismPublicationDiscovery)]>,
        ),
        RelationalAnalysisCatalogError,
    > {
        self.validate_complete()?;
        let (snapshot, publication_discoveries) =
            self.into_snapshot_with_mechanism_publication_discovery();
        Ok((
            ClosedRelationalAnalysisCatalog { snapshot },
            publication_discoveries,
        ))
    }

    /// Check every terminal analysis frontier and cross-layer seal in place.
    /// No layer builder or evidence map is cloned. Journals use this as the
    /// failure-safe preflight before consuming the builder at the one terminal
    /// analysis-close event.
    pub(crate) fn validate_complete(&self) -> Result<(), RelationalAnalysisCatalogError> {
        self.validate_analysis_edges()?;
        for (layer_id, layer) in &self.layers {
            let status = layer.status();
            if !status.is_exact() {
                return Err(RelationalAnalysisCatalogError::AnalysisFrontierOpen {
                    layer_id: *layer_id,
                    status,
                });
            }
            match layer {
                AnalysisLayerBuilder::Result(result) => {
                    let registered = result.registered.as_ref().ok_or(
                        RelationalAnalysisCatalogError::AnalysisFrontierOpen {
                            layer_id: *layer_id,
                            status,
                        },
                    )?;
                    validate_published_result_identity(
                        self.plan.root(),
                        &result.registration,
                        registered,
                    )?;
                }
                AnalysisLayerBuilder::Mechanisms(mechanism) => {
                    mechanism
                        .incidence
                        .validate_complete()
                        .map_err(RelationalAnalysisCatalogError::Mechanism)?;
                }
            }
        }
        Ok(())
    }

    fn result_registration(
        &self,
        view_id: ViewId,
    ) -> Result<&RelationalResultLayerRegistration, RelationalAnalysisCatalogError> {
        Ok(&self.result_layer(view_id)?.registration)
    }

    fn mechanism_registration(
        &self,
        request_id: MechanismRequestId,
    ) -> Result<&RelationalMechanismLayerRegistration, RelationalAnalysisCatalogError> {
        Ok(&self.mechanism_layer(request_id)?.registration)
    }

    fn result_layer(
        &self,
        view_id: ViewId,
    ) -> Result<&ResultLayerBuilder, RelationalAnalysisCatalogError> {
        match self.layers.get(&RelationalAnalysisLayerId::Result(view_id)) {
            Some(AnalysisLayerBuilder::Result(result)) => Ok(result),
            _ => Err(RelationalAnalysisCatalogError::UnknownResultLayer { view_id }),
        }
    }

    fn result_layer_mut(
        &mut self,
        view_id: ViewId,
    ) -> Result<&mut ResultLayerBuilder, RelationalAnalysisCatalogError> {
        match self
            .layers
            .get_mut(&RelationalAnalysisLayerId::Result(view_id))
        {
            Some(AnalysisLayerBuilder::Result(result)) => Ok(result),
            _ => Err(RelationalAnalysisCatalogError::UnknownResultLayer { view_id }),
        }
    }

    fn registered_result(
        &self,
        view_id: ViewId,
    ) -> Result<&RegisteredResultLayer, RelationalAnalysisCatalogError> {
        self.result_layer(view_id)?
            .registered
            .as_ref()
            .ok_or(RelationalAnalysisCatalogError::ResultSpecNotRegistered { view_id })
    }

    fn registered_result_mut(
        &mut self,
        view_id: ViewId,
    ) -> Result<&mut RegisteredResultLayer, RelationalAnalysisCatalogError> {
        self.result_layer_mut(view_id)?
            .registered
            .as_mut()
            .ok_or(RelationalAnalysisCatalogError::ResultSpecNotRegistered { view_id })
    }

    fn mechanism_layer(
        &self,
        request_id: MechanismRequestId,
    ) -> Result<&MechanismLayerBuilder, RelationalAnalysisCatalogError> {
        match self
            .layers
            .get(&RelationalAnalysisLayerId::Mechanisms(request_id))
        {
            Some(AnalysisLayerBuilder::Mechanisms(mechanism)) => Ok(mechanism),
            _ => Err(RelationalAnalysisCatalogError::UnknownMechanismLayer { request_id }),
        }
    }

    fn mechanism_layer_mut(
        &mut self,
        request_id: MechanismRequestId,
    ) -> Result<&mut MechanismLayerBuilder, RelationalAnalysisCatalogError> {
        match self
            .layers
            .get_mut(&RelationalAnalysisLayerId::Mechanisms(request_id))
        {
            Some(AnalysisLayerBuilder::Mechanisms(mechanism)) => Ok(mechanism),
            _ => Err(RelationalAnalysisCatalogError::UnknownMechanismLayer { request_id }),
        }
    }

    fn require_matching_publication(
        &self,
        view: &ClosedResultView,
    ) -> Result<RelationalResultPublication, RelationalAnalysisCatalogError> {
        let registered = self.registered_result(view.view_id())?;
        let publication =
            registered
                .publication
                .ok_or(RelationalAnalysisCatalogError::ResultNotPublished {
                    view_id: view.view_id(),
                })?;
        publication.validate_for(
            self.plan.root(),
            view.view_id(),
            registered.spec.spec_root(),
            registered.evidence.root(),
        )?;
        if publication.result_root() != view.root()
            || view.snapshot().spec() != &registered.spec
            || !same_contributions(&registered.evidence, view.snapshot().contributions())
        {
            return Err(
                RelationalAnalysisCatalogError::PublishedResultRootMismatch {
                    view_id: view.view_id(),
                },
            );
        }
        Ok(publication)
    }

    /// Revalidate every sealed DAG edge against its current local upstream
    /// content. A subordinate catalog can remain internally valid while a
    /// later upstream mutation makes its previously accepted seal stale.
    fn validate_analysis_edges(&self) -> Result<(), RelationalAnalysisCatalogError> {
        for (layer_id, layer) in &self.layers {
            match layer {
                AnalysisLayerBuilder::Result(result) => {
                    let Some(registered) = result.registered.as_ref() else {
                        continue;
                    };
                    validate_certified_source_binding(
                        self.plan.root(),
                        &result.registration,
                        registered,
                    )?;
                    let Some(seal) = registered.evidence.input_seal() else {
                        continue;
                    };
                    let valid = match (result.registration.input(), seal.upstream()) {
                        (
                            RelationalResolvedResultInput::Sources(expected),
                            ResultEvidenceUpstreamRoot::Sources {
                                relation_id: actual,
                                ..
                            }
                            | ResultEvidenceUpstreamRoot::CertifiedSources {
                                relation_id: actual,
                                ..
                            },
                        ) => expected == actual,
                        (
                            RelationalResolvedResultInput::Selected(expected),
                            ResultEvidenceUpstreamRoot::Selected {
                                question_id: actual,
                                ..
                            }
                            | ResultEvidenceUpstreamRoot::CertifiedSelectedSupport {
                                question_id: actual,
                                ..
                            },
                        ) => expected == actual,
                        (
                            RelationalResolvedResultInput::MechanismIncidence(expected),
                            ResultEvidenceUpstreamRoot::MechanismIncidence {
                                request_id: actual,
                                completed_root,
                            }
                            | ResultEvidenceUpstreamRoot::StructuralMechanismIncidence {
                                request_id: actual,
                                completed_root,
                                ..
                            },
                        ) if expected == actual => {
                            self.mechanism_layer(expected).is_ok_and(|upstream| {
                                upstream.incidence.frontier_is_complete()
                                    && upstream.incidence.root() == completed_root
                            })
                        }
                        _ => false,
                    };
                    if !valid {
                        return Err(
                            RelationalAnalysisCatalogError::AnalysisDependencyEvidenceMismatch {
                                layer_id: *layer_id,
                            },
                        );
                    }
                }
                AnalysisLayerBuilder::Mechanisms(mechanism) => {
                    let Some(seal) = mechanism.incidence.target_seal() else {
                        continue;
                    };
                    seal.validate_identity()
                        .map_err(RelationalAnalysisCatalogError::Mechanism)?;
                    let valid = match (mechanism.registration.target(), seal.upstream()) {
                        (
                            RelationalResolvedMechanismTarget::Selected(expected),
                            MechanismTargetSealUpstream::SelectedQuestion { .. }
                            | MechanismTargetSealUpstream::CertifiedSelectedSupport { .. },
                        ) => expected == seal.scope().question_id(),
                        (
                            RelationalResolvedMechanismTarget::ChosenView(expected),
                            MechanismTargetSealUpstream::ChosenResultView {
                                view_id: actual,
                                root,
                            },
                        ) if expected == actual => self
                            .registered_result(expected)
                            .ok()
                            .and_then(|result| result.publication)
                            .is_some_and(|publication| publication.result_root() == root),
                        _ => false,
                    };
                    if !valid {
                        return Err(
                            RelationalAnalysisCatalogError::AnalysisDependencyEvidenceMismatch {
                                layer_id: *layer_id,
                            },
                        );
                    }
                }
            }
        }
        Ok(())
    }

    fn restore_result_layer(
        &mut self,
        snapshot: RelationalResultLayerSnapshot,
    ) -> Result<(), RelationalAnalysisCatalogError> {
        let registration = self.result_registration(snapshot.view_id)?.clone();
        if registration.input() != snapshot.input
            || registration.semantic_spec_digest() != snapshot.semantic_spec_digest
        {
            return Err(
                RelationalAnalysisCatalogError::SnapshotLayerMetadataMismatch {
                    layer_id: RelationalAnalysisLayerId::Result(snapshot.view_id),
                },
            );
        }
        match snapshot.state {
            RelationalResultLayerSnapshotState::Unregistered => Ok(()),
            RelationalResultLayerSnapshotState::Registered {
                spec,
                evidence,
                projection,
                certified_source_summary,
                publication,
            } => {
                if let Some(seal) = evidence.input_seal() {
                    reject_standalone_structural_result_snapshot(
                        snapshot.view_id,
                        seal.upstream(),
                    )?;
                }
                self.register_result_spec(snapshot.view_id, snapshot.input, spec.clone())?;
                let rebuilt =
                    RelationalResultEvidenceCatalogBuilder::from_snapshot(evidence, &spec)
                        .map_err(RelationalAnalysisCatalogError::ResultEvidence)?;
                let rebuilt_projection =
                    ResultProjectionCatalogBuilder::from_snapshot(projection, &spec)
                        .map_err(RelationalAnalysisCatalogError::ResultProjection)?;
                let mut rebuilt = RegisteredResultLayer {
                    spec,
                    evidence: rebuilt,
                    projection: rebuilt_projection,
                    certified_source_summary,
                    publication,
                    validated_publication: None,
                };
                validate_certified_source_binding(self.plan.root(), &registration, &rebuilt)?;
                if let Some(publication) = publication {
                    if !rebuilt.evidence.input_is_sealed() {
                        return Err(
                            RelationalAnalysisCatalogError::PublishedResultEvidenceMismatch {
                                view_id: snapshot.view_id,
                            },
                        );
                    }
                    publication.validate_for(
                        self.plan.root(),
                        snapshot.view_id,
                        rebuilt.spec.spec_root(),
                        rebuilt.evidence.root(),
                    )?;
                    let closure = prepare_registered_result_projection_closure(
                        self.plan.root(),
                        &registration,
                        &rebuilt,
                    )?;
                    if closure.result_root() != publication.result_root() {
                        return Err(
                            RelationalAnalysisCatalogError::PublishedResultRootMismatch {
                                view_id: snapshot.view_id,
                            },
                        );
                    }
                    rebuilt.validated_publication = Some(
                        ValidatedResultPublication::after_full_validation(publication, closure)?,
                    );
                    validate_published_result_identity(self.plan.root(), &registration, &rebuilt)?;
                }
                *self.registered_result_mut(snapshot.view_id)? = rebuilt;
                Ok(())
            }
        }
    }

    fn restore_mechanism_layer(
        &mut self,
        snapshot: RelationalMechanismLayerSnapshot,
    ) -> Result<(), RelationalAnalysisCatalogError> {
        let layer = self.mechanism_layer_mut(snapshot.request_id)?;
        if layer.registration.target() != snapshot.target
            || layer.registration.observation_id() != snapshot.observation_id
            || layer.registration.observation_digest() != snapshot.observation_digest
        {
            return Err(
                RelationalAnalysisCatalogError::SnapshotLayerMetadataMismatch {
                    layer_id: RelationalAnalysisLayerId::Mechanisms(snapshot.request_id),
                },
            );
        }
        let expected_scope = layer.incidence.scope();
        layer.incidence =
            MechanismIncidenceCatalogBuilder::from_snapshot(snapshot.incidence, expected_scope)
                .map_err(RelationalAnalysisCatalogError::Mechanism)?;
        Ok(())
    }
}

/// Immutable exact analysis evidence. The constructor is private to
/// all-layer validation in [`RelationalAnalysisCatalogBuilder::finish`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClosedRelationalAnalysisCatalog {
    snapshot: RelationalAnalysisCatalogSnapshot,
}

impl ClosedRelationalAnalysisCatalog {
    pub(crate) const fn plan_root(&self) -> RelationalAnalysisPlanRoot {
        self.snapshot.plan_root
    }

    pub(crate) const fn root(&self) -> RelationalAnalysisCatalogRoot {
        self.snapshot.root
    }

    pub(crate) const fn snapshot(&self) -> &RelationalAnalysisCatalogSnapshot {
        &self.snapshot
    }

    /// Borrow the compact source-summary artifact retained in an exact result
    /// layer. Cold-invocation theorem rebinding must remain possible after the
    /// mutable catalog has been consumed by final analysis closure.
    pub(crate) fn certified_source_summary(
        &self,
        view_id: ViewId,
    ) -> Option<&RelationalCertifiedSourceSummaryArtifact> {
        let RelationalAnalysisLayerSnapshot::Result(result) = self
            .snapshot
            .layer(RelationalAnalysisLayerId::Result(view_id))?
        else {
            return None;
        };
        let RelationalResultLayerSnapshotState::Registered {
            certified_source_summary,
            ..
        } = result.state()
        else {
            return None;
        };
        certified_source_summary.as_ref()
    }

    /// Borrow one request's mechanism payload after the final analysis-close
    /// move. Operational closure authority remains in the journal receipt;
    /// this accessor serves definition and reason publication only.
    pub(crate) fn mechanism_incidence(
        &self,
        request_id: MechanismRequestId,
    ) -> Result<&MechanismIncidenceSnapshot, RelationalAnalysisCatalogError> {
        let layer = self
            .snapshot
            .layer(RelationalAnalysisLayerId::Mechanisms(request_id))
            .ok_or(RelationalAnalysisCatalogError::UnknownMechanismLayer { request_id })?;
        let RelationalAnalysisLayerSnapshot::Mechanisms(mechanism) = layer else {
            return Err(RelationalAnalysisCatalogError::UnknownMechanismLayer { request_id });
        };
        Ok(mechanism.incidence())
    }

    /// Rebuild one exact published projection from the closed snapshot. This
    /// is the final-reporting path after the mutable catalog has been consumed;
    /// it requires no expression runtime and trusts no detached row count.
    pub(crate) fn materialize_published_result(
        &self,
        view_id: ViewId,
    ) -> Result<ClosedResultView, RelationalAnalysisCatalogError> {
        let layer = self
            .snapshot
            .layer(RelationalAnalysisLayerId::Result(view_id))
            .ok_or(RelationalAnalysisCatalogError::UnknownResultLayer { view_id })?;
        let RelationalAnalysisLayerSnapshot::Result(result) = layer else {
            return Err(RelationalAnalysisCatalogError::UnknownResultLayer { view_id });
        };
        let RelationalResultLayerSnapshotState::Registered {
            spec,
            evidence,
            projection,
            certified_source_summary,
            publication: Some(publication),
        } = result.state()
        else {
            return Err(RelationalAnalysisCatalogError::ResultNotPublished { view_id });
        };
        if certified_source_summary.is_some() {
            return Err(
                RelationalAnalysisCatalogError::CertifiedSourceResultRequiresCompactMaterialization {
                    view_id,
                },
            );
        }
        publication.validate_for(
            self.snapshot.plan_root,
            view_id,
            spec.spec_root(),
            evidence.root(),
        )?;
        let evidence =
            RelationalResultEvidenceCatalogBuilder::from_snapshot(evidence.clone(), spec)
                .map_err(RelationalAnalysisCatalogError::ResultEvidence)?;
        let projection = ResultProjectionCatalogBuilder::from_snapshot(projection.clone(), spec)
            .map_err(RelationalAnalysisCatalogError::ResultProjection)?;
        let closed = projection
            .materialize_closed(spec, &evidence)
            .map_err(RelationalAnalysisCatalogError::ResultProjection)?;
        if closed.root() != publication.result_root() {
            return Err(RelationalAnalysisCatalogError::PublishedResultRootMismatch { view_id });
        }
        Ok(closed)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalAnalysisCatalogError {
    InvalidPlanRoot,
    DuplicatePlanLayer {
        layer_id: RelationalAnalysisLayerId,
    },
    InvalidPlanDependency {
        layer_id: RelationalAnalysisLayerId,
    },
    ForeignQuestionDependency {
        layer_id: RelationalAnalysisLayerId,
        expected: QuestionId,
        actual: QuestionId,
    },
    UnknownResultLayer {
        view_id: ViewId,
    },
    UnknownMechanismLayer {
        request_id: MechanismRequestId,
    },
    ResultViewIdMismatch {
        expected: ViewId,
        actual: ViewId,
    },
    ResultInputDependencyMismatch {
        view_id: ViewId,
    },
    MechanismResultSealAuthorityUnavailable {
        view_id: ViewId,
    },
    ResultInputKindMismatch {
        view_id: ViewId,
        expected: ResultViewInputKind,
        actual: ResultViewInputKind,
    },
    ResultSpecReplacement {
        view_id: ViewId,
    },
    ResultSpecNotRegistered {
        view_id: ViewId,
    },
    CertifiedSourceSummaryScopeMismatch {
        view_id: ViewId,
    },
    CertifiedSourceSummaryConflict {
        view_id: ViewId,
    },
    CertifiedSourceSummaryMissing {
        view_id: ViewId,
    },
    CertifiedSourceResultRequiresCompactMaterialization {
        view_id: ViewId,
    },
    QuestionScopeMismatch {
        expected: QuestionId,
        actual: QuestionId,
    },
    MechanismRequestMismatch {
        expected: MechanismRequestId,
        actual: MechanismRequestId,
    },
    MechanismUpstreamMismatch {
        request_id: MechanismRequestId,
    },
    MechanismTargetDependencyMismatch {
        request_id: MechanismRequestId,
    },
    MechanismLayerAlreadyClosed {
        request_id: MechanismRequestId,
    },
    AnalysisDependencyEvidenceMismatch {
        layer_id: RelationalAnalysisLayerId,
    },
    ResultEvidenceFrontierOpen {
        view_id: ViewId,
    },
    ResultProjectionFrontierOpen {
        view_id: ViewId,
    },
    ResultProjectionAlreadyPublished {
        view_id: ViewId,
    },
    PublishedResultSpecMismatch {
        view_id: ViewId,
    },
    PublishedResultEvidenceMismatch {
        view_id: ViewId,
    },
    PublishedResultValidationMissing {
        view_id: ViewId,
    },
    ResultPublicationConflict {
        view_id: ViewId,
    },
    ResultNotPublished {
        view_id: ViewId,
    },
    PublishedResultRootMismatch {
        view_id: ViewId,
    },
    UnsupportedResultPublicationVersion {
        actual: u32,
        expected: u32,
    },
    ResultPublicationScopeMismatch {
        view_id: ViewId,
    },
    ResultPublicationIdMismatch {
        claimed: RelationalResultPublicationId,
        derived: RelationalResultPublicationId,
    },
    AnalysisFrontierOpen {
        layer_id: RelationalAnalysisLayerId,
        status: RelationalAnalysisLayerStatus,
    },
    UnsupportedSnapshotVersion {
        actual: u32,
        expected: u32,
    },
    SnapshotPlanMismatch,
    SnapshotLayerSetMismatch,
    SnapshotLayerMetadataMismatch {
        layer_id: RelationalAnalysisLayerId,
    },
    NonCanonicalSnapshotOrder,
    SnapshotRootMismatch,
    ResultSpec(super::result_view::ResultViewError),
    ResultEvidence(ResultEvidenceError),
    ResultProjection(ResultProjectionError),
    Mechanism(MechanismIncidenceError),
}

impl fmt::Display for RelationalAnalysisCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPlanRoot => formatter
                .write_str("relational analysis plan root does not match its canonical payload"),
            Self::DuplicatePlanLayer { .. } => {
                formatter.write_str("relational analysis plan contains a duplicate semantic layer")
            }
            Self::InvalidPlanDependency { .. } => formatter.write_str(
                "relational analysis layer dependency does not match its resolved input or target",
            ),
            Self::ForeignQuestionDependency { .. } => formatter.write_str(
                "relational analysis layer refers to a different FIND question than its plan",
            ),
            Self::UnknownResultLayer { .. } => {
                formatter.write_str("result view is not declared by this analysis plan")
            }
            Self::UnknownMechanismLayer { .. } => {
                formatter.write_str("mechanism request is not declared by this analysis plan")
            }
            Self::ResultViewIdMismatch { .. } => formatter
                .write_str("result-view spec or publication has the wrong semantic view ID"),
            Self::ResultInputDependencyMismatch { .. } => formatter.write_str(
                "result-view operation names a different resolved input than the analysis plan",
            ),
            Self::MechanismResultSealAuthorityUnavailable { .. } => formatter.write_str(
                "mechanism-incidence result seals require journal-validated incidence and structural-quotient authority",
            ),
            Self::ResultInputKindMismatch { .. } => formatter.write_str(
                "result-view spec input kind does not match its plan-resolved dependency",
            ),
            Self::ResultSpecReplacement { .. } => {
                formatter.write_str("a registered result-view spec cannot be replaced")
            }
            Self::ResultSpecNotRegistered { .. } => {
                formatter.write_str("result-view evidence arrived before its spec registration")
            }
            Self::CertifiedSourceSummaryScopeMismatch { .. } => formatter.write_str(
                "certified source-summary artifact does not match its analysis result layer",
            ),
            Self::CertifiedSourceSummaryConflict { .. } => formatter.write_str(
                "result layer already contains different or incompatible source-summary evidence",
            ),
            Self::CertifiedSourceSummaryMissing { .. } => formatter.write_str(
                "certified source result seal has no retained source-summary artifact",
            ),
            Self::CertifiedSourceResultRequiresCompactMaterialization { .. } => formatter.write_str(
                "certified source result requires the compact proof-specialized materialization path",
            ),
            Self::QuestionScopeMismatch { .. } => {
                formatter.write_str("closed FIND catalog does not match the plan-resolved question")
            }
            Self::MechanismRequestMismatch { .. } => {
                formatter.write_str("closed mechanism incidence has the wrong request identity")
            }
            Self::MechanismUpstreamMismatch { .. } => formatter.write_str(
                "closed mechanism incidence is not the exact completed local plan layer",
            ),
            Self::MechanismTargetDependencyMismatch { .. } => formatter
                .write_str("mechanism target closure does not match the plan-resolved target edge"),
            Self::MechanismLayerAlreadyClosed { .. } => formatter
                .write_str("completed mechanism evidence cannot accept a new signature definition"),
            Self::AnalysisDependencyEvidenceMismatch { .. } => formatter.write_str(
                "sealed analysis-layer evidence no longer matches its exact local upstream root",
            ),
            Self::ResultEvidenceFrontierOpen { .. } => formatter
                .write_str("result projection cannot publish before exact input evidence closes"),
            Self::ResultProjectionFrontierOpen { .. } => {
                formatter.write_str("only an exact closed result view can be published")
            }
            Self::ResultProjectionAlreadyPublished { .. } => formatter
                .write_str("a published result projection cannot accept a new output record"),
            Self::PublishedResultSpecMismatch { .. } => formatter
                .write_str("closed result view does not use the registered checked reducer spec"),
            Self::PublishedResultEvidenceMismatch { .. } => formatter.write_str(
                "closed result view contributions do not equal the exact evidence catalog",
            ),
            Self::PublishedResultValidationMissing { .. } => formatter.write_str(
                "published result is missing its process-local full-validation witness",
            ),
            Self::ResultPublicationConflict { .. } => formatter
                .write_str("result layer already has a different projected publication root"),
            Self::ResultNotPublished { .. } => formatter.write_str(
                "downstream mechanism target requires a published result-view dependency",
            ),
            Self::PublishedResultRootMismatch { .. } => formatter
                .write_str("closed result view does not match its local publication receipt"),
            Self::UnsupportedResultPublicationVersion { actual, expected } => write!(
                formatter,
                "unsupported result publication version {actual}; expected {expected}"
            ),
            Self::ResultPublicationScopeMismatch { .. } => formatter.write_str(
                "result publication receipt does not match its plan, spec, or evidence layer",
            ),
            Self::ResultPublicationIdMismatch { .. } => formatter
                .write_str("result publication ID does not match its canonical semantic content"),
            Self::AnalysisFrontierOpen { .. } => formatter
                .write_str("relational analysis cannot finish while a declared layer is open"),
            Self::UnsupportedSnapshotVersion { actual, expected } => write!(
                formatter,
                "unsupported relational analysis snapshot version {actual}; expected {expected}"
            ),
            Self::SnapshotPlanMismatch => {
                formatter.write_str("relational analysis snapshot belongs to another analysis plan")
            }
            Self::SnapshotLayerSetMismatch => formatter.write_str(
                "relational analysis snapshot does not contain exactly the declared plan layers",
            ),
            Self::SnapshotLayerMetadataMismatch { .. } => formatter
                .write_str("relational analysis snapshot layer metadata disagrees with the plan"),
            Self::NonCanonicalSnapshotOrder => formatter.write_str(
                "relational analysis snapshot layers are not strictly ordered by semantic ID",
            ),
            Self::SnapshotRootMismatch => formatter.write_str(
                "relational analysis snapshot root does not authenticate reconstructed content",
            ),
            Self::ResultSpec(error) => error.fmt(formatter),
            Self::ResultEvidence(error) => error.fmt(formatter),
            Self::ResultProjection(error) => error.fmt(formatter),
            Self::Mechanism(error) => error.fmt(formatter),
        }
    }
}

impl Error for RelationalAnalysisCatalogError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ResultSpec(error) => Some(error),
            Self::ResultEvidence(error) => Some(error),
            Self::ResultProjection(error) => Some(error),
            Self::Mechanism(error) => Some(error),
            _ => None,
        }
    }
}

/// A generic durable receipt carries no structural-quotient authority. Keep
/// mechanism incidence on the dedicated replay path, where the journal first
/// authenticates both the completed incidence root and the structural root.
fn reject_generic_mechanism_result_seal(
    view_id: ViewId,
    upstream: ResultEvidenceUpstreamRoot,
) -> Result<(), RelationalAnalysisCatalogError> {
    if matches!(
        upstream,
        ResultEvidenceUpstreamRoot::MechanismIncidence { .. }
            | ResultEvidenceUpstreamRoot::StructuralMechanismIncidence { .. }
    ) {
        return Err(
            RelationalAnalysisCatalogError::MechanismResultSealAuthorityUnavailable { view_id },
        );
    }
    Ok(())
}

/// A standalone analysis-catalog snapshot contains the claimed structural
/// root but not the structural quotient catalog that authenticates it. Such a
/// seal is restorable only by replaying the outer relational journal.
fn reject_standalone_structural_result_snapshot(
    view_id: ViewId,
    upstream: ResultEvidenceUpstreamRoot,
) -> Result<(), RelationalAnalysisCatalogError> {
    if matches!(
        upstream,
        ResultEvidenceUpstreamRoot::StructuralMechanismIncidence { .. }
    ) {
        return Err(
            RelationalAnalysisCatalogError::MechanismResultSealAuthorityUnavailable { view_id },
        );
    }
    Ok(())
}

fn validate_registration(
    plan_question_id: QuestionId,
    registration: &RelationalAnalysisLayerRegistration,
) -> Result<(), RelationalAnalysisCatalogError> {
    let (layer_id, expected_dependency) = match registration {
        RelationalAnalysisLayerRegistration::Result(result) => (
            RelationalAnalysisLayerId::Result(result.view_id()),
            match result.input() {
                RelationalResolvedResultInput::Sources(relation_id) => {
                    RelationalAnalysisDependencyId::Relation(relation_id)
                }
                RelationalResolvedResultInput::Selected(question_id) => {
                    RelationalAnalysisDependencyId::Question(question_id)
                }
                RelationalResolvedResultInput::MechanismIncidence(request_id) => {
                    RelationalAnalysisDependencyId::Mechanisms(request_id)
                }
            },
        ),
        RelationalAnalysisLayerRegistration::Mechanisms(mechanism) => (
            RelationalAnalysisLayerId::Mechanisms(mechanism.request_id()),
            match mechanism.target() {
                RelationalResolvedMechanismTarget::Selected(question_id) => {
                    RelationalAnalysisDependencyId::Question(question_id)
                }
                RelationalResolvedMechanismTarget::ChosenView(view_id) => {
                    RelationalAnalysisDependencyId::Result(view_id)
                }
            },
        ),
    };
    if registration.dependencies() != std::slice::from_ref(&expected_dependency) {
        return Err(RelationalAnalysisCatalogError::InvalidPlanDependency { layer_id });
    }
    match expected_dependency {
        RelationalAnalysisDependencyId::Question(actual) if actual != plan_question_id => {
            return Err(RelationalAnalysisCatalogError::ForeignQuestionDependency {
                layer_id,
                expected: plan_question_id,
                actual,
            });
        }
        RelationalAnalysisDependencyId::Relation(_)
        | RelationalAnalysisDependencyId::Question(_)
        | RelationalAnalysisDependencyId::Result(_)
        | RelationalAnalysisDependencyId::Mechanisms(_) => {}
    }
    Ok(())
}

fn mechanism_scope(
    question_id: QuestionId,
    registration: &RelationalMechanismLayerRegistration,
) -> MechanismRequestScope {
    let target = match registration.target() {
        RelationalResolvedMechanismTarget::Selected(_) => MechanismTargetId::Selected,
        RelationalResolvedMechanismTarget::ChosenView(view_id) => {
            MechanismTargetId::ChosenView(view_id)
        }
    };
    MechanismRequestScope::new(registration.request_id(), question_id, target)
}

const fn result_input_kind(input: RelationalResolvedResultInput) -> ResultViewInputKind {
    match input {
        RelationalResolvedResultInput::Sources(_) => ResultViewInputKind::Source,
        RelationalResolvedResultInput::Selected(_) => ResultViewInputKind::Case,
        RelationalResolvedResultInput::MechanismIncidence(_) => ResultViewInputKind::Incidence,
    }
}

fn validate_question(
    expected: QuestionId,
    question: &QuestionCatalog,
) -> Result<(), RelationalAnalysisCatalogError> {
    if question.question_id() == expected {
        Ok(())
    } else {
        Err(RelationalAnalysisCatalogError::QuestionScopeMismatch {
            expected,
            actual: question.question_id(),
        })
    }
}

fn same_contributions(
    evidence: &RelationalResultEvidenceCatalogBuilder,
    contributions: &[super::result_view::EvaluatedResultContribution],
) -> bool {
    evidence.len() == contributions.len()
        && evidence
            .records()
            .zip(contributions)
            .all(|(record, contribution)| record.contribution() == contribution)
}

fn validate_certified_source_binding(
    plan_root: RelationalAnalysisPlanRoot,
    registration: &RelationalResultLayerRegistration,
    registered: &RegisteredResultLayer,
) -> Result<(), RelationalAnalysisCatalogError> {
    let sealed_artifact_id = registered
        .evidence
        .input_seal()
        .and_then(RelationalResultInputSeal::certified_source_summary_artifact_id);
    match &registered.certified_source_summary {
        Some(artifact) => {
            let expected_seal = RelationalResultInputSeal::from_certified_source_summary(artifact);
            if !artifact.validate_identity()
                || artifact.analysis_plan_root() != plan_root
                || artifact.view_id() != registration.view_id()
                || artifact.spec_root() != registered.spec.spec_root()
                || artifact.semantic_spec_digest() != registration.semantic_spec_digest()
                || registration.input()
                    != RelationalResolvedResultInput::Sources(artifact.relation_id())
                || registered.evidence.input_seal() != Some(expected_seal)
                || !registered.evidence.is_empty()
            {
                return Err(
                    RelationalAnalysisCatalogError::CertifiedSourceSummaryScopeMismatch {
                        view_id: registration.view_id(),
                    },
                );
            }
        }
        None if sealed_artifact_id.is_some() => {
            return Err(
                RelationalAnalysisCatalogError::CertifiedSourceSummaryMissing {
                    view_id: registration.view_id(),
                },
            );
        }
        None => {}
    }
    Ok(())
}

/// Rebuild one published result at a typed-snapshot restoration boundary.
/// Live journal replay performs this full check when `ResultViewPublished` is
/// first applied and retains only [`ValidatedResultPublication`] afterward.
fn prepare_registered_result_projection_closure(
    plan_root: RelationalAnalysisPlanRoot,
    registration: &RelationalResultLayerRegistration,
    registered: &RegisteredResultLayer,
) -> Result<ResultProjectionClosure, RelationalAnalysisCatalogError> {
    validate_certified_source_binding(plan_root, registration, registered)?;
    match &registered.certified_source_summary {
        Some(artifact) => registered
            .projection
            .closure_from_certified_source_groups(
                &registered.spec,
                artifact.certified_input_root(),
                artifact.exact_cardinality(),
                artifact.groups(),
            )
            .map_err(RelationalAnalysisCatalogError::ResultProjection),
        None => registered
            .projection
            .closure_from_durable(&registered.spec, &registered.evidence)
            .map_err(RelationalAnalysisCatalogError::ResultProjection),
    }
}

/// Revalidate a publication after its evidence and projection frontiers have
/// become immutable. The private witness was minted only by a full result
/// reconstruction, so terminal callers need compare only stable identities
/// and exact summary counts; no output row or group is rebuilt here.
fn validate_published_result_identity(
    plan_root: RelationalAnalysisPlanRoot,
    registration: &RelationalResultLayerRegistration,
    registered: &RegisteredResultLayer,
) -> Result<(), RelationalAnalysisCatalogError> {
    let view_id = registration.view_id();
    validate_certified_source_binding(plan_root, registration, registered)?;
    registered
        .evidence
        .validate_complete()
        .map_err(RelationalAnalysisCatalogError::ResultEvidence)?;
    let publication = registered
        .publication
        .ok_or(RelationalAnalysisCatalogError::ResultNotPublished { view_id })?;
    publication.validate_for(
        plan_root,
        view_id,
        registered.spec.spec_root(),
        registered.evidence.root(),
    )?;
    let validated = registered
        .validated_publication
        .ok_or(RelationalAnalysisCatalogError::PublishedResultValidationMissing { view_id })?;
    validated.validate_unchanged(
        publication,
        &registered.spec,
        &registered.evidence,
        &registered.projection,
    )
}

fn strictly_sorted_layer_ids(layers: &[RelationalAnalysisLayerSnapshot]) -> bool {
    layers
        .windows(2)
        .all(|pair| pair[0].layer_id() < pair[1].layer_id())
}

fn derive_result_publication_id(
    version: u32,
    plan_root: RelationalAnalysisPlanRoot,
    view_id: ViewId,
    spec_root: ResultViewSpecRoot,
    evidence_root: RelationalResultEvidenceRoot,
    result_root: ResultViewRoot,
) -> RelationalResultPublicationId {
    let mut hasher = AnalysisCatalogHasher::new(RESULT_PUBLICATION_ID_V1);
    hasher.u32(version);
    hasher.digest(plan_root.bytes());
    hasher.digest(view_id.bytes());
    hasher.digest(spec_root.bytes());
    hasher.digest(evidence_root.bytes());
    hasher.digest(result_root.bytes());
    RelationalResultPublicationId(hasher.finish())
}

fn derive_analysis_catalog_root(
    plan_root: RelationalAnalysisPlanRoot,
    layers: &[RelationalAnalysisLayerSnapshot],
) -> RelationalAnalysisCatalogRoot {
    let mut hasher = AnalysisCatalogHasher::new(ANALYSIS_CATALOG_ROOT_V4);
    hasher.u32(RELATIONAL_ANALYSIS_CATALOG_SNAPSHOT_VERSION);
    hasher.digest(plan_root.bytes());
    hasher.u128(layers.len() as u128);
    for layer in layers {
        hash_layer_snapshot(&mut hasher, layer);
    }
    RelationalAnalysisCatalogRoot(hasher.finish())
}

fn derive_analysis_catalog_builder_root(
    plan_root: RelationalAnalysisPlanRoot,
    layers: &BTreeMap<RelationalAnalysisLayerId, AnalysisLayerBuilder>,
) -> RelationalAnalysisCatalogRoot {
    let mut hasher = AnalysisCatalogHasher::new(ANALYSIS_CATALOG_ROOT_V4);
    hasher.u32(RELATIONAL_ANALYSIS_CATALOG_SNAPSHOT_VERSION);
    hasher.digest(plan_root.bytes());
    hasher.u128(layers.len() as u128);
    for layer in layers.values() {
        hash_layer_builder(&mut hasher, layer);
    }
    RelationalAnalysisCatalogRoot(hasher.finish())
}

#[derive(Clone, Copy)]
enum ResultLayerHashState {
    Unregistered,
    Registered {
        spec_root: ResultViewSpecRoot,
        evidence_root: RelationalResultEvidenceRoot,
        input_is_sealed: bool,
        projection_root: ResultProjectionRoot,
        projection_record_count: u128,
        certified_source_summary_id: Option<RelationalCertifiedSourceSummaryArtifactId>,
        publication: Option<RelationalResultPublication>,
    },
}

fn hash_result_layer(
    hasher: &mut AnalysisCatalogHasher,
    view_id: ViewId,
    input: RelationalResolvedResultInput,
    semantic_spec_digest: RelationalResultSpecDigest,
    state: ResultLayerHashState,
) {
    hasher.tag(0x01);
    hasher.digest(view_id.bytes());
    hash_result_input(hasher, input);
    hasher.digest(semantic_spec_digest.bytes());
    match state {
        ResultLayerHashState::Unregistered => hasher.tag(0x01),
        ResultLayerHashState::Registered {
            spec_root,
            evidence_root,
            input_is_sealed,
            projection_root,
            projection_record_count,
            certified_source_summary_id,
            publication,
        } => {
            hasher.tag(0x02);
            hasher.digest(spec_root.bytes());
            hasher.digest(evidence_root.bytes());
            hasher.bool(input_is_sealed);
            hasher.digest(projection_root.bytes());
            hasher.u128(projection_record_count);
            match certified_source_summary_id {
                None => hasher.tag(0x01),
                Some(artifact_id) => {
                    hasher.tag(0x02);
                    hasher.digest(artifact_id.bytes());
                }
            }
            match publication {
                None => hasher.tag(0x01),
                Some(publication) => {
                    hasher.tag(0x02);
                    hash_publication(hasher, publication);
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn hash_mechanism_layer(
    hasher: &mut AnalysisCatalogHasher,
    request_id: MechanismRequestId,
    target: RelationalResolvedMechanismTarget,
    observation_id: RelationalMechanismObservationId,
    observation_digest: RelationalMechanismObservationDigest,
    incidence_root: super::mechanism_incidence::MechanismIncidenceRoot,
    target_is_sealed: bool,
    frontier_is_complete: bool,
) {
    hasher.tag(0x02);
    hasher.digest(request_id.bytes());
    hash_mechanism_target(hasher, target);
    hasher.digest(observation_id.bytes());
    hasher.digest(observation_digest.bytes());
    hasher.digest(incidence_root.bytes());
    hasher.bool(target_is_sealed);
    hasher.bool(frontier_is_complete);
}

fn hash_layer_builder(hasher: &mut AnalysisCatalogHasher, layer: &AnalysisLayerBuilder) {
    match layer {
        AnalysisLayerBuilder::Result(result) => {
            let state = match result.registered.as_ref() {
                None => ResultLayerHashState::Unregistered,
                Some(registered) => ResultLayerHashState::Registered {
                    spec_root: registered.spec.spec_root(),
                    evidence_root: registered.evidence.root(),
                    input_is_sealed: registered.evidence.input_is_sealed(),
                    projection_root: registered.projection.root(),
                    projection_record_count: registered.projection.len() as u128,
                    certified_source_summary_id: registered
                        .certified_source_summary
                        .as_ref()
                        .map(RelationalCertifiedSourceSummaryArtifact::artifact_id),
                    publication: registered.publication,
                },
            };
            hash_result_layer(
                hasher,
                result.registration.view_id(),
                result.registration.input(),
                result.registration.semantic_spec_digest(),
                state,
            );
        }
        AnalysisLayerBuilder::Mechanisms(mechanism) => hash_mechanism_layer(
            hasher,
            mechanism.registration.request_id(),
            mechanism.registration.target(),
            mechanism.registration.observation_id(),
            mechanism.registration.observation_digest(),
            mechanism.incidence.root(),
            mechanism.incidence.target_is_sealed(),
            mechanism.incidence.frontier_is_complete(),
        ),
    }
}

fn hash_layer_snapshot(
    hasher: &mut AnalysisCatalogHasher,
    layer: &RelationalAnalysisLayerSnapshot,
) {
    match layer {
        RelationalAnalysisLayerSnapshot::Result(result) => {
            let state = match &result.state {
                RelationalResultLayerSnapshotState::Unregistered => {
                    ResultLayerHashState::Unregistered
                }
                RelationalResultLayerSnapshotState::Registered {
                    spec,
                    evidence,
                    projection,
                    certified_source_summary,
                    publication,
                } => ResultLayerHashState::Registered {
                    spec_root: spec.spec_root(),
                    evidence_root: evidence.root(),
                    input_is_sealed: evidence.input_is_sealed(),
                    projection_root: projection.root(),
                    projection_record_count: projection.records().len() as u128,
                    certified_source_summary_id: certified_source_summary
                        .as_ref()
                        .map(RelationalCertifiedSourceSummaryArtifact::artifact_id),
                    publication: *publication,
                },
            };
            hash_result_layer(
                hasher,
                result.view_id,
                result.input,
                result.semantic_spec_digest,
                state,
            );
        }
        RelationalAnalysisLayerSnapshot::Mechanisms(mechanism) => hash_mechanism_layer(
            hasher,
            mechanism.request_id,
            mechanism.target,
            mechanism.observation_id,
            mechanism.observation_digest,
            mechanism.incidence.root(),
            mechanism.incidence.target_is_sealed(),
            mechanism.incidence.frontier_is_complete(),
        ),
    }
}

fn hash_publication(hasher: &mut AnalysisCatalogHasher, publication: RelationalResultPublication) {
    hasher.u32(publication.version());
    hasher.digest(publication.id().bytes());
    hasher.digest(publication.plan_root().bytes());
    hasher.digest(publication.view_id().bytes());
    hasher.digest(publication.spec_root().bytes());
    hasher.digest(publication.evidence_root().bytes());
    hasher.digest(publication.result_root().bytes());
}

fn hash_result_input(hasher: &mut AnalysisCatalogHasher, input: RelationalResolvedResultInput) {
    match input {
        RelationalResolvedResultInput::Sources(relation_id) => {
            hasher.tag(0x03);
            hasher.digest(relation_id.bytes());
        }
        RelationalResolvedResultInput::Selected(question_id) => {
            hasher.tag(0x01);
            hasher.digest(question_id.bytes());
        }
        RelationalResolvedResultInput::MechanismIncidence(request_id) => {
            hasher.tag(0x02);
            hasher.digest(request_id.bytes());
        }
    }
}

fn hash_mechanism_target(
    hasher: &mut AnalysisCatalogHasher,
    target: RelationalResolvedMechanismTarget,
) {
    match target {
        RelationalResolvedMechanismTarget::Selected(question_id) => {
            hasher.tag(0x01);
            hasher.digest(question_id.bytes());
        }
        RelationalResolvedMechanismTarget::ChosenView(view_id) => {
            hasher.tag(0x02);
            hasher.digest(view_id.bytes());
        }
    }
}

struct AnalysisCatalogHasher(Sha256);

impl AnalysisCatalogHasher {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update((domain.len() as u64).to_le_bytes());
        hasher.update(domain);
        Self(hasher)
    }

    fn tag(&mut self, value: u8) {
        self.0.update([value]);
    }

    fn bool(&mut self, value: bool) {
        self.tag(u8::from(value));
    }

    fn u32(&mut self, value: u32) {
        self.0.update(value.to_le_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.0.update(value.to_le_bytes());
    }

    fn digest(&mut self, value: [u8; 32]) {
        self.0.update(value);
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mechanism_result_seals_require_outer_journal_authority() {
        let view_id = ViewId::from_journal_codec_bytes([0x11; 32]);
        let request_id = MechanismRequestId::from_journal_codec_bytes([0x22; 32]);
        let completed_root = MechanismIncidenceRoot::from_journal_codec_bytes([0x33; 32]);
        let structural_root = StructuralQuotientClosureRoot::from_journal_codec_bytes([0x44; 32]);
        let plain = ResultEvidenceUpstreamRoot::MechanismIncidence {
            request_id,
            completed_root,
        };
        let structural = ResultEvidenceUpstreamRoot::StructuralMechanismIncidence {
            request_id,
            completed_root,
            structural_root,
        };
        let expected = Err(
            RelationalAnalysisCatalogError::MechanismResultSealAuthorityUnavailable { view_id },
        );

        assert_eq!(
            reject_generic_mechanism_result_seal(view_id, plain),
            expected
        );
        assert_eq!(
            reject_generic_mechanism_result_seal(view_id, structural),
            expected
        );
        assert!(reject_standalone_structural_result_snapshot(view_id, plain).is_ok());
        assert_eq!(
            reject_standalone_structural_result_snapshot(view_id, structural),
            expected
        );
    }
}
