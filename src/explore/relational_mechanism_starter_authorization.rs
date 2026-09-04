//! Checked value authorization for mechanism starter-support publication.
//!
//! Mechanism support authority is keyed by opaque source and successor keys.
//! Turning those keys back into typed `(Context, Before) -> After` rows is a
//! separate publication decision. This module derives that decision only from
//! an existing selected-case result view whose public projection already
//! exposes the complete case identity and endpoint values.
//!
//! The module deliberately does not register an analysis layer or publish any
//! rows. It produces a compact receipt which a later projection job can bind
//! alongside its `MechanismSupportKey` and starter-projection plan.

use std::cmp::Ordering;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::relation::{QuestionId, ViewId};
use super::relational_ir::{
    relational_tys_equivalent, ExploreAnalysisNodeIr, ExploreResultFieldIr, ExploreResultGrainIr,
    ExploreResultInputIr, ExploreResultViewIr,
};
use crate::{CheckedExploreAnalysisIdentity, CheckedExploreQueryView, ExprKind, Ty};

pub(crate) const RELATIONAL_MECHANISM_STARTER_VALUE_AUTHORIZATION_VERSION: u32 = 1;

const STARTER_VALUE_ROLE_SCHEMA_V1: &[u8] =
    b"futuruna.explore.mechanism-starter-value-role-schema.v1";
const STARTER_VALUE_AUTHORIZATION_ID_V1: &[u8] =
    b"futuruna.explore.mechanism-starter-value-authorization-id.v1";

/// Semantic values which a starter-support projection must recover from an
/// already authorized selected-case view.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum RelationalMechanismStarterValueRole {
    CaseId,
    Context,
    Before,
    After,
}

impl RelationalMechanismStarterValueRole {
    const ALL: [Self; 4] = [Self::CaseId, Self::Context, Self::Before, Self::After];

    const fn ordinal(self) -> usize {
        match self {
            Self::CaseId => 0,
            Self::Context => 1,
            Self::Before => 2,
            Self::After => 3,
        }
    }

    const fn canonical_tag(self) -> u8 {
        match self {
            Self::CaseId => 0x01,
            Self::Context => 0x02,
            Self::Before => 0x03,
            Self::After => 0x04,
        }
    }

    pub(crate) const fn binding_name(self) -> &'static str {
        match self {
            Self::CaseId => "case_id",
            Self::Context => "context",
            Self::Before => "before",
            Self::After => "after",
        }
    }
}

/// Public-projection coordinate for one required semantic value.
///
/// `select_index` and `output_name` are operational addresses in the current
/// checked view. Both are nevertheless committed by `role_schema_digest`, so
/// a publisher cannot silently read a different selected field under the same
/// authorization receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalMechanismStarterAuthorizedProjection {
    role: RelationalMechanismStarterValueRole,
    select_index: usize,
    output_name: Box<str>,
    type_digest: [u8; 32],
}

impl RelationalMechanismStarterAuthorizedProjection {
    pub(crate) const fn role(&self) -> RelationalMechanismStarterValueRole {
        self.role
    }

    pub(crate) const fn select_index(&self) -> usize {
        self.select_index
    }

    pub(crate) fn output_name(&self) -> &str {
        &self.output_name
    }

    pub(crate) const fn type_digest(&self) -> [u8; 32] {
        self.type_digest
    }
}

/// Content identity for one checked authorization receipt.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalMechanismStarterValueAuthorizationId([u8; 32]);

impl RelationalMechanismStarterValueAuthorizationId {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Reusable checked authority to join mechanism starter keys back to the
/// typed case values already exposed by one selected-case result view.
///
/// The view's source name and node index are lookup addresses and do not enter
/// `authorization_id`. Semantic authority is the selected `QuestionId`, the
/// producer-minted `ViewId`, and the exact role/schema digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalMechanismStarterValueAuthorization {
    version: u32,
    authorization_id: RelationalMechanismStarterValueAuthorizationId,
    question_id: QuestionId,
    view_id: ViewId,
    role_schema_digest: [u8; 32],
    authorizing_view_node_index: usize,
    authorizing_view_name: Box<str>,
    projections: [RelationalMechanismStarterAuthorizedProjection; 4],
}

impl RelationalMechanismStarterValueAuthorization {
    pub(crate) const fn version(&self) -> u32 {
        self.version
    }

    pub(crate) const fn authorization_id(&self) -> RelationalMechanismStarterValueAuthorizationId {
        self.authorization_id
    }

