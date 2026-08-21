//! Proof-backed one-axis classification regions for checked Explore queries.
//!
//! Source events are used only as deterministic split hints. Closure comes
//! from the exact profile-specialized Boolean formula retained by the checked
//! adapter. Each quasi-affine atom is bounded over a finite integer interval;
//! a region closes only when the resulting Boolean truth domain is a
//! singleton. Unproved regions remain explicit and can be refined, sent to a
//! solver, or evaluated as residual singletons.

use std::collections::BTreeSet;
use std::fmt;
use std::num::NonZeroUsize;

use sha2::{Digest, Sha256};

use super::boundary_plan::BoundaryInterval;
use super::case_graph::CaseTerminal;
use super::source_events::{
    AffineForm, BoundaryRelation, PreparedResolvedEventAdapter, ResolvedBoundaryFragment,
    ResolvedClassificationFormula, ResolvedFragmentCoverage, ResolvedQuasiAffineForm,
    SourceEventExtraction,
};
use super::{ExploreExactDomain, ExplorePolarity, ExploreQueryIr};

const CERTIFICATE_DOMAIN: &[u8] = b"futuruna.explore.classification-region.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ClassificationRegionOptions {
    pub(super) max_refinement_cells: NonZeroUsize,
}

/// First-generation durable-probe proof-refinement budget for one profile.
///
/// Across the 64-profile probe cap this permits at most 32,768 inspected
/// refinement cells. Any cells beyond the cap are retained as explicit open
/// intervals and therefore remain exact evaluator fallback work.
pub(super) const SOURCE_PROOF_CLASSIFICATION_OPTIONS_V1: ClassificationRegionOptions =
    ClassificationRegionOptions {
        max_refinement_cells: NonZeroUsize::new(512).unwrap(),
    };

impl Default for ClassificationRegionOptions {
    fn default() -> Self {
        SOURCE_PROOF_CLASSIFICATION_OPTIONS_V1
    }
}

/// Opaque proof identity retained on one scheduler interval. Its fields bind
/// the exact checked program/query/profile/formula and the proved raw truth.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct ClassificationRegionCertificate {
    id: Box<str>,
    analysis_program_hash: Box<str>,
    query_hash: Box<str>,
    formula_hash: Box<str>,
    outer_ordinals: Box<[u128]>,
    axis_name: Box<str>,
    interval: BoundaryInterval,
    raw_question_value: bool,
}

impl ClassificationRegionCertificate {
    pub(super) fn id(&self) -> &str {
        &self.id
    }

    pub(super) fn interval(&self) -> BoundaryInterval {
        self.interval
    }

