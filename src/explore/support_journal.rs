//! Replayable semantic journal events for exact Explore support evidence.
//!
//! Every content-bearing event carries both the complete accepted value and a
//! claimed content identity. Replay validates the claim before applying the
//! event through the causal catalog boundary. Operational materialization
//! cursors, retained examples, immutable layer registrations, and work-frontier
//! progress deliberately live outside this semantic event vocabulary.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::support_cell::{
    SupportCardinality, SupportCell, SupportCellEvidenceId, SupportCellId, SupportCellSpace,
    SupportExtensionalTarget, SupportPartitionCertificate, SupportPartitionId,
    SupportPartitionKind, SupportProofObligationId,
};
use super::support_evidence::{
    SupportEvidenceCatalogBuilder, SupportEvidenceError, SupportEvidenceKind,
    SupportEvidenceRecord, SupportObligationRecord, SupportObligationRefinement,
    SupportObligationRefinementId,
};

const SUPPORT_JOURNAL_EVENT_HASH_V1: &[u8] = b"futuruna.explore.support-journal-event.v1";

pub(crate) const SUPPORT_JOURNAL_EVENT_SCHEMA_VERSION: u32 = 1;

/// Canonical digest of one support-journal event.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SupportJournalEventDigest([u8; 32]);

impl SupportJournalEventDigest {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// One semantic mutation of an exact support/evidence catalog.
///
/// Root-obligation declaration includes the complete obligation record, and a
/// refinement includes every complete child record. Those composite variants
/// are the atomic units needed to keep every durable replay prefix free of
/// dangling or unreachable obligations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SupportJournalEvent {
    CellInserted {
        cell_id: SupportCellId,
        cell: SupportCell,
    },
    RootCellDeclared {
        cell_id: SupportCellId,
    },
    RootFrontierSealed,
    PartitionAccepted {
        partition_id: SupportPartitionId,
        certificate: SupportPartitionCertificate,
    },
    LeafSealed {
        cell_id: SupportCellId,
    },
    RootObligationDeclared {
        obligation_id: SupportProofObligationId,
        obligation: SupportObligationRecord,
    },
    ObligationRefined {
        refinement_id: SupportObligationRefinementId,
        refinement: SupportObligationRefinement,
        child_obligations: Box<[SupportObligationRecord]>,
    },
    EvidenceAccepted {
        evidence_id: SupportCellEvidenceId,
        evidence: SupportEvidenceRecord,
    },
    ObligationFrontierSealed,
    CatalogSealed,
}

impl SupportJournalEvent {
    pub(crate) fn cell_inserted(cell: SupportCell) -> Self {
        Self::CellInserted {
            cell_id: cell.id(),
            cell,
        }
    }

    pub(crate) const fn root_cell_declared(cell_id: SupportCellId) -> Self {
        Self::RootCellDeclared { cell_id }
    }

    pub(crate) fn partition_accepted(certificate: SupportPartitionCertificate) -> Self {
        Self::PartitionAccepted {
            partition_id: certificate.id(),
            certificate,
        }
    }

    pub(crate) const fn leaf_sealed(cell_id: SupportCellId) -> Self {
        Self::LeafSealed { cell_id }
    }

    pub(crate) fn root_obligation_declared(obligation: SupportObligationRecord) -> Self {
        Self::RootObligationDeclared {
            obligation_id: obligation.id(),
            obligation,
        }
    }

    /// Build a canonical composite refinement event in refinement-child order.
    pub(crate) fn obligation_refined(
        refinement: SupportObligationRefinement,
        child_obligations: impl IntoIterator<Item = SupportObligationRecord>,
    ) -> Result<Self, SupportJournalError> {
        let refinement_id = refinement.id();
        let mut children_by_id = BTreeMap::new();
        for child in child_obligations {
            if children_by_id.insert(child.id(), child).is_some() {
                return Err(SupportJournalError::RefinementChildrenClaimMismatch { refinement_id });
            }
        }
        let mut canonical_children = Vec::with_capacity(children_by_id.len());
        for child_id in refinement.child_obligation_ids() {
            let child = children_by_id
                .remove(child_id)
                .ok_or(SupportJournalError::RefinementChildrenClaimMismatch { refinement_id })?;
            canonical_children.push(child);
        }
        if !children_by_id.is_empty() {
            return Err(SupportJournalError::RefinementChildrenClaimMismatch { refinement_id });
        }
        Ok(Self::ObligationRefined {
            refinement_id,
            refinement,
            child_obligations: canonical_children.into_boxed_slice(),
        })
    }