    pub(crate) const fn question_id(&self) -> QuestionId {
        self.question_id
    }

    pub(crate) const fn view_id(&self) -> ViewId {
        self.view_id
    }

    pub(crate) const fn role_schema_digest(&self) -> [u8; 32] {
        self.role_schema_digest
    }

    pub(crate) const fn authorizing_view_node_index(&self) -> usize {
        self.authorizing_view_node_index
    }

    pub(crate) fn authorizing_view_name(&self) -> &str {
        &self.authorizing_view_name
    }

    pub(crate) fn projection(
        &self,
        role: RelationalMechanismStarterValueRole,
    ) -> &RelationalMechanismStarterAuthorizedProjection {
        &self.projections[role.ordinal()]
    }

    pub(crate) fn projections(&self) -> &[RelationalMechanismStarterAuthorizedProjection; 4] {
        &self.projections
    }

    pub(crate) fn validate_identity(&self) -> bool {
        self.version == RELATIONAL_MECHANISM_STARTER_VALUE_AUTHORIZATION_VERSION
            && self
                .projections
                .iter()
                .zip(RelationalMechanismStarterValueRole::ALL)
                .all(|(projection, role)| projection.role == role)
            && projections_are_unique(&self.projections)
            && derive_role_schema_digest(&self.projections) == self.role_schema_digest
            && derive_authorization_id(
                self.version,
                self.question_id,
                self.view_id,
                self.role_schema_digest,
            ) == self.authorization_id
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum RelationalMechanismStarterAuthorizationError {
    CheckedAnalysisIdentityKindMismatch { node_index: usize },
    NoCompatibleSelectedCaseView,
    RequestedViewNotFound { view_id: ViewId },
    RequestedViewIsNotCompatible { view_id: ViewId },
}

impl fmt::Display for RelationalMechanismStarterAuthorizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CheckedAnalysisIdentityKindMismatch { node_index } => write!(
                formatter,
                "checked Explore analysis node {node_index} has a mismatched producer identity"
            ),
            Self::NoCompatibleSelectedCaseView => write!(
                formatter,
                "mechanism starter values require an existing selected-input each-case result view which directly selects case_id, context, before, and after without aggregates, having, or choice"
            ),
            Self::RequestedViewNotFound { view_id } => write!(
                formatter,
                "checked Explore analysis does not contain requested authorizing view {}",
                lowercase_hex(view_id.bytes())
            ),
            Self::RequestedViewIsNotCompatible { view_id } => write!(
                formatter,
                "requested authorizing view {} does not directly expose the complete selected case identity and endpoint values",
                lowercase_hex(view_id.bytes())
            ),
        }
    }
}

impl Error for RelationalMechanismStarterAuthorizationError {}

/// Find the least-privilege compatible authorizing view deterministically.
///
/// Fewer public selected fields win first. Equal-width candidates are ordered
/// by semantic `ViewId`, then role/schema digest. Authored declaration order
/// and view name therefore cannot change the semantic authorization choice.
/// Duplicate declarations with the same semantic view use name/index only to
/// pick a current lookup address; those addresses do not enter the receipt ID.
pub(crate) fn find_relational_mechanism_starter_value_authorization(
    checked: CheckedExploreQueryView<'_>,
) -> Result<
    RelationalMechanismStarterValueAuthorization,
    RelationalMechanismStarterAuthorizationError,
> {
    let mut candidates = compatible_candidates(checked)?;
    candidates.sort_by(compare_candidates);
    candidates
        .into_iter()
        .next()
        .map(|candidate| candidate.authorization)
        .ok_or(RelationalMechanismStarterAuthorizationError::NoCompatibleSelectedCaseView)
}

/// Validate one explicitly selected semantic view and derive its current
/// projection addresses. This is the integration point for future authored
/// `using values from VIEW` syntax; callers resolve the name to `ViewId` at the
/// checked boundary and never authorize by a raw name alone.
pub(crate) fn relational_mechanism_starter_value_authorization_for_view(
    checked: CheckedExploreQueryView<'_>,
    requested_view_id: ViewId,
) -> Result<
    RelationalMechanismStarterValueAuthorization,
    RelationalMechanismStarterAuthorizationError,
