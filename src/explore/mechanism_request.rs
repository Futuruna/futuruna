//! Checked construction of a query-owned mechanism observation request.
//!
//! The Explore declaration owns one checked endpoint template. Result fields
//! and their positions never select mechanism semantics.

use std::error::Error;
use std::fmt;

use super::mechanism::{
    CheckedMechanismObservationRequestV1, MechanismDisclosureV1, MechanismIncidenceDisclosure,
    MechanismNormalization, MechanismObservationRequest, MechanismObservationTarget,
    MechanismQueryId, MechanismSamplingPlan,
};
use super::mechanism_stream::{
    validate_mechanism_stream_request_v1, MAX_AXES, MAX_RETAINED_EXAMPLES_PER_SIGNATURE,
};
use super::source_events::{preflight_checked_query_access, ResolvedEventAdapterLimits};
use crate::TypeCheckArtifacts;

/// Invocation policy that does not alter the query-owned observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MechanismObservationSelectionV1 {
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

/// Build the complete request from the checked Explore declaration.
///
/// This performs no I/O and must run before opening or creating durable state.
/// The only caller policy here is disclosure; the observation template itself
/// is producer-minted in the checked query artifact.
pub(crate) fn build_checked_mechanism_request_v1(
    artifacts: &TypeCheckArtifacts,
    accepted_query_index: usize,
    selection: MechanismObservationSelectionV1,
) -> Result<CheckedMechanismObservationRequestV1, MechanismRequestConfigurationError> {
    if selection.retained_examples_per_signature as usize > MAX_RETAINED_EXAMPLES_PER_SIGNATURE {
        return Err(invalid(format!(
            "mechanism retained examples per signature {} exceeds limit {MAX_RETAINED_EXAMPLES_PER_SIGNATURE}",
            selection.retained_examples_per_signature
        )));
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
    let template = checked
        .artifact
        .mechanism_observation
        .clone()
        .ok_or_else(|| {
            invalid(
                "mechanism observation is not declared; add `observe mechanisms with CALLABLE` to the Explore query",
            )
        })?;

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
        template,
        MechanismNormalization::DynamicControlV1,
        axis_cardinalities,
        MechanismSamplingPlan::empty(),
        Box::default(),
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
