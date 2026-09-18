//! Durable semantic choice relations for relational Explore.
//!
//! A choice is deliberately smaller than a result view. It owns the input
//! partition, measures, HAVING predicate, objective vector, and deterministic
//! membership policy, but no SELECT projection or privacy/display metadata.
//! Candidate evidence may accumulate while the FIND question remains open;
//! exact membership can be minted only after the exact selected-question seal
//! has been installed.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::mechanism_incidence::MechanismTargetCaseSetCommitment;
use super::relation::{ChoiceId, QuestionId, RelationalCaseId};
use super::relational_analysis_journal::RelationalSelectedQuestionSealId;
use super::result_view::{ResultValue, ResultViewChoice, ResultViewHaving};
use crate::{ExploreChooseCardinality, ExploreOptimizeDirection};

pub(crate) const CHOICE_RELATION_VERSION: u32 = 1;

const CHOICE_FRONTIER_ROOT_V1: &[u8] = b"futuruna.explore.choice-frontier.v1";
const CHOICE_CONTENT_ROOT_V1: &[u8] = b"futuruna.explore.choice-content.v1";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ChoiceFrontierRoot([u8; 32]);

impl ChoiceFrontierRoot {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ChoiceContentRoot([u8; 32]);

impl ChoiceContentRoot {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Canonical, expression-free reducer schema persisted with the analysis
/// plan. Expressions remain checked-query/runtime concerns; these dimensions
/// are sufficient to independently replay and validate membership semantics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ChoiceRelationSpec {
    choice_id: ChoiceId,
    partition_value_count: usize,
    measure_count: usize,
    having: Option<ResultViewHaving>,
    policy: ResultViewChoice,
}

impl ChoiceRelationSpec {
    pub(crate) fn new(
        choice_id: ChoiceId,
        partition_value_count: usize,
        measure_count: usize,
        having: Option<ResultViewHaving>,
        policy: ResultViewChoice,
    ) -> Result<Self, ChoiceRelationError> {
        if let Some(ResultViewHaving::Varies { measure_index }) = having {
            if measure_index >= measure_count {
                return Err(ChoiceRelationError::UnknownHavingMeasure { measure_index });
            }
        }
        if matches!(&policy, ResultViewChoice::Pareto { directions } if directions.is_empty()) {
            return Err(ChoiceRelationError::EmptyParetoObjectives);
        }
        Ok(Self {
            choice_id,
            partition_value_count,
            measure_count,
            having,
            policy,
        })
    }

    pub(super) fn restore_from_journal_codec(
        choice_id: ChoiceId,
        partition_value_count: usize,
        measure_count: usize,
        having: Option<ResultViewHaving>,
        policy: ResultViewChoice,
    ) -> Result<Self, ChoiceRelationError> {
        Self::new(
            choice_id,
            partition_value_count,
            measure_count,
            having,
            policy,
        )
    }

    pub(crate) const fn choice_id(&self) -> ChoiceId {
        self.choice_id
    }

    pub(crate) const fn partition_value_count(&self) -> usize {
        self.partition_value_count
    }

    pub(crate) const fn measure_count(&self) -> usize {
        self.measure_count
    }

    pub(crate) const fn having(&self) -> Option<ResultViewHaving> {
        self.having
    }

    pub(crate) const fn policy(&self) -> &ResultViewChoice {
        &self.policy
    }

    pub(crate) fn objective_count(&self) -> usize {
        self.policy.objective_count()
    }
}

/// Exact upstream seal installed only after the selected question closes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ChoiceInputSeal {
    question_id: QuestionId,
    selected_question_seal_id: RelationalSelectedQuestionSealId,
    case_set: MechanismTargetCaseSetCommitment,
}

impl ChoiceInputSeal {
    pub(crate) const fn new(
        question_id: QuestionId,
        selected_question_seal_id: RelationalSelectedQuestionSealId,
        case_set: MechanismTargetCaseSetCommitment,
    ) -> Self {
        Self {
            question_id,
            selected_question_seal_id,
            case_set,
        }
    }

    pub(crate) const fn question_id(self) -> QuestionId {
        self.question_id
    }

    pub(crate) const fn selected_question_seal_id(self) -> RelationalSelectedQuestionSealId {
        self.selected_question_seal_id
    }

    pub(crate) const fn case_set(self) -> MechanismTargetCaseSetCommitment {
        self.case_set
    }
}