> {
    let candidates = compatible_candidates(checked)?;
    let mut matching = candidates
        .into_iter()
        .filter(|candidate| candidate.authorization.view_id == requested_view_id)
        .collect::<Vec<_>>();
    if matching.is_empty() {
        let view_exists = checked.analysis_nodes().any(|(node, identity)| {
            matches!(
                (node, identity),
                (
                    ExploreAnalysisNodeIr::Result(_),
                    CheckedExploreAnalysisIdentity::View { view_id, .. }
                ) if *view_id == requested_view_id
            )
        });
        return Err(if view_exists {
            RelationalMechanismStarterAuthorizationError::RequestedViewIsNotCompatible {
                view_id: requested_view_id,
            }
        } else {
            RelationalMechanismStarterAuthorizationError::RequestedViewNotFound {
                view_id: requested_view_id,
            }
        });
    }
    matching.sort_by(compare_candidates);
    Ok(matching.remove(0).authorization)
}

#[derive(Clone, Debug)]
struct AuthorizationCandidate {
    selected_field_count: usize,
    authorization: RelationalMechanismStarterValueAuthorization,
}

fn compatible_candidates(
    checked: CheckedExploreQueryView<'_>,
) -> Result<Vec<AuthorizationCandidate>, RelationalMechanismStarterAuthorizationError> {
    let mut candidates = Vec::new();
    for (node, identity) in checked.analysis_nodes() {
        match (node, identity) {
            (
                ExploreAnalysisNodeIr::Result(view),
                CheckedExploreAnalysisIdentity::View { view_id, .. },
            ) => {
                if let Some(authorization) = compatible_authorization(checked, view, *view_id) {
                    candidates.push(AuthorizationCandidate {
                        selected_field_count: view.select.len(),
                        authorization,
                    });
                }
            }
            (
                ExploreAnalysisNodeIr::Mechanisms(_),
                CheckedExploreAnalysisIdentity::Mechanisms { .. },
            ) => {}
            _ => {
                return Err(
                    RelationalMechanismStarterAuthorizationError::CheckedAnalysisIdentityKindMismatch {
                        node_index: node.node_index(),
                    },
                );
            }
        }
    }
    Ok(candidates)
}

fn compatible_authorization(
    checked: CheckedExploreQueryView<'_>,
    view: &ExploreResultViewIr,
    view_id: ViewId,
) -> Option<RelationalMechanismStarterValueAuthorization> {
    let ExploreResultInputIr::Find { find_index, .. } = &view.input else {
        return None;
    };
    if !matches!(&view.grain, ExploreResultGrainIr::EachCase { .. })
        || !view.aggregates.is_empty()
        || view.having.is_some()
        || view.choose.is_some()
    {
        return None;
    }

    let mut projections: [Option<RelationalMechanismStarterAuthorizedProjection>; 4] =
        [None, None, None, None];
    for (select_index, field) in view.select.iter().enumerate() {
        let Some(role) = direct_required_role(field) else {
            continue;
        };
        if projections[role.ordinal()].is_some()
            || !field_type_matches_role(checked, role, &field.ty)
        {
            return None;
        }
        projections[role.ordinal()] = Some(RelationalMechanismStarterAuthorizedProjection {
            role,
            select_index,
            output_name: field.name.as_str().into(),
            type_digest: role_value_schema_digest(checked, role),
        });
    }

    let [Some(case_id), Some(context), Some(before), Some(after)] = projections else {
        return None;
    };
    let projections = [case_id, context, before, after];
    if !projections_are_unique(&projections) {
        return None;
    }
    let role_schema_digest = derive_role_schema_digest(&projections);
    let question_id = checked.find_question_id(*find_index)?;
    let authorization_id = derive_authorization_id(
        RELATIONAL_MECHANISM_STARTER_VALUE_AUTHORIZATION_VERSION,
        question_id,
        view_id,
        role_schema_digest,
    );
    let authorization = RelationalMechanismStarterValueAuthorization {
        version: RELATIONAL_MECHANISM_STARTER_VALUE_AUTHORIZATION_VERSION,
        authorization_id,
        question_id,
        view_id,
        role_schema_digest,
        authorizing_view_node_index: view.node_index,
        authorizing_view_name: view.name.as_str().into(),
        projections,
    };
    authorization.validate_identity().then_some(authorization)
}

fn direct_required_role(
    field: &ExploreResultFieldIr,
) -> Option<RelationalMechanismStarterValueRole> {
    let ExprKind::Var(binding) = &field.value.kind else {
        return None;
    };
    RelationalMechanismStarterValueRole::ALL
        .into_iter()
        .find(|role| binding == role.binding_name())
}

