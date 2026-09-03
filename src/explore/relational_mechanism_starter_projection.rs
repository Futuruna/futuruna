//! Authenticated, bounded projection of closed mechanism starter support.
//!
//! The support catalog remains the key-only semantic authority. This layer
//! binds that immutable authority to an independently checked result-view
//! authorization, resolves only one bounded page of typed case values at a
//! time, and accumulates a page-boundary-independent content root. It neither
//! registers an analysis layer nor publishes or persists the resulting rows.

use std::error::Error;
use std::fmt;
use std::num::NonZeroU16;

use sha2::{Digest, Sha256};

use super::mechanism_incidence::MechanismSignatureId;
use super::mechanism_support::{
    MechanismClosedSubjectStarterProjectionAuthority, MechanismCorrelatedSupportStatus,
    MechanismStarterProjectionPlanId, MechanismStarterSetStatus, MechanismSupportCatalogBuilder,
    MechanismSupportError, MechanismSupportFacet, MechanismSupportKey,
    MechanismSupportStarterCursor, MechanismSupportStarterMember, MechanismSupportSubject,
};
use super::relation::{
    MechanismTargetId, QuestionId, RelationId, RelationalCaseId, RelationalCaseRef, SourceKey,
    SuccessorKey, ViewId,
};
use super::relational_mechanism_starter_authorization::{
    RelationalMechanismStarterValueAuthorization, RelationalMechanismStarterValueAuthorizationId,
};
use super::structural_mechanism::{
    StructuralMechanismCatalogBuilder, StructuralMechanismId, StructuralQuotientClosureRoot,
};
use super::transition::{canonical_explore_value_digest, TransitionSchemaIdentities};
use super::ExploreValue;

pub(crate) const RELATIONAL_MECHANISM_STARTER_PROJECTION_VERSION: u32 = 3;

const JOB_ID_V3: &[u8] =
    b"futuruna.explore.relational-mechanism-subject-starter-projection.job-id.v3";
const MEMBER_ID_V1: &[u8] =
    b"futuruna.explore.relational-mechanism-starter-projection.member-id.v1";
const CONTENT_GENESIS_V1: &[u8] =
    b"futuruna.explore.relational-mechanism-starter-projection.content-genesis.v1";
const CONTENT_APPEND_V1: &[u8] =
    b"futuruna.explore.relational-mechanism-starter-projection.content-append.v1";
const PAGE_ROOT_V1: &[u8] =
    b"futuruna.explore.relational-mechanism-starter-projection.page-root.v1";
const PAGE_ID_V1: &[u8] = b"futuruna.explore.relational-mechanism-starter-projection.page-id.v1";
const PAGE_MANIFEST_GENESIS_V1: &[u8] =
    b"futuruna.explore.relational-mechanism-starter-projection.page-manifest-genesis.v1";
const PAGE_MANIFEST_APPEND_V1: &[u8] =
    b"futuruna.explore.relational-mechanism-starter-projection.page-manifest-append.v1";
const CLOSURE_ROOT_V3: &[u8] =
    b"futuruna.explore.relational-mechanism-subject-starter-projection.closure-root.v3";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalMechanismStarterProjectionJobId([u8; 32]);

impl RelationalMechanismStarterProjectionJobId {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalMechanismStarterProjectionMemberId([u8; 32]);

impl RelationalMechanismStarterProjectionMemberId {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Canonical semantic root of the ordered member stream. Unlike page roots,
/// this identity is independent of page size and resume boundaries.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalMechanismStarterProjectionContentRoot([u8; 32]);

impl RelationalMechanismStarterProjectionContentRoot {
    /// Restore a root only after the enclosing journal event/checkpoint has
    /// authenticated its bytes. Structural checkpoint invariants are checked
    /// separately by `restore_from_authenticated_checkpoint`.
    pub(super) const fn from_authenticated_checkpoint_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalMechanismStarterProjectionPageRoot([u8; 32]);

impl RelationalMechanismStarterProjectionPageRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalMechanismStarterProjectionPageId([u8; 32]);

impl RelationalMechanismStarterProjectionPageId {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Operational commitment to the exact page prefix accepted so far. It is
/// retained for resumability but deliberately excluded from semantic closure.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalMechanismStarterProjectionPageManifestRoot([u8; 32]);

impl RelationalMechanismStarterProjectionPageManifestRoot {
    /// Restore a root only after the enclosing journal event/checkpoint has
    /// authenticated its bytes.
    pub(super) const fn from_authenticated_checkpoint_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalMechanismStarterProjectionClosureRoot([u8; 32]);

impl RelationalMechanismStarterProjectionClosureRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Immutable projection job. The support plan remains authorization-neutral;
/// this identity becomes value-bearing only by binding the checked view
/// authorization and the producer's relation/schema identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalMechanismStarterProjectionJob {
    id: RelationalMechanismStarterProjectionJobId,
    authority: MechanismClosedSubjectStarterProjectionAuthority,
    authorization_id: RelationalMechanismStarterValueAuthorizationId,
    authorizing_question_id: QuestionId,
    authorizing_view_id: ViewId,
    role_schema_digest: [u8; 32],
    relation_id: RelationId,
    state_schema_id: [u8; 32],
    context_schema_id: [u8; 32],
    transition_type_id: [u8; 32],
}

impl RelationalMechanismStarterProjectionJob {
    pub(crate) fn new<A>(
        authority: A,
        relation_id: RelationId,
        schemas: &TransitionSchemaIdentities,
        authorization: &RelationalMechanismStarterValueAuthorization,
    ) -> Result<Self, RelationalMechanismStarterProjectionError>
    where
        A: Into<MechanismClosedSubjectStarterProjectionAuthority>,
    {
        let authority = authority.into();
        if !authorization.validate_identity() {
            return Err(RelationalMechanismStarterProjectionError::InvalidValueAuthorization);
        }
        // A compatible authorization is an unfiltered each-case view over
        // the complete selected population. It therefore covers both the
        // Selected target and every ChosenView subset of the same QuestionId;
        // requiring the chosen ViewId itself would reject precisely those
        // views because choice-bearing views are not lossless value sources.
        if authorization.question_id() != authority.question_id() {
            return Err(RelationalMechanismStarterProjectionError::ValueAuthorizationScopeMismatch);
        }
        let mut job = Self {
            id: RelationalMechanismStarterProjectionJobId([0; 32]),
            authority,
            authorization_id: authorization.authorization_id(),
            authorizing_question_id: authorization.question_id(),
            authorizing_view_id: authorization.view_id(),
            role_schema_digest: authorization.role_schema_digest(),
            relation_id,
            state_schema_id: schemas.state_schema_id().bytes(),
            context_schema_id: schemas.context_schema_id().bytes(),
            transition_type_id: schemas.transition_type_id().bytes(),
        };
        job.id = derive_job_id(job);
        Ok(job)
    }

