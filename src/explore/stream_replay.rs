//! Canonical validation manifest for final representative/witness replay.
//!
//! The manifest is journal provenance, not normalized answer identity. It may
//! contain deterministic evaluator receipt digests, while the semantic fact
//! that closes the replay obligation is derived separately from the canonical
//! answer/witness state and therefore excludes those receipts.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::exact_stream::{
    decode_exact_case_observation_v1, encode_exact_case_observation_v1,
    ExactCaseObservationProposalV1, ExactClosedClassificationV1, ExactEvidenceReducer,
    ExactValidationReceiptDigestV1,
};
use super::run_stream::CanonicalDigest;

pub(crate) const EXACT_REPLAY_CLOSURE_BLOB_KIND_V1: &str = "exact-replay-closure-v1";

const REPLAY_MANIFEST_MAGIC_V1: &[u8; 8] = b"FXRPL001";
const MAX_REPLAY_OBSERVATIONS_V1: usize = 65_536;
const MAX_REPLAY_MANIFEST_BYTES_V1: usize = 64 * 1024 * 1024;
const REPLAY_WITNESS_DIGEST_V1: &[u8] = b"futuruna.explore.replay-witness-set.v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExactReplayClosureManifestV1 {
    observations: Box<[ExactCaseObservationProposalV1]>,
}

impl ExactReplayClosureManifestV1 {
    pub(crate) fn new(
        observations: impl Into<Box<[ExactCaseObservationProposalV1]>>,
    ) -> Result<Self, ExactReplayManifestError> {
        let mut observations = observations.into().into_vec();
        if observations.len() > MAX_REPLAY_OBSERVATIONS_V1 {
            return Err(ExactReplayManifestError::invalid(format!(
                "replay manifest has {} observations; limit is {MAX_REPLAY_OBSERVATIONS_V1}",
                observations.len()
            )));
        }
        observations.sort_by_key(|observation| observation.case_id.rank);
        if observations
            .windows(2)
            .any(|pair| pair[0].case_id.rank >= pair[1].case_id.rank)
        {
            return Err(ExactReplayManifestError::invalid(
                "replay manifest CaseId ranks are not unique",
            ));
        }
        // Round-trip every proposal through its strict record codec at the
        // boundary, including the empty-manifest case where this loop is inert.
        for observation in &observations {
            let bytes = encode_exact_case_observation_v1(observation)
                .map_err(|error| ExactReplayManifestError::invalid(error.to_string()))?;
            let decoded = decode_exact_case_observation_v1(&bytes)
                .map_err(|error| ExactReplayManifestError::invalid(error.to_string()))?;
            if &decoded != observation {
                return Err(ExactReplayManifestError::invalid(
                    "replay observation failed canonical round-trip",
                ));
            }
        }
        Ok(Self {
            observations: observations.into_boxed_slice(),
        })
    }

    pub(crate) fn observations(&self) -> &[ExactCaseObservationProposalV1] {
        &self.observations
    }
}

/// Result of checking a fresh replay manifest against the closed exact
/// aggregate.  The digest excludes evaluator receipts and is therefore safe
/// to use as normalized semantic evidence for the replay obligation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ExactReplayClosureValidationV1 {
    normalized_witness_digest: CanonicalDigest,
}

