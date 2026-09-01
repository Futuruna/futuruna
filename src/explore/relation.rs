//! Canonical content identities and set semantics for relational Explore cases.
//!
//! This module is deliberately independent of the current Cartesian executor,
//! output projection, probe plan, and durable rank encoding. It is the common
//! identity boundary for a finite source relation and each source row's finite
//! successor relation.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::{transition::canonical_explore_value_digest, ExploreValue};

const RELATION_ID_HASH_V1: &[u8] = b"futuruna.explore.relation-id.v1";
const ADMISSION_ID_HASH_V1: &[u8] = b"futuruna.explore.admission-id.v1";
const QUESTION_ID_HASH_V1: &[u8] = b"futuruna.explore.question-id.v1";
const VIEW_ID_HASH_V1: &[u8] = b"futuruna.explore.view-id.v1";
const MECHANISM_REQUEST_ID_HASH_V1: &[u8] = b"futuruna.explore.mechanism-request-id.v1";
const SOURCE_KEY_HASH_V1: &[u8] = b"futuruna.explore.source-key.v1";
const SOURCE_KEY_SET_ROOT_HASH_V1: &[u8] = b"futuruna.explore.source-key-set-root.v1";
const SUCCESSOR_KEY_HASH_V1: &[u8] = b"futuruna.explore.successor-key.v1";
const RELATIONAL_CASE_ID_HASH_V1: &[u8] = b"futuruna.explore.relational-case-id.v1";
const RELATION_LINEAGE_ID_HASH_V1: &[u8] = b"futuruna.explore.relation-lineage-id.v1";
const RELATION_SUPPORT_ID_HASH_V1: &[u8] = b"futuruna.explore.relation-support-id.v1";
const RELATION_FRONTIER_ROOT_HASH_V1: &[u8] = b"futuruna.explore.relation-frontier-root.v1";
const RELATION_CONTENT_ROOT_HASH_V1: &[u8] = b"futuruna.explore.relation-content-root.v1";
const ADMISSION_FRONTIER_ROOT_HASH_V1: &[u8] = b"futuruna.explore.admission-frontier-root.v1";
const ADMISSION_CONTENT_ROOT_HASH_V1: &[u8] = b"futuruna.explore.admission-content-root.v1";
const QUESTION_FRONTIER_ROOT_HASH_V1: &[u8] = b"futuruna.explore.question-frontier-root.v1";
const QUESTION_CONTENT_ROOT_HASH_V1: &[u8] = b"futuruna.explore.question-content-root.v1";

const RELATION_SEMANTIC_DIGEST_ROLE: u8 = 0x01;
const ADMISSION_RELATION_ROLE: u8 = 0x01;
const ADMISSION_SEMANTIC_DIGEST_ROLE: u8 = 0x02;
const QUESTION_ADMISSION_ROLE: u8 = 0x01;
const QUESTION_FIND_SEMANTIC_DIGEST_ROLE: u8 = 0x02;
const QUESTION_FIND_POLARITY_ROLE: u8 = 0x03;
const VIEW_INPUT_ROLE: u8 = 0x01;
const VIEW_SEMANTIC_DIGEST_ROLE: u8 = 0x02;
const MECHANISM_QUESTION_ROLE: u8 = 0x01;
const MECHANISM_TARGET_ROLE: u8 = 0x02;
const MECHANISM_OBSERVATION_DIGEST_ROLE: u8 = 0x03;
const MECHANISM_NORMALIZATION_DIGEST_ROLE: u8 = 0x04;
const RELATION_ROLE: u8 = 0x01;
const SOURCE_CONTEXT_ROLE: u8 = 0x02;
const SOURCE_BEFORE_ROLE: u8 = 0x03;
const SOURCE_ROLE: u8 = 0x02;
const SUCCESSOR_AFTER_ROLE: u8 = 0x03;
const SUCCESSOR_ROLE: u8 = 0x03;

/// Identity of one normalized source/successor relation contract.
///
/// Its canonical semantic digest is supplied by the checked relational IR. In
/// particular, admission, FIND selection, presentation views,
/// probe/scheduling choices, and run-local limits are not inputs to this
/// identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationId([u8; 32]);

impl RelationId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Derive an identity directly from the canonical semantic relation
    /// preimage. This is equivalent to first taking its SHA-256 digest and
    /// calling [`Self::from_canonical_semantic_digest`].
    pub(crate) fn from_canonical_semantic_preimage(preimage: &[u8]) -> Self {
        Self::from_canonical_semantic_digest(Sha256::digest(preimage).into())
    }

    /// Derive an identity from an already computed canonical semantic digest.
    pub(crate) fn from_canonical_semantic_digest(semantic_digest: [u8; 32]) -> Self {
        let mut hasher = IdentityHasher::new(RELATION_ID_HASH_V1);
        hasher.tag(RELATION_SEMANTIC_DIGEST_ROLE);
        hasher.digest(semantic_digest);
        Self(hasher.finish())
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Authenticated content of one open relation frontier.
///
/// This root changes when rows, provenance, or closure facts change. It is a
/// resumable evidence commitment, not the semantic identity of the producer.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationFrontierRoot([u8; 32]);

impl RelationFrontierRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Authenticated content of one completely enumerated relation.
///
/// Eager and incremental executions of the same [`RelationId`] converge to
/// this root when they discover the same canonical rows and complete support.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationContentRoot([u8; 32]);

impl RelationContentRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Identity of one normalized admission layer over a finite relation.
///
/// Source and successor construction or intrinsic membership belong to
/// [`RelationId`]. This identity begins at the logically separate scoped
/// admission predicates and deliberately excludes FIND selection and views.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct AdmissionId([u8; 32]);

impl AdmissionId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) fn from_canonical_admission_preimage(
        relation_id: RelationId,
        admission_preimage: &[u8],
    ) -> Self {
        Self::from_canonical_admission_digest(
            relation_id,
            Sha256::digest(admission_preimage).into(),
        )
    }

    pub(crate) fn from_canonical_admission_digest(
        relation_id: RelationId,
        admission_digest: [u8; 32],
    ) -> Self {
        let mut hasher = IdentityHasher::new(ADMISSION_ID_HASH_V1);
        hasher.tag(ADMISSION_RELATION_ROLE);
        hasher.digest(relation_id.0);
        hasher.tag(ADMISSION_SEMANTIC_DIGEST_ROLE);
        hasher.digest(admission_digest);
        Self(hasher.finish())
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Authenticated prefix of admission classifications over one upstream
/// relation frontier or completed relation content root.
///
/// Counts alone are not this identity: the root commits every canonical
/// `CaseId -> AdmissionDecision` member as well as the semantic admission
/// layer and the exact upstream commitment it classified.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct AdmissionFrontierRoot([u8; 32]);

impl AdmissionFrontierRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Authenticated content of a complete admission classification.
///
/// Construction is reserved for a validated closed catalog and binds the
/// completed [`RelationContentRoot`] rather than an open relation frontier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct AdmissionContentRoot([u8; 32]);

impl AdmissionContentRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Canonical polarity of a normalized FIND selection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum FindPolarity {
    All,
    Matches,
    Violations,
}

impl FindPolarity {
    const fn canonical_tag(self) -> u8 {
        match self {
            Self::All => 0x01,
            Self::Matches => 0x02,
            Self::Violations => 0x03,
        }
    }
}

/// Identity of one normalized FIND question over an admitted relation.
///
/// The canonical FIND digest seals the checked predicate or the canonical
/// `find all` form. Polarity is additionally tagged so complementary selections
/// over the same predicate can share relation and admission evidence without
/// sharing question identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct QuestionId([u8; 32]);

impl QuestionId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) fn from_canonical_find_preimage(
        admission_id: AdmissionId,
        find_preimage: &[u8],
        polarity: FindPolarity,
    ) -> Self {
        Self::from_canonical_find_digest(
            admission_id,
            Sha256::digest(find_preimage).into(),
            polarity,
        )
    }

    pub(crate) fn from_canonical_find_digest(
        admission_id: AdmissionId,
        find_digest: [u8; 32],
        polarity: FindPolarity,
    ) -> Self {
        let mut hasher = IdentityHasher::new(QUESTION_ID_HASH_V1);
        hasher.tag(QUESTION_ADMISSION_ROLE);
        hasher.digest(admission_id.0);
        hasher.tag(QUESTION_FIND_SEMANTIC_DIGEST_ROLE);
        hasher.digest(find_digest);
        hasher.tag(QUESTION_FIND_POLARITY_ROLE);
        hasher.bytes(&[polarity.canonical_tag()]);
        Self(hasher.finish())
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Authenticated prefix of FIND classifications over one admission frontier
/// or completed admission content root.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct QuestionFrontierRoot([u8; 32]);

impl QuestionFrontierRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Authenticated content of a complete FIND classification.
///
/// Construction is reserved for a validated closed catalog and binds the
/// completed [`AdmissionContentRoot`] rather than an open admission frontier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct QuestionContentRoot([u8; 32]);

impl QuestionContentRoot {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Semantic input relation consumed by one named result view.
///
/// A case view reads the selected relation of a [`QuestionId`]. A
/// post-replay view reads the incidence relation produced by one
/// [`MechanismRequestId`]. Source-level declaration names are resolved before
/// this boundary and therefore cannot rename an otherwise equal view.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ViewInputId {
    Sources(RelationId),
    Selected(QuestionId),
    MechanismIncidence(MechanismRequestId),
}

impl ViewInputId {
    fn hash_into(self, hasher: &mut IdentityHasher) {
        match self {
            Self::Sources(relation_id) => {
                hasher.bytes(&[0x03]);
                hasher.digest(relation_id.0);
            }
            Self::Selected(question_id) => {
                hasher.bytes(&[0x01]);
                hasher.digest(question_id.0);
            }
            Self::MechanismIncidence(request_id) => {
                hasher.bytes(&[0x02]);
                hasher.digest(request_id.0);
            }
        }
    }
}

/// Identity of one typed result view over a resolved input relation.
///
/// The supplied semantic digest seals the view's grain, measures, closed
/// reducers, `having`, public projection, deterministic choice and privacy
/// policy. The source-level view name and declaration position are addresses,
/// not semantic identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ViewId([u8; 32]);

impl ViewId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) fn from_canonical_view_preimage(input: ViewInputId, view_preimage: &[u8]) -> Self {
        Self::from_canonical_view_digest(input, Sha256::digest(view_preimage).into())
    }

    pub(crate) fn from_canonical_view_digest(input: ViewInputId, view_digest: [u8; 32]) -> Self {
        let mut hasher = IdentityHasher::new(VIEW_ID_HASH_V1);
        hasher.tag(VIEW_INPUT_ROLE);
        input.hash_into(&mut hasher);
        hasher.tag(VIEW_SEMANTIC_DIGEST_ROLE);
        hasher.digest(view_digest);
        Self(hasher.finish())
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Resolved case population requested for differential mechanism replay.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum MechanismTargetId {
    /// Every case selected by the request's [`QuestionId`].
    Selected,
    /// The deterministically chosen case set of a resolved case view.
    ChosenView(ViewId),
}

impl MechanismTargetId {
    fn hash_into(self, hasher: &mut IdentityHasher) {
        match self {
            Self::Selected => hasher.bytes(&[0x01]),
            Self::ChosenView(view_id) => {
                hasher.bytes(&[0x02]);
                hasher.digest(view_id.0);
            }
        }
    }
}

/// Identity of one differential mechanism request.
///
/// A request is scoped to a question, a resolved case target, one checked
/// endpoint observation and a signature-normalization contract. The authored
/// request name is deliberately absent. Incidence and post-replay views key
/// their evidence with this identity rather than with a display address.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismRequestId([u8; 32]);

impl MechanismRequestId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) fn from_canonical_request_preimages(
        question_id: QuestionId,
        target: MechanismTargetId,
        observation_preimage: &[u8],
        normalization_preimage: &[u8],
    ) -> Self {
        Self::from_canonical_request_digests(
            question_id,
            target,
            Sha256::digest(observation_preimage).into(),
            Sha256::digest(normalization_preimage).into(),
        )
    }

    pub(crate) fn from_canonical_request_digests(
        question_id: QuestionId,
        target: MechanismTargetId,
        observation_digest: [u8; 32],
        normalization_digest: [u8; 32],
    ) -> Self {
        let mut hasher = IdentityHasher::new(MECHANISM_REQUEST_ID_HASH_V1);
        hasher.tag(MECHANISM_QUESTION_ROLE);
        hasher.digest(question_id.0);
        hasher.tag(MECHANISM_TARGET_ROLE);
        target.hash_into(&mut hasher);
        hasher.tag(MECHANISM_OBSERVATION_DIGEST_ROLE);
        hasher.digest(observation_digest);
        hasher.tag(MECHANISM_NORMALIZATION_DIGEST_ROLE);
        hasher.digest(normalization_digest);
        Self(hasher.finish())
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Content-stable identity of one canonical `(Context, Before)` source row.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SourceKey([u8; 32]);

impl SourceKey {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) fn derive(relation_id: RelationId, row: &SourceRow) -> Self {
        let mut hasher = IdentityHasher::new(SOURCE_KEY_HASH_V1);
        hasher.tag(RELATION_ROLE);
        hasher.digest(relation_id.0);
        hasher.tag(SOURCE_CONTEXT_ROLE);
        hasher.digest(canonical_explore_value_digest(&row.context));
        hasher.tag(SOURCE_BEFORE_ROLE);
        hasher.digest(canonical_explore_value_digest(&row.before));
        Self(hasher.finish())
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Canonical set commitment for the exact source keys of one relation.
///
/// This is deliberately narrower than a [`RelationContentRoot`]: source
/// closure can compare it before any dependent successor relation is closed.
/// The relation identity and exact member count are committed alongside the
/// canonically ordered keys.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SourceKeySetRoot([u8; 32]);

impl SourceKeySetRoot {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Derive the canonical source-key set root and its exact count in one pass.
///
/// Callers supply keys in strict `SourceKey` order. Both maintained callers
/// use `BTreeMap`/`BTreeSet` iterators, so no second audit-sized collection is
/// materialized merely to close the source relation.
pub(crate) fn canonical_source_key_set_commitment(
    relation_id: RelationId,
    source_keys: impl ExactSizeIterator<Item = SourceKey>,
) -> (SourceKeySetRoot, u128) {
    let exact_count = source_keys.len() as u128;
    let mut hasher = IdentityHasher::new(SOURCE_KEY_SET_ROOT_HASH_V1);
    hasher.tag(0x01);
    hasher.digest(relation_id.bytes());
    hasher.tag(0x02);
    hasher.bytes(&exact_count.to_be_bytes());
    for source_key in source_keys {
        hasher.tag(0x03);
        hasher.digest(source_key.bytes());
    }
    (SourceKeySetRoot(hasher.finish()), exact_count)
}

/// Content-stable identity of one After row in a particular source's
/// dependent successor relation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SuccessorKey([u8; 32]);

impl SuccessorKey {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) fn derive(
        relation_id: RelationId,
        source_key: SourceKey,
        row: &SuccessorRow,
    ) -> Self {
        let mut hasher = IdentityHasher::new(SUCCESSOR_KEY_HASH_V1);
        hasher.tag(RELATION_ROLE);
        hasher.digest(relation_id.0);
        hasher.tag(SOURCE_ROLE);
        hasher.digest(source_key.0);
        hasher.tag(SUCCESSOR_AFTER_ROLE);
        hasher.digest(canonical_explore_value_digest(&row.after));
        Self(hasher.finish())
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Stable case identity for one source/successor coordinate.
///
/// No ordinal, discovery sequence, worker, or scheduler decision contributes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalCaseId([u8; 32]);

impl RelationalCaseId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) fn derive(
        relation_id: RelationId,
        source_key: SourceKey,
        successor_key: SuccessorKey,
    ) -> Self {
        let mut hasher = IdentityHasher::new(RELATIONAL_CASE_ID_HASH_V1);
        hasher.tag(RELATION_ROLE);
        hasher.digest(relation_id.0);
        hasher.tag(SOURCE_ROLE);
        hasher.digest(source_key.0);
        hasher.tag(SUCCESSOR_ROLE);
        hasher.digest(successor_key.0);
        Self(hasher.finish())
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Canonical identity of one producer lineage claim retained behind a row.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationLineageId([u8; 32]);

impl RelationLineageId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) fn from_canonical_preimage(preimage: &[u8]) -> Self {
        Self(derive_content_identity(
            RELATION_LINEAGE_ID_HASH_V1,
            preimage,
        ))
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Canonical identity of one producer support coordinate retained behind a
/// deduplicated row.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationSupportId([u8; 32]);

impl RelationSupportId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) fn from_canonical_preimage(preimage: &[u8]) -> Self {
        Self(derive_content_identity(
            RELATION_SUPPORT_ID_HASH_V1,
            preimage,
        ))
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Exact provenance retained behind one canonical set member.
///
/// Provenance does not rename a row. Rediscovering equal content unions both
/// sets so duplicate generation cannot inflate source or case counts and does
/// not discard support.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RelationProvenance {
    lineage: InlineCanonicalSet<RelationLineageId>,
    support: InlineCanonicalSet<RelationSupportId>,
}

impl RelationProvenance {
    pub(crate) fn new(
        lineage: impl IntoIterator<Item = RelationLineageId>,
        support: impl IntoIterator<Item = RelationSupportId>,
    ) -> Self {
        Self {
            lineage: lineage.into_iter().collect(),
            support: support.into_iter().collect(),
        }
    }

    pub(crate) fn lineage(&self) -> &InlineCanonicalSet<RelationLineageId> {
        &self.lineage
    }

    pub(crate) fn support(&self) -> &InlineCanonicalSet<RelationSupportId> {
        &self.support
    }

    fn union(&mut self, other: Self) {
        self.lineage.union(other.lineage);
        self.support.union(other.support);
    }
}

/// Canonically ordered set with allocation-free storage for the common
/// zero-, one-, and two-member cases.
///
/// Every `Two` value is strictly ascending and every `Many` value contains at
/// least three members. Those representation invariants make derived set
/// equality exact while keeping iteration identical to `BTreeSet` order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InlineCanonicalSet<T>(InlineCanonicalSetStorage<T>);

#[derive(Clone, Debug, Eq, PartialEq)]
enum InlineCanonicalSetStorage<T> {
    Empty,
    One(T),
    Two([T; 2]),
    Many(BTreeSet<T>),
}

impl<T> Default for InlineCanonicalSet<T> {
    fn default() -> Self {
        Self(InlineCanonicalSetStorage::Empty)
    }
}

impl<T: Copy + Ord> InlineCanonicalSet<T> {
    pub(crate) fn len(&self) -> usize {
        match &self.0 {
            InlineCanonicalSetStorage::Empty => 0,
            InlineCanonicalSetStorage::One(_) => 1,
            InlineCanonicalSetStorage::Two(_) => 2,
            InlineCanonicalSetStorage::Many(values) => values.len(),
        }
    }

    fn insert(&mut self, value: T) -> bool {
        match &mut self.0 {
            InlineCanonicalSetStorage::Empty => {
                self.0 = InlineCanonicalSetStorage::One(value);
                true
            }
            InlineCanonicalSetStorage::One(existing) => match value.cmp(existing) {
                std::cmp::Ordering::Less => {
                    self.0 = InlineCanonicalSetStorage::Two([value, *existing]);
                    true
                }
                std::cmp::Ordering::Equal => false,
                std::cmp::Ordering::Greater => {
                    self.0 = InlineCanonicalSetStorage::Two([*existing, value]);
                    true
                }
            },
            InlineCanonicalSetStorage::Two(existing) => {
                if existing.binary_search(&value).is_ok() {
                    return false;
                }
                let mut values = BTreeSet::new();
                values.extend(existing.iter().copied());
                let inserted = values.insert(value);
                debug_assert!(inserted);
                self.0 = InlineCanonicalSetStorage::Many(values);
                true
            }
            InlineCanonicalSetStorage::Many(values) => values.insert(value),
        }
    }

    fn union(&mut self, other: Self) {
        for value in &other {
            self.insert(*value);
        }
    }

    fn iter(&self) -> InlineCanonicalSetIter<'_, T> {
        match &self.0 {
            InlineCanonicalSetStorage::Empty => InlineCanonicalSetIter::Empty,
            InlineCanonicalSetStorage::One(value) => {
                InlineCanonicalSetIter::Inline(std::slice::from_ref(value).iter())
            }
            InlineCanonicalSetStorage::Two(values) => InlineCanonicalSetIter::Inline(values.iter()),
            InlineCanonicalSetStorage::Many(values) => InlineCanonicalSetIter::Many(values.iter()),
        }
    }
}

impl<T: Copy + Ord> FromIterator<T> for InlineCanonicalSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(values: I) -> Self {
        let mut set = Self::default();
        for value in values {
            set.insert(value);
        }
        set
    }
}

pub(crate) enum InlineCanonicalSetIter<'a, T> {
    Empty,
    Inline(std::slice::Iter<'a, T>),
    Many(std::collections::btree_set::Iter<'a, T>),
}

impl<'a, T> Iterator for InlineCanonicalSetIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Empty => None,
            Self::Inline(values) => values.next(),
            Self::Many(values) => values.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len();
        (remaining, Some(remaining))
    }
}