    pub(crate) fn evidence_accepted(evidence: SupportEvidenceRecord) -> Self {
        Self::EvidenceAccepted {
            evidence_id: evidence.id(),
            evidence,
        }
    }

    /// Apply one validated event to a causal replay prefix.
    pub(crate) fn apply(
        &self,
        catalog: &mut SupportEvidenceCatalogBuilder,
    ) -> Result<SupportJournalApply, SupportJournalError> {
        self.validate_claimed_ids()?;
        let changed = match self {
            Self::CellInserted { cell, .. } => catalog.insert_known_cell(cell.clone())?,
            Self::RootCellDeclared { cell_id } => catalog.declare_known_root_cell(*cell_id)?,
            Self::RootFrontierSealed => catalog.seal_root_frontier()?,
            Self::PartitionAccepted { certificate, .. } => {
                catalog.insert_known_partition(certificate.clone())?
            }
            Self::LeafSealed { cell_id } => catalog.seal_known_leaf(*cell_id)?,
            Self::RootObligationDeclared { obligation, .. } => {
                catalog.declare_root_obligation_record(obligation.clone())?
            }
            Self::ObligationRefined {
                refinement,
                child_obligations,
                ..
            } => catalog.insert_obligation_refinement_with_children(
                refinement.clone(),
                child_obligations.clone(),
            )?,
            Self::EvidenceAccepted { evidence, .. } => {
                catalog.insert_declared_evidence_record(evidence.clone())?
            }
            Self::ObligationFrontierSealed => catalog.seal_obligation_frontier()?,
            Self::CatalogSealed => catalog.seal_catalog()?,
        };
        Ok(if changed {
            SupportJournalApply::Applied
        } else {
            SupportJournalApply::AlreadyAccepted
        })
    }

    /// Hash the complete, content-addressed event payload in one stable domain.
    pub(crate) fn digest(&self) -> SupportJournalEventDigest {
        let mut hasher = SupportJournalHasher::new(SUPPORT_JOURNAL_EVENT_HASH_V1);
        hasher.u32(SUPPORT_JOURNAL_EVENT_SCHEMA_VERSION);
        match self {
            Self::CellInserted { cell_id, cell } => {
                hasher.tag(0x01);
                hasher.digest(cell_id.bytes());
                hash_cell(&mut hasher, cell);
            }
            Self::RootCellDeclared { cell_id } => {
                hasher.tag(0x02);
                hasher.digest(cell_id.bytes());
            }
            Self::RootFrontierSealed => hasher.tag(0x03),
            Self::PartitionAccepted {
                partition_id,
                certificate,
            } => {
                hasher.tag(0x04);
                hasher.digest(partition_id.bytes());
                hash_partition(&mut hasher, certificate);
            }
            Self::LeafSealed { cell_id } => {
                hasher.tag(0x05);
                hasher.digest(cell_id.bytes());
            }
            Self::RootObligationDeclared {
                obligation_id,
                obligation,
            } => {
                hasher.tag(0x06);
                hasher.digest(obligation_id.bytes());
                hash_obligation(&mut hasher, obligation);
            }
            Self::ObligationRefined {
                refinement_id,
                refinement,
                child_obligations,
            } => {
                hasher.tag(0x07);
                hasher.digest(refinement_id.bytes());
                hasher.digest(refinement.id().bytes());
                hasher.digest(refinement.parent_obligation_id().bytes());
                hasher.digest(refinement.partition_id().bytes());
                hasher.len(refinement.child_obligation_ids().len());
                for child_id in refinement.child_obligation_ids() {
                    hasher.digest(child_id.bytes());
                }
                hasher.len(child_obligations.len());
                for child in child_obligations {
                    hash_obligation(&mut hasher, child);
                }
            }
            Self::EvidenceAccepted {
                evidence_id,
                evidence,
            } => {
                hasher.tag(0x08);
                hasher.digest(evidence_id.bytes());
                hasher.digest(evidence.id().bytes());
                hasher.tag(evidence_kind_tag(evidence.kind()));
                hasher.digest(evidence.obligation_id().bytes());
                hasher.digest(evidence.cell_id().bytes());
                hasher.digest(evidence.conclusion_digest());
            }
            Self::ObligationFrontierSealed => hasher.tag(0x09),
            Self::CatalogSealed => hasher.tag(0x0a),
        }
        SupportJournalEventDigest(hasher.finish())
    }

