//! Fail-closed process boundary for a query-bound native classifier.
//!
//! V2 classifies one ordered batch from the checked source enumerator's finite
//! integer values and bounded canonical finite-type ordinals. Derived
//! singleton bindings, structured finite values, composite Context/Before
//! values and the singleton successor are reconstructed inside the query-bound
//! executable. These inputs are operational only: the host retains every
//! semantic value and remains the sole producer of IDs and journal evidence.
//! The executable is valid only for the exact checked
//! program/relation/admission/question header carried in both directions. The
//! host never accepts a prefix: every response must decode completely before
//! any outcome is returned.
//!
//! Canonical request framing (all integers big-endian):
//! `request-magic | version | 4 * digest | factors:u32 | count:u32 |
//! count * factors * scalar:i64`. A scalar is either an integer factor value
//! or a producer-issued finite-type ordinal, according to the query-bound
//! executable plan.
//! The response mirrors that header and ends with exactly `count` outcome tags.
//! Digest order is program, relation, admission, then question.
//! Outcome tags are `1 = rejected`, `2 = admitted/not-selected`, and
//! `3 = admitted/selected`.

use std::error::Error;
use std::fmt;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};
use std::{cmp, thread};

use crate::{CheckedExploreQueryView, CheckedExploreSourceProjectionFactorKind, Ty};

use super::relational_bounded_chunk_partition::RELATIONAL_CASE_CHUNK_MAX_COORDINATES_V1;
use super::relational_classified_sweep::{
    RelationalCheckedClassificationContext, RelationalClassifiedCaseOutcome,
    RelationalClassifiedSweepError, RelationalOrderedClassificationBackend,
    RelationalOrderedClassificationSubject,
};
use super::relational_executor::RelationalExpressionRuntime;
use super::stream_resource::ExactStreamOneWorkerEnvelope;
use super::{
    relational_tys_equivalent, ExploreExactDomain, ExploreFiniteDomainIr,
    ExploreSourceBindingKindIr, ExploreSuccessorKindIr, ExploreValue,
};

/// Frozen wire constants shared with a generated V2 sidecar executable.
///
/// Identity digests are ordered program, relation, admission, question.
/// Counts and signed inputs are big-endian. No field is length-prefixed except
/// the subject count; request EOF and exact response length close each frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelationalNativeClassifierProtocolV2;

impl RelationalNativeClassifierProtocolV2 {
    pub const VERSION: u32 = 2;
    pub const REQUEST_MAGIC: &'static [u8] = b"futuruna.explore.native-classifier.request.v2\0";
    pub const RESPONSE_MAGIC: &'static [u8] = b"futuruna.explore.native-classifier.response.v2\0";
    pub const IDENTITY_DIGEST_BYTES: usize = 32;
    pub const IDENTITY_DIGEST_COUNT: usize = 4;
    pub const COUNT_BYTES: usize = 4;
    pub const FACTOR_COUNT_BYTES: usize = 4;
    pub const FACTOR_INT_BYTES: usize = 8;
    pub const OUTCOME_BYTES: usize = 1;
    pub const MAX_FACTORS_PER_SUBJECT: usize = 32;
    pub const MAX_BATCH_SUBJECTS: usize = RELATIONAL_CASE_CHUNK_MAX_COORDINATES_V1 as usize;
    pub const OUTCOME_REJECTED: u8 = 0x01;
    pub const OUTCOME_ADMITTED_NOT_SELECTED: u8 = 0x02;
    pub const OUTCOME_ADMITTED_SELECTED: u8 = 0x03;
}

const REQUEST_MAGIC_V2: &[u8] = RelationalNativeClassifierProtocolV2::REQUEST_MAGIC;
const RESPONSE_MAGIC_V2: &[u8] = RelationalNativeClassifierProtocolV2::RESPONSE_MAGIC;
const DIGEST_BYTES: usize = RelationalNativeClassifierProtocolV2::IDENTITY_DIGEST_BYTES;
const U32_BYTES: usize = RelationalNativeClassifierProtocolV2::COUNT_BYTES;
const I64_BYTES: usize = RelationalNativeClassifierProtocolV2::FACTOR_INT_BYTES;
const RELATIONAL_NATIVE_CLASSIFIER_MAX_BATCH_SUBJECTS_V2: usize =
    RelationalNativeClassifierProtocolV2::MAX_BATCH_SUBJECTS;
const INVOCATION_TIMEOUT_V2: Duration = Duration::from_secs(30);
const MIN_PARALLEL_BATCH_SUBJECTS_V2: usize = 64;
const REQUEST_HEADER_BYTES_V2: usize = REQUEST_MAGIC_V2.len()
    + U32_BYTES
    + RelationalNativeClassifierProtocolV2::IDENTITY_DIGEST_COUNT * DIGEST_BYTES
    + RelationalNativeClassifierProtocolV2::FACTOR_COUNT_BYTES
    + U32_BYTES;
const MAX_REQUEST_BYTES_V2: usize = REQUEST_HEADER_BYTES_V2
    + RELATIONAL_NATIVE_CLASSIFIER_MAX_BATCH_SUBJECTS_V2
        * RelationalNativeClassifierProtocolV2::MAX_FACTORS_PER_SUBJECT
        * I64_BYTES;
const MAX_RESPONSE_BYTES_V2: usize = RESPONSE_MAGIC_V2.len()
    + U32_BYTES
    + RelationalNativeClassifierProtocolV2::IDENTITY_DIGEST_COUNT * DIGEST_BYTES
    + U32_BYTES
    + RELATIONAL_NATIVE_CLASSIFIER_MAX_BATCH_SUBJECTS_V2
        * RelationalNativeClassifierProtocolV2::OUTCOME_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RelationalNativeClassifierIdentityV2 {
    program_hash: [u8; DIGEST_BYTES],
    relation_id: [u8; DIGEST_BYTES],
    admission_id: [u8; DIGEST_BYTES],
    question_id: [u8; DIGEST_BYTES],
}