fn field_type_matches_role(
    checked: CheckedExploreQueryView<'_>,
    role: RelationalMechanismStarterValueRole,
    actual: &Ty,
) -> bool {
    match role {
        RelationalMechanismStarterValueRole::CaseId => {
            matches!(actual, Ty::Name(name) if name == "CaseId")
        }
        RelationalMechanismStarterValueRole::Context => {
            relational_tys_equivalent(actual, &checked.closed_query.source.context_ty)
        }
        RelationalMechanismStarterValueRole::Before => {
            relational_tys_equivalent(actual, &checked.closed_query.source.before_ty)
        }
        RelationalMechanismStarterValueRole::After => {
            relational_tys_equivalent(actual, &checked.closed_query.successor.after_ty)
        }
    }
}

fn projections_are_unique(
    projections: &[RelationalMechanismStarterAuthorizedProjection; 4],
) -> bool {
    for (index, projection) in projections.iter().enumerate() {
        if projection.output_name.is_empty()
            || projections[..index].iter().any(|previous| {
                previous.select_index == projection.select_index
                    || previous.output_name == projection.output_name
            })
        {
            return false;
        }
    }
    true
}

fn compare_candidates(left: &AuthorizationCandidate, right: &AuthorizationCandidate) -> Ordering {
    left.selected_field_count
        .cmp(&right.selected_field_count)
        .then_with(|| {
            left.authorization
                .view_id
                .bytes()
                .cmp(&right.authorization.view_id.bytes())
        })
        .then_with(|| {
            left.authorization
                .role_schema_digest
                .cmp(&right.authorization.role_schema_digest)
        })
        .then_with(|| {
            left.authorization
                .authorizing_view_name
                .cmp(&right.authorization.authorizing_view_name)
        })
        .then_with(|| {
            left.authorization
                .authorizing_view_node_index
                .cmp(&right.authorization.authorizing_view_node_index)
        })
}

fn derive_role_schema_digest(
    projections: &[RelationalMechanismStarterAuthorizedProjection; 4],
) -> [u8; 32] {
    let mut hasher = StableAuthorizationHasher::new(STARTER_VALUE_ROLE_SCHEMA_V1);
    hasher.u32(RELATIONAL_MECHANISM_STARTER_VALUE_AUTHORIZATION_VERSION);
    hasher.u64(projections.len() as u64);
    for projection in projections {
        hasher.u8(projection.role.canonical_tag());
        hasher.u128(projection.select_index as u128);
        hasher.text(&projection.output_name);
        hasher.digest(projection.type_digest);
    }
    hasher.finish()
}

fn derive_authorization_id(
    version: u32,
    question_id: QuestionId,
    view_id: ViewId,
    role_schema_digest: [u8; 32],
) -> RelationalMechanismStarterValueAuthorizationId {
    let mut hasher = StableAuthorizationHasher::new(STARTER_VALUE_AUTHORIZATION_ID_V1);
    hasher.u32(version);
    hasher.digest(question_id.bytes());
    hasher.digest(view_id.bytes());
    hasher.digest(role_schema_digest);
    RelationalMechanismStarterValueAuthorizationId(hasher.finish())
}

fn role_value_schema_digest(
    checked: CheckedExploreQueryView<'_>,
    role: RelationalMechanismStarterValueRole,
) -> [u8; 32] {
    match role {
        RelationalMechanismStarterValueRole::CaseId => {
            Sha256::digest(b"futuruna.explore.semantic-type.CaseId.v1").into()
        }
        RelationalMechanismStarterValueRole::Context => {
            checked.transition_schemas().context_schema_id().bytes()
        }
        RelationalMechanismStarterValueRole::Before
        | RelationalMechanismStarterValueRole::After => {
            checked.transition_schemas().state_schema_id().bytes()
        }
    }
}

struct StableAuthorizationHasher(Sha256);