    fn validate_claimed_ids(&self) -> Result<(), SupportJournalError> {
        match self {
            Self::CellInserted { cell_id, cell } if *cell_id != cell.id() => {
                Err(SupportJournalError::CellIdClaimMismatch {
                    claimed: *cell_id,
                    derived: cell.id(),
                })
            }
            Self::PartitionAccepted {
                partition_id,
                certificate,
            } if *partition_id != certificate.id() => {
                Err(SupportJournalError::PartitionIdClaimMismatch {
                    claimed: *partition_id,
                    derived: certificate.id(),
                })
            }
            Self::RootObligationDeclared {
                obligation_id,
                obligation,
            } if *obligation_id != obligation.id() => {
                Err(SupportJournalError::ObligationIdClaimMismatch {
                    claimed: *obligation_id,
                    derived: obligation.id(),
                })
            }
            Self::ObligationRefined {
                refinement_id,
                refinement,
                child_obligations,
            } => {
                if *refinement_id != refinement.id() {
                    return Err(SupportJournalError::RefinementIdClaimMismatch {
                        claimed: *refinement_id,
                        derived: refinement.id(),
                    });
                }
                if child_obligations
                    .iter()
                    .map(SupportObligationRecord::id)
                    .ne(refinement.child_obligation_ids().iter().copied())
                {
                    return Err(SupportJournalError::RefinementChildrenClaimMismatch {
                        refinement_id: *refinement_id,
                    });
                }
                Ok(())
            }
            Self::EvidenceAccepted {
                evidence_id,
                evidence,
            } if *evidence_id != evidence.id() => {
                Err(SupportJournalError::EvidenceIdClaimMismatch {
                    claimed: *evidence_id,
                    derived: evidence.id(),
                })
            }
            _ => Ok(()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SupportJournalApply {
    Applied,
    AlreadyAccepted,
}

impl SupportJournalApply {
    pub(crate) const fn changed(self) -> bool {
        matches!(self, Self::Applied)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SupportJournalError {
    Evidence(SupportEvidenceError),
    CellIdClaimMismatch {
        claimed: SupportCellId,
        derived: SupportCellId,
    },
    PartitionIdClaimMismatch {
        claimed: SupportPartitionId,
        derived: SupportPartitionId,
    },
    ObligationIdClaimMismatch {
        claimed: SupportProofObligationId,
        derived: SupportProofObligationId,
    },
    RefinementIdClaimMismatch {
        claimed: SupportObligationRefinementId,
        derived: SupportObligationRefinementId,
    },
    RefinementChildrenClaimMismatch {
        refinement_id: SupportObligationRefinementId,
    },
    EvidenceIdClaimMismatch {
        claimed: SupportCellEvidenceId,
        derived: SupportCellEvidenceId,
    },
}

impl From<SupportEvidenceError> for SupportJournalError {
    fn from(error: SupportEvidenceError) -> Self {
        Self::Evidence(error)
    }
}

impl fmt::Display for SupportJournalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Evidence(error) => fmt::Display::fmt(error, formatter),
            Self::CellIdClaimMismatch { .. } => {
                formatter.write_str("support journal cell ID claim does not match its content")
            }
            Self::PartitionIdClaimMismatch { .. } => {
                formatter.write_str("support journal partition ID claim does not match its content")
            }
            Self::ObligationIdClaimMismatch { .. } => formatter
                .write_str("support journal obligation ID claim does not match its content"),
            Self::RefinementIdClaimMismatch { .. } => formatter
                .write_str("support journal refinement ID claim does not match its content"),
            Self::RefinementChildrenClaimMismatch { .. } => formatter.write_str(
                "support journal refinement children do not match its canonical child IDs",
            ),
            Self::EvidenceIdClaimMismatch { .. } => {
                formatter.write_str("support journal evidence ID claim does not match its content")
            }
        }
    }
}

impl Error for SupportJournalError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Evidence(error) => Some(error),
            _ => None,
        }
    }
}

