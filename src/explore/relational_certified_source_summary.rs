//! Bounded proof producer for exact grouped source summaries.
//!
//! V1 recognizes one singleton `Context` plus one independent finite `Before`
//! factor and closes a direct `count_distinct(before)` summary from one checked
//! representative. V2 consumes a compiler-minted ProductRank theorem: a
//! separating factor prefix supplies the group keys and the complementary
//! suffix supplies one separating `count_distinct` value. It evaluates exactly
//! one representative per output group, up to the hard compact-group bound.
//!
//! Representatives supply presentation values only. They are never retained
//! or presented as population evidence; exact membership remains committed by
//! the certified source-population root and the compiler theorem.

use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::{CheckedExploreAnalysisIdentity, CheckedExploreQueryView, Expr, ExprKind};

use super::relation::{RelationId, ViewId};
use super::relational_analysis_plan::{
    RelationalAnalysisLayerId, RelationalAnalysisLayerRegistration, RelationalAnalysisPlan,
    RelationalAnalysisPlanError, RelationalAnalysisPlanRoot, RelationalResolvedResultInput,
    RelationalResultSpecDigest,
};
use super::relational_executor::{
    RelationalExpressionRuntime, RelationalSourceEnumerator, RelationalSourceExecutorError,
};
use super::relational_ir::{
    relational_tys_equivalent, ExploreAggregateReducerIr, ExploreAnalysisNodeIr,
    ExploreResultGrainIr, ExploreResultInputIr, ExploreResultViewIr, ExploreSourceBindingKindIr,
    ExploreSourceBindingRoleIr,
};
use super::relational_result_executor::{
    RelationalResultExecutor, RelationalResultExecutorError, RelationalResultExpressionRuntime,
};
use super::relational_source_image_exactness::{
    CertifiedSourcePopulationBinding, CertifiedSourcePopulationRoot, CertifiedSourcePopulationShape,
};
use super::relational_support_planner::{
    RelationalBindingStageId, RelationalDimensionId, RelationalSupportPlanRoot,
};
use super::result_view::{
    CertifiedResultGroupSummary, CertifiedResultInputRoot, CompactClosedResultView, ResultValue,
    ResultViewInputKind, ResultViewSpec, ResultViewSpecRoot,
};
use super::support_cell::{SupportCellId, SupportMaterializerId};
use super::transition::canonical_explore_value_digest;

const CERTIFIED_SOURCE_SUMMARY_ARTIFACT_V1: &[u8] =
    b"futuruna.explore.certified-source-summary.artifact.v1";
const CERTIFIED_SOURCE_SUMMARY_ARTIFACT_V2: &[u8] =
    b"futuruna.explore.certified-source-summary.artifact.v2";
pub(crate) const RELATIONAL_CERTIFIED_SOURCE_SUMMARY_MAX_GROUPS: u128 = 256;

pub(crate) const RELATIONAL_CERTIFIED_SOURCE_SUMMARY_VERSION_V1: u32 = 1;
pub(crate) const RELATIONAL_CERTIFIED_SOURCE_SUMMARY_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalCertifiedSourceSummaryArtifactId([u8; 32]);

impl RelationalCertifiedSourceSummaryArtifactId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Canonical, population-sized-constant evidence for one recognized result.
///
/// This artifact intentionally contains no SourceKey and no representative
/// row. Group values are presentation witnesses only; the exact logical
/// population is named exclusively by `source_population_root`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalCertifiedSourceSummaryDirectShape {
    context_stage_id: RelationalBindingStageId,
    before_stage_id: RelationalBindingStageId,
    before_dimension_id: RelationalDimensionId,
    before_factor_cell_id: SupportCellId,
}

impl RelationalCertifiedSourceSummaryDirectShape {
    pub(crate) const fn context_stage_id(self) -> RelationalBindingStageId {
        self.context_stage_id
    }

    pub(crate) const fn before_stage_id(self) -> RelationalBindingStageId {
        self.before_stage_id
    }

    pub(crate) const fn before_dimension_id(self) -> RelationalDimensionId {
        self.before_dimension_id
    }