impl<T> ExactSizeIterator for InlineCanonicalSetIter<'_, T> {
    fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Inline(values) => values.len(),
            Self::Many(values) => values.len(),
        }
    }
}

impl<'a, T: Copy + Ord> IntoIterator for &'a InlineCanonicalSet<T> {
    type Item = &'a T;
    type IntoIter = InlineCanonicalSetIter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Canonical source-relation row. Context deliberately includes the action or
/// intervention identity whenever two otherwise equal endpoint pairs are
/// materially different cases.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceRow {
    context: ExploreValue,
    before: ExploreValue,
    provenance: RelationProvenance,
}

impl SourceRow {
    pub(crate) fn new(
        context: ExploreValue,
        before: ExploreValue,
        provenance: RelationProvenance,
    ) -> Self {
        Self {
            context,
            before,
            provenance,
        }
    }

    pub(crate) fn context(&self) -> &ExploreValue {
        &self.context
    }

    pub(crate) fn before(&self) -> &ExploreValue {
        &self.before
    }

    pub(crate) fn provenance(&self) -> &RelationProvenance {
        &self.provenance
    }

    fn has_same_content(&self, other: &Self) -> bool {
        self.context == other.context && self.before == other.before
    }

    fn union_provenance(&mut self, other: Self) {
        self.provenance.union(other.provenance);
    }
}

/// Canonical member of one source row's dependent successor relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SuccessorRow {
    after: ExploreValue,
    provenance: RelationProvenance,
}

impl SuccessorRow {
    pub(crate) fn new(after: ExploreValue, provenance: RelationProvenance) -> Self {
        Self { after, provenance }
    }

    pub(crate) fn after(&self) -> &ExploreValue {
        &self.after
    }

    pub(crate) fn provenance(&self) -> &RelationProvenance {
        &self.provenance
    }

    fn has_same_content(&self, other: &Self) -> bool {
        self.after == other.after
    }

    fn union_provenance(&mut self, other: Self) {
        self.provenance.union(other.provenance);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceDraft {
    row: SourceRow,
    successors: SmallSuccessorMap,
    successor_enumeration_closed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SuccessorDraft {
    row: SuccessorRow,
    case_id: RelationalCaseId,
}

/// Deterministic successor storage without paying for a `BTreeMap` node when
/// a source has no successor or exactly one successor.
///
/// The outer `Option<Box<_>>` keeps the empty representation to one nullable
/// pointer without making every [`SourceDraft`] large enough to hold an inline
/// [`SuccessorDraft`]. The second distinct key upgrades the boxed singleton to
/// a `BTreeMap`; all iteration therefore remains in strict `SuccessorKey`
/// order in every representation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct SmallSuccessorMap {
    storage: Option<Box<SmallSuccessorStorage>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SmallSuccessorStorage {
    One {
        key: SuccessorKey,
        value: SuccessorDraft,
    },
    Many(BTreeMap<SuccessorKey, SuccessorDraft>),
}

impl SmallSuccessorMap {
    const fn new() -> Self {
        Self { storage: None }
    }

    fn len(&self) -> usize {
        match self.storage.as_deref() {
            None => 0,
            Some(SmallSuccessorStorage::One { .. }) => 1,
            Some(SmallSuccessorStorage::Many(successors)) => successors.len(),
        }
    }

    fn get(&self, key: &SuccessorKey) -> Option<&SuccessorDraft> {
        match self.storage.as_deref() {
            None => None,
            Some(SmallSuccessorStorage::One {
                key: existing_key,
                value,
            }) if existing_key == key => Some(value),
            Some(SmallSuccessorStorage::One { .. }) => None,
            Some(SmallSuccessorStorage::Many(successors)) => successors.get(key),
        }
    }

    fn get_mut(&mut self, key: &SuccessorKey) -> Option<&mut SuccessorDraft> {
        match self.storage.as_deref_mut() {
            None => None,
            Some(SmallSuccessorStorage::One {
                key: existing_key,
                value,
            }) if existing_key == key => Some(value),
            Some(SmallSuccessorStorage::One { .. }) => None,
            Some(SmallSuccessorStorage::Many(successors)) => successors.get_mut(key),
        }
    }

    fn insert(&mut self, key: SuccessorKey, value: SuccessorDraft) -> Option<SuccessorDraft> {
        match self.storage.as_deref_mut() {
            None => {
                self.storage = Some(Box::new(SmallSuccessorStorage::One { key, value }));
                return None;
            }
            Some(SmallSuccessorStorage::One {
                key: existing_key,
                value: existing_value,
            }) if *existing_key == key => {
                return Some(std::mem::replace(existing_value, value));
            }
            Some(SmallSuccessorStorage::One { .. }) => {}
            Some(SmallSuccessorStorage::Many(successors)) => {
                return successors.insert(key, value);
            }
        }

        let Some(singleton) = self.storage.take() else {
            unreachable!("nonempty successor storage disappeared during upgrade")
        };
        let SmallSuccessorStorage::One {
            key: existing_key,
            value: existing_value,
        } = *singleton
        else {
            unreachable!("only singleton successor storage upgrades to a tree")
        };
        let mut successors = BTreeMap::new();
        successors.insert(existing_key, existing_value);
        let replaced = successors.insert(key, value);
        debug_assert!(replaced.is_none());
        self.storage = Some(Box::new(SmallSuccessorStorage::Many(successors)));
        None
    }

    fn iter(&self) -> SmallSuccessorIter<'_> {
        match self.storage.as_deref() {
            None => SmallSuccessorIter::Empty,
            Some(SmallSuccessorStorage::One { key, value }) => {
                SmallSuccessorIter::One(Some((key, value)))
            }
            Some(SmallSuccessorStorage::Many(successors)) => {
                SmallSuccessorIter::Many(successors.iter())
            }
        }
    }

    fn values(&self) -> impl ExactSizeIterator<Item = &SuccessorDraft> {
        self.iter().map(|(_, value)| value)
    }
}

enum SmallSuccessorIter<'a> {
    Empty,
    One(Option<(&'a SuccessorKey, &'a SuccessorDraft)>),
    Many(std::collections::btree_map::Iter<'a, SuccessorKey, SuccessorDraft>),
}

impl<'a> Iterator for SmallSuccessorIter<'a> {
    type Item = (&'a SuccessorKey, &'a SuccessorDraft);

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Empty => None,
            Self::One(successor) => successor.take(),
            Self::Many(successors) => successors.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len();
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for SmallSuccessorIter<'_> {
    fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::One(successor) => usize::from(successor.is_some()),
            Self::Many(successors) => successors.len(),
        }
    }
}

enum SmallSuccessorIntoIter {
    Empty,
    One(Option<(SuccessorKey, SuccessorDraft)>),
    Many(std::collections::btree_map::IntoIter<SuccessorKey, SuccessorDraft>),
}

impl Iterator for SmallSuccessorIntoIter {
    type Item = (SuccessorKey, SuccessorDraft);

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Empty => None,
            Self::One(successor) => successor.take(),
            Self::Many(successors) => successors.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len();
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for SmallSuccessorIntoIter {
    fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::One(successor) => usize::from(successor.is_some()),
            Self::Many(successors) => successors.len(),
        }
    }
}

impl IntoIterator for SmallSuccessorMap {
    type Item = (SuccessorKey, SuccessorDraft);
    type IntoIter = SmallSuccessorIntoIter;

    fn into_iter(self) -> Self::IntoIter {
        match self.storage {
            None => SmallSuccessorIntoIter::Empty,
            Some(storage) => match *storage {
                SmallSuccessorStorage::One { key, value } => {
                    SmallSuccessorIntoIter::One(Some((key, value)))
                }
                SmallSuccessorStorage::Many(successors) => {
                    SmallSuccessorIntoIter::Many(successors.into_iter())
                }
            },
        }
    }
}

/// Closure-aware cardinality of a discovered finite relation population.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationCountEvidence {
    LowerBound(u128),
    Exact(u128),
}

impl RelationCountEvidence {
    pub(crate) const fn observed(self) -> u128 {
        match self {
            Self::LowerBound(value) | Self::Exact(value) => value,
        }
    }

    pub(crate) const fn is_exact(self) -> bool {
        matches!(self, Self::Exact(_))
    }
}

/// Source and case counts at one enumeration frontier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationEnumerationCounts {
    sources: RelationCountEvidence,
    cases: RelationCountEvidence,
}

impl RelationEnumerationCounts {
    pub(crate) const fn sources(self) -> RelationCountEvidence {
        self.sources
    }

    pub(crate) const fn cases(self) -> RelationCountEvidence {
        self.cases
    }
}

/// Incremental, collision-checking set builder for one relational exploration
/// universe.
#[derive(Clone, Debug)]
pub(crate) struct RelationCatalogBuilder {
    relation_id: RelationId,
    source_enumeration_closed: bool,
    sources: BTreeMap<SourceKey, SourceDraft>,
    successor_claims: BTreeMap<SuccessorKey, (SourceKey, ExploreValue)>,
    case_claims: BTreeMap<RelationalCaseId, (SourceKey, SuccessorKey)>,
}

#[derive(Clone, Debug)]
pub(crate) struct PreparedSuccessorInsert {
    source_key: SourceKey,
    successor_key: SuccessorKey,
    case_id: RelationalCaseId,
    row: SuccessorRow,
}

impl PreparedSuccessorInsert {
    pub(crate) const fn successor_key(&self) -> SuccessorKey {
        self.successor_key
    }

    pub(crate) const fn case_id(&self) -> RelationalCaseId {
        self.case_id
    }
}

impl RelationCatalogBuilder {
    pub(crate) fn new(relation_id: RelationId) -> Self {
        Self {
            relation_id,
            source_enumeration_closed: false,
            sources: BTreeMap::new(),
            successor_claims: BTreeMap::new(),
            case_claims: BTreeMap::new(),
        }
    }

    pub(crate) const fn relation_id(&self) -> RelationId {
        self.relation_id
    }

    pub(crate) fn contains_source(&self, source_key: SourceKey) -> bool {
        self.sources.contains_key(&source_key)
    }

    /// Borrow one discovered source row without rebuilding canonical snapshot
    /// order. Scheduler lookups use this O(log N) path between durable events.
    pub(crate) fn source_row(&self, source_key: SourceKey) -> Option<&SourceRow> {
        self.sources.get(&source_key).map(|source| &source.row)
    }