    pub(crate) const fn id(self) -> RelationalMechanismStarterProjectionJobId {
        self.id
    }

    pub(crate) const fn authority(self) -> MechanismClosedSubjectStarterProjectionAuthority {
        self.authority
    }

    pub(crate) const fn key(self) -> MechanismSupportKey {
        self.authority.key()
    }

    pub(crate) const fn subject(self) -> MechanismSupportSubject {
        self.authority.subject()
    }

    pub(crate) const fn projection_plan_id(self) -> MechanismStarterProjectionPlanId {
        self.authority.projection_plan_id()
    }

    /// Temporary adapter for the whole-mechanism publisher. Subject-generic
    /// consumers must use [`Self::subject`] so node/edge facets remain
    /// explicit rather than being attributed to an arbitrary owner.
    pub(crate) fn mechanism_id(self) -> StructuralMechanismId {
        whole_mechanism_id(self.subject())
    }

    pub(crate) const fn authorization_id(self) -> RelationalMechanismStarterValueAuthorizationId {
        self.authorization_id
    }

    pub(crate) const fn relation_id(self) -> RelationId {
        self.relation_id
    }

    pub(crate) const fn authorizing_question_id(self) -> QuestionId {
        self.authorizing_question_id
    }

    pub(crate) const fn authorizing_view_id(self) -> ViewId {
        self.authorizing_view_id
    }

    pub(crate) const fn structural_root(self) -> StructuralQuotientClosureRoot {
        self.authority.structural_root()
    }

    /// Resolve one bounded canonical page against retained closed case values.
    /// The resolver should read the completed journal/materialization catalog;
    /// it is never asked to re-execute the query.
    pub(crate) fn derive_next_page<'case, F>(
        self,
        support: &MechanismSupportCatalogBuilder,
        structural: &StructuralMechanismCatalogBuilder,
        accumulator: &RelationalMechanismStarterProjectionAccumulator,
        maximum_members: NonZeroU16,
        mut resolve_case: F,
    ) -> Result<RelationalMechanismStarterProjectionPage, RelationalMechanismStarterProjectionError>
    where
        F: FnMut(RelationalCaseId) -> Option<RelationalCaseRef<'case>>,
    {
        if accumulator.job_id != self.id || accumulator.exhausted {
            return Err(RelationalMechanismStarterProjectionError::AccumulatorStateMismatch);
        }
        let support_page = support.closed_subject_starter_page(
            self.authority,
            structural,
            self.relation_id,
            accumulator.last_cursor,
            maximum_members,
        )?;
        let mut members = Vec::with_capacity(support_page.members().len());
        for key_member in support_page.members().iter().copied() {
            let case = resolve_case(key_member.case_id()).ok_or(
                RelationalMechanismStarterProjectionError::UnresolvedAuthorizedCase {
                    case_id: key_member.case_id(),
                },
            )?;
            members.push(derive_member(self, key_member, case)?);
        }
        if !support_page.exhausted() && members.is_empty() {
            return Err(RelationalMechanismStarterProjectionError::EmptyOpenPage);
        }
        let start_after = support_page.start_after();
        let end_cursor = support_page.end_cursor();
        let members = members.into_boxed_slice();
        let root = derive_page_root(
            self.id,
            accumulator.next_page_ordinal,
            start_after,
            end_cursor,
            support_page.exhausted(),
            &members,
        );
        let id = derive_page_id(self.id, accumulator.next_page_ordinal, root);
        Ok(RelationalMechanismStarterProjectionPage {
            id,
            root,
            job_id: self.id,
            page_ordinal: accumulator.next_page_ordinal,
            start_after,
            end_cursor,
            members,
            exhausted: support_page.exhausted(),
        })
    }
}

/// One bounded, typed mechanism-support incidence. A CaseId appears exactly
/// once because the raw signature fibers are a disjoint case partition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalMechanismStarterProjectionMember {
    id: RelationalMechanismStarterProjectionMemberId,
    raw_signature_id: MechanismSignatureId,
    case_id: RelationalCaseId,
    source_key: SourceKey,
    context: ExploreValue,
    before: ExploreValue,
    successor_key: SuccessorKey,
    after: ExploreValue,
}

impl RelationalMechanismStarterProjectionMember {
    pub(crate) const fn id(&self) -> RelationalMechanismStarterProjectionMemberId {
        self.id
    }

