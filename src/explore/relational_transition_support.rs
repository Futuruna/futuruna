//! Canonical layered semantic-transition support retained by the journal.
//!
//! Relation discovery creates the universe layer `U`; admitted classifications
//! add `D`; selected FIND classifications add `M`.  The index stores only
//! semantic identities, source/successor coordinates and collision witnesses.
//! The coordinates authenticate each support member's route back to its
//! relation-scoped `(Context, Before)` starter and per-starter After fiber.
//! This does not alter the global semantic TransitionId, which already binds
//! canonical Context/Before/After, and does not expose those typed values.
//! Typed values remain in the checked relation catalog and require their
//! existing authorization.
//!
//! Within one RelationId, canonical relation normalization makes
//! CaseId-to-TransitionId injective: equal Context/Before/After is one source
//! and successor coordinate. Case and transition counts remain separate
//! commitments, but their U/D/M values are conservation equalities here.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::authenticated_treap::{
    AuthenticatedTreapError, AuthenticatedTreapMap, AuthenticatedTreapValue,
};
use super::relation::{
    AdmissionDecision, RelationCatalogBuilder, RelationalCaseId, SelectionDecision, SourceKey,
    SourceRow, SuccessorKey, SuccessorRow,
};
use super::transition::{
    canonical_explore_value_digest, ContextSchemaId, StateId, StateSchemaId, TransitionId,
    TransitionTypeId,
};

pub(crate) const RELATIONAL_TRANSITION_SUPPORT_VERSION: u32 = 1;

const STATE_TREE_DOMAIN: &[u8] = b"futuruna.transition-support.states.v1";
const TRANSITION_TREE_DOMAIN: &[u8] = b"futuruna.transition-support.transitions.v1";
const UNIVERSE_SUPPORT_TREE_DOMAIN: &[u8] = b"futuruna.transition-support.universe.v1";
const ADMITTED_TRANSITION_TREE_DOMAIN: &[u8] =
    b"futuruna.transition-support.admitted-transitions.v1";
const ADMITTED_SUPPORT_TREE_DOMAIN: &[u8] = b"futuruna.transition-support.admitted.v1";
const MATCHED_TRANSITION_TREE_DOMAIN: &[u8] = b"futuruna.transition-support.matched-transitions.v1";
const MATCHED_SUPPORT_TREE_DOMAIN: &[u8] = b"futuruna.transition-support.matched.v1";
const ROOT_DOMAIN: &[u8] = b"futuruna.transition-support.root.v1";
const STATE_MEMBER_DOMAIN: &[u8] = b"futuruna.transition-support.state-member.v1";
const TRANSITION_MEMBER_DOMAIN: &[u8] = b"futuruna.transition-support.transition-member.v1";
const SUPPORT_MEMBER_DOMAIN: &[u8] = b"futuruna.transition-support.support-member.v1";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalTransitionSupportRoot([u8; 32]);

impl RelationalTransitionSupportRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalTransitionLayer {
    Universe,
    Admitted,
    Matched,
}

