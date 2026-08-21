//! Canonical bounded transcript for the resumable source-probe phase.
//!
//! The transcript deliberately stores only facts the checked source adapter
//! already produced: canonical candidate ranks, exact coverage accounting and
//! cap/fallback status. It does not assign mechanism labels. The candidate and
//! optional closed-region bodies remain in their existing content-addressed
//! codecs; this compact manifest binds those blobs so replay can continue with
//! coverage and candidate evaluation without running source analysis again.

use std::error::Error;
use std::fmt;

use super::run_stream::{CanonicalDigest, ExploreRunHeader};
use super::source_proof_plan::DEFAULT_SOURCE_PROOF_PROFILE_LIMIT;
use super::stream_proof::{
    source_proof_candidate_rank_limit_v1, SourceProofExactCoverageSummaryV1,
};

const SOURCE_PROBE_MANIFEST_MAGIC_V1: &[u8; 8] = b"FXPRB001";
const SOURCE_PROBE_MANIFEST_VERSION_V1: u16 = 1;

pub(super) const SOURCE_PROBE_MANIFEST_BLOB_KIND_V1: &str = "source-probe-manifest-v1";
pub(super) const SOURCE_PROBE_MANIFEST_MAX_BYTES_V1: usize = 512;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum ExactSourceProbeModeV1 {
    CheckedSourceProof,
    CanonicalFallback,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) struct ExactSourceProbeCoverageSummaryV1 {
    universe_case_count: u128,
    certified_closed_case_count: u128,
    residual_open_case_count: u128,
    boundary_rank_stride: Option<u128>,
    sealed_proof_nonmatch_cases: Option<u128>,
    open_proof_nonmatch_cases: Option<u128>,
    open_proof_match_cases: Option<u128>,
    sealed_structural_excluded_cases: Option<u128>,
    open_structural_excluded_cases: Option<u128>,
    total_outer_profiles: Option<u128>,
    analyzed_outer_profiles: Option<u128>,
    proof_incomplete_profiles: Option<u128>,
    profile_limit_reached: Option<bool>,
    region_limit_reached: Option<bool>,
    candidate_limit_reached: Option<bool>,
}

impl ExactSourceProbeCoverageSummaryV1 {
    fn from_checked_source(
        summary: SourceProofExactCoverageSummaryV1,
        total_outer_profiles: u128,
        analyzed_outer_profiles: u128,
        proof_incomplete_profiles: u128,
        profile_limit_reached: bool,
    ) -> Result<Self, ExactSourceProbeManifestError> {
        let certified_closed_case_count = summary
            .sealed_proof_nonmatch_cases()
            .checked_add(summary.sealed_structural_excluded_cases())
            .ok_or_else(|| {
                ExactSourceProbeManifestError::invalid(
                    "source-probe certified coverage count exceeds u128::MAX",
                )
            })?;
        let residual_open_case_count = summary
            .universe_case_count()
            .checked_sub(certified_closed_case_count)
            .ok_or_else(|| {
                ExactSourceProbeManifestError::invalid(
                    "source-probe certified coverage exceeds its universe",
                )
            })?;
        if analyzed_outer_profiles > total_outer_profiles
            || proof_incomplete_profiles > analyzed_outer_profiles
            || profile_limit_reached != (analyzed_outer_profiles < total_outer_profiles)
        {
            return Err(ExactSourceProbeManifestError::invalid(
                "source-probe outer-profile accounting is inconsistent",
            ));
        }
        Ok(Self {
            universe_case_count: summary.universe_case_count(),
            certified_closed_case_count,
            residual_open_case_count,
            boundary_rank_stride: Some(summary.boundary_rank_stride()),
            sealed_proof_nonmatch_cases: Some(summary.sealed_proof_nonmatch_cases()),
            open_proof_nonmatch_cases: Some(summary.open_proof_nonmatch_cases()),
            open_proof_match_cases: Some(summary.open_proof_match_cases()),
            sealed_structural_excluded_cases: Some(summary.sealed_structural_excluded_cases()),
            open_structural_excluded_cases: Some(summary.open_structural_excluded_cases()),
            total_outer_profiles: Some(total_outer_profiles),
            analyzed_outer_profiles: Some(analyzed_outer_profiles),
            proof_incomplete_profiles: Some(proof_incomplete_profiles),
            profile_limit_reached: Some(profile_limit_reached),
            region_limit_reached: Some(summary.region_limit_reached()),
            candidate_limit_reached: Some(summary.candidate_limit_reached()),
        })
    }

