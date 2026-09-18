//! Canonical carrier for bounded endpoint-observer totality evidence.
//!
//! This module deliberately owns no abstract interpreter and grants no replay
//! authority by itself. A proof producer supplies the Before and After proof-
//! domain commitments plus its abstract-proof commitment; this carrier binds
//! those values to one mechanism request and relation with a deterministic,
//! separately versioned identity. The `RelationId` commits the exact declared
//! relation; proof-domain roots commit the sound abstract over-approximation
//! on which totality was established.

use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::relation::{MechanismRequestId, RelationId};
use crate::ExprSiteId;

pub(crate) const RELATIONAL_ENDPOINT_TOTALITY_CERTIFICATE_VERSION: u32 = 1;

const CERTIFICATE_ID_V1: &[u8] =
    b"futuruna.explore.relational-endpoint-totality.certificate-id.v1\0";
const CERTIFICATE_VERSION_ROLE: u8 = 0x01;
const CERTIFICATE_REQUEST_ROLE: u8 = 0x02;
const CERTIFICATE_RELATION_ROLE: u8 = 0x03;
const CERTIFICATE_BEFORE_PROOF_DOMAIN_ROLE: u8 = 0x04;
const CERTIFICATE_AFTER_PROOF_DOMAIN_ROLE: u8 = 0x05;
const CERTIFICATE_ABSTRACT_PROOF_ROLE: u8 = 0x06;
const CERTIFICATE_OBLIGATION_COUNT_ROLE: u8 = 0x07;

/// Commitment to one endpoint proof domain under the totality prover's
/// canonical abstract encoding.
///
/// This is deliberately a sound over-approximation, not a materialized or
/// independently exact correlated starter set. Exact declared semantics remain
/// bound by the certificate's [`RelationId`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalEndpointProofDomainRoot([u8; 32]);

impl RelationalEndpointProofDomainRoot {
    pub(crate) const fn from_canonical_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Commitment to the complete normalized abstract proof for both endpoints.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalEndpointAbstractProofRoot([u8; 32]);

impl RelationalEndpointAbstractProofRoot {
    pub(crate) const fn from_canonical_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Content identity of one request-scoped endpoint-totality certificate.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalEndpointTotalityCertificateId([u8; 32]);

impl RelationalEndpointTotalityCertificateId {
    pub(crate) const fn from_canonical_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Cross-platform count of the deterministic obligations committed by a
/// proof. Zero remains representable for a vacuous exact source relation.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalEndpointTotalityObligationCount(u64);

impl RelationalEndpointTotalityObligationCount {
    pub(crate) const ZERO: Self = Self(0);

    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }

    pub(crate) fn try_from_usize(
        value: usize,
    ) -> Result<Self, RelationalEndpointTotalityCertificateError> {
        u64::try_from(value)
            .map(Self)
            .map_err(|_| RelationalEndpointTotalityCertificateError::ObligationCountOverflow)
    }