    /// Borrow the canonical source-key order without materializing a full
    /// relation snapshot. Closure receipts can compare coverage in O(S) time
    /// and O(1) additional row memory.
    pub(crate) fn source_keys(&self) -> impl ExactSizeIterator<Item = SourceKey> + '_ {
        self.sources.keys().copied()
    }

    /// Commit the currently discovered canonical source-key set without
    /// cloning it. Source-seal replay compares this independently maintained
    /// catalog commitment with the source traversal's aggregate receipt.
    pub(crate) fn source_key_set_commitment(&self) -> (SourceKeySetRoot, u128) {
        canonical_source_key_set_commitment(self.relation_id, self.source_keys())
    }

    /// Count the currently discovered successors for one source without
    /// rebuilding or sorting the complete relation snapshot. Receipt
    /// validation calls this once per source fiber, so the keyed path is
    /// essential for keeping full traversal linearithmic rather than
    /// quadratic in the number of source rows.
    pub(crate) fn successor_count(
        &self,
        source_key: SourceKey,
    ) -> Result<usize, RelationCatalogError> {
        self.sources
            .get(&source_key)
            .map(|source| source.successors.len())
            .ok_or(RelationCatalogError::UnknownSource { source_key })
    }

    /// Test membership in the currently discovered case prefix without
    /// materializing or canonically sorting a relation snapshot.
    pub(crate) fn contains_case(&self, case_id: RelationalCaseId) -> bool {
        self.case_claims.contains_key(&case_id)
    }

    /// Borrow one discovered case directly from the mutable keyed catalogs.
    /// This has the same extensional frame as a snapshot/catalog case but does
    /// not sort or clone the accumulated relation.
    pub(crate) fn case(&self, case_id: RelationalCaseId) -> Option<RelationalCaseRef<'_>> {
        let (source_key, successor_key) = *self.case_claims.get(&case_id)?;
        let source = self.sources.get(&source_key)?;
        let successor = source.successors.get(&successor_key)?;
        Some(RelationalCaseRef::new(
            self.relation_id,
            source_key,
            successor_key,
            case_id,
            &source.row,
            &successor.row,
        ))
    }

    pub(crate) const fn source_enumeration_is_closed(&self) -> bool {
        self.source_enumeration_closed
    }

    /// Idempotently seal discovery of new source rows. Existing sources may
    /// continue enumerating successors until their own frontiers are sealed.
    pub(crate) fn seal_source_enumeration(&mut self) -> bool {
        let changed = !self.source_enumeration_closed;
        self.source_enumeration_closed = true;
        changed
    }

    pub(crate) fn successor_enumeration_is_closed(
        &self,
        source_key: SourceKey,
    ) -> Result<bool, RelationCatalogError> {
        self.sources
            .get(&source_key)
            .map(|source| source.successor_enumeration_closed)
            .ok_or(RelationCatalogError::UnknownSource { source_key })
    }

    /// Idempotently seal one discovered source's dependent successor set.
    pub(crate) fn seal_successor_enumeration(
        &mut self,
        source_key: SourceKey,
    ) -> Result<bool, RelationCatalogError> {
        let source = self
            .sources
            .get_mut(&source_key)
            .ok_or(RelationCatalogError::UnknownSource { source_key })?;
        let changed = !source.successor_enumeration_closed;
        source.successor_enumeration_closed = true;
        Ok(changed)
    }

    pub(crate) fn enumeration_is_complete(&self) -> bool {
        self.source_enumeration_closed
            && self
                .sources
                .values()
                .all(|source| source.successor_enumeration_closed)
    }

    pub(crate) fn counts(&self) -> RelationEnumerationCounts {
        let source_count = self.sources.len() as u128;
        let case_count = self.case_claims.len() as u128;
        RelationEnumerationCounts {
            sources: if self.source_enumeration_closed {
                RelationCountEvidence::Exact(source_count)
            } else {
                RelationCountEvidence::LowerBound(source_count)
            },
            cases: if self.enumeration_is_complete() {
                RelationCountEvidence::Exact(case_count)
            } else {
                RelationCountEvidence::LowerBound(case_count)
            },
        }
    }

    pub(crate) fn source_count(&self) -> usize {
        self.sources.len()
    }

    pub(crate) fn case_count(&self) -> usize {
        self.case_claims.len()
    }

    /// Canonically ordered keys whose dependent successor enumerations remain
    /// open. The source frontier itself is reported separately.
    pub(crate) fn open_source_keys(&self) -> Box<[SourceKey]> {
        let mut sources = self
            .sources
            .iter()
            .filter(|(_, source)| !source.successor_enumeration_closed)
            .map(|(key, source)| (*key, &source.row))
            .collect::<Vec<_>>();
        sources.sort_by(|(left_key, left), (right_key, right)| {
            left.context
                .cmp(&right.context)
                .then_with(|| left.before.cmp(&right.before))
                .then_with(|| left_key.cmp(right_key))
        });
        sources
            .into_iter()
            .map(|(key, _)| key)
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }

    /// Insert or merge one canonical source member.
    pub(crate) fn insert_source(
        &mut self,
        row: SourceRow,
    ) -> Result<SourceKey, RelationCatalogError> {
        if self.source_enumeration_closed {
            return Err(RelationCatalogError::SourceEnumerationClosed);
        }
        let source_key = SourceKey::derive(self.relation_id, &row);
        match self.sources.get_mut(&source_key) {
            Some(existing) if existing.row.has_same_content(&row) => {
                existing.row.union_provenance(row);
            }
            Some(_) => {
                return Err(RelationCatalogError::SourceKeyCollision { source_key });
            }
            None => {
                self.sources.insert(
                    source_key,
                    SourceDraft {
                        row,
                        successors: SmallSuccessorMap::new(),
                        successor_enumeration_closed: false,
                    },
                );
            }
        }
        Ok(source_key)
    }

    /// Insert or merge one member of a source row's dependent successor set.
    ///
    /// All collision checks happen before mutation, so a rejected insertion is
    /// atomic with respect to the catalog under construction.
    pub(crate) fn insert_successor(
        &mut self,
        source_key: SourceKey,
        row: SuccessorRow,
    ) -> Result<(SuccessorKey, RelationalCaseId), RelationCatalogError> {
        let prepared = self.preflight_insert_successor(source_key, row)?;
        Ok(self.commit_preflight_successor(prepared))
    }

    /// Validate all identity, collision and closure conditions for one
    /// successor without mutating the relation layer.
    pub(crate) fn preflight_insert_successor(
        &self,
        source_key: SourceKey,
        row: SuccessorRow,
    ) -> Result<PreparedSuccessorInsert, RelationCatalogError> {
        let source = self
            .sources
            .get(&source_key)
            .ok_or(RelationCatalogError::UnknownSource { source_key })?;
        if source.successor_enumeration_closed {
            return Err(RelationCatalogError::SuccessorEnumerationClosed { source_key });
        }
        let successor_key = SuccessorKey::derive(self.relation_id, source_key, &row);
        let case_id = RelationalCaseId::derive(self.relation_id, source_key, successor_key);

        if let Some((claimed_source, claimed_after)) = self.successor_claims.get(&successor_key) {
            if *claimed_source != source_key || claimed_after != &row.after {
                return Err(RelationCatalogError::SuccessorKeyCollision { successor_key });
            }
        }
        if let Some((claimed_source, claimed_successor)) = self.case_claims.get(&case_id) {
            if *claimed_source != source_key || *claimed_successor != successor_key {
                return Err(RelationCatalogError::CaseIdCollision { case_id });
            }
        }
        if let Some(existing) = source.successors.get(&successor_key) {
            if !existing.row.has_same_content(&row) {
                return Err(RelationCatalogError::SuccessorKeyCollision { successor_key });
            }
        }

        Ok(PreparedSuccessorInsert {
            source_key,
            successor_key,
            case_id,
            row,
        })
    }

    pub(crate) fn commit_preflight_successor(
        &mut self,
        prepared: PreparedSuccessorInsert,
    ) -> (SuccessorKey, RelationalCaseId) {
        let PreparedSuccessorInsert {
            source_key,
            successor_key,
            case_id,
            row,
        } = prepared;
        self.successor_claims
            .entry(successor_key)
            .or_insert_with(|| (source_key, row.after.clone()));
        self.case_claims
            .entry(case_id)
            .or_insert((source_key, successor_key));
        let source = self
            .sources
            .get_mut(&source_key)
            .expect("source existence was checked before relational insertion");
        match source.successors.get_mut(&successor_key) {
            Some(existing) => existing.row.union_provenance(row),
            None => {
                source
                    .successors
                    .insert(successor_key, SuccessorDraft { row, case_id });
            }
        }
        debug_assert_eq!(source.successors.values().len(), source.successors.len());

        (successor_key, case_id)
    }

    /// Merge a batch-local relation overlay after every semantic check has
    /// succeeded. This is private to the atomic selected-case installer above;
    /// callers cannot mint an unchecked delta directly.
    fn apply_selected_delta(&mut self, delta: Self) {
        debug_assert_eq!(self.relation_id, delta.relation_id);
        debug_assert!(!self.source_enumeration_closed);
        let Self {
            relation_id: _,
            source_enumeration_closed: _,
            sources,
            successor_claims,
            case_claims,
        } = delta;

        for (source_key, incoming) in sources {
            let SourceDraft {
                row,
                successors,
                successor_enumeration_closed: _,
            } = incoming;
            match self.sources.get_mut(&source_key) {
                Some(existing) => {
                    debug_assert!(existing.row.has_same_content(&row));
                    debug_assert!(!existing.successor_enumeration_closed);
                    existing.row.union_provenance(row);
                    for (successor_key, incoming) in successors {
                        match existing.successors.get_mut(&successor_key) {
                            Some(current) => {
                                debug_assert!(current.row.has_same_content(&incoming.row));
                                current.row.union_provenance(incoming.row);
                            }
                            None => {
                                let previous = existing.successors.insert(successor_key, incoming);
                                debug_assert!(previous.is_none());
                            }
                        }
                    }
                }
                None => {
                    let previous = self.sources.insert(
                        source_key,
                        SourceDraft {
                            row,
                            successors,
                            successor_enumeration_closed: false,
                        },
                    );
                    debug_assert!(previous.is_none());
                }
            }
        }
        for (successor_key, claim) in successor_claims {
            self.successor_claims.entry(successor_key).or_insert(claim);
        }
        for (case_id, claim) in case_claims {
            self.case_claims.entry(case_id).or_insert(claim);
        }
    }

    /// Materialize the currently discovered prefix in canonical value order.
    /// The snapshot is honest about every open producer frontier and does not
    /// imply that its lower-bound populations are complete.
    pub(crate) fn snapshot(&self) -> RelationCatalogSnapshot {
        let rows = canonical_catalog_rows(
            self.sources
                .iter()
                .map(|(key, source)| (*key, source.clone())),
        );
        RelationCatalogSnapshot {
            relation_id: self.relation_id,
            source_enumeration_closed: self.source_enumeration_closed,
            open_source_keys: self.open_source_keys(),
            counts: self.counts(),
            rows,
        }
    }

    /// Freeze a completely enumerated relation into its sealed catalog.
    /// Open source or successor frontiers must be represented by a partial
    /// snapshot instead of being silently promoted to exact closure.
    pub(crate) fn finish(self) -> Result<RelationCatalog, RelationCatalogError> {
        if !self.enumeration_is_complete() {
            return Err(RelationCatalogError::EnumerationIncomplete {
                source_enumeration_open: !self.source_enumeration_closed,
                open_successor_enumerations: self
                    .sources
                    .values()
                    .filter(|source| !source.successor_enumeration_closed)
                    .count(),
            });
        }
        Ok(RelationCatalog {
            relation_id: self.relation_id,
            rows: canonical_catalog_rows(self.sources),
        })
    }

    /// Validate closure and commit this builder without cloning or consuming
    /// its row catalogs. The returned capability borrows the exact builder
    /// whose canonical content root it authenticates, so downstream borrowed
    /// admission and question closure cannot substitute an unclosed relation.
    pub(crate) fn close_borrowed(
        &self,
    ) -> Result<ClosedRelationCatalogRef<'_>, RelationCatalogError> {
        if !self.enumeration_is_complete() {
            return Err(RelationCatalogError::EnumerationIncomplete {
                source_enumeration_open: !self.source_enumeration_closed,
                open_successor_enumerations: self
                    .sources
                    .values()
                    .filter(|source| !source.successor_enumeration_closed)
                    .count(),
            });
        }
        Ok(ClosedRelationCatalogRef {
            builder: self,
            content_root: RelationContentRoot(hash_closed_catalog_builder(self)),
        })
    }
}

/// Borrowed closure capability for one fully enumerated relation builder.
///
/// Unlike [`RelationCatalog`], this view does not own canonical rows or either
/// lookup index. Its only allocation is temporary pointer-order scratch while
/// the content root is minted.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ClosedRelationCatalogRef<'a> {
    builder: &'a RelationCatalogBuilder,
    content_root: RelationContentRoot,
}

impl ClosedRelationCatalogRef<'_> {
    pub(crate) const fn relation_id(self) -> RelationId {
        self.builder.relation_id
    }

    pub(crate) const fn content_root(self) -> RelationContentRoot {
        self.content_root
    }
}

/// One canonical source row and its canonically ordered dependent successor
/// set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CatalogSource {
    key: SourceKey,
    row: SourceRow,
    successors: Box<[CatalogSuccessor]>,
}

impl CatalogSource {
    pub(crate) const fn key(&self) -> SourceKey {
        self.key
    }

    pub(crate) fn row(&self) -> &SourceRow {
        &self.row
    }

    pub(crate) fn successors(&self) -> &[CatalogSuccessor] {
        &self.successors
    }
}

/// One canonical successor and the case identity formed with its owning
/// source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CatalogSuccessor {
    key: SuccessorKey,
    case_id: RelationalCaseId,
    row: SuccessorRow,
}

impl CatalogSuccessor {
    pub(crate) const fn key(&self) -> SuccessorKey {
        self.key
    }

    pub(crate) const fn case_id(&self) -> RelationalCaseId {
        self.case_id
    }

    pub(crate) fn row(&self) -> &SuccessorRow {
        &self.row
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CanonicalCatalogRows {
    sources: Box<[CatalogSource]>,
    source_index: BTreeMap<SourceKey, usize>,
    case_index: BTreeMap<RelationalCaseId, (usize, usize)>,
}

fn canonical_catalog_rows(
    sources: impl IntoIterator<Item = (SourceKey, SourceDraft)>,
) -> CanonicalCatalogRows {
    let mut sources = sources
        .into_iter()
        .map(|(key, source)| {
            let mut successors = source
                .successors
                .into_iter()
                .map(|(key, successor)| CatalogSuccessor {
                    key,
                    case_id: successor.case_id,
                    row: successor.row,
                })
                .collect::<Vec<_>>();
            successors.sort_by(|left, right| {
                left.row
                    .after
                    .cmp(&right.row.after)
                    .then_with(|| left.key.cmp(&right.key))
            });
            CatalogSource {
                key,
                row: source.row,
                successors: successors.into_boxed_slice(),
            }
        })
        .collect::<Vec<_>>();
    sources.sort_by(|left, right| {
        left.row
            .context
            .cmp(&right.row.context)
            .then_with(|| left.row.before.cmp(&right.row.before))
            .then_with(|| left.key.cmp(&right.key))
    });

    let mut source_index = BTreeMap::new();
    let mut case_index = BTreeMap::new();
    for (source_index_value, source) in sources.iter().enumerate() {
        source_index.insert(source.key, source_index_value);
        for (successor_index, successor) in source.successors.iter().enumerate() {
            case_index.insert(successor.case_id, (source_index_value, successor_index));
        }
    }
    CanonicalCatalogRows {
        sources: sources.into_boxed_slice(),
        source_index,
        case_index,
    }
}

fn relation_case<'a>(
    relation_id: RelationId,
    rows: &'a CanonicalCatalogRows,
    case_id: RelationalCaseId,
) -> Option<RelationalCaseRef<'a>> {
    let (source_index, successor_index) = *rows.case_index.get(&case_id)?;
    let source = rows.sources.get(source_index)?;
    let successor = source.successors.get(successor_index)?;
    Some(RelationalCaseRef::new(
        relation_id,
        source.key,
        successor.key,
        successor.case_id,
        &source.row,
        &successor.row,
    ))
}

/// Immutable canonical view of every row discovered at one open or closed
/// enumeration frontier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationCatalogSnapshot {
    relation_id: RelationId,
    source_enumeration_closed: bool,
    open_source_keys: Box<[SourceKey]>,
    counts: RelationEnumerationCounts,
    rows: CanonicalCatalogRows,
}

impl RelationCatalogSnapshot {
    pub(crate) const fn relation_id(&self) -> RelationId {
        self.relation_id
    }

    pub(crate) const fn source_enumeration_is_closed(&self) -> bool {
        self.source_enumeration_closed
    }

    pub(crate) fn enumeration_is_complete(&self) -> bool {
        self.source_enumeration_closed && self.open_source_keys.is_empty()
    }

    pub(crate) const fn counts(&self) -> RelationEnumerationCounts {
        self.counts
    }

    pub(crate) fn open_source_keys(&self) -> &[SourceKey] {
        &self.open_source_keys
    }

    pub(crate) fn sources(&self) -> &[CatalogSource] {
        &self.rows.sources
    }

    pub(crate) fn source(&self, source_key: SourceKey) -> Option<&CatalogSource> {
        self.rows
            .source_index
            .get(&source_key)
            .and_then(|index| self.rows.sources.get(*index))
    }

    pub(crate) fn case(&self, case_id: RelationalCaseId) -> Option<RelationalCaseRef<'_>> {
        relation_case(self.relation_id, &self.rows, case_id)
    }

    /// Iterate over the currently discovered cases in canonical value order.
    pub(crate) fn cases(&self) -> impl Iterator<Item = RelationalCaseRef<'_>> {
        self.rows.sources.iter().flat_map(move |source| {
            source.successors.iter().map(move |successor| {
                RelationalCaseRef::new(
                    self.relation_id,
                    source.key,
                    successor.key,
                    successor.case_id,
                    &source.row,
                    &successor.row,
                )
            })
        })
    }

    /// Commit the discovered relation prefix and every open/closed producer
    /// frontier without claiming extensional completion.
    pub(crate) fn frontier_root(&self) -> RelationFrontierRoot {
        RelationFrontierRoot(hash_catalog_state(
            RELATION_FRONTIER_ROOT_HASH_V1,
            self.relation_id,
            self.source_enumeration_closed,
            &self.open_source_keys,
            &self.rows,
        ))
    }
}

/// Immutable, canonically ordered set of source rows and their successors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationCatalog {
    relation_id: RelationId,
    rows: CanonicalCatalogRows,
}

impl RelationCatalog {
    pub(crate) const fn relation_id(&self) -> RelationId {
        self.relation_id
    }

    pub(crate) fn source_count(&self) -> usize {
        self.rows.sources.len()
    }

    pub(crate) fn case_count(&self) -> usize {
        self.rows.case_index.len()
    }

    pub(crate) fn counts(&self) -> RelationEnumerationCounts {
        RelationEnumerationCounts {
            sources: RelationCountEvidence::Exact(self.source_count() as u128),
            cases: RelationCountEvidence::Exact(self.case_count() as u128),
        }
    }

    pub(crate) fn sources(&self) -> &[CatalogSource] {
        &self.rows.sources
    }