    pub(super) const fn raw_question_value(&self) -> bool {
        self.raw_question_value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CertifiedBoundaryClassification {
    outer_ordinals: Box<[u128]>,
    interval: BoundaryInterval,
    classification: CaseTerminal,
    certificate: ClassificationRegionCertificate,
}

impl CertifiedBoundaryClassification {
    pub(super) fn outer_ordinals(&self) -> &[u128] {
        &self.outer_ordinals
    }

    pub(super) const fn interval(&self) -> BoundaryInterval {
        self.interval
    }

    pub(super) fn classification(&self) -> &CaseTerminal {
        &self.classification
    }

    pub(super) fn certificate(&self) -> &ClassificationRegionCertificate {
        &self.certificate
    }

    /// Revalidate the complete proof identity at the producer boundary. This
    /// is deliberately stronger than checking the interval alone: a cloned
    /// certificate cannot be reattached to another profile, truth value,
    /// polarity, formula, program, or checked query.
    fn validate_certificate(
        &self,
        prepared: &PreparedResolvedEventAdapter<'_>,
        extraction: &SourceEventExtraction,
        fragment: &ResolvedBoundaryFragment,
        expected_formula_hash: &str,
    ) -> Result<(), ClassificationRegionError> {
        let query = prepared.checked_query();
        let boundary = query
            .universe
            .boundary
            .as_ref()
            .ok_or(ClassificationRegionError::QueryHasNoBoundary)?;
        let dimension = query
            .universe
            .dimensions
            .get(boundary.axis_dimension_index)
            .ok_or(ClassificationRegionError::BoundaryAxisMismatch)?;
        let (start, end_exclusive) = match &dimension.domain {
            ExploreExactDomain::IntRange {
                start,
                end_exclusive,
                ..
            } => (*start, *end_exclusive),
            ExploreExactDomain::Enumerated { .. } | ExploreExactDomain::FiniteType { .. } => {
                return Err(ClassificationRegionError::UnsupportedBoundaryDomain)
            }
        };
        let eligible_end = i128::from(end_exclusive)
            .checked_sub(i128::from(boundary.step))
            .ok_or(ClassificationRegionError::ArithmeticOverflow(
                "validating the eligible certificate boundary",
            ))?
            .max(i128::from(start));
        let eligible_end = i64::try_from(eligible_end).map_err(|_| {
            ClassificationRegionError::ArithmeticOverflow(
                "converting the eligible certificate boundary",
            )
        })?;
        let eligible = BoundaryInterval::new(start, eligible_end)
            .map_err(|error| ClassificationRegionError::Boundary(error.to_string()))?;

        if dimension.name != boundary.axis
            || extraction.axis_name != boundary.axis
            || extraction.step != boundary.step
            || self.interval.is_empty()
            || !self.interval.is_within(eligible)
        {
            return Err(ClassificationRegionError::CertificateIdentityMismatch(
                "boundary axis or interval",
            ));
        }
        if extraction.analysis_program_hash != prepared.analysis_program_hash()
            || extraction.query_hash != prepared.query_hash()
            || extraction.outer_ordinals.as_ref() != self.outer_ordinals.as_ref()
            || fragment.analysis_program_hash.as_ref() != prepared.analysis_program_hash()
            || fragment.query_hash.as_ref() != prepared.query_hash()
            || fragment.outer_ordinals.as_ref() != self.outer_ordinals.as_ref()
            || fragment.axis_name.as_ref() != boundary.axis
            || fragment.step != boundary.step
        {
            return Err(ClassificationRegionError::CertificateIdentityMismatch(
                "producer, query, or outer profile",
            ));
        }
        if self.certificate.analysis_program_hash.as_ref() != prepared.analysis_program_hash()
            || self.certificate.query_hash.as_ref() != prepared.query_hash()
            || self.certificate.outer_ordinals.as_ref() != self.outer_ordinals.as_ref()
            || self.certificate.axis_name.as_ref() != boundary.axis
            || self.certificate.interval != self.interval
        {
            return Err(ClassificationRegionError::CertificateIdentityMismatch(
                "certificate payload",
            ));
        }
        if !is_lowercase_sha256(&self.certificate.formula_hash) {
            return Err(ClassificationRegionError::InvalidHash("formula_hash"));
        }
        if self.certificate.formula_hash.as_ref() != expected_formula_hash {
            return Err(ClassificationRegionError::CertificateIdentityMismatch(
                "classification formula",
            ));
        }
        let expected_classification =
            match (query.query.polarity, self.certificate.raw_question_value) {
                (ExplorePolarity::Matches, true) | (ExplorePolarity::Violations, false) => {
                    CaseTerminal::AdmissibleMatch
                }
                (ExplorePolarity::Matches, false) | (ExplorePolarity::Violations, true) => {
                    CaseTerminal::AdmissibleNonmatch
                }
            };
        if self.classification != expected_classification {
            return Err(ClassificationRegionError::CertificateIdentityMismatch(
                "query polarity and certified truth",
            ));
        }
        let expected_id = certificate_id(
            extraction,
            self.interval,
            self.certificate.raw_question_value,
            &self.certificate.formula_hash,
        );
        if self.certificate.id != expected_id {
            return Err(ClassificationRegionError::CertificateIdentityMismatch(
                "certificate id",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ClassificationRegionProof {
    regions: Box<[CertifiedBoundaryClassification]>,
    open_intervals: Box<[BoundaryInterval]>,
    inspected_cells: usize,
    certified_cases: u128,
    open_cases: u128,
    refinement_limit_reached: bool,
}

impl ClassificationRegionProof {
    pub(super) fn regions(&self) -> &[CertifiedBoundaryClassification] {
        &self.regions
    }

    pub(super) const fn is_complete(&self) -> bool {
        self.open_cases == 0
    }

    /// Validate the formula once, then bind every retained region to that same
    /// checked formula and producer identity. This avoids re-hashing a large
    /// formula for every cell while preserving per-certificate validation.
    pub(super) fn validate_certificates(
        &self,
        prepared: &PreparedResolvedEventAdapter<'_>,
        extraction: &SourceEventExtraction,
        fragment: &ResolvedBoundaryFragment,
    ) -> Result<(), ClassificationRegionError> {
        validate_formula(&fragment.classification)?;
        let expected_formula_hash = formula_hash(&fragment.classification);
        for region in self.regions.iter() {
            region.validate_certificate(prepared, extraction, fragment, &expected_formula_hash)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ClassificationRegionError {
    QueryHasNoBoundary,
    BoundaryAxisMismatch,
    UnsupportedBoundaryDomain,
    RuntimeConstraintsRequireSeparateProof,
    ExtractionIdentityMismatch,
    CertificateIdentityMismatch(&'static str),
    ExtractionIncomplete,
    FragmentIncomplete,
    InvalidHash(&'static str),
    InvalidFormula(&'static str),
    ArithmeticOverflow(&'static str),
    Boundary(String),
    CardinalityOverflow,
}

impl fmt::Display for ClassificationRegionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QueryHasNoBoundary => {
                formatter.write_str("classification-region proof requires a boundary query")
            }
            Self::BoundaryAxisMismatch => formatter.write_str(
                "classification-region query, extraction and fragment boundary identities disagree",
            ),
            Self::UnsupportedBoundaryDomain => formatter.write_str(
                "classification-region proof currently requires a dense Int boundary range",
            ),
            Self::RuntimeConstraintsRequireSeparateProof => formatter.write_str(
                "classification-region proof cannot absorb Explore where constraints without a separate admissibility formula",
            ),
            Self::ExtractionIdentityMismatch => formatter.write_str(
                "classification-region extraction does not identify the selected checked query/profile",
            ),
            Self::CertificateIdentityMismatch(field) => write!(
                formatter,
                "classification-region certificate does not match its {field}"
            ),
            Self::ExtractionIncomplete => formatter.write_str(
                "incomplete source-event extraction cannot seed classification closure",
            ),
            Self::FragmentIncomplete => formatter.write_str(
                "incomplete checked classification formula cannot close a region",
            ),
            Self::InvalidHash(field) => write!(formatter, "{field} is not a lowercase SHA-256"),
            Self::InvalidFormula(message) => write!(formatter, "invalid quasi-affine formula: {message}"),
            Self::ArithmeticOverflow(context) => {
                write!(formatter, "classification-region arithmetic overflowed while {context}")
            }
            Self::Boundary(message) => formatter.write_str(message),
            Self::CardinalityOverflow => {
                formatter.write_str("classification-region cardinality exceeds u128::MAX")
            }
        }
    }
}

impl std::error::Error for ClassificationRegionError {}

/// Certify exact classification cells for one already-specialized outer
/// profile. Candidate locations only pre-partition work; deleting every
/// candidate would preserve soundness, though it may increase refinements.
pub(super) fn certify_profile_classification_regions(
    prepared: &PreparedResolvedEventAdapter<'_>,
    extraction: &SourceEventExtraction,
    fragment: &ResolvedBoundaryFragment,
    options: ClassificationRegionOptions,
) -> Result<ClassificationRegionProof, ClassificationRegionError> {
    // Query semantics and producer-owned hashes come from one checked bundle;
    // callers cannot pair a same-shaped IR with another query's formula and
    // thereby change the polarity or domain under a valid-looking receipt.
    let query = prepared.checked_query();
    let boundary = query
        .universe
        .boundary
        .as_ref()
        .ok_or(ClassificationRegionError::QueryHasNoBoundary)?;
    let dimension = query
        .universe
        .dimensions
        .get(boundary.axis_dimension_index)
        .ok_or(ClassificationRegionError::BoundaryAxisMismatch)?;
    if dimension.name != boundary.axis
        || fragment.axis_name.as_ref() != boundary.axis
        || extraction.axis_name != boundary.axis
        || extraction.step != boundary.step
        || extraction.query_name != query.query.name.as_deref().unwrap_or("<anonymous>")
    {
        return Err(ClassificationRegionError::BoundaryAxisMismatch);
    }
    if !query.universe.constraints.is_empty() {
        return Err(ClassificationRegionError::RuntimeConstraintsRequireSeparateProof);
    }
    if !is_lowercase_sha256(&extraction.analysis_program_hash) {
        return Err(ClassificationRegionError::InvalidHash(
            "analysis_program_hash",
        ));
    }
    if !is_lowercase_sha256(&extraction.query_hash) {
        return Err(ClassificationRegionError::InvalidHash("query_hash"));
    }
    if extraction.analysis_program_hash != prepared.analysis_program_hash()
        || extraction.query_hash != prepared.query_hash()
    {
        return Err(ClassificationRegionError::ExtractionIdentityMismatch);
    }
    if extraction.outer_ordinals.len() + 1 != query.universe.dimensions.len() {
        return Err(ClassificationRegionError::ExtractionIdentityMismatch);
    }
    if fragment.analysis_program_hash.as_ref() != extraction.analysis_program_hash
        || fragment.query_hash.as_ref() != extraction.query_hash
        || fragment.outer_ordinals.as_ref() != extraction.outer_ordinals.as_ref()
        || fragment.step != extraction.step
    {
        return Err(ClassificationRegionError::ExtractionIdentityMismatch);
    }
    if !extraction.extraction_complete {
        return Err(ClassificationRegionError::ExtractionIncomplete);
    }
    if !matches!(&fragment.coverage, ResolvedFragmentCoverage::Complete) {
        return Err(ClassificationRegionError::FragmentIncomplete);
    }
    validate_formula(&fragment.classification)?;

    let (start, end_exclusive) = match &dimension.domain {
        ExploreExactDomain::IntRange {
            start,
            end_exclusive,
            ..
        } => (*start, *end_exclusive),
        ExploreExactDomain::Enumerated { .. } | ExploreExactDomain::FiniteType { .. } => {
            return Err(ClassificationRegionError::UnsupportedBoundaryDomain)
        }
    };
    let eligible_end = i128::from(end_exclusive)
        .checked_sub(i128::from(boundary.step))
        .ok_or(ClassificationRegionError::ArithmeticOverflow(
            "computing the eligible boundary end",
        ))?
        .max(i128::from(start));
    let eligible_end = i64::try_from(eligible_end).map_err(|_| {
        ClassificationRegionError::ArithmeticOverflow("converting the eligible boundary end")
    })?;
    let eligible = BoundaryInterval::new(start, eligible_end)
        .map_err(|error| ClassificationRegionError::Boundary(error.to_string()))?;
    if eligible.is_empty() {
        return Ok(ClassificationRegionProof {
            regions: Box::new([]),
            open_intervals: Box::new([]),
            inspected_cells: 0,
            certified_cases: 0,
            open_cases: 0,
            refinement_limit_reached: false,
        });
    }

    let formula_hash = formula_hash(&fragment.classification);
    let mut cuts = BTreeSet::from([eligible.start(), eligible.end_exclusive()]);
    for candidate in extraction.candidates.iter() {
        let point = candidate.boundary_value;
        if !eligible.contains(point) {
            return Err(ClassificationRegionError::ExtractionIdentityMismatch);
        }
        cuts.insert(point);
        cuts.insert(
            point
                .checked_add(1)
                .ok_or(ClassificationRegionError::ArithmeticOverflow(
                    "isolating a source candidate",
                ))?,
        );
    }
    let cuts = cuts.into_iter().collect::<Vec<_>>();
    let mut stack = cuts
        .windows(2)
        .rev()
        .map(|window| BoundaryInterval::new(window[0], window[1]))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| ClassificationRegionError::Boundary(error.to_string()))?;
    let mut regions = Vec::new();
    let mut open = Vec::new();
    let mut inspected_cells = 0_usize;
    let mut refinement_limit_reached = false;

    while let Some(interval) = stack.pop() {
        if inspected_cells >= options.max_refinement_cells.get() {
            refinement_limit_reached = true;
            open.push(interval);
            open.extend(stack.drain(..));
            break;
        }
        inspected_cells += 1;
        let truth = truth_domain(&fragment.classification, interval)?;
        if let Some(raw_question_value) = truth.exact() {
            regions.push(certified_region(
                query,
                extraction,
                interval,
                raw_question_value,
                &formula_hash,
            ));
            continue;
        }
        if interval.cardinality() == 1 {
            match evaluate_formula(&fragment.classification, i128::from(interval.start()))? {
                Some(raw_question_value) => regions.push(certified_region(
                    query,
                    extraction,
                    interval,
                    raw_question_value,
                    &formula_hash,
                )),
                None => open.push(interval),
            }
            continue;
        }
        let midpoint =
            interval
                .canonical_midpoint()
                .ok_or(ClassificationRegionError::InvalidFormula(
                    "nonempty refinement cell has no midpoint",
                ))?;
        let left = BoundaryInterval::new(interval.start(), midpoint)
            .map_err(|error| ClassificationRegionError::Boundary(error.to_string()))?;
        let right = BoundaryInterval::new(midpoint, interval.end_exclusive())
            .map_err(|error| ClassificationRegionError::Boundary(error.to_string()))?;
        if left.is_empty() || right.is_empty() {
            return Err(ClassificationRegionError::InvalidFormula(
                "refinement did not strictly partition a non-singleton cell",
            ));
        }
        stack.push(right);
        stack.push(left);
    }

    regions.sort_by_key(|region| region.interval);
    open.sort();
    let certified_cases = sum_intervals(regions.iter().map(|region| region.interval))?;
    let open_cases = sum_intervals(open.iter().copied())?;
    if certified_cases
        .checked_add(open_cases)
        .ok_or(ClassificationRegionError::CardinalityOverflow)?
        != eligible.cardinality()
    {
        return Err(ClassificationRegionError::InvalidFormula(
            "certified and open cells do not cover the eligible boundary",
        ));
    }
    Ok(ClassificationRegionProof {
        regions: regions.into_boxed_slice(),
        open_intervals: open.into_boxed_slice(),
        inspected_cells,
        certified_cases,
        open_cases,
        refinement_limit_reached,
    })
}

fn certified_region(
    query: &ExploreQueryIr,
    extraction: &SourceEventExtraction,
    interval: BoundaryInterval,
    raw_question_value: bool,
    formula_hash: &str,
) -> CertifiedBoundaryClassification {
    let is_match = match query.query.polarity {
        ExplorePolarity::Matches => raw_question_value,
        ExplorePolarity::Violations => !raw_question_value,
    };
    let classification = if is_match {
        CaseTerminal::AdmissibleMatch
    } else {
        CaseTerminal::AdmissibleNonmatch
    };
    let id = certificate_id(extraction, interval, raw_question_value, formula_hash);
    CertifiedBoundaryClassification {
        outer_ordinals: extraction.outer_ordinals.clone(),
        interval,
        classification,
        certificate: ClassificationRegionCertificate {
            id,
            analysis_program_hash: extraction.analysis_program_hash.clone().into_boxed_str(),
            query_hash: extraction.query_hash.clone().into_boxed_str(),
            formula_hash: formula_hash.into(),
            outer_ordinals: extraction.outer_ordinals.clone(),
            axis_name: extraction.axis_name.clone().into_boxed_str(),
            interval,
            raw_question_value,
        },
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TruthDomain {
    can_be_false: bool,
    can_be_true: bool,
}

impl TruthDomain {
    const FALSE: Self = Self {
        can_be_false: true,
        can_be_true: false,
    };
    const TRUE: Self = Self {
        can_be_false: false,
        can_be_true: true,
    };
    const BOTH: Self = Self {
        can_be_false: true,
        can_be_true: true,
    };

    const fn exact(self) -> Option<bool> {
        match (self.can_be_false, self.can_be_true) {
            (true, false) => Some(false),
            (false, true) => Some(true),
            _ => None,
        }
    }

    const fn negate(self) -> Self {
        Self {
            can_be_false: self.can_be_true,
            can_be_true: self.can_be_false,
        }
    }
}

fn truth_domain(
    formula: &ResolvedClassificationFormula,
    interval: BoundaryInterval,
) -> Result<TruthDomain, ClassificationRegionError> {
    match formula {
        ResolvedClassificationFormula::Constant(false) => Ok(TruthDomain::FALSE),
        ResolvedClassificationFormula::Constant(true) => Ok(TruthDomain::TRUE),
        ResolvedClassificationFormula::Comparison {
            difference,
            relation,
            ..
        } => {
            let (minimum, maximum) = bound_quasi_affine(difference, interval)?;
            Ok(relation_domain(*relation, minimum, maximum))
        }
        ResolvedClassificationFormula::Not(inner) => {
            truth_domain(inner, interval).map(TruthDomain::negate)
        }
        ResolvedClassificationFormula::All(parts) => {
            let mut can_be_true = true;
            let mut can_be_false = false;
            for part in parts.iter() {
                let part = truth_domain(part, interval)?;
                can_be_true &= part.can_be_true;
                can_be_false |= part.can_be_false;
            }
            Ok(TruthDomain {
                can_be_false,
                can_be_true,
            })
        }
        ResolvedClassificationFormula::Any(parts) => {
            let mut can_be_true = false;
            let mut can_be_false = true;
            for part in parts.iter() {
                let part = truth_domain(part, interval)?;
                can_be_true |= part.can_be_true;
                can_be_false &= part.can_be_false;
            }
            Ok(TruthDomain {
                can_be_false,
                can_be_true,
            })
        }
        ResolvedClassificationFormula::Unsupported(_) => Ok(TruthDomain::BOTH),
    }
}

fn relation_domain(relation: BoundaryRelation, minimum: i128, maximum: i128) -> TruthDomain {
    let exact = match relation {
        BoundaryRelation::Less if maximum < 0 => Some(true),
        BoundaryRelation::Less if minimum >= 0 => Some(false),
        BoundaryRelation::LessOrEqual if maximum <= 0 => Some(true),
        BoundaryRelation::LessOrEqual if minimum > 0 => Some(false),
        BoundaryRelation::Equal if minimum == 0 && maximum == 0 => Some(true),
        BoundaryRelation::Equal if maximum < 0 || minimum > 0 => Some(false),
        BoundaryRelation::NotEqual if maximum < 0 || minimum > 0 => Some(true),
        BoundaryRelation::NotEqual if minimum == 0 && maximum == 0 => Some(false),
        BoundaryRelation::GreaterOrEqual if minimum >= 0 => Some(true),
        BoundaryRelation::GreaterOrEqual if maximum < 0 => Some(false),
        BoundaryRelation::Greater if minimum > 0 => Some(true),
        BoundaryRelation::Greater if maximum <= 0 => Some(false),
        _ => None,
    };
    match exact {
        Some(false) => TruthDomain::FALSE,
        Some(true) => TruthDomain::TRUE,
        None => TruthDomain::BOTH,
    }
}

fn bound_quasi_affine(
    form: &ResolvedQuasiAffineForm,
    interval: BoundaryInterval,
) -> Result<(i128, i128), ClassificationRegionError> {
    let lower_axis = i128::from(interval.start());
    let upper_axis = i128::from(interval.end_exclusive().checked_sub(1).ok_or(
        ClassificationRegionError::InvalidFormula("empty proof interval"),
    )?);
    let (mut minimum, mut maximum) = affine_bounds(form.affine, lower_axis, upper_axis)?;
    for term in form.quantized_terms.iter() {
        let (numerator_minimum, numerator_maximum) =
            affine_bounds(term.numerator, lower_axis, upper_axis)?;
        let numerator_minimum = if term.nonnegative_numerator {
            numerator_minimum.max(0)
        } else {
            numerator_minimum
        };
        let numerator_maximum = if term.nonnegative_numerator {
            numerator_maximum.max(0)
        } else {
            numerator_maximum
        };
        let divisor = i128::from(term.positive_divisor);
        let quotient_minimum = numerator_minimum.checked_div(divisor).ok_or(
            ClassificationRegionError::ArithmeticOverflow("bounding constant division"),
        )?;
        let quotient_maximum = numerator_maximum.checked_div(divisor).ok_or(
            ClassificationRegionError::ArithmeticOverflow("bounding constant division"),
        )?;
        let left = quotient_minimum.checked_mul(term.coefficient).ok_or(
            ClassificationRegionError::ArithmeticOverflow("scaling a quantized bound"),
        )?;
        let right = quotient_maximum.checked_mul(term.coefficient).ok_or(
            ClassificationRegionError::ArithmeticOverflow("scaling a quantized bound"),
        )?;
        minimum = minimum.checked_add(left.min(right)).ok_or(
            ClassificationRegionError::ArithmeticOverflow("summing lower bounds"),
        )?;
        maximum = maximum.checked_add(left.max(right)).ok_or(
            ClassificationRegionError::ArithmeticOverflow("summing upper bounds"),
        )?;
    }
    Ok((minimum, maximum))
}

fn affine_bounds(
    affine: AffineForm,
    lower_axis: i128,
    upper_axis: i128,
) -> Result<(i128, i128), ClassificationRegionError> {
    let lower = evaluate_affine(affine, lower_axis)?;
    let upper = evaluate_affine(affine, upper_axis)?;
    Ok((lower.min(upper), lower.max(upper)))
}

fn evaluate_formula(
    formula: &ResolvedClassificationFormula,
    axis: i128,
) -> Result<Option<bool>, ClassificationRegionError> {
    match formula {
        ResolvedClassificationFormula::Constant(value) => Ok(Some(*value)),
        ResolvedClassificationFormula::Comparison {
            difference,
            relation,
            ..
        } => evaluate_quasi_affine(difference, axis)
            .map(|difference| Some(evaluate_relation(*relation, difference))),
        ResolvedClassificationFormula::Not(inner) => {
            evaluate_formula(inner, axis).map(|value| value.map(|value| !value))
        }
        ResolvedClassificationFormula::All(parts) => {
            let mut unknown = false;
            for part in parts.iter() {
                match evaluate_formula(part, axis)? {
                    Some(false) => return Ok(Some(false)),
                    Some(true) => {}
                    None => unknown = true,
                }
            }
            Ok((!unknown).then_some(true))
        }
        ResolvedClassificationFormula::Any(parts) => {
            let mut unknown = false;
            for part in parts.iter() {
                match evaluate_formula(part, axis)? {
                    Some(true) => return Ok(Some(true)),
                    Some(false) => {}
                    None => unknown = true,
                }
            }
            Ok((!unknown).then_some(false))
        }
        ResolvedClassificationFormula::Unsupported(_) => Ok(None),
    }
}

fn evaluate_quasi_affine(
    form: &ResolvedQuasiAffineForm,
    axis: i128,
) -> Result<i128, ClassificationRegionError> {
    let mut value = evaluate_affine(form.affine, axis)?;
    for term in form.quantized_terms.iter() {
        let numerator = evaluate_affine(term.numerator, axis)?;
        let numerator = if term.nonnegative_numerator {
            numerator.max(0)
        } else {
            numerator
        };
        let quotient = numerator
            .checked_div(i128::from(term.positive_divisor))
            .ok_or(ClassificationRegionError::ArithmeticOverflow(
                "evaluating constant division",
            ))?;
        value = quotient
            .checked_mul(term.coefficient)
            .and_then(|term| value.checked_add(term))
            .ok_or(ClassificationRegionError::ArithmeticOverflow(
                "evaluating a quasi-affine expression",
            ))?;
    }
    Ok(value)
}

fn evaluate_affine(affine: AffineForm, axis: i128) -> Result<i128, ClassificationRegionError> {
    affine
        .coefficient
        .checked_mul(axis)
        .and_then(|value| value.checked_add(affine.intercept))
        .ok_or(ClassificationRegionError::ArithmeticOverflow(
            "evaluating an affine expression",
        ))
}

fn evaluate_relation(relation: BoundaryRelation, difference: i128) -> bool {
    match relation {
        BoundaryRelation::Less => difference < 0,
        BoundaryRelation::LessOrEqual => difference <= 0,
        BoundaryRelation::Equal => difference == 0,
        BoundaryRelation::NotEqual => difference != 0,
        BoundaryRelation::GreaterOrEqual => difference >= 0,
        BoundaryRelation::Greater => difference > 0,
    }
}

fn validate_formula(
    formula: &ResolvedClassificationFormula,
) -> Result<(), ClassificationRegionError> {
    match formula {
        ResolvedClassificationFormula::Constant(_) => Ok(()),
        ResolvedClassificationFormula::Comparison { difference, .. } => {
            let mut prior = None;
            for term in difference.quantized_terms.iter() {
                if term.positive_divisor <= 0 || term.coefficient == 0 {
                    return Err(ClassificationRegionError::InvalidFormula(
                        "quantized terms require a positive divisor and nonzero coefficient",
                    ));
                }
                if prior.is_some_and(|prior| prior >= term) {
                    return Err(ClassificationRegionError::InvalidFormula(
                        "quantized terms are not in strict canonical order",
                    ));
                }
                prior = Some(term);
            }
            Ok(())
        }
        ResolvedClassificationFormula::Not(inner) => validate_formula(inner),
        ResolvedClassificationFormula::All(parts) | ResolvedClassificationFormula::Any(parts) => {
            for part in parts.iter() {
                validate_formula(part)?;
            }
            Ok(())
        }
        ResolvedClassificationFormula::Unsupported(_) => {
            Err(ClassificationRegionError::FragmentIncomplete)
        }
    }
}

fn sum_intervals(
    intervals: impl IntoIterator<Item = BoundaryInterval>,
) -> Result<u128, ClassificationRegionError> {
    intervals.into_iter().try_fold(0_u128, |total, interval| {
        total
            .checked_add(interval.cardinality())
            .ok_or(ClassificationRegionError::CardinalityOverflow)
    })
}

fn formula_hash(formula: &ResolvedClassificationFormula) -> String {
    let mut hash = FramedHash::new(b"classification-formula.v1");
    hash.formula(formula);
    hash.finish()
}

fn certificate_id(
    extraction: &SourceEventExtraction,
    interval: BoundaryInterval,
    raw_question_value: bool,
    formula_hash: &str,
) -> Box<str> {
    let mut hash = FramedHash::new(CERTIFICATE_DOMAIN);
    hash.bytes(extraction.analysis_program_hash.as_bytes());
    hash.bytes(extraction.query_hash.as_bytes());
    hash.bytes(extraction.axis_name.as_bytes());
    hash.i64(extraction.step);
    hash.u64(extraction.outer_ordinals.len() as u64);
    for ordinal in extraction.outer_ordinals.iter() {
        hash.u128(*ordinal);
    }
    hash.i64(interval.start());
    hash.i64(interval.end_exclusive());
    hash.byte(u8::from(raw_question_value));
    hash.bytes(formula_hash.as_bytes());
    hash.finish().into_boxed_str()
}

struct FramedHash(Sha256);

impl FramedHash {
    fn new(domain: &[u8]) -> Self {
        let mut value = Self(Sha256::new());
        value.bytes(domain);
        value
    }

    fn bytes(&mut self, value: &[u8]) {
        self.0.update((value.len() as u64).to_le_bytes());
        self.0.update(value);
    }

    fn byte(&mut self, value: u8) {
        self.bytes(&[value]);
    }

    fn i64(&mut self, value: i64) {
        self.bytes(&value.to_le_bytes());
    }

    fn i128(&mut self, value: i128) {
        self.bytes(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.bytes(&value.to_le_bytes());
    }

    fn affine(&mut self, value: AffineForm) {
        self.i128(value.coefficient);
        self.i128(value.intercept);
    }

    fn formula(&mut self, formula: &ResolvedClassificationFormula) {
        match formula {
            ResolvedClassificationFormula::Constant(value) => {
                self.byte(0);
                self.byte(u8::from(*value));
            }
            ResolvedClassificationFormula::Comparison {
                difference,
                relation,
                source,
            } => {
                self.byte(1);
                self.affine(difference.affine);
                self.u64(difference.quantized_terms.len() as u64);
                for term in difference.quantized_terms.iter() {
                    self.bytes(term.source.declaration_id.as_bytes());
                    self.u64(term.source.ast_path.len() as u64);
                    for child in term.source.ast_path.iter() {
                        self.bytes(&child.to_le_bytes());
                    }
                    self.affine(term.numerator);
                    self.i64(term.positive_divisor);
                    self.byte(u8::from(term.nonnegative_numerator));
                    self.i128(term.coefficient);
                }
                self.byte(relation_tag(*relation));
                self.bytes(source.id.declaration_id.as_bytes());
                self.u64(source.id.ast_path.len() as u64);
                for child in source.id.ast_path.iter() {
                    self.bytes(&child.to_le_bytes());
                }
            }
            ResolvedClassificationFormula::Not(inner) => {
                self.byte(2);
                self.formula(inner);
            }
            ResolvedClassificationFormula::All(parts) => {
                self.byte(3);
                self.u64(parts.len() as u64);
                for part in parts.iter() {
                    self.formula(part);
                }
            }
            ResolvedClassificationFormula::Any(parts) => {
                self.byte(4);
                self.u64(parts.len() as u64);
                for part in parts.iter() {
                    self.formula(part);
                }
            }
            ResolvedClassificationFormula::Unsupported(residual) => {
                self.byte(5);
                self.bytes(residual.detail.as_bytes());
            }
        }
    }

    fn finish(self) -> String {
        format!("{:x}", self.0.finalize())
    }
}

fn relation_tag(relation: BoundaryRelation) -> u8 {
    match relation {
        BoundaryRelation::Less => 0,
        BoundaryRelation::LessOrEqual => 1,
        BoundaryRelation::Equal => 2,
        BoundaryRelation::NotEqual => 3,
        BoundaryRelation::GreaterOrEqual => 4,
        BoundaryRelation::Greater => 5,
    }
}

fn is_lowercase_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}