impl RelationalNativeClassifierIdentityV2 {
    fn from_checked(
        checked: &CheckedExploreQueryView<'_>,
    ) -> Result<Self, RelationalNativeClassifierUnavailable> {
        let [question_id] = checked.find_question_ids() else {
            return Err(RelationalNativeClassifierUnavailable::UnsupportedQuestionSet);
        };
        Ok(Self {
            program_hash: decode_lowercase_sha256(checked.program_hash())
                .ok_or(RelationalNativeClassifierUnavailable::InvalidCheckedProgramHash)?,
            relation_id: checked.relation_id().bytes(),
            admission_id: checked.admission_id().bytes(),
            question_id: question_id.bytes(),
        })
    }

    fn encode_into(self, output: &mut Vec<u8>) {
        output.extend_from_slice(&self.program_hash);
        output.extend_from_slice(&self.relation_id);
        output.extend_from_slice(&self.admission_id);
        output.extend_from_slice(&self.question_id);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RelationalNativeClassifierFiniteInputKindV2 {
    IntValue,
    ExactFiniteOrdinal {
        exact_cardinality: u128,
        plan_digest: [u8; 32],
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RelationalNativeClassifierFiniteInputV2 {
    binding_index: usize,
    kind: RelationalNativeClassifierFiniteInputKindV2,
}

fn finite_inputs_from_checked(
    checked: &CheckedExploreQueryView<'_>,
) -> Result<Box<[RelationalNativeClassifierFiniteInputV2]>, RelationalNativeClassifierUnavailable> {
    checked
        .closed_query
        .validate()
        .map_err(|_| RelationalNativeClassifierUnavailable::InvalidFiniteInputShape)?;
    if !matches!(
        &checked.closed_query.successor.kind,
        ExploreSuccessorKindIr::Singleton { .. }
    ) {
        return Err(RelationalNativeClassifierUnavailable::InvalidFiniteInputShape);
    }
    let source_projection = checked.source_image_projection();
    let mut inputs = Vec::new();
    let mut structured_factor_count = 0usize;
    for (position, binding) in checked.closed_query.source.bindings.iter().enumerate() {
        if binding.binding_index != position {
            return Err(RelationalNativeClassifierUnavailable::InvalidFiniteInputShape);
        }
        if let ExploreSourceBindingKindIr::Finite { domain } = &binding.kind {
            let kind = match domain {
                ExploreFiniteDomainIr::IntRange { .. }
                    if binding.dependencies.is_empty()
                        && matches!(&binding.value_ty, Ty::Name(name) if matches!(name.as_str(), "Int" | "Heltal")) =>
                {
                    RelationalNativeClassifierFiniteInputKindV2::IntValue
                }
                ExploreFiniteDomainIr::Exact(ExploreExactDomain::FiniteType { ty, plan })
                    if binding.dependencies.is_empty()
                        && relational_tys_equivalent(ty, &binding.value_ty)
                        && structured_factor_count == 0 =>
                {
                    let exact_cardinality = plan
                        .cardinality()
                        .exact()
                        .filter(|count| *count > 0 && *count <= i64::MAX as u128)
                        .ok_or(RelationalNativeClassifierUnavailable::InvalidFiniteInputShape)?;
                    let plan_digest = crate::checked_explore_finite_plan_digest(plan);
                    let projection_factor = source_projection
                        .ok_or(RelationalNativeClassifierUnavailable::InvalidFiniteInputShape)?
                        .factors
                        .iter()
                        .find(|factor| usize::try_from(factor.binding_index).ok() == Some(position))
                        .ok_or(RelationalNativeClassifierUnavailable::InvalidFiniteInputShape)?;
                    if projection_factor.exact_cardinality != exact_cardinality
                        || !matches!(
                            projection_factor.kind,
                            CheckedExploreSourceProjectionFactorKind::ExactFinite {
                                plan_digest: certified_plan_digest,
                            } if certified_plan_digest == plan_digest
                        )
                    {
                        return Err(RelationalNativeClassifierUnavailable::InvalidFiniteInputShape);
                    }
                    structured_factor_count += 1;
                    RelationalNativeClassifierFiniteInputKindV2::ExactFiniteOrdinal {
                        exact_cardinality,
                        plan_digest,
                    }
                }
                _ => {
                    return Err(RelationalNativeClassifierUnavailable::InvalidFiniteInputShape);
                }
            };
            inputs.push(RelationalNativeClassifierFiniteInputV2 {
                binding_index: position,
                kind,
            });
        }
    }
    if inputs.is_empty()
        || inputs.len() > RelationalNativeClassifierProtocolV2::MAX_FACTORS_PER_SUBJECT
    {
        return Err(RelationalNativeClassifierUnavailable::InvalidFiniteInputShape);
    }
    Ok(inputs.into_boxed_slice())
}

/// One query-bound executable speaking the strict native-classifier V2 frame.
#[derive(Clone, Debug)]
pub(crate) struct RelationalNativeClassifierV2 {
    executable: PathBuf,
    identity: RelationalNativeClassifierIdentityV2,
    finite_inputs: Arc<[RelationalNativeClassifierFiniteInputV2]>,
    enabled: Arc<AtomicBool>,
    parity_checked: Arc<AtomicBool>,
    native_workers: u16,
}

impl RelationalNativeClassifierV2 {
    pub(crate) fn for_checked_query(
        executable: impl Into<PathBuf>,
        checked: &CheckedExploreQueryView<'_>,
    ) -> Result<Self, RelationalNativeClassifierUnavailable> {
        let finite_inputs = finite_inputs_from_checked(checked)?;
        Ok(Self {
            executable: executable.into(),
            identity: RelationalNativeClassifierIdentityV2::from_checked(checked)?,
            finite_inputs: finite_inputs.into(),
            enabled: Arc::new(AtomicBool::new(true)),
            parity_checked: Arc::new(AtomicBool::new(false)),
            native_workers: 1,
        })
    }

    /// Called only while opening an epoch, with that epoch's frozen resource
    /// envelope. Clones retain this limit; classification never reads the
    /// environment or expands its own CPU/memory reservation.
    pub(super) fn configure_for_resource_envelope(
        &mut self,
        resources: &ExactStreamOneWorkerEnvelope,
    ) {
        self.native_workers = resources.native_classifier_worker_limit();
    }

    pub(crate) fn executable(&self) -> &Path {
        &self.executable
    }

    /// Whether this process-local accelerator may still be attempted.
    ///
    /// Unavailability is sticky across clones so callers can route later
    /// batches to another exact backend without paying one checked native
    /// fallback per batch. This flag is operational only and carries no
    /// semantic or evidence authority.
    pub(crate) fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }

    pub(crate) fn classify_ordered_batch(
        &self,
        subjects: &[RelationalOrderedClassificationSubject<'_>],
    ) -> Result<Box<[RelationalClassifiedCaseOutcome]>, RelationalNativeClassifierUnavailable> {
        if !self.is_enabled() {
            return Err(RelationalNativeClassifierUnavailable::DisabledAfterUnavailable);
        }
        let request = encode_request(self.identity, &self.finite_inputs, subjects)?;
        classify_encoded_batch(self.identity, request, self.native_workers, &|request| {
            invoke_once(&self.executable, request)
        })
    }

    /// Execute native classification and atomically fall back to a checked
    /// whole batch. The first successful native batch is compared with that
    /// checked batch before any native outcome can be accepted.
    pub(crate) fn classify_or_fallback<E>(
        &self,
        subjects: &[RelationalOrderedClassificationSubject<'_>],
        fallback: impl FnOnce() -> Result<Box<[RelationalClassifiedCaseOutcome]>, E>,
    ) -> Result<
        (
            Box<[RelationalClassifiedCaseOutcome]>,
            Option<RelationalNativeClassifierUnavailable>,
        ),
        E,
    > {
        self.finish_or_fallback(self.classify_ordered_batch(subjects), fallback)
    }

    fn finish_or_fallback<E>(
        &self,
        native_result: Result<
            Box<[RelationalClassifiedCaseOutcome]>,
            RelationalNativeClassifierUnavailable,
        >,
        fallback: impl FnOnce() -> Result<Box<[RelationalClassifiedCaseOutcome]>, E>,
    ) -> Result<
        (
            Box<[RelationalClassifiedCaseOutcome]>,
            Option<RelationalNativeClassifierUnavailable>,
        ),
        E,
    > {
        match native_result {
            Ok(outcomes) if self.parity_checked() => Ok((outcomes, None)),
            Ok(outcomes) => {
                let checked_outcomes = fallback()?;
                if checked_outcomes != outcomes {
                    let unavailable = RelationalNativeClassifierUnavailable::ParityCanaryMismatch;
                    if self.disable() {
                        trace_native_classifier_unavailable(&unavailable);
                    }
                    return Ok((checked_outcomes, Some(unavailable)));
                }
                self.mark_parity_checked();
                Ok((outcomes, None))
            }
            Err(unavailable) => {
                if self.disable() {
                    trace_native_classifier_unavailable(&unavailable);
                }
                Ok((fallback()?, Some(unavailable)))
            }
        }
    }

    fn disable(&self) -> bool {
        self.enabled.swap(false, Ordering::AcqRel)
    }

    fn parity_checked(&self) -> bool {
        self.parity_checked.load(Ordering::Acquire)
    }

    fn mark_parity_checked(&self) {
        self.parity_checked.store(true, Ordering::Release);
    }
}

/// Native first, with one atomic interpreter fallback for any unavailable V2
/// execution. A malformed response can never contribute a prefix of outcomes.
#[derive(Clone, Debug)]
pub(crate) struct RelationalNativeClassifierFallbackBackendV2 {
    native: RelationalNativeClassifierV2,
    last_unavailable: Option<RelationalNativeClassifierUnavailable>,
}

impl RelationalNativeClassifierFallbackBackendV2 {
    pub(crate) fn new(native: RelationalNativeClassifierV2) -> Self {
        Self {
            native,
            last_unavailable: None,
        }
    }

    pub(crate) fn last_unavailable(&self) -> Option<&RelationalNativeClassifierUnavailable> {
        self.last_unavailable.as_ref()
    }
}

impl RelationalOrderedClassificationBackend for RelationalNativeClassifierFallbackBackendV2 {
    fn classify_ordered_batch<R: RelationalExpressionRuntime>(
        &mut self,
        subjects: &[RelationalOrderedClassificationSubject<'_>],
        checked: &mut RelationalCheckedClassificationContext<'_, '_, '_, R>,
    ) -> Result<Box<[RelationalClassifiedCaseOutcome]>, RelationalClassifiedSweepError> {
        let (outcomes, unavailable) = self.native.classify_or_fallback(subjects, || {
            subjects
                .iter()
                .copied()
                .map(|subject| checked.classify(subject))
                .collect::<Result<Vec<_>, _>>()
                .map(Vec::into_boxed_slice)
        })?;
        self.last_unavailable = unavailable;
        Ok(outcomes)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalNativeClassifierIdentityFieldV2 {
    ProgramHash,
    Relation,
    Admission,
    Question,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalNativeClassifierUnavailable {
    DisabledAfterUnavailable,
    ParityCanaryMismatch,
    InvalidCheckedProgramHash,
    /// Wire protocol V2 carries exactly one authored FIND program and
    /// QuestionId. Plural, empty, and aliased FIND sets use the checked
    /// interpreter; no authored question is treated as primary.
    UnsupportedQuestionSet,
    InvalidFiniteInputShape,
    BatchTooLarge {
        actual: usize,
        maximum: usize,
    },
    UnsupportedFiniteInputValue {
        subject_index: usize,
        binding_index: usize,
    },
    RequestTooLarge,
    InvocationThreadSpawnFailed,
    InvocationThreadPanicked,
    SpawnFailed,
    RequestPipeUnavailable,
    RequestWriteFailed,
    ResponsePipeUnavailable,
    ResponseReadFailed,
    ResponseTooLarge,
    InvocationTimedOut,
    WaitFailed,
    UnsuccessfulExit {
        code: Option<i32>,
    },
    TruncatedResponse {
        field: &'static str,
    },
    InvalidResponseMagic,
    UnsupportedResponseVersion(u32),
    IdentityMismatch(RelationalNativeClassifierIdentityFieldV2),
    OutcomeCountMismatch {
        expected: usize,
        actual: usize,
    },
    InvalidOutcomeTag {
        index: usize,
        tag: u8,
    },
    TrailingResponseBytes {
        count: usize,
    },
}

impl fmt::Display for RelationalNativeClassifierUnavailable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "native classifier unavailable: {self:?}")
    }
}

impl Error for RelationalNativeClassifierUnavailable {}

fn trace_native_classifier_unavailable(unavailable: &RelationalNativeClassifierUnavailable) {
    if std::env::var_os("FUTURUNA_EXPLORE_TRACE").is_some() {
        eprintln!("Explore native classifier disabled; using checked interpreter: {unavailable}");
    }
}

fn encode_request(
    identity: RelationalNativeClassifierIdentityV2,
    finite_inputs: &[RelationalNativeClassifierFiniteInputV2],
    subjects: &[RelationalOrderedClassificationSubject<'_>],
) -> Result<EncodedRequestV2, RelationalNativeClassifierUnavailable> {
    let count = bounded_subject_count(subjects.len())?;
    if finite_inputs.is_empty()
        || finite_inputs.len() > RelationalNativeClassifierProtocolV2::MAX_FACTORS_PER_SUBJECT
        || finite_inputs
            .windows(2)
            .any(|pair| pair[0].binding_index >= pair[1].binding_index)
    {
        return Err(RelationalNativeClassifierUnavailable::InvalidFiniteInputShape);
    }
    let factor_count = u32::try_from(finite_inputs.len())
        .map_err(|_| RelationalNativeClassifierUnavailable::InvalidFiniteInputShape)?;
    let mut request = Vec::with_capacity(MAX_REQUEST_BYTES_V2);
    request.extend_from_slice(REQUEST_MAGIC_V2);
    request.extend_from_slice(&RelationalNativeClassifierProtocolV2::VERSION.to_be_bytes());
    identity.encode_into(&mut request);
    request.extend_from_slice(&factor_count.to_be_bytes());
    request.extend_from_slice(&count.to_be_bytes());
    for (subject_index, subject) in subjects.iter().copied().enumerate() {
        for input in finite_inputs {
            let value = match input.kind {
                RelationalNativeClassifierFiniteInputKindV2::IntValue => {
                    let Some(ExploreValue::Int(value)) =
                        subject.source_binding(input.binding_index)
                    else {
                        return Err(
                            RelationalNativeClassifierUnavailable::UnsupportedFiniteInputValue {
                                subject_index,
                                binding_index: input.binding_index,
                            },
                        );
                    };
                    *value
                }
                RelationalNativeClassifierFiniteInputKindV2::ExactFiniteOrdinal {
                    exact_cardinality,
                    ..
                } => {
                    let Some(ordinal) =
                        subject.source_binding_canonical_ordinal(input.binding_index)
                    else {
                        return Err(
                            RelationalNativeClassifierUnavailable::UnsupportedFiniteInputValue {
                                subject_index,
                                binding_index: input.binding_index,
                            },
                        );
                    };
                    if ordinal >= exact_cardinality {
                        return Err(
                            RelationalNativeClassifierUnavailable::UnsupportedFiniteInputValue {
                                subject_index,
                                binding_index: input.binding_index,
                            },
                        );
                    }
                    i64::try_from(ordinal).map_err(|_| {
                        RelationalNativeClassifierUnavailable::UnsupportedFiniteInputValue {
                            subject_index,
                            binding_index: input.binding_index,
                        }
                    })?
                }
            };
            request.extend_from_slice(&value.to_be_bytes());
        }
    }
    if request.len() > MAX_REQUEST_BYTES_V2 {
        return Err(RelationalNativeClassifierUnavailable::RequestTooLarge);
    }
    Ok(EncodedRequestV2 {
        bytes: request,
        factors: finite_inputs.len(),
        subjects: subjects.len(),
    })
}

fn bounded_subject_count(count: usize) -> Result<u32, RelationalNativeClassifierUnavailable> {
    if count > RELATIONAL_NATIVE_CLASSIFIER_MAX_BATCH_SUBJECTS_V2 {
        return Err(RelationalNativeClassifierUnavailable::BatchTooLarge {
            actual: count,
            maximum: RELATIONAL_NATIVE_CLASSIFIER_MAX_BATCH_SUBJECTS_V2,
        });
    }
    Ok(count as u32)
}

/// Only the whole-batch encoder constructs this in production. Splitting
/// therefore cannot bypass the global batch bound or finite-input validation.
struct EncodedRequestV2 {
    bytes: Vec<u8>,
    factors: usize,
    subjects: usize,
}

impl EncodedRequestV2 {
    fn split(self) -> [Self; 2] {
        let left_count = self.subjects / 2;
        let middle = REQUEST_HEADER_BYTES_V2 + left_count * self.factors * I64_BYTES;
        let part = |rows: &[u8], subjects: usize| {
            let mut bytes = Vec::with_capacity(REQUEST_HEADER_BYTES_V2 + rows.len());
            bytes.extend_from_slice(&self.bytes[..REQUEST_HEADER_BYTES_V2 - U32_BYTES]);
            bytes.extend_from_slice(&(subjects as u32).to_be_bytes());
            bytes.extend_from_slice(rows);
            Self {
                bytes,
                factors: self.factors,
                subjects,
            }
        };
        [
            part(&self.bytes[REQUEST_HEADER_BYTES_V2..middle], left_count),
            part(&self.bytes[middle..], self.subjects - left_count),
        ]
    }
}

fn classify_encoded_batch(
    identity: RelationalNativeClassifierIdentityV2,
    request: EncodedRequestV2,
    native_workers: u16,
    invoke: &(impl Fn(&[u8]) -> Result<Vec<u8>, RelationalNativeClassifierUnavailable> + Sync),
) -> Result<Box<[RelationalClassifiedCaseOutcome]>, RelationalNativeClassifierUnavailable> {
    let classify = |request: &EncodedRequestV2, offset: usize| {
        let response = invoke(&request.bytes)?;
        decode_response(identity, request.subjects, &response).map_err(|error| match error {
            RelationalNativeClassifierUnavailable::InvalidOutcomeTag { index, tag } => {
                RelationalNativeClassifierUnavailable::InvalidOutcomeTag {
                    index: offset + index,
                    tag,
                }
            }
            other => other,
        })
    };
    if native_workers != 2 || request.subjects < MIN_PARALLEL_BATCH_SUBJECTS_V2 {
        return classify(&request, 0);
    }
    let [left_request, right_request] = request.split();
    thread::scope(|scope| {
        // One scoped invocation plus the calling thread: at most two native
        // children, in the same outer-contained process group. Join before
        // inspecting either result, so failure cannot leak a successful prefix
        // or leave an invocation racing the checked whole-batch fallback.
        let left = thread::Builder::new()
            .name("futuruna-native-batch".into())
            .spawn_scoped(scope, || classify(&left_request, 0))
            .map_err(|_| RelationalNativeClassifierUnavailable::InvocationThreadSpawnFailed)?;
        let right = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            classify(&right_request, left_request.subjects)
        }))
        .unwrap_or(Err(
            RelationalNativeClassifierUnavailable::InvocationThreadPanicked,
        ));
        let left = left.join().unwrap_or(Err(
            RelationalNativeClassifierUnavailable::InvocationThreadPanicked,
        ));
        let mut outcomes = left?.into_vec();
        outcomes.extend(right?.into_vec());
        Ok(outcomes.into_boxed_slice())
    })
}

fn invoke_once(
    executable: &Path,
    request: &[u8],
) -> Result<Vec<u8>, RelationalNativeClassifierUnavailable> {
    let deadline = Instant::now() + INVOCATION_TIMEOUT_V2;
    let mut child = Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| RelationalNativeClassifierUnavailable::SpawnFailed)?;

    let Some(mut stdin) = child.stdin.take() else {
        terminate(&mut child);
        return Err(RelationalNativeClassifierUnavailable::RequestPipeUnavailable);
    };
    if stdin.write_all(request).is_err() {
        drop(stdin);
        terminate(&mut child);
        return Err(RelationalNativeClassifierUnavailable::RequestWriteFailed);
    }
    drop(stdin);

    let Some(stdout) = child.stdout.take() else {
        terminate(&mut child);
        return Err(RelationalNativeClassifierUnavailable::ResponsePipeUnavailable);
    };
    let (response_sender, response_receiver) = mpsc::sync_channel(1);
    // This handle is deliberately never joined synchronously. A successful
    // receive means the bounded read has completed. On timeout or process
    // failure a descendant may still hold the inherited pipe open, so dropping
    // the handle detaches that one blocked reader instead of hanging fallback.
    // The caller disables the classifier's shared latch before another
    // sequential batch can invoke it, preventing detached-reader accumulation.
    // A two-invocation batch can detach at most two readers before that latch.
    let _reader = match thread::Builder::new()
        .name("futuruna-native-classifier".to_owned())
        .spawn(move || {
            let response = read_bounded_response(stdout, MAX_RESPONSE_BYTES_V2);
            let _ = response_sender.send(response);
        }) {
        Ok(reader) => reader,
        Err(_) => {
            terminate(&mut child);
            return Err(RelationalNativeClassifierUnavailable::ResponseReadFailed);
        }
    };

    let mut response = None;
    let status = loop {
        if response.is_none() {
            match response_receiver.try_recv() {
                Ok(Ok(received)) => {
                    if received.overflowed {
                        terminate(&mut child);
                        return Err(RelationalNativeClassifierUnavailable::ResponseTooLarge);
                    }
                    response = Some(received.bytes);
                }
                Ok(Err(unavailable)) => {
                    terminate(&mut child);
                    return Err(unavailable);
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    terminate(&mut child);
                    return Err(RelationalNativeClassifierUnavailable::ResponseReadFailed);
                }
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                let now = Instant::now();
                if now >= deadline {
                    terminate(&mut child);
                    return Err(RelationalNativeClassifierUnavailable::InvocationTimedOut);
                }
                thread::sleep(cmp::min(
                    Duration::from_millis(5),
                    deadline.saturating_duration_since(now),
                ));
            }
            Err(_) => {
                terminate(&mut child);
                return Err(RelationalNativeClassifierUnavailable::WaitFailed);
            }
        }
    };
    if !status.success() {
        return Err(RelationalNativeClassifierUnavailable::UnsuccessfulExit {
            code: status.code(),
        });
    }
    let response = match response {
        Some(response) => response,
        None => {
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return Err(RelationalNativeClassifierUnavailable::InvocationTimedOut);
            };
            match response_receiver.recv_timeout(remaining) {
                Ok(Ok(received)) => {
                    if received.overflowed {
                        return Err(RelationalNativeClassifierUnavailable::ResponseTooLarge);
                    }
                    received.bytes
                }
                Ok(Err(unavailable)) => return Err(unavailable),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    return Err(RelationalNativeClassifierUnavailable::InvocationTimedOut);
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(RelationalNativeClassifierUnavailable::ResponseReadFailed);
                }
            }
        }
    };
    if Instant::now() > deadline {
        return Err(RelationalNativeClassifierUnavailable::InvocationTimedOut);
    }
    Ok(response)
}

fn terminate(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

struct BoundedResponse {
    bytes: Vec<u8>,
    overflowed: bool,
}

fn read_bounded_response(
    mut stdout: std::process::ChildStdout,
    limit: usize,
) -> Result<BoundedResponse, RelationalNativeClassifierUnavailable> {
    let mut bytes = Vec::with_capacity(limit);
    let mut overflowed = false;
    let mut buffer = [0_u8; 1024];
    loop {
        let count = stdout
            .read(&mut buffer)
            .map_err(|_| RelationalNativeClassifierUnavailable::ResponseReadFailed)?;
        if count == 0 {
            break;
        }
        let retained = cmp::min(limit.saturating_sub(bytes.len()), count);
        bytes.extend_from_slice(&buffer[..retained]);
        overflowed |= retained != count;
        if overflowed {
            break;
        }
    }
    Ok(BoundedResponse { bytes, overflowed })
}

fn decode_response(
    expected_identity: RelationalNativeClassifierIdentityV2,
    expected_count: usize,
    response: &[u8],
) -> Result<Box<[RelationalClassifiedCaseOutcome]>, RelationalNativeClassifierUnavailable> {
    let mut decoder = ResponseDecoder::new(response);
    if decoder.take(RESPONSE_MAGIC_V2.len(), "magic")? != RESPONSE_MAGIC_V2 {
        return Err(RelationalNativeClassifierUnavailable::InvalidResponseMagic);
    }
    let version = decoder.u32("version")?;
    if version != RelationalNativeClassifierProtocolV2::VERSION {
        return Err(RelationalNativeClassifierUnavailable::UnsupportedResponseVersion(version));
    }
    let actual_identity = RelationalNativeClassifierIdentityV2 {
        program_hash: decoder.digest("program hash")?,
        relation_id: decoder.digest("relation id")?,
        admission_id: decoder.digest("admission id")?,
        question_id: decoder.digest("question id")?,
    };
    for (field, matches) in [
        (
            RelationalNativeClassifierIdentityFieldV2::ProgramHash,
            actual_identity.program_hash == expected_identity.program_hash,
        ),
        (
            RelationalNativeClassifierIdentityFieldV2::Relation,
            actual_identity.relation_id == expected_identity.relation_id,
        ),
        (
            RelationalNativeClassifierIdentityFieldV2::Admission,
            actual_identity.admission_id == expected_identity.admission_id,
        ),
        (
            RelationalNativeClassifierIdentityFieldV2::Question,
            actual_identity.question_id == expected_identity.question_id,
        ),
    ] {
        if !matches {
            return Err(RelationalNativeClassifierUnavailable::IdentityMismatch(
                field,
            ));
        }
    }
    let actual_count = usize::try_from(decoder.u32("outcome count")?).map_err(|_| {
        RelationalNativeClassifierUnavailable::OutcomeCountMismatch {
            expected: expected_count,
            actual: usize::MAX,
        }
    })?;
    if actual_count != expected_count {
        return Err(
            RelationalNativeClassifierUnavailable::OutcomeCountMismatch {
                expected: expected_count,
                actual: actual_count,
            },
        );
    }
    let mut outcomes = Vec::with_capacity(actual_count);
    for index in 0..actual_count {
        let tag = decoder.u8("outcome")?;
        let outcome = RelationalClassifiedCaseOutcome::from_codec_tag(tag)
            .ok_or(RelationalNativeClassifierUnavailable::InvalidOutcomeTag { index, tag })?;
        outcomes.push(outcome);
    }
    if decoder.remaining() != 0 {
        return Err(
            RelationalNativeClassifierUnavailable::TrailingResponseBytes {
                count: decoder.remaining(),
            },
        );
    }
    Ok(outcomes.into_boxed_slice())
}

struct ResponseDecoder<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> ResponseDecoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take(
        &mut self,
        count: usize,
        field: &'static str,
    ) -> Result<&'a [u8], RelationalNativeClassifierUnavailable> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(RelationalNativeClassifierUnavailable::TruncatedResponse { field })?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(RelationalNativeClassifierUnavailable::TruncatedResponse { field })?;
        self.cursor = end;
        Ok(value)
    }

