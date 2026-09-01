//! Canonical semantic identities for before-to-after Explore transitions.
//!
//! Search coordinates and their exact support fibers remain in the reducer's
//! authenticated classification frontier. This module supplies the separate
//! semantic projection: role-neutral state nodes and directional transition
//! edges.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::Arc;

use sha2::{Digest, Sha256};

use crate::CheckedDataTypeId;

use super::{ExploreValue, Ty};

const RELATIONAL_SCHEMA_ENCODING_V1: &[u8] = b"futuruna.explore.relational-transition-schema.v1";
const TRANSITION_TYPE_ENCODING_V1: &[u8] = b"futuruna.explore.transition-type-id.v1";
const STATE_ID_HASH_V1: &[u8] = b"futuruna.explore.state-id.v1";
const TRANSITION_ID_HASH_V1: &[u8] = b"futuruna.explore.transition-id.v1";
const EXPLORE_VALUE_ID_HASH_V1: &[u8] = b"futuruna.explore.value-id.v1";

const STATE_SCHEMA_ROLE: u8 = 0x01;
const STATE_VALUE_ROLE: u8 = 0x02;

const TRANSITION_SCHEMA_ROLE: u8 = 0x01;
const TRANSITION_CONTEXT_ROLE: u8 = 0x02;
const TRANSITION_BEFORE_ROLE: u8 = 0x03;
const TRANSITION_AFTER_ROLE: u8 = 0x04;

const VALUE_INT: u8 = 0x01;
const VALUE_FLOAT_BITS: u8 = 0x02;
const VALUE_STRING: u8 = 0x03;
const VALUE_CHARACTER: u8 = 0x04;
const VALUE_BOOLEAN: u8 = 0x05;
const VALUE_UNIT: u8 = 0x06;
const VALUE_LIST: u8 = 0x07;
const VALUE_SET: u8 = 0x08;
const VALUE_TUPLE: u8 = 0x09;
const VALUE_CONSTRUCTOR: u8 = 0x0a;

const RELATIONAL_STATE_SCHEMA: u8 = 0x01;
const RELATIONAL_CONTEXT_SCHEMA: u8 = 0x02;

const DECLARED_OWNER_OCCURRENCE: u8 = 0x01;
const DECLARED_OWNER_INTRINSIC: u8 = 0x02;

const TYPE_NAME: u8 = 0x01;
const TYPE_APPLICATION: u8 = 0x02;
const TYPE_ARROW: u8 = 0x03;
const TYPE_REFERENCE: u8 = 0x04;
const TYPE_MUTABLE_REFERENCE: u8 = 0x05;
const TYPE_SHARED: u8 = 0x06;
const TYPE_VARIABLE: u8 = 0x08;
const TYPE_UNIT: u8 = 0x09;
const TYPE_HOLE: u8 = 0x0a;

/// Producer-side identities of the closed semantic transition schemas.
///
/// These identities intentionally exclude query syntax, generator bounds and
/// domains, question/output projections, membership/validity predicates,
/// compact aliases, source spans, boundary optimizer hints, transition mode,
/// and after-construction recipes. State and Context contribute their checked
/// nominal owners and canonical typed layouts. Generator provenance remains in
/// CaseId support; computation differences remain mechanism evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TransitionSchemaIdentities {
    state_schema_id: StateSchemaId,
    context_schema_id: ContextSchemaId,
    transition_type_id: TransitionTypeId,
    state_schema_preimage: Arc<[u8]>,
    context_schema_preimage: Arc<[u8]>,
    transition_type_preimage: Arc<[u8]>,
}

impl TransitionSchemaIdentities {
    /// Derive semantic transition schemas directly from the checked
    /// relational query's closed `(Before, Context)` types.
    ///
    /// This deliberately has its own canonical domain. Relational Explore has
    /// no authored product-schema identity, and manufacturing one would make
    /// a legacy lowering choice part of the proof boundary. Checked nominal
    /// owners still enter recursively through `encode_ty`, so equal spellings
    /// from different declarations cannot alias.
    pub(crate) fn derive_checked_relational(
        before_ty: &Ty,
        context_ty: &Ty,
        resolved_type_owners: &BTreeMap<Box<str>, CheckedDataTypeId>,
    ) -> Result<Self, String> {
        let state_schema_preimage = encode_relational_schema(
            "Before",
            RELATIONAL_STATE_SCHEMA,
            before_ty,
            resolved_type_owners,
        )?;
        let context_schema_preimage = encode_relational_schema(
            "Context",
            RELATIONAL_CONTEXT_SCHEMA,
            context_ty,
            resolved_type_owners,
        )?;
        Ok(Self::from_schema_preimages(
            state_schema_preimage,
            context_schema_preimage,
        ))
    }

    fn from_schema_preimages(
        state_schema_preimage: Arc<[u8]>,
        context_schema_preimage: Arc<[u8]>,
    ) -> Self {
        let state_schema_id = StateSchemaId::derive(&state_schema_preimage);
        let context_schema_id = ContextSchemaId::derive(&context_schema_preimage);

        let mut encoder = CanonicalEncoder::new(TRANSITION_TYPE_ENCODING_V1);
        encoder.bytes(context_schema_id.as_ref());
        encoder.bytes(state_schema_id.as_ref());
        let transition_type_preimage = encoder.finish();
        let transition_type_id = TransitionTypeId::derive(&transition_type_preimage);

        Self {
            state_schema_id,
            context_schema_id,
            transition_type_id,
            state_schema_preimage,
            context_schema_preimage,
            transition_type_preimage,
        }
    }

    pub(crate) fn instantiate(
        &self,
        context: ExploreValue,
        before: ExploreValue,
        after: ExploreValue,
    ) -> TransitionInstance {
        TransitionInstance::from_shared_schema(
            self.state_schema_id,
            self.context_schema_id,
            self.transition_type_id,
            self.state_schema_preimage.clone(),
            self.context_schema_preimage.clone(),
            self.transition_type_preimage.clone(),
            context,
            before,
            after,
        )
    }

    /// Rehydrate one self-contained durable transition and additionally bind
    /// it to this checked query's exact schema preimages.
    ///
    /// [`TransitionInstance::from_canonical_v1`] proves that the record is
    /// internally self-consistent. This stronger boundary also prevents a
    /// different, self-consistent schema from being replayed into a run whose
    /// checked query selected these identities.
    pub(crate) fn rehydrate_canonical_v1(
        &self,
        canonical: TransitionInstanceCanonicalV1,
    ) -> Result<TransitionInstance, TransitionIdentityError> {
        if canonical.state_schema_id != self.state_schema_id
            || canonical.state_schema_preimage.as_ref() != self.state_schema_preimage.as_ref()
        {
            return Err(TransitionIdentityError::CheckedStateSchemaMismatch);
        }
        if canonical.context_schema_id != self.context_schema_id
            || canonical.context_schema_preimage.as_ref() != self.context_schema_preimage.as_ref()
        {
            return Err(TransitionIdentityError::CheckedContextSchemaMismatch);
        }
        if canonical.transition_type_id != self.transition_type_id
            || canonical.transition_type_preimage.as_ref() != self.transition_type_preimage.as_ref()
        {
            return Err(TransitionIdentityError::CheckedTransitionTypeMismatch);
        }
        TransitionInstance::from_canonical_v1(canonical)
    }

    /// Rehydrate a compact durable claim whose schema identities are supplied
    /// by this already checked run contract. The resulting transition shares
    /// the schema preimage allocations instead of copying them once per case.
    pub(crate) fn rehydrate_checked_claim_v1(
        &self,
        before_state_id: StateId,
        after_state_id: StateId,
        transition_id: TransitionId,
        context: ExploreValue,
        before: ExploreValue,
        after: ExploreValue,
    ) -> Result<TransitionInstance, TransitionIdentityError> {
        let transition = TransitionInstance::from_shared_schema(
            self.state_schema_id,
            self.context_schema_id,
            self.transition_type_id,
            self.state_schema_preimage.clone(),
            self.context_schema_preimage.clone(),
            self.transition_type_preimage.clone(),
            context,
            before,
            after,
        );
        if transition.before_state_id != before_state_id {
            return Err(TransitionIdentityError::BeforeStateIdMismatch);
        }
        if transition.after_state_id != after_state_id {
            return Err(TransitionIdentityError::AfterStateIdMismatch);
        }
        if transition.id != transition_id {
            return Err(TransitionIdentityError::TransitionIdMismatch);
        }
        Ok(transition)
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

    pub(crate) fn state_schema_preimage(&self) -> &[u8] {
        &self.state_schema_preimage
    }

    pub(crate) fn context_schema_preimage(&self) -> &[u8] {
        &self.context_schema_preimage
    }

    pub(crate) fn transition_type_preimage(&self) -> &[u8] {
        &self.transition_type_preimage
    }
}

/// Canonical identity of one closed State product schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct StateSchemaId([u8; 32]);

impl StateSchemaId {
    fn derive(preimage: &[u8]) -> Self {
        Self(Sha256::digest(preimage).into())
    }

    const fn as_ref(&self) -> &[u8; 32] {
        &self.0
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }

    pub(crate) const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

/// Canonical identity of one closed Context product schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ContextSchemaId([u8; 32]);

impl ContextSchemaId {
    fn derive(preimage: &[u8]) -> Self {
        Self(Sha256::digest(preimage).into())
    }

    const fn as_ref(&self) -> &[u8; 32] {
        &self.0
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }

    pub(crate) const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

/// Canonical identity of the typed Context + State transition relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct TransitionTypeId([u8; 32]);

impl TransitionTypeId {
    fn derive(preimage: &[u8]) -> Self {
        Self(Sha256::digest(preimage).into())
    }

    const fn as_ref(&self) -> &[u8; 32] {
        &self.0
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }

    pub(crate) const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

/// Role-neutral identity of one canonical value under one state schema.
///
/// `before` and `after` are deliberately absent from this identity. The same
/// typed value therefore denotes the same graph node in either endpoint role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct StateId([u8; 32]);

impl StateId {
    pub(crate) fn derive(state_schema_id: StateSchemaId, value: &ExploreValue) -> Self {
        let mut hasher = CanonicalHasher::new(STATE_ID_HASH_V1);
        hasher.tag(STATE_SCHEMA_ROLE);
        hasher.bytes(state_schema_id.as_ref());
        hasher.tag(STATE_VALUE_ROLE);
        hash_explore_value(&mut hasher, value);
        Self(hasher.finish())
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }

    pub(crate) const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

/// Directional identity of one canonical context/before/after edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct TransitionId([u8; 32]);

impl TransitionId {
    fn derive(
        transition_type_id: TransitionTypeId,
        context: &ExploreValue,
        before: StateId,
        after: StateId,
    ) -> Self {
        let mut hasher = CanonicalHasher::new(TRANSITION_ID_HASH_V1);
        hasher.tag(TRANSITION_SCHEMA_ROLE);
        hasher.bytes(transition_type_id.as_ref());
        hasher.tag(TRANSITION_CONTEXT_ROLE);
        hash_explore_value(&mut hasher, context);
        hasher.tag(TRANSITION_BEFORE_ROLE);
        hasher.bytes(&before.0);
        hasher.tag(TRANSITION_AFTER_ROLE);
        hasher.bytes(&after.0);
        Self(hasher.finish())
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }

    pub(crate) const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

/// Self-contained canonical wire projection of one semantic transition.
///
/// Every digest is retained as a claim rather than trusted input. Durable
/// replay reconstructs a [`TransitionInstance`] with
/// [`TransitionInstance::from_canonical_v1`], which rederives the schema,
/// state, and edge identities from these exact preimages and values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TransitionInstanceCanonicalV1 {
    state_schema_id: StateSchemaId,
    context_schema_id: ContextSchemaId,
    transition_type_id: TransitionTypeId,
    before_state_id: StateId,
    after_state_id: StateId,
    transition_id: TransitionId,
    state_schema_preimage: Arc<[u8]>,
    context_schema_preimage: Arc<[u8]>,
    transition_type_preimage: Arc<[u8]>,
    context: ExploreValue,
    before: ExploreValue,
    after: ExploreValue,
}

impl TransitionInstanceCanonicalV1 {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        state_schema_id: StateSchemaId,
        context_schema_id: ContextSchemaId,
        transition_type_id: TransitionTypeId,
        before_state_id: StateId,
        after_state_id: StateId,
        transition_id: TransitionId,
        state_schema_preimage: impl Into<Arc<[u8]>>,
        context_schema_preimage: impl Into<Arc<[u8]>>,
        transition_type_preimage: impl Into<Arc<[u8]>>,
        context: ExploreValue,
        before: ExploreValue,
        after: ExploreValue,
    ) -> Self {
        Self {
            state_schema_id,
            context_schema_id,
            transition_type_id,
            before_state_id,
            after_state_id,
            transition_id,
            state_schema_preimage: state_schema_preimage.into(),
            context_schema_preimage: context_schema_preimage.into(),
            transition_type_preimage: transition_type_preimage.into(),
            context,
            before,
            after,
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

    pub(crate) const fn before_state_id(&self) -> StateId {
        self.before_state_id
    }

    pub(crate) const fn after_state_id(&self) -> StateId {
        self.after_state_id
    }

    pub(crate) const fn transition_id(&self) -> TransitionId {
        self.transition_id
    }

    pub(crate) fn state_schema_preimage(&self) -> &[u8] {
        &self.state_schema_preimage
    }

    pub(crate) fn context_schema_preimage(&self) -> &[u8] {
        &self.context_schema_preimage
    }

    pub(crate) fn transition_type_preimage(&self) -> &[u8] {
        &self.transition_type_preimage
    }

    pub(crate) fn context(&self) -> &ExploreValue {
        &self.context
    }

    pub(crate) fn before(&self) -> &ExploreValue {
        &self.before
    }

    pub(crate) fn after(&self) -> &ExploreValue {
        &self.after
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TransitionIdentityError {
    StateSchemaIdMismatch,
    ContextSchemaIdMismatch,
    TransitionTypePreimageMismatch,
    TransitionTypeIdMismatch,
    BeforeStateIdMismatch,
    AfterStateIdMismatch,
    TransitionIdMismatch,
    CheckedStateSchemaMismatch,
    CheckedContextSchemaMismatch,
    CheckedTransitionTypeMismatch,
}

impl fmt::Display for TransitionIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::StateSchemaIdMismatch => {
                "canonical transition State schema ID does not match its preimage"
            }
            Self::ContextSchemaIdMismatch => {
                "canonical transition Context schema ID does not match its preimage"
            }
            Self::TransitionTypePreimageMismatch => {
                "canonical transition-type preimage does not bind Context then State"
            }
            Self::TransitionTypeIdMismatch => {
                "canonical transition-type ID does not match its preimage"
            }
            Self::BeforeStateIdMismatch => {
                "canonical transition before-State ID does not match its value"
            }
            Self::AfterStateIdMismatch => {
                "canonical transition after-State ID does not match its value"
            }
            Self::TransitionIdMismatch => {
                "canonical transition ID does not match its context and endpoints"
            }
            Self::CheckedStateSchemaMismatch => {
                "canonical transition State schema does not match the checked Explore query"
            }
            Self::CheckedContextSchemaMismatch => {
                "canonical transition Context schema does not match the checked Explore query"
            }
            Self::CheckedTransitionTypeMismatch => {
                "canonical transition type does not match the checked Explore query"
            }
        };
        formatter.write_str(message)
    }
}

impl Error for TransitionIdentityError {}

/// One normalized semantic edge and the canonical values that produced it.
///
/// Schema preimages are retained privately so the support interner can reject
/// a SHA-256 collision instead of silently treating unequal edges as equal.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TransitionInstance {
    id: TransitionId,
    before_state_id: StateId,
    after_state_id: StateId,
    state_schema_id: StateSchemaId,
    context_schema_id: ContextSchemaId,
    transition_type_id: TransitionTypeId,
    state_schema_preimage: Arc<[u8]>,
    context_schema_preimage: Arc<[u8]>,
    transition_type_preimage: Arc<[u8]>,
    context: Arc<ExploreValue>,
    before: Arc<ExploreValue>,
    after: Arc<ExploreValue>,
}

impl TransitionInstance {
    #[cfg(test)]
    pub(crate) fn new(
        state_schema_preimage: impl Into<Box<[u8]>>,
        transition_type_preimage: impl Into<Box<[u8]>>,
        context: ExploreValue,
        before: ExploreValue,
        after: ExploreValue,
    ) -> Self {
        let state_schema_preimage: Arc<[u8]> = Arc::from(state_schema_preimage.into());
        let context_schema_preimage: Arc<[u8]> = Arc::from([]);
        let transition_type_preimage: Arc<[u8]> = Arc::from(transition_type_preimage.into());
        Self::from_shared_schema(
            StateSchemaId::derive(&state_schema_preimage),
            ContextSchemaId::derive(&context_schema_preimage),
            TransitionTypeId::derive(&transition_type_preimage),
            state_schema_preimage,
            context_schema_preimage,
            transition_type_preimage,
            context,
            before,
            after,
        )
    }

    fn from_shared_schema(
        state_schema_id: StateSchemaId,
        context_schema_id: ContextSchemaId,
        transition_type_id: TransitionTypeId,
        state_schema_preimage: Arc<[u8]>,
        context_schema_preimage: Arc<[u8]>,
        transition_type_preimage: Arc<[u8]>,
        context: ExploreValue,
        before: ExploreValue,
        after: ExploreValue,
    ) -> Self {
        let context = Arc::new(context);
        let before = Arc::new(before);
        let after = Arc::new(after);
        let before_state_id = StateId::derive(state_schema_id, before.as_ref());
        let after_state_id = StateId::derive(state_schema_id, after.as_ref());
        let id = TransitionId::derive(
            transition_type_id,
            context.as_ref(),
            before_state_id,
            after_state_id,
        );
        Self {
            id,
            before_state_id,
            after_state_id,
            state_schema_id,
            context_schema_id,
            transition_type_id,
            state_schema_preimage,
            context_schema_preimage,
            transition_type_preimage,
            context,
            before,
            after,
        }
    }