/// One checked candidate, before membership selection. Positional values are
/// semantic; display aliases and projections never enter this record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ChoiceCandidate {
    choice_id: ChoiceId,
    case_id: RelationalCaseId,
    partition_values: Box<[ResultValue]>,
    measures: Box<[ResultValue]>,
    objectives: Box<[i64]>,
}

impl ChoiceCandidate {
    pub(crate) fn new(
        choice_id: ChoiceId,
        case_id: RelationalCaseId,
        partition_values: impl Into<Box<[ResultValue]>>,
        measures: impl Into<Box<[ResultValue]>>,
        objectives: impl Into<Box<[i64]>>,
    ) -> Self {
        Self {
            choice_id,
            case_id,
            partition_values: partition_values.into(),
            measures: measures.into(),
            objectives: objectives.into(),
        }
    }

    pub(crate) const fn choice_id(&self) -> ChoiceId {
        self.choice_id
    }

    pub(crate) const fn case_id(&self) -> RelationalCaseId {
        self.case_id
    }

    pub(crate) fn partition_values(&self) -> &[ResultValue] {
        &self.partition_values
    }

    pub(crate) fn measures(&self) -> &[ResultValue] {
        &self.measures
    }

    pub(crate) fn objectives(&self) -> &[i64] {
        &self.objectives
    }

    pub(crate) fn canonical_digest(&self) -> [u8; 32] {
        let mut hasher = ChoiceHasher::new(b"futuruna.explore.choice-candidate.v1");
        hash_candidate(&mut hasher, self);
        hasher.finish()
    }
}

/// One exact member in canonical partition/CaseId order. It retains the
/// semantic evaluated row, making the content root independently auditable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ChoiceMember {
    ordinal: u128,
    candidate: ChoiceCandidate,
}

impl ChoiceMember {
    fn from_candidate(ordinal: u128, candidate: ChoiceCandidate) -> Self {
        Self { ordinal, candidate }
    }

    pub(super) fn restore_from_journal_codec(ordinal: u128, candidate: ChoiceCandidate) -> Self {
        Self { ordinal, candidate }
    }

    pub(crate) const fn ordinal(&self) -> u128 {
        self.ordinal
    }

    pub(crate) const fn candidate(&self) -> &ChoiceCandidate {
        &self.candidate
    }

    pub(crate) const fn case_id(&self) -> RelationalCaseId {
        self.candidate.case_id
    }