    pub(crate) fn source(&self, source_key: SourceKey) -> Option<&CatalogSource> {
        self.rows
            .source_index
            .get(&source_key)
            .and_then(|index| self.rows.sources.get(*index))
    }

    pub(crate) fn case(&self, case_id: RelationalCaseId) -> Option<RelationalCaseRef<'_>> {
        relation_case(self.relation_id, &self.rows, case_id)
    }

    /// Iterate in canonical `(Context, Before, After, identity)` value order.
    pub(crate) fn cases(&self) -> impl Iterator<Item = RelationalCaseRef<'_>> {
        self.rows.sources.iter().flat_map(move |source| {
            source.successors.iter().map(move |successor| {
                RelationalCaseRef::new(
                    self.relation_id,
                    source.key,
                    successor.key,
                    successor.case_id,
                    &source.row,
                    &successor.row,
                )
            })
        })
    }

    /// Commit the complete extensional relation and its unioned provenance.
    pub(crate) fn content_root(&self) -> RelationContentRoot {
        RelationContentRoot(hash_catalog_state(
            RELATION_CONTENT_ROOT_HASH_V1,
            self.relation_id,
            true,
            &[],
            &self.rows,
        ))
    }
}

fn hash_catalog_state(
    domain: &[u8],
    relation_id: RelationId,
    source_enumeration_closed: bool,
    open_source_keys: &[SourceKey],
    rows: &CanonicalCatalogRows,
) -> [u8; 32] {
    let mut hasher = begin_catalog_state_hash(
        domain,
        relation_id,
        source_enumeration_closed,
        open_source_keys,
        rows.sources.len(),
    );
    for source in rows.sources.iter() {
        hash_catalog_source(
            &mut hasher,
            source.key,
            source.row.provenance(),
            source.successors.len(),
        );
        for successor in source.successors.iter() {
            hash_catalog_successor(
                &mut hasher,
                successor.key,
                successor.case_id,
                successor.row.provenance(),
            );
        }
    }
    hasher.finish()
}

/// Hash a closed builder in the same canonical value order as
/// [`canonical_catalog_rows`] without cloning values or constructing indexes.
fn hash_closed_catalog_builder(builder: &RelationCatalogBuilder) -> [u8; 32] {
    let mut sources = Vec::with_capacity(builder.sources.len());
    sources.extend(builder.sources.iter());
    sources.sort_unstable_by(|(left_key, left), (right_key, right)| {
        left.row
            .context
            .cmp(&right.row.context)
            .then_with(|| left.row.before.cmp(&right.row.before))
            .then_with(|| left_key.cmp(right_key))
    });

    let max_successors = sources
        .iter()
        .map(|(_, source)| source.successors.len())
        .max()
        .unwrap_or(0);
    let mut successors = Vec::with_capacity(max_successors);
    let mut hasher = begin_catalog_state_hash(
        RELATION_CONTENT_ROOT_HASH_V1,
        builder.relation_id,
        true,
        &[],
        sources.len(),
    );
    for (source_key, source) in sources {
        successors.clear();
        successors.extend(source.successors.iter());
        successors.sort_unstable_by(|(left_key, left), (right_key, right)| {
            left.row
                .after
                .cmp(&right.row.after)
                .then_with(|| left_key.cmp(right_key))
        });
        hash_catalog_source(
            &mut hasher,
            *source_key,
            source.row.provenance(),
            successors.len(),
        );
        for (successor_key, successor) in successors.iter().copied() {
            hash_catalog_successor(
                &mut hasher,
                *successor_key,
                successor.case_id,
                successor.row.provenance(),
            );
        }
    }
    hasher.finish()
}

fn begin_catalog_state_hash(
    domain: &[u8],
    relation_id: RelationId,
    source_enumeration_closed: bool,
    open_source_keys: &[SourceKey],
    source_count: usize,
) -> IdentityHasher {
    let mut hasher = IdentityHasher::new(domain);
    hasher.tag(0x01);
    hasher.digest(relation_id.bytes());
    hasher.tag(0x02);
    hasher.bytes(&[u8::from(source_enumeration_closed)]);

    let mut open_source_keys = open_source_keys.to_vec();
    open_source_keys.sort_unstable();
    hasher.tag(0x03);
    hasher.bytes(&(open_source_keys.len() as u128).to_be_bytes());
    for source_key in open_source_keys {
        hasher.digest(source_key.bytes());
    }

    hasher.tag(0x04);
    hasher.bytes(&(source_count as u128).to_be_bytes());
    hasher
}

fn hash_catalog_source(
    hasher: &mut IdentityHasher,
    source_key: SourceKey,
    provenance: &RelationProvenance,
    successor_count: usize,
) {
    hasher.tag(0x05);
    hasher.digest(source_key.bytes());
    hash_relation_provenance(hasher, provenance);
    hasher.tag(0x06);
    hasher.bytes(&(successor_count as u128).to_be_bytes());
}

fn hash_catalog_successor(
    hasher: &mut IdentityHasher,
    successor_key: SuccessorKey,
    case_id: RelationalCaseId,
    provenance: &RelationProvenance,
) {
    hasher.tag(0x07);
    hasher.digest(successor_key.bytes());
    hasher.digest(case_id.bytes());
    hash_relation_provenance(hasher, provenance);
}

fn hash_relation_provenance(hasher: &mut IdentityHasher, provenance: &RelationProvenance) {
    hasher.tag(0x08);
    hasher.bytes(&(provenance.lineage().len() as u128).to_be_bytes());
    for lineage in provenance.lineage() {
        hasher.digest(lineage.bytes());
    }
    hasher.tag(0x09);
    hasher.bytes(&(provenance.support().len() as u128).to_be_bytes());
    for support in provenance.support() {
        hasher.digest(support.bytes());
    }
}

/// Admission classification of one already constructed relational case.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AdmissionDecision {
    Rejected,
    Admitted,
}

impl AdmissionDecision {
    const fn canonical_tag(self) -> u8 {
        match self {
            Self::Rejected => 0x01,
            Self::Admitted => 0x02,
        }
    }
}

/// FIND classification of one admitted case.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SelectionDecision {
    NotSelected,
    Selected,
}

impl SelectionDecision {
    const fn canonical_tag(self) -> u8 {
        match self {
            Self::NotSelected => 0x01,
            Self::Selected => 0x02,
        }
    }
}

/// Closure-aware counts for an admission layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AdmissionCounts {
    classified: RelationCountEvidence,
    admitted: RelationCountEvidence,
    rejected: RelationCountEvidence,
}

impl AdmissionCounts {
    pub(crate) const fn classified(self) -> RelationCountEvidence {
        self.classified
    }

    pub(crate) const fn admitted(self) -> RelationCountEvidence {
        self.admitted
    }

    pub(crate) const fn rejected(self) -> RelationCountEvidence {
        self.rejected
    }
}

/// Closure-aware counts for a FIND question over an admitted relation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SelectionCounts {
    classified: RelationCountEvidence,
    selected: RelationCountEvidence,
    not_selected: RelationCountEvidence,
}

impl SelectionCounts {
    pub(crate) const fn classified(self) -> RelationCountEvidence {
        self.classified
    }

    pub(crate) const fn selected(self) -> RelationCountEvidence {
        self.selected
    }

    pub(crate) const fn not_selected(self) -> RelationCountEvidence {
        self.not_selected
    }
}

#[derive(Clone, Copy)]
enum ClassificationUpstreamRoot {
    Frontier([u8; 32]),
    Content([u8; 32]),
}

fn hash_classification_state<Decision: Copy>(
    domain: &[u8],
    semantic_layer_id: [u8; 32],
    upstream: ClassificationUpstreamRoot,
    decisions: &BTreeMap<RelationalCaseId, Decision>,
    decision_tag: fn(Decision) -> u8,
) -> [u8; 32] {
    let mut hasher = IdentityHasher::new(domain);
    hasher.tag(0x01);
    hasher.digest(semantic_layer_id);
    hasher.tag(0x02);
    match upstream {
        ClassificationUpstreamRoot::Frontier(root) => {
            hasher.tag(0x01);
            hasher.digest(root);
        }
        ClassificationUpstreamRoot::Content(root) => {
            hasher.tag(0x02);
            hasher.digest(root);
        }
    }
    hasher.tag(0x03);
    hasher.bytes(&(decisions.len() as u128).to_be_bytes());
    for (case_id, decision) in decisions {
        hasher.tag(0x04);
        hasher.digest(case_id.bytes());
        hasher.tag(decision_tag(*decision));
    }
    hasher.finish()
}

/// Incremental admission evidence over one discovered relation.
///
/// Decisions are keyed by `(AdmissionId, CaseId)` through the builder's fixed
/// identity. Repeating equal evidence is idempotent; contradictory evidence is
/// rejected rather than rewriting a previously classified case.
#[derive(Clone, Debug)]
pub(crate) struct AdmissionCatalogBuilder {
    relation_id: RelationId,
    admission_id: AdmissionId,
    decisions: BTreeMap<RelationalCaseId, AdmissionDecision>,
    admitted_count: usize,
}

impl AdmissionCatalogBuilder {
    pub(crate) fn new(relation_id: RelationId, admission_id: AdmissionId) -> Self {
        Self {
            relation_id,
            admission_id,
            decisions: BTreeMap::new(),
            admitted_count: 0,
        }
    }

    pub(crate) const fn relation_id(&self) -> RelationId {
        self.relation_id
    }

    pub(crate) const fn admission_id(&self) -> AdmissionId {
        self.admission_id
    }

    pub(crate) fn decision(&self, case_id: RelationalCaseId) -> Option<AdmissionDecision> {
        self.decisions.get(&case_id).copied()
    }

    pub(crate) fn admitted_case_ids(&self) -> impl Iterator<Item = RelationalCaseId> + '_ {
        self.decisions.iter().filter_map(|(case_id, decision)| {
            matches!(decision, AdmissionDecision::Admitted).then_some(*case_id)
        })
    }

    pub(crate) fn decision_count(&self) -> usize {
        self.decisions.len()
    }

    pub(crate) fn contains_decision(&self, decision: AdmissionDecision) -> bool {
        match decision {
            AdmissionDecision::Admitted => self.admitted_count != 0,
            AdmissionDecision::Rejected => self.decisions.len() != self.admitted_count,
        }
    }

    pub(crate) fn admitted_count(&self) -> usize {
        self.admitted_count
    }

    /// Commit this admission prefix over an already authenticated open
    /// relation frontier. The caller can reuse the upstream root already held
    /// by its journal snapshot; no relation rows are rebuilt here.
    pub(crate) fn frontier_root(
        &self,
        relation_frontier_root: RelationFrontierRoot,
    ) -> AdmissionFrontierRoot {
        AdmissionFrontierRoot(hash_classification_state(
            ADMISSION_FRONTIER_ROOT_HASH_V1,
            self.admission_id.bytes(),
            ClassificationUpstreamRoot::Frontier(relation_frontier_root.bytes()),
            &self.decisions,
            AdmissionDecision::canonical_tag,
        ))
    }

    /// Commit an open admission prefix after its upstream relation has closed.
    /// The result remains a frontier root until every admission decision has
    /// been validated by [`Self::finish`].
    pub(crate) fn frontier_root_over_closed_relation(
        &self,
        relation_content_root: RelationContentRoot,
    ) -> AdmissionFrontierRoot {
        AdmissionFrontierRoot(hash_classification_state(
            ADMISSION_FRONTIER_ROOT_HASH_V1,
            self.admission_id.bytes(),
            ClassificationUpstreamRoot::Content(relation_content_root.bytes()),
            &self.decisions,
            AdmissionDecision::canonical_tag,
        ))
    }

    pub(crate) fn classify(
        &mut self,
        relation: &RelationCatalogSnapshot,
        case_id: RelationalCaseId,
        decision: AdmissionDecision,
    ) -> Result<bool, RelationClassificationError> {
        if relation.relation_id() != self.relation_id {
            return Err(RelationClassificationError::RelationIdentityMismatch);
        }
        if relation.case(case_id).is_none() {
            return Err(RelationClassificationError::UnknownCase { case_id });
        }
        self.classify_known_case(case_id, decision)
    }

    /// Classify against the mutable relation frontier by keyed membership.
    /// This is the journal ingestion path: it avoids constructing a full
    /// canonical snapshot for a one-case preflight.
    pub(crate) fn classify_open(
        &mut self,
        relation: &RelationCatalogBuilder,
        case_id: RelationalCaseId,
        decision: AdmissionDecision,
    ) -> Result<bool, RelationClassificationError> {
        let insert = self.preflight_classify_open(relation, case_id, decision)?;
        Ok(self.commit_preflight_classification(case_id, decision, insert))
    }

    /// Validate one admission event without mutating the classification
    /// layer. Journal transactions use this together with downstream semantic
    /// transition preflights before committing either layer.
    pub(crate) fn preflight_classify_open(
        &self,
        relation: &RelationCatalogBuilder,
        case_id: RelationalCaseId,
        decision: AdmissionDecision,
    ) -> Result<bool, RelationClassificationError> {
        if relation.relation_id() != self.relation_id {
            return Err(RelationClassificationError::RelationIdentityMismatch);
        }
        if !relation.contains_case(case_id) {
            return Err(RelationClassificationError::UnknownCase { case_id });
        }
        match self.decisions.get(&case_id) {
            Some(existing) if *existing == decision => Ok(false),
            Some(_) => Err(RelationClassificationError::AdmissionDecisionConflict { case_id }),
            None => Ok(true),
        }
    }

    pub(crate) fn commit_preflight_classification(
        &mut self,
        case_id: RelationalCaseId,
        decision: AdmissionDecision,
        insert: bool,
    ) -> bool {
        if !insert {
            return false;
        }
        let previous = self.decisions.insert(case_id, decision);
        debug_assert!(previous.is_none());
        if decision == AdmissionDecision::Admitted {
            self.admitted_count += 1;
        }
        true
    }

    fn classify_known_case(
        &mut self,
        case_id: RelationalCaseId,
        decision: AdmissionDecision,
    ) -> Result<bool, RelationClassificationError> {
        match self.decisions.get(&case_id) {
            Some(existing) if *existing == decision => Ok(false),
            Some(_) => Err(RelationClassificationError::AdmissionDecisionConflict { case_id }),
            None => {
                self.decisions.insert(case_id, decision);
                if decision == AdmissionDecision::Admitted {
                    self.admitted_count += 1;
                }
                Ok(true)
            }
        }
    }

    pub(crate) fn counts(&self) -> AdmissionCounts {
        let admitted = self.admitted_count as u128;
        let rejected = self.decisions.len() as u128 - admitted;
        AdmissionCounts {
            classified: RelationCountEvidence::LowerBound(self.decisions.len() as u128),
            admitted: RelationCountEvidence::LowerBound(admitted),
            rejected: RelationCountEvidence::LowerBound(rejected),
        }
    }

    /// Report the classification prefix against the same open relation
    /// frontier. Counts become exact only when relation enumeration is closed
    /// and every constructible case has an admission decision.
    pub(crate) fn counts_at(
        &self,
        relation: &RelationCatalogSnapshot,
    ) -> Result<AdmissionCounts, RelationClassificationError> {
        if relation.relation_id() != self.relation_id {
            return Err(RelationClassificationError::RelationIdentityMismatch);
        }
        let admitted = self.admitted_count as u128;
        let rejected = self.decisions.len() as u128 - admitted;
        let complete =
            relation.enumeration_is_complete() && self.decisions.len() == relation.cases().count();
        let evidence = |value| {
            if complete {
                RelationCountEvidence::Exact(value)
            } else {
                RelationCountEvidence::LowerBound(value)
            }
        };
        Ok(AdmissionCounts {
            classified: evidence(self.decisions.len() as u128),
            admitted: evidence(admitted),
            rejected: evidence(rejected),
        })
    }

    pub(crate) fn finish(
        self,
        relation: &RelationCatalog,
    ) -> Result<AdmissionCatalog, RelationClassificationError> {
        if relation.relation_id() != self.relation_id {
            return Err(RelationClassificationError::RelationIdentityMismatch);
        }
        let missing = relation
            .cases()
            .filter(|case| !self.decisions.contains_key(&case.case_id()))
            .count();
        if missing != 0 || self.decisions.len() != relation.case_count() {
            return Err(RelationClassificationError::AdmissionIncomplete {
                missing,
                unexpected: self.decisions.len().saturating_sub(relation.case_count()),
            });
        }
        let content_root = AdmissionContentRoot(hash_classification_state(
            ADMISSION_CONTENT_ROOT_HASH_V1,
            self.admission_id.bytes(),
            ClassificationUpstreamRoot::Content(relation.content_root().bytes()),
            &self.decisions,
            AdmissionDecision::canonical_tag,
        ));
        Ok(AdmissionCatalog {
            relation_id: self.relation_id,
            admission_id: self.admission_id,
            content_root,
            decisions: self.decisions,
        })
    }

    /// Validate exact admission coverage over a borrowed closed relation and
    /// commit the decision map without cloning it.
    pub(crate) fn close_borrowed<'a>(
        &'a self,
        relation: &ClosedRelationCatalogRef<'_>,
    ) -> Result<ClosedAdmissionCatalogRef<'a>, RelationClassificationError> {
        if relation.relation_id() != self.relation_id {
            return Err(RelationClassificationError::RelationIdentityMismatch);
        }
        let missing = relation
            .builder
            .case_claims
            .keys()
            .filter(|case_id| !self.decisions.contains_key(*case_id))
            .count();
        let relation_case_count = relation.builder.case_claims.len();
        if missing != 0 || self.decisions.len() != relation_case_count {
            return Err(RelationClassificationError::AdmissionIncomplete {
                missing,
                unexpected: self.decisions.len().saturating_sub(relation_case_count),
            });
        }
        let content_root = AdmissionContentRoot(hash_classification_state(
            ADMISSION_CONTENT_ROOT_HASH_V1,
            self.admission_id.bytes(),
            ClassificationUpstreamRoot::Content(relation.content_root.bytes()),
            &self.decisions,
            AdmissionDecision::canonical_tag,
        ));
        Ok(ClosedAdmissionCatalogRef {
            relation_id: self.relation_id,
            admission_id: self.admission_id,
            relation_content_root: relation.content_root,
            content_root,
            decisions: &self.decisions,
        })
    }
}