    pub(crate) const fn raw_signature_id(&self) -> MechanismSignatureId {
        self.raw_signature_id
    }

    pub(crate) const fn case_id(&self) -> RelationalCaseId {
        self.case_id
    }

    pub(crate) const fn source_key(&self) -> SourceKey {
        self.source_key
    }

    pub(crate) const fn successor_key(&self) -> SuccessorKey {
        self.successor_key
    }

    pub(crate) const fn cursor(&self) -> MechanismSupportStarterCursor {
        MechanismSupportStarterCursor::new(self.source_key, self.successor_key)
    }

    pub(crate) const fn context(&self) -> &ExploreValue {
        &self.context
    }

    pub(crate) const fn before(&self) -> &ExploreValue {
        &self.before
    }

    pub(crate) const fn after(&self) -> &ExploreValue {
        &self.after
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalMechanismStarterProjectionPage {
    id: RelationalMechanismStarterProjectionPageId,
    root: RelationalMechanismStarterProjectionPageRoot,
    job_id: RelationalMechanismStarterProjectionJobId,
    page_ordinal: u128,
    start_after: Option<MechanismSupportStarterCursor>,
    end_cursor: Option<MechanismSupportStarterCursor>,
    members: Box<[RelationalMechanismStarterProjectionMember]>,
    exhausted: bool,
}

impl RelationalMechanismStarterProjectionPage {
    pub(crate) const fn id(&self) -> RelationalMechanismStarterProjectionPageId {
        self.id
    }

    pub(crate) const fn root(&self) -> RelationalMechanismStarterProjectionPageRoot {
        self.root
    }

    pub(crate) const fn page_ordinal(&self) -> u128 {
        self.page_ordinal
    }

    pub(crate) const fn start_after(&self) -> Option<MechanismSupportStarterCursor> {
        self.start_after
    }

    pub(crate) const fn end_cursor(&self) -> Option<MechanismSupportStarterCursor> {
        self.end_cursor
    }

    pub(crate) fn members(&self) -> &[RelationalMechanismStarterProjectionMember] {
        &self.members
    }

    pub(crate) const fn exhausted(&self) -> bool {
        self.exhausted
    }
}

/// Resumable O(1) accumulator. Typed values live only in the current page;
/// checkpoint state retains canonical cursors and authenticated prefix roots.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalMechanismStarterProjectionAccumulator {
    job_id: RelationalMechanismStarterProjectionJobId,
    next_page_ordinal: u128,
    last_cursor: Option<MechanismSupportStarterCursor>,
    last_source_key: Option<SourceKey>,
    exact_member_count: u128,
    exact_starter_count: u128,
    content_root: RelationalMechanismStarterProjectionContentRoot,
    page_manifest_root: RelationalMechanismStarterProjectionPageManifestRoot,
    exhausted: bool,
}

impl RelationalMechanismStarterProjectionAccumulator {
    pub(crate) fn new(job: RelationalMechanismStarterProjectionJob) -> Self {
        Self {
            job_id: job.id,
            next_page_ordinal: 0,
            last_cursor: None,
            last_source_key: None,
            exact_member_count: 0,
            exact_starter_count: 0,
            content_root: derive_content_genesis(job.id),
            page_manifest_root: derive_page_manifest_genesis(job.id),
            exhausted: false,
        }
    }

    /// Restore an accumulator from an already authenticated journal
    /// checkpoint. Authentication proves the opaque roots came from the
    /// recorded prefix; this constructor independently checks every
    /// relationship recoverable from the compact state itself.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn restore_from_authenticated_checkpoint(
        job: RelationalMechanismStarterProjectionJob,
        next_page_ordinal: u128,
        last_cursor: Option<MechanismSupportStarterCursor>,
        last_source_key: Option<SourceKey>,
        exact_member_count: u128,
        exact_starter_count: u128,
        content_root: RelationalMechanismStarterProjectionContentRoot,
        page_manifest_root: RelationalMechanismStarterProjectionPageManifestRoot,
        exhausted: bool,
    ) -> Result<Self, RelationalMechanismStarterProjectionError> {
        let content_genesis = derive_content_genesis(job.id);
        let page_manifest_genesis = derive_page_manifest_genesis(job.id);
        let authority_case_count = job.authority.exact_case_count();
        let valid = if next_page_ordinal == 0 {
            last_cursor.is_none()
                && last_source_key.is_none()
                && exact_member_count == 0
                && exact_starter_count == 0
                && content_root == content_genesis
                && page_manifest_root == page_manifest_genesis
                && !exhausted
        } else if exact_member_count == 0 {
            // The only accepted empty page is the terminal page of an exact
            // empty structural-subject projection.
            authority_case_count == 0
                && next_page_ordinal == 1
                && last_cursor.is_none()
                && last_source_key.is_none()
                && exact_starter_count == 0
                && content_root == content_genesis
                && page_manifest_root != page_manifest_genesis
                && exhausted
        } else {
            next_page_ordinal <= exact_member_count
                && last_cursor.is_some()
                && last_source_key == last_cursor.map(|cursor| cursor.source_key())
                && (1..=exact_member_count).contains(&exact_starter_count)
                && exact_member_count <= authority_case_count
                && content_root != content_genesis
                && page_manifest_root != page_manifest_genesis
                && if exhausted {
                    exact_member_count == authority_case_count
                } else {
                    exact_member_count < authority_case_count
                }
        };
        if !valid {
            return Err(RelationalMechanismStarterProjectionError::InvalidAuthenticatedCheckpoint);
        }
        Ok(Self {
            job_id: job.id,
            next_page_ordinal,
            last_cursor,
            last_source_key,
            exact_member_count,
            exact_starter_count,
            content_root,
            page_manifest_root,
            exhausted,
        })
    }