    pub(crate) fn canonical_digest(&self) -> [u8; 32] {
        let mut hasher = ChoiceHasher::new(b"futuruna.explore.choice-member.v1");
        hasher.u128(self.ordinal);
        hash_candidate(&mut hasher, &self.candidate);
        hasher.finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ChoiceCount {
    LowerBound(u128),
    Provisional(u128),
    Exact(u128),
}

impl ChoiceCount {
    pub(crate) const fn current(self) -> u128 {
        match self {
            Self::LowerBound(value) | Self::Provisional(value) | Self::Exact(value) => value,
        }
    }

    pub(crate) const fn is_exact(self) -> bool {
        matches!(self, Self::Exact(_))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ChoiceRelationCounts {
    candidates: ChoiceCount,
    members: ChoiceCount,
}

impl ChoiceRelationCounts {
    pub(crate) const fn candidates(self) -> ChoiceCount {
        self.candidates
    }

    pub(crate) const fn members(self) -> ChoiceCount {
        self.members
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ChoiceRelationStatus {
    InputOpen,
    MembersOpen,
    Closed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ChoiceRelationSnapshot {
    version: u32,
    spec: ChoiceRelationSpec,
    candidates: Box<[ChoiceCandidate]>,
    input_seal: Option<ChoiceInputSeal>,
    members: Box<[ChoiceMember]>,
    content_root: Option<ChoiceContentRoot>,
    frontier_root: ChoiceFrontierRoot,
}

impl ChoiceRelationSnapshot {
    pub(crate) const fn version(&self) -> u32 {
        self.version
    }

    pub(crate) const fn spec(&self) -> &ChoiceRelationSpec {
        &self.spec
    }

    pub(crate) fn candidates(&self) -> &[ChoiceCandidate] {
        &self.candidates
    }

    pub(crate) const fn input_seal(&self) -> Option<ChoiceInputSeal> {
        self.input_seal
    }

    pub(crate) fn members(&self) -> &[ChoiceMember] {
        &self.members
    }

    pub(crate) const fn content_root(&self) -> Option<ChoiceContentRoot> {
        self.content_root
    }

    pub(crate) const fn frontier_root(&self) -> ChoiceFrontierRoot {
        self.frontier_root
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ChoiceRelationBuilder {
    spec: ChoiceRelationSpec,
    candidates: BTreeMap<RelationalCaseId, ChoiceCandidate>,
    input_seal: Option<ChoiceInputSeal>,
    /// Deterministically derived once when the input seal closes. This is an
    /// operational cache: snapshots and roots continue to commit only the
    /// candidates, seal, admitted member prefix, and final content root.
    expected_members: Option<Box<[ChoiceMember]>>,
    members: Vec<ChoiceMember>,
    content_root: Option<ChoiceContentRoot>,
}

impl ChoiceRelationBuilder {
    pub(crate) fn new(spec: ChoiceRelationSpec) -> Self {
        Self {
            spec,
            candidates: BTreeMap::new(),
            input_seal: None,
            expected_members: None,
            members: Vec::new(),
            content_root: None,
        }
    }

    pub(crate) const fn spec(&self) -> &ChoiceRelationSpec {
        &self.spec
    }

    pub(crate) const fn input_seal(&self) -> Option<ChoiceInputSeal> {
        self.input_seal
    }

    pub(crate) const fn content_root(&self) -> Option<ChoiceContentRoot> {
        self.content_root
    }

    pub(crate) fn candidate(&self, case_id: RelationalCaseId) -> Option<&ChoiceCandidate> {
        self.candidates.get(&case_id)
    }

    pub(crate) fn candidates(&self) -> impl ExactSizeIterator<Item = &ChoiceCandidate> {
        self.candidates.values()
    }

    pub(crate) fn members(&self) -> &[ChoiceMember] {
        &self.members
    }

    pub(crate) fn member(&self, ordinal: u128) -> Option<&ChoiceMember> {
        usize::try_from(ordinal)
            .ok()
            .and_then(|ordinal| self.members.get(ordinal))
    }

    pub(crate) fn status(&self) -> ChoiceRelationStatus {
        if self.content_root.is_some() {
            ChoiceRelationStatus::Closed
        } else if self.input_seal.is_some() {
            ChoiceRelationStatus::MembersOpen
        } else {
            ChoiceRelationStatus::InputOpen
        }
    }

    pub(crate) fn counts(&self) -> ChoiceRelationCounts {
        let candidate_count = self.candidates.len() as u128;
        let member_count = self.members.len() as u128;
        ChoiceRelationCounts {
            candidates: if self.input_seal.is_some() {
                ChoiceCount::Exact(candidate_count)
            } else {
                ChoiceCount::LowerBound(candidate_count)
            },
            members: if self.content_root.is_some() {
                ChoiceCount::Exact(member_count)
            } else {
                ChoiceCount::Provisional(member_count)
            },
        }
    }

    pub(crate) fn accept_candidate(
        &mut self,
        candidate: ChoiceCandidate,
    ) -> Result<bool, ChoiceRelationError> {
        self.validate_candidate(&candidate)?;
        if self.input_seal.is_some() || self.content_root.is_some() {
            return Err(ChoiceRelationError::CandidateAfterInputSeal);
        }
        match self.candidates.get(&candidate.case_id) {
            Some(existing) if existing == &candidate => Ok(false),
            Some(_) => Err(ChoiceRelationError::CandidateConflict(candidate.case_id)),
            None => {
                self.candidates.insert(candidate.case_id, candidate);
                Ok(true)
            }
        }
    }

    pub(crate) fn seal_input(
        &mut self,
        seal: ChoiceInputSeal,
    ) -> Result<bool, ChoiceRelationError> {
        if self.content_root.is_some() {
            return Err(ChoiceRelationError::InputSealAfterClosure);
        }
        if let Some(existing) = self.input_seal {
            return if existing == seal {
                Ok(false)
            } else {
                Err(ChoiceRelationError::InputSealReplacement)
            };
        }
        let actual = MechanismTargetCaseSetCommitment::from_cases(self.candidates.keys().copied());
        if seal.case_set != actual {
            return Err(ChoiceRelationError::InputCaseSetMismatch);
        }
        self.input_seal = Some(seal);
        self.expected_members = Some(self.derive_expected_members().into_boxed_slice());
        Ok(true)
    }

    pub(crate) fn expected_members(&self) -> Result<&[ChoiceMember], ChoiceRelationError> {
        self.expected_members
            .as_deref()
            .ok_or(ChoiceRelationError::InputStillOpen)
    }

    fn derive_expected_members(&self) -> Vec<ChoiceMember> {
        let mut partitions = BTreeMap::<Box<[ResultValue]>, Vec<&ChoiceCandidate>>::new();
        for candidate in self.candidates.values() {
            partitions
                .entry(candidate.partition_values.clone())
                .or_default()
                .push(candidate);
        }
        let mut chosen = Vec::<ChoiceCandidate>::new();
        for candidates in partitions.values() {
            if !partition_passes_having(&self.spec, candidates) {
                continue;
            }
            chosen.extend(select_partition_members(&self.spec.policy, candidates));
        }
        chosen.sort_by(|left, right| {
            left.partition_values
                .cmp(&right.partition_values)
                .then_with(|| left.case_id.cmp(&right.case_id))
        });
        chosen
            .into_iter()
            .enumerate()
            .map(|(ordinal, candidate)| ChoiceMember::from_candidate(ordinal as u128, candidate))
            .collect()
    }

    pub(crate) fn accept_member(
        &mut self,
        member: ChoiceMember,
    ) -> Result<bool, ChoiceRelationError> {
        if self.content_root.is_some() {
            return Err(ChoiceRelationError::MemberAfterClosure);
        }
        let ordinal = usize::try_from(member.ordinal)
            .map_err(|_| ChoiceRelationError::MemberOrdinalOutOfRange(member.ordinal))?;
        let expected_member = self
            .expected_members()?
            .get(ordinal)
            .ok_or(ChoiceRelationError::MemberOrdinalOutOfRange(member.ordinal))?;
        if expected_member != &member {
            return Err(ChoiceRelationError::MemberMismatch(member.ordinal));
        }
        if ordinal < self.members.len() {
            return if self.members[ordinal] == member {
                Ok(false)
            } else {
                Err(ChoiceRelationError::MemberMismatch(member.ordinal))
            };
        }
        if ordinal != self.members.len() {
            return Err(ChoiceRelationError::MemberOrderMismatch {
                expected: self.members.len() as u128,
                actual: member.ordinal,
            });
        }
        self.members.push(member);
        Ok(true)
    }

    pub(crate) fn close(
        &mut self,
        claimed_root: ChoiceContentRoot,
    ) -> Result<bool, ChoiceRelationError> {
        if let Some(existing) = self.content_root {
            return if existing == claimed_root {
                Ok(false)
            } else {
                Err(ChoiceRelationError::ContentRootConflict)
            };
        }
        let expected = self.expected_members()?;
        if expected != self.members.as_slice() {
            return Err(ChoiceRelationError::MembersIncomplete {
                expected: expected.len() as u128,
                actual: self.members.len() as u128,
            });
        }
        let derived = self.derive_content_root()?;
        if derived != claimed_root {
            return Err(ChoiceRelationError::ContentRootMismatch {
                claimed: claimed_root,
                derived,
            });
        }
        self.content_root = Some(derived);
        Ok(true)
    }

    pub(crate) fn prepare_content_root(&self) -> Result<ChoiceContentRoot, ChoiceRelationError> {
        let expected = self.expected_members()?;
        if expected != self.members.as_slice() {
            return Err(ChoiceRelationError::MembersIncomplete {
                expected: expected.len() as u128,
                actual: self.members.len() as u128,
            });
        }
        self.derive_content_root()
    }

    pub(crate) fn frontier_root(&self) -> ChoiceFrontierRoot {
        ChoiceFrontierRoot(hash_choice_state(
            CHOICE_FRONTIER_ROOT_V1,
            &self.spec,
            self.candidates.values(),
            self.input_seal,
            &self.members,
            self.content_root,
        ))
    }

    pub(crate) fn snapshot(&self) -> ChoiceRelationSnapshot {
        ChoiceRelationSnapshot {
            version: CHOICE_RELATION_VERSION,
            spec: self.spec.clone(),
            candidates: self.candidates.values().cloned().collect(),
            input_seal: self.input_seal,
            members: self.members.clone().into_boxed_slice(),
            content_root: self.content_root,
            frontier_root: self.frontier_root(),
        }
    }

    pub(crate) fn from_snapshot(
        snapshot: ChoiceRelationSnapshot,
    ) -> Result<Self, ChoiceRelationError> {
        if snapshot.version != CHOICE_RELATION_VERSION {
            return Err(ChoiceRelationError::UnsupportedVersion {
                actual: snapshot.version,
                expected: CHOICE_RELATION_VERSION,
            });
        }
        let claimed_frontier = snapshot.frontier_root;
        let mut builder = Self::new(snapshot.spec);
        for candidate in snapshot.candidates.into_vec() {
            builder.accept_candidate(candidate)?;
        }
        if let Some(seal) = snapshot.input_seal {
            builder.seal_input(seal)?;
        }
        for member in snapshot.members.into_vec() {
            builder.accept_member(member)?;
        }
        if let Some(content_root) = snapshot.content_root {
            builder.close(content_root)?;
        }
        if builder.frontier_root() != claimed_frontier {
            return Err(ChoiceRelationError::FrontierRootMismatch);
        }
        Ok(builder)
    }

    fn validate_candidate(&self, candidate: &ChoiceCandidate) -> Result<(), ChoiceRelationError> {
        if candidate.choice_id != self.spec.choice_id {
            return Err(ChoiceRelationError::ChoiceIdMismatch {
                expected: self.spec.choice_id,
                actual: candidate.choice_id,
            });
        }
        if candidate.partition_values.len() != self.spec.partition_value_count {
            return Err(ChoiceRelationError::CandidateShape("partition values"));
        }
        if candidate.measures.len() != self.spec.measure_count {
            return Err(ChoiceRelationError::CandidateShape("measures"));
        }
        if candidate.objectives.len() != self.spec.objective_count() {
            return Err(ChoiceRelationError::CandidateShape("objectives"));
        }
        Ok(())
    }

    fn derive_content_root(&self) -> Result<ChoiceContentRoot, ChoiceRelationError> {
        let input_seal = self.input_seal.ok_or(ChoiceRelationError::InputStillOpen)?;
        Ok(ChoiceContentRoot(hash_choice_state(
            CHOICE_CONTENT_ROOT_V1,
            &self.spec,
            std::iter::empty(),
            Some(input_seal),
            &self.members,
            None,
        )))
    }
}

fn partition_passes_having(spec: &ChoiceRelationSpec, candidates: &[&ChoiceCandidate]) -> bool {
    match spec.having {
        None => true,
        Some(ResultViewHaving::Varies { measure_index }) => {
            let mut values = candidates
                .iter()
                .map(|candidate| &candidate.measures[measure_index]);
            let Some(first) = values.next() else {
                return false;
            };
            values.any(|value| value != first)
        }
    }
}

fn select_partition_members(
    policy: &ResultViewChoice,
    candidates: &[&ChoiceCandidate],
) -> Vec<ChoiceCandidate> {
    match policy {
        ResultViewChoice::Optimize {
            cardinality,
            direction,
        } => {
            let best = candidates
                .iter()
                .map(|candidate| candidate.objectives[0])
                .reduce(|best, candidate| match direction {
                    ExploreOptimizeDirection::Minimize => best.min(candidate),
                    ExploreOptimizeDirection::Maximize => best.max(candidate),
                });
            let Some(best) = best else {
                return Vec::new();
            };
            let mut tied = candidates
                .iter()
                .copied()
                .filter(|candidate| candidate.objectives[0] == best)
                .cloned()
                .collect::<Vec<_>>();
            tied.sort_by_key(ChoiceCandidate::case_id);
            match cardinality {
                ExploreChooseCardinality::One => tied.into_iter().take(1).collect(),
                ExploreChooseCardinality::All => tied,
            }
        }
        ResultViewChoice::Pareto { directions } => {
            let mut frontier = Vec::<&ChoiceCandidate>::new();
            for candidate in candidates {
                if frontier
                    .iter()
                    .any(|existing| dominates(existing, candidate, directions))
                {
                    continue;
                }
                frontier.retain(|existing| !dominates(candidate, existing, directions));
                frontier.push(candidate);
            }
            frontier.sort_by_key(|candidate| candidate.case_id);
            frontier.into_iter().cloned().collect()
        }
    }
}

fn dominates(
    left: &ChoiceCandidate,
    right: &ChoiceCandidate,
    directions: &[ExploreOptimizeDirection],
) -> bool {
    let mut strictly_better = false;
    for ((left, right), direction) in left
        .objectives
        .iter()
        .zip(right.objectives.iter())
        .zip(directions)
    {
        match direction {
            ExploreOptimizeDirection::Minimize => {
                if left > right {
                    return false;
                }
                strictly_better |= left < right;
            }
            ExploreOptimizeDirection::Maximize => {
                if left < right {
                    return false;
                }
                strictly_better |= left > right;
            }
        }
    }
    strictly_better
}

fn hash_choice_state<'a>(
    domain: &[u8],
    spec: &ChoiceRelationSpec,
    candidates: impl IntoIterator<Item = &'a ChoiceCandidate>,
    input_seal: Option<ChoiceInputSeal>,
    members: &[ChoiceMember],
    content_root: Option<ChoiceContentRoot>,
) -> [u8; 32] {
    let candidates = candidates.into_iter().collect::<Vec<_>>();
    let mut hasher = ChoiceHasher::new(domain);
    hasher.u32(CHOICE_RELATION_VERSION);
    hash_spec(&mut hasher, spec);
    hasher.u128(candidates.len() as u128);
    for candidate in candidates {
        hash_candidate(&mut hasher, candidate);
    }
    match input_seal {
        None => hasher.tag(0x00),
        Some(seal) => {
            hasher.tag(0x01);
            hasher.digest(seal.question_id.bytes());
            hasher.digest(seal.selected_question_seal_id.bytes());
            hasher.digest(seal.case_set.root().bytes());
            hasher.u128(seal.case_set.count());
        }
    }
    hasher.u128(members.len() as u128);
    for member in members {
        hasher.u128(member.ordinal);
        hash_candidate(&mut hasher, &member.candidate);
    }
    match content_root {
        None => hasher.tag(0x00),
        Some(root) => {
            hasher.tag(0x01);
            hasher.digest(root.bytes());
        }
    }
    hasher.finish()
}

fn hash_spec(hasher: &mut ChoiceHasher, spec: &ChoiceRelationSpec) {
    hasher.digest(spec.choice_id.bytes());
    hasher.u128(spec.partition_value_count as u128);
    hasher.u128(spec.measure_count as u128);
    match spec.having {
        None => hasher.tag(0x00),
        Some(ResultViewHaving::Varies { measure_index }) => {
            hasher.tag(0x01);
            hasher.u128(measure_index as u128);
        }
    }
    match &spec.policy {
        ResultViewChoice::Optimize {
            cardinality,
            direction,
        } => {
            hasher.tag(0x01);
            hasher.tag(match cardinality {
                ExploreChooseCardinality::One => 0x01,
                ExploreChooseCardinality::All => 0x02,
            });
            hash_direction(hasher, *direction);
        }
        ResultViewChoice::Pareto { directions } => {
            hasher.tag(0x02);
            hasher.u128(directions.len() as u128);
            for direction in directions.iter().copied() {
                hash_direction(hasher, direction);
            }
        }
    }
}

fn hash_direction(hasher: &mut ChoiceHasher, direction: ExploreOptimizeDirection) {
    hasher.tag(match direction {
        ExploreOptimizeDirection::Minimize => 0x01,
        ExploreOptimizeDirection::Maximize => 0x02,
    });
}

fn hash_candidate(hasher: &mut ChoiceHasher, candidate: &ChoiceCandidate) {
    hasher.digest(candidate.choice_id.bytes());
    hasher.digest(candidate.case_id.bytes());
    hash_values(hasher, &candidate.partition_values);
    hash_values(hasher, &candidate.measures);
    hasher.u128(candidate.objectives.len() as u128);
    for objective in candidate.objectives.iter().copied() {
        hasher.i64(objective);
    }
}

fn hash_values(hasher: &mut ChoiceHasher, values: &[ResultValue]) {
    hasher.u128(values.len() as u128);
    for value in values {
        match value {
            ResultValue::Value(value) => {
                hasher.tag(0x01);
                hasher.digest(super::transition::canonical_explore_value_digest(value));
            }
            ResultValue::CaseId(case_id) => {
                hasher.tag(0x02);
                hasher.digest(case_id.bytes());
            }
            ResultValue::TransitionId(transition_id) => {
                hasher.tag(0x03);
                hasher.digest(transition_id.bytes());
            }
            ResultValue::SignatureId(signature_id) => {
                hasher.tag(0x04);
                hasher.digest(signature_id.request_id().bytes());
                hasher.digest(signature_id.bytes());
            }
            ResultValue::StructuralMechanismId(mechanism_id) => {
                hasher.tag(0x05);
                hasher.digest(mechanism_id.bytes());
            }
            ResultValue::ExecutionProfileId(profile_id) => {
                hasher.tag(0x06);
                hasher.digest(profile_id.bytes());
            }
        }
    }
}

struct ChoiceHasher(Sha256);

impl ChoiceHasher {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update((domain.len() as u64).to_be_bytes());
        hasher.update(domain);
        Self(hasher)
    }

    fn tag(&mut self, value: u8) {
        self.0.update([value]);
    }

    fn u32(&mut self, value: u32) {
        self.0.update(value.to_be_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.0.update(value.to_be_bytes());
    }

    fn i64(&mut self, value: i64) {
        self.0.update(value.to_be_bytes());
    }

    fn digest(&mut self, value: [u8; 32]) {
        self.0.update(value);
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ChoiceRelationError {
    UnsupportedVersion {
        actual: u32,
        expected: u32,
    },
    ChoiceIdMismatch {
        expected: ChoiceId,
        actual: ChoiceId,
    },
    UnknownHavingMeasure {
        measure_index: usize,
    },
    EmptyParetoObjectives,
    CandidateShape(&'static str),
    CandidateConflict(RelationalCaseId),
    CandidateAfterInputSeal,
    InputStillOpen,
    InputSealReplacement,
    InputSealAfterClosure,
    InputCaseSetMismatch,
    MemberAfterClosure,
    MemberOrdinalOutOfRange(u128),
    MemberOrderMismatch {
        expected: u128,
        actual: u128,
    },
    MemberMismatch(u128),
    MembersIncomplete {
        expected: u128,
        actual: u128,
    },
    ContentRootMismatch {
        claimed: ChoiceContentRoot,
        derived: ChoiceContentRoot,
    },
    ContentRootConflict,
    FrontierRootMismatch,
}

impl fmt::Display for ChoiceRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion { actual, expected } => write!(
                formatter,
                "unsupported choice relation version {actual}; expected {expected}"
            ),
            Self::ChoiceIdMismatch { .. } => {
                formatter.write_str("choice candidate belongs to another choice relation")
            }
            Self::UnknownHavingMeasure { .. } => {
                formatter.write_str("choice HAVING names an absent measure")
            }
            Self::EmptyParetoObjectives => {
                formatter.write_str("Pareto choice requires at least one objective")
            }
            Self::CandidateShape(component) => write!(
                formatter,
                "choice candidate has the wrong {component} shape"
            ),
            Self::CandidateConflict(_) => formatter
                .write_str("choice candidate CaseId was replayed with different semantic values"),
            Self::CandidateAfterInputSeal => {
                formatter.write_str("choice candidates cannot arrive after the input seal")
            }
            Self::InputStillOpen => formatter
                .write_str("choice membership cannot be exact before its question input closes"),
            Self::InputSealReplacement => {
                formatter.write_str("choice input seal cannot be replaced")
            }
            Self::InputSealAfterClosure => {
                formatter.write_str("choice input cannot be sealed after membership closure")
            }
            Self::InputCaseSetMismatch => formatter.write_str(
                "choice candidates do not equal the sealed selected-question population",
            ),
            Self::MemberAfterClosure => {
                formatter.write_str("choice members cannot arrive after closure")
            }
            Self::MemberOrdinalOutOfRange(_) => {
                formatter.write_str("choice member ordinal is outside the derived exact membership")
            }
            Self::MemberOrderMismatch { .. } => {
                formatter.write_str("choice members must arrive in canonical contiguous order")
            }
            Self::MemberMismatch(_) => formatter
                .write_str("choice member does not match the deterministic semantic choice"),
            Self::MembersIncomplete { .. } => {
                formatter.write_str("choice membership prefix is not the complete derived relation")
            }
            Self::ContentRootMismatch { .. } => {
                formatter.write_str("choice content-root claim does not match its canonical rows")
            }
            Self::ContentRootConflict => {
                formatter.write_str("choice relation was already closed under another content root")
            }
            Self::FrontierRootMismatch => {
                formatter.write_str("choice snapshot frontier root does not match its content")
            }
        }
    }
}

impl Error for ChoiceRelationError {}

#[cfg(test)]
mod tests {
    use super::super::relation::RelationalCaseId;
    use super::*;
    use crate::explore::ExploreValue;

    fn choice_id(seed: u64) -> ChoiceId {
        ChoiceId::from_journal_codec_bytes([seed as u8; 32])
    }

    fn case_id(seed: u64) -> RelationalCaseId {
        RelationalCaseId::from_journal_codec_bytes([seed as u8; 32])
    }

    fn int(value: i64) -> ResultValue {
        ResultValue::Value(ExploreValue::Int(value))
    }

    #[test]
    fn exact_choice_waits_for_input_seal_and_preserves_ties_having_and_pareto() {
        let id = choice_id(7);
        let spec = ChoiceRelationSpec::new(
            id,
            1,
            1,
            Some(ResultViewHaving::Varies { measure_index: 0 }),
            ResultViewChoice::Optimize {
                cardinality: ExploreChooseCardinality::All,
                direction: ExploreOptimizeDirection::Minimize,
            },
        )
        .unwrap();
        let mut builder = ChoiceRelationBuilder::new(spec);
        for (seed, partition, measure, objective) in [
            (1, 10, 1, 4),
            (2, 10, 2, 4),
            (3, 10, 3, 8),
            (4, 20, 5, 1),
            (5, 20, 5, 0),
        ] {
            builder
                .accept_candidate(ChoiceCandidate::new(
                    id,
                    case_id(seed),
                    vec![int(partition)],
                    vec![int(measure)],
                    vec![objective],
                ))
                .unwrap();
        }
        assert_eq!(builder.status(), ChoiceRelationStatus::InputOpen);
        assert_eq!(builder.counts().candidates(), ChoiceCount::LowerBound(5));
        assert_eq!(builder.counts().members(), ChoiceCount::Provisional(0));
        assert!(!builder.counts().members().is_exact());
        assert_eq!(
            builder.expected_members(),
            Err(ChoiceRelationError::InputStillOpen)
        );

        let target = MechanismTargetCaseSetCommitment::from_cases(
            builder.candidates().map(ChoiceCandidate::case_id),
        );
        let seal = ChoiceInputSeal::new(
            QuestionId::from_journal_codec_bytes([3; 32]),
            RelationalSelectedQuestionSealId::from_journal_codec_bytes([9; 32]),
            target,
        );
        builder.seal_input(seal).unwrap();
        assert_eq!(builder.counts().candidates(), ChoiceCount::Exact(5));
        assert_eq!(builder.counts().members(), ChoiceCount::Provisional(0));
        let expected = builder.expected_members().unwrap().to_vec();
        assert_eq!(expected.len(), 2);
        assert_eq!(expected[0].case_id(), case_id(1));
        assert_eq!(expected[1].case_id(), case_id(2));
        for member in expected {
            builder.accept_member(member).unwrap();
        }
        let root = builder.prepare_content_root().unwrap();
        builder.close(root).unwrap();
        assert_eq!(builder.counts().members(), ChoiceCount::Exact(2));

        let pareto = ChoiceRelationSpec::new(
            id,
            0,
            0,
            None,
            ResultViewChoice::Pareto {
                directions: vec![
                    ExploreOptimizeDirection::Minimize,
                    ExploreOptimizeDirection::Maximize,
                ]
                .into_boxed_slice(),
            },
        )
        .unwrap();
        let rows = [
            ChoiceCandidate::new(id, case_id(10), vec![], vec![], vec![1, 1]),
            ChoiceCandidate::new(id, case_id(11), vec![], vec![], vec![2, 3]),
            ChoiceCandidate::new(id, case_id(12), vec![], vec![], vec![3, 0]),
            ChoiceCandidate::new(id, case_id(13), vec![], vec![], vec![1, 1]),
        ];
        let refs = rows.iter().collect::<Vec<_>>();
        let selected = select_partition_members(pareto.policy(), &refs);
        assert_eq!(selected.len(), 3);
        assert_eq!(selected[0].case_id(), case_id(10));
        assert_eq!(selected[1].case_id(), case_id(11));
        assert_eq!(selected[2].case_id(), case_id(13));
    }
}