    fn digest(
        &mut self,
        field: &'static str,
    ) -> Result<[u8; DIGEST_BYTES], RelationalNativeClassifierUnavailable> {
        let mut digest = [0_u8; DIGEST_BYTES];
        digest.copy_from_slice(self.take(DIGEST_BYTES, field)?);
        Ok(digest)
    }

    fn u32(&mut self, field: &'static str) -> Result<u32, RelationalNativeClassifierUnavailable> {
        let mut bytes = [0_u8; U32_BYTES];
        bytes.copy_from_slice(self.take(U32_BYTES, field)?);
        Ok(u32::from_be_bytes(bytes))
    }

    fn u8(&mut self, field: &'static str) -> Result<u8, RelationalNativeClassifierUnavailable> {
        Ok(self.take(1, field)?[0])
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.cursor
    }
}

fn decode_lowercase_sha256(value: &str) -> Option<[u8; DIGEST_BYTES]> {
    if value.len() != DIGEST_BYTES * 2
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return None;
    }
    let mut digest = [0_u8; DIGEST_BYTES];
    for (index, output) in digest.iter_mut().enumerate() {
        let high = decode_hex(value.as_bytes()[index * 2])?;
        let low = decode_hex(value.as_bytes()[index * 2 + 1])?;
        *output = (high << 4) | low;
    }
    Some(digest)
}