/// Borrowed closure capability for a complete admission decision map.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ClosedAdmissionCatalogRef<'a> {
    relation_id: RelationId,
    admission_id: AdmissionId,
    relation_content_root: RelationContentRoot,
    content_root: AdmissionContentRoot,
    decisions: &'a BTreeMap<RelationalCaseId, AdmissionDecision>,
}

impl ClosedAdmissionCatalogRef<'_> {
    fn admitted_case_ids(&self) -> impl Iterator<Item = RelationalCaseId> + '_ {
        self.decisions.iter().filter_map(|(case_id, decision)| {
            matches!(decision, AdmissionDecision::Admitted).then_some(*case_id)
        })
    }
}

/// Exact immutable admission relation after the base relation and every
/// admission decision have closed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AdmissionCatalog {
    relation_id: RelationId,
    admission_id: AdmissionId,
    content_root: AdmissionContentRoot,
    decisions: BTreeMap<RelationalCaseId, AdmissionDecision>,
}

impl AdmissionCatalog {
    pub(crate) const fn relation_id(&self) -> RelationId {
        self.relation_id
    }

    pub(crate) const fn admission_id(&self) -> AdmissionId {
        self.admission_id
    }

    pub(crate) const fn content_root(&self) -> AdmissionContentRoot {
        self.content_root
    }

    pub(crate) fn decision(&self, case_id: RelationalCaseId) -> Option<AdmissionDecision> {
        self.decisions.get(&case_id).copied()
    }

    pub(crate) fn admitted_case_ids(&self) -> impl Iterator<Item = RelationalCaseId> + '_ {
        self.decisions.iter().filter_map(|(case_id, decision)| {
            matches!(decision, AdmissionDecision::Admitted).then_some(*case_id)
        })
    }

    pub(crate) fn counts(&self) -> AdmissionCounts {
        let admitted = self.admitted_case_ids().count() as u128;
        let rejected = self.decisions.len() as u128 - admitted;
        AdmissionCounts {
            classified: RelationCountEvidence::Exact(self.decisions.len() as u128),
            admitted: RelationCountEvidence::Exact(admitted),
            rejected: RelationCountEvidence::Exact(rejected),
        }
    }
}

/// Incremental selection evidence over one exact admitted relation.
#[derive(Clone, Debug)]
pub(crate) struct QuestionCatalogBuilder {
    relation_id: RelationId,
    admission_id: AdmissionId,
    question_id: QuestionId,
    decisions: BTreeMap<RelationalCaseId, SelectionDecision>,
    /// Operational discovery order rebuilt from `QuestionClassified` journal
    /// events. It deliberately does not participate in frontier/content
    /// roots: answer identity remains arrival-order independent while bounded
    /// downstream schedulers avoid treating hash-ordered CaseIds as a
    /// monotone discovery cursor.
    selected_discovery_order: Vec<RelationalCaseId>,
}

/// One already authenticated sparse selected-case insertion.
///
/// The claimed identities remain part of the input so the batch boundary can
/// reject a mismatched producer transcript before mutating any catalog.
#[derive(Clone, Debug)]
pub(crate) struct SelectedCaseBatchRow {
    source_key: SourceKey,
    source: SourceRow,
    successor_key: SuccessorKey,
    successor: SuccessorRow,
    case_id: RelationalCaseId,
}

impl SelectedCaseBatchRow {
    pub(crate) fn new(
        source_key: SourceKey,
        source: SourceRow,
        successor_key: SuccessorKey,
        successor: SuccessorRow,
        case_id: RelationalCaseId,
    ) -> Self {
        Self {
            source_key,
            source,
            successor_key,
            successor,
            case_id,
        }
    }
}

/// Validate and install one bounded sparse selected-case delta atomically.
///
/// Validation builds only a batch-local relation overlay. Every collision,
/// identity, closure, and classification check completes before the committed
/// relation/admission/question prefixes are touched. The final merge therefore
/// has no semantic failure path and is proportional to the incoming batch,
/// rather than cloning all previously selected cases for every journal event.
pub(crate) fn install_selected_case_batch(
    relation: &mut RelationCatalogBuilder,
    admission: &mut AdmissionCatalogBuilder,
    question: &mut QuestionCatalogBuilder,
    rows: impl IntoIterator<Item = SelectedCaseBatchRow>,
) -> Result<(), SelectedCaseBatchError> {
    if admission.relation_id != relation.relation_id || question.relation_id != relation.relation_id
    {
        return Err(RelationClassificationError::RelationIdentityMismatch.into());
    }
    if question.admission_id != admission.admission_id {
        return Err(RelationClassificationError::AdmissionIdentityMismatch.into());
    }
    if relation.source_enumeration_closed {
        return Err(RelationCatalogError::SourceEnumerationClosed.into());
    }

    let rows = rows.into_iter().collect::<Vec<_>>();
    let mut delta = RelationCatalogBuilder::new(relation.relation_id);
    let mut case_ids = Vec::with_capacity(rows.len());
    let mut unique_case_ids = BTreeSet::new();
    for row in rows {
        let derived_source_key = SourceKey::derive(relation.relation_id, &row.source);
        if derived_source_key != row.source_key {
            return Err(SelectedCaseBatchError::SourceKeyClaimMismatch {
                claimed: row.source_key,
                derived: derived_source_key,
            });
        }
        let derived_successor_key =
            SuccessorKey::derive(relation.relation_id, row.source_key, &row.successor);
        if derived_successor_key != row.successor_key {
            return Err(SelectedCaseBatchError::SuccessorKeyClaimMismatch {
                claimed: row.successor_key,
                derived: derived_successor_key,
            });
        }
        let derived_case_id =
            RelationalCaseId::derive(relation.relation_id, row.source_key, row.successor_key);
        if derived_case_id != row.case_id {
            return Err(SelectedCaseBatchError::CaseIdClaimMismatch {
                claimed: row.case_id,
                derived: derived_case_id,
            });
        }
        if !unique_case_ids.insert(row.case_id) {
            return Err(SelectedCaseBatchError::DuplicateCase {
                case_id: row.case_id,
            });
        }
        if relation.case_claims.contains_key(&row.case_id) {
            return Err(SelectedCaseBatchError::CaseAlreadyPresent {
                case_id: row.case_id,
            });
        }

        if let Some(existing) = relation.sources.get(&row.source_key) {
            if !existing.row.has_same_content(&row.source) {
                return Err(RelationCatalogError::SourceKeyCollision {
                    source_key: row.source_key,
                }
                .into());
            }
            if existing.successor_enumeration_closed {
                return Err(RelationCatalogError::SuccessorEnumerationClosed {
                    source_key: row.source_key,
                }
                .into());
            }
            if let Some(existing) = existing.successors.get(&row.successor_key) {
                if !existing.row.has_same_content(&row.successor) {
                    return Err(RelationCatalogError::SuccessorKeyCollision {
                        successor_key: row.successor_key,
                    }
                    .into());
                }
            }
        }
        if let Some((claimed_source, claimed_after)) =
            relation.successor_claims.get(&row.successor_key)
        {
            if *claimed_source != row.source_key || claimed_after != row.successor.after() {
                return Err(RelationCatalogError::SuccessorKeyCollision {
                    successor_key: row.successor_key,
                }
                .into());
            }
        }
        if let Some((claimed_source, claimed_successor)) = relation.case_claims.get(&row.case_id) {
            if *claimed_source != row.source_key || *claimed_successor != row.successor_key {
                return Err(RelationCatalogError::CaseIdCollision {
                    case_id: row.case_id,
                }
                .into());
            }
        }
        if let Some(existing) = admission.decision(row.case_id) {
            return Err(match existing {
                AdmissionDecision::Admitted => SelectedCaseBatchError::CaseAlreadyPresent {
                    case_id: row.case_id,
                },
                AdmissionDecision::Rejected => {
                    RelationClassificationError::AdmissionDecisionConflict {
                        case_id: row.case_id,
                    }
                    .into()
                }
            });
        }
        if question.decision(row.case_id).is_some() {
            return Err(RelationClassificationError::SelectionDecisionConflict {
                case_id: row.case_id,
            }
            .into());
        }

        let source_key = delta.insert_source(row.source)?;
        let (successor_key, case_id) = delta.insert_successor(source_key, row.successor)?;
        debug_assert_eq!(source_key, row.source_key);
        debug_assert_eq!(successor_key, row.successor_key);
        debug_assert_eq!(case_id, row.case_id);
        case_ids.push(case_id);
    }

    // Capacity is operational, not semantic. Reserve before the move-only
    // commits so a recoverable allocation refusal cannot expose a partial
    // three-catalog transaction.
    question
        .selected_discovery_order
        .try_reserve(case_ids.len())
        .map_err(|_| SelectedCaseBatchError::AllocationFailed)?;

    relation.apply_selected_delta(delta);
    for case_id in case_ids {
        let previous = admission
            .decisions
            .insert(case_id, AdmissionDecision::Admitted);
        debug_assert!(previous.is_none());
        admission.admitted_count += 1;
        let previous = question
            .decisions
            .insert(case_id, SelectionDecision::Selected);
        debug_assert!(previous.is_none());
        question.selected_discovery_order.push(case_id);
    }
    Ok(())
}

impl QuestionCatalogBuilder {
    pub(crate) fn new(
        relation_id: RelationId,
        admission_id: AdmissionId,
        question_id: QuestionId,
    ) -> Self {
        Self {
            relation_id,
            admission_id,
            question_id,
            decisions: BTreeMap::new(),
            selected_discovery_order: Vec::new(),
        }
    }

    pub(crate) const fn relation_id(&self) -> RelationId {
        self.relation_id
    }

    pub(crate) const fn admission_id(&self) -> AdmissionId {
        self.admission_id
    }

    pub(crate) const fn question_id(&self) -> QuestionId {
        self.question_id
    }

    pub(crate) fn decision(&self, case_id: RelationalCaseId) -> Option<SelectionDecision> {
        self.decisions.get(&case_id).copied()
    }

    pub(crate) fn decision_count(&self) -> usize {
        self.decisions.len()
    }

    pub(crate) fn selected_count(&self) -> usize {
        self.selected_discovery_order.len()
    }

    /// Selected CaseIds in journal discovery order, starting at an
    /// invocation-local ordinal. This is an operational readiness index, not
    /// a canonical answer ordering or closure receipt.
    pub(crate) fn selected_discovery_suffix(&self, from_ordinal: usize) -> &[RelationalCaseId] {
        &self.selected_discovery_order[from_ordinal..]
    }