    /// Reconstruct a durable transition only after rederiving every claimed
    /// semantic identity from its canonical preimage.
    pub(crate) fn from_canonical_v1(
        canonical: TransitionInstanceCanonicalV1,
    ) -> Result<Self, TransitionIdentityError> {
        let TransitionInstanceCanonicalV1 {
            state_schema_id,
            context_schema_id,
            transition_type_id,
            before_state_id,
            after_state_id,
            transition_id,
            state_schema_preimage,
            context_schema_preimage,
            transition_type_preimage,
            context,
            before,
            after,
        } = canonical;

        if StateSchemaId::derive(&state_schema_preimage) != state_schema_id {
            return Err(TransitionIdentityError::StateSchemaIdMismatch);
        }
        if ContextSchemaId::derive(&context_schema_preimage) != context_schema_id {
            return Err(TransitionIdentityError::ContextSchemaIdMismatch);
        }

        let mut expected_transition_type = CanonicalEncoder::new(TRANSITION_TYPE_ENCODING_V1);
        expected_transition_type.bytes(context_schema_id.as_ref());
        expected_transition_type.bytes(state_schema_id.as_ref());
        if expected_transition_type.finish().as_ref() != transition_type_preimage.as_ref() {
            return Err(TransitionIdentityError::TransitionTypePreimageMismatch);
        }
        if TransitionTypeId::derive(&transition_type_preimage) != transition_type_id {
            return Err(TransitionIdentityError::TransitionTypeIdMismatch);
        }

        let transition = Self::from_shared_schema(
            state_schema_id,
            context_schema_id,
            transition_type_id,
            state_schema_preimage,
            context_schema_preimage,
            transition_type_preimage,
            context,
            before,
            after,
        );
        if transition.before_state_id != before_state_id {
            return Err(TransitionIdentityError::BeforeStateIdMismatch);
        }
        if transition.after_state_id != after_state_id {
            return Err(TransitionIdentityError::AfterStateIdMismatch);
        }
        if transition.id != transition_id {
            return Err(TransitionIdentityError::TransitionIdMismatch);
        }
        Ok(transition)
    }

    pub(crate) fn canonical_v1(&self) -> TransitionInstanceCanonicalV1 {
        TransitionInstanceCanonicalV1 {
            state_schema_id: self.state_schema_id,
            context_schema_id: self.context_schema_id,
            transition_type_id: self.transition_type_id,
            before_state_id: self.before_state_id,
            after_state_id: self.after_state_id,
            transition_id: self.id,
            state_schema_preimage: self.state_schema_preimage.clone(),
            context_schema_preimage: self.context_schema_preimage.clone(),
            transition_type_preimage: self.transition_type_preimage.clone(),
            context: self.context.as_ref().clone(),
            before: self.before.as_ref().clone(),
            after: self.after.as_ref().clone(),
        }
    }

    pub(crate) const fn id(&self) -> TransitionId {
        self.id
    }

    pub(crate) const fn before_state_id(&self) -> StateId {
        self.before_state_id
    }

    pub(crate) const fn after_state_id(&self) -> StateId {
        self.after_state_id
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

    pub(crate) fn state_schema_preimage(&self) -> &[u8] {
        &self.state_schema_preimage
    }

    pub(crate) fn context_schema_preimage(&self) -> &[u8] {
        &self.context_schema_preimage
    }

    pub(crate) fn transition_type_preimage(&self) -> &[u8] {
        &self.transition_type_preimage
    }

    pub(crate) fn context(&self) -> &ExploreValue {
        self.context.as_ref()
    }

    pub(crate) fn before(&self) -> &ExploreValue {
        self.before.as_ref()
    }

    pub(crate) fn after(&self) -> &ExploreValue {
        self.after.as_ref()
    }
}

/// Compact collision witness for one interned directional edge.
///
/// Endpoint values live only in [`CanonicalState`]. Keeping their identities
/// here is sufficient because insertion checks each `StateId` against that
/// canonical state preimage before checking this edge witness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SupportedTransition {
    state_schema_id: StateSchemaId,
    context_schema_id: ContextSchemaId,
    transition_type_id: TransitionTypeId,
    before_state_id: StateId,
    after_state_id: StateId,
    context: Arc<ExploreValue>,
    admissible: bool,
    matching: bool,
}

impl SupportedTransition {
    fn from_transition(transition: &TransitionInstance, admissible: bool, matching: bool) -> Self {
        Self {
            state_schema_id: transition.state_schema_id,
            context_schema_id: transition.context_schema_id,
            transition_type_id: transition.transition_type_id,
            before_state_id: transition.before_state_id,
            after_state_id: transition.after_state_id,
            context: transition.context.clone(),
            admissible,
            matching,
        }
    }