impl RelationalTransitionLayer {
    pub(crate) const fn canonical_tag(self) -> u8 {
        match self {
            Self::Universe => 1,
            Self::Admitted => 2,
            Self::Matched => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalTransitionSupportCounts {
    states: u128,
    universe_cases: u128,
    universe_transitions: u128,
    admitted_cases: u128,
    admitted_transitions: u128,
    matched_cases: u128,
    matched_transitions: u128,
}

impl RelationalTransitionSupportCounts {
    pub(crate) const fn states(self) -> u128 {
        self.states
    }

    pub(crate) const fn cases(self, layer: RelationalTransitionLayer) -> u128 {
        match layer {
            RelationalTransitionLayer::Universe => self.universe_cases,
            RelationalTransitionLayer::Admitted => self.admitted_cases,
            RelationalTransitionLayer::Matched => self.matched_cases,
        }
    }

    pub(crate) const fn transitions(self, layer: RelationalTransitionLayer) -> u128 {
        match layer {
            RelationalTransitionLayer::Universe => self.universe_transitions,
            RelationalTransitionLayer::Admitted => self.admitted_transitions,
            RelationalTransitionLayer::Matched => self.matched_transitions,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalSemanticTransition {
    transition_id: TransitionId,
    before_state_id: StateId,
    after_state_id: StateId,
}

impl RelationalSemanticTransition {
    pub(crate) const fn transition_id(self) -> TransitionId {
        self.transition_id
    }

    pub(crate) const fn before_state_id(self) -> StateId {
        self.before_state_id
    }

    pub(crate) const fn after_state_id(self) -> StateId {
        self.after_state_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalTransitionCaseSupport {
    layer: RelationalTransitionLayer,
    transition_id: TransitionId,
    case_id: RelationalCaseId,
    source_key: SourceKey,
    successor_key: SuccessorKey,
}

impl RelationalTransitionCaseSupport {
    pub(crate) const fn layer(self) -> RelationalTransitionLayer {
        self.layer
    }

    pub(crate) const fn transition_id(self) -> TransitionId {
        self.transition_id
    }

    pub(crate) const fn case_id(self) -> RelationalCaseId {
        self.case_id
    }

    pub(crate) const fn source_key(self) -> SourceKey {
        self.source_key
    }

    pub(crate) const fn successor_key(self) -> SuccessorKey {
        self.successor_key
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StateEndpoint {
    Before,
    After,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StateWitness {
    value_digest: [u8; 32],
    case_id: RelationalCaseId,
    endpoint: StateEndpoint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TransitionNode {
    case_id: RelationalCaseId,
    before_state_id: StateId,
    after_state_id: StateId,
    context_digest: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CaseTransition {
    transition_id: TransitionId,
    before_state_id: StateId,
    after_state_id: StateId,
    source_key: SourceKey,
    successor_key: SuccessorKey,
}

#[derive(Clone, Debug)]
pub(crate) struct PreparedUniverseTransition {
    case_id: RelationalCaseId,
    source_key: SourceKey,
    successor_key: SuccessorKey,
    transition_id: TransitionId,
    before_state_id: StateId,
    after_state_id: StateId,
    before_digest: [u8; 32],
    after_digest: [u8; 32],
    context_digest: [u8; 32],
    already_present: bool,
    staged_state_tree: Option<AuthenticatedTreapMap>,
    staged_transition_tree: Option<AuthenticatedTreapMap>,
    staged_universe_support_tree: Option<AuthenticatedTreapMap>,
}

#[derive(Clone, Debug)]
pub(crate) struct PreparedTransitionClassification {
    layer: RelationalTransitionLayer,
    transition_id: TransitionId,
    case_id: RelationalCaseId,
    insert: bool,
    staged_transition_tree: Option<AuthenticatedTreapMap>,
    staged_support_tree: Option<AuthenticatedTreapMap>,
}

#[derive(Clone, Debug)]
pub(crate) struct RelationalTransitionSupportIndex {
    state_schema_id: StateSchemaId,
    context_schema_id: ContextSchemaId,
    transition_type_id: TransitionTypeId,
    states: BTreeMap<StateId, StateWitness>,
    transitions: BTreeMap<TransitionId, TransitionNode>,
    cases: BTreeMap<RelationalCaseId, CaseTransition>,
    state_tree: AuthenticatedTreapMap,
    transition_tree: AuthenticatedTreapMap,
    universe_support_tree: AuthenticatedTreapMap,
    admitted_transition_tree: AuthenticatedTreapMap,
    admitted_support_tree: AuthenticatedTreapMap,
    matched_transition_tree: AuthenticatedTreapMap,
    matched_support_tree: AuthenticatedTreapMap,
}

impl RelationalTransitionSupportIndex {
    pub(crate) fn new(
        state_schema_id: StateSchemaId,
        context_schema_id: ContextSchemaId,
        transition_type_id: TransitionTypeId,
    ) -> Self {
        Self {
            state_schema_id,
            context_schema_id,
            transition_type_id,
            states: BTreeMap::new(),
            transitions: BTreeMap::new(),
            cases: BTreeMap::new(),
            state_tree: AuthenticatedTreapMap::new(STATE_TREE_DOMAIN),
            transition_tree: AuthenticatedTreapMap::new(TRANSITION_TREE_DOMAIN),
            universe_support_tree: AuthenticatedTreapMap::new(UNIVERSE_SUPPORT_TREE_DOMAIN),
            admitted_transition_tree: AuthenticatedTreapMap::new(ADMITTED_TRANSITION_TREE_DOMAIN),
            admitted_support_tree: AuthenticatedTreapMap::new(ADMITTED_SUPPORT_TREE_DOMAIN),
            matched_transition_tree: AuthenticatedTreapMap::new(MATCHED_TRANSITION_TREE_DOMAIN),
            matched_support_tree: AuthenticatedTreapMap::new(MATCHED_SUPPORT_TREE_DOMAIN),
        }
    }

    pub(crate) const fn state_schema_id(&self) -> StateSchemaId {
        self.state_schema_id
    }

    pub(crate) const fn context_schema_id(&self) -> ContextSchemaId {
        self.context_schema_id
    }

    pub(crate) const fn transition_type_id(&self) -> TransitionTypeId {
        self.transition_type_id
    }

    pub(crate) fn counts(&self) -> RelationalTransitionSupportCounts {
        RelationalTransitionSupportCounts {
            states: self.state_tree.entry_count(),
            universe_cases: self.universe_support_tree.entry_count(),
            universe_transitions: self.transition_tree.entry_count(),
            admitted_cases: self.admitted_support_tree.entry_count(),
            admitted_transitions: self.admitted_transition_tree.entry_count(),
            matched_cases: self.matched_support_tree.entry_count(),
            matched_transitions: self.matched_transition_tree.entry_count(),
        }
    }

    pub(crate) fn root(&self) -> RelationalTransitionSupportRoot {
        let counts = self.counts();
        let mut hasher = Sha256::new();
        hasher.update(ROOT_DOMAIN);
        hasher.update(RELATIONAL_TRANSITION_SUPPORT_VERSION.to_be_bytes());
        hasher.update(self.state_schema_id.bytes());
        hasher.update(self.context_schema_id.bytes());
        hasher.update(self.transition_type_id.bytes());
        hash_tree(&mut hasher, &self.state_tree);
        hash_tree(&mut hasher, &self.transition_tree);
        hash_tree(&mut hasher, &self.universe_support_tree);
        hash_tree(&mut hasher, &self.admitted_transition_tree);
        hash_tree(&mut hasher, &self.admitted_support_tree);
        hash_tree(&mut hasher, &self.matched_transition_tree);
        hash_tree(&mut hasher, &self.matched_support_tree);
        hasher.update(counts.states.to_be_bytes());
        hasher.update(counts.universe_cases.to_be_bytes());
        hasher.update(counts.universe_transitions.to_be_bytes());
        hasher.update(counts.admitted_cases.to_be_bytes());
        hasher.update(counts.admitted_transitions.to_be_bytes());
        hasher.update(counts.matched_cases.to_be_bytes());
        hasher.update(counts.matched_transitions.to_be_bytes());
        RelationalTransitionSupportRoot(hasher.finalize().into())
    }

    pub(crate) fn preflight_universe(
        &self,
        relation: &RelationCatalogBuilder,
        case_id: RelationalCaseId,
        source_key: SourceKey,
        source: &SourceRow,
        successor_key: SuccessorKey,
        successor: &SuccessorRow,
    ) -> Result<PreparedUniverseTransition, RelationalTransitionSupportError> {
        let derived_source_key = SourceKey::derive(relation.relation_id(), source);
        let derived_successor_key =
            SuccessorKey::derive(relation.relation_id(), derived_source_key, successor);
        let derived_case_id = RelationalCaseId::derive(
            relation.relation_id(),
            derived_source_key,
            derived_successor_key,
        );
        if source_key != derived_source_key
            || successor_key != derived_successor_key
            || case_id != derived_case_id
        {
            return Err(RelationalTransitionSupportError::CoordinateClaimMismatch { case_id });
        }
        let before_state_id = StateId::derive(self.state_schema_id, source.before());
        let after_state_id = StateId::derive(self.state_schema_id, successor.after());
        let transition_id = TransitionId::derive(
            self.transition_type_id,
            source.context(),
            before_state_id,
            after_state_id,
        );
        let before_digest = canonical_explore_value_digest(source.before());
        let after_digest = canonical_explore_value_digest(successor.after());
        let context_digest = canonical_explore_value_digest(source.context());

        if before_state_id == after_state_id && before_digest != after_digest {
            return Err(RelationalTransitionSupportError::StateIdCollision {
                state_id: before_state_id,
            });
        }
        self.require_state_compatible(before_state_id, before_digest)?;
        self.require_state_compatible(after_state_id, after_digest)?;
        if let Some(existing) = self.transitions.get(&transition_id) {
            if existing.before_state_id != before_state_id
                || existing.after_state_id != after_state_id
                || existing.context_digest != context_digest
            {
                return Err(RelationalTransitionSupportError::TransitionIdCollision {
                    transition_id,
                });
            }
            if existing.case_id != case_id {
                return Err(
                    RelationalTransitionSupportError::WithinRelationInjectivityViolation {
                        transition_id,
                        first_case_id: existing.case_id,
                        second_case_id: case_id,
                    },
                );
            }
        }

        let already_present = match self.cases.get(&case_id) {
            Some(existing)
                if existing.transition_id == transition_id
                    && existing.before_state_id == before_state_id
                    && existing.after_state_id == after_state_id
                    && existing.source_key == source_key
                    && existing.successor_key == successor_key =>
            {
                if self
                    .universe_support_tree
                    .get(&support_key(transition_id, case_id))?
                    .is_none()
                {
                    return Err(RelationalTransitionSupportError::LayerInvariant { case_id });
                }
                true
            }
            Some(_) => {
                return Err(RelationalTransitionSupportError::CaseTransitionCollision { case_id })
            }
            None => false,
        };
        if already_present && relation.case(case_id).is_none() {
            return Err(RelationalTransitionSupportError::LayerInvariant { case_id });
        }
        if self.transitions.contains_key(&transition_id) != already_present {
            return Err(RelationalTransitionSupportError::IndexCorrupt);
        }

        let (staged_state_tree, staged_transition_tree, staged_universe_support_tree) =
            if already_present {
                (None, None, None)
            } else {
                let mut state_tree = self.state_tree.clone();
                for (state_id, digest) in [
                    (before_state_id, before_digest),
                    (after_state_id, after_digest),
                ] {
                    if !self.states.contains_key(&state_id)
                        && state_tree.get(&state_id.bytes())?.is_none()
                    {
                        state_tree.insert(
                            state_id.bytes().to_vec(),
                            AuthenticatedTreapValue::new(state_member_digest(state_id, digest), 1),
                        )?;
                    }
                }
                let transition_value = AuthenticatedTreapValue::new(
                    transition_member_digest(transition_id, before_state_id, after_state_id),
                    1,
                );
                let mut transition_tree = self.transition_tree.clone();
                transition_tree.insert(transition_id.bytes().to_vec(), transition_value)?;
                let mut support_tree = self.universe_support_tree.clone();
                support_tree.insert(
                    support_key(transition_id, case_id),
                    AuthenticatedTreapValue::new(
                        support_member_digest(
                            RelationalTransitionLayer::Universe,
                            transition_id,
                            case_id,
                            source_key,
                            successor_key,
                        ),
                        1,
                    ),
                )?;
                (Some(state_tree), Some(transition_tree), Some(support_tree))
            };

        Ok(PreparedUniverseTransition {
            case_id,
            source_key,
            successor_key,
            transition_id,
            before_state_id,
            after_state_id,
            before_digest,
            after_digest,
            context_digest,
            already_present,
            staged_state_tree,
            staged_transition_tree,
            staged_universe_support_tree,
        })
    }

    pub(crate) fn commit_universe(&mut self, prepared: PreparedUniverseTransition) -> bool {
        if prepared.already_present {
            return false;
        }
        self.state_tree = prepared
            .staged_state_tree
            .expect("new universe transition stages state tree");
        self.transition_tree = prepared
            .staged_transition_tree
            .expect("new universe transition stages transition tree");
        self.universe_support_tree = prepared
            .staged_universe_support_tree
            .expect("new universe transition stages support tree");
        self.states
            .entry(prepared.before_state_id)
            .or_insert(StateWitness {
                value_digest: prepared.before_digest,
                case_id: prepared.case_id,
                endpoint: StateEndpoint::Before,
            });
        self.states
            .entry(prepared.after_state_id)
            .or_insert(StateWitness {
                value_digest: prepared.after_digest,
                case_id: prepared.case_id,
                endpoint: StateEndpoint::After,
            });
        let previous = self.transitions.insert(
            prepared.transition_id,
            TransitionNode {
                case_id: prepared.case_id,
                before_state_id: prepared.before_state_id,
                after_state_id: prepared.after_state_id,
                context_digest: prepared.context_digest,
            },
        );
        debug_assert!(previous.is_none());
        self.cases.insert(
            prepared.case_id,
            CaseTransition {
                transition_id: prepared.transition_id,
                before_state_id: prepared.before_state_id,
                after_state_id: prepared.after_state_id,
                source_key: prepared.source_key,
                successor_key: prepared.successor_key,
            },
        );
        true
    }

    pub(crate) fn preflight_admission(
        &self,
        case_id: RelationalCaseId,
        decision: AdmissionDecision,
    ) -> Result<PreparedTransitionClassification, RelationalTransitionSupportError> {
        self.preflight_layer(
            RelationalTransitionLayer::Admitted,
            case_id,
            decision == AdmissionDecision::Admitted,
        )
    }

    pub(crate) fn preflight_question(
        &self,
        case_id: RelationalCaseId,
        decision: SelectionDecision,
    ) -> Result<PreparedTransitionClassification, RelationalTransitionSupportError> {
        if self
            .admitted_support_tree
            .get(&self.support_key_for_case(case_id)?)?
            .is_none()
        {
            return Err(RelationalTransitionSupportError::LayerInvariant { case_id });
        }
        self.preflight_layer(
            RelationalTransitionLayer::Matched,
            case_id,
            decision == SelectionDecision::Selected,
        )
    }

    fn preflight_layer(
        &self,
        layer: RelationalTransitionLayer,
        case_id: RelationalCaseId,
        included: bool,
    ) -> Result<PreparedTransitionClassification, RelationalTransitionSupportError> {
        let case = self
            .cases
            .get(&case_id)
            .ok_or(RelationalTransitionSupportError::UnknownCase { case_id })?;
        let transition_id = case.transition_id;
        let key = support_key(transition_id, case_id);
        let support = self.support_tree(layer).get(&key)?.is_some();
        let transition_support = self
            .support_transition_tree(layer)
            .get(&transition_id.bytes())?
            .is_some();
        if support != transition_support {
            return Err(RelationalTransitionSupportError::IndexCorrupt);
        }
        if !included && support {
            return Err(RelationalTransitionSupportError::ClassificationConflict {
                layer,
                case_id,
            });
        }
        let (staged_transition_tree, staged_support_tree) = if included && !support {
            let node = self
                .transitions
                .get(&transition_id)
                .ok_or(RelationalTransitionSupportError::UnknownCase { case_id })?;
            let transition_value = AuthenticatedTreapValue::new(
                transition_member_digest(transition_id, node.before_state_id, node.after_state_id),
                1,
            );
            let mut transition_tree = self.support_transition_tree(layer).clone();
            transition_tree.insert(transition_id.bytes().to_vec(), transition_value)?;
            let mut support_tree = self.support_tree(layer).clone();
            support_tree.insert(
                key,
                AuthenticatedTreapValue::new(
                    support_member_digest(
                        layer,
                        transition_id,
                        case_id,
                        case.source_key,
                        case.successor_key,
                    ),
                    1,
                ),
            )?;
            (Some(transition_tree), Some(support_tree))
        } else {
            (None, None)
        };
        Ok(PreparedTransitionClassification {
            layer,
            transition_id,
            case_id,
            insert: included && !support,
            staged_transition_tree,
            staged_support_tree,
        })
    }

    pub(crate) fn commit_classification(
        &mut self,
        prepared: PreparedTransitionClassification,
    ) -> bool {
        if !prepared.insert {
            return false;
        }
        let transition_tree = prepared
            .staged_transition_tree
            .expect("included classification stages transition tree");
        let support_tree = prepared
            .staged_support_tree
            .expect("included classification stages support tree");
        match prepared.layer {
            RelationalTransitionLayer::Universe => {
                unreachable!("universe support is committed with its relation event")
            }
            RelationalTransitionLayer::Admitted => {
                self.admitted_transition_tree = transition_tree;
                self.admitted_support_tree = support_tree;
            }
            RelationalTransitionLayer::Matched => {
                self.matched_transition_tree = transition_tree;
                self.matched_support_tree = support_tree;
            }
        }
        true
    }

    pub(crate) fn state_at_ordinal(
        &self,
        ordinal: u128,
    ) -> Result<Option<StateId>, RelationalTransitionSupportError> {
        self.state_tree
            .entry_at_ordinal(ordinal)
            .map(|entry| entry.map(|(key, _)| StateId::from_bytes(array32(key))))
            .map_err(Into::into)
    }

    pub(crate) fn transition_at_ordinal(
        &self,
        ordinal: u128,
    ) -> Result<Option<RelationalSemanticTransition>, RelationalTransitionSupportError> {
        let Some((key, _)) = self.transition_tree.entry_at_ordinal(ordinal)? else {
            return Ok(None);
        };
        let transition_id = TransitionId::from_bytes(array32(key));
        let node = self
            .transitions
            .get(&transition_id)
            .ok_or(RelationalTransitionSupportError::IndexCorrupt)?;
        Ok(Some(RelationalSemanticTransition {
            transition_id,
            before_state_id: node.before_state_id,
            after_state_id: node.after_state_id,
        }))
    }

    pub(crate) fn support_at_ordinal(
        &self,
        layer: RelationalTransitionLayer,
        ordinal: u128,
    ) -> Result<Option<RelationalTransitionCaseSupport>, RelationalTransitionSupportError> {
        let Some((key, _)) = self.support_tree(layer).entry_at_ordinal(ordinal)? else {
            return Ok(None);
        };
        if key.len() != 64 {
            return Err(RelationalTransitionSupportError::IndexCorrupt);
        }
        let transition_id = TransitionId::from_bytes(array32(&key[..32]));
        let case_id = RelationalCaseId::from_journal_codec_bytes(array32(&key[32..]));
        let case = self
            .cases
            .get(&case_id)
            .ok_or(RelationalTransitionSupportError::IndexCorrupt)?;
        if case.transition_id != transition_id {
            return Err(RelationalTransitionSupportError::IndexCorrupt);
        }
        Ok(Some(RelationalTransitionCaseSupport {
            layer,
            transition_id,
            case_id,
            source_key: case.source_key,
            successor_key: case.successor_key,
        }))
    }

    fn require_state_compatible(
        &self,
        state_id: StateId,
        value_digest: [u8; 32],
    ) -> Result<(), RelationalTransitionSupportError> {
        if self
            .states
            .get(&state_id)
            .is_some_and(|witness| witness.value_digest != value_digest)
        {
            return Err(RelationalTransitionSupportError::StateIdCollision { state_id });
        }
        Ok(())
    }

    fn support_key_for_case(
        &self,
        case_id: RelationalCaseId,
    ) -> Result<Vec<u8>, RelationalTransitionSupportError> {
        let transition_id = self
            .cases
            .get(&case_id)
            .ok_or(RelationalTransitionSupportError::UnknownCase { case_id })?
            .transition_id;
        Ok(support_key(transition_id, case_id))
    }

    fn support_tree(&self, layer: RelationalTransitionLayer) -> &AuthenticatedTreapMap {
        match layer {
            RelationalTransitionLayer::Universe => &self.universe_support_tree,
            RelationalTransitionLayer::Admitted => &self.admitted_support_tree,
            RelationalTransitionLayer::Matched => &self.matched_support_tree,
        }
    }

    fn support_transition_tree(&self, layer: RelationalTransitionLayer) -> &AuthenticatedTreapMap {
        match layer {
            RelationalTransitionLayer::Universe => &self.transition_tree,
            RelationalTransitionLayer::Admitted => &self.admitted_transition_tree,
            RelationalTransitionLayer::Matched => &self.matched_transition_tree,
        }
    }
}

fn hash_tree(hasher: &mut Sha256, tree: &AuthenticatedTreapMap) {
    hasher.update(tree.root_hash());
    hasher.update(tree.entry_count().to_be_bytes());
    hasher.update(tree.total_weight().to_be_bytes());
}

fn state_member_digest(state_id: StateId, value_digest: [u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(STATE_MEMBER_DOMAIN);
    hasher.update(state_id.bytes());
    hasher.update(value_digest);
    hasher.finalize().into()
}

fn transition_member_digest(
    transition_id: TransitionId,
    before_state_id: StateId,
    after_state_id: StateId,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(TRANSITION_MEMBER_DOMAIN);
    hasher.update(transition_id.bytes());
    hasher.update(before_state_id.bytes());
    hasher.update(after_state_id.bytes());
    hasher.finalize().into()
}

fn support_member_digest(
    layer: RelationalTransitionLayer,
    transition_id: TransitionId,
    case_id: RelationalCaseId,
    source_key: SourceKey,
    successor_key: SuccessorKey,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(SUPPORT_MEMBER_DOMAIN);
    hasher.update([layer.canonical_tag()]);
    hasher.update(transition_id.bytes());
    hasher.update(case_id.bytes());
    hasher.update(source_key.bytes());
    hasher.update(successor_key.bytes());
    hasher.finalize().into()
}

fn support_key(transition_id: TransitionId, case_id: RelationalCaseId) -> Vec<u8> {
    let mut key = Vec::with_capacity(64);
    key.extend_from_slice(&transition_id.bytes());
    key.extend_from_slice(&case_id.bytes());
    key
}

fn array32(bytes: &[u8]) -> [u8; 32] {
    bytes
        .try_into()
        .expect("authenticated transition-support key width")
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalTransitionSupportError {
    UnknownCase {
        case_id: RelationalCaseId,
    },
    StateIdCollision {
        state_id: StateId,
    },
    TransitionIdCollision {
        transition_id: TransitionId,
    },
    WithinRelationInjectivityViolation {
        transition_id: TransitionId,
        first_case_id: RelationalCaseId,
        second_case_id: RelationalCaseId,
    },
    CaseTransitionCollision {
        case_id: RelationalCaseId,
    },
    CoordinateClaimMismatch {
        case_id: RelationalCaseId,
    },
    ClassificationConflict {
        layer: RelationalTransitionLayer,
        case_id: RelationalCaseId,
    },
    LayerInvariant {
        case_id: RelationalCaseId,
    },
    IndexCorrupt,
    AuthenticatedIndex(AuthenticatedTreapError),
}

impl fmt::Display for RelationalTransitionSupportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownCase { case_id } => {
                write!(formatter, "semantic transition case {case_id:?} is unknown")
            }
            Self::StateIdCollision { state_id } => {
                write!(formatter, "semantic transition StateId collision at {state_id:?}")
            }
            Self::TransitionIdCollision { transition_id } => write!(
                formatter,
                "semantic transition TransitionId collision at {transition_id:?}"
            ),
            Self::WithinRelationInjectivityViolation {
                transition_id,
                first_case_id,
                second_case_id,
            } => write!(
                formatter,
                "semantic transition {transition_id:?} violates within-relation CaseId-to-TransitionId injectivity for cases {first_case_id:?} and {second_case_id:?}"
            ),
            Self::CaseTransitionCollision { case_id } => write!(
                formatter,
                "case {case_id:?} aliases incompatible semantic transitions"
            ),
            Self::CoordinateClaimMismatch { case_id } => write!(
                formatter,
                "case {case_id:?} has an invalid source or successor coordinate claim"
            ),
            Self::ClassificationConflict { layer, case_id } => write!(
                formatter,
                "case {case_id:?} conflicts with retained {layer:?} transition support"
            ),
            Self::LayerInvariant { case_id } => write!(
                formatter,
                "case {case_id:?} violates U/D/M transition-layer containment"
            ),
            Self::IndexCorrupt => formatter.write_str("semantic transition index is corrupt"),
            Self::AuthenticatedIndex(error) => error.fmt(formatter),
        }
    }
}

impl Error for RelationalTransitionSupportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::AuthenticatedIndex(error) => Some(error),
            _ => None,
        }
    }
}

impl From<AuthenticatedTreapError> for RelationalTransitionSupportError {
    fn from(error: AuthenticatedTreapError) -> Self {
        Self::AuthenticatedIndex(error)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::explore::relation::{RelationId, RelationProvenance};
    use crate::explore::ExploreValue;

    #[derive(Clone, Copy)]
    struct FixtureCase {
        case_id: RelationalCaseId,
        source_key: SourceKey,
        successor_key: SuccessorKey,
        admission: AdmissionDecision,
        question: Option<SelectionDecision>,
    }

    fn build_index(order: &[usize]) -> (RelationalTransitionSupportIndex, Vec<FixtureCase>) {
        let relation_id = RelationId::from_canonical_semantic_digest([41; 32]);
        let mut relation = RelationCatalogBuilder::new(relation_id);
        let mut support = RelationalTransitionSupportIndex::new(
            StateSchemaId::from_bytes([42; 32]),
            ContextSchemaId::from_bytes([43; 32]),
            TransitionTypeId::from_bytes([44; 32]),
        );
        let rows = [
            (10, 100, 100, AdmissionDecision::Rejected, None),
            (
                20,
                200,
                201,
                AdmissionDecision::Admitted,
                Some(SelectionDecision::NotSelected),
            ),
            (
                30,
                300,
                301,
                AdmissionDecision::Admitted,
                Some(SelectionDecision::Selected),
            ),
        ];
        let mut cases = vec![None; rows.len()];

        for &ordinal in order {
            let (context, before, after, admission, question) = rows[ordinal];
            let source = SourceRow::new(
                ExploreValue::Int(context),
                ExploreValue::Int(before),
                RelationProvenance::default(),
            );
            let source_key = relation.insert_source(source.clone()).unwrap();
            let successor =
                SuccessorRow::new(ExploreValue::Int(after), RelationProvenance::default());
            let successor_key = SuccessorKey::derive(relation_id, source_key, &successor);
            let case_id = RelationalCaseId::derive(relation_id, source_key, successor_key);

            let transition = support
                .preflight_universe(
                    &relation,
                    case_id,
                    source_key,
                    &source,
                    successor_key,
                    &successor,
                )
                .unwrap();
            let relation_insert = relation
                .preflight_insert_successor(source_key, successor)
                .unwrap();
            relation.commit_preflight_successor(relation_insert);
            assert!(support.commit_universe(transition));
            cases[ordinal] = Some(FixtureCase {
                case_id,
                source_key,
                successor_key,
                admission,
                question,
            });
        }

        let cases = cases.into_iter().map(Option::unwrap).collect::<Vec<_>>();
        for &ordinal in order.iter().rev() {
            let case = cases[ordinal];
            let admission = support
                .preflight_admission(case.case_id, case.admission)
                .unwrap();
            support.commit_classification(admission);
            if let Some(decision) = case.question {
                let question = support.preflight_question(case.case_id, decision).unwrap();
                support.commit_classification(question);
            }
        }
        (support, cases)
    }

    #[test]
    fn layered_support_is_canonical_and_retains_starter_fiber_routes() {
        let (forward, cases) = build_index(&[0, 1, 2]);
        let (reverse, _) = build_index(&[2, 1, 0]);
        assert_eq!(forward.root(), reverse.root());
        assert_eq!(forward.counts(), reverse.counts());

        let counts = forward.counts();
        assert_eq!(
            counts.states(),
            5,
            "the self-edge interns one role-neutral state"
        );
        assert_eq!(counts.cases(RelationalTransitionLayer::Universe), 3);
        assert_eq!(counts.transitions(RelationalTransitionLayer::Universe), 3);
        assert_eq!(counts.cases(RelationalTransitionLayer::Admitted), 2);
        assert_eq!(counts.transitions(RelationalTransitionLayer::Admitted), 2);
        assert_eq!(counts.cases(RelationalTransitionLayer::Matched), 1);
        assert_eq!(counts.transitions(RelationalTransitionLayer::Matched), 1);

        let expected_universe = cases
            .iter()
            .map(|case| (case.case_id, case.source_key, case.successor_key))
            .collect::<BTreeSet<_>>();
        let actual_universe = (0..counts.cases(RelationalTransitionLayer::Universe))
            .map(|ordinal| {
                let row = forward
                    .support_at_ordinal(RelationalTransitionLayer::Universe, ordinal)
                    .unwrap()
                    .unwrap();
                (row.case_id(), row.source_key(), row.successor_key())
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(actual_universe, expected_universe);

        let rejected = cases[0];
        let nonmatch = cases[1];
        let selected = cases[2];
        let admitted_cases = (0..counts.cases(RelationalTransitionLayer::Admitted))
            .map(|ordinal| {
                forward
                    .support_at_ordinal(RelationalTransitionLayer::Admitted, ordinal)
                    .unwrap()
                    .unwrap()
                    .case_id()
            })
            .collect::<BTreeSet<_>>();
        assert!(!admitted_cases.contains(&rejected.case_id));
        assert!(admitted_cases.contains(&nonmatch.case_id));
        assert!(admitted_cases.contains(&selected.case_id));
        assert_eq!(
            forward
                .support_at_ordinal(RelationalTransitionLayer::Matched, 0)
                .unwrap()
                .unwrap()
                .case_id(),
            selected.case_id
        );
    }

    #[test]
    fn rejected_coordinate_or_classification_claim_does_not_mutate_the_root() {
        let (support, cases) = build_index(&[0, 1, 2]);
        let before = support.root();
        let relation_id = RelationId::from_canonical_semantic_digest([41; 32]);
        let relation = RelationCatalogBuilder::new(relation_id);
        let source = SourceRow::new(
            ExploreValue::Int(99),
            ExploreValue::Int(999),
            RelationProvenance::default(),
        );
        let successor = SuccessorRow::new(ExploreValue::Int(1000), RelationProvenance::default());
        let source_key = SourceKey::derive(relation_id, &source);
        let successor_key = SuccessorKey::derive(relation_id, source_key, &successor);
        let case_id = RelationalCaseId::derive(relation_id, source_key, successor_key);
        assert!(matches!(
            support.preflight_universe(
                &relation,
                case_id,
                cases[0].source_key,
                &source,
                successor_key,
                &successor,
            ),
            Err(RelationalTransitionSupportError::CoordinateClaimMismatch { .. })
        ));
        assert_eq!(support.root(), before);

        assert!(matches!(
            support.preflight_question(cases[2].case_id, SelectionDecision::NotSelected),
            Err(RelationalTransitionSupportError::ClassificationConflict { .. })
        ));
        assert_eq!(support.root(), before);
    }
}