impl StableAuthorizationHasher {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update((domain.len() as u64).to_le_bytes());
        hasher.update(domain);
        Self(hasher)
    }

    fn u8(&mut self, value: u8) {
        self.0.update([value]);
    }

    fn u32(&mut self, value: u32) {
        self.0.update(value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.0.update(value.to_le_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.0.update(value.to_le_bytes());
    }

    fn text(&mut self, value: &str) {
        self.u64(value.len() as u64);
        self.0.update(value.as_bytes());
    }

    fn digest(&mut self, value: [u8; 32]) {
        self.0.update(value);
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

fn lowercase_hex(bytes: [u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Lexer, Parser, TypeCheckArtifacts, TypeChecker};

    fn checked_artifacts(source: &str) -> TypeCheckArtifacts {
        let mut lexer = Lexer::new(source);
        let statements = Parser::new(lexer.tokenize(), source)
            .parse_program()
            .expect("parse starter-authorization fixture");
        let artifacts = TypeChecker::check_with_artifacts(&statements, None, source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        artifacts
    }

    fn source_with_result(result_body: &str) -> String {
        format!(
            r#"
? explore starter_authorization {{
    from {{
        vary before in [1, 2]
        given context = ()
    }}
    transition after = before + 1
    find all_cases = all
    results candidate from find all_cases {{
{result_body}
    }}
}}
"#
        )
    }

    fn only_view_id(checked: CheckedExploreQueryView<'_>) -> ViewId {
        let (_, identity) = checked.analysis_nodes().next().expect("one result view");
        let CheckedExploreAnalysisIdentity::View { view_id, .. } = identity else {
            panic!("expected result-view identity");
        };
        *view_id
    }

    #[test]
    fn aliased_out_of_order_direct_roles_mint_stable_authorization() {
        let source = source_with_result(
            r#"        each case
        select [
            ending_state = after,
            stable_case = case_id,
            starting_context = context,
            starting_state = before
        ]"#,
        );
        let artifacts = checked_artifacts(&source);
        let checked = artifacts
            .checked_exploration_query(0)
            .expect("revalidated checked Explore query");

        let found = find_relational_mechanism_starter_value_authorization(checked)
            .expect("direct complete projection is authorized");
        let found_again = find_relational_mechanism_starter_value_authorization(checked)
            .expect("authorization is deterministic");
        let explicit = relational_mechanism_starter_value_authorization_for_view(
            checked,
            only_view_id(checked),
        )
        .expect("explicit semantic view lookup agrees");

        assert!(found.validate_identity());
        assert_eq!(found, found_again);
        assert_eq!(found, explicit);
        assert_eq!(found.authorizing_view_name(), "candidate");

        let expected = [
            (
                RelationalMechanismStarterValueRole::CaseId,
                1,
                "stable_case",
            ),
            (
                RelationalMechanismStarterValueRole::Context,
                2,
                "starting_context",
            ),
            (
                RelationalMechanismStarterValueRole::Before,
                3,
                "starting_state",
            ),
            (
                RelationalMechanismStarterValueRole::After,
                0,
                "ending_state",
            ),
        ];
        for (projection, (role, select_index, output_name)) in
            found.projections().iter().zip(expected)
        {
            assert_eq!(projection.role(), role);
            assert_eq!(projection.select_index(), select_index);
            assert_eq!(projection.output_name(), output_name);
        }
    }

    #[test]
    fn lossy_or_ambiguous_selected_views_are_not_authorized() {
        let fixtures = [
            (
                "missing context",
                r#"        each case
        select [stable_case = case_id, starting_state = before, ending_state = after]"#,
            ),
            (
                "computed before",
                r#"        each case
        select [
            stable_case = case_id,
            starting_context = context,
            starting_state = before + 0,
            ending_state = after
        ]"#,
            ),
            (
                "duplicate before role",
                r#"        each case
        select [
            stable_case = case_id,
            starting_context = context,
            starting_state = before,
            copied_starting_state = before,
            ending_state = after
        ]"#,
            ),
            (
                "choice-bearing view",
                r#"        each case
        select [
            stable_case = case_id,
            starting_context = context,
            starting_state = before,
            ending_state = after
        ]
        choose one maximizing before"#,
            ),
            (
                "aggregate view",
                r#"        group all
        aggregate [cases = count_distinct(case_id)]
        select [cases]"#,
            ),
        ];

        for (label, result_body) in fixtures {
            let source = source_with_result(result_body);
            let artifacts = checked_artifacts(&source);
            let checked = artifacts
                .checked_exploration_query(0)
                .expect("revalidated checked Explore query");
            let view_id = only_view_id(checked);

            assert_eq!(
                find_relational_mechanism_starter_value_authorization(checked),
                Err(RelationalMechanismStarterAuthorizationError::NoCompatibleSelectedCaseView),
                "{label}"
            );
            assert_eq!(
                relational_mechanism_starter_value_authorization_for_view(checked, view_id),
                Err(
                    RelationalMechanismStarterAuthorizationError::RequestedViewIsNotCompatible {
                        view_id,
                    }
                ),
                "{label}"
            );
        }
    }
}