    fn matches(&self, transition: &TransitionInstance) -> bool {
        self.state_schema_id == transition.state_schema_id
            && self.context_schema_id == transition.context_schema_id
            && self.transition_type_id == transition.transition_type_id
            && self.before_state_id == transition.before_state_id
            && self.after_state_id == transition.after_state_id
            && self.context.as_ref() == transition.context.as_ref()
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

    pub(crate) const fn before_state_id(&self) -> StateId {
        self.before_state_id
    }

    pub(crate) const fn after_state_id(&self) -> StateId {
        self.after_state_id
    }

    pub(crate) fn context(&self) -> &ExploreValue {
        self.context.as_ref()
    }

    pub(crate) const fn admissible(&self) -> bool {
        self.admissible
    }

    pub(crate) const fn matching(&self) -> bool {
        self.matching
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalState {
    schema_id: StateSchemaId,
    schema_preimage: Arc<[u8]>,
    value: Arc<ExploreValue>,
}

/// Collision-checking interner for the exact semantic graph projection.
///
/// This is the authoritative owner of canonical state preimages and
/// directional edge preimages within the projection. Equal directional edges
/// share one entry. Exact transition-to-case support remains in the
/// authenticated run stream, while global classification fibers remain in the
/// reducer; this interner is a compact collision/population projection, not a
/// second source of execution truth.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct TransitionSupportIndex {
    revision: u64,
    admissible_transition_count: u128,
    matching_transition_count: u128,
    by_state_schema: BTreeMap<StateSchemaId, Arc<[u8]>>,
    by_context_schema: BTreeMap<ContextSchemaId, Arc<[u8]>>,
    by_transition_type: BTreeMap<TransitionTypeId, Arc<[u8]>>,
    by_state: BTreeMap<StateId, CanonicalState>,
    by_transition: BTreeMap<TransitionId, SupportedTransition>,
}

/// One collision-checked singleton insertion with no remaining semantic
/// failure. This is a convenience wrapper over the atomic batch protocol.
pub(crate) struct PreparedTransitionSupportInsert {
    transition_id: TransitionId,
    batch: PreparedTransitionSupportBatch,
}

/// An atomic, canonically ordered support delta bound to one index revision.
///
/// Its fields are private so only [`TransitionSupportIndex::prepare_batch`] can
/// mint a token. Applying a token has no semantic failure path: a stale token
/// is a caller protocol violation detected before any mutation.
#[derive(Debug)]
pub(crate) struct PreparedTransitionSupportBatch {
    prior_revision: u64,
    next_revision: u64,
    delta: TransitionSupportIndex,
}

#[derive(Debug)]
struct TransitionSupportCandidate {
    transition_id: TransitionId,
    transition: TransitionInstance,
    admissible: bool,
    matching: bool,
}

impl TransitionSupportIndex {
    /// Monotone mutation revision for binding prepared projections to this
    /// reducer state. It is operational metadata, not semantic graph identity.
    pub(crate) const fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn intern(
        &mut self,
        transition: TransitionInstance,
        admissible: bool,
        matching: bool,
    ) -> Result<TransitionId, TransitionSupportError> {
        let prepared = self.prepare_intern(transition, admissible, matching)?;
        Ok(self.commit_prepared(prepared))
    }

    pub(crate) fn prepare_intern(
        &self,
        transition: TransitionInstance,
        admissible: bool,
        matching: bool,
    ) -> Result<PreparedTransitionSupportInsert, TransitionSupportError> {
        let transition_id = transition.id();
        let batch = self.prepare_batch([(transition, admissible, matching)])?;
        Ok(PreparedTransitionSupportInsert {
            transition_id,
            batch,
        })
    }

    pub(crate) fn commit_prepared(
        &mut self,
        prepared: PreparedTransitionSupportInsert,
    ) -> TransitionId {
        self.apply_prepared_batch(prepared.batch);
        prepared.transition_id
    }

    /// Prepare a whole reducer delta against a private staged overlay.
    ///
    /// Inputs are sorted by their complete canonical transition before
    /// validation, so both successful state and collision outcomes are
    /// independent of worker arrival order. Equal transitions are idempotent.
    /// Every other transition is checked against both committed state and
    /// earlier staged entries before a token is returned; an error therefore
    /// leaves this index untouched.
    pub(crate) fn prepare_batch(
        &self,
        inserts: impl IntoIterator<Item = (TransitionInstance, bool, bool)>,
    ) -> Result<PreparedTransitionSupportBatch, TransitionSupportError> {
        let mut inserts = inserts
            .into_iter()
            .map(
                |(transition, admissible, matching)| TransitionSupportCandidate {
                    transition_id: transition.id(),
                    transition,
                    admissible,
                    matching,
                },
            )
            .collect::<Vec<_>>();
        inserts.sort_by(|left, right| {
            (&left.transition, left.admissible, left.matching).cmp(&(
                &right.transition,
                right.admissible,
                right.matching,
            ))
        });

        let mut staged = Self::default();
        for insert in inserts {
            if insert.matching && !insert.admissible {
                return Err(TransitionSupportError::MatchingWithoutAdmissible {
                    transition_id: insert.transition_id,
                });
            }
            self.require_compatible_transition(&insert.transition)?;
            staged.require_compatible_transition(&insert.transition)?;

            let committed = self.by_transition.get(&insert.transition_id);
            let pending = staged.by_transition.get(&insert.transition_id);
            let prior_admissible = committed.is_some_and(SupportedTransition::admissible)
                || pending.is_some_and(SupportedTransition::admissible);
            let prior_matching = committed.is_some_and(SupportedTransition::matching)
                || pending.is_some_and(SupportedTransition::matching);
            let adds_population =
                (insert.admissible && !prior_admissible) || (insert.matching && !prior_matching);

            if committed.is_some() || pending.is_some() {
                if adds_population {
                    staged.observe_supported_transition_unchecked(
                        insert.transition_id,
                        &insert.transition,
                        insert.admissible,
                        insert.matching,
                    );
                }
                // Compatibility was checked first, so an observation with no
                // new population bit is the exact edge already interned.
                continue;
            }

            staged.commit_unchecked(
                insert.transition,
                insert.transition_id,
                insert.admissible,
                insert.matching,
            );
        }

        let next_revision = if staged.by_transition.is_empty() {
            self.revision
        } else {
            self.revision
                .checked_add(1)
                .ok_or(TransitionSupportError::RevisionExhausted)?
        };
        Ok(PreparedTransitionSupportBatch {
            prior_revision: self.revision,
            next_revision,
            delta: staged,
        })
    }

    /// Apply a successfully prepared batch. Staleness is a reducer protocol
    /// violation and is asserted before the first mutation.
    pub(crate) fn apply_prepared_batch(&mut self, prepared: PreparedTransitionSupportBatch) {
        assert_eq!(
            self.revision, prepared.prior_revision,
            "prepared transition-support batch is stale"
        );
        self.apply_delta_unchecked(prepared.delta);
        self.revision = prepared.next_revision;
    }

    fn apply_delta_unchecked(&mut self, delta: Self) {
        let Self {
            revision: _,
            admissible_transition_count: _,
            matching_transition_count: _,
            by_state_schema,
            by_context_schema,
            by_transition_type,
            by_state,
            by_transition,
        } = delta;
        for (id, preimage) in by_state_schema {
            self.by_state_schema.entry(id).or_insert(preimage);
        }
        for (id, preimage) in by_context_schema {
            self.by_context_schema.entry(id).or_insert(preimage);
        }
        for (id, preimage) in by_transition_type {
            self.by_transition_type.entry(id).or_insert(preimage);
        }
        for (id, state) in by_state {
            self.by_state.entry(id).or_insert(state);
        }
        for (id, incoming) in by_transition {
            self.merge_supported_transition_unchecked(id, incoming);
        }
    }

    fn require_compatible_transition(
        &self,
        transition: &TransitionInstance,
    ) -> Result<(), TransitionSupportError> {
        if self
            .by_state_schema
            .get(&transition.state_schema_id)
            .is_some_and(|existing| existing.as_ref() != transition.state_schema_preimage.as_ref())
        {
            return Err(TransitionSupportError::StateSchemaIdCollision {
                schema_id: transition.state_schema_id,
            });
        }
        if self
            .by_context_schema
            .get(&transition.context_schema_id)
            .is_some_and(|existing| {
                existing.as_ref() != transition.context_schema_preimage.as_ref()
            })
        {
            return Err(TransitionSupportError::ContextSchemaIdCollision {
                schema_id: transition.context_schema_id,
            });
        }
        if self
            .by_transition_type
            .get(&transition.transition_type_id)
            .is_some_and(|existing| {
                existing.as_ref() != transition.transition_type_preimage.as_ref()
            })
        {
            return Err(TransitionSupportError::TransitionTypeIdCollision {
                type_id: transition.transition_type_id,
            });
        }

        self.require_equal_state_preimage(
            transition.before_state_id,
            transition.state_schema_id,
            &transition.state_schema_preimage,
            transition.before.as_ref(),
        )?;
        self.require_equal_state_preimage(
            transition.after_state_id,
            transition.state_schema_id,
            &transition.state_schema_preimage,
            transition.after.as_ref(),
        )?;
        if transition.before_state_id == transition.after_state_id
            && transition.before != transition.after
        {
            return Err(TransitionSupportError::StateIdCollision {
                state_id: transition.before_state_id,
            });
        }

        if let Some(existing) = self.by_transition.get(&transition.id) {
            if !existing.matches(transition) {
                return Err(TransitionSupportError::TransitionIdCollision {
                    transition_id: transition.id,
                });
            }
        }
        Ok(())
    }

    fn commit_unchecked(
        &mut self,
        mut transition: TransitionInstance,
        transition_id: TransitionId,
        admissible: bool,
        matching: bool,
    ) {
        self.by_state_schema
            .entry(transition.state_schema_id)
            .or_insert_with(|| transition.state_schema_preimage.clone());
        self.by_context_schema
            .entry(transition.context_schema_id)
            .or_insert_with(|| transition.context_schema_preimage.clone());
        self.by_transition_type
            .entry(transition.transition_type_id)
            .or_insert_with(|| transition.transition_type_preimage.clone());

        transition.before = self.intern_state_unchecked(
            transition.before_state_id,
            transition.state_schema_id,
            &transition.state_schema_preimage,
            transition.before.clone(),
        );
        transition.after = self.intern_state_unchecked(
            transition.after_state_id,
            transition.state_schema_id,
            &transition.state_schema_preimage,
            transition.after.clone(),
        );

        self.observe_supported_transition_unchecked(
            transition_id,
            &transition,
            admissible,
            matching,
        );
    }

    fn observe_supported_transition_unchecked(
        &mut self,
        transition_id: TransitionId,
        transition: &TransitionInstance,
        admissible: bool,
        matching: bool,
    ) {
        let incoming = SupportedTransition::from_transition(transition, admissible, matching);
        self.merge_supported_transition_unchecked(transition_id, incoming);
    }

    fn merge_supported_transition_unchecked(
        &mut self,
        transition_id: TransitionId,
        incoming: SupportedTransition,
    ) {
        debug_assert!(!incoming.matching || incoming.admissible);
        let (new_admissible, new_matching) = match self.by_transition.entry(transition_id) {
            std::collections::btree_map::Entry::Occupied(mut existing) => {
                debug_assert_eq!(existing.get().state_schema_id, incoming.state_schema_id);
                debug_assert_eq!(existing.get().context_schema_id, incoming.context_schema_id);
                debug_assert_eq!(
                    existing.get().transition_type_id,
                    incoming.transition_type_id
                );
                debug_assert_eq!(existing.get().before_state_id, incoming.before_state_id);
                debug_assert_eq!(existing.get().after_state_id, incoming.after_state_id);
                debug_assert_eq!(existing.get().context, incoming.context);
                let new_admissible = incoming.admissible && !existing.get().admissible;
                let new_matching = incoming.matching && !existing.get().matching;
                existing.get_mut().admissible |= incoming.admissible;
                existing.get_mut().matching |= incoming.matching;
                (new_admissible, new_matching)
            }
            std::collections::btree_map::Entry::Vacant(vacant) => {
                let new_admissible = incoming.admissible;
                let new_matching = incoming.matching;
                vacant.insert(incoming);
                (new_admissible, new_matching)
            }
        };
        if new_admissible {
            self.admissible_transition_count = self
                .admissible_transition_count
                .checked_add(1)
                .expect("admissible transition count cannot exceed addressable edges");
        }
        if new_matching {
            self.matching_transition_count = self
                .matching_transition_count
                .checked_add(1)
                .expect("matching transition count cannot exceed addressable edges");
        }
    }

    fn intern_state_unchecked(
        &mut self,
        state_id: StateId,
        schema_id: StateSchemaId,
        schema_preimage: &Arc<[u8]>,
        value: Arc<ExploreValue>,
    ) -> Arc<ExploreValue> {
        match self.by_state.entry(state_id) {
            std::collections::btree_map::Entry::Occupied(existing) => {
                debug_assert_eq!(existing.get().schema_id, schema_id);
                debug_assert_eq!(
                    existing.get().schema_preimage.as_ref(),
                    schema_preimage.as_ref()
                );
                debug_assert_eq!(existing.get().value.as_ref(), value.as_ref());
                existing.get().value.clone()
            }
            std::collections::btree_map::Entry::Vacant(vacant) => {
                vacant.insert(CanonicalState {
                    schema_id,
                    schema_preimage: schema_preimage.clone(),
                    value: value.clone(),
                });
                value
            }
        }
    }

    fn require_equal_state_preimage(
        &self,
        state_id: StateId,
        schema_id: StateSchemaId,
        schema_preimage: &[u8],
        value: &ExploreValue,
    ) -> Result<(), TransitionSupportError> {
        if self.by_state.get(&state_id).is_some_and(|existing| {
            existing.schema_id != schema_id
                || existing.schema_preimage.as_ref() != schema_preimage
                || existing.value.as_ref() != value
        }) {
            return Err(TransitionSupportError::StateIdCollision { state_id });
        }
        Ok(())
    }

    /// Number of distinct directional semantic edges.
    pub(crate) fn len(&self) -> usize {
        self.by_transition.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.by_transition.is_empty()
    }

    pub(crate) fn state_len(&self) -> usize {
        self.by_state.len()
    }

    pub(crate) const fn admissible_transition_count(&self) -> u128 {
        self.admissible_transition_count
    }

    pub(crate) const fn matching_transition_count(&self) -> u128 {
        self.matching_transition_count
    }

    /// State schemas in ascending canonical identity order.
    pub(crate) fn iter_state_schemas(&self) -> impl Iterator<Item = (StateSchemaId, &[u8])> {
        self.by_state_schema
            .iter()
            .map(|(id, preimage)| (*id, preimage.as_ref()))
    }

    /// Context schemas in ascending canonical identity order.
    pub(crate) fn iter_context_schemas(&self) -> impl Iterator<Item = (ContextSchemaId, &[u8])> {
        self.by_context_schema
            .iter()
            .map(|(id, preimage)| (*id, preimage.as_ref()))
    }

    /// Transition relation types in ascending canonical identity order.
    pub(crate) fn iter_transition_types(&self) -> impl Iterator<Item = (TransitionTypeId, &[u8])> {
        self.by_transition_type
            .iter()
            .map(|(id, preimage)| (*id, preimage.as_ref()))
    }

    /// Exact schema identity, schema preimage and value for one interned state node.
    pub(crate) fn state(&self, id: StateId) -> Option<(StateSchemaId, &[u8], &ExploreValue)> {
        self.by_state.get(&id).map(|state| {
            (
                state.schema_id,
                state.schema_preimage.as_ref(),
                state.value.as_ref(),
            )
        })
    }

    /// State nodes in ascending [`StateId`] order.
    pub(crate) fn iter_states(
        &self,
    ) -> impl Iterator<Item = (StateId, StateSchemaId, &[u8], &ExploreValue)> {
        self.by_state.iter().map(|(id, state)| {
            (
                *id,
                state.schema_id,
                state.schema_preimage.as_ref(),
                state.value.as_ref(),
            )
        })
    }

    /// Check a claimed transition against the complete canonical collision
    /// witnesses already interned for `id`.
    pub(crate) fn transition_matches(
        &self,
        id: TransitionId,
        transition: &TransitionInstance,
    ) -> bool {
        id == transition.id
            && self.by_transition.contains_key(&id)
            && self.require_compatible_transition(transition).is_ok()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (TransitionId, &SupportedTransition)> {
        self.by_transition.iter().map(|(id, edge)| (*id, edge))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TransitionSupportError {
    StateSchemaIdCollision { schema_id: StateSchemaId },
    ContextSchemaIdCollision { schema_id: ContextSchemaId },
    TransitionTypeIdCollision { type_id: TransitionTypeId },
    StateIdCollision { state_id: StateId },
    TransitionIdCollision { transition_id: TransitionId },
    MatchingWithoutAdmissible { transition_id: TransitionId },
    RevisionExhausted,
}

impl fmt::Display for TransitionSupportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StateSchemaIdCollision { .. } => formatter.write_str(
                "Explore State schema SHA-256 collision rejected by canonical support interner",
            ),
            Self::ContextSchemaIdCollision { .. } => formatter.write_str(
                "Explore Context schema SHA-256 collision rejected by canonical support interner",
            ),
            Self::TransitionTypeIdCollision { .. } => formatter.write_str(
                "Explore transition-type SHA-256 collision rejected by canonical support interner",
            ),
            Self::StateIdCollision { .. } => formatter.write_str(
                "Explore state SHA-256 collision rejected by canonical support interner",
            ),
            Self::TransitionIdCollision { .. } => formatter.write_str(
                "Explore transition SHA-256 collision rejected by canonical support interner",
            ),
            Self::MatchingWithoutAdmissible { .. } => formatter
                .write_str("an Explore matching transition observation must also be admissible"),
            Self::RevisionExhausted => {
                formatter.write_str("Explore transition-support revision counter exhausted")
            }
        }
    }
}

impl Error for TransitionSupportError {}

fn encode_relational_schema(
    role: &str,
    role_tag: u8,
    ty: &Ty,
    resolved_type_owners: &BTreeMap<Box<str>, CheckedDataTypeId>,
) -> Result<Arc<[u8]>, String> {
    if !relational_schema_ty_is_closed(ty) {
        return Err(format!(
            "relational Explore {role} schema contains an unresolved checked type"
        ));
    }
    let mut encoder = CanonicalEncoder::new(RELATIONAL_SCHEMA_ENCODING_V1);
    encoder.tag(role_tag);
    encode_ty(&mut encoder, ty, resolved_type_owners)
        .map_err(|message| format!("relational Explore {role} schema identity: {message}"))?;
    Ok(encoder.finish())
}

fn relational_schema_ty_is_closed(ty: &Ty) -> bool {
    match ty {
        Ty::Name(_) | Ty::Unit => true,
        Ty::App(constructor, arguments) => {
            relational_schema_ty_is_closed(constructor)
                && arguments.iter().all(relational_schema_ty_is_closed)
        }
        Ty::Arrow(argument, result) => {
            relational_schema_ty_is_closed(argument) && relational_schema_ty_is_closed(result)
        }
        Ty::Ref(inner) | Ty::MutRef(inner) | Ty::Shared(inner) | Ty::Optional(inner) => {
            relational_schema_ty_is_closed(inner)
        }
        Ty::Var(_) | Ty::Hole => false,
    }
}

fn encode_checked_data_type_id(encoder: &mut CanonicalEncoder, owner: &CheckedDataTypeId) {
    match owner {
        CheckedDataTypeId::Declared(occurrence) => {
            encoder.tag(DECLARED_OWNER_OCCURRENCE);
            // DeclarationId identifies the declaration inside its source
            // module and qualified semantic namespace. The equal-declaration
            // occurrence rank preserves genuinely repeated retained owners,
            // while the checked program's global position remains only an
            // address for diagnostics/resolution.
            encoder.bytes(occurrence.declaration.semantic_key().as_bytes());
            encoder.count(occurrence.declaration_occurrence_ordinal);
        }
        CheckedDataTypeId::Intrinsic { canonical_name } => {
            encoder.tag(DECLARED_OWNER_INTRINSIC);
            encoder.bytes(canonical_name.as_bytes());
        }
    }
}

/// Canonical type traversal for identities minted from the checked closed IR.
///
/// Checked transition products should contain no open variables or holes, but
/// those variants remain explicitly tagged so this traversal stays total and
/// cannot silently alias a malformed IR node with another type.
fn encode_ty(
    encoder: &mut CanonicalEncoder,
    ty: &Ty,
    resolved_type_owners: &BTreeMap<Box<str>, CheckedDataTypeId>,
) -> Result<(), String> {
    match ty {
        Ty::Name(name) => {
            encoder.tag(TYPE_NAME);
            let owner = resolved_type_owners.get(name.as_str()).ok_or_else(|| {
                format!("nominal type `{name}` has no checked declaration/intrinsic owner")
            })?;
            encode_checked_data_type_id(encoder, owner);
        }
        Ty::App(constructor, arguments) => {
            encoder.tag(TYPE_APPLICATION);
            encode_ty(encoder, constructor, resolved_type_owners)?;
            encoder.count(arguments.len());
            for argument in arguments {
                encode_ty(encoder, argument, resolved_type_owners)?;
            }
        }
        Ty::Arrow(argument, result) => {
            encoder.tag(TYPE_ARROW);
            encode_ty(encoder, argument, resolved_type_owners)?;
            encode_ty(encoder, result, resolved_type_owners)?;
        }
        Ty::Ref(inner) => {
            encoder.tag(TYPE_REFERENCE);
            encode_ty(encoder, inner, resolved_type_owners)?;
        }
        Ty::MutRef(inner) => {
            encoder.tag(TYPE_MUTABLE_REFERENCE);
            encode_ty(encoder, inner, resolved_type_owners)?;
        }
        Ty::Shared(inner) => {
            encoder.tag(TYPE_SHARED);
            encode_ty(encoder, inner, resolved_type_owners)?;
        }
        Ty::Optional(inner) => {
            // `T?` is syntax sugar for `Option(T)`. Schema identity is over
            // the checked type, so both spellings must have one canonical
            // application encoding and the same intrinsic/declared owner.
            encoder.tag(TYPE_APPLICATION);
            encoder.tag(TYPE_NAME);
            let option_owner = resolved_type_owners.get("Option").ok_or_else(|| {
                "nominal type `Option` has no checked declaration/intrinsic owner".to_string()
            })?;
            encode_checked_data_type_id(encoder, option_owner);
            encoder.count(1);
            encode_ty(encoder, inner, resolved_type_owners)?;
        }
        Ty::Var(name) => {
            encoder.tag(TYPE_VARIABLE);
            encoder.bytes(name.as_bytes());
        }
        Ty::Unit => encoder.tag(TYPE_UNIT),
        Ty::Hole => encoder.tag(TYPE_HOLE),
    }
    Ok(())
}

/// Versioned, unambiguous binary traversal of every canonical Explore value.
///
/// Variant tags, collection lengths, constructor field names, and exact scalar
/// bits are all structural input. This must never be replaced with a debug or
/// presentation rendering.
fn hash_explore_value(hasher: &mut CanonicalHasher, value: &ExploreValue) {
    match value {
        ExploreValue::Int(value) => {
            hasher.tag(VALUE_INT);
            hasher.i64(*value);
        }
        ExploreValue::FloatBits(bits) => {
            hasher.tag(VALUE_FLOAT_BITS);
            hasher.u64(*bits);
        }
        ExploreValue::String(value) => {
            hasher.tag(VALUE_STRING);
            hasher.bytes(value.as_bytes());
        }
        ExploreValue::Character(value) => {
            hasher.tag(VALUE_CHARACTER);
            hasher.u32(u32::from(*value));
        }
        ExploreValue::Boolean(value) => {
            hasher.tag(VALUE_BOOLEAN);
            hasher.tag(u8::from(*value));
        }
        ExploreValue::Unit => hasher.tag(VALUE_UNIT),
        ExploreValue::List(values) => {
            hasher.tag(VALUE_LIST);
            hasher.count(values.len());
            for value in values {
                hash_explore_value(hasher, value);
            }
        }
        ExploreValue::Set(values) => {
            hasher.tag(VALUE_SET);
            hasher.count(values.len());
            for value in values {
                hash_explore_value(hasher, value);
            }
        }
        ExploreValue::Tuple(values) => {
            hasher.tag(VALUE_TUPLE);
            hasher.count(values.len());
            for value in values {
                hash_explore_value(hasher, value);
            }
        }
        ExploreValue::Constructor {
            type_name,
            variant,
            positional,
            fields,
        } => {
            hasher.tag(VALUE_CONSTRUCTOR);
            hasher.bytes(type_name.as_bytes());
            hasher.bytes(variant.as_bytes());
            hasher.tag(u8::from(*positional));
            hasher.count(fields.len());
            for (name, value) in fields.iter() {
                hasher.bytes(name.as_bytes());
                hash_explore_value(hasher, value);
            }
        }
    }
}

/// Content identity for one canonical Explore value.
///
/// Relational source and successor identities use this narrow entry point so
/// they share the transition graph's versioned value traversal without
/// exposing its hasher or duplicating the wire contract.
pub(super) fn canonical_explore_value_digest(value: &ExploreValue) -> [u8; 32] {
    let mut hasher = CanonicalHasher::new(EXPLORE_VALUE_ID_HASH_V1);
    hash_explore_value(&mut hasher, value);
    hasher.finish()
}

/// Canonical schema preimage retained behind every derived schema identity.
/// Keeping the bytes, rather than only their digest, lets the support index
/// reject a schema-hash collision when it checks state and edge preimages.
struct CanonicalEncoder(Vec<u8>);

impl CanonicalEncoder {
    fn new(domain: &[u8]) -> Self {
        let mut encoder = Self(Vec::new());
        encoder.bytes(domain);
        encoder
    }

    fn tag(&mut self, tag: u8) {
        self.0.push(tag);
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.0
            .extend_from_slice(&(bytes.len() as u128).to_be_bytes());
        self.0.extend_from_slice(bytes);
    }

    fn count(&mut self, count: usize) {
        self.0.extend_from_slice(&(count as u128).to_be_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.0.extend_from_slice(&value.to_be_bytes());
    }

    fn finish(self) -> Arc<[u8]> {
        Arc::from(self.0)
    }
}

struct CanonicalHasher(Sha256);

impl CanonicalHasher {
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

    fn count(&mut self, count: usize) {
        self.0.update((count as u128).to_be_bytes());
    }

    fn i64(&mut self, value: i64) {
        self.0.update(value.to_be_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.0.update(value.to_be_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.0.update(value.to_be_bytes());
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATE_SCHEMA: &[u8] = b"test.income-state.schema.v1";
    const TRANSITION_SCHEMA: &[u8] = b"test.income-transition.schema.v1";

    fn intrinsic_type_owners() -> BTreeMap<Box<str>, CheckedDataTypeId> {
        ["Int", "Option", "Nat", "Tuple"]
            .into_iter()
            .map(|name| {
                (
                    name.into(),
                    CheckedDataTypeId::Intrinsic {
                        canonical_name: name.into(),
                    },
                )
            })
            .collect()
    }

    fn rich_state(seed: i64) -> ExploreValue {
        ExploreValue::Constructor {
            type_name: "IncomeState".to_string(),
            variant: "IncomeState".to_string(),
            positional: false,
            fields: vec![
                ("income".to_string(), ExploreValue::Int(seed)),
                (
                    "rate_bits".to_string(),
                    ExploreValue::FloatBits((seed as f64 / 10.0).to_bits()),
                ),
                (
                    "commune".to_string(),
                    ExploreValue::String("Copenhagen".to_string()),
                ),
                ("band".to_string(), ExploreValue::Character('A')),
                ("resident".to_string(), ExploreValue::Boolean(true)),
                ("marker".to_string(), ExploreValue::Unit),
                (
                    "history".to_string(),
                    ExploreValue::List(vec![ExploreValue::Int(seed - 1)]),
                ),
                (
                    "flags".to_string(),
                    ExploreValue::Set(vec![ExploreValue::String("exact".to_string())]),
                ),
                (
                    "coordinates".to_string(),
                    ExploreValue::Tuple(vec![
                        ExploreValue::Int(seed),
                        ExploreValue::Boolean(false),
                    ]),
                ),
            ]
            .into(),
        }
    }

    fn transition(
        context: ExploreValue,
        before: ExploreValue,
        after: ExploreValue,
    ) -> TransitionInstance {
        TransitionInstance::new(STATE_SCHEMA, TRANSITION_SCHEMA, context, before, after)
    }

    #[cfg(any())]
    fn minimal_transition_ir(
        after_source: ExploreAfterFieldSourceIr,
        context_schema_version: u32,
    ) -> ExploreTransitionIr {
        ExploreTransitionIr {
            normalization_version: 1,
            state_schema: ExploreProductSchemaIr {
                identity: TypedExploreProductSchemaIdentity::Synthetic { version: 1 },
                fields: vec![ExploreProductFieldIr {
                    field_index: 0,
                    name: "amount".to_string(),
                    value_ty: Ty::Name("Int".to_string()),
                    source: ExploreProductFieldSourceIr::Dimension { dimension_index: 0 },
                    span: Span::dummy(),
                }],
            },
            context_schema: ExploreProductSchemaIr {
                identity: TypedExploreProductSchemaIdentity::Synthetic {
                    version: context_schema_version,
                },
                fields: Vec::new(),
            },
            after_fields: vec![ExploreAfterFieldIr {
                field_index: 0,
                name: "amount".to_string(),
                value_ty: Ty::Name("Int".to_string()),
                source: after_source,
                span: Span::dummy(),
            }],
            after_membership: Vec::new(),
            flat_aliases: Vec::new(),
            boundary_hint: None,
        }
    }

    fn minimal_semantic_edge(schemas: &TransitionSchemaIdentities) -> TransitionInstance {
        schemas.instantiate(
            ExploreValue::Tuple(Vec::new()),
            ExploreValue::Tuple(vec![ExploreValue::Int(199_000)]),
            ExploreValue::Tuple(vec![ExploreValue::Int(200_000)]),
        )
    }

    #[cfg(any())]
    #[test]
    fn transition_schema_identity_excludes_after_construction_topology() {
        let baseline = minimal_transition_ir(
            ExploreAfterFieldSourceIr::FrameBefore {
                before_field_index: 0,
            },
            1,
        );
        let different_topology = minimal_transition_ir(
            ExploreAfterFieldSourceIr::IndependentDomain { dimension_index: 1 },
            1,
        );

        let owners = intrinsic_type_owners();
        let baseline =
            TransitionSchemaIdentities::derive_checked(&baseline, None, None, &owners).unwrap();
        let different_topology =
            TransitionSchemaIdentities::derive_checked(&different_topology, None, None, &owners)
                .unwrap();

        assert_eq!(
            baseline.state_schema_id(),
            different_topology.state_schema_id()
        );
        assert_eq!(
            baseline.context_schema_id(),
            different_topology.context_schema_id()
        );
        assert_eq!(
            baseline.transition_type_id(),
            different_topology.transition_type_id()
        );
        assert_eq!(
            minimal_semantic_edge(&baseline).id(),
            minimal_semantic_edge(&different_topology).id()
        );
    }

    #[cfg(any())]
    #[test]
    fn relation_schema_identity_encodes_context_before_state() {
        let transition = minimal_transition_ir(
            ExploreAfterFieldSourceIr::FrameBefore {
                before_field_index: 0,
            },
            1,
        );
        let schemas = TransitionSchemaIdentities::derive_checked(
            &transition,
            None,
            None,
            &intrinsic_type_owners(),
        )
        .unwrap();

        let mut expected = CanonicalEncoder::new(TRANSITION_TYPE_ENCODING_V1);
        expected.bytes(schemas.context_schema_id().as_ref());
        expected.bytes(schemas.state_schema_id().as_ref());
        let expected = expected.finish();
        assert_eq!(schemas.transition_type_preimage(), expected.as_ref());
        assert_eq!(
            schemas.transition_type_id(),
            TransitionTypeId::derive(&expected)
        );

        let mut reversed = CanonicalEncoder::new(TRANSITION_TYPE_ENCODING_V1);
        reversed.bytes(schemas.state_schema_id().as_ref());
        reversed.bytes(schemas.context_schema_id().as_ref());
        assert_ne!(
            schemas.transition_type_preimage(),
            reversed.finish().as_ref()
        );
    }

    #[cfg(any())]
    #[test]
    fn context_schema_identity_changes_relation_and_transition_identity() {
        let context_v1 = minimal_transition_ir(
            ExploreAfterFieldSourceIr::FrameBefore {
                before_field_index: 0,
            },
            1,
        );
        let context_v2 = minimal_transition_ir(
            ExploreAfterFieldSourceIr::FrameBefore {
                before_field_index: 0,
            },
            2,
        );

        let owners = intrinsic_type_owners();
        let context_v1 =
            TransitionSchemaIdentities::derive_checked(&context_v1, None, None, &owners).unwrap();
        let context_v2 =
            TransitionSchemaIdentities::derive_checked(&context_v2, None, None, &owners).unwrap();

        assert_eq!(context_v1.state_schema_id(), context_v2.state_schema_id());
        assert_ne!(
            context_v1.context_schema_id(),
            context_v2.context_schema_id()
        );
        assert_ne!(
            context_v1.transition_type_id(),
            context_v2.transition_type_id()
        );
        let edge_v1 = minimal_semantic_edge(&context_v1);
        let edge_v2 = minimal_semantic_edge(&context_v2);
        assert_eq!(edge_v1.before_state_id(), edge_v2.before_state_id());
        assert_eq!(edge_v1.after_state_id(), edge_v2.after_state_id());
        assert_ne!(edge_v1.id(), edge_v2.id());
    }

    #[cfg(any())]
    #[test]
    fn declared_product_schema_identity_includes_checked_owner() {
        let mut transition = minimal_transition_ir(
            ExploreAfterFieldSourceIr::FrameBefore {
                before_field_index: 0,
            },
            1,
        );
        transition.state_schema.identity = TypedExploreProductSchemaIdentity::Declared {
            ty: Ty::Name("IncomeState".to_string()),
        };
        let otherwise_identical = transition.clone();
        let first_owner = crate::CheckedDataTypeId::Intrinsic {
            canonical_name: "test.owner.first".into(),
        };
        let second_owner = crate::CheckedDataTypeId::Intrinsic {
            canonical_name: "test.owner.second".into(),
        };

        let mut first_owners = intrinsic_type_owners();
        first_owners.insert("IncomeState".into(), first_owner.clone());
        let mut second_owners = intrinsic_type_owners();
        second_owners.insert("IncomeState".into(), second_owner.clone());

        let first = TransitionSchemaIdentities::derive_checked(
            &transition,
            Some(&first_owner),
            None,
            &first_owners,
        )
        .unwrap();
        let second = TransitionSchemaIdentities::derive_checked(
            &otherwise_identical,
            Some(&second_owner),
            None,
            &second_owners,
        )
        .unwrap();

        assert_ne!(first.state_schema_id(), second.state_schema_id());
        assert_eq!(first.context_schema_id(), second.context_schema_id());
        assert_ne!(first.transition_type_id(), second.transition_type_id());
        let first_edge = minimal_semantic_edge(&first);
        let second_edge = minimal_semantic_edge(&second);
        assert_ne!(first_edge.before_state_id(), second_edge.before_state_id());
        assert_ne!(first_edge.after_state_id(), second_edge.after_state_id());
        assert_ne!(first_edge.id(), second_edge.id());
    }

    #[cfg(any())]
    #[test]
    fn declared_product_schema_identity_includes_nested_checked_owners() {
        let mut transition = minimal_transition_ir(
            ExploreAfterFieldSourceIr::FrameBefore {
                before_field_index: 0,
            },
            1,
        );
        transition.state_schema.identity = TypedExploreProductSchemaIdentity::Declared {
            ty: Ty::Name("IncomeState".to_string()),
        };
        transition.state_schema.fields[0].value_ty = Ty::Name("Money".to_string());
        transition.after_fields[0].value_ty = Ty::Name("Money".to_string());

        let state_owner = CheckedDataTypeId::Intrinsic {
            canonical_name: "test.owner.income-state".into(),
        };
        let first_money_owner = CheckedDataTypeId::Intrinsic {
            canonical_name: "test.owner.money.first".into(),
        };
        let second_money_owner = CheckedDataTypeId::Intrinsic {
            canonical_name: "test.owner.money.second".into(),
        };
        let first_owners: BTreeMap<Box<str>, CheckedDataTypeId> = BTreeMap::from([
            ("IncomeState".into(), state_owner.clone()),
            ("Money".into(), first_money_owner),
        ]);
        let second_owners: BTreeMap<Box<str>, CheckedDataTypeId> = BTreeMap::from([
            ("IncomeState".into(), state_owner.clone()),
            ("Money".into(), second_money_owner),
        ]);

        let first = TransitionSchemaIdentities::derive_checked(
            &transition,
            Some(&state_owner),
            None,
            &first_owners,
        )
        .unwrap();
        let second = TransitionSchemaIdentities::derive_checked(
            &transition,
            Some(&state_owner),
            None,
            &second_owners,
        )
        .unwrap();

        assert_ne!(first.state_schema_id(), second.state_schema_id());
        assert_ne!(first.transition_type_id(), second.transition_type_id());
    }

    #[cfg(any())]
    #[test]
    fn optional_sugar_and_option_application_share_schema_identity() {
        let mut sugar = minimal_transition_ir(
            ExploreAfterFieldSourceIr::FrameBefore {
                before_field_index: 0,
            },
            1,
        );
        sugar.state_schema.fields[0].value_ty = Ty::Optional(Box::new(Ty::Name("Int".to_string())));
        let mut application = sugar.clone();
        application.state_schema.fields[0].value_ty = Ty::App(
            Box::new(Ty::Name("Option".to_string())),
            vec![Ty::Name("Int".to_string())],
        );
        let owners = intrinsic_type_owners();

        let sugar =
            TransitionSchemaIdentities::derive_checked(&sugar, None, None, &owners).unwrap();
        let application =
            TransitionSchemaIdentities::derive_checked(&application, None, None, &owners).unwrap();

        assert_eq!(sugar.state_schema_id(), application.state_schema_id());
        assert_eq!(sugar.transition_type_id(), application.transition_type_id());
    }

    #[test]
    fn declared_owner_identity_ignores_global_retained_occurrence_position() {
        let declaration = crate::DeclarationId {
            module: crate::ModuleId {
                content_hash: "stable-module".into(),
                internal_path: Box::default(),
            },
            kind: crate::DeclarationKind::Adt,
            owner: None,
            name: "IncomeState".into(),
            arity: None,
            ordinal: 3,
        };
        let owner_at_first_position =
            CheckedDataTypeId::Declared(crate::CheckedDeclarationOccurrenceId {
                declaration: declaration.clone(),
                declaration_occurrence_ordinal: 2,
                normalized_ordinal: 4,
            });
        let owner_after_unrelated_insert =
            CheckedDataTypeId::Declared(crate::CheckedDeclarationOccurrenceId {
                declaration: declaration.clone(),
                declaration_occurrence_ordinal: 2,
                normalized_ordinal: 9,
            });
        let mut first = CanonicalEncoder::new(b"owner-test.v1");
        encode_checked_data_type_id(&mut first, &owner_at_first_position);
        let mut second = CanonicalEncoder::new(b"owner-test.v1");
        encode_checked_data_type_id(&mut second, &owner_after_unrelated_insert);

        let first = first.finish();
        assert_eq!(first, second.finish());

        let repeated_owner = CheckedDataTypeId::Declared(crate::CheckedDeclarationOccurrenceId {
            declaration,
            declaration_occurrence_ordinal: 3,
            normalized_ordinal: 10,
        });
        let mut repeated = CanonicalEncoder::new(b"owner-test.v1");
        encode_checked_data_type_id(&mut repeated, &repeated_owner);
        assert_ne!(first, repeated.finish());
    }

    #[test]
    fn state_identity_is_reused_across_endpoint_roles() {
        let first = transition(ExploreValue::Unit, rich_state(199_000), rich_state(200_000));
        let second = transition(ExploreValue::Unit, rich_state(200_000), rich_state(201_000));
        let shared_state_id = first.after_state_id();
        let first_shared_allocation = Arc::downgrade(&first.after);
        let duplicate_shared_allocation = Arc::downgrade(&second.before);
        let mut index = TransitionSupportIndex::default();

        assert_eq!(shared_state_id, second.before_state_id());
        assert_eq!(first.after(), second.before());
        assert!(!Arc::ptr_eq(&first.after, &second.before));

        index.intern(first, false, false).unwrap();
        index.intern(second, false, false).unwrap();

        assert_eq!(index.state_len(), 3);
        assert!(Arc::ptr_eq(
            &index.by_state[&shared_state_id].value,
            &first_shared_allocation.upgrade().unwrap()
        ));
        assert!(duplicate_shared_allocation.upgrade().is_none());
        let expected_shared_state = rich_state(200_000);
        assert_eq!(
            index.state(shared_state_id),
            Some((
                StateSchemaId::derive(STATE_SCHEMA),
                STATE_SCHEMA,
                &expected_shared_state
            ))
        );
        let state_ids = index
            .iter_states()
            .map(|(state_id, _, _, _)| state_id)
            .collect::<Vec<_>>();
        assert!(state_ids.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn state_preimage_collisions_at_either_endpoint_are_atomic() {
        for collide_before in [true, false] {
            let existing = transition(ExploreValue::Unit, rich_state(199_000), rich_state(200_000));
            let colliding_state_id = existing.before_state_id();
            let mut attempted =
                transition(ExploreValue::Unit, rich_state(300_000), rich_state(301_000));
            if collide_before {
                attempted.before_state_id = colliding_state_id;
            } else {
                attempted.after_state_id = colliding_state_id;
            }
            let mut index = TransitionSupportIndex::default();
            index.intern(existing, false, false).unwrap();

            assert_eq!(
                index.intern(attempted, false, false),
                Err(TransitionSupportError::StateIdCollision {
                    state_id: colliding_state_id,
                })
            );
            assert_eq!(index.state_len(), 2);
            assert_eq!(index.len(), 1);
        }
    }

    #[test]
    fn schema_preimage_collisions_are_rejected_before_value_identity() {
        let existing = transition(
            ExploreValue::Int(1_000),
            rich_state(199_000),
            rich_state(200_000),
        );
        let mut attempted = transition(
            ExploreValue::Int(2_000),
            rich_state(300_000),
            rich_state(301_000),
        );
        attempted.context_schema_preimage = Arc::from(b"different-context-schema".as_slice());
        let colliding_schema_id = attempted.context_schema_id;
        let mut index = TransitionSupportIndex::default();
        index.intern(existing, false, false).unwrap();

        assert_eq!(
            index.intern(attempted, false, false),
            Err(TransitionSupportError::ContextSchemaIdCollision {
                schema_id: colliding_schema_id,
            })
        );
        assert_eq!(index.state_len(), 2);
        assert_eq!(index.len(), 1);
    }

    #[test]
    fn compact_edge_witness_rejects_transition_id_collision_atomically() {
        let existing = transition(
            ExploreValue::Int(1_000),
            rich_state(199_000),
            rich_state(200_000),
        );
        let collision = existing.id();
        let mut attempted = transition(
            ExploreValue::Int(2_000),
            rich_state(300_000),
            rich_state(301_000),
        );
        attempted.id = collision;
        let mut index = TransitionSupportIndex::default();
        index.intern(existing, false, false).unwrap();

        assert_eq!(
            index.intern(attempted, false, false),
            Err(TransitionSupportError::TransitionIdCollision {
                transition_id: collision,
            })
        );
        assert_eq!(index.state_len(), 2);
        assert_eq!(index.len(), 1);
    }

    #[test]
    fn reversing_endpoints_changes_transition_identity() {
        let before = rich_state(199_000);
        let after = rich_state(200_000);
        let forward = transition(ExploreValue::Unit, before.clone(), after.clone());
        let reverse = transition(ExploreValue::Unit, after, before);

        assert_ne!(forward.id(), reverse.id());
    }

    #[test]
    fn changing_context_changes_transition_identity() {
        let before = rich_state(199_000);
        let after = rich_state(200_000);
        let one_thousand = transition(ExploreValue::Int(1_000), before.clone(), after.clone());
        let two_thousand = transition(ExploreValue::Int(2_000), before, after);

        assert_eq!(
            one_thousand.before_state_id(),
            two_thousand.before_state_id()
        );
        assert_eq!(one_thousand.after_state_id(), two_thousand.after_state_id());
        assert_ne!(one_thousand.id(), two_thousand.id());
    }

    #[test]
    fn equal_edges_are_set_idempotent_without_retaining_case_coordinates() {
        let edge = transition(
            ExploreValue::Int(1_000),
            rich_state(199_000),
            rich_state(200_000),
        );
        let equal_edge = edge.clone();
        let mut index = TransitionSupportIndex::default();

        let first_id = index.intern(edge.clone(), false, false).unwrap();
        let revision_after_first = index.revision();
        let second_id = index.intern(equal_edge, false, false).unwrap();

        assert_eq!(first_id, second_id);
        assert_eq!(index.len(), 1);
        assert_eq!(index.revision(), revision_after_first);
        assert!(index.transition_matches(first_id, &edge));
    }

    #[test]
    fn transition_population_bits_are_monotone_and_counted_once() {
        let edge = transition(
            ExploreValue::Int(1_000),
            rich_state(199_000),
            rich_state(200_000),
        );
        let edge_id = edge.id();
        let mut index = TransitionSupportIndex::default();

        index.intern(edge.clone(), false, false).unwrap();
        assert_eq!(index.admissible_transition_count(), 0);
        assert_eq!(index.matching_transition_count(), 0);

        index.intern(edge.clone(), true, false).unwrap();
        assert_eq!(index.admissible_transition_count(), 1);
        assert_eq!(index.matching_transition_count(), 0);

        index.intern(edge.clone(), true, true).unwrap();
        let fully_observed_revision = index.revision();
        index.intern(edge.clone(), true, true).unwrap();
        index.intern(edge.clone(), false, false).unwrap();

        assert_eq!(index.revision(), fully_observed_revision);
        assert_eq!(index.admissible_transition_count(), 1);
        assert_eq!(index.matching_transition_count(), 1);
        let (_, supported) = index.iter().next().unwrap();
        assert_eq!(supported.transition_type_id(), edge.transition_type_id());
        assert!(supported.admissible());
        assert!(supported.matching());
        assert!(index.transition_matches(edge_id, &edge));
    }

    #[test]
    fn matching_without_admissibility_is_rejected_atomically() {
        let edge = transition(ExploreValue::Unit, rich_state(199_000), rich_state(200_000));
        let transition_id = edge.id();
        let index = TransitionSupportIndex::default();

        assert_eq!(
            index.prepare_batch([(edge, false, true)]).unwrap_err(),
            TransitionSupportError::MatchingWithoutAdmissible { transition_id }
        );
        assert_eq!(index, TransitionSupportIndex::default());
    }

    #[test]
    fn canonical_transition_rehydration_rederives_every_claimed_id() {
        let schemas = TransitionSchemaIdentities::derive_checked_relational(
            &Ty::Name("Int".to_string()),
            &Ty::Unit,
            &intrinsic_type_owners(),
        )
        .unwrap();
        let transition = minimal_semantic_edge(&schemas);
        let canonical = transition.canonical_v1();

        assert_eq!(
            TransitionInstance::from_canonical_v1(canonical.clone()),
            Ok(transition.clone())
        );
        assert_eq!(
            schemas.rehydrate_canonical_v1(canonical.clone()),
            Ok(transition)
        );

        let mut damaged = canonical.clone();
        damaged.state_schema_id = StateSchemaId::from_bytes([0; 32]);
        assert_eq!(
            TransitionInstance::from_canonical_v1(damaged),
            Err(TransitionIdentityError::StateSchemaIdMismatch)
        );

        let mut damaged = canonical.clone();
        damaged.context_schema_id = ContextSchemaId::from_bytes([0; 32]);
        assert_eq!(
            TransitionInstance::from_canonical_v1(damaged),
            Err(TransitionIdentityError::ContextSchemaIdMismatch)
        );

        let mut damaged = canonical.clone();
        damaged.transition_type_preimage = Arc::from(b"wrong-relation".as_slice());
        assert_eq!(
            TransitionInstance::from_canonical_v1(damaged),
            Err(TransitionIdentityError::TransitionTypePreimageMismatch)
        );

        let mut damaged = canonical.clone();
        damaged.transition_type_id = TransitionTypeId::from_bytes([0; 32]);
        assert_eq!(
            TransitionInstance::from_canonical_v1(damaged),
            Err(TransitionIdentityError::TransitionTypeIdMismatch)
        );

        let mut damaged = canonical.clone();
        damaged.before_state_id = StateId::from_bytes([0; 32]);
        assert_eq!(
            TransitionInstance::from_canonical_v1(damaged),
            Err(TransitionIdentityError::BeforeStateIdMismatch)
        );

        let mut damaged = canonical.clone();
        damaged.after_state_id = StateId::from_bytes([0; 32]);
        assert_eq!(
            TransitionInstance::from_canonical_v1(damaged),
            Err(TransitionIdentityError::AfterStateIdMismatch)
        );

        let mut damaged = canonical;
        damaged.transition_id = TransitionId::from_bytes([0; 32]);
        assert_eq!(
            TransitionInstance::from_canonical_v1(damaged),
            Err(TransitionIdentityError::TransitionIdMismatch)
        );
    }

    #[test]
    fn transition_support_batch_is_arrival_order_independent_and_set_idempotent() {
        let first = transition(ExploreValue::Unit, rich_state(199_000), rich_state(200_000));
        let second = first.clone();
        let third = transition(ExploreValue::Unit, rich_state(200_000), rich_state(201_000));
        let inputs = vec![
            (first.clone(), false, false),
            (second, true, true),
            (third, true, false),
            (first.clone(), true, false),
        ];

        let mut forward = TransitionSupportIndex::default();
        let prepared = forward.prepare_batch(inputs.clone()).unwrap();
        forward.apply_prepared_batch(prepared);

        let mut reverse = TransitionSupportIndex::default();
        let prepared = reverse.prepare_batch(inputs.into_iter().rev()).unwrap();
        reverse.apply_prepared_batch(prepared);

        assert_eq!(forward, reverse);
        assert_eq!(forward.len(), 2);
        assert_eq!(forward.state_len(), 3);
        assert_eq!(forward.admissible_transition_count(), 2);
        assert_eq!(forward.matching_transition_count(), 1);
        assert!(forward.transition_matches(first.id(), &first));
        let ordered_transitions = forward
            .iter()
            .map(|(transition_id, _)| transition_id)
            .collect::<Vec<_>>();
        assert!(ordered_transitions.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn transition_support_batch_rejects_intra_batch_state_collision_atomically() {
        let first = transition(ExploreValue::Unit, rich_state(199_000), rich_state(200_000));
        let collision = first.before_state_id();
        let mut second = transition(ExploreValue::Unit, rich_state(299_000), rich_state(300_000));
        second.before_state_id = collision;
        let index = TransitionSupportIndex::default();

        let error = index
            .prepare_batch(vec![(second, false, false), (first, false, false)])
            .unwrap_err();
        assert_eq!(
            error,
            TransitionSupportError::StateIdCollision {
                state_id: collision,
            }
        );
        assert_eq!(index, TransitionSupportIndex::default());
    }

    #[test]
    fn prepared_transition_support_batch_is_revision_bound() {
        let first = transition(ExploreValue::Unit, rich_state(199_000), rich_state(200_000));
        let second = transition(ExploreValue::Unit, rich_state(200_000), rich_state(201_000));
        let mut index = TransitionSupportIndex::default();
        let first_batch = index
            .prepare_batch([(first.clone(), false, false)])
            .unwrap();
        let stale_batch = index
            .prepare_batch([(second.clone(), false, false)])
            .unwrap();

        index.apply_prepared_batch(first_batch);
        let after_first = index.clone();
        let stale_apply = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            index.apply_prepared_batch(stale_batch);
        }));

        assert!(stale_apply.is_err());
        assert_eq!(index, after_first);
        assert!(index.transition_matches(first.id(), &first));
        assert!(!index.transition_matches(second.id(), &second));
    }
}
