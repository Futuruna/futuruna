//! Identity-only publication of the complete layered semantic transition graph.
//!
//! Unlike the convenient selected-case edge list, this projection never
//! exposes typed Context/Before/After values. It publishes canonical StateId
//! and TransitionId nodes plus exact U/D/M CaseId support when the extensional
//! journal relation is closed. A proof-closed but not materialized run reports
//! that state explicitly instead of silently rerunning or changing identity.

use std::error::Error;
use std::fmt;

use super::relational_journal::{RelationalJournalContract, RelationalSchedulerView};
use super::relational_transition_support::{
    RelationalSemanticTransition, RelationalTransitionCaseSupport, RelationalTransitionLayer,
    RelationalTransitionSupportCounts, RelationalTransitionSupportError,
    RelationalTransitionSupportIndex, RelationalTransitionSupportRoot,
};
use super::StateId;

pub(crate) const RELATIONAL_SEMANTIC_TRANSITION_GRAPH_PROJECTION_VERSION: u32 = 2;
pub(crate) const RELATIONAL_SEMANTIC_TRANSITION_GRAPH_PROJECTION_SCHEMA: &str =
    "futuruna.relational-semantic-transition-graph.v2";
pub(crate) const RELATIONAL_SEMANTIC_TRANSITION_GRAPH_MAX_DATA_RECORDS_V1: u128 = 65_536;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalSemanticTransitionGraphProjectionId([u8; 32]);