    pub(crate) fn checked_add(
        self,
        additional: Self,
    ) -> Result<Self, RelationalEndpointTotalityCertificateError> {
        self.0
            .checked_add(additional.0)
            .map(Self)
            .ok_or(RelationalEndpointTotalityCertificateError::ObligationCountOverflow)
    }

    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

/// Endpoint at which a source-linked totality obligation could not be closed.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum RelationalEndpointRole {
    Before,
    After,
}

impl RelationalEndpointRole {
    pub(crate) const fn canonical_tag(self) -> u8 {
        match self {
            Self::Before => 0x01,
            Self::After => 0x02,
        }
    }
}

/// Stable diagnostic category for one unclosed endpoint-totality obligation.
///
/// The `NotExcluded` reasons mean exactly that the abstract proof was
/// insufficient; they do not claim that the concrete failure must occur.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum RelationalEndpointTotalityIssueReason {
    ExactDomainUnavailable,
    CheckedResolutionUnavailable,
    UnsupportedExpression,
    UnknownCall,
    EffectfulCall,
    RecursiveCall,
    NonExhaustivePattern,
    PartialRuleDispatch,
    ArithmeticOverflowNotExcluded,
    DivisionByZeroNotExcluded,
    ProofCapacityExceeded,
}

impl RelationalEndpointTotalityIssueReason {
    pub(crate) const fn canonical_tag(self) -> u8 {
        match self {
            Self::ExactDomainUnavailable => 0x01,
            Self::CheckedResolutionUnavailable => 0x02,
            Self::UnsupportedExpression => 0x03,
            Self::UnknownCall => 0x04,
            Self::EffectfulCall => 0x05,
            Self::RecursiveCall => 0x06,
            Self::NonExhaustivePattern => 0x07,
            Self::PartialRuleDispatch => 0x08,
            Self::ArithmeticOverflowNotExcluded => 0x09,
            Self::DivisionByZeroNotExcluded => 0x0a,
            Self::ProofCapacityExceeded => 0x0b,
        }
    }
}

/// Source-linked explanation of why no certificate could be minted.
///
/// `detail` is intentionally display-only and concise.  It is not a semantic
/// address and does not replace the stable [`ExprSiteId`].
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalEndpointTotalityIssue {
    endpoint: RelationalEndpointRole,
    site: ExprSiteId,
    reason: RelationalEndpointTotalityIssueReason,
    detail: Box<str>,
}

impl RelationalEndpointTotalityIssue {
    pub(crate) fn new(
        endpoint: RelationalEndpointRole,
        site: ExprSiteId,
        reason: RelationalEndpointTotalityIssueReason,
        detail: impl Into<Box<str>>,
    ) -> Self {
        Self {
            endpoint,
            site,
            reason,
            detail: detail.into(),
        }
    }

    pub(crate) const fn endpoint(&self) -> RelationalEndpointRole {
        self.endpoint
    }

    pub(crate) const fn site(&self) -> &ExprSiteId {
        &self.site
    }

    pub(crate) const fn reason(&self) -> RelationalEndpointTotalityIssueReason {
        self.reason
    }

    pub(crate) fn detail(&self) -> &str {
        self.detail.as_ref()
    }
}

impl fmt::Display for RelationalEndpointTotalityIssue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:?} endpoint totality is unproved at {:?} ({:?}): {}",
            self.endpoint, self.site, self.reason, self.detail
        )
    }
}

impl Error for RelationalEndpointTotalityIssue {}

/// Canonical request-scoped evidence carrier for endpoint-observer totality.
///
/// The endpoint proof-domain roots deliberately precede WHERE/FIND or target
/// narrowing: a mechanism request may run the observer only after a proof has
/// closed it over sound over-approximations of both complete endpoint domains
/// induced by the exact relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalEndpointTotalityCertificate {
    schema_version: u32,
    request_id: MechanismRequestId,
    relation_id: RelationId,
    before_proof_domain_root: RelationalEndpointProofDomainRoot,
    after_proof_domain_root: RelationalEndpointProofDomainRoot,
    abstract_proof_root: RelationalEndpointAbstractProofRoot,
    obligation_count: RelationalEndpointTotalityObligationCount,
    certificate_id: RelationalEndpointTotalityCertificateId,
}

impl RelationalEndpointTotalityCertificate {
    pub(crate) fn new(
        request_id: MechanismRequestId,
        relation_id: RelationId,
        before_proof_domain_root: RelationalEndpointProofDomainRoot,
        after_proof_domain_root: RelationalEndpointProofDomainRoot,
        abstract_proof_root: RelationalEndpointAbstractProofRoot,
        obligation_count: RelationalEndpointTotalityObligationCount,
    ) -> Result<Self, RelationalEndpointTotalityCertificateError> {
        let mut certificate = Self {
            schema_version: RELATIONAL_ENDPOINT_TOTALITY_CERTIFICATE_VERSION,
            request_id,
            relation_id,
            before_proof_domain_root,
            after_proof_domain_root,
            abstract_proof_root,
            obligation_count,
            certificate_id: RelationalEndpointTotalityCertificateId([0; 32]),
        };
        certificate.certificate_id = derive_certificate_id(&certificate);
        certificate.validate_identity()?;
        Ok(certificate)
    }

