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
    RelationalTransitionSupportRoot,
};
use super::StateId;

pub(crate) const RELATIONAL_SEMANTIC_TRANSITION_GRAPH_PROJECTION_VERSION: u32 = 1;
pub(crate) const RELATIONAL_SEMANTIC_TRANSITION_GRAPH_PROJECTION_SCHEMA: &str =
    "futuruna.relational-semantic-transition-graph.v1";
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalSemanticTransitionGraphClosure {
    root: RelationalTransitionSupportRoot,
    counts: RelationalTransitionSupportCounts,
    data_record_count: u128,
}

impl RelationalSemanticTransitionGraphClosure {
    pub(crate) const fn root(self) -> RelationalTransitionSupportRoot {
        self.root
    }

    pub(crate) const fn counts(self) -> RelationalTransitionSupportCounts {
        self.counts
    }

    pub(crate) const fn data_record_count(self) -> u128 {
        self.data_record_count
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalSemanticTransitionGraphCapacity {
    maximum_data_records: u128,
    required_data_records: u128,
    root: RelationalTransitionSupportRoot,
    counts: RelationalTransitionSupportCounts,
}

impl RelationalSemanticTransitionGraphCapacity {
    pub(crate) const fn maximum_data_records(self) -> u128 {
        self.maximum_data_records
    }

    pub(crate) const fn required_data_records(self) -> u128 {
        self.required_data_records
    }

    pub(crate) const fn root(self) -> RelationalTransitionSupportRoot {
        self.root
    }

    pub(crate) const fn counts(self) -> RelationalTransitionSupportCounts {
        self.counts
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalSemanticTransitionGraphUnmaterialized {
    logical_universe_cases: u128,
    materialized_universe_cases: u128,
    materialized_root: RelationalTransitionSupportRoot,
}

impl RelationalSemanticTransitionGraphUnmaterialized {
    pub(crate) const fn logical_universe_cases(self) -> u128 {
        self.logical_universe_cases
    }

    pub(crate) const fn materialized_universe_cases(self) -> u128 {
        self.materialized_universe_cases
    }

    pub(crate) const fn materialized_root(self) -> RelationalTransitionSupportRoot {
        self.materialized_root
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
        let data_record_count = graph_data_record_count(counts)?;
        let terminal = if scheduler.transition_support_is_extentionally_closed() {
            if data_record_count > RELATIONAL_SEMANTIC_TRANSITION_GRAPH_MAX_DATA_RECORDS_V1 {
                ProjectionTerminal::Capacity(RelationalSemanticTransitionGraphCapacity {
                    maximum_data_records: RELATIONAL_SEMANTIC_TRANSITION_GRAPH_MAX_DATA_RECORDS_V1,
                    required_data_records: data_record_count,
                    root: support.root(),
                    counts,
                })
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
                    if counts.cases(RelationalTransitionLayer::Universe) == 0
                        && counts.cases(RelationalTransitionLayer::Admitted) == 0
                        && counts.cases(RelationalTransitionLayer::Matched) == 0 =>
                {
                    ProjectionTerminal::Exact(RelationalSemanticTransitionGraphClosure {
                        root: support.root(),
                        counts,
                        data_record_count,
                    })
                }
                Some(logical_universe_cases) => ProjectionTerminal::Unmaterialized(
                    RelationalSemanticTransitionGraphUnmaterialized {
                        logical_universe_cases,
                        materialized_universe_cases: counts
                            .cases(RelationalTransitionLayer::Universe),
                        materialized_root: support.root(),
                    },
                ),
                None => ProjectionTerminal::Open,
            }
        } else {
            ProjectionTerminal::Open
        };
        Ok(Self {
            projection_id,
            contract,
            scheduler,
            terminal,
        })
    }

    pub(crate) const fn is_open(&self) -> bool {
        matches!(self.terminal, ProjectionTerminal::Open)
    }

    pub(crate) const fn terminal_record(&self) -> Option<RelationalSemanticTransitionGraphRecord> {
        match self.terminal {
            ProjectionTerminal::Open => None,
            ProjectionTerminal::Exact(closure) => {
                Some(RelationalSemanticTransitionGraphRecord::Closure(closure))
            }
            ProjectionTerminal::Capacity(capacity) => Some(
                RelationalSemanticTransitionGraphRecord::CapacityLimited(capacity),
            ),
            ProjectionTerminal::Unmaterialized(status) => Some(
                RelationalSemanticTransitionGraphRecord::Unmaterialized(status),
            ),
        }
    }

    pub(crate) fn available_source_record_count(&self) -> u128 {
        match self.terminal {
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
                contract: self.contract,
            }));
        }
        match self.terminal {
            ProjectionTerminal::Open => Ok(None),
            ProjectionTerminal::Capacity(capacity) => Ok((source_ordinal == 1).then_some(
                RelationalSemanticTransitionGraphRecord::CapacityLimited(capacity),
            )),
            ProjectionTerminal::Unmaterialized(status) => Ok((source_ordinal == 1).then_some(
                RelationalSemanticTransitionGraphRecord::Unmaterialized(status),
            )),
            ProjectionTerminal::Exact(closure) => {
                let data_ordinal = source_ordinal - 1;
                if data_ordinal == closure.data_record_count() {
                    return Ok(Some(RelationalSemanticTransitionGraphRecord::Closure(
                        closure,
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
        let universe_transitions = counts.transitions(RelationalTransitionLayer::Universe);
        if ordinal < universe_transitions {
            return support
                .transition_at_ordinal(ordinal)
                .map(|record| record.map(RelationalSemanticTransitionGraphRecord::Transition))
                .map_err(Into::into);
        }
        ordinal -= universe_transitions;
        for layer in [
            RelationalTransitionLayer::Universe,
            RelationalTransitionLayer::Admitted,
            RelationalTransitionLayer::Matched,
        ] {
            let layer_cases = counts.cases(layer);
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
    counts: RelationalTransitionSupportCounts,
) -> Result<u128, RelationalSemanticTransitionGraphProjectionError> {
    counts
        .states()
        .checked_add(counts.transitions(RelationalTransitionLayer::Universe))
        .and_then(|count| count.checked_add(counts.cases(RelationalTransitionLayer::Universe)))
        .and_then(|count| count.checked_add(counts.cases(RelationalTransitionLayer::Admitted)))
        .and_then(|count| count.checked_add(counts.cases(RelationalTransitionLayer::Matched)))
        .ok_or(RelationalSemanticTransitionGraphProjectionError::ArithmeticOverflow)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalSemanticTransitionGraphProjectionError {
    SchemaMismatch,
    ArithmeticOverflow,
    TransitionSupport(RelationalTransitionSupportError),
}

impl fmt::Display for RelationalSemanticTransitionGraphProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => formatter
                .write_str("semantic transition graph schema does not match its journal contract"),
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