fn hash_cell(hasher: &mut SupportJournalHasher, cell: &SupportCell) {
    hasher.digest(cell.id().bytes());
    match cell.space() {
        SupportCellSpace::ProducerCoordinates(producer_id) => {
            hasher.tag(0x01);
            hasher.digest(producer_id.bytes());
        }
        SupportCellSpace::ExtensionalValues(target) => {
            hasher.tag(0x02);
            hash_extensional_target(hasher, target);
        }
        SupportCellSpace::MappedImage {
            producer_id,
            target,
        } => {
            hasher.tag(0x03);
            hasher.digest(producer_id.bytes());
            hash_extensional_target(hasher, target);
        }
    }
    hasher.digest(cell.expression().id().bytes());
    hasher.digest(cell.materializer_id().bytes());
}

fn hash_extensional_target(hasher: &mut SupportJournalHasher, target: SupportExtensionalTarget) {
    match target {
        SupportExtensionalTarget::SourceRows(relation_id) => {
            hasher.tag(0x01);
            hasher.digest(relation_id.bytes());
        }
        SupportExtensionalTarget::SuccessorRows(relation_id) => {
            hasher.tag(0x02);
            hasher.digest(relation_id.bytes());
        }
        SupportExtensionalTarget::Cases(relation_id) => {
            hasher.tag(0x03);
            hasher.digest(relation_id.bytes());
        }
        SupportExtensionalTarget::Derived(producer_id) => {
            hasher.tag(0x04);
            hasher.digest(producer_id.bytes());
        }
    }
}

fn hash_partition(hasher: &mut SupportJournalHasher, certificate: &SupportPartitionCertificate) {
    hasher.digest(certificate.id().bytes());
    hasher.tag(match certificate.kind() {
        SupportPartitionKind::OrdinalIntervalCover => 0x01,
        SupportPartitionKind::AcceptedDisjointUnion => 0x02,
        SupportPartitionKind::MappedInjectiveOrdinalCover => 0x03,
        SupportPartitionKind::ProductFactorCover => 0x04,
        SupportPartitionKind::MappedInjectiveProductFactorCover => 0x05,
        SupportPartitionKind::MappedInjectiveProductRankIntervalCover => 0x06,
    });
    hasher.digest(certificate.parent_id().bytes());
    hasher.len(certificate.child_ids().len());
    for child_id in certificate.child_ids() {
        hasher.digest(child_id.bytes());
    }
    match certificate.cardinality() {
        SupportCardinality::Exact(count) => {
            hasher.tag(0x01);
            hasher.u128(count);
        }
        SupportCardinality::Open {
            confirmed_lower_bound,
        } => {
            hasher.tag(0x02);
            hasher.u128(confirmed_lower_bound);
        }
    }
    let receipt = certificate.receipt();
    hasher.digest(receipt.id().bytes());
    hasher.digest(receipt.obligation_id().bytes());
    hasher.digest(receipt.verifier_id().bytes());
    hasher.digest(receipt.conclusion_digest());
    hasher.digest(receipt.proof_digest());
}

fn hash_obligation(hasher: &mut SupportJournalHasher, obligation: &SupportObligationRecord) {
    hasher.tag(evidence_kind_tag(obligation.kind()));
    hasher.digest(obligation.id().bytes());
    hasher.digest(obligation.cell_id().bytes());
}

const fn evidence_kind_tag(kind: SupportEvidenceKind) -> u8 {
    match kind {
        SupportEvidenceKind::Cardinality => 0x01,
        SupportEvidenceKind::Injectivity => 0x02,
        SupportEvidenceKind::Admission => 0x03,
        SupportEvidenceKind::Selection => 0x04,
        SupportEvidenceKind::UniformValue => 0x05,
        SupportEvidenceKind::UniformMechanism => 0x06,
    }
}

struct SupportJournalHasher(Sha256);

impl SupportJournalHasher {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = Self(Sha256::new());
        hasher.bytes(domain);
        hasher
    }

    fn tag(&mut self, value: u8) {
        self.0.update([value]);
    }

    fn bytes(&mut self, value: &[u8]) {
        self.0.update((value.len() as u128).to_be_bytes());
        self.0.update(value);
    }

    fn digest(&mut self, value: [u8; 32]) {
        self.0.update(value);
    }

    fn len(&mut self, value: usize) {
        self.0.update((value as u128).to_be_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.0.update(value.to_be_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.0.update(value.to_be_bytes());
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}
