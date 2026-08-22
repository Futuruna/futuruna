//! Canonical semantic identities for before-to-after Explore transitions.
//!
//! Search coordinates keep their query-local [`ExploreCaseId`] identity. This
//! module supplies the separate semantic projection: role-neutral state nodes,
//! directional transition edges, and the exact support relation from generator
//! coordinates to those edges.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::sync::Arc;

use sha2::{Digest, Sha256};

use crate::CheckedDataTypeId;

use super::report::ExploreCaseId;
use super::{
    ExploreProductSchemaIr, ExploreTransitionIr, ExploreValue, Ty,
    TypedExploreProductSchemaIdentity,
};

const STATE_SCHEMA_ENCODING_V1: &[u8] = b"futuruna.explore.state-schema.v1";
const CONTEXT_SCHEMA_ENCODING_V1: &[u8] = b"futuruna.explore.context-schema.v1";
const TRANSITION_TYPE_ENCODING_V1: &[u8] = b"futuruna.explore.transition-type-id.v1";
const STATE_ID_HASH_V1: &[u8] = b"futuruna.explore.state-id.v1";
const TRANSITION_ID_HASH_V1: &[u8] = b"futuruna.explore.transition-id.v1";

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

const SCHEMA_IDENTITY_SYNTHETIC: u8 = 0x01;
const SCHEMA_IDENTITY_DECLARED: u8 = 0x02;
const SCHEMA_IDENTITY_UNIT: u8 = 0x03;
const SCHEMA_FIELD: u8 = 0x04;

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
    pub(crate) fn derive_checked(
        schema: &ExploreTransitionIr,
        state_owner: Option<&CheckedDataTypeId>,
        context_owner: Option<&CheckedDataTypeId>,
        resolved_type_owners: &BTreeMap<Box<str>, CheckedDataTypeId>,
    ) -> Result<Self, String> {
        let state_schema_preimage = encode_product_schema(
            "State",
            STATE_SCHEMA_ENCODING_V1,
            &schema.state_schema,
            state_owner,
            resolved_type_owners,
        )?;
        let context_schema_preimage = encode_product_schema(
            "Context",
            CONTEXT_SCHEMA_ENCODING_V1,
            &schema.context_schema,
            context_owner,
            resolved_type_owners,
        )?;
        let state_schema_id = StateSchemaId::derive(&state_schema_preimage);
        let context_schema_id = ContextSchemaId::derive(&context_schema_preimage);

        let mut encoder = CanonicalEncoder::new(TRANSITION_TYPE_ENCODING_V1);
        encoder.bytes(context_schema_id.as_ref());
        encoder.bytes(state_schema_id.as_ref());
        let transition_type_preimage = encoder.finish();
        let transition_type_id = TransitionTypeId::derive(&transition_type_preimage);

        Ok(Self {
            state_schema_id,
            context_schema_id,
            transition_type_id,
            state_schema_preimage,
            context_schema_preimage,
            transition_type_preimage,
        })
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

    pub(crate) const fn state_schema_id(&self) -> StateSchemaId {
        self.state_schema_id
    }

    pub(crate) const fn context_schema_id(&self) -> ContextSchemaId {
        self.context_schema_id
    }

    pub(crate) const fn transition_type_id(&self) -> TransitionTypeId {
        self.transition_type_id
    }

    #[cfg(test)]
    fn transition_type_preimage(&self) -> &[u8] {
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
}

/// One normalized semantic edge and the canonical values that produced it.
///
/// Schema preimages are retained privately so the support interner can reject
/// a SHA-256 collision instead of silently treating unequal edges as equal.
#[derive(Clone, Debug, PartialEq, Eq)]
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
    context: ExploreValue,
    before: ExploreValue,
    after: ExploreValue,
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
        let before_state_id = StateId::derive(state_schema_id, &before);
        let after_state_id = StateId::derive(state_schema_id, &after);
        let id = TransitionId::derive(
            transition_type_id,
            &context,
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
struct SupportedTransition {
    transition: TransitionInstance,
    case_ids: BTreeSet<ExploreCaseId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalState {
    schema_id: StateSchemaId,
    schema_preimage: Arc<[u8]>,
    value: ExploreValue,
}

/// Collision-checking interner for the exact semantic graph projection.
///
/// This is the authoritative owner of canonical state preimages and
/// directional edge preimages within the projection. Equal directional edges
/// share one entry. Multiple distinct case coordinates may support that entry,
/// while a single case coordinate may support only one edge because transition
/// normalization is a total function. It remains a projection over evaluated
/// cases, not a second source of execution truth.
#[derive(Debug, Default)]
pub(crate) struct TransitionSupportIndex {
    by_state_schema: BTreeMap<StateSchemaId, Arc<[u8]>>,
    by_context_schema: BTreeMap<ContextSchemaId, Arc<[u8]>>,
    by_transition_type: BTreeMap<TransitionTypeId, Arc<[u8]>>,
    by_state: BTreeMap<StateId, CanonicalState>,
    by_transition: BTreeMap<TransitionId, SupportedTransition>,
    by_case: BTreeMap<ExploreCaseId, TransitionId>,
}

/// A collision-checked support insertion with no remaining semantic failure.
///
/// The exact accumulator prepares this together with its classification
/// update, then applies both only after every fallible check has succeeded.
pub(crate) struct PreparedTransitionSupportInsert {
    transition_id: TransitionId,
    transition: TransitionInstance,
    case_id: ExploreCaseId,
}

impl TransitionSupportIndex {
    pub(crate) fn intern(
        &mut self,
        transition: TransitionInstance,
        case_id: ExploreCaseId,
    ) -> Result<TransitionId, TransitionSupportError> {
        let prepared = self.prepare_intern(transition, case_id)?;
        Ok(self.commit_prepared(prepared))
    }

    pub(crate) fn prepare_intern(
        &self,
        transition: TransitionInstance,
        case_id: ExploreCaseId,
    ) -> Result<PreparedTransitionSupportInsert, TransitionSupportError> {
        let transition_id = transition.id();

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
            &transition.before,
        )?;
        self.require_equal_state_preimage(
            transition.after_state_id,
            transition.state_schema_id,
            &transition.state_schema_preimage,
            &transition.after,
        )?;
        if transition.before_state_id == transition.after_state_id
            && transition.before != transition.after
        {
            return Err(TransitionSupportError::StateIdCollision {
                state_id: transition.before_state_id,
            });
        }

        if let Some(existing) = self.by_case.get(&case_id) {
            if *existing != transition_id {
                return Err(TransitionSupportError::CaseRemapped {
                    case_id: case_id.clone(),
                    existing: *existing,
                    attempted: transition_id,
                });
            }
        }

        if let Some(existing) = self.by_transition.get(&transition_id) {
            if existing.transition != transition {
                return Err(TransitionSupportError::TransitionIdCollision { transition_id });
            }
        }

        Ok(PreparedTransitionSupportInsert {
            transition_id,
            transition,
            case_id,
        })
    }

    pub(crate) fn commit_prepared(
        &mut self,
        prepared: PreparedTransitionSupportInsert,
    ) -> TransitionId {
        let PreparedTransitionSupportInsert {
            transition_id,
            transition,
            case_id,
        } = prepared;

        self.by_state_schema
            .entry(transition.state_schema_id)
            .or_insert_with(|| transition.state_schema_preimage.clone());
        self.by_context_schema
            .entry(transition.context_schema_id)
            .or_insert_with(|| transition.context_schema_preimage.clone());
        self.by_transition_type
            .entry(transition.transition_type_id)
            .or_insert_with(|| transition.transition_type_preimage.clone());

        let before_state = CanonicalState {
            schema_id: transition.state_schema_id,
            schema_preimage: transition.state_schema_preimage.clone(),
            value: transition.before.clone(),
        };
        let after_state = CanonicalState {
            schema_id: transition.state_schema_id,
            schema_preimage: transition.state_schema_preimage.clone(),
            value: transition.after.clone(),
        };
        self.by_state
            .entry(transition.before_state_id)
            .or_insert(before_state);
        self.by_state
            .entry(transition.after_state_id)
            .or_insert(after_state);

        let entry =
            self.by_transition
                .entry(transition_id)
                .or_insert_with(|| SupportedTransition {
                    transition,
                    case_ids: BTreeSet::new(),
                });
        entry.case_ids.insert(case_id.clone());
        self.by_case.entry(case_id).or_insert(transition_id);
        transition_id
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
                || &existing.value != value
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

    pub(crate) fn case_len(&self) -> usize {
        self.by_case.len()
    }

    /// Exact schema identity, schema preimage and value for one interned state node.
    pub(crate) fn state(&self, id: StateId) -> Option<(StateSchemaId, &[u8], &ExploreValue)> {
        self.by_state.get(&id).map(|state| {
            (
                state.schema_id,
                state.schema_preimage.as_ref(),
                &state.value,
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
                &state.value,
            )
        })
    }

    pub(crate) fn transition(&self, id: TransitionId) -> Option<&TransitionInstance> {
        self.by_transition.get(&id).map(|entry| &entry.transition)
    }

    pub(crate) fn support(&self, id: TransitionId) -> Option<&BTreeSet<ExploreCaseId>> {
        self.by_transition.get(&id).map(|entry| &entry.case_ids)
    }

    pub(crate) fn transition_for_case(&self, case_id: &ExploreCaseId) -> Option<TransitionId> {
        self.by_case.get(case_id).copied()
    }

    pub(crate) fn iter(
        &self,
    ) -> impl Iterator<Item = (TransitionId, &TransitionInstance, &BTreeSet<ExploreCaseId>)> {
        self.by_transition
            .iter()
            .map(|(id, entry)| (*id, &entry.transition, &entry.case_ids))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TransitionSupportError {
    StateSchemaIdCollision {
        schema_id: StateSchemaId,
    },
    ContextSchemaIdCollision {
        schema_id: ContextSchemaId,
    },
    TransitionTypeIdCollision {
        type_id: TransitionTypeId,
    },
    StateIdCollision {
        state_id: StateId,
    },
    TransitionIdCollision {
        transition_id: TransitionId,
    },
    CaseRemapped {
        case_id: ExploreCaseId,
        existing: TransitionId,
        attempted: TransitionId,
    },
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
            Self::CaseRemapped { .. } => formatter
                .write_str("one Explore case coordinate cannot support two semantic transitions"),
        }
    }
}

impl Error for TransitionSupportError {}

fn encode_product_schema(
    role: &str,
    domain: &[u8],
    schema: &ExploreProductSchemaIr,
    declared_owner: Option<&CheckedDataTypeId>,
    resolved_type_owners: &BTreeMap<Box<str>, CheckedDataTypeId>,
) -> Result<Arc<[u8]>, String> {
    let mut encoder = CanonicalEncoder::new(domain);
    match &schema.identity {
        TypedExploreProductSchemaIdentity::Synthetic { version } => {
            if declared_owner.is_some() {
                return Err(format!(
                    "synthetic Explore {role} schema unexpectedly has a declared type owner"
                ));
            }
            encoder.tag(SCHEMA_IDENTITY_SYNTHETIC);
            encoder.u32(*version);
        }
        TypedExploreProductSchemaIdentity::Declared { ty } => {
            let owner = declared_owner.ok_or_else(|| {
                format!("declared Explore {role} schema has no checked type owner")
            })?;
            encoder.tag(SCHEMA_IDENTITY_DECLARED);
            encode_checked_data_type_id(&mut encoder, owner);
            encode_ty(&mut encoder, ty, resolved_type_owners)
                .map_err(|message| format!("declared Explore {role} schema identity: {message}"))?;
        }
        TypedExploreProductSchemaIdentity::Unit => {
            if declared_owner.is_some() {
                return Err(format!(
                    "unit Explore {role} schema unexpectedly has a declared type owner"
                ));
            }
            encoder.tag(SCHEMA_IDENTITY_UNIT);
        }
    }
    encoder.count(schema.fields.len());
    for field in &schema.fields {
        encoder.tag(SCHEMA_FIELD);
        encoder.count(field.field_index);
        encoder.bytes(field.name.as_bytes());
        encode_ty(&mut encoder, &field.value_ty, resolved_type_owners).map_err(|message| {
            format!(
                "Explore {role} schema field `{}` identity: {message}",
                field.name
            )
        })?;
    }
    Ok(encoder.finish())
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
            for (name, value) in fields {
                hasher.bytes(name.as_bytes());
                hash_explore_value(hasher, value);
            }
        }
    }
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
    use crate::{ExploreTransitionMode, Span};

    use super::super::{
        ExploreAfterFieldIr, ExploreAfterFieldSourceIr, ExploreProductFieldIr,
        ExploreProductFieldSourceIr,
    };
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
            ],
        }
    }

    fn transition(
        context: ExploreValue,
        before: ExploreValue,
        after: ExploreValue,
    ) -> TransitionInstance {
        TransitionInstance::new(STATE_SCHEMA, TRANSITION_SCHEMA, context, before, after)
    }

    fn minimal_transition_ir(
        mode: ExploreTransitionMode,
        after_source: ExploreAfterFieldSourceIr,
        context_schema_version: u32,
    ) -> ExploreTransitionIr {
        ExploreTransitionIr {
            normalization_version: 1,
            mode,
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

    #[test]
    fn relation_identity_excludes_mode_and_after_construction_topology() {
        let baseline = minimal_transition_ir(
            ExploreTransitionMode::Relative,
            ExploreAfterFieldSourceIr::FrameBefore {
                before_field_index: 0,
            },
            1,
        );
        let different_mode = minimal_transition_ir(
            ExploreTransitionMode::Independent,
            ExploreAfterFieldSourceIr::FrameBefore {
                before_field_index: 0,
            },
            1,
        );
        let different_topology = minimal_transition_ir(
            ExploreTransitionMode::Relative,
            ExploreAfterFieldSourceIr::IndependentDomain { dimension_index: 1 },
            1,
        );

        let owners = intrinsic_type_owners();
        let baseline =
            TransitionSchemaIdentities::derive_checked(&baseline, None, None, &owners).unwrap();
        let different_mode =
            TransitionSchemaIdentities::derive_checked(&different_mode, None, None, &owners)
                .unwrap();
        let different_topology =
            TransitionSchemaIdentities::derive_checked(&different_topology, None, None, &owners)
                .unwrap();

        for schemas in [&different_mode, &different_topology] {
            assert_eq!(baseline.state_schema_id(), schemas.state_schema_id());
            assert_eq!(baseline.context_schema_id(), schemas.context_schema_id());
            assert_eq!(baseline.transition_type_id(), schemas.transition_type_id());
            assert_eq!(
                minimal_semantic_edge(&baseline).id(),
                minimal_semantic_edge(schemas).id()
            );
        }
    }

    #[test]
    fn relation_schema_identity_encodes_context_before_state() {
        let transition = minimal_transition_ir(
            ExploreTransitionMode::Relative,
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

    #[test]
    fn context_schema_identity_changes_relation_and_transition_identity() {
        let context_v1 = minimal_transition_ir(
            ExploreTransitionMode::Relative,
            ExploreAfterFieldSourceIr::FrameBefore {
                before_field_index: 0,
            },
            1,
        );
        let context_v2 = minimal_transition_ir(
            ExploreTransitionMode::Relative,
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

    #[test]
    fn declared_product_schema_identity_includes_checked_owner() {
        let mut transition = minimal_transition_ir(
            ExploreTransitionMode::Identity,
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

    #[test]
    fn declared_product_schema_identity_includes_nested_checked_owners() {
        let mut transition = minimal_transition_ir(
            ExploreTransitionMode::Identity,
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

    #[test]
    fn optional_sugar_and_option_application_share_schema_identity() {
        let mut sugar = minimal_transition_ir(
            ExploreTransitionMode::Identity,
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
        let first_case = ExploreCaseId::new(vec![0_u128]);
        let second_case = ExploreCaseId::new(vec![1_u128]);
        let first = transition(ExploreValue::Unit, rich_state(199_000), rich_state(200_000));
        let second = transition(ExploreValue::Unit, rich_state(200_000), rich_state(201_000));
        let shared_state_id = first.after_state_id();
        let mut index = TransitionSupportIndex::default();

        assert_eq!(shared_state_id, second.before_state_id());
        assert_eq!(first.after(), second.before());

        index.intern(first, first_case).unwrap();
        index.intern(second, second_case).unwrap();

        assert_eq!(index.state_len(), 3);
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
            let first_case = ExploreCaseId::new(vec![0_u128]);
            let attempted_case = ExploreCaseId::new(vec![1_u128]);
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
            index.intern(existing, first_case).unwrap();

            assert_eq!(
                index.intern(attempted, attempted_case.clone()),
                Err(TransitionSupportError::StateIdCollision {
                    state_id: colliding_state_id,
                })
            );
            assert_eq!(index.state_len(), 2);
            assert_eq!(index.len(), 1);
            assert_eq!(index.transition_for_case(&attempted_case), None);
        }
    }

    #[test]
    fn schema_preimage_collisions_are_rejected_before_value_identity() {
        let first_case = ExploreCaseId::new(vec![0_u128]);
        let attempted_case = ExploreCaseId::new(vec![1_u128]);
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
        index.intern(existing, first_case).unwrap();

        assert_eq!(
            index.intern(attempted, attempted_case.clone()),
            Err(TransitionSupportError::ContextSchemaIdCollision {
                schema_id: colliding_schema_id,
            })
        );
        assert_eq!(index.state_len(), 2);
        assert_eq!(index.len(), 1);
        assert_eq!(index.transition_for_case(&attempted_case), None);
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
    fn two_case_ids_retain_exact_support_for_one_edge() {
        let first_case = ExploreCaseId::new(vec![0_u128, 2_u128]);
        let second_case = ExploreCaseId::new(vec![1_u128, 2_u128]);
        let edge = transition(
            ExploreValue::Int(1_000),
            rich_state(199_000),
            rich_state(200_000),
        );
        let equal_edge = edge.clone();
        let mut index = TransitionSupportIndex::default();

        let first_id = index.intern(edge, first_case.clone()).unwrap();
        let second_id = index.intern(equal_edge, second_case.clone()).unwrap();

        assert_eq!(first_id, second_id);
        assert_eq!(index.len(), 1);
        assert_eq!(
            index.support(first_id),
            Some(&BTreeSet::from([first_case.clone(), second_case.clone()]))
        );
        assert_eq!(index.transition_for_case(&first_case), Some(first_id));
        assert_eq!(index.transition_for_case(&second_case), Some(first_id));
    }
}
