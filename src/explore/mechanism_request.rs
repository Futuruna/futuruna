//! Checked construction of a private mechanism observation request.
//!
//! This is an invocation-side selection seam, not source syntax. Callers name
//! producer-minted `output.show` positions; checked expression sites, callable
//! identity, query/domain identity, and disclosure are derived here before a
//! durable stream is opened. Display names never select a semantic root.

use std::error::Error;
use std::fmt;

use super::mechanism::{
    CheckedMechanismObservationRequestV1, MechanismBinField, MechanismDisclosureV1,
    MechanismEndpointPairingV1, MechanismIncidenceDisclosure, MechanismNormalization,
    MechanismNumericBin, MechanismObservationRequest, MechanismObservationTarget, MechanismQueryId,
    MechanismSamplingPlan, MechanismSemanticRootId,
};
use super::mechanism_stream::{
    validate_mechanism_stream_request_v1, MAX_AXES, MAX_BINS_PER_FIELD, MAX_BIN_FIELDS,
    MAX_RETAINED_EXAMPLES_PER_SIGNATURE, MAX_TOTAL_BINS,
};
use super::source_events::{preflight_checked_query_access, ResolvedEventAdapterLimits};
use crate::{CheckedCallTarget, CheckedExpressionType, ExprSiteId, Ty, TypeCheckArtifacts};

/// One numeric `output.show` expression whose replayed values are assigned to
/// the declared bins. `show_index`, rather than the field's presentation name,
/// selects the producer-minted checked expression site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MechanismBinShowSelectionV1 {
    pub(crate) show_index: usize,
    pub(crate) bins: Box<[MechanismNumericBin]>,
}

/// Private invocation configuration for the first checked mechanism slice.
///
/// The two endpoint roots must be distinct direct calls to the same exact
/// checked function or global rule family. Target population, normalization,
/// sampling, axes, query identity, and full matching-case disclosure are
/// deliberately not caller configurable in this version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MechanismTraceSelectionV1 {
    pub(crate) before_show_index: usize,
    pub(crate) after_show_index: usize,
    pub(crate) bin_fields: Box<[MechanismBinShowSelectionV1]>,
    pub(crate) retained_examples_per_signature: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MechanismRequestConfigurationError(Box<str>);

impl fmt::Display for MechanismRequestConfigurationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for MechanismRequestConfigurationError {}

fn invalid(message: impl Into<Box<str>>) -> MechanismRequestConfigurationError {
    MechanismRequestConfigurationError(message.into())
}