impl RelationalSemanticTransitionGraphProjectionId {
    pub(crate) const fn from_checked_consumer(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalSemanticTransitionGraphClosure {
    root: RelationalTransitionSupportRoot,
    counts: RelationalTransitionSupportCounts,
    data_record_count: u128,
}

impl RelationalSemanticTransitionGraphClosure {
    pub(crate) const fn root(&self) -> RelationalTransitionSupportRoot {
        self.root
    }

    pub(crate) const fn counts(&self) -> &RelationalTransitionSupportCounts {
        &self.counts
    }

    pub(crate) const fn data_record_count(&self) -> u128 {
        self.data_record_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalSemanticTransitionGraphCapacity {
    maximum_data_records: u128,
    required_data_records: u128,
    root: RelationalTransitionSupportRoot,
    counts: RelationalTransitionSupportCounts,
}

impl RelationalSemanticTransitionGraphCapacity {
    pub(crate) fn from_retained_support(
        maximum_data_records: u128,
        support: &RelationalTransitionSupportIndex,
    ) -> Result<Option<Self>, RelationalSemanticTransitionGraphProjectionError> {
        let counts = support.counts();
        let required_data_records = graph_data_record_count(&counts)?;
        Ok(
            (required_data_records > maximum_data_records).then_some(Self {
                maximum_data_records,
                required_data_records,
                root: support.root(),
                counts,
            }),
        )
    }

    pub(crate) const fn maximum_data_records(&self) -> u128 {
        self.maximum_data_records
    }

    pub(crate) const fn required_data_records(&self) -> u128 {
        self.required_data_records
    }

    pub(crate) const fn root(&self) -> RelationalTransitionSupportRoot {
        self.root
    }

    pub(crate) const fn counts(&self) -> &RelationalTransitionSupportCounts {
        &self.counts
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalSemanticTransitionGraphUnmaterialized {
    logical_universe_cases: u128,
    materialized_universe_cases: u128,
    materialized_root: RelationalTransitionSupportRoot,
    counts: RelationalTransitionSupportCounts,
}

impl RelationalSemanticTransitionGraphUnmaterialized {
    pub(crate) fn from_retained_support(
        logical_universe_cases: u128,
        support: &RelationalTransitionSupportIndex,
    ) -> Self {
        let counts = support.counts();
        Self {
            logical_universe_cases,
            materialized_universe_cases: counts
                .cases(RelationalTransitionLayer::Universe)
                .expect("the universe transition layer is always registered"),
            materialized_root: support.root(),
            counts,
        }
    }

    pub(crate) const fn logical_universe_cases(&self) -> u128 {
        self.logical_universe_cases
    }

    pub(crate) const fn materialized_universe_cases(&self) -> u128 {
        self.materialized_universe_cases
    }

    pub(crate) const fn materialized_root(&self) -> RelationalTransitionSupportRoot {
        self.materialized_root
    }

    pub(crate) const fn counts(&self) -> &RelationalTransitionSupportCounts {
        &self.counts
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalSemanticTransitionGraphRecord {
    Header {
        projection_id: RelationalSemanticTransitionGraphProjectionId,
        contract: RelationalJournalContract,
    },
    State(StateId),
    Transition(RelationalSemanticTransition),
    CaseSupport(RelationalTransitionCaseSupport),
    Closure(RelationalSemanticTransitionGraphClosure),
    CapacityLimited(RelationalSemanticTransitionGraphCapacity),
    Unmaterialized(RelationalSemanticTransitionGraphUnmaterialized),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ProjectionTerminal {
    Open,
    Exact(RelationalSemanticTransitionGraphClosure),
    Capacity(RelationalSemanticTransitionGraphCapacity),
    Unmaterialized(RelationalSemanticTransitionGraphUnmaterialized),
}

pub(crate) struct RelationalSemanticTransitionGraphProjection<'journal> {
    projection_id: RelationalSemanticTransitionGraphProjectionId,
    contract: RelationalJournalContract,
    scheduler: RelationalSchedulerView<'journal>,
    terminal: ProjectionTerminal,
}

impl<'journal> RelationalSemanticTransitionGraphProjection<'journal> {
    pub(crate) fn derive(
        scheduler: RelationalSchedulerView<'journal>,
        projection_id: RelationalSemanticTransitionGraphProjectionId,
    ) -> Result<Self, RelationalSemanticTransitionGraphProjectionError> {
        let support = scheduler.transition_support();
        let contract = scheduler.contract();
        if support.state_schema_id() != contract.state_schema_id()
            || support.context_schema_id() != contract.context_schema_id()
            || support.transition_type_id() != contract.transition_type_id()
        {
            return Err(RelationalSemanticTransitionGraphProjectionError::SchemaMismatch);
        }
        let counts = support.counts();
        if !counts
            .matched()
            .map(|(question_id, _)| question_id)
            .eq(contract.question_ids().iter().copied())
        {
            return Err(RelationalSemanticTransitionGraphProjectionError::QuestionLayerSetMismatch);
        }
        let data_record_count = graph_data_record_count(&counts)?;
        let base_is_extentionally_closed = scheduler.relation_enumeration_is_complete()
            && scheduler.admission_decision_count() == scheduler.case_count();
        let questions_are_extentionally_closed =
            contract.question_ids().iter().copied().all(|question_id| {
                scheduler
                    .question_decision_count(question_id)
                    .is_ok_and(|count| count == scheduler.admitted_count())
            });
        let transition_support_is_extentionally_closed =
            base_is_extentionally_closed && questions_are_extentionally_closed;
        let terminal = if transition_support_is_extentionally_closed {
            if data_record_count > RELATIONAL_SEMANTIC_TRANSITION_GRAPH_MAX_DATA_RECORDS_V1 {
                ProjectionTerminal::Capacity(
                    RelationalSemanticTransitionGraphCapacity::from_retained_support(
                        RELATIONAL_SEMANTIC_TRANSITION_GRAPH_MAX_DATA_RECORDS_V1,
                        support,
                    )?
                    .expect("the checked data-record count exceeds publication capacity"),
                )
            } else {
                ProjectionTerminal::Exact(RelationalSemanticTransitionGraphClosure {
                    root: support.root(),
                    counts,
                    data_record_count,
                })
            }
        } else if scheduler.analysis_is_closed() && scheduler.support_catalog_is_sealed() {
            match scheduler.certified_root_case_cardinality() {
                Some(0)
                    if counts.cases(RelationalTransitionLayer::Universe) == Some(0)
                        && counts.cases(RelationalTransitionLayer::Admitted) == Some(0)
                        && counts.matched().all(|(_, matched)| matched.cases() == 0) =>
                {
                    ProjectionTerminal::Exact(RelationalSemanticTransitionGraphClosure {
                        root: support.root(),
                        counts,
                        data_record_count,
                    })
                }
                Some(logical_universe_cases) => ProjectionTerminal::Unmaterialized(
                    RelationalSemanticTransitionGraphUnmaterialized::from_retained_support(
                        logical_universe_cases,
                        support,
                    ),
                ),
                None => ProjectionTerminal::Open,
            }
        } else {
            ProjectionTerminal::Open
        };
        Ok(Self {
            projection_id,
            contract: contract.clone(),
            scheduler,
            terminal,
        })
    }

    pub(crate) const fn is_open(&self) -> bool {
        matches!(&self.terminal, ProjectionTerminal::Open)
    }

    pub(crate) fn terminal_record(&self) -> Option<RelationalSemanticTransitionGraphRecord> {
        match &self.terminal {
            ProjectionTerminal::Open => None,
            ProjectionTerminal::Exact(closure) => Some(
                RelationalSemanticTransitionGraphRecord::Closure(closure.clone()),
            ),
            ProjectionTerminal::Capacity(capacity) => Some(
                RelationalSemanticTransitionGraphRecord::CapacityLimited(capacity.clone()),
            ),
            ProjectionTerminal::Unmaterialized(status) => Some(
                RelationalSemanticTransitionGraphRecord::Unmaterialized(status.clone()),
            ),
        }
    }

    pub(crate) fn available_source_record_count(&self) -> u128 {
        match &self.terminal {
            ProjectionTerminal::Open => 1,
            ProjectionTerminal::Exact(closure) => 1_u128
                .checked_add(closure.data_record_count())
                .and_then(|count| count.checked_add(1))
                .expect("bounded transition graph record count"),
            ProjectionTerminal::Capacity(_) | ProjectionTerminal::Unmaterialized(_) => 2,
        }
    }

    pub(crate) fn record_at(
        &self,
        source_ordinal: u128,
    ) -> Result<
        Option<RelationalSemanticTransitionGraphRecord>,
        RelationalSemanticTransitionGraphProjectionError,
    > {
        if source_ordinal == 0 {
            return Ok(Some(RelationalSemanticTransitionGraphRecord::Header {
                projection_id: self.projection_id,
                contract: self.contract.clone(),
            }));
        }
        match &self.terminal {
            ProjectionTerminal::Open => Ok(None),
            ProjectionTerminal::Capacity(capacity) => Ok((source_ordinal == 1).then_some(
                RelationalSemanticTransitionGraphRecord::CapacityLimited(capacity.clone()),
            )),
            ProjectionTerminal::Unmaterialized(status) => Ok((source_ordinal == 1).then_some(
                RelationalSemanticTransitionGraphRecord::Unmaterialized(status.clone()),
            )),
            ProjectionTerminal::Exact(closure) => {
                let data_ordinal = source_ordinal - 1;
                if data_ordinal == closure.data_record_count() {
                    return Ok(Some(RelationalSemanticTransitionGraphRecord::Closure(
                        closure.clone(),
                    )));
                }
                if data_ordinal > closure.data_record_count() {
                    return Ok(None);
                }
                self.data_record_at(data_ordinal)
            }
        }
    }

    fn data_record_at(
        &self,
        mut ordinal: u128,
    ) -> Result<
        Option<RelationalSemanticTransitionGraphRecord>,
        RelationalSemanticTransitionGraphProjectionError,
    > {
        let support = self.scheduler.transition_support();
        let counts = support.counts();
        if ordinal < counts.states() {
            return support
                .state_at_ordinal(ordinal)
                .map(|record| record.map(RelationalSemanticTransitionGraphRecord::State))
                .map_err(Into::into);
        }
        ordinal -= counts.states();
        let universe_transitions = counts
            .transitions(RelationalTransitionLayer::Universe)
            .ok_or(RelationalSemanticTransitionGraphProjectionError::UnknownLayer)?;
        if ordinal < universe_transitions {
            return support
                .transition_at_ordinal(ordinal)
                .map(|record| record.map(RelationalSemanticTransitionGraphRecord::Transition))
                .map_err(Into::into);
        }
        ordinal -= universe_transitions;
        let layers = [
            RelationalTransitionLayer::Universe,
            RelationalTransitionLayer::Admitted,
        ]
        .into_iter()
        .chain(
            counts
                .matched()
                .map(|(question_id, _)| RelationalTransitionLayer::Matched(question_id)),
        );
        for layer in layers {
            let layer_cases = counts
                .cases(layer)
                .ok_or(RelationalSemanticTransitionGraphProjectionError::UnknownLayer)?;
            if ordinal < layer_cases {
                return support
                    .support_at_ordinal(layer, ordinal)
                    .map(|record| record.map(RelationalSemanticTransitionGraphRecord::CaseSupport))
                    .map_err(Into::into);
            }
            ordinal -= layer_cases;
        }
        Ok(None)
    }
}

fn graph_data_record_count(
    counts: &RelationalTransitionSupportCounts,
) -> Result<u128, RelationalSemanticTransitionGraphProjectionError> {
    let universe_transitions = counts
        .transitions(RelationalTransitionLayer::Universe)
        .ok_or(RelationalSemanticTransitionGraphProjectionError::UnknownLayer)?;
    let universe_cases = counts
        .cases(RelationalTransitionLayer::Universe)
        .ok_or(RelationalSemanticTransitionGraphProjectionError::UnknownLayer)?;
    let admitted_cases = counts
        .cases(RelationalTransitionLayer::Admitted)
        .ok_or(RelationalSemanticTransitionGraphProjectionError::UnknownLayer)?;
    let mut total = counts
        .states()
        .checked_add(universe_transitions)
        .and_then(|count| count.checked_add(universe_cases))
        .and_then(|count| count.checked_add(admitted_cases))
        .ok_or(RelationalSemanticTransitionGraphProjectionError::ArithmeticOverflow)?;
    for (_, matched) in counts.matched() {
        total = total
            .checked_add(matched.cases())
            .ok_or(RelationalSemanticTransitionGraphProjectionError::ArithmeticOverflow)?;
    }
    Ok(total)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalSemanticTransitionGraphProjectionError {
    SchemaMismatch,
    QuestionLayerSetMismatch,
    UnknownLayer,
    ArithmeticOverflow,
    TransitionSupport(RelationalTransitionSupportError),
}

impl fmt::Display for RelationalSemanticTransitionGraphProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => formatter
                .write_str("semantic transition graph schema does not match its journal contract"),
            Self::QuestionLayerSetMismatch => formatter.write_str(
                "semantic transition graph question layers do not match its journal contract",
            ),
            Self::UnknownLayer => formatter
                .write_str("semantic transition graph names an unregistered question layer"),
            Self::ArithmeticOverflow => {
                formatter.write_str("semantic transition graph record count overflowed")
            }
            Self::TransitionSupport(error) => error.fmt(formatter),
        }
    }
}

impl Error for RelationalSemanticTransitionGraphProjectionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::TransitionSupport(error) => Some(error),
            _ => None,
        }
    }
}

impl From<RelationalTransitionSupportError> for RelationalSemanticTransitionGraphProjectionError {
    fn from(error: RelationalTransitionSupportError) -> Self {
        Self::TransitionSupport(error)
    }
}