    /// Borrow the canonical selected CaseId order without cloning or closing
    /// the incremental FIND catalog. Post-FIND schedulers may consume this
    /// only after an authenticated selected-question seal exists; the
    /// iterator itself is not closure authority.
    pub(crate) fn selected_case_ids(&self) -> impl Iterator<Item = RelationalCaseId> + '_ {
        self.decisions.iter().filter_map(|(case_id, decision)| {
            matches!(decision, SelectionDecision::Selected).then_some(*case_id)
        })
    }

    /// Commit this FIND prefix over an already authenticated admission
    /// frontier without rebuilding the admission decision map.
    pub(crate) fn frontier_root(
        &self,
        admission_frontier_root: AdmissionFrontierRoot,
    ) -> QuestionFrontierRoot {
        QuestionFrontierRoot(hash_classification_state(
            QUESTION_FRONTIER_ROOT_HASH_V1,
            self.question_id.bytes(),
            ClassificationUpstreamRoot::Frontier(admission_frontier_root.bytes()),
            &self.decisions,
            SelectionDecision::canonical_tag,
        ))
    }

    /// Commit an open FIND prefix after admission has closed. The root binds
    /// the exact admitted content but remains a frontier commitment until the
    /// FIND decision map itself passes [`Self::finish`].
    pub(crate) fn frontier_root_over_closed_admission(
        &self,
        admission_content_root: AdmissionContentRoot,
    ) -> QuestionFrontierRoot {
        QuestionFrontierRoot(hash_classification_state(
            QUESTION_FRONTIER_ROOT_HASH_V1,
            self.question_id.bytes(),
            ClassificationUpstreamRoot::Content(admission_content_root.bytes()),
            &self.decisions,
            SelectionDecision::canonical_tag,
        ))
    }

    pub(crate) fn classify(
        &mut self,
        relation: &RelationCatalogSnapshot,
        admission: &AdmissionCatalogBuilder,
        case_id: RelationalCaseId,
        decision: SelectionDecision,
    ) -> Result<bool, RelationClassificationError> {
        self.validate_open_inputs(relation, admission)?;
        self.classify_admitted_case(admission, case_id, decision)
    }

    /// Classify against the mutable relation and admission frontiers using
    /// only keyed lookups. This keeps one journal event proportional to the
    /// affected records instead of rebuilding the canonical relation prefix.
    pub(crate) fn classify_open(
        &mut self,
        relation: &RelationCatalogBuilder,
        admission: &AdmissionCatalogBuilder,
        case_id: RelationalCaseId,
        decision: SelectionDecision,
    ) -> Result<bool, RelationClassificationError> {
        let insert = self.preflight_classify_open(relation, admission, case_id, decision)?;
        Ok(self.commit_preflight_classification(case_id, decision, insert))
    }

    /// Validate one FIND decision without mutating its layer. This keeps the
    /// question and semantic M-support commits behind one shared preflight.
    pub(crate) fn preflight_classify_open(
        &self,
        relation: &RelationCatalogBuilder,
        admission: &AdmissionCatalogBuilder,
        case_id: RelationalCaseId,
        decision: SelectionDecision,
    ) -> Result<bool, RelationClassificationError> {
        self.validate_open_builder_inputs(relation, admission)?;
        if !relation.contains_case(case_id) {
            return Err(RelationClassificationError::UnknownCase { case_id });
        }
        match admission.decision(case_id) {
            Some(AdmissionDecision::Admitted) => {}
            Some(AdmissionDecision::Rejected) => {
                return Err(RelationClassificationError::SelectionForRejectedCase { case_id });
            }
            None => return Err(RelationClassificationError::UnknownCase { case_id }),
        }
        match self.decisions.get(&case_id) {
            Some(existing) if *existing == decision => Ok(false),
            Some(_) => Err(RelationClassificationError::SelectionDecisionConflict { case_id }),
            None => Ok(true),
        }
    }

    pub(crate) fn commit_preflight_classification(
        &mut self,
        case_id: RelationalCaseId,
        decision: SelectionDecision,
        insert: bool,
    ) -> bool {
        if !insert {
            return false;
        }
        let previous = self.decisions.insert(case_id, decision);
        debug_assert!(previous.is_none());
        if decision == SelectionDecision::Selected {
            self.selected_discovery_order.push(case_id);
        }
        true
    }

    fn classify_admitted_case(
        &mut self,
        admission: &AdmissionCatalogBuilder,
        case_id: RelationalCaseId,
        decision: SelectionDecision,
    ) -> Result<bool, RelationClassificationError> {
        match admission.decision(case_id) {
            Some(AdmissionDecision::Admitted) => {}
            Some(AdmissionDecision::Rejected) => {
                return Err(RelationClassificationError::SelectionForRejectedCase { case_id });
            }
            None => return Err(RelationClassificationError::UnknownCase { case_id }),
        }
        match self.decisions.get(&case_id) {
            Some(existing) if *existing == decision => Ok(false),
            Some(_) => Err(RelationClassificationError::SelectionDecisionConflict { case_id }),
            None => {
                self.decisions.insert(case_id, decision);
                if decision == SelectionDecision::Selected {
                    self.selected_discovery_order.push(case_id);
                }
                Ok(true)
            }
        }
    }

    /// Select every currently admitted case for predicate-free `find all`.
    ///
    /// This operation is intentionally repeatable as source and successor
    /// frontiers advance. It does not impose a global admission barrier and it
    /// does not invent an always-true predicate.
    pub(crate) fn classify_all_observed(
        &mut self,
        relation: &RelationCatalogSnapshot,
        admission: &AdmissionCatalogBuilder,
    ) -> Result<usize, RelationClassificationError> {
        self.validate_open_inputs(relation, admission)?;
        let mut inserted = 0usize;
        for case_id in admission.admitted_case_ids() {
            if relation.case(case_id).is_none() {
                return Err(RelationClassificationError::UnknownCase { case_id });
            }
            match self.decisions.get(&case_id) {
                Some(SelectionDecision::Selected) => {}
                Some(SelectionDecision::NotSelected) => {
                    return Err(RelationClassificationError::SelectionDecisionConflict { case_id });
                }
                None => {
                    self.decisions.insert(case_id, SelectionDecision::Selected);
                    self.selected_discovery_order.push(case_id);
                    inserted += 1;
                }
            }
        }
        Ok(inserted)
    }

    pub(crate) fn counts(&self) -> SelectionCounts {
        let selected = self.selected_discovery_order.len() as u128;
        let not_selected = self.decisions.len() as u128 - selected;
        SelectionCounts {
            classified: RelationCountEvidence::LowerBound(self.decisions.len() as u128),
            selected: RelationCountEvidence::LowerBound(selected),
            not_selected: RelationCountEvidence::LowerBound(not_selected),
        }
    }

    /// Report the FIND prefix without imposing a global admission phase.
    /// Exactness requires closed relation enumeration, complete admission, and
    /// one FIND decision for every admitted case.
    pub(crate) fn counts_at(
        &self,
        relation: &RelationCatalogSnapshot,
        admission: &AdmissionCatalogBuilder,
    ) -> Result<SelectionCounts, RelationClassificationError> {
        self.validate_open_inputs(relation, admission)?;
        let selected = self.selected_discovery_order.len() as u128;
        let not_selected = self.decisions.len() as u128 - selected;
        let admission_complete = relation.enumeration_is_complete()
            && admission.decisions.len() == relation.cases().count();
        let complete = admission_complete && self.decisions.len() == admission.admitted_count();
        let evidence = |value| {
            if complete {
                RelationCountEvidence::Exact(value)
            } else {
                RelationCountEvidence::LowerBound(value)
            }
        };
        Ok(SelectionCounts {
            classified: evidence(self.decisions.len() as u128),
            selected: evidence(selected),
            not_selected: evidence(not_selected),
        })
    }

    pub(crate) fn finish(
        self,
        relation: &RelationCatalog,
        admission: &AdmissionCatalog,
    ) -> Result<QuestionCatalog, RelationClassificationError> {
        self.validate_closed_inputs(relation, admission)?;
        let admitted = admission.admitted_case_ids().collect::<BTreeSet<_>>();
        let missing = admitted
            .iter()
            .filter(|case_id| !self.decisions.contains_key(case_id))
            .count();
        let unexpected = self
            .decisions
            .keys()
            .filter(|case_id| !admitted.contains(case_id))
            .count();
        if missing != 0 || unexpected != 0 {
            return Err(RelationClassificationError::SelectionIncomplete {
                missing,
                unexpected,
            });
        }
        let content_root = QuestionContentRoot(hash_classification_state(
            QUESTION_CONTENT_ROOT_HASH_V1,
            self.question_id.bytes(),
            ClassificationUpstreamRoot::Content(admission.content_root().bytes()),
            &self.decisions,
            SelectionDecision::canonical_tag,
        ));
        Ok(QuestionCatalog {
            relation_id: self.relation_id,
            admission_id: self.admission_id,
            question_id: self.question_id,
            content_root,
            decisions: self.decisions,
        })
    }

    /// Validate exact FIND coverage over borrowed closed upstream catalogs and
    /// commit the decision map without cloning it or collecting admitted IDs.
    pub(crate) fn close_borrowed<'a>(
        &'a self,
        relation: &ClosedRelationCatalogRef<'_>,
        admission: &ClosedAdmissionCatalogRef<'_>,
    ) -> Result<ClosedQuestionCatalogRef<'a>, RelationClassificationError> {
        if relation.relation_id() != self.relation_id
            || admission.relation_id != self.relation_id
            || admission.relation_content_root != relation.content_root()
        {
            return Err(RelationClassificationError::RelationIdentityMismatch);
        }
        if admission.admission_id != self.admission_id {
            return Err(RelationClassificationError::AdmissionIdentityMismatch);
        }
        let missing = admission
            .admitted_case_ids()
            .filter(|case_id| !self.decisions.contains_key(case_id))
            .count();
        let unexpected = self
            .decisions
            .keys()
            .filter(|case_id| {
                admission.decisions.get(*case_id) != Some(&AdmissionDecision::Admitted)
            })
            .count();
        if missing != 0 || unexpected != 0 {
            return Err(RelationClassificationError::SelectionIncomplete {
                missing,
                unexpected,
            });
        }
        let content_root = QuestionContentRoot(hash_classification_state(
            QUESTION_CONTENT_ROOT_HASH_V1,
            self.question_id.bytes(),
            ClassificationUpstreamRoot::Content(admission.content_root.bytes()),
            &self.decisions,
            SelectionDecision::canonical_tag,
        ));
        let selected_count = self
            .decisions
            .values()
            .filter(|decision| **decision == SelectionDecision::Selected)
            .count() as u128;
        Ok(ClosedQuestionCatalogRef {
            question_id: self.question_id,
            content_root,
            selected_count,
            decisions: &self.decisions,
        })
    }

    fn validate_open_inputs(
        &self,
        relation: &RelationCatalogSnapshot,
        admission: &AdmissionCatalogBuilder,
    ) -> Result<(), RelationClassificationError> {
        if relation.relation_id() != self.relation_id || admission.relation_id != self.relation_id {
            return Err(RelationClassificationError::RelationIdentityMismatch);
        }
        if admission.admission_id != self.admission_id {
            return Err(RelationClassificationError::AdmissionIdentityMismatch);
        }
        Ok(())
    }

    fn validate_open_builder_inputs(
        &self,
        relation: &RelationCatalogBuilder,
        admission: &AdmissionCatalogBuilder,
    ) -> Result<(), RelationClassificationError> {
        if relation.relation_id() != self.relation_id || admission.relation_id != self.relation_id {
            return Err(RelationClassificationError::RelationIdentityMismatch);
        }
        if admission.admission_id != self.admission_id {
            return Err(RelationClassificationError::AdmissionIdentityMismatch);
        }
        Ok(())
    }

    fn validate_closed_inputs(
        &self,
        relation: &RelationCatalog,
        admission: &AdmissionCatalog,
    ) -> Result<(), RelationClassificationError> {
        if relation.relation_id() != self.relation_id || admission.relation_id != self.relation_id {
            return Err(RelationClassificationError::RelationIdentityMismatch);
        }
        if admission.admission_id != self.admission_id {
            return Err(RelationClassificationError::AdmissionIdentityMismatch);
        }
        Ok(())
    }
}

/// Borrowed closure capability for a complete FIND decision map.
///
/// Selected IDs are yielded directly from the map's canonical CaseId order;
/// callers therefore need no owned set to derive downstream commitments.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ClosedQuestionCatalogRef<'a> {
    question_id: QuestionId,
    content_root: QuestionContentRoot,
    selected_count: u128,
    decisions: &'a BTreeMap<RelationalCaseId, SelectionDecision>,
}

impl ClosedQuestionCatalogRef<'_> {
    pub(crate) const fn question_id(self) -> QuestionId {
        self.question_id
    }

    pub(crate) const fn content_root(self) -> QuestionContentRoot {
        self.content_root
    }

    pub(crate) const fn selected_count(self) -> u128 {
        self.selected_count
    }

    pub(crate) fn selected_case_ids(&self) -> impl Iterator<Item = RelationalCaseId> + '_ {
        self.decisions.iter().filter_map(|(case_id, decision)| {
            matches!(decision, SelectionDecision::Selected).then_some(*case_id)
        })
    }
}

/// Exact immutable selected relation after every admitted case is classified.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct QuestionCatalog {
    relation_id: RelationId,
    admission_id: AdmissionId,
    question_id: QuestionId,
    content_root: QuestionContentRoot,
    decisions: BTreeMap<RelationalCaseId, SelectionDecision>,
}

impl QuestionCatalog {
    pub(crate) const fn relation_id(&self) -> RelationId {
        self.relation_id
    }

    pub(crate) const fn admission_id(&self) -> AdmissionId {
        self.admission_id
    }

    pub(crate) const fn question_id(&self) -> QuestionId {
        self.question_id
    }

    pub(crate) const fn content_root(&self) -> QuestionContentRoot {
        self.content_root
    }

    pub(crate) fn decision(&self, case_id: RelationalCaseId) -> Option<SelectionDecision> {
        self.decisions.get(&case_id).copied()
    }

    pub(crate) fn selected_case_ids(&self) -> impl Iterator<Item = RelationalCaseId> + '_ {
        self.decisions.iter().filter_map(|(case_id, decision)| {
            matches!(decision, SelectionDecision::Selected).then_some(*case_id)
        })
    }

    pub(crate) fn counts(&self) -> SelectionCounts {
        let selected = self.selected_case_ids().count() as u128;
        let not_selected = self.decisions.len() as u128 - selected;
        SelectionCounts {
            classified: RelationCountEvidence::Exact(self.decisions.len() as u128),
            selected: RelationCountEvidence::Exact(selected),
            not_selected: RelationCountEvidence::Exact(not_selected),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationClassificationError {
    RelationIdentityMismatch,
    AdmissionIdentityMismatch,
    UnknownCase { case_id: RelationalCaseId },
    AdmissionDecisionConflict { case_id: RelationalCaseId },
    SelectionDecisionConflict { case_id: RelationalCaseId },
    SelectionForRejectedCase { case_id: RelationalCaseId },
    AdmissionIncomplete { missing: usize, unexpected: usize },
    SelectionIncomplete { missing: usize, unexpected: usize },
}

#[derive(Debug)]
pub(crate) enum SelectedCaseBatchError {
    Catalog(RelationCatalogError),
    Classification(RelationClassificationError),
    SourceKeyClaimMismatch {
        claimed: SourceKey,
        derived: SourceKey,
    },
    SuccessorKeyClaimMismatch {
        claimed: SuccessorKey,
        derived: SuccessorKey,
    },
    CaseIdClaimMismatch {
        claimed: RelationalCaseId,
        derived: RelationalCaseId,
    },
    DuplicateCase {
        case_id: RelationalCaseId,
    },
    CaseAlreadyPresent {
        case_id: RelationalCaseId,
    },
    AllocationFailed,
}

impl From<RelationCatalogError> for SelectedCaseBatchError {
    fn from(error: RelationCatalogError) -> Self {
        Self::Catalog(error)
    }
}

impl From<RelationClassificationError> for SelectedCaseBatchError {
    fn from(error: RelationClassificationError) -> Self {
        Self::Classification(error)
    }
}

impl fmt::Display for SelectedCaseBatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Catalog(error) => fmt::Display::fmt(error, formatter),
            Self::Classification(error) => fmt::Display::fmt(error, formatter),
            Self::SourceKeyClaimMismatch { .. } => {
                formatter.write_str("selected-case batch SourceKey claim mismatch")
            }
            Self::SuccessorKeyClaimMismatch { .. } => {
                formatter.write_str("selected-case batch SuccessorKey claim mismatch")
            }
            Self::CaseIdClaimMismatch { .. } => {
                formatter.write_str("selected-case batch CaseId claim mismatch")
            }
            Self::DuplicateCase { .. } => {
                formatter.write_str("selected-case batch contains a duplicate CaseId")
            }
            Self::CaseAlreadyPresent { .. } => {
                formatter.write_str("selected-case batch repeats a durable CaseId")
            }
            Self::AllocationFailed => {
                formatter.write_str("cannot reserve bounded selected-case discovery-order capacity")
            }
        }
    }
}

impl Error for SelectedCaseBatchError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Catalog(error) => Some(error),
            Self::Classification(error) => Some(error),
            _ => None,
        }
    }
}

impl fmt::Display for RelationClassificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RelationIdentityMismatch => {
                formatter.write_str("Explore classification relation identity mismatch")
            }
            Self::AdmissionIdentityMismatch => {
                formatter.write_str("Explore selection admission identity mismatch")
            }
            Self::UnknownCase { .. } => {
                formatter.write_str("Explore classification names an unknown relational CaseId")
            }
            Self::AdmissionDecisionConflict { .. } => formatter.write_str(
                "Explore admission evidence contradicts an existing CaseId classification",
            ),
            Self::SelectionDecisionConflict { .. } => formatter.write_str(
                "Explore selection evidence contradicts an existing CaseId classification",
            ),
            Self::SelectionForRejectedCase { .. } => formatter
                .write_str("Explore FIND selection cannot classify an admission-rejected case"),
            Self::AdmissionIncomplete { .. } => formatter.write_str(
                "Explore admission cannot close before every relational case is classified",
            ),
            Self::SelectionIncomplete { .. } => formatter
                .write_str("Explore FIND cannot close before every admitted case is classified"),
        }
    }
}

impl Error for RelationClassificationError {}

/// Borrowed extensional transition frame for one stable relational CaseId.
///
/// The existing transition interner can independently consume `context`,
/// `before`, and `after`; neither its `TransitionId` nor its deduplication is
/// coupled to the relational CaseId.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RelationalCaseRef<'a> {
    relation_id: RelationId,
    source_key: SourceKey,
    successor_key: SuccessorKey,
    case_id: RelationalCaseId,
    context: &'a ExploreValue,
    before: &'a ExploreValue,
    after: &'a ExploreValue,
}

impl<'a> RelationalCaseRef<'a> {
    fn new(
        relation_id: RelationId,
        source_key: SourceKey,
        successor_key: SuccessorKey,
        case_id: RelationalCaseId,
        source: &'a SourceRow,
        successor: &'a SuccessorRow,
    ) -> Self {
        Self {
            relation_id,
            source_key,
            successor_key,
            case_id,
            context: &source.context,
            before: &source.before,
            after: &successor.after,
        }
    }

    pub(crate) const fn relation_id(self) -> RelationId {
        self.relation_id
    }

    pub(crate) const fn source_key(self) -> SourceKey {
        self.source_key
    }

    pub(crate) const fn successor_key(self) -> SuccessorKey {
        self.successor_key
    }

    pub(crate) const fn case_id(self) -> RelationalCaseId {
        self.case_id
    }

    pub(crate) fn context(self) -> &'a ExploreValue {
        self.context
    }

    pub(crate) fn before(self) -> &'a ExploreValue {
        self.before
    }

    pub(crate) fn after(self) -> &'a ExploreValue {
        self.after
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationCatalogError {
    UnknownSource {
        source_key: SourceKey,
    },
    SourceEnumerationClosed,
    SuccessorEnumerationClosed {
        source_key: SourceKey,
    },
    EnumerationIncomplete {
        source_enumeration_open: bool,
        open_successor_enumerations: usize,
    },
    SourceKeyCollision {
        source_key: SourceKey,
    },
    SuccessorKeyCollision {
        successor_key: SuccessorKey,
    },
    CaseIdCollision {
        case_id: RelationalCaseId,
    },
}

impl fmt::Display for RelationCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSource { .. } => {
                formatter.write_str("relational successor names an unknown SourceKey")
            }
            Self::SourceEnumerationClosed => formatter
                .write_str("Explore source discovery continued after its enumeration was sealed"),
            Self::SuccessorEnumerationClosed { .. } => formatter.write_str(
                "Explore successor discovery continued after the source frontier was sealed",
            ),
            Self::EnumerationIncomplete { .. } => formatter.write_str(
                "Explore relation cannot finish while source or successor enumeration remains open",
            ),
            Self::SourceKeyCollision { .. } => formatter
                .write_str("Explore SourceKey SHA-256 collision rejected by relation catalog"),
            Self::SuccessorKeyCollision { .. } => formatter
                .write_str("Explore SuccessorKey SHA-256 collision rejected by relation catalog"),
            Self::CaseIdCollision { .. } => formatter
                .write_str("Explore relational CaseId SHA-256 collision rejected by catalog"),
        }
    }
}

impl Error for RelationCatalogError {}

fn derive_content_identity(domain: &[u8], preimage: &[u8]) -> [u8; 32] {
    let mut hasher = IdentityHasher::new(domain);
    hasher.bytes(preimage);
    hasher.finish()
}

struct IdentityHasher(Sha256);