    pub(crate) const fn job_id(self) -> RelationalMechanismStarterProjectionJobId {
        self.job_id
    }

    pub(crate) const fn next_page_ordinal(self) -> u128 {
        self.next_page_ordinal
    }

    pub(crate) const fn last_cursor(self) -> Option<MechanismSupportStarterCursor> {
        self.last_cursor
    }

    pub(crate) const fn last_source_key(self) -> Option<SourceKey> {
        self.last_source_key
    }

    pub(crate) const fn exact_member_count(self) -> u128 {
        self.exact_member_count
    }

    pub(crate) const fn exact_starter_count(self) -> u128 {
        self.exact_starter_count
    }

    pub(crate) const fn content_root(self) -> RelationalMechanismStarterProjectionContentRoot {
        self.content_root
    }

    pub(crate) const fn page_manifest_root(
        self,
    ) -> RelationalMechanismStarterProjectionPageManifestRoot {
        self.page_manifest_root
    }

    pub(crate) const fn exhausted(self) -> bool {
        self.exhausted
    }

    pub(crate) fn accept_page(
        &mut self,
        page: &RelationalMechanismStarterProjectionPage,
    ) -> Result<(), RelationalMechanismStarterProjectionError> {
        if self.exhausted
            || page.job_id != self.job_id
            || page.page_ordinal != self.next_page_ordinal
            || page.start_after != self.last_cursor
            || (!page.exhausted && page.members.is_empty())
            || page.end_cursor
                != page
                    .members
                    .last()
                    .map(RelationalMechanismStarterProjectionMember::cursor)
                    .or(page.start_after)
        {
            return Err(RelationalMechanismStarterProjectionError::AccumulatorStateMismatch);
        }
        let expected_root = derive_page_root(
            page.job_id,
            page.page_ordinal,
            page.start_after,
            page.end_cursor,
            page.exhausted,
            &page.members,
        );
        if expected_root != page.root
            || derive_page_id(page.job_id, page.page_ordinal, expected_root) != page.id
        {
            return Err(RelationalMechanismStarterProjectionError::PageIdentityMismatch);
        }

        // Validate and advance a copy so malformed restored input or count
        // overflow can never leave a partially advanced checkpoint.
        let mut advanced = *self;
        let mut previous_cursor = advanced.last_cursor;
        for member in page.members.iter() {
            let cursor = member.cursor();
            if previous_cursor.is_some_and(|previous| cursor <= previous) {
                return Err(RelationalMechanismStarterProjectionError::NonCanonicalMemberOrder);
            }
            advanced.content_root = append_content_member(advanced.content_root, member.id);
            advanced.exact_member_count = advanced
                .exact_member_count
                .checked_add(1)
                .ok_or(RelationalMechanismStarterProjectionError::CountOverflow)?;
            if advanced.last_source_key != Some(member.source_key) {
                advanced.exact_starter_count = advanced
                    .exact_starter_count
                    .checked_add(1)
                    .ok_or(RelationalMechanismStarterProjectionError::CountOverflow)?;
                advanced.last_source_key = Some(member.source_key);
            }
            previous_cursor = Some(cursor);
        }
        advanced.last_cursor = page.end_cursor;
        advanced.page_manifest_root = append_page_manifest(advanced.page_manifest_root, page.id);
        advanced.next_page_ordinal = advanced
            .next_page_ordinal
            .checked_add(1)
            .ok_or(RelationalMechanismStarterProjectionError::CountOverflow)?;
        advanced.exhausted = page.exhausted;
        *self = advanced;
        Ok(())
    }