impl ExactReplayClosureValidationV1 {
    pub(crate) const fn normalized_witness_digest(self) -> CanonicalDigest {
        self.normalized_witness_digest
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExactReplayManifestError(Box<str>);

impl ExactReplayManifestError {
    fn invalid(message: impl Into<String>) -> Self {
        Self(message.into().into_boxed_str())
    }
}

impl fmt::Display for ExactReplayManifestError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

impl Error for ExactReplayManifestError {}

/// Verify that a freshly evaluated manifest contains exactly the selected
/// representative and extrema witness CaseIds, and that every replayed value
/// agrees with the closed reducer aggregate.
///
/// The manifest may be empty only when the closed result has no matching
/// groups.  Extra replayed cases are rejected: provenance volume must not
/// silently change normalized answer identity.
pub(crate) fn validate_exact_replay_closure_v1(
    reducer: &ExactEvidenceReducer,
    manifest: &ExactReplayClosureManifestV1,
) -> Result<ExactReplayClosureValidationV1, ExactReplayManifestError> {
    let snapshot = reducer.snapshot();
    if snapshot.open_case_count != 0 || !snapshot.projection_complete {
        return Err(ExactReplayManifestError::invalid(
            "representative/extrema replay requires closed classification and projection",
        ));
    }

    let observations = manifest
        .observations
        .iter()
        .map(|observation| (observation.case_id.rank, observation))
        .collect::<BTreeMap<_, _>>();
    let expected_ranks = exact_replay_witness_ranks_v1(reducer)?
        .into_vec()
        .into_iter()
        .collect::<BTreeSet<_>>();
    if observations.keys().copied().collect::<BTreeSet<_>>() != expected_ranks {
        return Err(ExactReplayManifestError::invalid(
            "replay manifest is not the exact selected representative/extrema witness set",
        ));
    }

    for observation in manifest.observations.iter() {
        let expected_case_id = reducer
            .canonical_case_id_at_rank(observation.case_id.rank)
            .map_err(|error| ExactReplayManifestError::invalid(error.to_string()))?;
        if observation.case_id != expected_case_id {
            return Err(ExactReplayManifestError::invalid(format!(
                "replay CaseId rank {} has noncanonical ordinals",
                observation.case_id.rank
            )));
        }
        if observation.classification != ExactClosedClassificationV1::AdmissibleMatch
            || observation.match_projection.is_none()
        {
            return Err(ExactReplayManifestError::invalid(format!(
                "selected replay rank {} is not a complete matching observation",
                observation.case_id.rank
            )));
        }
    }

    for result in snapshot.results.iter() {
        let representative = observations
            .get(&result.representative_case_id.rank)
            .expect("exact selected rank set was checked above");
        let projection = representative
            .match_projection
            .as_ref()
            .expect("selected replay observations were checked as matching");
        if projection.key.as_ref() != result.key.as_ref()
            || projection.shown.as_ref() != result.representative_shown.as_ref()
            || projection.representative_objective != result.representative_objective
        {
            return Err(ExactReplayManifestError::invalid(format!(
                "representative replay at rank {} disagrees with the closed result group",
                representative.case_id.rank
            )));
        }

        for (index, extrema) in result.extrema.iter().enumerate() {
            for (kind, rank, expected_value) in [
                ("minimum", extrema.minimum_witness.rank, extrema.minimum),
                ("maximum", extrema.maximum_witness.rank, extrema.maximum),
            ] {
                let witness = observations
                    .get(&rank)
                    .expect("exact selected rank set was checked above");
                let witness_projection = witness
                    .match_projection
                    .as_ref()
                    .expect("selected replay observations were checked as matching");
                if witness_projection.key.as_ref() != result.key.as_ref()
                    || witness_projection.extrema.get(index).copied() != Some(expected_value)
                {
                    return Err(ExactReplayManifestError::invalid(format!(
                        "{kind} witness replay at rank {rank} disagrees with extrema field {index}"
                    )));
                }
            }
        }
    }

    let mut hasher = Sha256::new();
    hasher.update(REPLAY_WITNESS_DIGEST_V1);
    hasher.update((manifest.observations.len() as u128).to_le_bytes());
    for observation in manifest.observations.iter() {
        let mut normalized = observation.clone();
        normalized.validation_receipt_digest = ExactValidationReceiptDigestV1::new([0; 32]);
        let bytes = encode_exact_case_observation_v1(&normalized)
            .map_err(|error| ExactReplayManifestError::invalid(error.to_string()))?;
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    Ok(ExactReplayClosureValidationV1 {
        normalized_witness_digest: CanonicalDigest::from_sha256_bytes(hasher.finalize().into()),
    })
}

/// Canonical fresh-evaluation schedule for closing the replay obligation.
/// Duplicate roles collapse to one CaseId and ranks are returned in ascending
/// order, making retries and manifests deterministic.
pub(crate) fn exact_replay_witness_ranks_v1(
    reducer: &ExactEvidenceReducer,
) -> Result<Box<[u128]>, ExactReplayManifestError> {
    let snapshot = reducer.snapshot();
    if snapshot.open_case_count != 0 || !snapshot.projection_complete {
        return Err(ExactReplayManifestError::invalid(
            "replay witness scheduling requires closed classification and projection",
        ));
    }
    let mut ranks = BTreeSet::new();
    for result in snapshot.results.iter() {
        ranks.insert(result.representative_case_id.rank);
        for extrema in result.extrema.iter() {
            ranks.insert(extrema.minimum_witness.rank);
            ranks.insert(extrema.maximum_witness.rank);
        }
    }
    if ranks.len() > MAX_REPLAY_OBSERVATIONS_V1 {
        return Err(ExactReplayManifestError::invalid(format!(
            "selected replay witness set has {} CaseIds; atomic v1 limit is {MAX_REPLAY_OBSERVATIONS_V1}",
            ranks.len()
        )));
    }
    Ok(ranks.into_iter().collect::<Vec<_>>().into_boxed_slice())
}

pub(crate) fn encode_exact_replay_closure_manifest_v1(
    manifest: &ExactReplayClosureManifestV1,
) -> Result<Vec<u8>, ExactReplayManifestError> {
    let count = u32::try_from(manifest.observations.len())
        .map_err(|_| ExactReplayManifestError::invalid("replay observation count exceeds u32"))?;
    let mut encoded_observations = Vec::with_capacity(manifest.observations.len());
    let mut encoded_len = REPLAY_MANIFEST_MAGIC_V1
        .len()
        .checked_add(4)
        .ok_or_else(|| ExactReplayManifestError::invalid("replay manifest length overflow"))?;
    for observation in manifest.observations.iter() {
        let bytes = encode_exact_case_observation_v1(observation)
            .map_err(|error| ExactReplayManifestError::invalid(error.to_string()))?;
        let _ = u32::try_from(bytes.len()).map_err(|_| {
            ExactReplayManifestError::invalid("one replay observation exceeds u32 bytes")
        })?;
        encoded_len = encoded_len
            .checked_add(4)
            .and_then(|length| length.checked_add(bytes.len()))
            .ok_or_else(|| ExactReplayManifestError::invalid("replay manifest length overflow"))?;
        if encoded_len > MAX_REPLAY_MANIFEST_BYTES_V1 {
            return Err(ExactReplayManifestError::invalid(format!(
                "replay manifest exceeds {MAX_REPLAY_MANIFEST_BYTES_V1} bytes"
            )));
        }
        encoded_observations.push(bytes);
    }

    let mut output = Vec::with_capacity(encoded_len);
    output.extend_from_slice(REPLAY_MANIFEST_MAGIC_V1);
    output.extend_from_slice(&count.to_le_bytes());
    for observation in encoded_observations {
        output.extend_from_slice(&(observation.len() as u32).to_le_bytes());
        output.extend_from_slice(&observation);
    }
    Ok(output)
}

pub(crate) fn decode_exact_replay_closure_manifest_v1(
    bytes: &[u8],
) -> Result<ExactReplayClosureManifestV1, ExactReplayManifestError> {
    if bytes.len() > MAX_REPLAY_MANIFEST_BYTES_V1 {
        return Err(ExactReplayManifestError::invalid(format!(
            "replay manifest has {} bytes; limit is {MAX_REPLAY_MANIFEST_BYTES_V1}",
            bytes.len()
        )));
    }
    if bytes.len() < REPLAY_MANIFEST_MAGIC_V1.len() + 4
        || &bytes[..REPLAY_MANIFEST_MAGIC_V1.len()] != REPLAY_MANIFEST_MAGIC_V1
    {
        return Err(ExactReplayManifestError::invalid(
            "replay manifest has invalid v1 magic or a truncated header",
        ));
    }
    let mut cursor = REPLAY_MANIFEST_MAGIC_V1.len();
    let count = read_u32(bytes, &mut cursor, "replay observation count")? as usize;
    if count > MAX_REPLAY_OBSERVATIONS_V1 {
        return Err(ExactReplayManifestError::invalid(format!(
            "replay manifest count {count} exceeds {MAX_REPLAY_OBSERVATIONS_V1}"
        )));
    }
    // Every entry needs at least its four-byte length, so reject hostile counts
    // before reserving the observation vector.
    if count > bytes.len().saturating_sub(cursor) / 4 {
        return Err(ExactReplayManifestError::invalid(
            "replay manifest count exceeds its remaining bytes",
        ));
    }
    let mut observations = Vec::with_capacity(count);
    for _ in 0..count {
        let length = read_u32(bytes, &mut cursor, "replay observation length")? as usize;
        let end = cursor.checked_add(length).ok_or_else(|| {
            ExactReplayManifestError::invalid("replay observation offset overflow")
        })?;
        let body = bytes.get(cursor..end).ok_or_else(|| {
            ExactReplayManifestError::invalid("truncated replay observation body")
        })?;
        let observation = decode_exact_case_observation_v1(body)
            .map_err(|error| ExactReplayManifestError::invalid(error.to_string()))?;
        observations.push(observation);
        cursor = end;
    }
    if cursor != bytes.len() {
        return Err(ExactReplayManifestError::invalid(
            "replay manifest contains trailing bytes",
        ));
    }
    let manifest = ExactReplayClosureManifestV1::new(observations.into_boxed_slice())?;
    let canonical = encode_exact_replay_closure_manifest_v1(&manifest)?;
    if canonical.as_slice() != bytes {
        return Err(ExactReplayManifestError::invalid(
            "replay manifest bytes are not canonical",
        ));
    }
    Ok(manifest)
}

fn read_u32(
    bytes: &[u8],
    cursor: &mut usize,
    field: &'static str,
) -> Result<u32, ExactReplayManifestError> {
    let end = cursor
        .checked_add(4)
        .ok_or_else(|| ExactReplayManifestError::invalid(format!("{field} offset overflow")))?;
    let body = bytes
        .get(*cursor..end)
        .ok_or_else(|| ExactReplayManifestError::invalid(format!("truncated {field}")))?;
    *cursor = end;
    Ok(u32::from_le_bytes(
        body.try_into()
            .expect("u32 body has an exactly checked length"),
    ))
}