impl IdentityHasher {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = Self(Sha256::new());
        hasher.bytes(domain);
        hasher
    }

    fn tag(&mut self, tag: u8) {
        self.0.update([tag]);
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.0.update((bytes.len() as u128).to_be_bytes());
        self.0.update(bytes);
    }

    fn digest(&mut self, digest: [u8; 32]) {
        self.0.update(digest);
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lineage(name: &str) -> RelationLineageId {
        RelationLineageId::from_canonical_preimage(name.as_bytes())
    }

    fn support(name: &str) -> RelationSupportId {
        RelationSupportId::from_canonical_preimage(name.as_bytes())
    }

    fn provenance(lineage_name: &str, support_name: &str) -> RelationProvenance {
        RelationProvenance::new([lineage(lineage_name)], [support(support_name)])
    }

    fn source(context: &str, before: i64, suffix: &str) -> SourceRow {
        SourceRow::new(
            ExploreValue::String(context.to_string()),
            ExploreValue::Int(before),
            provenance(&format!("lineage-{suffix}"), &format!("support-{suffix}")),
        )
    }

    fn successor(after: i64, suffix: &str) -> SuccessorRow {
        SuccessorRow::new(
            ExploreValue::Int(after),
            provenance(&format!("lineage-{suffix}"), &format!("support-{suffix}")),
        )
    }

    #[test]
    fn admission_identity_is_relation_scoped_and_canonical_digest_stable() {
        let relation_id = RelationId::from_canonical_semantic_preimage(b"shared-relation");
        let other_relation_id = RelationId::from_canonical_semantic_preimage(b"different-relation");
        let admission_preimage = b"before-supported;after-supported";
        let admission_digest = Sha256::digest(admission_preimage).into();

        let from_preimage =
            AdmissionId::from_canonical_admission_preimage(relation_id, admission_preimage);
        let from_digest =
            AdmissionId::from_canonical_admission_digest(relation_id, admission_digest);

        assert_eq!(from_preimage, from_digest);
        assert_eq!(from_preimage.bytes(), from_digest.bytes());
        assert_ne!(relation_id.bytes(), from_preimage.bytes());
        assert_ne!(
            from_preimage,
            AdmissionId::from_canonical_admission_preimage(other_relation_id, admission_preimage,)
        );
        assert_ne!(
            from_preimage,
            AdmissionId::from_canonical_admission_preimage(relation_id, b"after-supported")
        );
    }

    #[test]
    fn question_identity_is_scoped_by_admission_find_and_polarity() {
        let relation_id = RelationId::from_canonical_semantic_preimage(b"question-relation");
        let admission_id =
            AdmissionId::from_canonical_admission_preimage(relation_id, b"supported");
        let other_admission_id =
            AdmissionId::from_canonical_admission_preimage(relation_id, b"permitted");
        let find_preimage = b"resources(after) >= resources(before)";
        let find_digest = Sha256::digest(find_preimage).into();

        let from_preimage = QuestionId::from_canonical_find_preimage(
            admission_id,
            find_preimage,
            FindPolarity::Matches,
        );
        let from_digest = QuestionId::from_canonical_find_digest(
            admission_id,
            find_digest,
            FindPolarity::Matches,
        );

        assert_eq!(from_preimage, from_digest);
        assert_eq!(from_preimage.bytes(), from_digest.bytes());
        assert_ne!(admission_id.bytes(), from_preimage.bytes());
        assert_ne!(
            from_preimage,
            QuestionId::from_canonical_find_digest(
                admission_id,
                find_digest,
                FindPolarity::Violations,
            )
        );
        assert_ne!(
            from_preimage,
            QuestionId::from_canonical_find_preimage(
                admission_id,
                b"tax(after) < tax(before)",
                FindPolarity::Matches,
            )
        );
        assert_ne!(
            from_preimage,
            QuestionId::from_canonical_find_digest(
                other_admission_id,
                find_digest,
                FindPolarity::Matches,
            )
        );
        assert_ne!(
            from_preimage,
            QuestionId::from_canonical_find_digest(admission_id, find_digest, FindPolarity::All,)
        );
    }

    #[test]
    fn view_identity_seals_resolved_input_and_semantics_not_its_address() {
        let relation_id = RelationId::from_canonical_semantic_preimage(b"view-relation");
        let admission_id =
            AdmissionId::from_canonical_admission_preimage(relation_id, b"supported");
        let question_id = QuestionId::from_canonical_find_preimage(
            admission_id,
            b"resources-never-fall",
            FindPolarity::Violations,
        );
        let other_question_id = QuestionId::from_canonical_find_preimage(
            admission_id,
            b"tax-falls",
            FindPolarity::Matches,
        );

        let semantics = b"each-case;measure=loss;select=profile,income,loss";
        let first =
            ViewId::from_canonical_view_preimage(ViewInputId::Selected(question_id), semantics);
        let renamed_address =
            ViewId::from_canonical_view_preimage(ViewInputId::Selected(question_id), semantics);

        assert_eq!(first, renamed_address);
        assert_eq!(first.bytes(), renamed_address.bytes());
        assert_ne!(
            first,
            ViewId::from_canonical_view_preimage(
                ViewInputId::Selected(other_question_id),
                semantics,
            )
        );
        assert_ne!(
            first,
            ViewId::from_canonical_view_preimage(
                ViewInputId::Selected(question_id),
                b"group-by=income;aggregate=max(loss)",
            )
        );
    }

    #[test]
    fn mechanism_request_identity_separates_target_observer_and_normalization() {
        let relation_id = RelationId::from_canonical_semantic_preimage(b"mechanism-relation");
        let admission_id =
            AdmissionId::from_canonical_admission_preimage(relation_id, b"supported");
        let question_id = QuestionId::from_canonical_find_preimage(
            admission_id,
            b"resources-never-fall",
            FindPolarity::Violations,
        );
        let chosen_view = ViewId::from_canonical_view_preimage(
            ViewInputId::Selected(question_id),
            b"group-by=income;choose=max-loss",
        );

        let selected = MechanismRequestId::from_canonical_request_preimages(
            question_id,
            MechanismTargetId::Selected,
            b"assess-personskat",
            b"differential-signature-v1",
        );
        let selected_again = MechanismRequestId::from_canonical_request_preimages(
            question_id,
            MechanismTargetId::Selected,
            b"assess-personskat",
            b"differential-signature-v1",
        );

        assert_eq!(selected, selected_again);
        assert_eq!(selected.bytes(), selected_again.bytes());
        assert_ne!(
            selected,
            MechanismRequestId::from_canonical_request_preimages(
                question_id,
                MechanismTargetId::ChosenView(chosen_view),
                b"assess-personskat",
                b"differential-signature-v1",
            )
        );
        assert_ne!(
            selected,
            MechanismRequestId::from_canonical_request_preimages(
                question_id,
                MechanismTargetId::Selected,
                b"assess-disposable-income",
                b"differential-signature-v1",
            )
        );
        assert_ne!(
            selected,
            MechanismRequestId::from_canonical_request_preimages(
                question_id,
                MechanismTargetId::Selected,
                b"assess-personskat",
                b"differential-signature-v2",
            )
        );

        let evidence_view = ViewId::from_canonical_view_preimage(
            ViewInputId::MechanismIncidence(selected),
            b"group-by=loss-bin;aggregate=count-distinct-signatures",
        );
        assert_ne!(chosen_view, evidence_view);
    }

    #[test]
    fn admission_and_question_layers_do_not_rename_relation_cases() {
        let relation_id = RelationId::from_canonical_semantic_preimage(b"reusable-case-world");

        let mut first = RelationCatalogBuilder::new(relation_id);
        let first_source = first.insert_source(source("salary", 100, "first")).unwrap();
        let (first_successor, first_case) = first
            .insert_successor(first_source, successor(101, "first"))
            .unwrap();

        let first_admission =
            AdmissionId::from_canonical_admission_preimage(relation_id, b"supported");
        let first_question = QuestionId::from_canonical_find_preimage(
            first_admission,
            b"resources-never-fall",
            FindPolarity::Violations,
        );
        let second_admission =
            AdmissionId::from_canonical_admission_preimage(relation_id, b"supported-and-permitted");
        let second_question = QuestionId::from_canonical_find_preimage(
            second_admission,
            b"resources-never-fall",
            FindPolarity::Matches,
        );

        let mut second = RelationCatalogBuilder::new(relation_id);
        let second_source = second
            .insert_source(source("salary", 100, "different-lineage"))
            .unwrap();
        let (second_successor, second_case) = second
            .insert_successor(second_source, successor(101, "different-successor-lineage"))
            .unwrap();

        assert_ne!(first_admission, second_admission);
        assert_ne!(first_question, second_question);
        assert_eq!(first_source, second_source);
        assert_eq!(first_successor, second_successor);
        assert_eq!(first_case, second_case);
    }

    #[test]
    fn admission_and_selection_close_as_independent_case_relations() {
        let relation_id = RelationId::from_canonical_semantic_preimage(b"classified-relation");
        let admission_id =
            AdmissionId::from_canonical_admission_preimage(relation_id, b"supported");
        let question_id = QuestionId::from_canonical_find_preimage(
            admission_id,
            b"resources-never-fall",
            FindPolarity::Violations,
        );

        let mut relation = RelationCatalogBuilder::new(relation_id);
        let source_key = relation
            .insert_source(source("salary", 100, "source"))
            .unwrap();
        let (_, admitted_case) = relation
            .insert_successor(source_key, successor(101, "admitted"))
            .unwrap();
        let (_, rejected_case) = relation
            .insert_successor(source_key, successor(102, "rejected"))
            .unwrap();

        let snapshot = relation.snapshot();
        let mut admission = AdmissionCatalogBuilder::new(relation_id, admission_id);
        assert!(admission
            .classify(&snapshot, admitted_case, AdmissionDecision::Admitted)
            .unwrap());
        assert!(admission
            .classify(&snapshot, rejected_case, AdmissionDecision::Rejected)
            .unwrap());
        assert_eq!(
            admission.counts(),
            AdmissionCounts {
                classified: RelationCountEvidence::LowerBound(2),
                admitted: RelationCountEvidence::LowerBound(1),
                rejected: RelationCountEvidence::LowerBound(1),
            }
        );

        let mut question = QuestionCatalogBuilder::new(relation_id, admission_id, question_id);
        assert!(question
            .classify(
                &snapshot,
                &admission,
                admitted_case,
                SelectionDecision::Selected,
            )
            .unwrap());
        assert_eq!(
            question.classify(
                &snapshot,
                &admission,
                rejected_case,
                SelectionDecision::NotSelected,
            ),
            Err(RelationClassificationError::SelectionForRejectedCase {
                case_id: rejected_case,
            })
        );
        assert_eq!(
            question.counts_at(&snapshot, &admission).unwrap(),
            SelectionCounts {
                classified: RelationCountEvidence::LowerBound(1),
                selected: RelationCountEvidence::LowerBound(1),
                not_selected: RelationCountEvidence::LowerBound(0),
            }
        );

        relation.seal_successor_enumeration(source_key).unwrap();
        relation.seal_source_enumeration();
        let closed_snapshot = relation.snapshot();
        assert_eq!(
            admission.counts_at(&closed_snapshot).unwrap(),
            AdmissionCounts {
                classified: RelationCountEvidence::Exact(2),
                admitted: RelationCountEvidence::Exact(1),
                rejected: RelationCountEvidence::Exact(1),
            }
        );
        assert_eq!(
            question.counts_at(&closed_snapshot, &admission).unwrap(),
            SelectionCounts {
                classified: RelationCountEvidence::Exact(1),
                selected: RelationCountEvidence::Exact(1),
                not_selected: RelationCountEvidence::Exact(0),
            }
        );
        let relation = relation.finish().unwrap();
        let admission = admission.finish(&relation).unwrap();
        assert_eq!(
            admission.counts(),
            AdmissionCounts {
                classified: RelationCountEvidence::Exact(2),
                admitted: RelationCountEvidence::Exact(1),
                rejected: RelationCountEvidence::Exact(1),
            }
        );

        let question = question.finish(&relation, &admission).unwrap();
        assert_eq!(question.relation_id(), relation_id);
        assert_eq!(question.admission_id(), admission_id);
        assert_eq!(question.question_id(), question_id);
        assert_eq!(
            question.counts(),
            SelectionCounts {
                classified: RelationCountEvidence::Exact(1),
                selected: RelationCountEvidence::Exact(1),
                not_selected: RelationCountEvidence::Exact(0),
            }
        );
        assert_eq!(
            question.decision(admitted_case),
            Some(SelectionDecision::Selected)
        );
        assert_eq!(question.decision(rejected_case), None);
    }

    #[test]
    fn classification_roots_commit_case_membership_not_only_equal_counts() {
        let relation_id = RelationId::from_canonical_semantic_preimage(b"classification-members");
        let admission_id =
            AdmissionId::from_canonical_admission_preimage(relation_id, b"supported");
        let question_id = QuestionId::from_canonical_find_preimage(
            admission_id,
            b"matches",
            FindPolarity::Matches,
        );
        let mut relation = RelationCatalogBuilder::new(relation_id);
        let source_key = relation
            .insert_source(source("salary", 100, "source"))
            .unwrap();
        let (_, first_case) = relation
            .insert_successor(source_key, successor(101, "first"))
            .unwrap();
        let (_, second_case) = relation
            .insert_successor(source_key, successor(102, "second"))
            .unwrap();
        let snapshot = relation.snapshot();
        let relation_frontier_root = snapshot.frontier_root();

        let mut first_admitted = AdmissionCatalogBuilder::new(relation_id, admission_id);
        first_admitted
            .classify(&snapshot, first_case, AdmissionDecision::Admitted)
            .unwrap();
        first_admitted
            .classify(&snapshot, second_case, AdmissionDecision::Rejected)
            .unwrap();
        let mut second_admitted = AdmissionCatalogBuilder::new(relation_id, admission_id);
        second_admitted
            .classify(&snapshot, first_case, AdmissionDecision::Rejected)
            .unwrap();
        second_admitted
            .classify(&snapshot, second_case, AdmissionDecision::Admitted)
            .unwrap();
        assert_eq!(first_admitted.counts(), second_admitted.counts());
        assert_ne!(
            first_admitted.frontier_root(relation_frontier_root),
            second_admitted.frontier_root(relation_frontier_root)
        );

        let mut all_admitted = AdmissionCatalogBuilder::new(relation_id, admission_id);
        all_admitted
            .classify(&snapshot, first_case, AdmissionDecision::Admitted)
            .unwrap();
        all_admitted
            .classify(&snapshot, second_case, AdmissionDecision::Admitted)
            .unwrap();
        let admission_frontier_root = all_admitted.frontier_root(relation_frontier_root);
        let mut first_selected =
            QuestionCatalogBuilder::new(relation_id, admission_id, question_id);
        first_selected
            .classify(
                &snapshot,
                &all_admitted,
                first_case,
                SelectionDecision::Selected,
            )
            .unwrap();
        first_selected
            .classify(
                &snapshot,
                &all_admitted,
                second_case,
                SelectionDecision::NotSelected,
            )
            .unwrap();
        let mut second_selected =
            QuestionCatalogBuilder::new(relation_id, admission_id, question_id);
        second_selected
            .classify(
                &snapshot,
                &all_admitted,
                first_case,
                SelectionDecision::NotSelected,
            )
            .unwrap();
        second_selected
            .classify(
                &snapshot,
                &all_admitted,
                second_case,
                SelectionDecision::Selected,
            )
            .unwrap();
        assert_eq!(first_selected.counts(), second_selected.counts());
        assert_ne!(
            first_selected.frontier_root(admission_frontier_root),
            second_selected.frontier_root(admission_frontier_root)
        );

        relation.seal_successor_enumeration(source_key).unwrap();
        relation.seal_source_enumeration();
        let relation = relation.finish().unwrap();
        let first_admitted = first_admitted.finish(&relation).unwrap();
        let second_admitted = second_admitted.finish(&relation).unwrap();
        assert_ne!(
            first_admitted.content_root(),
            second_admitted.content_root()
        );
        let all_admitted = all_admitted.finish(&relation).unwrap();
        let first_selected = first_selected.finish(&relation, &all_admitted).unwrap();
        let second_selected = second_selected.finish(&relation, &all_admitted).unwrap();
        assert_ne!(
            first_selected.content_root(),
            second_selected.content_root()
        );
    }

    #[test]
    fn classification_roots_converge_across_arrival_orders() {
        let relation_id = RelationId::from_canonical_semantic_preimage(b"classification-order");
        let admission_id =
            AdmissionId::from_canonical_admission_preimage(relation_id, b"supported");
        let question_id = QuestionId::from_canonical_find_preimage(
            admission_id,
            b"matches",
            FindPolarity::Matches,
        );
        let mut relation = RelationCatalogBuilder::new(relation_id);
        let source_key = relation
            .insert_source(source("salary", 100, "source"))
            .unwrap();
        let (_, first_case) = relation
            .insert_successor(source_key, successor(101, "first"))
            .unwrap();
        let (_, second_case) = relation
            .insert_successor(source_key, successor(102, "second"))
            .unwrap();
        let snapshot = relation.snapshot();
        let relation_frontier_root = snapshot.frontier_root();

        let mut forward_admission = AdmissionCatalogBuilder::new(relation_id, admission_id);
        forward_admission
            .classify(&snapshot, first_case, AdmissionDecision::Admitted)
            .unwrap();
        forward_admission
            .classify(&snapshot, second_case, AdmissionDecision::Admitted)
            .unwrap();
        let mut reverse_admission = AdmissionCatalogBuilder::new(relation_id, admission_id);
        reverse_admission
            .classify(&snapshot, second_case, AdmissionDecision::Admitted)
            .unwrap();
        reverse_admission
            .classify(&snapshot, first_case, AdmissionDecision::Admitted)
            .unwrap();
        let forward_admission_root = forward_admission.frontier_root(relation_frontier_root);
        let reverse_admission_root = reverse_admission.frontier_root(relation_frontier_root);
        assert_eq!(forward_admission_root, reverse_admission_root);

        let mut forward_question =
            QuestionCatalogBuilder::new(relation_id, admission_id, question_id);
        forward_question
            .classify(
                &snapshot,
                &forward_admission,
                first_case,
                SelectionDecision::Selected,
            )
            .unwrap();
        forward_question
            .classify(
                &snapshot,
                &forward_admission,
                second_case,
                SelectionDecision::NotSelected,
            )
            .unwrap();
        let mut reverse_question =
            QuestionCatalogBuilder::new(relation_id, admission_id, question_id);
        reverse_question
            .classify(
                &snapshot,
                &reverse_admission,
                second_case,
                SelectionDecision::NotSelected,
            )
            .unwrap();
        reverse_question
            .classify(
                &snapshot,
                &reverse_admission,
                first_case,
                SelectionDecision::Selected,
            )
            .unwrap();
        assert_eq!(
            forward_question.frontier_root(forward_admission_root),
            reverse_question.frontier_root(reverse_admission_root)
        );

        relation.seal_successor_enumeration(source_key).unwrap();
        relation.seal_source_enumeration();
        let relation = relation.finish().unwrap();
        assert_eq!(
            forward_admission.frontier_root_over_closed_relation(relation.content_root()),
            reverse_admission.frontier_root_over_closed_relation(relation.content_root())
        );
        let forward_admission = forward_admission.finish(&relation).unwrap();
        let reverse_admission = reverse_admission.finish(&relation).unwrap();
        assert_eq!(
            forward_admission.content_root(),
            reverse_admission.content_root()
        );
        assert_eq!(
            forward_question.frontier_root_over_closed_admission(forward_admission.content_root()),
            reverse_question.frontier_root_over_closed_admission(reverse_admission.content_root())
        );
        let forward_question = forward_question
            .finish(&relation, &forward_admission)
            .unwrap();
        let reverse_question = reverse_question
            .finish(&relation, &reverse_admission)
            .unwrap();
        assert_eq!(
            forward_question.content_root(),
            reverse_question.content_root()
        );
    }

    #[test]
    fn find_all_selects_every_admitted_case_without_a_synthetic_predicate() {
        let relation_id = RelationId::from_canonical_semantic_preimage(b"find-all-relation");
        let admission_id = AdmissionId::from_canonical_admission_preimage(relation_id, b"all");
        let question_id = QuestionId::from_canonical_find_preimage(
            admission_id,
            b"predicate-free-find-all",
            FindPolarity::All,
        );

        let mut relation = RelationCatalogBuilder::new(relation_id);
        let source_key = relation
            .insert_source(source("compare", 50, "source"))
            .unwrap();
        let (_, first_case) = relation
            .insert_successor(source_key, successor(51, "first"))
            .unwrap();
        let (_, second_case) = relation
            .insert_successor(source_key, successor(52, "second"))
            .unwrap();
        let snapshot = relation.snapshot();

        let mut admission = AdmissionCatalogBuilder::new(relation_id, admission_id);
        admission
            .classify(&snapshot, first_case, AdmissionDecision::Admitted)
            .unwrap();
        admission
            .classify(&snapshot, second_case, AdmissionDecision::Admitted)
            .unwrap();
        let mut question = QuestionCatalogBuilder::new(relation_id, admission_id, question_id);
        assert_eq!(
            question
                .classify_all_observed(&snapshot, &admission)
                .unwrap(),
            2
        );
        assert_eq!(
            question
                .classify_all_observed(&snapshot, &admission)
                .unwrap(),
            0
        );

        relation.seal_successor_enumeration(source_key).unwrap();
        relation.seal_source_enumeration();
        let relation = relation.finish().unwrap();
        let admission = admission.finish(&relation).unwrap();
        let question = question.finish(&relation, &admission).unwrap();
        assert_eq!(
            question.selected_case_ids().collect::<BTreeSet<_>>(),
            BTreeSet::from([first_case, second_case])
        );
        assert_eq!(
            question.counts().selected(),
            RelationCountEvidence::Exact(2)
        );
    }

    #[test]
    fn catalog_and_ids_are_arrival_order_independent() {
        let relation_id = RelationId::from_canonical_semantic_preimage(b"income-cliff-relation");

        let mut forward = RelationCatalogBuilder::new(relation_id);
        let salary = forward.insert_source(source("salary", 100, "s1")).unwrap();
        forward
            .insert_successor(salary, successor(101, "a1"))
            .unwrap();
        forward
            .insert_successor(salary, successor(102, "a2"))
            .unwrap();
        let pension = forward.insert_source(source("pension", 200, "s2")).unwrap();
        forward
            .insert_successor(pension, successor(201, "a3"))
            .unwrap();
        forward.seal_successor_enumeration(salary).unwrap();
        forward.seal_successor_enumeration(pension).unwrap();
        forward.seal_source_enumeration();
        let forward = forward.finish().unwrap();

        let mut reverse = RelationCatalogBuilder::new(relation_id);
        let pension = reverse.insert_source(source("pension", 200, "s2")).unwrap();
        reverse
            .insert_successor(pension, successor(201, "a3"))
            .unwrap();
        let salary = reverse.insert_source(source("salary", 100, "s1")).unwrap();
        reverse
            .insert_successor(salary, successor(102, "a2"))
            .unwrap();
        reverse
            .insert_successor(salary, successor(101, "a1"))
            .unwrap();
        reverse.seal_successor_enumeration(salary).unwrap();
        reverse.seal_successor_enumeration(pension).unwrap();
        reverse.seal_source_enumeration();
        let reverse = reverse.finish().unwrap();

        assert_eq!(forward, reverse);
        assert_eq!(forward.content_root(), reverse.content_root());
    }

    #[test]
    fn duplicate_rows_collapse_and_union_lineage_and_support() {
        let relation_id = RelationId::from_canonical_semantic_preimage(b"set-semantics");
        let mut builder = RelationCatalogBuilder::new(relation_id);

        let first_source = builder
            .insert_source(source("salary", 100, "first"))
            .unwrap();
        let duplicate_source = builder
            .insert_source(source("salary", 100, "second"))
            .unwrap();
        assert_eq!(first_source, duplicate_source);

        let first_successor = builder
            .insert_successor(first_source, successor(101, "third"))
            .unwrap();
        let duplicate_successor = builder
            .insert_successor(first_source, successor(101, "fourth"))
            .unwrap();
        assert_eq!(first_successor, duplicate_successor);

        builder.seal_successor_enumeration(first_source).unwrap();
        builder.seal_source_enumeration();
        let catalog = builder.finish().unwrap();
        assert_eq!(catalog.source_count(), 1);
        assert_eq!(catalog.case_count(), 1);
        let source = &catalog.sources()[0];
        assert_eq!(source.row().provenance().lineage().len(), 2);
        assert_eq!(source.row().provenance().support().len(), 2);
        let successor = &source.successors()[0];
        assert_eq!(successor.row().provenance().lineage().len(), 2);
        assert_eq!(successor.row().provenance().support().len(), 2);
    }

    #[test]
    fn materially_distinct_actions_in_context_remain_distinct_cases() {
        let relation_id = RelationId::from_canonical_semantic_preimage(b"action-identity");
        let mut builder = RelationCatalogBuilder::new(relation_id);

        let salary = builder
            .insert_source(source("increase-salary", 100, "salary"))
            .unwrap();
        let pension = builder
            .insert_source(source("increase-pension", 100, "pension"))
            .unwrap();
        let (_, salary_case) = builder
            .insert_successor(salary, successor(101, "after-salary"))
            .unwrap();
        let (_, pension_case) = builder
            .insert_successor(pension, successor(101, "after-pension"))
            .unwrap();

        assert_ne!(salary, pension);
        assert_ne!(salary_case, pension_case);
        builder.seal_successor_enumeration(salary).unwrap();
        builder.seal_successor_enumeration(pension).unwrap();
        builder.seal_source_enumeration();
        let catalog = builder.finish().unwrap();
        assert_eq!(catalog.case_count(), 2);
        assert_eq!(
            catalog.case(salary_case).unwrap().before(),
            &ExploreValue::Int(100)
        );
        assert_eq!(
            catalog.case(salary_case).unwrap().after(),
            &ExploreValue::Int(101)
        );
        assert_ne!(
            catalog.case(salary_case).unwrap().context(),
            catalog.case(pension_case).unwrap().context()
        );
    }

    #[test]
    fn semantic_preimage_and_digest_produce_stable_relation_and_case_ids() {
        let semantic_preimage = b"canonical-source-and-successor-semantics";
        let semantic_digest = Sha256::digest(semantic_preimage).into();
        let from_preimage = RelationId::from_canonical_semantic_preimage(semantic_preimage);
        let from_digest = RelationId::from_canonical_semantic_digest(semantic_digest);
        assert_eq!(from_preimage, from_digest);

        let mut first = RelationCatalogBuilder::new(from_preimage);
        let first_source = first.insert_source(source("salary", 100, "one")).unwrap();
        let (first_successor, first_case) = first
            .insert_successor(first_source, successor(101, "two"))
            .unwrap();

        let mut second = RelationCatalogBuilder::new(from_digest);
        let second_source = second
            .insert_source(source("salary", 100, "different-lineage"))
            .unwrap();
        let (second_successor, second_case) = second
            .insert_successor(second_source, successor(101, "different-successor-lineage"))
            .unwrap();

        assert_eq!(first_source, second_source);
        assert_eq!(first_successor, second_successor);
        assert_eq!(first_case, second_case);
        first.seal_successor_enumeration(first_source).unwrap();
        first.seal_source_enumeration();
        second.seal_successor_enumeration(second_source).unwrap();
        second.seal_source_enumeration();
        assert_eq!(
            first.finish().unwrap().cases().next().unwrap().case_id(),
            second.finish().unwrap().cases().next().unwrap().case_id()
        );
    }

    #[test]
    fn partial_snapshot_reports_lower_bounds_before_total_cardinality_is_known() {
        let relation_id = RelationId::from_canonical_semantic_preimage(b"partial-frontier");
        let mut builder = RelationCatalogBuilder::new(relation_id);
        let source_key = builder
            .insert_source(source("salary", 100, "source"))
            .unwrap();
        let (_, case_id) = builder
            .insert_successor(source_key, successor(101, "successor"))
            .unwrap();

        let snapshot = builder.snapshot();
        assert!(!snapshot.source_enumeration_is_closed());
        assert!(!snapshot.enumeration_is_complete());
        assert_eq!(
            snapshot.counts().sources(),
            RelationCountEvidence::LowerBound(1)
        );
        assert_eq!(
            snapshot.counts().cases(),
            RelationCountEvidence::LowerBound(1)
        );
        assert_eq!(snapshot.open_source_keys(), &[source_key]);
        assert_eq!(
            snapshot
                .cases()
                .map(|case| case.case_id())
                .collect::<Vec<_>>(),
            vec![case_id]
        );
        assert!(matches!(
            builder.clone().finish(),
            Err(RelationCatalogError::EnumerationIncomplete {
                source_enumeration_open: true,
                open_successor_enumerations: 1,
            })
        ));
    }

    #[test]
    fn each_source_owns_an_independent_successor_frontier() {
        let relation_id = RelationId::from_canonical_semantic_preimage(b"per-source-frontier");
        let mut builder = RelationCatalogBuilder::new(relation_id);
        let first = builder.insert_source(source("first", 1, "first")).unwrap();
        let second = builder
            .insert_source(source("second", 2, "second"))
            .unwrap();

        assert!(builder.seal_successor_enumeration(first).unwrap());
        assert!(!builder.seal_successor_enumeration(first).unwrap());
        assert!(builder.successor_enumeration_is_closed(first).unwrap());
        assert!(!builder.successor_enumeration_is_closed(second).unwrap());
        assert_eq!(builder.open_source_keys().as_ref(), &[second]);

        builder.seal_source_enumeration();
        assert_eq!(builder.counts().sources(), RelationCountEvidence::Exact(2));
        assert_eq!(
            builder.counts().cases(),
            RelationCountEvidence::LowerBound(0)
        );
    }

    #[test]
    fn resume_can_discover_more_successors_without_renaming_committed_cases() {
        let relation_id = RelationId::from_canonical_semantic_preimage(b"resume-frontier");
        let mut builder = RelationCatalogBuilder::new(relation_id);
        let source_key = builder
            .insert_source(source("salary", 100, "source"))
            .unwrap();
        let (_, first_case) = builder
            .insert_successor(source_key, successor(101, "first"))
            .unwrap();
        let paused = builder.snapshot();
        let paused_root = paused.frontier_root();
        assert_eq!(
            paused.case(first_case).unwrap().after(),
            &ExploreValue::Int(101)
        );

        builder.seal_source_enumeration();
        let (_, second_case) = builder
            .insert_successor(source_key, successor(102, "second"))
            .unwrap();
        let resumed = builder.snapshot();
        assert_ne!(paused_root, resumed.frontier_root());
        assert_eq!(resumed.case(first_case).unwrap().case_id(), first_case);
        assert_eq!(
            resumed.case(second_case).unwrap().after(),
            &ExploreValue::Int(102)
        );
        assert_eq!(
            resumed.counts().cases(),
            RelationCountEvidence::LowerBound(2)
        );

        builder.seal_successor_enumeration(source_key).unwrap();
        let closed_frontier_root = builder.snapshot().frontier_root();
        assert_ne!(resumed.frontier_root(), closed_frontier_root);
        assert_eq!(builder.counts().cases(), RelationCountEvidence::Exact(2));
    }

    #[test]
    fn discovery_after_the_relevant_seal_fails_closed() {
        let relation_id = RelationId::from_canonical_semantic_preimage(b"sealed-frontier");
        let mut builder = RelationCatalogBuilder::new(relation_id);
        let source_key = builder
            .insert_source(source("salary", 100, "source"))
            .unwrap();
        builder
            .insert_successor(source_key, successor(101, "successor"))
            .unwrap();

        assert!(builder.seal_source_enumeration());
        assert!(!builder.seal_source_enumeration());
        assert!(matches!(
            builder.insert_source(source("salary", 100, "duplicate")),
            Err(RelationCatalogError::SourceEnumerationClosed)
        ));

        assert!(builder.seal_successor_enumeration(source_key).unwrap());
        assert!(!builder.seal_successor_enumeration(source_key).unwrap());
        assert!(matches!(
            builder.insert_successor(source_key, successor(101, "duplicate")),
            Err(RelationCatalogError::SuccessorEnumerationClosed {
                source_key: rejected,
            }) if rejected == source_key
        ));
    }

    #[test]
    fn closure_changes_exactness_without_changing_discovered_support() {
        let relation_id = RelationId::from_canonical_semantic_preimage(b"closure-conservation");
        let mut builder = RelationCatalogBuilder::new(relation_id);
        let empty = builder.insert_source(source("empty", 0, "empty")).unwrap();
        let populated = builder
            .insert_source(source("populated", 10, "populated"))
            .unwrap();
        let (_, first_case) = builder
            .insert_successor(populated, successor(11, "first"))
            .unwrap();
        let (_, second_case) = builder
            .insert_successor(populated, successor(12, "second"))
            .unwrap();
        let discovered = [first_case, second_case]
            .into_iter()
            .collect::<BTreeSet<_>>();

        assert_eq!(
            builder.counts().cases(),
            RelationCountEvidence::LowerBound(2)
        );
        builder.seal_source_enumeration();
        assert_eq!(builder.counts().sources(), RelationCountEvidence::Exact(2));
        builder.seal_successor_enumeration(empty).unwrap();
        assert_eq!(
            builder.counts().cases(),
            RelationCountEvidence::LowerBound(2)
        );
        builder.seal_successor_enumeration(populated).unwrap();
        assert!(builder.enumeration_is_complete());
        assert_eq!(builder.counts().cases(), RelationCountEvidence::Exact(2));

        let closed_snapshot = builder.snapshot();
        assert!(closed_snapshot.enumeration_is_complete());
        assert!(closed_snapshot.open_source_keys().is_empty());
        assert_eq!(
            closed_snapshot
                .cases()
                .map(|case| case.case_id())
                .collect::<BTreeSet<_>>(),
            discovered
        );

        let catalog = builder.finish().unwrap();
        assert_eq!(catalog.counts().sources(), RelationCountEvidence::Exact(2));
        assert_eq!(catalog.counts().cases(), RelationCountEvidence::Exact(2));
        assert_eq!(
            catalog
                .cases()
                .map(|case| case.case_id())
                .collect::<BTreeSet<_>>(),
            discovered
        );
    }
}