#[cfg(test)]
mod native_batch_tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::sync::{Condvar, Mutex};

    fn identity() -> RelationalNativeClassifierIdentityV2 {
        RelationalNativeClassifierIdentityV2 {
            program_hash: [1; DIGEST_BYTES],
            relation_id: [2; DIGEST_BYTES],
            admission_id: [3; DIGEST_BYTES],
            question_id: [4; DIGEST_BYTES],
        }
    }

    fn request(count: usize, factors: usize) -> EncodedRequestV2 {
        let inputs = (0..factors)
            .map(|binding_index| RelationalNativeClassifierFiniteInputV2 {
                binding_index,
                kind: RelationalNativeClassifierFiniteInputKindV2::IntValue,
            })
            .collect::<Vec<_>>();
        let mut request = encode_request(identity(), &inputs, &[]).unwrap();
        request.subjects = count;
        request.bytes[REQUEST_HEADER_BYTES_V2 - U32_BYTES..]
            .copy_from_slice(&bounded_subject_count(count).unwrap().to_be_bytes());
        for row in 0..count {
            for factor in 0..factors {
                let scalar = if factor == 0 {
                    row as i64
                } else {
                    -(row as i64) - factor as i64
                };
                request.bytes.extend_from_slice(&scalar.to_be_bytes());
            }
        }
        request
    }

    fn response(tags: &[u8]) -> Vec<u8> {
        let mut response = RESPONSE_MAGIC_V2.to_vec();
        response.extend_from_slice(&RelationalNativeClassifierProtocolV2::VERSION.to_be_bytes());
        identity().encode_into(&mut response);
        response.extend_from_slice(&(tags.len() as u32).to_be_bytes());
        response.extend_from_slice(tags);
        response
    }

    fn first_row(request: &[u8]) -> i64 {
        i64::from_be_bytes(
            request[REQUEST_HEADER_BYTES_V2..REQUEST_HEADER_BYTES_V2 + I64_BYTES]
                .try_into()
                .unwrap(),
        )
    }

    fn classifier() -> RelationalNativeClassifierV2 {
        RelationalNativeClassifierV2 {
            executable: PathBuf::from("unused-native-batch-test"),
            identity: identity(),
            finite_inputs: Arc::from([]),
            enabled: Arc::new(AtomicBool::new(true)),
            parity_checked: Arc::new(AtomicBool::new(false)),
            native_workers: 1,
        }
    }

    #[test]
    fn native_batch_frames_preserve_all_rows_header_and_global_bound() {
        let maximum = RELATIONAL_NATIVE_CLASSIFIER_MAX_BATCH_SUBJECTS_V2;
        for (count, factors) in [(0, 1), (1, 1), (63, 2), (64, 2), (65, 32), (maximum, 2)] {
            let original = request(count, factors);
            let [left, right] = request(count, factors).split();
            let mut rows = Vec::new();
            for part in [&left, &right] {
                assert_eq!(
                    &part.bytes[..REQUEST_HEADER_BYTES_V2 - U32_BYTES],
                    &original.bytes[..REQUEST_HEADER_BYTES_V2 - U32_BYTES]
                );
                assert_eq!(
                    &part.bytes[REQUEST_HEADER_BYTES_V2 - U32_BYTES..REQUEST_HEADER_BYTES_V2],
                    &(part.subjects as u32).to_be_bytes()
                );
                assert_eq!(
                    part.bytes.len(),
                    REQUEST_HEADER_BYTES_V2 + part.subjects * factors * I64_BYTES
                );
                rows.extend_from_slice(&part.bytes[REQUEST_HEADER_BYTES_V2..]);
            }
            assert_eq!(left.subjects + right.subjects, count);
            assert_eq!(rows, original.bytes[REQUEST_HEADER_BYTES_V2..]);
        }
        assert!(bounded_subject_count(maximum).is_ok());
        for count in [maximum + 1, usize::MAX] {
            assert_eq!(
                bounded_subject_count(count),
                Err(RelationalNativeClassifierUnavailable::BatchTooLarge {
                    actual: count,
                    maximum,
                })
            );
        }
    }

    #[test]
    fn native_batch_serial_default_and_small_batches_make_one_invocation() {
        for (workers, count) in [(1, 65), (2, 0), (2, 1), (2, 63)] {
            let calls = AtomicUsize::new(0);
            let original = request(count, 2);
            let outcomes =
                classify_encoded_batch(identity(), request(count, 2), workers, &|bytes| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    assert_eq!(bytes, original.bytes);
                    Ok(response(&vec![1; count]))
                })
                .unwrap();
            assert_eq!(outcomes.len(), count);
            assert_eq!(calls.load(Ordering::SeqCst), 1);
        }
    }

    #[test]
    fn native_batch_parallel_completion_is_reordered_and_bounded_to_two() {
        let completed = (Mutex::new(Vec::new()), Condvar::new());
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let outcomes = classify_encoded_batch(identity(), request(65, 2), 2, &|bytes| {
            let active_now = active.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(active_now, Ordering::SeqCst);
            let first = first_row(bytes);
            let mut done = completed.0.lock().unwrap();
            if first == 0 {
                let (guard, timeout) = completed
                    .1
                    .wait_timeout_while(done, Duration::from_secs(2), |done| done.is_empty())
                    .unwrap();
                assert!(!timeout.timed_out());
                done = guard;
            } else {
                // Keep the second invocation resident until its peer starts,
                // making actual overlap deterministic without a timed sleep.
                drop(done);
                let deadline = Instant::now() + Duration::from_secs(2);
                while active.load(Ordering::SeqCst) < 2 {
                    assert!(Instant::now() < deadline, "peer invocation did not start");
                    thread::yield_now();
                }
                done = completed.0.lock().unwrap();
            }
            done.push(first);
            completed.1.notify_all();
            active.fetch_sub(1, Ordering::SeqCst);
            Ok(response(&vec![
                if first == 0 { 1 } else { 3 };
                if first == 0 { 32 } else { 33 }
            ]))
        })
        .unwrap();
        assert_eq!(*completed.0.lock().unwrap(), [32, 0]);
        assert_eq!(peak.load(Ordering::SeqCst), 2);
        assert_eq!(active.load(Ordering::SeqCst), 0);
        assert_eq!(
            &*outcomes,
            &*decode_response(
                identity(),
                65,
                &response(&[vec![1; 32], vec![3; 33]].concat())
            )
            .unwrap()
        );
    }

    #[test]
    fn native_batch_rejects_bad_subresponses_atomically_and_disables_clones() {
        let mut corruptions = Vec::new();
        let good = response(&[3; 32]);
        let identity_start = RESPONSE_MAGIC_V2.len() + U32_BYTES;
        for field in 0..RelationalNativeClassifierProtocolV2::IDENTITY_DIGEST_COUNT {
            let mut bad = good.clone();
            bad[identity_start + field * DIGEST_BYTES] ^= 1;
            corruptions.push(Ok(bad));
        }
        let mut bad_magic = good.clone();
        bad_magic[0] ^= 1;
        corruptions.push(Ok(bad_magic));
        let mut bad_version = good.clone();
        bad_version[RESPONSE_MAGIC_V2.len()] ^= 1;
        corruptions.push(Ok(bad_version));
        corruptions.extend([
            Ok(response(&[3; 31])),
            Ok(response(&[3; 33])),
            Ok(good[..good.len() - 1].to_vec()),
            Ok([good.clone(), vec![0]].concat()),
            Ok(response(&[0; 32])),
            Err(RelationalNativeClassifierUnavailable::InvocationTimedOut),
            Err(RelationalNativeClassifierUnavailable::SpawnFailed),
        ]);
        for corrupt in corruptions {
            for bad_first in [0, 32] {
                let calls = AtomicUsize::new(0);
                let result = classify_encoded_batch(identity(), request(64, 1), 2, &|bytes| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    if first_row(bytes) == bad_first {
                        corrupt.clone()
                    } else {
                        Ok(good.clone())
                    }
                });
                assert!(result.is_err());
                assert_eq!(calls.load(Ordering::SeqCst), 2);
                if let Err(RelationalNativeClassifierUnavailable::InvalidOutcomeTag {
                    index, ..
                }) = &result
                {
                    assert_eq!(*index, bad_first as usize);
                }
                let native = classifier();
                let clone = native.clone();
                let fallback_calls = AtomicUsize::new(0);
                let (outcomes, reason) = native
                    .finish_or_fallback(result, || {
                        fallback_calls.fetch_add(1, Ordering::SeqCst);
                        Ok::<_, ()>(
                            vec![RelationalClassifiedCaseOutcome::Rejected; 64].into_boxed_slice(),
                        )
                    })
                    .unwrap();
                assert_eq!(outcomes.len(), 64);
                assert!(outcomes
                    .iter()
                    .all(|outcome| *outcome == RelationalClassifiedCaseOutcome::Rejected));
                assert!(reason.is_some());
                assert_eq!(fallback_calls.load(Ordering::SeqCst), 1);
                assert!(!clone.is_enabled());
                assert_eq!(
                    clone.classify_ordered_batch(&[]),
                    Err(RelationalNativeClassifierUnavailable::DisabledAfterUnavailable)
                );
            }
        }
    }

    #[test]
    fn native_batch_panics_join_both_invocations_before_fallback() {
        for panic_first in [0, 32] {
            let retired = AtomicUsize::new(0);
            struct Retire<'a>(&'a AtomicUsize);
            impl Drop for Retire<'_> {
                fn drop(&mut self) {
                    self.0.fetch_add(1, Ordering::SeqCst);
                }
            }
            let result = classify_encoded_batch(identity(), request(64, 1), 2, &|bytes| {
                let _retire = Retire(&retired);
                assert_ne!(
                    first_row(bytes),
                    panic_first,
                    "synthetic native invocation panic"
                );
                Ok(response(&[1; 32]))
            });
            assert_eq!(
                result,
                Err(RelationalNativeClassifierUnavailable::InvocationThreadPanicked)
            );
            assert_eq!(retired.load(Ordering::SeqCst), 2);
        }
    }

    #[test]
    fn native_batch_first_success_still_requires_whole_batch_checked_parity() {
        let native = classifier();
        let outcomes = decode_response(identity(), 65, &response(&[3; 65])).unwrap();
        let calls = AtomicUsize::new(0);
        let (accepted, unavailable) = native
            .finish_or_fallback(Ok(outcomes.clone()), || {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok::<_, ()>(outcomes.clone())
            })
            .unwrap();
        assert_eq!(accepted, outcomes);
        assert_eq!(unavailable, None);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(native.parity_checked());
        native
            .finish_or_fallback(Ok(outcomes.clone()), || -> Result<_, ()> {
                panic!("checked parity must not repeat after its first success")
            })
            .unwrap();
        let other = classifier();
        let fallback = vec![RelationalClassifiedCaseOutcome::Rejected; 65].into_boxed_slice();
        let (accepted, unavailable) = other
            .finish_or_fallback(Ok(outcomes), || Ok::<_, ()>(fallback.clone()))
            .unwrap();
        assert_eq!(accepted, fallback);
        assert_eq!(
            unavailable,
            Some(RelationalNativeClassifierUnavailable::ParityCanaryMismatch)
        );
        assert!(!other.is_enabled());
    }
}

const fn decode_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}