/// Turn a positional private selection into the complete checked request bound
/// at sequence zero. This function performs no I/O and must run before opening
/// or creating the durable run store.
pub(crate) fn build_checked_mechanism_request_v1(
    artifacts: &TypeCheckArtifacts,
    accepted_query_index: usize,
    selection: MechanismTraceSelectionV1,
) -> Result<CheckedMechanismObservationRequestV1, MechanismRequestConfigurationError> {
    if selection.bin_fields.len() > MAX_BIN_FIELDS {
        return Err(invalid(format!(
            "mechanism request has {} bin fields; limit is {MAX_BIN_FIELDS}",
            selection.bin_fields.len()
        )));
    }
    if selection.retained_examples_per_signature as usize > MAX_RETAINED_EXAMPLES_PER_SIGNATURE {
        return Err(invalid(format!(
            "mechanism retained examples per signature {} exceeds limit {MAX_RETAINED_EXAMPLES_PER_SIGNATURE}",
            selection.retained_examples_per_signature
        )));
    }
    let mut total_bins = 0_usize;
    for field in selection.bin_fields.iter() {
        if field.bins.len() > MAX_BINS_PER_FIELD {
            return Err(invalid(format!(
                "mechanism bin field has {} bins; limit is {MAX_BINS_PER_FIELD}",
                field.bins.len()
            )));
        }
        total_bins = total_bins
            .checked_add(field.bins.len())
            .ok_or_else(|| invalid("mechanism request total bin count exceeds usize::MAX"))?;
        if total_bins > MAX_TOTAL_BINS {
            return Err(invalid(format!(
                "mechanism request has {total_bins} total bins; limit is {MAX_TOTAL_BINS}"
            )));
        }
    }
    if !artifacts.diagnostics.is_empty() {
        return Err(invalid(format!(
            "cannot configure mechanism tracing from a program rejected by the ordinary checker ({} diagnostic{})",
            artifacts.diagnostics.len(),
            if artifacts.diagnostics.len() == 1 { "" } else { "s" }
        )));
    }
    preflight_checked_query_access(
        artifacts,
        accepted_query_index,
        ResolvedEventAdapterLimits::default(),
    )
    .map_err(|error| {
        invalid(format!(
            "cannot preflight checked Explore query for mechanism tracing: {error}"
        ))
    })?;
    if artifacts
        .exploration_universes
        .get(accepted_query_index)
        .is_some_and(|query| query.universe.dimensions.len() > MAX_AXES)
    {
        return Err(invalid(format!(
            "mechanism universe has more than {MAX_AXES} axes"
        )));
    }
    let checked = artifacts
        .checked_exploration_query(accepted_query_index)
        .map_err(|error| {
            invalid(format!(
                "cannot select checked Explore query for mechanism tracing: {error:?}"
            ))
        })?;
    if checked.closed_query.boundary_hint().is_none() {
        return Err(invalid(
            "differential mechanism tracing requires a checked boundary query",
        ));
    }

    let shown = &checked.closed_query.query.output.show;
    let shown_sites = &checked.artifact.sites.show;
    if shown.len() != shown_sites.len() {
        return Err(invalid(
            "checked Explore show fields disagree with their producer-minted sites",
        ));
    }
    if selection.before_show_index == selection.after_show_index {
        return Err(invalid(
            "before and after mechanism roots must select distinct show fields",
        ));
    }
    let before_site = show_site(shown_sites, selection.before_show_index, "before")?;
    let after_site = show_site(shown_sites, selection.after_show_index, "after")?;
    if before_site == after_site {
        return Err(invalid(
            "before and after mechanism roots must have distinct checked expression sites",
        ));
    }

    require_issue_free_sites(
        artifacts,
        [before_site, after_site].into_iter().chain(
            selection
                .bin_fields
                .iter()
                .filter_map(|field| shown_sites.get(field.show_index)),
        ),
    )?;
    let before_type = direct_traceable_call_type(artifacts, before_site, "before")?;
    let after_type = direct_traceable_call_type(artifacts, after_site, "after")?;
    if !same_ty(before_type.1, &shown[selection.before_show_index].ty)
        || !same_ty(after_type.1, &shown[selection.after_show_index].ty)
    {
        return Err(invalid(
            "mechanism endpoint resolution disagrees with the checked show-field type",
        ));
    }
    if before_type.0 != after_type.0 {
        return Err(invalid(
            "before and after mechanism roots resolve to different checked call targets",
        ));
    }
    if !same_ty(before_type.1, after_type.1) {
        return Err(invalid(
            "before and after mechanism roots have different checked result types",
        ));
    }

    let endpoint_pairing = MechanismEndpointPairingV1::from_checked_calls(
        &artifacts.checked_resolutions,
        before_site.clone(),
        after_site.clone(),
    )
    .map_err(|error| invalid(format!("cannot check mechanism endpoint pairing: {error}")))?;
    let before_root = MechanismSemanticRootId::from_checked_expression(
        &artifacts.checked_resolutions,
        before_site,
    )
    .map_err(|error| invalid(format!("cannot check before mechanism root: {error}")))?;
    let after_root = MechanismSemanticRootId::from_checked_expression(
        &artifacts.checked_resolutions,
        after_site,
    )
    .map_err(|error| invalid(format!("cannot check after mechanism root: {error}")))?;

    let mut selected_bin_fields = selection.bin_fields.into_vec();
    selected_bin_fields.sort_by_key(|field| field.show_index);
    for pair in selected_bin_fields.windows(2) {
        if pair[0].show_index == pair[1].show_index {
            return Err(invalid(format!(
                "mechanism bin field selects show index {} more than once",
                pair[0].show_index
            )));
        }
    }
    let bin_fields = selected_bin_fields
        .into_iter()
        .map(|selected| {
            let field = shown.get(selected.show_index).ok_or_else(|| {
                invalid(format!(
                    "mechanism bin field show index {} is outside {} checked show fields",
                    selected.show_index,
                    shown.len()
                ))
            })?;
            if !matches!(&field.ty, Ty::Name(name) if name == "Int") {
                return Err(invalid(format!(
                    "mechanism bin field at show index {} must have checked type Int",
                    selected.show_index
                )));
            }
            let site = show_site(shown_sites, selected.show_index, "bin")?;
            let resolution = artifacts
                .checked_resolutions
                .expressions
                .get(site)
                .ok_or_else(|| {
                    invalid(format!(
                        "mechanism bin field at show index {} has no checked expression resolution",
                        selected.show_index
                    ))
                })?;
            if !matches!(
                &resolution.resolved_type,
                CheckedExpressionType::Resolved(ty) if same_ty(ty, &field.ty)
            ) {
                return Err(invalid(format!(
                    "mechanism bin field at show index {} disagrees with its checked Int type",
                    selected.show_index
                )));
            }
            let semantic_root = MechanismSemanticRootId::from_checked_expression(
                &artifacts.checked_resolutions,
                site,
            )
            .map_err(|error| {
                invalid(format!(
                    "cannot check mechanism bin root at show index {}: {error}",
                    selected.show_index
                ))
            })?;
            MechanismBinField::new(field.name.clone(), semantic_root, selected.bins)
                .map_err(|error| invalid(format!("cannot check mechanism bin field: {error}")))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let axis_cardinalities = checked
        .closed_query
        .universe
        .dimensions
        .iter()
        .map(|dimension| {
            dimension.domain.cardinality().exact().ok_or_else(|| {
                invalid(format!(
                    "Explore dimension `{}` cardinality exceeds u128::MAX",
                    dimension.name
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let query = MechanismQueryId::from_checked_query(&checked).map_err(|error| {
        invalid(format!(
            "cannot derive checked mechanism query identity: {error}"
        ))
    })?;
    let observation = MechanismObservationRequest::new(
        checked.artifact.identity.analysis_program.clone(),
        query,
        MechanismObservationTarget::MatchingConfigurations,
        before_root,
        after_root,
        endpoint_pairing,
        MechanismNormalization::DynamicControlV1,
        axis_cardinalities,
        MechanismSamplingPlan::empty(),
        bin_fields,
    )
    .map_err(|error| {
        invalid(format!(
            "cannot construct mechanism observation request: {error}"
        ))
    })?;
    let checked_request = CheckedMechanismObservationRequestV1::new(
        observation,
        MechanismDisclosureV1::new(
            MechanismIncidenceDisclosure::FullMatchingIncidence,
            selection.retained_examples_per_signature,
        ),
    )
    .map_err(|error| {
        invalid(format!(
            "cannot check mechanism observation request: {error}"
        ))
    })?;
    validate_mechanism_stream_request_v1(&checked_request).map_err(|error| {
        invalid(format!(
            "mechanism observation request exceeds the durable stream contract: {error}"
        ))
    })?;
    Ok(checked_request)
}

fn same_ty(left: &Ty, right: &Ty) -> bool {
    match (left, right) {
        (Ty::Name(left), Ty::Name(right)) | (Ty::Var(left), Ty::Var(right)) => left == right,
        (Ty::App(left_head, left_args), Ty::App(right_head, right_args)) => {
            same_ty(left_head, right_head)
                && left_args.len() == right_args.len()
                && left_args
                    .iter()
                    .zip(right_args)
                    .all(|(left, right)| same_ty(left, right))
        }
        (Ty::Arrow(left_input, left_output), Ty::Arrow(right_input, right_output)) => {
            same_ty(left_input, right_input) && same_ty(left_output, right_output)
        }
        (Ty::Ref(left), Ty::Ref(right))
        | (Ty::MutRef(left), Ty::MutRef(right))
        | (Ty::Shared(left), Ty::Shared(right))
        | (Ty::Optional(left), Ty::Optional(right)) => same_ty(left, right),
        (Ty::Unit, Ty::Unit) | (Ty::Hole, Ty::Hole) => true,
        _ => false,
    }
}

fn show_site<'a>(
    sites: &'a [ExprSiteId],
    index: usize,
    role: &str,
) -> Result<&'a ExprSiteId, MechanismRequestConfigurationError> {
    sites.get(index).ok_or_else(|| {
        invalid(format!(
            "{role} mechanism show index {index} is outside {} checked show fields",
            sites.len()
        ))
    })
}

fn require_issue_free_sites<'a>(
    artifacts: &TypeCheckArtifacts,
    sites: impl IntoIterator<Item = &'a ExprSiteId>,
) -> Result<(), MechanismRequestConfigurationError> {
    let issues = artifacts
        .checked_resolutions
        .issues_for_reachable_sites(sites);
    if issues.is_empty() {
        Ok(())
    } else {
        Err(invalid(format!(
            "mechanism observation roots have unresolved checked-source issues: {issues:?}"
        )))
    }
}

fn direct_traceable_call_type<'a>(
    artifacts: &'a TypeCheckArtifacts,
    site: &ExprSiteId,
    role: &str,
) -> Result<(&'a CheckedCallTarget, &'a Ty), MechanismRequestConfigurationError> {
    let resolution = artifacts
        .checked_resolutions
        .expressions
        .get(site)
        .ok_or_else(|| invalid(format!("{role} mechanism root has no checked resolution")))?;
    let target = resolution
        .call_target
        .as_ref()
        .ok_or_else(|| invalid(format!("{role} mechanism root is not a direct call")))?;
    if !matches!(
        target,
        CheckedCallTarget::Function { .. } | CheckedCallTarget::RuleFamily(_)
    ) {
        return Err(invalid(format!(
            "{role} mechanism root must resolve to a checked function or rule family"
        )));
    }
    let CheckedExpressionType::Resolved(ty) = &resolution.resolved_type else {
        return Err(invalid(format!(
            "{role} mechanism root has no checked result type"
        )));
    };
    Ok((target, ty))
}