    pub(crate) fn finish(
        self,
        job: RelationalMechanismStarterProjectionJob,
    ) -> Result<
        RelationalMechanismStarterProjectionClosure,
        RelationalMechanismStarterProjectionError,
    > {
        if self.job_id != job.id || !self.exhausted {
            return Err(RelationalMechanismStarterProjectionError::ProjectionStillOpen);
        }
        if self.exact_member_count != job.authority.exact_case_count() {
            return Err(
                RelationalMechanismStarterProjectionError::ExactCaseCountMismatch {
                    expected: job.authority.exact_case_count(),
                    actual: self.exact_member_count,
                },
            );
        }
        let root = derive_closure_root(
            job.id,
            job.key(),
            job.projection_plan_id(),
            self.content_root,
            self.exact_member_count,
            self.exact_starter_count,
        );
        Ok(RelationalMechanismStarterProjectionClosure {
            root,
            job_id: job.id,
            projection_plan_id: job.projection_plan_id(),
            key: job.key(),
            content_root: self.content_root,
            exact_case_count: self.exact_member_count,
            exact_starter_count: self.exact_starter_count,
            page_count: self.next_page_ordinal,
            page_manifest_root: self.page_manifest_root,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalMechanismStarterProjectionClosure {
    root: RelationalMechanismStarterProjectionClosureRoot,
    job_id: RelationalMechanismStarterProjectionJobId,
    projection_plan_id: MechanismStarterProjectionPlanId,
    key: MechanismSupportKey,
    content_root: RelationalMechanismStarterProjectionContentRoot,
    exact_case_count: u128,
    exact_starter_count: u128,
    page_count: u128,
    page_manifest_root: RelationalMechanismStarterProjectionPageManifestRoot,
}

impl RelationalMechanismStarterProjectionClosure {
    pub(crate) const fn root(self) -> RelationalMechanismStarterProjectionClosureRoot {
        self.root
    }

    pub(crate) const fn job_id(self) -> RelationalMechanismStarterProjectionJobId {
        self.job_id
    }

    pub(crate) const fn projection_plan_id(self) -> MechanismStarterProjectionPlanId {
        self.projection_plan_id
    }

    pub(crate) const fn key(self) -> MechanismSupportKey {
        self.key
    }

    pub(crate) const fn subject(self) -> MechanismSupportSubject {
        self.key.subject()
    }

    /// Temporary adapter for the whole-mechanism publisher. Subject-generic
    /// consumers must use [`Self::subject`].
    pub(crate) fn mechanism_id(self) -> StructuralMechanismId {
        whole_mechanism_id(self.subject())
    }

    pub(crate) const fn content_root(self) -> RelationalMechanismStarterProjectionContentRoot {
        self.content_root
    }

    pub(crate) const fn exact_case_count(self) -> u128 {
        self.exact_case_count
    }

    pub(crate) const fn exact_starter_count(self) -> u128 {
        self.exact_starter_count
    }

    pub(crate) const fn page_count(self) -> u128 {
        self.page_count
    }

    pub(crate) const fn page_manifest_root(
        self,
    ) -> RelationalMechanismStarterProjectionPageManifestRoot {
        self.page_manifest_root
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalMechanismStarterProjectionError {
    InvalidValueAuthorization,
    ValueAuthorizationScopeMismatch,
    Support(MechanismSupportError),
    UnresolvedAuthorizedCase { case_id: RelationalCaseId },
    ResolvedCaseScopeMismatch,
    ResolvedCaseCoordinateMismatch,
    ResolvedCaseIdentityMismatch,
    EmptyOpenPage,
    NonCanonicalMemberOrder,
    PageIdentityMismatch,
    AccumulatorStateMismatch,
    InvalidAuthenticatedCheckpoint,
    ProjectionStillOpen,
    ExactCaseCountMismatch { expected: u128, actual: u128 },
    CountOverflow,
}

impl From<MechanismSupportError> for RelationalMechanismStarterProjectionError {
    fn from(error: MechanismSupportError) -> Self {
        Self::Support(error)
    }
}

impl fmt::Display for RelationalMechanismStarterProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidValueAuthorization => formatter.write_str(
                "mechanism starter projection requires a valid checked value authorization",
            ),
            Self::ValueAuthorizationScopeMismatch => formatter.write_str(
                "mechanism starter value authorization does not cover this mechanism target",
            ),
            Self::Support(error) => write!(formatter, "mechanism starter support failed: {error}"),
            Self::UnresolvedAuthorizedCase { case_id } => write!(
                formatter,
                "mechanism starter projection cannot resolve authorized CaseId {}",
                lowercase_hex(case_id.bytes())
            ),
            Self::ResolvedCaseScopeMismatch => formatter
                .write_str("mechanism starter projection resolved a case from another relation"),
            Self::ResolvedCaseCoordinateMismatch => formatter.write_str(
                "mechanism starter projection resolved values for a different source/successor coordinate",
            ),
            Self::ResolvedCaseIdentityMismatch => formatter.write_str(
                "mechanism starter projection resolved a case whose CaseId is not derived from its keys",
            ),
            Self::EmptyOpenPage => {
                formatter.write_str("mechanism starter projection produced an empty open page")
            }
            Self::NonCanonicalMemberOrder => formatter
                .write_str("mechanism starter projection members are not in canonical key order"),
            Self::PageIdentityMismatch => formatter
                .write_str("mechanism starter projection page identity does not match its content"),
            Self::AccumulatorStateMismatch => formatter.write_str(
                "mechanism starter projection page does not continue the authenticated prefix",
            ),
            Self::InvalidAuthenticatedCheckpoint => formatter.write_str(
                "mechanism starter projection checkpoint violates its compact prefix invariants",
            ),
            Self::ProjectionStillOpen => formatter
                .write_str("mechanism starter projection cannot close before its pager is exhausted"),
            Self::ExactCaseCountMismatch { expected, actual } => write!(
                formatter,
                "mechanism starter projection closed with {actual} cases; support authority requires {expected}"
            ),
            Self::CountOverflow => {
                formatter.write_str("mechanism starter projection count overflowed")
            }
        }
    }
}

impl Error for RelationalMechanismStarterProjectionError {}

fn derive_member(
    job: RelationalMechanismStarterProjectionJob,
    key_member: MechanismSupportStarterMember,
    case: RelationalCaseRef<'_>,
) -> Result<RelationalMechanismStarterProjectionMember, RelationalMechanismStarterProjectionError> {
    if case.relation_id() != job.relation_id {
        return Err(RelationalMechanismStarterProjectionError::ResolvedCaseScopeMismatch);
    }
    if case.source_key() != key_member.source_key()
        || case.successor_key() != key_member.successor_key()
    {
        return Err(RelationalMechanismStarterProjectionError::ResolvedCaseCoordinateMismatch);
    }
    let derived_case_id = RelationalCaseId::derive(
        job.relation_id,
        key_member.source_key(),
        key_member.successor_key(),
    );
    if derived_case_id != key_member.case_id() || case.case_id() != derived_case_id {
        return Err(RelationalMechanismStarterProjectionError::ResolvedCaseIdentityMismatch);
    }
    let context_digest = canonical_explore_value_digest(case.context());
    let before_digest = canonical_explore_value_digest(case.before());
    let after_digest = canonical_explore_value_digest(case.after());
    let id = derive_member_id(
        job.id,
        key_member,
        context_digest,
        before_digest,
        after_digest,
    );
    Ok(RelationalMechanismStarterProjectionMember {
        id,
        raw_signature_id: key_member.raw_signature_id(),
        case_id: key_member.case_id(),
        source_key: key_member.source_key(),
        context: case.context().clone(),
        before: case.before().clone(),
        successor_key: key_member.successor_key(),
        after: case.after().clone(),
    })
}

fn derive_job_id(
    job: RelationalMechanismStarterProjectionJob,
) -> RelationalMechanismStarterProjectionJobId {
    derive_job_id_from_parts(
        job.authority,
        job.authorization_id.bytes(),
        job.authorizing_question_id,
        job.authorizing_view_id,
        job.role_schema_digest,
        job.relation_id,
        job.state_schema_id,
        job.context_schema_id,
        job.transition_type_id,
    )
}

#[allow(clippy::too_many_arguments)]
fn derive_job_id_from_parts(
    authority: MechanismClosedSubjectStarterProjectionAuthority,
    authorization_id: [u8; 32],
    authorizing_question_id: QuestionId,
    authorizing_view_id: ViewId,
    role_schema_digest: [u8; 32],
    relation_id: RelationId,
    state_schema_id: [u8; 32],
    context_schema_id: [u8; 32],
    transition_type_id: [u8; 32],
) -> RelationalMechanismStarterProjectionJobId {
    let mut encoder = ProjectionEncoder::new(JOB_ID_V3);
    encoder.u32(RELATIONAL_MECHANISM_STARTER_PROJECTION_VERSION);
    encode_support_key(&mut encoder, authority.key());
    encoder.digest(authority.question_id().bytes());
    encoder.digest(authority.projection_plan_id().bytes());
    let support_bounds = authority.support_expression_bounds();
    encoder.digest(support_bounds.case_inner_root().bytes());
    encoder.digest(support_bounds.case_outer_root().bytes());
    encoder.digest(support_bounds.starter_inner_root().bytes());
    encoder.digest(support_bounds.starter_outer_root().bytes());
    encoder.u8(match support_bounds.starter_set_status() {
        MechanismStarterSetStatus::Open => 0x01,
        MechanismStarterSetStatus::ExactStarterSet => 0x02,
    });
    encoder.u8(match support_bounds.correlated_support_status() {
        MechanismCorrelatedSupportStatus::Open => 0x01,
        MechanismCorrelatedSupportStatus::ExactCorrelatedSupport => 0x02,
    });
    encoder.digest(authority.structural_root().bytes());
    encoder.digest(authority.support_root().bytes());
    encoder.u128(authority.exact_case_count());
    encoder.digest(authorization_id);
    encoder.digest(authorizing_question_id.bytes());
    encoder.digest(authorizing_view_id.bytes());
    encoder.digest(role_schema_digest);
    encoder.digest(relation_id.bytes());
    encoder.digest(state_schema_id);
    encoder.digest(context_schema_id);
    encoder.digest(transition_type_id);
    RelationalMechanismStarterProjectionJobId(encoder.finish())
}

fn derive_member_id(
    job_id: RelationalMechanismStarterProjectionJobId,
    member: MechanismSupportStarterMember,
    context_digest: [u8; 32],
    before_digest: [u8; 32],
    after_digest: [u8; 32],
) -> RelationalMechanismStarterProjectionMemberId {
    let mut encoder = ProjectionEncoder::new(MEMBER_ID_V1);
    encoder.digest(job_id.bytes());
    encoder.digest(member.raw_signature_id().request_id().bytes());
    encoder.digest(member.raw_signature_id().bytes());
    encoder.digest(member.case_id().bytes());
    encoder.digest(member.source_key().bytes());
    encoder.digest(context_digest);
    encoder.digest(before_digest);
    encoder.digest(member.successor_key().bytes());
    encoder.digest(after_digest);
    RelationalMechanismStarterProjectionMemberId(encoder.finish())
}

fn derive_content_genesis(
    job_id: RelationalMechanismStarterProjectionJobId,
) -> RelationalMechanismStarterProjectionContentRoot {
    let mut encoder = ProjectionEncoder::new(CONTENT_GENESIS_V1);
    encoder.digest(job_id.bytes());
    RelationalMechanismStarterProjectionContentRoot(encoder.finish())
}

fn append_content_member(
    prior: RelationalMechanismStarterProjectionContentRoot,
    member_id: RelationalMechanismStarterProjectionMemberId,
) -> RelationalMechanismStarterProjectionContentRoot {
    let mut encoder = ProjectionEncoder::new(CONTENT_APPEND_V1);
    encoder.digest(prior.bytes());
    encoder.digest(member_id.bytes());
    RelationalMechanismStarterProjectionContentRoot(encoder.finish())
}

fn derive_page_root(
    job_id: RelationalMechanismStarterProjectionJobId,
    page_ordinal: u128,
    start_after: Option<MechanismSupportStarterCursor>,
    end_cursor: Option<MechanismSupportStarterCursor>,
    exhausted: bool,
    members: &[RelationalMechanismStarterProjectionMember],
) -> RelationalMechanismStarterProjectionPageRoot {
    let mut encoder = ProjectionEncoder::new(PAGE_ROOT_V1);
    encoder.digest(job_id.bytes());
    encoder.u128(page_ordinal);
    encode_optional_cursor(&mut encoder, start_after);
    encode_optional_cursor(&mut encoder, end_cursor);
    encoder.u8(u8::from(exhausted));
    encoder.u128(members.len() as u128);
    for member in members {
        encoder.digest(member.id.bytes());
    }
    RelationalMechanismStarterProjectionPageRoot(encoder.finish())
}

fn derive_page_id(
    job_id: RelationalMechanismStarterProjectionJobId,
    page_ordinal: u128,
    root: RelationalMechanismStarterProjectionPageRoot,
) -> RelationalMechanismStarterProjectionPageId {
    let mut encoder = ProjectionEncoder::new(PAGE_ID_V1);
    encoder.digest(job_id.bytes());
    encoder.u128(page_ordinal);
    encoder.digest(root.bytes());
    RelationalMechanismStarterProjectionPageId(encoder.finish())
}

fn derive_page_manifest_genesis(
    job_id: RelationalMechanismStarterProjectionJobId,
) -> RelationalMechanismStarterProjectionPageManifestRoot {
    let mut encoder = ProjectionEncoder::new(PAGE_MANIFEST_GENESIS_V1);
    encoder.digest(job_id.bytes());
    RelationalMechanismStarterProjectionPageManifestRoot(encoder.finish())
}

fn append_page_manifest(
    prior: RelationalMechanismStarterProjectionPageManifestRoot,
    page_id: RelationalMechanismStarterProjectionPageId,
) -> RelationalMechanismStarterProjectionPageManifestRoot {
    let mut encoder = ProjectionEncoder::new(PAGE_MANIFEST_APPEND_V1);
    encoder.digest(prior.bytes());
    encoder.digest(page_id.bytes());
    RelationalMechanismStarterProjectionPageManifestRoot(encoder.finish())
}

fn derive_closure_root(
    job_id: RelationalMechanismStarterProjectionJobId,
    key: MechanismSupportKey,
    projection_plan_id: MechanismStarterProjectionPlanId,
    content_root: RelationalMechanismStarterProjectionContentRoot,
    exact_case_count: u128,
    exact_starter_count: u128,
) -> RelationalMechanismStarterProjectionClosureRoot {
    let mut encoder = ProjectionEncoder::new(CLOSURE_ROOT_V3);
    encoder.u32(RELATIONAL_MECHANISM_STARTER_PROJECTION_VERSION);
    encoder.digest(job_id.bytes());
    encode_support_key(&mut encoder, key);
    encoder.digest(projection_plan_id.bytes());
    encoder.digest(content_root.bytes());
    encoder.u128(exact_case_count);
    encoder.u128(exact_starter_count);
    RelationalMechanismStarterProjectionClosureRoot(encoder.finish())
}

fn encode_support_key(encoder: &mut ProjectionEncoder, key: MechanismSupportKey) {
    encoder.digest(key.request_id().bytes());
    match key.target() {
        MechanismTargetId::Selected => encoder.u8(0x01),
        MechanismTargetId::ChosenView(view_id) => {
            encoder.u8(0x02);
            encoder.digest(view_id.bytes());
        }
    }
    match key.subject() {
        MechanismSupportSubject::Mechanism(mechanism_id) => {
            encoder.u8(0x01);
            encoder.digest(mechanism_id.bytes());
        }
        MechanismSupportSubject::Node { facet, node_id } => {
            encoder.u8(0x02);
            encode_support_facet(encoder, facet);
            encoder.digest(node_id.bytes());
        }
        MechanismSupportSubject::Edge { facet, edge_id } => {
            encoder.u8(0x03);
            encode_support_facet(encoder, facet);
            encoder.digest(edge_id.bytes());
        }
    }
}

fn encode_support_facet(encoder: &mut ProjectionEncoder, facet: MechanismSupportFacet) {
    encoder.u8(match facet {
        MechanismSupportFacet::Activation => 0x01,
        MechanismSupportFacet::DifferentialParticipation => 0x02,
    });
}

fn whole_mechanism_id(subject: MechanismSupportSubject) -> StructuralMechanismId {
    match subject {
        MechanismSupportSubject::Mechanism(mechanism_id) => mechanism_id,
        MechanismSupportSubject::Node { .. } | MechanismSupportSubject::Edge { .. } => {
            panic!("whole-mechanism publication adapter received a node/edge starter subject")
        }
    }
}

fn encode_optional_cursor(
    encoder: &mut ProjectionEncoder,
    cursor: Option<MechanismSupportStarterCursor>,
) {
    match cursor {
        Some(cursor) => {
            encoder.u8(0x01);
            encoder.digest(cursor.source_key().bytes());
            encoder.digest(cursor.successor_key().bytes());
        }
        None => encoder.u8(0x00),
    }
}

fn lowercase_hex(bytes: [u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

struct ProjectionEncoder(Sha256);

impl ProjectionEncoder {
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

    fn digest(&mut self, value: [u8; 32]) {
        self.0.update(value);
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::explore::mechanism_support::closed_subject_starter_fixture;
    use crate::explore::relation::ViewInputId;

    fn job_id_for(
        authority: MechanismClosedSubjectStarterProjectionAuthority,
        relation_id: RelationId,
    ) -> RelationalMechanismStarterProjectionJobId {
        let view_id = ViewId::from_canonical_view_preimage(
            ViewInputId::Selected(authority.question_id()),
            b"subject-starter-test-authorization",
        );
        derive_job_id_from_parts(
            authority,
            [0xa1; 32],
            authority.question_id(),
            view_id,
            [0xa2; 32],
            relation_id,
            [0xa3; 32],
            [0xa4; 32],
            [0xa5; 32],
        )
    }

    #[test]
    fn job_and_closure_identities_bind_whole_subject_and_facet() {
        let fixture = closed_subject_starter_fixture();
        let scope = fixture.support.scope();
        let subjects = [
            MechanismSupportSubject::Mechanism(fixture.mechanism_id),
            MechanismSupportSubject::Node {
                facet: MechanismSupportFacet::Activation,
                node_id: fixture.node_ids[0],
            },
            MechanismSupportSubject::Node {
                facet: MechanismSupportFacet::DifferentialParticipation,
                node_id: fixture.node_ids[0],
            },
            MechanismSupportSubject::Edge {
                facet: MechanismSupportFacet::Activation,
                edge_id: fixture.edge_ids[0],
            },
            MechanismSupportSubject::Edge {
                facet: MechanismSupportFacet::DifferentialParticipation,
                edge_id: fixture.edge_ids[0],
            },
        ];
        let mut job_ids = BTreeSet::new();
        let mut closure_roots = BTreeSet::new();

        for subject in subjects {
            let authority = fixture
                .support
                .derive_closed_subject_starter_projection_authority(
                    MechanismSupportKey::new(scope, subject),
                    &fixture.structural,
                )
                .expect("closed subject authority");
            let job_id = job_id_for(authority, fixture.relation_id);
            assert!(job_ids.insert(job_id));
            let content_root = derive_content_genesis(job_id);
            assert!(closure_roots.insert(derive_closure_root(
                job_id,
                authority.key(),
                authority.projection_plan_id(),
                content_root,
                authority.exact_case_count(),
                0,
            )));
        }
    }

    #[test]
    fn semantic_content_root_ignores_page_boundaries_while_page_roots_bind_cursors() {
        let fixture = closed_subject_starter_fixture();
        let authority = fixture
            .support
            .derive_closed_subject_starter_projection_authority(
                MechanismSupportKey::new(
                    fixture.support.scope(),
                    MechanismSupportSubject::Mechanism(fixture.mechanism_id),
                ),
                &fixture.structural,
            )
            .expect("closed mechanism authority");
        let first = fixture
            .support
            .closed_subject_starter_page(
                authority,
                &fixture.structural,
                fixture.relation_id,
                None,
                NonZeroU16::new(1).unwrap(),
            )
            .expect("first key page");
        let first_cursor = first.end_cursor().unwrap();
        let second = fixture
            .support
            .closed_subject_starter_page(
                authority,
                &fixture.structural,
                fixture.relation_id,
                Some(first_cursor),
                NonZeroU16::new(1).unwrap(),
            )
            .expect("second key page");
        let key_members = [first.members()[0], second.members()[0]];
        let job_id = job_id_for(authority, fixture.relation_id);
        let members = key_members
            .into_iter()
            .enumerate()
            .map(|(ordinal, key_member)| {
                let context = ExploreValue::Int(ordinal as i64);
                let before = ExploreValue::Int(100 + ordinal as i64);
                let after = ExploreValue::Int(101 + ordinal as i64);
                RelationalMechanismStarterProjectionMember {
                    id: derive_member_id(
                        job_id,
                        key_member,
                        canonical_explore_value_digest(&context),
                        canonical_explore_value_digest(&before),
                        canonical_explore_value_digest(&after),
                    ),
                    raw_signature_id: key_member.raw_signature_id(),
                    case_id: key_member.case_id(),
                    source_key: key_member.source_key(),
                    context,
                    before,
                    successor_key: key_member.successor_key(),
                    after,
                }
            })
            .collect::<Vec<_>>();
        let end_cursor = members[1].cursor();
        let one_page_root = derive_page_root(job_id, 0, None, Some(end_cursor), true, &members);
        let split_first_root =
            derive_page_root(job_id, 0, None, Some(first_cursor), false, &members[..1]);
        let split_second_root = derive_page_root(
            job_id,
            1,
            Some(first_cursor),
            Some(end_cursor),
            true,
            &members[1..],
        );
        assert_ne!(one_page_root, split_first_root);
        assert_ne!(one_page_root, split_second_root);
        assert_ne!(split_first_root, split_second_root);

        let content_root = members
            .iter()
            .fold(derive_content_genesis(job_id), |root, member| {
                append_content_member(root, member.id())
            });
        let one_page_closure = derive_closure_root(
            job_id,
            authority.key(),
            authority.projection_plan_id(),
            content_root,
            2,
            2,
        );
        let split_page_closure = derive_closure_root(
            job_id,
            authority.key(),
            authority.projection_plan_id(),
            content_root,
            2,
            2,
        );
        assert_eq!(one_page_closure, split_page_closure);
    }
}
