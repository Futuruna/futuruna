//! Public semantic transition graph over the selected relational cases.
//!
//! The classified support graph answers how the finite search was partitioned.
//! This projection answers the different semantic question: which checked
//! `Context + Before -> After` transitions are supported by selected cases?
//! It reads only journal-retained cases, derives the producer-owned state and
//! transition identities, and never invokes the evaluator or mechanism replay.
//!
//! Records follow the journal-authenticated selected-discovery order so an open
//! stream remains append-only.  The exact closure separately commits the
//! canonical CaseId-sorted set, making graph identity independent of discovery
//! order.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::relation::{RelationalCaseId, SelectionDecision, ViewId};
use super::relational_analysis_journal::RelationalSelectedQuestionSealId;
use super::relational_journal::{RelationalJournalContract, RelationalSchedulerView};
use super::relational_mechanism_starter_authorization::{
    RelationalMechanismStarterValueAuthorization, RelationalMechanismStarterValueAuthorizationId,
};
use super::result_evidence::ResultInputCoverageRoot;
use super::transition::{
    ContextSchemaId, StateId, StateSchemaId, TransitionId, TransitionSchemaIdentities,
    TransitionTypeId,
};
use super::{ExploreValue, SourceKey, SuccessorKey};

pub(crate) const RELATIONAL_CASE_TRANSITION_PROJECTION_VERSION: u32 = 2;
pub(crate) const RELATIONAL_CASE_TRANSITION_PROJECTION_SCHEMA: &str =
    "futuruna.relational-selected-case-transitions.v2";
/// V2 keeps collision checking and exact distinct-node closure in memory.
/// Bound that auxiliary index independently of the much larger durable
/// relation so publishing an authorized graph cannot exhaust the worker.
/// Changing this bound requires a projection schema/version migration.
pub(crate) const RELATIONAL_CASE_TRANSITION_MAX_MEMBERS_V2: usize = 65_536;

const PROJECTION_ID_HASH_V2: &[u8] =
    b"futuruna.explore.relational-case-transition-projection-id.v2";
const CONTENT_ROOT_HASH_V2: &[u8] = b"futuruna.explore.relational-case-transition-content-root.v2";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalCaseTransitionProjectionId([u8; 32]);

impl RelationalCaseTransitionProjectionId {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalCaseTransitionContentRoot([u8; 32]);

impl RelationalCaseTransitionContentRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// One selected CaseId supporting one canonical semantic transition.
///
/// Endpoint values remain in the journal and in the checked authorizing view;
/// the publication renderer retrieves them only for the addressed record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalCaseTransitionMember {
    case_id: RelationalCaseId,
    source_key: SourceKey,
    successor_key: SuccessorKey,
    before_state_id: StateId,
    after_state_id: StateId,
    transition_id: TransitionId,
}

impl RelationalCaseTransitionMember {
    pub(crate) const fn case_id(self) -> RelationalCaseId {
        self.case_id
    }

    pub(crate) const fn source_key(self) -> SourceKey {
        self.source_key
    }

    pub(crate) const fn successor_key(self) -> SuccessorKey {
        self.successor_key
    }

    pub(crate) const fn before_state_id(self) -> StateId {
        self.before_state_id
    }

    pub(crate) const fn after_state_id(self) -> StateId {
        self.after_state_id
    }