    fn canonical_fallback(universe_case_count: u128) -> Self {
        Self {
            universe_case_count,
            certified_closed_case_count: 0,
            residual_open_case_count: universe_case_count,
            boundary_rank_stride: None,
            sealed_proof_nonmatch_cases: None,
            open_proof_nonmatch_cases: None,
            open_proof_match_cases: None,
            sealed_structural_excluded_cases: None,
            open_structural_excluded_cases: None,
            total_outer_profiles: None,
            analyzed_outer_profiles: None,
            proof_incomplete_profiles: None,
            profile_limit_reached: None,
            region_limit_reached: None,
            candidate_limit_reached: None,
        }
    }

    pub(super) const fn universe_case_count(self) -> u128 {
        self.universe_case_count
    }

    pub(super) const fn certified_closed_case_count(self) -> u128 {
        self.certified_closed_case_count
    }

    pub(super) const fn residual_open_case_count(self) -> u128 {
        self.residual_open_case_count
    }

    pub(super) const fn boundary_rank_stride(self) -> Option<u128> {
        self.boundary_rank_stride
    }

    pub(super) const fn sealed_proof_nonmatch_cases(self) -> Option<u128> {
        self.sealed_proof_nonmatch_cases
    }

    pub(super) const fn open_proof_nonmatch_cases(self) -> Option<u128> {
        self.open_proof_nonmatch_cases
    }

    pub(super) const fn open_proof_match_cases(self) -> Option<u128> {
        self.open_proof_match_cases
    }

    pub(super) const fn sealed_structural_excluded_cases(self) -> Option<u128> {
        self.sealed_structural_excluded_cases
    }

    pub(super) const fn open_structural_excluded_cases(self) -> Option<u128> {
        self.open_structural_excluded_cases
    }

    pub(super) const fn total_outer_profiles(self) -> Option<u128> {
        self.total_outer_profiles
    }

    pub(super) const fn analyzed_outer_profiles(self) -> Option<u128> {
        self.analyzed_outer_profiles
    }

    pub(super) const fn proof_incomplete_profiles(self) -> Option<u128> {
        self.proof_incomplete_profiles
    }

    pub(super) const fn profile_limit_reached(self) -> Option<bool> {
        self.profile_limit_reached
    }

    pub(super) const fn region_limit_reached(self) -> Option<bool> {
        self.region_limit_reached
    }

    pub(super) const fn candidate_limit_reached(self) -> Option<bool> {
        self.candidate_limit_reached
    }

    /// Residual CaseIds for which the bounded source pass produced no explicit
    /// proof/structural classification. This includes profiles beyond the
    /// atomic profile cap and eligible cells left open by incomplete proofs.
    pub(super) fn unaccounted_open_case_count(self) -> Option<u128> {
        if !self.source_summary_available() {
            return None;
        }
        let explicitly_accounted_open = self
            .open_proof_nonmatch_cases?
            .checked_add(self.open_proof_match_cases?)?
            .checked_add(self.open_structural_excluded_cases?)?;
        self.residual_open_case_count
            .checked_sub(explicitly_accounted_open)
    }