    pub(crate) const fn before_factor_cell_id(self) -> SupportCellId {
        self.before_factor_cell_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalCertifiedSourceSummaryProductShape {
    summary_certificate_id: [u8; 32],
    compiler_projection_certificate_id: [u8; 32],
    factor_binding_root: [u8; 32],
}

impl RelationalCertifiedSourceSummaryProductShape {
    pub(crate) const fn summary_certificate_id(self) -> [u8; 32] {
        self.summary_certificate_id
    }

    pub(crate) const fn compiler_projection_certificate_id(self) -> [u8; 32] {
        self.compiler_projection_certificate_id
    }

    pub(crate) const fn factor_binding_root(self) -> [u8; 32] {
        self.factor_binding_root
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalCertifiedSourceSummaryArtifact {
    version: u32,
    artifact_id: RelationalCertifiedSourceSummaryArtifactId,
    analysis_plan_root: RelationalAnalysisPlanRoot,
    semantic_spec_digest: RelationalResultSpecDigest,
    view_id: ViewId,
    spec_root: ResultViewSpecRoot,
    relation_id: RelationId,
    source_plan_root: RelationalSupportPlanRoot,
    source_certificate_id: [u8; 32],
    source_population_root: CertifiedSourcePopulationRoot,
    source_cell_id: SupportCellId,
    source_materializer_id: SupportMaterializerId,
    direct_shape: Option<RelationalCertifiedSourceSummaryDirectShape>,
    product_shape: Option<RelationalCertifiedSourceSummaryProductShape>,
    exact_cardinality: u128,
    certified_input_root: CertifiedResultInputRoot,
    groups: Box<[CertifiedResultGroupSummary]>,
}

impl RelationalCertifiedSourceSummaryArtifact {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn restore_v1_from_journal_codec(
        version: u32,
        artifact_id: RelationalCertifiedSourceSummaryArtifactId,
        analysis_plan_root: RelationalAnalysisPlanRoot,
        semantic_spec_digest: RelationalResultSpecDigest,
        view_id: ViewId,
        spec_root: ResultViewSpecRoot,
        relation_id: RelationId,
        source_plan_root: RelationalSupportPlanRoot,
        source_certificate_id: [u8; 32],
        source_population_root: CertifiedSourcePopulationRoot,
        source_cell_id: SupportCellId,
        source_materializer_id: SupportMaterializerId,
        context_stage_id: RelationalBindingStageId,
        before_stage_id: RelationalBindingStageId,
        before_dimension_id: RelationalDimensionId,
        before_factor_cell_id: SupportCellId,
        exact_cardinality: u128,
        certified_input_root: CertifiedResultInputRoot,
        group_values: Box<[ResultValue]>,
    ) -> Result<Self, RelationalCertifiedSourceSummaryError> {
        let artifact = Self {
            version,
            artifact_id,
            analysis_plan_root,
            semantic_spec_digest,
            view_id,
            spec_root,
            relation_id,
            source_plan_root,
            source_certificate_id,
            source_population_root,
            source_cell_id,
            source_materializer_id,
            direct_shape: Some(RelationalCertifiedSourceSummaryDirectShape {
                context_stage_id,
                before_stage_id,
                before_dimension_id,
                before_factor_cell_id,
            }),
            product_shape: None,
            exact_cardinality,
            certified_input_root,
            groups: vec![CertifiedResultGroupSummary::new(
                group_values,
                exact_cardinality,
                vec![exact_cardinality].into_boxed_slice(),
            )]
            .into_boxed_slice(),
        };
        if !artifact.validate_identity() {
            return Err(RelationalCertifiedSourceSummaryError::ArtifactIdentityMismatch);
        }
        Ok(artifact)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn restore_v2_from_journal_codec(
        version: u32,
        artifact_id: RelationalCertifiedSourceSummaryArtifactId,
        analysis_plan_root: RelationalAnalysisPlanRoot,
        semantic_spec_digest: RelationalResultSpecDigest,
        view_id: ViewId,
        spec_root: ResultViewSpecRoot,
        relation_id: RelationId,
        source_plan_root: RelationalSupportPlanRoot,
        source_certificate_id: [u8; 32],
        source_population_root: CertifiedSourcePopulationRoot,
        source_cell_id: SupportCellId,
        source_materializer_id: SupportMaterializerId,
        summary_certificate_id: [u8; 32],
        compiler_projection_certificate_id: [u8; 32],
        factor_binding_root: [u8; 32],
        exact_cardinality: u128,
        certified_input_root: CertifiedResultInputRoot,
        groups: Box<[CertifiedResultGroupSummary]>,
    ) -> Result<Self, RelationalCertifiedSourceSummaryError> {
        let artifact = Self {
            version,
            artifact_id,
            analysis_plan_root,
            semantic_spec_digest,
            view_id,
            spec_root,
            relation_id,
            source_plan_root,
            source_certificate_id,
            source_population_root,
            source_cell_id,
            source_materializer_id,
            direct_shape: None,
            product_shape: Some(RelationalCertifiedSourceSummaryProductShape {
                summary_certificate_id,
                compiler_projection_certificate_id,
                factor_binding_root,
            }),
            exact_cardinality,
            certified_input_root,
            groups,
        };
        if !artifact.validate_identity() {
            return Err(RelationalCertifiedSourceSummaryError::ArtifactIdentityMismatch);
        }
        Ok(artifact)
    }

    pub(crate) const fn version(&self) -> u32 {
        self.version
    }

    pub(crate) const fn artifact_id(&self) -> RelationalCertifiedSourceSummaryArtifactId {
        self.artifact_id
    }

    pub(crate) const fn analysis_plan_root(&self) -> RelationalAnalysisPlanRoot {
        self.analysis_plan_root
    }

    pub(crate) const fn semantic_spec_digest(&self) -> RelationalResultSpecDigest {
        self.semantic_spec_digest
    }

    pub(crate) const fn view_id(&self) -> ViewId {
        self.view_id
    }

    pub(crate) const fn spec_root(&self) -> ResultViewSpecRoot {
        self.spec_root
    }

    pub(crate) const fn relation_id(&self) -> RelationId {
        self.relation_id
    }

    pub(crate) const fn source_plan_root(&self) -> RelationalSupportPlanRoot {
        self.source_plan_root
    }

    pub(crate) const fn source_certificate_id(&self) -> [u8; 32] {
        self.source_certificate_id
    }

    pub(crate) const fn source_population_root(&self) -> CertifiedSourcePopulationRoot {
        self.source_population_root
    }

    pub(crate) const fn source_cell_id(&self) -> SupportCellId {
        self.source_cell_id
    }

    pub(crate) const fn source_materializer_id(&self) -> SupportMaterializerId {
        self.source_materializer_id
    }

    pub(crate) const fn direct_shape(&self) -> Option<RelationalCertifiedSourceSummaryDirectShape> {
        self.direct_shape
    }

    pub(crate) const fn product_shape(
        &self,
    ) -> Option<RelationalCertifiedSourceSummaryProductShape> {
        self.product_shape
    }

    pub(crate) const fn exact_cardinality(&self) -> u128 {
        self.exact_cardinality
    }

    pub(crate) const fn certified_input_root(&self) -> CertifiedResultInputRoot {
        self.certified_input_root
    }

    pub(crate) fn groups(&self) -> &[CertifiedResultGroupSummary] {
        &self.groups
    }

    pub(crate) fn validate_identity(&self) -> bool {
        let groups_are_canonical = !self.groups.is_empty()
            && self.groups.iter().all(|group| {
                group.exact_member_count() > 0
                    && !group.group_values().is_empty()
                    && !group.exact_distinct_counts().is_empty()
                    && group
                        .exact_distinct_counts()
                        .iter()
                        .all(|count| *count > 0 && *count <= group.exact_member_count())
            })
            && self
                .groups
                .windows(2)
                .all(|pair| pair[0].group_values() < pair[1].group_values())
            && self.groups.iter().try_fold(0_u128, |total, group| {
                total.checked_add(group.exact_member_count())
            }) == Some(self.exact_cardinality);
        let shape_is_valid = match self.version {
            RELATIONAL_CERTIFIED_SOURCE_SUMMARY_VERSION_V1 => {
                self.direct_shape.is_some()
                    && self.product_shape.is_none()
                    && self.groups.len() == 1
                    && self.groups[0].exact_distinct_counts() == [self.exact_cardinality]
            }
            RELATIONAL_CERTIFIED_SOURCE_SUMMARY_VERSION => {
                self.direct_shape.is_none()
                    && self.product_shape.is_some()
                    && u128::try_from(self.groups.len()).is_ok_and(|group_count| {
                        group_count <= RELATIONAL_CERTIFIED_SOURCE_SUMMARY_MAX_GROUPS
                    })
            }
            _ => false,
        };
        shape_is_valid
            && self.exact_cardinality > 0
            && self.exact_cardinality <= i64::MAX as u128
            && groups_are_canonical
            && self.certified_input_root
                == CertifiedResultInputRoot::from_certified_source_population(
                    self.source_population_root.bytes(),
                    self.exact_cardinality,
                )
            && self.artifact_id == derive_artifact_id(self)
    }
}

/// Rebind a decoded artifact to the exact installed analysis/spec/source
/// authorities without evaluating user code. The live producer still runs
/// once before first admission (and after restart before projection) so the
/// checked ordinal-zero group values are compared byte-for-byte with this
/// retained artifact.
pub(crate) fn reverify_relational_certified_source_summary_artifact(
    artifact: &RelationalCertifiedSourceSummaryArtifact,
    analysis_plan: &RelationalAnalysisPlan,
    spec: &ResultViewSpec,
    source: CertifiedSourcePopulationBinding,
) -> Result<VerifiedRelationalCertifiedSourceSummary, RelationalCertifiedSourceSummaryError> {
    if !artifact.validate_identity() || !analysis_plan.validate_root() {
        return Err(RelationalCertifiedSourceSummaryError::ArtifactIdentityMismatch);
    }
    let registration = analysis_plan
        .registration(RelationalAnalysisLayerId::Result(artifact.view_id))
        .ok_or(RelationalCertifiedSourceSummaryError::ViewMissing(
            artifact.view_id,
        ))?;
    let RelationalAnalysisLayerRegistration::Result(registration) = registration else {
        return Err(RelationalCertifiedSourceSummaryError::ViewIdentityMismatch);
    };
    let source_shape_matches = match (
        artifact.direct_shape,
        artifact.product_shape,
        source.shape(),
    ) {
        (
            Some(direct),
            None,
            CertifiedSourcePopulationShape::DirectBeforeFactor {
                context_stage_id,
                before_stage_id,
                before_dimension_id,
                before_factor_cell_id,
            },
        ) => {
            direct.context_stage_id == context_stage_id
                && direct.before_stage_id == before_stage_id
                && direct.before_dimension_id == before_dimension_id
                && direct.before_factor_cell_id == before_factor_cell_id
        }
        (
            None,
            Some(product),
            CertifiedSourcePopulationShape::SeparatedProjection {
                compiler_certificate_id,
                factor_binding_root,
            },
        ) => {
            product.compiler_projection_certificate_id == compiler_certificate_id
                && product.factor_binding_root == factor_binding_root
        }
        _ => false,
    };
    if artifact.analysis_plan_root != analysis_plan.root()
        || registration.input() != RelationalResolvedResultInput::Sources(artifact.relation_id)
        || registration.semantic_spec_digest() != artifact.semantic_spec_digest
        || spec.view_id() != artifact.view_id
        || spec.spec_root() != artifact.spec_root
        || spec.input_kind() != ResultViewInputKind::Source
        || artifact.groups.iter().any(|group| {
            spec.grain().group_value_count() != group.group_values().len()
                || spec.aggregate_names().len() != group.exact_distinct_counts().len()
        })
        || source.relation_id() != artifact.relation_id
        || source.plan_root() != artifact.source_plan_root
        || source.certificate_id() != artifact.source_certificate_id
        || source.population_root() != artifact.source_population_root
        || source.source_cell_id() != artifact.source_cell_id
        || source.source_materializer_id() != artifact.source_materializer_id
        || !source_shape_matches
        || source.exact_cardinality() != artifact.exact_cardinality
        || artifact.certified_input_root
            != CertifiedResultInputRoot::from_certified_source_population(
                source.population_root().bytes(),
                source.exact_cardinality(),
            )
    {
        return Err(RelationalCertifiedSourceSummaryError::ArtifactScopeMismatch);
    }
    Ok(VerifiedRelationalCertifiedSourceSummary {
        artifact: artifact.clone(),
    })
}

/// In-memory proof authority. A later durable bridge should journal only the
/// artifact and remint this wrapper by comparing it with the checked producer
/// result and the installed certified source binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VerifiedRelationalCertifiedSourceSummary {
    artifact: RelationalCertifiedSourceSummaryArtifact,
}

impl VerifiedRelationalCertifiedSourceSummary {
    pub(crate) const fn artifact(&self) -> &RelationalCertifiedSourceSummaryArtifact {
        &self.artifact
    }

    pub(crate) fn into_artifact(self) -> RelationalCertifiedSourceSummaryArtifact {
        self.artifact
    }

    /// Evaluate only the closed public SELECT expression over the exact groups.
    /// The reducer root commits the certified source population and exact N in
    /// its own domain; it never hashes a representative as an input row.
    pub(crate) fn close<R: RelationalResultExpressionRuntime>(
        &self,
        executor: &RelationalResultExecutor<'_>,
        runtime: &mut R,
    ) -> Result<CompactClosedResultView, RelationalCertifiedSourceSummaryError> {
        if executor.spec().view_id() != self.artifact.view_id
            || executor.spec().spec_root() != self.artifact.spec_root
        {
            return Err(RelationalCertifiedSourceSummaryError::ResultSpecMismatch);
        }
        executor
            .close_certified_source_groups(
                self.artifact.certified_input_root,
                self.artifact.exact_cardinality,
                &self.artifact.groups,
                runtime,
            )
            .map_err(RelationalCertifiedSourceSummaryError::ResultExecutor)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCertifiedSourceSummaryUnsupported {
    SourceShape,
    ResultInput,
    ResultShape(&'static str),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCertifiedSourceSummaryCertification {
    Certified(VerifiedRelationalCertifiedSourceSummary),
    Unsupported(RelationalCertifiedSourceSummaryUnsupported),
}

/// Recognize and prove one certified source summary. The direct theorem
/// evaluates ordinal zero; the ProductRank theorem evaluates one canonical
/// representative per exact output group. Unsupported views fail closed
/// without changing any journal or catalog state.
pub(crate) fn certify_relational_source_summary<R>(
    checked: &CheckedExploreQueryView<'_>,
    view_id: ViewId,
    source: CertifiedSourcePopulationBinding,
    runtime: &mut R,
) -> Result<RelationalCertifiedSourceSummaryCertification, RelationalCertifiedSourceSummaryError>
where
    R: RelationalExpressionRuntime + RelationalResultExpressionRuntime,
{
    if source.relation_id() != checked.relation_id() || source.exact_cardinality() == 0 {
        return Err(RelationalCertifiedSourceSummaryError::SourcePopulationScopeMismatch);
    }
    if source.exact_cardinality() > i64::MAX as u128 {
        return Err(
            RelationalCertifiedSourceSummaryError::AggregateCountOverflow(
                source.exact_cardinality(),
            ),
        );
    }

    let Some(view) = checked
        .analysis_nodes()
        .find_map(|(node, identity)| match (node, identity) {
            (
                ExploreAnalysisNodeIr::Result(view),
                CheckedExploreAnalysisIdentity::View { view_id: candidate },
            ) if *candidate == view_id => Some(view),
            _ => None,
        })
    else {
        return Err(RelationalCertifiedSourceSummaryError::ViewMissing(view_id));
    };

    if !matches!(&view.input, ExploreResultInputIr::Sources) {
        return Ok(RelationalCertifiedSourceSummaryCertification::Unsupported(
            RelationalCertifiedSourceSummaryUnsupported::ResultInput,
        ));
    }
    let analysis_plan = RelationalAnalysisPlan::from_checked(checked)?;
    let registration = analysis_plan
        .registration(RelationalAnalysisLayerId::Result(view_id))
        .ok_or(RelationalCertifiedSourceSummaryError::ViewMissing(view_id))?;
    let RelationalAnalysisLayerRegistration::Result(registration) = registration else {
        return Err(RelationalCertifiedSourceSummaryError::ViewIdentityMismatch);
    };
    if registration.input() != RelationalResolvedResultInput::Sources(checked.relation_id()) {
        return Err(RelationalCertifiedSourceSummaryError::ViewIdentityMismatch);
    }

    let executor = RelationalResultExecutor::lower(view_id, view)?;
    let sources =
        RelationalSourceEnumerator::new(checked.relation_id(), &checked.closed_query.source)?;
    let certified_input_root = CertifiedResultInputRoot::from_certified_source_population(
        source.population_root().bytes(),
        source.exact_cardinality(),
    );

    let (version, direct_shape, product_shape, groups) = match source.shape() {
        CertifiedSourcePopulationShape::DirectBeforeFactor {
            context_stage_id,
            before_stage_id,
            before_dimension_id,
            before_factor_cell_id,
        } => {
            if !recognized_source_shape(checked) {
                return Ok(RelationalCertifiedSourceSummaryCertification::Unsupported(
                    RelationalCertifiedSourceSummaryUnsupported::SourceShape,
                ));
            }
            if let Some(reason) = unsupported_result_shape(view, checked) {
                return Ok(RelationalCertifiedSourceSummaryCertification::Unsupported(
                    RelationalCertifiedSourceSummaryUnsupported::ResultShape(reason),
                ));
            }
            let representative =
                sources.completed_source_at_independent_finite_ordinals(&[0], runtime)?;
            let evaluated = executor.evaluate_concrete_source(
                representative.source_key(),
                representative.row(),
                runtime,
            )?;
            let contribution = evaluated.contribution();
            let [distinct_before] = contribution.distinct_arguments() else {
                return Err(RelationalCertifiedSourceSummaryError::RepresentativeShapeMismatch);
            };
            if distinct_before != &ResultValue::Value(representative.row().before().clone()) {
                return Err(RelationalCertifiedSourceSummaryError::DirectBeforeWitnessMismatch);
            }
            (
                RELATIONAL_CERTIFIED_SOURCE_SUMMARY_VERSION_V1,
                Some(RelationalCertifiedSourceSummaryDirectShape {
                    context_stage_id,
                    before_stage_id,
                    before_dimension_id,
                    before_factor_cell_id,
                }),
                None,
                vec![CertifiedResultGroupSummary::new(
                    contribution.group_values().to_vec().into_boxed_slice(),
                    source.exact_cardinality(),
                    vec![source.exact_cardinality()].into_boxed_slice(),
                )]
                .into_boxed_slice(),
            )
        }
        CertifiedSourcePopulationShape::SeparatedProjection {
            compiler_certificate_id,
            factor_binding_root,
        } => {
            let Some(summary_certificate) =
                checked.product_rank_grouped_distinct_certificate(view_id)
            else {
                return Ok(RelationalCertifiedSourceSummaryCertification::Unsupported(
                    RelationalCertifiedSourceSummaryUnsupported::ResultShape(
                        "source result has no checked ProductRank grouped-distinct theorem",
                    ),
                ));
            };
            let Some(source_projection) = checked.source_image_projection() else {
                return Err(RelationalCertifiedSourceSummaryError::ProductSummaryScopeMismatch);
            };
            if !summary_certificate.validate_identity()
                || !summary_certificate.validates_source_projection(source_projection)
                || summary_certificate.relation_id() != checked.relation_id()
                || summary_certificate.view_id() != view_id
                || summary_certificate.source_projection_certificate_id() != compiler_certificate_id
                || source_projection.certificate_id != compiler_certificate_id
                || summary_certificate
                    .exact_group_count()
                    .checked_mul(summary_certificate.exact_members_per_group())
                    != Some(source.exact_cardinality())
            {
                return Err(RelationalCertifiedSourceSummaryError::ProductSummaryScopeMismatch);
            }
            if summary_certificate.exact_group_count()
                > RELATIONAL_CERTIFIED_SOURCE_SUMMARY_MAX_GROUPS
            {
                return Ok(RelationalCertifiedSourceSummaryCertification::Unsupported(
                    RelationalCertifiedSourceSummaryUnsupported::ResultShape(
                        "ProductRank summary exceeds the bounded compact-group limit",
                    ),
                ));
            }
            let factor_count = usize::try_from(summary_certificate.factor_count())
                .map_err(|_| RelationalCertifiedSourceSummaryError::ProductSummaryScopeMismatch)?;
            let group_factor_count = usize::try_from(summary_certificate.group_factor_count())
                .map_err(|_| RelationalCertifiedSourceSummaryError::ProductSummaryScopeMismatch)?;
            if source_projection.factors.len() != factor_count || group_factor_count >= factor_count
            {
                return Err(RelationalCertifiedSourceSummaryError::ProductSummaryScopeMismatch);
            }

            let group_capacity = usize::try_from(summary_certificate.exact_group_count())
                .map_err(|_| RelationalCertifiedSourceSummaryError::ProductSummaryScopeMismatch)?;
            let mut summaries = Vec::with_capacity(group_capacity);
            for group_rank in 0..summary_certificate.exact_group_count() {
                let mut remaining = group_rank;
                let mut ordinals = vec![0_u128; factor_count];
                for factor_index in (0..group_factor_count).rev() {
                    let cardinality = source_projection.factors[factor_index].exact_cardinality;
                    ordinals[factor_index] = remaining % cardinality;
                    remaining /= cardinality;
                }
                if remaining != 0 {
                    return Err(RelationalCertifiedSourceSummaryError::ProductSummaryScopeMismatch);
                }
                let representative =
                    sources.completed_source_at_independent_finite_ordinals(&ordinals, runtime)?;
                let evaluated = executor.evaluate_concrete_source(
                    representative.source_key(),
                    representative.row(),
                    runtime,
                )?;
                let contribution = evaluated.contribution();
                if contribution.distinct_arguments().len() != 1
                    || !contribution.measures().is_empty()
                {
                    return Err(RelationalCertifiedSourceSummaryError::RepresentativeShapeMismatch);
                }
                summaries.push(CertifiedResultGroupSummary::new(
                    contribution.group_values().to_vec().into_boxed_slice(),
                    summary_certificate.exact_members_per_group(),
                    vec![summary_certificate.exact_members_per_group()].into_boxed_slice(),
                ));
            }
            summaries.sort_by(|left, right| left.group_values().cmp(right.group_values()));
            if summaries
                .windows(2)
                .any(|pair| pair[0].group_values() >= pair[1].group_values())
            {
                return Err(RelationalCertifiedSourceSummaryError::RepresentativeShapeMismatch);
            }
            (
                RELATIONAL_CERTIFIED_SOURCE_SUMMARY_VERSION,
                None,
                Some(RelationalCertifiedSourceSummaryProductShape {
                    summary_certificate_id: summary_certificate.certificate_id(),
                    compiler_projection_certificate_id: compiler_certificate_id,
                    factor_binding_root,
                }),
                summaries.into_boxed_slice(),
            )
        }
    };

    let mut artifact = RelationalCertifiedSourceSummaryArtifact {
        version,
        artifact_id: RelationalCertifiedSourceSummaryArtifactId([0; 32]),
        analysis_plan_root: analysis_plan.root(),
        semantic_spec_digest: registration.semantic_spec_digest(),
        view_id,
        spec_root: executor.spec().spec_root(),
        relation_id: checked.relation_id(),
        source_plan_root: source.plan_root(),
        source_certificate_id: source.certificate_id(),
        source_population_root: source.population_root(),
        source_cell_id: source.source_cell_id(),
        source_materializer_id: source.source_materializer_id(),
        direct_shape,
        product_shape,
        exact_cardinality: source.exact_cardinality(),
        certified_input_root,
        groups,
    };
    artifact.artifact_id = derive_artifact_id(&artifact);
    if !artifact.validate_identity() {
        return Err(RelationalCertifiedSourceSummaryError::ArtifactIdentityMismatch);
    }
    Ok(RelationalCertifiedSourceSummaryCertification::Certified(
        VerifiedRelationalCertifiedSourceSummary { artifact },
    ))
}

fn recognized_source_shape(checked: &CheckedExploreQueryView<'_>) -> bool {
    let relation = &checked.closed_query.source;
    let Some(context) = relation.bindings.get(relation.context_binding_index) else {
        return false;
    };
    let Some(before) = relation.bindings.get(relation.before_binding_index) else {
        return false;
    };
    relation.bindings.len() == 2
        && relation.context_binding_index == 0
        && relation.before_binding_index == 1
        && context.binding_index == 0
        && before.binding_index == 1
        && context.name == "context"
        && before.name == "before"
        && context.role == ExploreSourceBindingRoleIr::Context
        && before.role == ExploreSourceBindingRoleIr::Before
        && matches!(&context.kind, ExploreSourceBindingKindIr::Singleton { .. })
        && matches!(&before.kind, ExploreSourceBindingKindIr::Finite { .. })
        && context.dependencies.is_empty()
        && before.dependencies.is_empty()
}

fn unsupported_result_shape(
    view: &ExploreResultViewIr,
    checked: &CheckedExploreQueryView<'_>,
) -> Option<&'static str> {
    let ExploreResultGrainIr::GroupBy { fields, .. } = &view.grain else {
        return Some("certified source summary requires nonempty group by");
    };
    if fields.is_empty() {
        return Some("certified source summary requires nonempty group by");
    }
    if !view.measures.is_empty() {
        return Some("certified source summary does not accept measures");
    }
    if view.having.is_some() {
        return Some("certified source summary does not accept having");
    }
    if view.choose.is_some() {
        return Some("certified source summary does not accept choice");
    }
    if fields.iter().any(|field| {
        field.name == "context"
            || field.name == "before"
            || !is_direct_field_path_from(&field.value, "context")
    }) {
        return Some("every certified group key must be a direct Context projection");
    }

    let [aggregate] = view.aggregates.as_ref() else {
        return Some("certified source summary requires one count_distinct(before)");
    };
    if aggregate.name == "context" || aggregate.name == "before" {
        return Some("certified aggregate alias shadows a source role");
    }
    let ExploreAggregateReducerIr::CountDistinct { value, value_ty } = &aggregate.reducer;
    if !matches!(&value.kind, ExprKind::Var(name) if name == "before")
        || !relational_tys_equivalent(value_ty, &checked.closed_query.source.before_ty)
    {
        return Some("certified distinct reducer must be the direct Before binding");
    }

    // Keep the first producer intentionally narrow. The existing grouped
    // projector supports richer expressions, but direct aliases make it
    // impossible for SELECT to reach row-local Context/Before bindings that
    // do not exist at certified group closure.
    let mut closed_names = fields
        .iter()
        .map(|field| field.name.as_str())
        .chain(std::iter::once(aggregate.name.as_str()))
        .collect::<Vec<_>>();
    for field in view.select.iter() {
        let ExprKind::Var(name) = &field.value.kind else {
            return Some("certified SELECT fields must be direct closed-group aliases");
        };
        if !closed_names
            .iter()
            .any(|candidate| *candidate == name.as_str())
        {
            return Some("certified SELECT reaches outside the closed group environment");
        }
        closed_names.push(field.name.as_str());
    }
    None
}

fn is_direct_field_path_from(expression: &Expr, root: &str) -> bool {
    match &expression.kind {
        ExprKind::Var(name) => name == root,
        ExprKind::Field(base, field) => !field.is_empty() && is_direct_field_path_from(base, root),
        _ => false,
    }
}

fn derive_artifact_id(
    artifact: &RelationalCertifiedSourceSummaryArtifact,
) -> RelationalCertifiedSourceSummaryArtifactId {
    let mut hasher = CanonicalHasher::new(
        if artifact.version == RELATIONAL_CERTIFIED_SOURCE_SUMMARY_VERSION_V1 {
            CERTIFIED_SOURCE_SUMMARY_ARTIFACT_V1
        } else {
            CERTIFIED_SOURCE_SUMMARY_ARTIFACT_V2
        },
    );
    hasher.u32(artifact.version);
    hasher.digest(artifact.analysis_plan_root.bytes());
    hasher.digest(artifact.semantic_spec_digest.bytes());
    hasher.digest(artifact.view_id.bytes());
    hasher.digest(artifact.spec_root.bytes());
    hasher.digest(artifact.relation_id.bytes());
    hasher.digest(artifact.source_plan_root.bytes());
    hasher.digest(artifact.source_certificate_id);
    hasher.digest(artifact.source_population_root.bytes());
    hasher.digest(artifact.source_cell_id.bytes());
    hasher.digest(artifact.source_materializer_id.bytes());
    match (artifact.direct_shape, artifact.product_shape) {
        (Some(direct), None) => {
            hasher.digest(direct.context_stage_id.bytes());
            hasher.digest(direct.before_stage_id.bytes());
            hasher.digest(direct.before_dimension_id.bytes());
            hasher.digest(direct.before_factor_cell_id.bytes());
        }
        (None, Some(product)) => {
            hasher.digest(product.summary_certificate_id);
            hasher.digest(product.compiler_projection_certificate_id);
            hasher.digest(product.factor_binding_root);
        }
        _ => return RelationalCertifiedSourceSummaryArtifactId([0; 32]),
    }
    hasher.u128(artifact.exact_cardinality);
    hasher.digest(artifact.certified_input_root.bytes());
    if artifact.version == RELATIONAL_CERTIFIED_SOURCE_SUMMARY_VERSION_V1 {
        let Some(group) = artifact.groups.first() else {
            return RelationalCertifiedSourceSummaryArtifactId([0; 32]);
        };
        hasher.u128(group.group_values().len() as u128);
        for value in group.group_values() {
            hash_result_value(&mut hasher, value);
        }
    } else {
        hasher.u128(artifact.groups.len() as u128);
        for group in artifact.groups.iter() {
            hasher.u128(group.group_values().len() as u128);
            for value in group.group_values() {
                hash_result_value(&mut hasher, value);
            }
            hasher.u128(group.exact_member_count());
            hasher.u128(group.exact_distinct_counts().len() as u128);
            for count in group.exact_distinct_counts() {
                hasher.u128(*count);
            }
        }
    }
    RelationalCertifiedSourceSummaryArtifactId(hasher.finish())
}

fn hash_result_value(hasher: &mut CanonicalHasher, value: &ResultValue) {
    match value {
        ResultValue::Value(value) => {
            hasher.tag(0x01);
            hasher.digest(canonical_explore_value_digest(value));
        }
        ResultValue::CaseId(case_id) => {
            hasher.tag(0x02);
            hasher.digest(case_id.bytes());
        }
        ResultValue::TransitionId(transition_id) => {
            hasher.tag(0x03);
            hasher.digest(transition_id.bytes());
        }
        ResultValue::SignatureId(signature_id) => {
            hasher.tag(0x04);
            hasher.digest(signature_id.request_id().bytes());
            hasher.digest(signature_id.bytes());
        }
        ResultValue::StructuralMechanismId(mechanism_id) => {
            hasher.tag(0x05);
            hasher.digest(mechanism_id.bytes());
        }
        ResultValue::ExecutionProfileId(profile_id) => {
            hasher.tag(0x06);
            hasher.digest(profile_id.bytes());
        }
    }
}

struct CanonicalHasher(Sha256);

impl CanonicalHasher {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = Self(Sha256::new());
        hasher.bytes(domain);
        hasher
    }

    fn tag(&mut self, value: u8) {
        self.0.update([value]);
    }

    fn u32(&mut self, value: u32) {
        self.0.update(value.to_be_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.0.update(value.to_be_bytes());
    }

    fn digest(&mut self, value: [u8; 32]) {
        self.0.update(value);
    }

    fn bytes(&mut self, value: &[u8]) {
        self.u128(value.len() as u128);
        self.0.update(value);
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCertifiedSourceSummaryError {
    AnalysisPlan(RelationalAnalysisPlanError),
    SourceExecutor(RelationalSourceExecutorError),
    ResultExecutor(RelationalResultExecutorError),
    ViewMissing(ViewId),
    ViewIdentityMismatch,
    SourcePopulationScopeMismatch,
    AggregateCountOverflow(u128),
    RepresentativeShapeMismatch,
    DirectBeforeWitnessMismatch,
    ProductSummaryScopeMismatch,
    ResultSpecMismatch,
    ArtifactIdentityMismatch,
    ArtifactScopeMismatch,
}

impl From<RelationalAnalysisPlanError> for RelationalCertifiedSourceSummaryError {
    fn from(error: RelationalAnalysisPlanError) -> Self {
        Self::AnalysisPlan(error)
    }
}

impl From<RelationalSourceExecutorError> for RelationalCertifiedSourceSummaryError {
    fn from(error: RelationalSourceExecutorError) -> Self {
        Self::SourceExecutor(error)
    }
}

impl From<RelationalResultExecutorError> for RelationalCertifiedSourceSummaryError {
    fn from(error: RelationalResultExecutorError) -> Self {
        Self::ResultExecutor(error)
    }
}

impl fmt::Display for RelationalCertifiedSourceSummaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AnalysisPlan(error) => write!(formatter, "source summary plan failed: {error}"),
            Self::SourceExecutor(error) => {
                write!(formatter, "source summary representative failed: {error}")
            }
            Self::ResultExecutor(error) => {
                write!(
                    formatter,
                    "source summary result evaluation failed: {error}"
                )
            }
            Self::ViewMissing(_) => formatter.write_str("source summary result view is absent"),
            Self::ViewIdentityMismatch => {
                formatter.write_str("source summary view identity and checked IR disagree")
            }
            Self::SourcePopulationScopeMismatch => formatter
                .write_str("source summary population does not bind this checked source relation"),
            Self::AggregateCountOverflow(count) => write!(
                formatter,
                "source summary exact count {count} cannot inhabit the result Int aggregate"
            ),
            Self::RepresentativeShapeMismatch => formatter
                .write_str("source summary checked representative has the wrong reducer shape"),
            Self::DirectBeforeWitnessMismatch => formatter.write_str(
                "source summary direct-Before reducer disagrees with its checked witness",
            ),
            Self::ProductSummaryScopeMismatch => formatter.write_str(
                "ProductRank source summary does not match its checked view and source proofs",
            ),
            Self::ResultSpecMismatch => formatter
                .write_str("source summary closure received another result-view specification"),
            Self::ArtifactIdentityMismatch => formatter
                .write_str("source summary artifact identity does not match its proof payload"),
            Self::ArtifactScopeMismatch => formatter.write_str(
                "source summary artifact does not match the installed plan, result spec, or source proof",
            ),
        }
    }
}

impl Error for RelationalCertifiedSourceSummaryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::AnalysisPlan(error) => Some(error),
            Self::SourceExecutor(error) => Some(error),
            Self::ResultExecutor(error) => Some(error),
            _ => None,
        }
    }
}