    pub(crate) const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub(crate) const fn request_id(&self) -> MechanismRequestId {
        self.request_id
    }

    pub(crate) const fn relation_id(&self) -> RelationId {
        self.relation_id
    }

    pub(crate) const fn before_proof_domain_root(&self) -> RelationalEndpointProofDomainRoot {
        self.before_proof_domain_root
    }

    pub(crate) const fn after_proof_domain_root(&self) -> RelationalEndpointProofDomainRoot {
        self.after_proof_domain_root
    }

    pub(crate) const fn abstract_proof_root(&self) -> RelationalEndpointAbstractProofRoot {
        self.abstract_proof_root
    }

    pub(crate) const fn obligation_count(&self) -> RelationalEndpointTotalityObligationCount {
        self.obligation_count
    }

    pub(crate) const fn certificate_id(&self) -> RelationalEndpointTotalityCertificateId {
        self.certificate_id
    }

    /// Recompute every identity input under the supported schema version.
    /// Unknown versions and any payload/identity divergence fail closed.
    pub(crate) fn validate_identity(
        &self,
    ) -> Result<(), RelationalEndpointTotalityCertificateError> {
        if self.schema_version != RELATIONAL_ENDPOINT_TOTALITY_CERTIFICATE_VERSION {
            return Err(
                RelationalEndpointTotalityCertificateError::UnsupportedCertificateVersion(
                    self.schema_version,
                ),
            );
        }
        if derive_certificate_id(self) != self.certificate_id {
            return Err(RelationalEndpointTotalityCertificateError::CertificateIdentityMismatch);
        }
        Ok(())
    }
}

fn derive_certificate_id(
    certificate: &RelationalEndpointTotalityCertificate,
) -> RelationalEndpointTotalityCertificateId {
    let mut hasher = CertificateIdentityHasher::new();
    hasher.tag(CERTIFICATE_VERSION_ROLE);
    hasher.u32(certificate.schema_version);
    hasher.tag(CERTIFICATE_REQUEST_ROLE);
    hasher.digest(certificate.request_id.bytes());
    hasher.tag(CERTIFICATE_RELATION_ROLE);
    hasher.digest(certificate.relation_id.bytes());
    hasher.tag(CERTIFICATE_BEFORE_PROOF_DOMAIN_ROLE);
    hasher.digest(certificate.before_proof_domain_root.bytes());
    hasher.tag(CERTIFICATE_AFTER_PROOF_DOMAIN_ROLE);
    hasher.digest(certificate.after_proof_domain_root.bytes());
    hasher.tag(CERTIFICATE_ABSTRACT_PROOF_ROLE);
    hasher.digest(certificate.abstract_proof_root.bytes());
    hasher.tag(CERTIFICATE_OBLIGATION_COUNT_ROLE);
    hasher.u64(certificate.obligation_count.get());
    RelationalEndpointTotalityCertificateId(hasher.finish())
}

struct CertificateIdentityHasher(Sha256);

impl CertificateIdentityHasher {
    fn new() -> Self {
        let mut hasher = Sha256::new();
        hasher.update(CERTIFICATE_ID_V1);
        Self(hasher)
    }

    fn tag(&mut self, value: u8) {
        self.0.update([value]);
    }

    fn u32(&mut self, value: u32) {
        self.0.update(value.to_be_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.0.update(value.to_be_bytes());
    }

    fn digest(&mut self, value: [u8; 32]) {
        self.0.update(value);
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalEndpointTotalityCertificateError {
    UnsupportedCertificateVersion(u32),
    ObligationCountOverflow,
    CertificateIdentityMismatch,
}

impl fmt::Display for RelationalEndpointTotalityCertificateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedCertificateVersion(version) => write!(
                formatter,
                "relational endpoint-totality certificate version {version} is unsupported"
            ),
            Self::ObligationCountOverflow => formatter
                .write_str("relational endpoint-totality proof obligation count exceeds u64"),
            Self::CertificateIdentityMismatch => formatter.write_str(
                "relational endpoint-totality certificate identity does not match its payload",
            ),
        }
    }
}

impl Error for RelationalEndpointTotalityCertificateError {}