    pub(super) const fn source_summary_available(self) -> bool {
        self.boundary_rank_stride.is_some()
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) struct ExactSourceProbeManifestV1 {
    mode: ExactSourceProbeModeV1,
    proof_set_id: CanonicalDigest,
    candidate_blob: CanonicalDigest,
    candidate_count: u128,
    closed_region_blob: Option<CanonicalDigest>,
    coverage: ExactSourceProbeCoverageSummaryV1,
}

impl ExactSourceProbeManifestV1 {
    pub(super) fn checked_source(
        proof_set_id: CanonicalDigest,
        candidate_blob: CanonicalDigest,
        candidate_count: usize,
        closed_region_blob: Option<CanonicalDigest>,
        summary: SourceProofExactCoverageSummaryV1,
        total_outer_profiles: u128,
        analyzed_outer_profiles: u128,
        proof_incomplete_profiles: u128,
        profile_limit_reached: bool,
    ) -> Result<Self, ExactSourceProbeManifestError> {
        Self::validated(
            ExactSourceProbeModeV1::CheckedSourceProof,
            proof_set_id,
            candidate_blob,
            candidate_count as u128,
            closed_region_blob,
            ExactSourceProbeCoverageSummaryV1::from_checked_source(
                summary,
                total_outer_profiles,
                analyzed_outer_profiles,
                proof_incomplete_profiles,
                profile_limit_reached,
            )?,
        )
    }

    pub(super) fn canonical_fallback(
        proof_set_id: CanonicalDigest,
        candidate_blob: CanonicalDigest,
        universe_case_count: u128,
    ) -> Result<Self, ExactSourceProbeManifestError> {
        Self::validated(
            ExactSourceProbeModeV1::CanonicalFallback,
            proof_set_id,
            candidate_blob,
            0,
            None,
            ExactSourceProbeCoverageSummaryV1::canonical_fallback(universe_case_count),
        )
    }

    fn validated(
        mode: ExactSourceProbeModeV1,
        proof_set_id: CanonicalDigest,
        candidate_blob: CanonicalDigest,
        candidate_count: u128,
        closed_region_blob: Option<CanonicalDigest>,
        coverage: ExactSourceProbeCoverageSummaryV1,
    ) -> Result<Self, ExactSourceProbeManifestError> {
        if candidate_count > source_proof_candidate_rank_limit_v1() as u128 {
            return Err(ExactSourceProbeManifestError::invalid(format!(
                "source-probe manifest candidate count {candidate_count} exceeds bound {}",
                source_proof_candidate_rank_limit_v1(),
            )));
        }
        if coverage
            .certified_closed_case_count
            .checked_add(coverage.residual_open_case_count)
            != Some(coverage.universe_case_count)
        {
            return Err(ExactSourceProbeManifestError::invalid(
                "source-probe manifest coverage does not conserve its universe",
            ));
        }
        if candidate_count > coverage.residual_open_case_count {
            return Err(ExactSourceProbeManifestError::invalid(
                "source-probe manifest contains more candidates than residual cases",
            ));
        }
        if closed_region_blob.is_none() != (coverage.certified_closed_case_count == 0) {
            return Err(ExactSourceProbeManifestError::invalid(
                "source-probe closed-region blob presence disagrees with certified coverage",
            ));
        }
        if coverage.source_summary_available() {
            let sealed_proof = coverage.sealed_proof_nonmatch_cases.ok_or_else(|| {
                ExactSourceProbeManifestError::invalid(
                    "source-probe summary is missing sealed proof nonmatches",
                )
            })?;
            let open_proof = coverage.open_proof_nonmatch_cases.ok_or_else(|| {
                ExactSourceProbeManifestError::invalid(
                    "source-probe summary is missing open proof nonmatches",
                )
            })?;
            let open_matches = coverage.open_proof_match_cases.ok_or_else(|| {
                ExactSourceProbeManifestError::invalid(
                    "source-probe summary is missing open proof matches",
                )
            })?;
            let sealed_structural = coverage.sealed_structural_excluded_cases.ok_or_else(|| {
                ExactSourceProbeManifestError::invalid(
                    "source-probe summary is missing sealed structural exclusions",
                )
            })?;
            let open_structural = coverage.open_structural_excluded_cases.ok_or_else(|| {
                ExactSourceProbeManifestError::invalid(
                    "source-probe summary is missing open structural exclusions",
                )
            })?;
            let explicitly_accounted_open = open_proof
                .checked_add(open_matches)
                .and_then(|count| count.checked_add(open_structural))
                .ok_or_else(|| {
                    ExactSourceProbeManifestError::invalid(
                        "source-probe open coverage accounting exceeds u128::MAX",
                    )
                })?;
            let _region_limit_reached = coverage.region_limit_reached.ok_or_else(|| {
                ExactSourceProbeManifestError::invalid(
                    "source-probe summary is missing region-cap status",
                )
            })?;
            let candidate_limit_reached = coverage.candidate_limit_reached.ok_or_else(|| {
                ExactSourceProbeManifestError::invalid(
                    "source-probe summary is missing candidate-cap status",
                )
            })?;
            if sealed_proof.checked_add(sealed_structural)
                != Some(coverage.certified_closed_case_count)
                || explicitly_accounted_open > coverage.residual_open_case_count
                || (candidate_limit_reached
                    && candidate_count != source_proof_candidate_rank_limit_v1() as u128)
            {
                return Err(ExactSourceProbeManifestError::invalid(
                    "source-probe source coverage summary is inconsistent",
                ));
            }
            let total = coverage.total_outer_profiles.ok_or_else(|| {
                ExactSourceProbeManifestError::invalid(
                    "source-probe summary is missing total outer profiles",
                )
            })?;
            let analyzed = coverage.analyzed_outer_profiles.ok_or_else(|| {
                ExactSourceProbeManifestError::invalid(
                    "source-probe summary is missing analyzed outer profiles",
                )
            })?;
            let incomplete = coverage.proof_incomplete_profiles.ok_or_else(|| {
                ExactSourceProbeManifestError::invalid(
                    "source-probe summary is missing incomplete-proof profiles",
                )
            })?;
            let limit_reached = coverage.profile_limit_reached.ok_or_else(|| {
                ExactSourceProbeManifestError::invalid(
                    "source-probe summary is missing profile-cap status",
                )
            })?;
            if analyzed > total
                || incomplete > analyzed
                || analyzed > DEFAULT_SOURCE_PROOF_PROFILE_LIMIT.get() as u128
                || limit_reached != (analyzed < total)
                || (limit_reached && analyzed != DEFAULT_SOURCE_PROOF_PROFILE_LIMIT.get() as u128)
            {
                return Err(ExactSourceProbeManifestError::invalid(
                    "source-probe outer-profile accounting is inconsistent",
                ));
            }
        }
        match mode {
            ExactSourceProbeModeV1::CheckedSourceProof if !coverage.source_summary_available() => {
                return Err(ExactSourceProbeManifestError::invalid(
                    "checked source-probe manifest is missing its source coverage summary",
                ));
            }
            ExactSourceProbeModeV1::CanonicalFallback
                if coverage.source_summary_available()
                    || candidate_count != 0
                    || closed_region_blob.is_some() =>
            {
                return Err(ExactSourceProbeManifestError::invalid(
                    "canonical source-probe fallback claims source-derived output",
                ));
            }
            _ => {}
        }
        Ok(Self {
            mode,
            proof_set_id,
            candidate_blob,
            candidate_count,
            closed_region_blob,
            coverage,
        })
    }