    pub(crate) const fn transition_id(self) -> TransitionId {
        self.transition_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalCaseTransitionClosure {
    selected_question_seal_id: RelationalSelectedQuestionSealId,
    selected_case_set_root: ResultInputCoverageRoot,
    exact_case_count: u128,
    exact_state_count: u128,
    exact_transition_count: u128,
    content_root: RelationalCaseTransitionContentRoot,
    data_record_count: u128,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalCaseTransitionCapacity {
    maximum_members: u128,
    required_at_least: u128,
}

impl RelationalCaseTransitionCapacity {
    pub(crate) const fn maximum_members(self) -> u128 {
        self.maximum_members
    }

    pub(crate) const fn required_at_least(self) -> u128 {
        self.required_at_least
    }
}

impl RelationalCaseTransitionClosure {
    pub(crate) const fn selected_question_seal_id(self) -> RelationalSelectedQuestionSealId {
        self.selected_question_seal_id
    }

    pub(crate) const fn selected_case_set_root(self) -> ResultInputCoverageRoot {
        self.selected_case_set_root
    }

    pub(crate) const fn exact_case_count(self) -> u128 {
        self.exact_case_count
    }

    pub(crate) const fn exact_state_count(self) -> u128 {
        self.exact_state_count
    }

    pub(crate) const fn exact_transition_count(self) -> u128 {
        self.exact_transition_count
    }

    pub(crate) const fn content_root(self) -> RelationalCaseTransitionContentRoot {
        self.content_root
    }

    pub(crate) const fn data_record_count(self) -> u128 {
        self.data_record_count
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCaseTransitionProjectionRecord {
    Header {
        projection_id: RelationalCaseTransitionProjectionId,
        contract: RelationalJournalContract,
        state_schema_id: StateSchemaId,
        context_schema_id: ContextSchemaId,
        transition_type_id: TransitionTypeId,
        authorization_id: RelationalMechanismStarterValueAuthorizationId,
        authorizing_view_id: ViewId,
    },
    CaseTransition(RelationalCaseTransitionMember),
    Closure(RelationalCaseTransitionClosure),
    CapacityLimited(RelationalCaseTransitionCapacity),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalCaseTransitionProjectionMetadata {
    projection_id: RelationalCaseTransitionProjectionId,
    selected_case_count: u128,
    distinct_state_count: u128,
    distinct_transition_count: u128,
    closure: Option<RelationalCaseTransitionClosure>,
    capacity: Option<RelationalCaseTransitionCapacity>,
    available_source_record_count: u128,
}

impl RelationalCaseTransitionProjectionMetadata {
    pub(crate) const fn projection_id(self) -> RelationalCaseTransitionProjectionId {
        self.projection_id
    }

    pub(crate) const fn selected_case_count(self) -> u128 {
        self.selected_case_count
    }

    pub(crate) const fn distinct_state_count(self) -> u128 {
        self.distinct_state_count
    }

    pub(crate) const fn distinct_transition_count(self) -> u128 {
        self.distinct_transition_count
    }

    pub(crate) const fn closure(self) -> Option<RelationalCaseTransitionClosure> {
        self.closure
    }

    pub(crate) const fn capacity(self) -> Option<RelationalCaseTransitionCapacity> {
        self.capacity
    }

    pub(crate) const fn available_source_record_count(self) -> u128 {
        self.available_source_record_count
    }
}

pub(crate) struct RelationalCaseTransitionProjection {
    projection_id: RelationalCaseTransitionProjectionId,
    contract: RelationalJournalContract,
    schemas: TransitionSchemaIdentities,
    authorization: RelationalMechanismStarterValueAuthorization,
    members: Box<[RelationalCaseTransitionMember]>,
    distinct_state_count: u128,
    distinct_transition_count: u128,
    closure: Option<RelationalCaseTransitionClosure>,
    capacity: Option<RelationalCaseTransitionCapacity>,
}

impl RelationalCaseTransitionProjection {
    pub(crate) fn metadata(&self) -> RelationalCaseTransitionProjectionMetadata {
        RelationalCaseTransitionProjectionMetadata {
            projection_id: self.projection_id,
            selected_case_count: self.members.len() as u128,
            distinct_state_count: self.distinct_state_count,
            distinct_transition_count: self.distinct_transition_count,
            closure: self.closure,
            capacity: self.capacity,
            available_source_record_count: self.available_source_record_count(),
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        self.closure.is_none() && self.capacity.is_none()
    }

    pub(crate) fn available_source_record_count(&self) -> u128 {
        1_u128
            .checked_add(self.members.len() as u128)
            .and_then(|count| {
                count.checked_add(u128::from(
                    self.closure.is_some() || self.capacity.is_some(),
                ))
            })
            .expect("validated case-transition record count")
    }

    pub(crate) fn record_at(
        &self,
        source_ordinal: u128,
    ) -> Option<RelationalCaseTransitionProjectionRecord> {
        if source_ordinal == 0 {
            return Some(RelationalCaseTransitionProjectionRecord::Header {
                projection_id: self.projection_id,
                contract: self.contract,
                state_schema_id: self.schemas.state_schema_id(),
                context_schema_id: self.schemas.context_schema_id(),
                transition_type_id: self.schemas.transition_type_id(),
                authorization_id: self.authorization.authorization_id(),
                authorizing_view_id: self.authorization.view_id(),
            });
        }
        let member_ordinal = source_ordinal - 1;
        if let Ok(member_index) = usize::try_from(member_ordinal) {
            if let Some(member) = self.members.get(member_index) {
                return Some(RelationalCaseTransitionProjectionRecord::CaseTransition(
                    *member,
                ));
            }
        }
        if member_ordinal == self.members.len() as u128 {
            return self
                .closure
                .map(RelationalCaseTransitionProjectionRecord::Closure)
                .or_else(|| {
                    self.capacity
                        .map(RelationalCaseTransitionProjectionRecord::CapacityLimited)
                });
        }
        None
    }
}

pub(crate) fn derive_relational_case_transition_projection(
    scheduler: RelationalSchedulerView<'_>,
    schemas: TransitionSchemaIdentities,
    authorization: RelationalMechanismStarterValueAuthorization,
    selected_question_seal: Option<
        super::relational_analysis_journal::RelationalSelectedQuestionSeal,
    >,
) -> Result<RelationalCaseTransitionProjection, RelationalCaseTransitionProjectionError> {
    if !authorization.validate_identity() {
        return Err(RelationalCaseTransitionProjectionError::InvalidAuthorization);
    }
    let contract = scheduler.contract();
    if authorization.question_id() != contract.question_id() {
        return Err(RelationalCaseTransitionProjectionError::QuestionIdentityMismatch);
    }
    let projection_id = derive_projection_id(contract, &schemas, &authorization);

    let selected_discovery = scheduler.selected_discovery_suffix(0);
    let retained_member_count = selected_discovery
        .len()
        .min(RELATIONAL_CASE_TRANSITION_MAX_MEMBERS_V2);
    let capacity = projection_capacity(selected_discovery.len());
    let mut members = Vec::new();
    members
        .try_reserve_exact(retained_member_count)
        .map_err(|_| RelationalCaseTransitionProjectionError::AllocationFailed)?;
    let mut canonical_members = BTreeMap::new();
    let mut states: BTreeMap<StateId, &ExploreValue> = BTreeMap::new();
    let mut transitions: BTreeMap<
        TransitionId,
        (RelationalCaseId, StateId, StateId, &ExploreValue),
    > = BTreeMap::new();

    for case_id in selected_discovery
        .iter()
        .take(retained_member_count)
        .copied()
    {
        if scheduler.question_decision(case_id) != Some(SelectionDecision::Selected) {
            return Err(RelationalCaseTransitionProjectionError::CaseIsNotSelected { case_id });
        }
        let case = scheduler
            .case(case_id)
            .ok_or(RelationalCaseTransitionProjectionError::SelectedCaseMissing { case_id })?;
        if case.relation_id() != contract.relation_id() || case.case_id() != case_id {
            return Err(RelationalCaseTransitionProjectionError::CaseIdentityMismatch { case_id });
        }

        let before_state_id = StateId::derive(schemas.state_schema_id(), case.before());
        let after_state_id = StateId::derive(schemas.state_schema_id(), case.after());
        let transition_id = TransitionId::derive(
            schemas.transition_type_id(),
            case.context(),
            before_state_id,
            after_state_id,
        );

        require_state_compatible(&mut states, before_state_id, case.before())?;
        require_state_compatible(&mut states, after_state_id, case.after())?;
        if let Some((existing_case_id, existing_before, existing_after, existing_context)) =
            transitions.get(&transition_id)
        {
            if *existing_case_id != case_id {
                return Err(
                    RelationalCaseTransitionProjectionError::TransitionSupportAliasing {
                        first_case_id: *existing_case_id,
                        second_case_id: case_id,
                        transition_id,
                    },
                );
            }
            if *existing_before != before_state_id
                || *existing_after != after_state_id
                || *existing_context != case.context()
            {
                return Err(
                    RelationalCaseTransitionProjectionError::TransitionIdCollision {
                        transition_id,
                    },
                );
            }
        } else {
            transitions.insert(
                transition_id,
                (case_id, before_state_id, after_state_id, case.context()),
            );
        }

        let member = RelationalCaseTransitionMember {
            case_id,
            source_key: case.source_key(),
            successor_key: case.successor_key(),
            before_state_id,
            after_state_id,
            transition_id,
        };
        if canonical_members.insert(case_id, member).is_some() {
            return Err(RelationalCaseTransitionProjectionError::DuplicateSelectedCase { case_id });
        }
        members.push(member);
    }

    let distinct_state_count = states.len() as u128;
    let distinct_transition_count = transitions.len() as u128;
    if let (Some(capacity), Some(seal)) = (capacity, selected_question_seal) {
        seal.validate_identity()
            .map_err(|_| RelationalCaseTransitionProjectionError::InvalidSelectedQuestionSeal)?;
        let coverage = seal.result_input_seal().coverage();
        if seal.question_id() != contract.question_id()
            || coverage.row_count() < capacity.required_at_least()
        {
            return Err(RelationalCaseTransitionProjectionError::SelectedClosureMismatch);
        }
    }
    let closure = (capacity.is_none())
        .then_some(selected_question_seal)
        .flatten()
        .map(|seal| {
            seal.validate_identity().map_err(|_| {
                RelationalCaseTransitionProjectionError::InvalidSelectedQuestionSeal
            })?;
            let coverage = seal.result_input_seal().coverage();
            let canonical_selected = scheduler.selected_case_ids().collect::<BTreeSet<_>>();
            let exact_case_count = canonical_members.len() as u128;
            if seal.question_id() != contract.question_id()
                || coverage.row_count() != exact_case_count
                || canonical_selected.len() != canonical_members.len()
                || !canonical_selected
                    .iter()
                    .copied()
                    .eq(canonical_members.keys().copied())
            {
                return Err(RelationalCaseTransitionProjectionError::SelectedClosureMismatch);
            }
            let content_root = derive_content_root(
                projection_id,
                seal.id(),
                coverage.row_set_root(),
                canonical_members.values().copied(),
            );
            let data_record_count = 1_u128
                .checked_add(exact_case_count)
                .ok_or(RelationalCaseTransitionProjectionError::ArithmeticOverflow)?;
            Ok(RelationalCaseTransitionClosure {
                selected_question_seal_id: seal.id(),
                selected_case_set_root: coverage.row_set_root(),
                exact_case_count,
                exact_state_count: distinct_state_count,
                exact_transition_count: distinct_transition_count,
                content_root,
                data_record_count,
            })
        })
        .transpose()?;

    Ok(RelationalCaseTransitionProjection {
        projection_id,
        contract,
        schemas,
        authorization,
        members: members.into_boxed_slice(),
        distinct_state_count,
        distinct_transition_count,
        closure,
        capacity,
    })
}

fn require_state_compatible<'value>(
    states: &mut BTreeMap<StateId, &'value ExploreValue>,
    state_id: StateId,
    value: &'value ExploreValue,
) -> Result<(), RelationalCaseTransitionProjectionError> {
    if let Some(existing) = states.get(&state_id) {
        if *existing != value {
            return Err(RelationalCaseTransitionProjectionError::StateIdCollision { state_id });
        }
    } else {
        states.insert(state_id, value);
    }
    Ok(())
}

fn derive_projection_id(
    contract: RelationalJournalContract,
    schemas: &TransitionSchemaIdentities,
    authorization: &RelationalMechanismStarterValueAuthorization,
) -> RelationalCaseTransitionProjectionId {
    let mut hasher = Sha256::new();
    hasher.update(PROJECTION_ID_HASH_V2);
    hasher.update(RELATIONAL_CASE_TRANSITION_PROJECTION_VERSION.to_be_bytes());
    hasher.update(contract.relation_id().bytes());
    hasher.update(contract.admission_id().bytes());
    hasher.update(contract.question_id().bytes());
    hasher.update(schemas.state_schema_id().bytes());
    hasher.update(schemas.context_schema_id().bytes());
    hasher.update(schemas.transition_type_id().bytes());
    hasher.update(authorization.authorization_id().bytes());
    RelationalCaseTransitionProjectionId(hasher.finalize().into())
}

fn derive_content_root(
    projection_id: RelationalCaseTransitionProjectionId,
    seal_id: RelationalSelectedQuestionSealId,
    selected_case_set_root: ResultInputCoverageRoot,
    members: impl ExactSizeIterator<Item = RelationalCaseTransitionMember>,
) -> RelationalCaseTransitionContentRoot {
    let mut hasher = Sha256::new();
    hasher.update(CONTENT_ROOT_HASH_V2);
    hasher.update(projection_id.bytes());
    hasher.update(seal_id.bytes());
    hasher.update(selected_case_set_root.bytes());
    hasher.update((members.len() as u128).to_be_bytes());
    for member in members {
        hasher.update(member.case_id.bytes());
        hasher.update(member.source_key.bytes());
        hasher.update(member.successor_key.bytes());
        hasher.update(member.before_state_id.bytes());
        hasher.update(member.after_state_id.bytes());
        hasher.update(member.transition_id.bytes());
    }
    RelationalCaseTransitionContentRoot(hasher.finalize().into())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCaseTransitionProjectionError {
    InvalidAuthorization,
    QuestionIdentityMismatch,
    SelectedCaseMissing {
        case_id: RelationalCaseId,
    },
    CaseIsNotSelected {
        case_id: RelationalCaseId,
    },
    CaseIdentityMismatch {
        case_id: RelationalCaseId,
    },
    DuplicateSelectedCase {
        case_id: RelationalCaseId,
    },
    StateIdCollision {
        state_id: StateId,
    },
    TransitionIdCollision {
        transition_id: TransitionId,
    },
    TransitionSupportAliasing {
        first_case_id: RelationalCaseId,
        second_case_id: RelationalCaseId,
        transition_id: TransitionId,
    },
    InvalidSelectedQuestionSeal,
    SelectedClosureMismatch,
    AllocationFailed,
    ArithmeticOverflow,
}

impl fmt::Display for RelationalCaseTransitionProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAuthorization => formatter.write_str(
                "selected case-transition graph has an invalid checked value authorization",
            ),
            Self::QuestionIdentityMismatch => formatter.write_str(
                "selected case-transition authorization names another Explore question",
            ),
            Self::SelectedCaseMissing { .. } => formatter.write_str(
                "selected case-transition projection cannot recover a selected journal case",
            ),
            Self::CaseIsNotSelected { .. } => formatter.write_str(
                "selected case-transition discovery contains a non-selected CaseId",
            ),
            Self::CaseIdentityMismatch { .. } => formatter.write_str(
                "selected case-transition member disagrees with the checked relation identity",
            ),
            Self::DuplicateSelectedCase { .. } => formatter.write_str(
                "selected case-transition discovery contains a duplicate CaseId",
            ),
            Self::StateIdCollision { .. } => formatter.write_str(
                "selected case-transition graph rejected a StateId collision",
            ),
            Self::TransitionIdCollision { .. } => formatter.write_str(
                "selected case-transition graph rejected a TransitionId collision",
            ),
            Self::TransitionSupportAliasing { .. } => formatter.write_str(
                "two CaseIds in one RelationId unexpectedly identify the same semantic transition",
            ),
            Self::InvalidSelectedQuestionSeal => formatter.write_str(
                "selected case-transition graph received an invalid selected-question seal",
            ),
            Self::SelectedClosureMismatch => formatter.write_str(
                "selected case-transition members disagree with the exact selected-question closure",
            ),
            Self::AllocationFailed => formatter.write_str(
                "selected case-transition projection could not allocate its bounded member index",
            ),
            Self::ArithmeticOverflow => formatter.write_str(
                "selected case-transition projection count exceeds u128::MAX",
            ),
        }
    }
}

impl Error for RelationalCaseTransitionProjectionError {}

fn projection_capacity(selected_count: usize) -> Option<RelationalCaseTransitionCapacity> {
    (selected_count > RELATIONAL_CASE_TRANSITION_MAX_MEMBERS_V2).then_some(
        RelationalCaseTransitionCapacity {
            maximum_members: RELATIONAL_CASE_TRANSITION_MAX_MEMBERS_V2 as u128,
            required_at_least: (RELATIONAL_CASE_TRANSITION_MAX_MEMBERS_V2 as u128) + 1,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v2_capacity_frontier_begins_only_after_the_exact_member_limit() {
        assert_eq!(
            projection_capacity(RELATIONAL_CASE_TRANSITION_MAX_MEMBERS_V2),
            None
        );
        let capacity = projection_capacity(RELATIONAL_CASE_TRANSITION_MAX_MEMBERS_V2 + 1)
            .expect("one member beyond the cap has an honest capacity frontier");
        assert_eq!(
            capacity.maximum_members(),
            RELATIONAL_CASE_TRANSITION_MAX_MEMBERS_V2 as u128
        );
        assert_eq!(
            capacity.required_at_least(),
            RELATIONAL_CASE_TRANSITION_MAX_MEMBERS_V2 as u128 + 1
        );
    }
}