    pub(super) const fn mode(self) -> ExactSourceProbeModeV1 {
        self.mode
    }

    pub(super) const fn proof_set_id(self) -> CanonicalDigest {
        self.proof_set_id
    }

    pub(super) const fn candidate_blob(self) -> CanonicalDigest {
        self.candidate_blob
    }

    pub(super) const fn candidate_count(self) -> u128 {
        self.candidate_count
    }

    pub(super) const fn closed_region_blob(self) -> Option<CanonicalDigest> {
        self.closed_region_blob
    }

    pub(super) const fn coverage(self) -> ExactSourceProbeCoverageSummaryV1 {
        self.coverage
    }

    pub(super) fn validate_for_header(
        self,
        header: &ExploreRunHeader,
    ) -> Result<(), ExactSourceProbeManifestError> {
        if self.coverage.universe_case_count != header.case_universe().case_count() {
            return Err(ExactSourceProbeManifestError::invalid(
                "source-probe manifest universe disagrees with the run header",
            ));
        }
        Self::validated(
            self.mode,
            self.proof_set_id,
            self.candidate_blob,
            self.candidate_count,
            self.closed_region_blob,
            self.coverage,
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum ExactSourceProbePhaseV1 {
    Unprepared,
    Prepared,
    CoverageAccepted,
    CandidateActive,
    Complete,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) struct ExactSourceProbeProgressV1 {
    phase: ExactSourceProbePhaseV1,
    manifest_blob: Option<CanonicalDigest>,
    manifest: Option<ExactSourceProbeManifestV1>,
    evaluated_candidate_count: u128,
    remaining_candidate_count: u128,
}

impl ExactSourceProbeProgressV1 {
    pub(super) fn derive(
        manifest_blob: Option<CanonicalDigest>,
        manifest: Option<ExactSourceProbeManifestV1>,
        coverage_accepted: bool,
        complete: bool,
        remaining_candidate_count: usize,
    ) -> Result<Self, ExactSourceProbeManifestError> {
        if manifest_blob.is_some() != manifest.is_some() {
            return Err(ExactSourceProbeManifestError::invalid(
                "source-probe manifest blob and decoded manifest presence disagree",
            ));
        }
        let remaining_candidate_count = remaining_candidate_count as u128;
        let (phase, evaluated_candidate_count) = match manifest {
            None => {
                if coverage_accepted || complete || remaining_candidate_count != 0 {
                    return Err(ExactSourceProbeManifestError::invalid(
                        "unprepared source probe claims durable progress",
                    ));
                }
                (ExactSourceProbePhaseV1::Unprepared, 0)
            }
            Some(manifest) => {
                if remaining_candidate_count > manifest.candidate_count {
                    return Err(ExactSourceProbeManifestError::invalid(
                        "remaining source-probe candidates exceed the prepared manifest",
                    ));
                }
                if complete && (!coverage_accepted || remaining_candidate_count != 0) {
                    return Err(ExactSourceProbeManifestError::invalid(
                        "completed source probe has open candidates or no accepted coverage",
                    ));
                }
                let phase = if complete {
                    ExactSourceProbePhaseV1::Complete
                } else if !coverage_accepted {
                    ExactSourceProbePhaseV1::Prepared
                } else if remaining_candidate_count == 0 {
                    ExactSourceProbePhaseV1::CoverageAccepted
                } else {
                    ExactSourceProbePhaseV1::CandidateActive
                };
                (phase, manifest.candidate_count - remaining_candidate_count)
            }
        };
        Ok(Self {
            phase,
            manifest_blob,
            manifest,
            evaluated_candidate_count,
            remaining_candidate_count,
        })
    }

    pub(super) const fn phase(self) -> ExactSourceProbePhaseV1 {
        self.phase
    }

    pub(super) const fn complete(self) -> bool {
        matches!(self.phase, ExactSourceProbePhaseV1::Complete)
    }

    pub(super) const fn manifest_blob(self) -> Option<CanonicalDigest> {
        self.manifest_blob
    }

    pub(super) const fn manifest(self) -> Option<ExactSourceProbeManifestV1> {
        self.manifest
    }

    pub(super) const fn evaluated_candidate_count(self) -> u128 {
        self.evaluated_candidate_count
    }

    pub(super) const fn remaining_candidate_count(self) -> u128 {
        self.remaining_candidate_count
    }
}

pub(super) fn encode_source_probe_manifest_v1(
    manifest: ExactSourceProbeManifestV1,
) -> Result<Vec<u8>, ExactSourceProbeManifestError> {
    let mut bytes = Vec::with_capacity(SOURCE_PROBE_MANIFEST_MAX_BYTES_V1);
    bytes.extend_from_slice(SOURCE_PROBE_MANIFEST_MAGIC_V1);
    bytes.extend_from_slice(&SOURCE_PROBE_MANIFEST_VERSION_V1.to_le_bytes());
    bytes.push(match manifest.mode {
        ExactSourceProbeModeV1::CheckedSourceProof => 0,
        ExactSourceProbeModeV1::CanonicalFallback => 1,
    });
    bytes.extend_from_slice(&manifest.proof_set_id.bytes());
    bytes.extend_from_slice(&manifest.candidate_blob.bytes());
    bytes.extend_from_slice(&manifest.candidate_count.to_le_bytes());
    encode_optional_digest(&mut bytes, manifest.closed_region_blob);
    encode_coverage(&mut bytes, manifest.coverage);
    if bytes.len() > SOURCE_PROBE_MANIFEST_MAX_BYTES_V1 {
        return Err(ExactSourceProbeManifestError::invalid(
            "canonical source-probe manifest exceeds its byte bound",
        ));
    }
    Ok(bytes)
}

pub(super) fn decode_source_probe_manifest_v1(
    bytes: &[u8],
) -> Result<ExactSourceProbeManifestV1, ExactSourceProbeManifestError> {
    if bytes.len() > SOURCE_PROBE_MANIFEST_MAX_BYTES_V1 {
        return Err(ExactSourceProbeManifestError::invalid(
            "source-probe manifest exceeds its byte bound",
        ));
    }
    let mut decoder = ManifestDecoder::new(bytes);
    if decoder.take(8)? != SOURCE_PROBE_MANIFEST_MAGIC_V1 {
        return Err(ExactSourceProbeManifestError::invalid(
            "source-probe manifest has invalid magic",
        ));
    }
    let version = decoder.u16()?;
    if version != SOURCE_PROBE_MANIFEST_VERSION_V1 {
        return Err(ExactSourceProbeManifestError::invalid(format!(
            "unsupported source-probe manifest version {version}",
        )));
    }
    let mode = match decoder.u8()? {
        0 => ExactSourceProbeModeV1::CheckedSourceProof,
        1 => ExactSourceProbeModeV1::CanonicalFallback,
        value => {
            return Err(ExactSourceProbeManifestError::invalid(format!(
                "invalid source-probe mode tag {value}",
            )))
        }
    };
    let proof_set_id = decoder.digest()?;
    let candidate_blob = decoder.digest()?;
    let candidate_count = decoder.u128()?;
    let closed_region_blob = decoder.optional_digest()?;
    let coverage = decoder.coverage()?;
    if !decoder.is_finished() {
        return Err(ExactSourceProbeManifestError::invalid(
            "source-probe manifest has trailing bytes",
        ));
    }
    ExactSourceProbeManifestV1::validated(
        mode,
        proof_set_id,
        candidate_blob,
        candidate_count,
        closed_region_blob,
        coverage,
    )
}

fn encode_optional_digest(bytes: &mut Vec<u8>, digest: Option<CanonicalDigest>) {
    match digest {
        None => bytes.push(0),
        Some(digest) => {
            bytes.push(1);
            bytes.extend_from_slice(&digest.bytes());
        }
    }
}

fn encode_coverage(bytes: &mut Vec<u8>, coverage: ExactSourceProbeCoverageSummaryV1) {
    bytes.extend_from_slice(&coverage.universe_case_count.to_le_bytes());
    bytes.extend_from_slice(&coverage.certified_closed_case_count.to_le_bytes());
    bytes.extend_from_slice(&coverage.residual_open_case_count.to_le_bytes());
    match coverage.boundary_rank_stride {
        None => bytes.push(0),
        Some(boundary_rank_stride) => {
            bytes.push(1);
            bytes.extend_from_slice(&boundary_rank_stride.to_le_bytes());
            for count in [
                coverage.sealed_proof_nonmatch_cases,
                coverage.open_proof_nonmatch_cases,
                coverage.open_proof_match_cases,
                coverage.sealed_structural_excluded_cases,
                coverage.open_structural_excluded_cases,
            ] {
                bytes.extend_from_slice(
                    &count
                        .expect("available source summary has every count")
                        .to_le_bytes(),
                );
            }
            for count in [
                coverage.total_outer_profiles,
                coverage.analyzed_outer_profiles,
                coverage.proof_incomplete_profiles,
            ] {
                bytes.extend_from_slice(
                    &count
                        .expect("available source summary has outer-profile accounting")
                        .to_le_bytes(),
                );
            }
            bytes.push(u8::from(
                coverage
                    .profile_limit_reached
                    .expect("available source summary has profile cap status"),
            ));
            bytes.push(u8::from(
                coverage
                    .region_limit_reached
                    .expect("available source summary has its region cap status"),
            ));
            bytes.push(u8::from(
                coverage
                    .candidate_limit_reached
                    .expect("available source summary has its candidate cap status"),
            ));
        }
    }
}

struct ManifestDecoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ManifestDecoder<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], ExactSourceProbeManifestError> {
        let end = self.offset.checked_add(len).ok_or_else(|| {
            ExactSourceProbeManifestError::invalid("source-probe manifest offset overflow")
        })?;
        let value = self.bytes.get(self.offset..end).ok_or_else(|| {
            ExactSourceProbeManifestError::invalid("truncated source-probe manifest")
        })?;
        self.offset = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, ExactSourceProbeManifestError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, ExactSourceProbeManifestError> {
        let mut bytes = [0_u8; 2];
        bytes.copy_from_slice(self.take(2)?);
        Ok(u16::from_le_bytes(bytes))
    }

    fn u128(&mut self) -> Result<u128, ExactSourceProbeManifestError> {
        let mut bytes = [0_u8; 16];
        bytes.copy_from_slice(self.take(16)?);
        Ok(u128::from_le_bytes(bytes))
    }

    fn digest(&mut self) -> Result<CanonicalDigest, ExactSourceProbeManifestError> {
        let mut bytes = [0_u8; 32];
        bytes.copy_from_slice(self.take(32)?);
        Ok(CanonicalDigest::from_sha256_bytes(bytes))
    }

    fn optional_digest(
        &mut self,
    ) -> Result<Option<CanonicalDigest>, ExactSourceProbeManifestError> {
        match self.u8()? {
            0 => Ok(None),
            1 => self.digest().map(Some),
            value => Err(ExactSourceProbeManifestError::invalid(format!(
                "invalid optional source-probe digest tag {value}",
            ))),
        }
    }

    fn coverage(
        &mut self,
    ) -> Result<ExactSourceProbeCoverageSummaryV1, ExactSourceProbeManifestError> {
        let universe_case_count = self.u128()?;
        let certified_closed_case_count = self.u128()?;
        let residual_open_case_count = self.u128()?;
        let source_summary = self.u8()?;
        let mut coverage = ExactSourceProbeCoverageSummaryV1 {
            universe_case_count,
            certified_closed_case_count,
            residual_open_case_count,
            boundary_rank_stride: None,
            sealed_proof_nonmatch_cases: None,
            open_proof_nonmatch_cases: None,
            open_proof_match_cases: None,
            sealed_structural_excluded_cases: None,
            open_structural_excluded_cases: None,
            total_outer_profiles: None,
            analyzed_outer_profiles: None,
            proof_incomplete_profiles: None,
            profile_limit_reached: None,
            region_limit_reached: None,
            candidate_limit_reached: None,
        };
        match source_summary {
            0 => {}
            1 => {
                coverage.boundary_rank_stride = Some(self.u128()?);
                coverage.sealed_proof_nonmatch_cases = Some(self.u128()?);
                coverage.open_proof_nonmatch_cases = Some(self.u128()?);
                coverage.open_proof_match_cases = Some(self.u128()?);
                coverage.sealed_structural_excluded_cases = Some(self.u128()?);
                coverage.open_structural_excluded_cases = Some(self.u128()?);
                coverage.total_outer_profiles = Some(self.u128()?);
                coverage.analyzed_outer_profiles = Some(self.u128()?);
                coverage.proof_incomplete_profiles = Some(self.u128()?);
                coverage.profile_limit_reached = Some(self.bool()?);
                coverage.region_limit_reached = Some(self.bool()?);
                coverage.candidate_limit_reached = Some(self.bool()?);
            }
            value => {
                return Err(ExactSourceProbeManifestError::invalid(format!(
                    "invalid source-probe summary tag {value}",
                )))
            }
        }
        Ok(coverage)
    }

    fn bool(&mut self) -> Result<bool, ExactSourceProbeManifestError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(ExactSourceProbeManifestError::invalid(format!(
                "invalid source-probe boolean {value}",
            ))),
        }
    }

    fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct ExactSourceProbeManifestError(Box<str>);

impl ExactSourceProbeManifestError {
    fn invalid(message: impl Into<String>) -> Self {
        Self(message.into().into_boxed_str())
    }
}

impl fmt::Display for ExactSourceProbeManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for ExactSourceProbeManifestError {}
