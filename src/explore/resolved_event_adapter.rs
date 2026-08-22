//! Proof-first bridge from checked source resolution to source-event IR.
//!
//! This adapter is intentionally a narrow consumer of the Phase-A declaration
//! snapshot and the Phase-B resolution artifact.  It never opens an import,
//! parses source again, or resolves a spelling.  The source contract consumed
//! by this file is frozen to the `src/lib.rs` digest below; changing that
//! contract requires a fresh seam review before this module may be trusted.
//!
//! The abstract interpreter is deliberately partial.  Unsupported semantics
//! become [`UnsupportedResidual`] values, while resolution uncertainty is a
//! fatal adapter error.  In particular, a residual can make extraction
//! incomplete but can never close the complement of emitted candidates.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::num::NonZeroUsize;
use std::sync::Arc;

use sha2::{Digest, Sha256};

use super::super::{
    ExploreExactDomain, ExploreFactValue, ExploreFiniteTypePlan, ExploreQueryIr, ExploreValue,
};
use super::{
    AffineForm, BoundaryAxisInterval, BoundaryFragmentRootRole, BoundaryIntExpr, BoundaryPredicate,
    BoundaryRelation, ResolvedAxisSupport, ResolvedBoundaryFragment, ResolvedBoundaryNode,
    ResolvedBoundaryRoot, ResolvedClassificationFormula, ResolvedFragmentCoverage,
    ResolvedLivenessCertificate, ResolvedQuantizedTerm, ResolvedQuasiAffineForm, SourceGuard,
    SourceSite, SourceSiteId, TieArm, UnsupportedResidual, UnsupportedResidualKind,
};
use crate::{
    named_arg_parts, AnalysisProgramId, AstChild, CheckedBinderKind, CheckedBinderSiteId,
    CheckedCallTarget, CheckedCallableId, CheckedConstructorIdentity, CheckedConstructorLayout,
    CheckedDataTypeId, CheckedDeclarationOccurrenceId, CheckedExploreGroundConstructorSite,
    CheckedExploreQueryAccessError, CheckedExploreQueryArtifact, CheckedExploreTypeUse,
    CheckedExpressionResolution, CheckedFieldResolution, CheckedNamedArgumentOrder,
    CheckedPatternSiteId, CheckedResolutionIssue, CheckedTopLevelBindingId, CheckedValueBinding,
    Defn, Expr, ExprKind, ExprSiteId, Literal, Pat, RuleDispatchKey, SourcedStmt, Stmt,
    TypeCheckArtifacts, TypeDecl, TypedExploreBound,
};

/// Reviewed Phase-B source-resolution contract consumed by this adapter.
pub(in crate::explore) const CHECKED_RESOLUTION_CONTRACT_SHA256: &str =
    "e82622d57dd1978274a82f907dd862a0e88206a83f822582172d718c6340d152";

const LIVENESS_CERTIFICATE_DOMAIN: &[u8] = b"futuruna.explore.resolved-event-liveness.v1";
const BINDER_PARAMETER: u32 = 0;
const BINDER_PATTERN: u32 = 1;

/// Hard bounds for an adapter pass.  These limit analysis work, never the
/// semantic query universe and never the ordinary evaluator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::explore) struct ResolvedEventAdapterLimits {
    pub(in crate::explore) max_reachable_sites: NonZeroUsize,
    pub(in crate::explore) max_abstract_steps: NonZeroUsize,
    pub(in crate::explore) max_call_depth: NonZeroUsize,
    pub(in crate::explore) max_collection_items: NonZeroUsize,
    pub(in crate::explore) max_residuals: NonZeroUsize,
}

/// First-generation durable-probe adapter budget.
///
/// Preparation pays the structural/reachability bounds once. Adaptation then
/// pays at most `max_abstract_steps` for each of the at most 64 selected outer
/// profiles, so the dominant profile-dependent abstract work is capped at
/// 4,194,304 charged steps. Exhaustion makes the optional source analysis
/// unavailable or incomplete; the exact universe remains canonical fallback
/// work. Every field is bound into the immutable probe-plan hash.
pub(in crate::explore) const SOURCE_PROOF_ADAPTER_LIMITS_V1: ResolvedEventAdapterLimits =
    ResolvedEventAdapterLimits {
        max_reachable_sites: NonZeroUsize::new(32_768).unwrap(),
        max_abstract_steps: NonZeroUsize::new(65_536).unwrap(),
        max_call_depth: NonZeroUsize::new(64).unwrap(),
        max_collection_items: NonZeroUsize::new(1_024).unwrap(),
        max_residuals: NonZeroUsize::new(512).unwrap(),
    };

impl Default for ResolvedEventAdapterLimits {
    fn default() -> Self {
        SOURCE_PROOF_ADAPTER_LIMITS_V1
    }
}

/// The query is selected by its accepted-artifact ordinal rather than by a
/// name.  `analysis_program_hash` must be the exact Phase-A identity.  The
/// query hash is supplied by the checked-query identity layer and is bound
/// into every liveness certificate.
pub(in crate::explore) struct ResolvedEventAdapterRequest<'a> {
    pub(in crate::explore) artifacts: &'a TypeCheckArtifacts,
    pub(in crate::explore) accepted_query_index: usize,
    pub(in crate::explore) analysis_program_hash: &'a str,
    pub(in crate::explore) query_hash: &'a str,
    /// Source-order ordinals for all dimensions except the boundary axis.
    pub(in crate::explore) outer_ordinals: &'a [u128],
    pub(in crate::explore) limits: ResolvedEventAdapterLimits,
}

#[derive(Debug, Clone)]
pub(in crate::explore) struct AdaptedBoundaryFragment {
    pub(in crate::explore) fragment: ResolvedBoundaryFragment,
    /// The exact, sorted Phase-B site closure on which fail-closed validation
    /// was performed.  This is audit evidence, not a complement certificate.
    pub(in crate::explore) reachable_sites: Box<[ExprSiteId]>,
}

#[derive(Debug, Clone)]
pub(in crate::explore) enum ResolvedEventAdapterError {
    OrdinaryCheckerRejected {
        diagnostics: usize,
    },
    AcceptedQueryArtifactDiverged,
    AnalysisProgramIdentityMismatch,
    AnalysisProgramHashMismatch,
    InvalidQueryHash,
    QueryHasNoBoundary,
    InvalidBoundaryAxis,
    CheckedQueryAccess(CheckedExploreQueryAccessError),
    OuterOrdinalArityMismatch {
        expected: usize,
        actual: usize,
    },
    OuterOrdinalOutOfBounds {
        dimension: Box<str>,
        ordinal: u128,
    },
    OuterProfileMaterializationLimit {
        dimension: Box<str>,
        limit: usize,
    },
    OuterProfileAccessLimit {
        dimension: Box<str>,
        resource: &'static str,
        limit: usize,
    },
    StructuralIndexLimit {
        limit: usize,
    },
    ReachabilityLimit {
        limit: usize,
    },
    ReachabilityWorkLimit {
        limit: usize,
    },
    RuleCandidateLimit {
        limit: usize,
    },
    FatalResolutionIssues(Box<[CheckedResolutionIssue]>),
    InternalArtifactGap(Box<str>),
}

impl fmt::Display for ResolvedEventAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OrdinaryCheckerRejected { diagnostics } => write!(
                formatter,
                "resolved source-event adaptation requires an accepted checker artifact; found {diagnostics} diagnostics"
            ),
            Self::AcceptedQueryArtifactDiverged => formatter.write_str(
                "typed Explore query and closed-universe artifacts do not identify the same accepted query",
            ),
            Self::AnalysisProgramIdentityMismatch => formatter.write_str(
                "Phase-A declarations and Phase-B resolutions belong to different analysis programs",
            ),
            Self::AnalysisProgramHashMismatch => formatter.write_str(
                "requested analysis-program hash is not the exact checked Phase-A identity",
            ),
            Self::InvalidQueryHash => formatter.write_str(
                "requested query hash is not the producer-minted checked-query digest",
            ),
            Self::QueryHasNoBoundary => formatter.write_str(
                "resolved source-event adaptation requires a boundary query",
            ),
            Self::InvalidBoundaryAxis => formatter.write_str(
                "checked boundary axis/index/step contract is internally inconsistent",
            ),
            Self::CheckedQueryAccess(error) => write!(
                formatter,
                "producer-minted checked Explore query is unavailable: {error:?}"
            ),
            Self::OuterOrdinalArityMismatch { expected, actual } => write!(
                formatter,
                "outer profile has {actual} ordinals, expected {expected}"
            ),
            Self::OuterOrdinalOutOfBounds { dimension, ordinal } => write!(
                formatter,
                "outer ordinal {ordinal} is outside dimension `{dimension}`"
            ),
            Self::OuterProfileMaterializationLimit { dimension, limit } => write!(
                formatter,
                "dimension `{dimension}` exceeds the bounded profile materialization limit {limit}"
            ),
            Self::OuterProfileAccessLimit {
                dimension,
                resource,
                limit,
            } => write!(
                formatter,
                "dimension `{dimension}` exceeds finite-type {resource} limit {limit}"
            ),
            Self::StructuralIndexLimit { limit } => write!(
                formatter,
                "Phase-A structural indexing exceeds bounded preparation limit {limit}"
            ),
            Self::ReachabilityLimit { limit } => write!(
                formatter,
                "checked reachable expression-site closure exceeds limit {limit}"
            ),
            Self::ReachabilityWorkLimit { limit } => write!(
                formatter,
                "checked reachability preparation exceeds bounded work limit {limit}"
            ),
            Self::RuleCandidateLimit { limit } => write!(
                formatter,
                "reachable checked rule family exceeds preparation candidate limit {limit}"
            ),
            Self::FatalResolutionIssues(issues) => write!(
                formatter,
                "reachable Phase-B resolution has fatal issues: {issues:?}"
            ),
            Self::InternalArtifactGap(detail) => formatter.write_str(detail),
        }
    }
}

impl std::error::Error for ResolvedEventAdapterError {}

#[derive(Clone, Copy)]
struct IndexedExpr<'a> {
    expression: &'a Expr,
    declaration: &'a SourcedStmt,
}

#[derive(Clone)]
struct CallableEntry {
    body_site: ExprSiteId,
    parameter_sites: Box<[CheckedBinderSiteId]>,
}

struct ProgramIndex<'a> {
    program_id: &'a AnalysisProgramId,
    declarations: BTreeMap<CheckedDeclarationOccurrenceId, &'a SourcedStmt>,
    expressions: BTreeMap<ExprSiteId, IndexedExpr<'a>>,
    callables: BTreeMap<CheckedCallableId, CallableEntry>,
}

/// Read-only checked source index shared with the private mechanism replay
/// adapter.  Every lookup is keyed by [`ExprSiteId`] or [`CheckedCallableId`];
/// source spans and AST addresses remain diagnostic-only and never become
/// durable identity.
pub(in crate::explore) struct CheckedProgramSiteIndex<'a> {
    inner: ProgramIndex<'a>,
}

pub(in crate::explore) struct CheckedSourceExpression<'a> {
    pub(in crate::explore) site: ExprSiteId,
    pub(in crate::explore) expression: &'a Expr,
}

pub(in crate::explore) struct CheckedSourceExpressionSlice<'a> {
    pub(in crate::explore) root: CheckedSourceExpression<'a>,
    pub(in crate::explore) descendants: Box<[CheckedSourceExpression<'a>]>,
}

pub(in crate::explore) struct CheckedCallableSourceSlice<'a> {
    pub(in crate::explore) declaration: &'a SourcedStmt,
    pub(in crate::explore) body: CheckedSourceExpressionSlice<'a>,
}

struct StructuralIndexBudget {
    work: usize,
    work_limit: usize,
    site_limit: usize,
    depth_limit: usize,
    exhausted: bool,
    failure_limit: Option<usize>,
}

impl StructuralIndexBudget {
    fn new(limits: ResolvedEventAdapterLimits) -> Self {
        Self {
            work: 0,
            work_limit: limits.max_abstract_steps.get(),
            site_limit: limits.max_reachable_sites.get(),
            depth_limit: limits.max_call_depth.get(),
            exhausted: false,
            failure_limit: None,
        }
    }

    fn charge(&mut self) -> bool {
        if self.exhausted || self.work >= self.work_limit {
            self.exhausted = true;
            self.failure_limit.get_or_insert(self.work_limit);
            return false;
        }
        self.work += 1;
        true
    }

    fn admit_site(&mut self, current_sites: usize) -> bool {
        if current_sites >= self.site_limit {
            self.exhausted = true;
            self.failure_limit.get_or_insert(self.site_limit);
            false
        } else {
            true
        }
    }

    fn exhausted_limit(&self) -> usize {
        self.failure_limit.unwrap_or(self.site_limit)
    }

    fn remaining_work(&self) -> usize {
        self.work_limit.saturating_sub(self.work)
    }

    fn admit_iteration_width(&mut self, width: usize) -> bool {
        if width > self.remaining_work() || width > u32::MAX as usize {
            self.exhausted = true;
            self.failure_limit
                .get_or_insert(self.work_limit.min(u32::MAX as usize));
            false
        } else {
            true
        }
    }

    fn admit_depth(&mut self, depth: usize) -> bool {
        if depth >= self.depth_limit {
            self.exhausted = true;
            self.failure_limit.get_or_insert(self.depth_limit);
            false
        } else {
            true
        }
    }
}

fn statement_iteration_width(statement: &Stmt) -> usize {
    match statement {
        Stmt::Defn(Defn::Actor { handlers, .. }) => handlers.len(),
        Stmt::Defn(Defn::Module { body, .. }) => body.len(),
        Stmt::TypeDecl(TypeDecl::ADT { methods, .. })
        | Stmt::TypeDecl(TypeDecl::ImplBlock { methods, .. }) => {
            methods.iter().fold(methods.len(), |width, method| {
                width.saturating_add(match method {
                    Defn::Fn { .. } => 1,
                    Defn::Actor { handlers, .. } => handlers.len(),
                    Defn::Module { body, .. } => body.len(),
                })
            })
        }
        Stmt::TypeDecl(TypeDecl::TraitDecl { methods, .. }) => methods.len(),
        Stmt::TypeDecl(TypeDecl::RuleScope { body, .. }) => body.len(),
        Stmt::Rule(crate::Rule::ReactiveScope { body, .. }) => body.len(),
        Stmt::Annot(_, arguments) | Stmt::Assert(_, arguments) | Stmt::Retract(_, arguments) => {
            arguments.len()
        }
        Stmt::For(_, _, body) | Stmt::While(_, body) => body.len().saturating_add(1),
        Stmt::StreamSub(_, arms) => arms.len().saturating_mul(2).saturating_add(1),
        Stmt::Prove {
            pass_block,
            else_block,
            ..
        } => pass_block
            .as_ref()
            .map_or(0, |statements| statements.len())
            .saturating_add(else_block.as_ref().map_or(0, |statements| statements.len())),
        Stmt::Explore(query) => query
            .bounds
            .len()
            .saturating_add(query.boundary.is_some() as usize)
            .saturating_add(query.output.key.len())
            .saturating_add(query.output.extrema.len())
            .saturating_add(query.output.show.len())
            .saturating_add(
                (!matches!(
                    &query.output.representative,
                    crate::ExploreRepresentative::First { .. }
                )) as usize,
            ),
        _ => 2,
    }
}

fn statement_container_width(statement: &Stmt) -> usize {
    match statement {
        Stmt::Defn(Defn::Actor { handlers, .. }) => handlers.len(),
        Stmt::Defn(Defn::Module { body, .. }) => body.len(),
        Stmt::TypeDecl(TypeDecl::ADT { methods, .. })
        | Stmt::TypeDecl(TypeDecl::ImplBlock { methods, .. }) => methods.len(),
        Stmt::TypeDecl(TypeDecl::TraitDecl { methods, .. }) => methods.len(),
        Stmt::TypeDecl(TypeDecl::RuleScope { body, .. }) => body.len(),
        Stmt::Rule(crate::Rule::ReactiveScope { body, .. }) => body.len(),
        Stmt::StreamSub(_, arms) => arms.len(),
        _ => statement_iteration_width(statement),
    }
}

fn expression_iteration_width(expression: &Expr) -> usize {
    match &expression.kind {
        ExprKind::App(_, arguments) => arguments.len().saturating_add(1),
        ExprKind::Match(_, arms) => arms.len().saturating_mul(2).saturating_add(1),
        ExprKind::Block(statements) => statements.len(),
        ExprKind::List(items)
        | ExprKind::Tuple(items)
        | ExprKind::Conjunction(items)
        | ExprKind::Disjunction(items) => items.len(),
        ExprKind::Effect(_, arguments) => arguments.len(),
        ExprKind::Handle { handlers, .. } => handlers.len().saturating_add(1),
        _ => 3,
    }
}

fn bounded_expression_child_count(expression: &Expr, limit: usize) -> (usize, bool) {
    let exact = match &expression.kind {
        ExprKind::App(_, arguments) => Some(arguments.len().saturating_add(1)),
        ExprKind::Lambda(_, _)
        | ExprKind::UnOp(_, _)
        | ExprKind::Field(_, _)
        | ExprKind::Try(_) => Some(1),
        ExprKind::BinOp(_, _, _) | ExprKind::Index(_, _) | ExprKind::Pipe(_, _) => Some(2),
        ExprKind::If(_, _, _) => Some(3),
        ExprKind::Match(_, arms) => {
            let mut count = 1_usize;
            for arm in arms.iter().take(limit) {
                count = count
                    .saturating_add(arm.guard.is_some() as usize)
                    .saturating_add(1);
                if count > limit {
                    return (limit, true);
                }
            }
            if arms.len() > limit {
                return (limit, true);
            }
            Some(count)
        }
        ExprKind::Block(statements) => return (0, !statements.is_empty()),
        ExprKind::List(items)
        | ExprKind::Tuple(items)
        | ExprKind::Conjunction(items)
        | ExprKind::Disjunction(items) => Some(items.len()),
        ExprKind::Effect(_, arguments) => Some(arguments.len()),
        ExprKind::Handle { handlers, .. } => Some(handlers.len().saturating_add(1)),
        ExprKind::Var(_) | ExprKind::Lit(_) | ExprKind::Unit => Some(0),
    }
    .unwrap_or(0);
    (exact.min(limit), exact > limit)
}

impl<'a> ProgramIndex<'a> {
    fn site_count(&self) -> usize {
        self.declarations
            .len()
            .saturating_add(self.expressions.len())
            .saturating_add(self.callables.len())
    }

    fn build(
        artifacts: &'a TypeCheckArtifacts,
        limits: ResolvedEventAdapterLimits,
    ) -> Result<Self, ResolvedEventAdapterError> {
        let program = &artifacts.analysis_program;
        let mut index = Self {
            program_id: &program.id,
            declarations: BTreeMap::new(),
            expressions: BTreeMap::new(),
            callables: BTreeMap::new(),
        };
        let mut budget = StructuralIndexBudget::new(limits);
        for declaration in program.declarations.iter() {
            if !budget.charge() || !budget.admit_site(index.site_count()) {
                return Err(ResolvedEventAdapterError::StructuralIndexLimit {
                    limit: budget.exhausted_limit(),
                });
            }
            let occurrence = occurrence(declaration);
            index.declarations.insert(occurrence, declaration);
            index.index_stmt_children(declaration, &declaration.statement, &[], 0, &mut budget);
            index.index_callable_declaration(declaration, &mut budget);
            if budget.exhausted {
                return Err(ResolvedEventAdapterError::StructuralIndexLimit {
                    limit: budget.exhausted_limit(),
                });
            }
        }
        Ok(index)
    }

    fn index_stmt_children(
        &mut self,
        declaration: &'a SourcedStmt,
        statement: &'a Stmt,
        path: &[u32],
        depth: usize,
        budget: &mut StructuralIndexBudget,
    ) {
        if !budget.admit_depth(depth) {
            return;
        }
        if !budget.admit_iteration_width(statement_container_width(statement)) {
            return;
        }
        if !budget.admit_iteration_width(statement_iteration_width(statement)) {
            return;
        }
        let mut child_index = 0_u32;
        crate::visit_ast_stmt_children(statement, &mut |child| {
            if !budget.charge() {
                return;
            }
            let mut child_path = path.to_vec();
            child_path.push(child_index);
            let Some(next_child) = child_index.checked_add(1) else {
                budget.exhausted = true;
                return;
            };
            child_index = next_child;
            match child {
                AstChild::Expr(expression) => self.index_expr(
                    declaration,
                    expression,
                    &child_path,
                    depth.saturating_add(1),
                    budget,
                ),
                AstChild::Stmt(statement) => self.index_stmt_children(
                    declaration,
                    statement,
                    &child_path,
                    depth.saturating_add(1),
                    budget,
                ),
            }
        });
    }

    fn index_expr(
        &mut self,
        declaration: &'a SourcedStmt,
        expression: &'a Expr,
        path: &[u32],
        depth: usize,
        budget: &mut StructuralIndexBudget,
    ) {
        if !budget.admit_depth(depth) {
            return;
        }
        if !budget.admit_site(self.site_count()) {
            return;
        }
        if !budget.admit_iteration_width(expression_iteration_width(expression)) {
            return;
        }
        let site = expression_site(self.program_id, declaration, path);
        self.expressions.insert(
            site,
            IndexedExpr {
                expression,
                declaration,
            },
        );
        let mut child_index = 0_u32;
        crate::visit_ast_expr_children(expression, &mut |child| {
            if !budget.charge() {
                return;
            }
            let mut child_path = path.to_vec();
            child_path.push(child_index);
            let Some(next_child) = child_index.checked_add(1) else {
                budget.exhausted = true;
                return;
            };
            child_index = next_child;
            match child {
                AstChild::Expr(expression) => self.index_expr(
                    declaration,
                    expression,
                    &child_path,
                    depth.saturating_add(1),
                    budget,
                ),
                AstChild::Stmt(statement) => self.index_stmt_children(
                    declaration,
                    statement,
                    &child_path,
                    depth.saturating_add(1),
                    budget,
                ),
            }
        });
    }

    fn index_callable_declaration(
        &mut self,
        declaration: &'a SourcedStmt,
        budget: &mut StructuralIndexBudget,
    ) {
        if !budget.charge() {
            return;
        }
        match &*declaration.statement {
            Stmt::Defn(Defn::Fn { params, body, .. }) => {
                self.insert_callable(declaration, &[], params, body, vec![0], &[], budget);
            }
            Stmt::TypeDecl(TypeDecl::ADT { methods, .. })
            | Stmt::TypeDecl(TypeDecl::ImplBlock { methods, .. }) => {
                let mut child = 0_u32;
                for method in methods {
                    if !budget.charge() {
                        return;
                    }
                    match method {
                        Defn::Fn { params, body, .. } => {
                            self.insert_callable(
                                declaration,
                                &[child],
                                params,
                                body,
                                vec![child],
                                &[child],
                                budget,
                            );
                            child += 1;
                        }
                        Defn::Actor { handlers, .. } => {
                            let Ok(width) = u32::try_from(handlers.len()) else {
                                budget.exhausted = true;
                                return;
                            };
                            let Some(next) = child.checked_add(width) else {
                                budget.exhausted = true;
                                return;
                            };
                            child = next;
                        }
                        Defn::Module { body, .. } => {
                            let Ok(width) = u32::try_from(body.len()) else {
                                budget.exhausted = true;
                                return;
                            };
                            let Some(next) = child.checked_add(width) else {
                                budget.exhausted = true;
                                return;
                            };
                            child = next;
                        }
                    }
                }
            }
            Stmt::TypeDecl(TypeDecl::RuleScope { body, .. }) => {
                for (statement_index, statement) in body.iter().enumerate() {
                    if !budget.charge() {
                        return;
                    }
                    if let Stmt::Defn(Defn::Fn { params, body, .. }) = statement {
                        let Ok(statement_index) = u32::try_from(statement_index) else {
                            budget.exhausted = true;
                            return;
                        };
                        self.insert_callable(
                            declaration,
                            &[statement_index],
                            params,
                            body,
                            vec![statement_index, 0],
                            &[],
                            budget,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    fn insert_callable(
        &mut self,
        declaration: &'a SourcedStmt,
        structural_path: &[u32],
        parameters: &[crate::Param],
        _body: &'a Expr,
        body_path: Vec<u32>,
        binder_prefix: &[u32],
        budget: &mut StructuralIndexBudget,
    ) {
        if parameters.len() > budget.site_limit
            || parameters.len() > u32::MAX as usize
            || !budget.admit_site(self.site_count())
        {
            budget.exhausted = true;
            return;
        }
        let callable = CheckedCallableId {
            declaration: occurrence(declaration),
            structural_path: structural_path.to_vec().into_boxed_slice(),
        };
        let body_site = expression_site(self.program_id, declaration, &body_path);
        let mut parameter_sites = Vec::with_capacity(parameters.len());
        for (index, _) in parameters.iter().enumerate() {
            if !budget.charge() {
                return;
            }
            let Ok(index) = u32::try_from(index) else {
                budget.exhausted = true;
                return;
            };
            let mut binder_path = binder_prefix.to_vec();
            binder_path.extend([BINDER_PARAMETER, index]);
            parameter_sites.push(structural_binder_site(
                self.program_id,
                declaration,
                &body_path,
                binder_path,
            ));
        }
        let parameter_sites = parameter_sites.into_boxed_slice();
        self.callables.insert(
            callable,
            CallableEntry {
                body_site,
                parameter_sites,
            },
        );
    }

    fn expression(&self, site: &ExprSiteId) -> Option<IndexedExpr<'a>> {
        self.expressions.get(site).copied()
    }

    fn descendants<'b>(
        &'b self,
        root: &'b ExprSiteId,
    ) -> impl Iterator<Item = &'b ExprSiteId> + 'b {
        self.descendants_from(root, root.clone())
    }

    fn descendants_from<'b>(
        &'b self,
        root: &'b ExprSiteId,
        start: ExprSiteId,
    ) -> impl Iterator<Item = &'b ExprSiteId> + 'b {
        self.expressions
            .range(start..)
            .map(|(site, _)| site)
            .take_while(move |candidate| {
                candidate.analysis_program == root.analysis_program
                    && candidate.declaration == root.declaration
                    && candidate.normalized_declaration_ordinal
                        == root.normalized_declaration_ordinal
                    && candidate.ast_path.starts_with(&root.ast_path)
            })
    }
}

impl<'a> CheckedProgramSiteIndex<'a> {
    pub(in crate::explore) fn build(
        artifacts: &'a TypeCheckArtifacts,
        limits: ResolvedEventAdapterLimits,
    ) -> Result<Self, ResolvedEventAdapterError> {
        ProgramIndex::build(artifacts, limits).map(|inner| Self { inner })
    }

    pub(in crate::explore) fn expression_slice(
        &self,
        root: &ExprSiteId,
    ) -> Result<CheckedSourceExpressionSlice<'a>, ResolvedEventAdapterError> {
        let indexed = self.inner.expression(root).ok_or_else(|| {
            ResolvedEventAdapterError::InternalArtifactGap(
                "checked expression site has no Phase-A source expression".into(),
            )
        })?;
        let descendants =
            self.inner
                .descendants(root)
                .map(|site| {
                    let indexed = self.inner.expression(site).expect(
                        "ProgramIndex descendants are drawn from the indexed expression map",
                    );
                    CheckedSourceExpression {
                        site: site.clone(),
                        expression: indexed.expression,
                    }
                })
                .collect::<Vec<_>>()
                .into_boxed_slice();
        Ok(CheckedSourceExpressionSlice {
            root: CheckedSourceExpression {
                site: root.clone(),
                expression: indexed.expression,
            },
            descendants,
        })
    }

    pub(in crate::explore) fn callable_slice(
        &self,
        callable: &CheckedCallableId,
    ) -> Result<CheckedCallableSourceSlice<'a>, ResolvedEventAdapterError> {
        let declaration = self
            .inner
            .declarations
            .get(&callable.declaration)
            .copied()
            .ok_or_else(|| {
                ResolvedEventAdapterError::InternalArtifactGap(
                    "checked callable has no Phase-A declaration occurrence".into(),
                )
            })?;
        let entry = self.inner.callables.get(callable).ok_or_else(|| {
            ResolvedEventAdapterError::InternalArtifactGap(
                "checked callable has no Phase-A body".into(),
            )
        })?;
        Ok(CheckedCallableSourceSlice {
            declaration,
            body: self.expression_slice(&entry.body_site)?,
        })
    }
}

fn occurrence(declaration: &SourcedStmt) -> CheckedDeclarationOccurrenceId {
    CheckedDeclarationOccurrenceId {
        declaration: declaration.id.clone(),
        normalized_ordinal: declaration.normalized_ordinal,
    }
}

fn expression_site(
    program: &AnalysisProgramId,
    declaration: &SourcedStmt,
    path: &[u32],
) -> ExprSiteId {
    ExprSiteId {
        analysis_program: program.clone(),
        declaration: declaration.id.clone(),
        normalized_declaration_ordinal: declaration.normalized_ordinal,
        ast_path: path.to_vec().into_boxed_slice(),
    }
}

fn structural_binder_site(
    program: &AnalysisProgramId,
    declaration: &SourcedStmt,
    ast_path: &[u32],
    binder_path: Vec<u32>,
) -> CheckedBinderSiteId {
    CheckedBinderSiteId::Structural {
        analysis_program: program.clone(),
        declaration: declaration.id.clone(),
        normalized_declaration_ordinal: declaration.normalized_ordinal,
        ast_path: ast_path.to_vec().into_boxed_slice(),
        binder_path: binder_path.into_boxed_slice(),
    }
}

fn source_site(site: &ExprSiteId, expression: &Expr) -> SourceSite {
    SourceSite {
        id: SourceSiteId {
            declaration_id: format!(
                "{};occurrence={}",
                site.declaration.semantic_key(),
                site.normalized_declaration_ordinal
            )
            .into_boxed_str(),
            ast_path: site.ast_path.clone(),
        },
        span: expression.span,
    }
}

fn is_lowercase_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

struct QueryRoots {
    all: BTreeSet<ExprSiteId>,
    validity: Vec<ExprSiteId>,
    requested: Vec<ExprSiteId>,
    bound_expression_sites: BTreeMap<usize, ExprSiteId>,
    bound_binder_sites: BTreeMap<usize, CheckedBinderSiteId>,
}

fn query_roots(
    query: &ExploreQueryIr,
    artifact: &CheckedExploreQueryArtifact,
    limits: ResolvedEventAdapterLimits,
) -> Result<QueryRoots, ResolvedEventAdapterError> {
    let sites = &artifact.sites;
    if sites.bounds.len() != query.query.bounds.len()
        || sites.key.len() != query.query.output.key.len()
        || sites.extrema.len() != query.query.output.extrema.len()
        || sites.show.len() != query.query.output.show.len()
        || sites.boundary_step.is_some() != query.query.boundary.is_some()
        || sites.representative_objective.is_some()
            != !matches!(
                &query.query.output.representative,
                crate::ExploreRepresentative::First { .. }
            )
    {
        return Err(ResolvedEventAdapterError::AcceptedQueryArtifactDiverged);
    }
    let root_count = sites
        .bounds
        .len()
        .saturating_add(sites.boundary_step.is_some() as usize)
        .saturating_add(sites.key.len())
        .saturating_add(sites.extrema.len())
        .saturating_add(sites.show.len())
        .saturating_add(sites.representative_objective.is_some() as usize);
    if root_count > limits.max_reachable_sites.get() || root_count > u32::MAX as usize {
        return Err(ResolvedEventAdapterError::ReachabilityLimit {
            limit: limits.max_reachable_sites.get(),
        });
    }
    if root_count > limits.max_abstract_steps.get() {
        return Err(ResolvedEventAdapterError::ReachabilityWorkLimit {
            limit: limits.max_abstract_steps.get(),
        });
    }
    let mut all = BTreeSet::new();
    let mut validity = Vec::new();
    let mut requested = Vec::new();
    let mut bound_expression_sites = BTreeMap::new();
    let mut bound_binder_sites = BTreeMap::new();
    for (bound_index, (bound, checked_sites)) in query
        .query
        .bounds
        .iter()
        .zip(sites.bounds.iter())
        .enumerate()
    {
        all.insert(checked_sites.expression.clone());
        bound_expression_sites.insert(bound_index, checked_sites.expression.clone());
        match bound {
            TypedExploreBound::Domain { .. } | TypedExploreBound::Value { .. } => {
                let binder = checked_sites.binder.clone().ok_or_else(|| {
                    ResolvedEventAdapterError::InternalArtifactGap(
                        "checked Explore value bound has no producer-minted binder site".into(),
                    )
                })?;
                bound_binder_sites.insert(bound_index, binder);
            }
            TypedExploreBound::Where { .. } => {
                if checked_sites.binder.is_some() {
                    return Err(ResolvedEventAdapterError::AcceptedQueryArtifactDiverged);
                }
                validity.push(checked_sites.expression.clone());
            }
        }
    }
    if let Some(site) = &sites.boundary_step {
        all.insert(site.clone());
    }
    for site in sites
        .key
        .iter()
        .chain(sites.extrema.iter())
        .chain(sites.show.iter())
        .chain(sites.representative_objective.iter())
    {
        all.insert(site.clone());
        requested.push(site.clone());
    }

    Ok(QueryRoots {
        all,
        validity,
        requested,
        bound_expression_sites,
        bound_binder_sites,
    })
}

struct ReachabilityBudget {
    work: usize,
    work_limit: usize,
    site_limit: usize,
    candidate_limit: usize,
}

impl ReachabilityBudget {
    fn new(limits: ResolvedEventAdapterLimits) -> Self {
        Self {
            work: 0,
            work_limit: limits.max_abstract_steps.get(),
            site_limit: limits.max_reachable_sites.get(),
            candidate_limit: limits.max_collection_items.get(),
        }
    }

    fn charge(&mut self) -> Result<(), ResolvedEventAdapterError> {
        if self.work >= self.work_limit {
            return Err(ResolvedEventAdapterError::ReachabilityWorkLimit {
                limit: self.work_limit,
            });
        }
        self.work += 1;
        Ok(())
    }
}

fn enforce_rule_candidate_limit(
    candidate_count: usize,
    limit: usize,
) -> Result<(), ResolvedEventAdapterError> {
    if candidate_count > limit {
        Err(ResolvedEventAdapterError::RuleCandidateLimit { limit })
    } else {
        Ok(())
    }
}

fn enqueue_reachable_root(
    site: ExprSiteId,
    pending: &mut Vec<ExprSiteId>,
    scheduled: &mut BTreeSet<ExprSiteId>,
    budget: &mut ReachabilityBudget,
) -> Result<(), ResolvedEventAdapterError> {
    budget.charge()?;
    if scheduled.contains(&site) {
        return Ok(());
    }
    if scheduled.len() >= budget.site_limit {
        return Err(ResolvedEventAdapterError::ReachabilityLimit {
            limit: budget.site_limit,
        });
    }
    scheduled.insert(site.clone());
    pending.push(site);
    Ok(())
}

fn add_expression_tree(
    index: &ProgramIndex<'_>,
    root: &ExprSiteId,
    reachable: &mut BTreeSet<ExprSiteId>,
    budget: &mut ReachabilityBudget,
) -> Result<Vec<ExprSiteId>, ResolvedEventAdapterError> {
    budget.charge()?;
    if index.expressions.contains_key(root) && reachable.contains(root) {
        // A prior ancestor expansion already inserted this exact subtree.
        return Ok(Vec::new());
    }
    let mut added = Vec::new();
    for site in index.descendants(root) {
        budget.charge()?;
        if reachable.contains(site) {
            continue;
        }
        if reachable.len() >= budget.site_limit {
            return Err(ResolvedEventAdapterError::ReachabilityLimit {
                limit: budget.site_limit,
            });
        }
        reachable.insert(site.clone());
        added.push(site.clone());
    }
    if !index.expressions.contains_key(root) {
        // Still include the requested identity so Phase B reports the exact
        // missing-site issue through its canonical issue seam.
        budget.charge()?;
        if !reachable.contains(root) {
            if reachable.len() >= budget.site_limit {
                return Err(ResolvedEventAdapterError::ReachabilityLimit {
                    limit: budget.site_limit,
                });
            }
            reachable.insert(root.clone());
            added.push(root.clone());
        }
    }
    Ok(added)
}

fn top_level_initializer_site(
    index: &ProgramIndex<'_>,
    binding: &CheckedTopLevelBindingId,
) -> Option<ExprSiteId> {
    let declaration = index.declarations.get(&binding.declaration)?;
    matches!(
        &*declaration.statement,
        Stmt::Bind(_, _, _) | Stmt::MonadicBind(_, _, _) | Stmt::StreamBind(_, _)
    )
    .then(|| expression_site(index.program_id, declaration, &[0]))
}

fn add_rule_family_roots(
    resolutions: &crate::CheckedResolutionArtifacts,
    family: &RuleDispatchKey,
    pending: &mut Vec<ExprSiteId>,
    scheduled: &mut BTreeSet<ExprSiteId>,
    expanded_families: &mut BTreeSet<RuleDispatchKey>,
    budget: &mut ReachabilityBudget,
) -> Result<(), ResolvedEventAdapterError> {
    budget.charge()?;
    if !expanded_families.insert(family.clone()) {
        return Ok(());
    }
    let family = resolutions.rule_families.get(family).ok_or_else(|| {
        ResolvedEventAdapterError::InternalArtifactGap(
            "checked call target references a missing rule-family resolution".into(),
        )
    })?;
    // Every family that abstract evaluation can dispatch is expanded through
    // this preparation closure. Reject an oversized family here, before any
    // profile can return events from only a source-order candidate prefix.
    enforce_rule_candidate_limit(family.candidates.len(), budget.candidate_limit)?;
    for candidate in family.candidates.iter() {
        budget.charge()?;
        enqueue_reachable_root(candidate.head_site.clone(), pending, scheduled, budget)?;
        if let Some(site) = &candidate.condition_site {
            enqueue_reachable_root(site.clone(), pending, scheduled, budget)?;
        }
        if let Some(site) = &candidate.value_site {
            enqueue_reachable_root(site.clone(), pending, scheduled, budget)?;
        }
    }
    Ok(())
}

fn checked_reachable_closure(
    artifacts: &TypeCheckArtifacts,
    index: &ProgramIndex<'_>,
    roots: impl IntoIterator<Item = ExprSiteId>,
    question: &RuleDispatchKey,
    limits: ResolvedEventAdapterLimits,
) -> Result<BTreeSet<ExprSiteId>, ResolvedEventAdapterError> {
    let resolutions = &artifacts.checked_resolutions;
    let mut budget = ReachabilityBudget::new(limits);
    let mut pending = Vec::new();
    let mut scheduled = BTreeSet::new();
    let mut expanded_families = BTreeSet::new();
    for root in roots {
        enqueue_reachable_root(root, &mut pending, &mut scheduled, &mut budget)?;
    }
    add_rule_family_roots(
        resolutions,
        question,
        &mut pending,
        &mut scheduled,
        &mut expanded_families,
        &mut budget,
    )?;
    let mut reachable = BTreeSet::new();
    let mut expanded = BTreeSet::new();

    while let Some(root) = pending.pop() {
        budget.charge()?;
        if !expanded.insert(root.clone()) {
            continue;
        }
        let newly_reachable = add_expression_tree(index, &root, &mut reachable, &mut budget)?;
        for site in newly_reachable {
            budget.charge()?;
            if index.expression(&site).is_none() {
                continue;
            }
            let Some(resolution) = resolutions.expressions.get(&site) else {
                continue;
            };
            if let Some(binding) = &resolution.value_binding {
                match binding {
                    CheckedValueBinding::TopLevel(binding) => {
                        let site = top_level_initializer_site(index, binding).ok_or_else(|| {
                            ResolvedEventAdapterError::InternalArtifactGap(
                                "checked top-level value binding has no Phase-A initializer".into(),
                            )
                        })?;
                        enqueue_reachable_root(site, &mut pending, &mut scheduled, &mut budget)?;
                    }
                    CheckedValueBinding::Callable(callable) => {
                        let entry = index.callables.get(callable).ok_or_else(|| {
                            ResolvedEventAdapterError::InternalArtifactGap(
                                "checked first-class callable has no Phase-A body".into(),
                            )
                        })?;
                        enqueue_reachable_root(
                            entry.body_site.clone(),
                            &mut pending,
                            &mut scheduled,
                            &mut budget,
                        )?;
                    }
                    CheckedValueBinding::RuleFamily(family) => {
                        add_rule_family_roots(
                            resolutions,
                            family,
                            &mut pending,
                            &mut scheduled,
                            &mut expanded_families,
                            &mut budget,
                        )?;
                    }
                    CheckedValueBinding::OpaqueQualifiedOwner(_) => {
                        // The Phase-B issue query below is the authority for
                        // the fatal diagnostic.
                    }
                    CheckedValueBinding::Binder { .. }
                    | CheckedValueBinding::Constructor { .. } => {}
                }
            }
            if let Some(target) = &resolution.call_target {
                match target {
                    CheckedCallTarget::Function { callable, .. } => {
                        let entry = index.callables.get(callable).ok_or_else(|| {
                            ResolvedEventAdapterError::InternalArtifactGap(
                                "checked function target has no Phase-A callable body".into(),
                            )
                        })?;
                        enqueue_reachable_root(
                            entry.body_site.clone(),
                            &mut pending,
                            &mut scheduled,
                            &mut budget,
                        )?;
                    }
                    CheckedCallTarget::RuleFamily(family) => {
                        add_rule_family_roots(
                            resolutions,
                            family,
                            &mut pending,
                            &mut scheduled,
                            &mut expanded_families,
                            &mut budget,
                        )?;
                    }
                    CheckedCallTarget::ScopedMember {
                        rule_family: Some(family),
                        ..
                    } => add_rule_family_roots(
                        resolutions,
                        family,
                        &mut pending,
                        &mut scheduled,
                        &mut expanded_families,
                        &mut budget,
                    )?,
                    CheckedCallTarget::Builtin { .. } | CheckedCallTarget::Constructor { .. } => {}
                    CheckedCallTarget::ScopedMember {
                        rule_family: None, ..
                    } => {
                        return Err(ResolvedEventAdapterError::InternalArtifactGap(
                            "reachable ordinary scoped member has no exact Phase-B callable identity"
                                .into(),
                        ));
                    }
                }
            }
            if let Some(CheckedFieldResolution::ScopedMember { rule_family, .. }) =
                &resolution.field
            {
                if let Some(family) = rule_family {
                    add_rule_family_roots(
                        resolutions,
                        family,
                        &mut pending,
                        &mut scheduled,
                        &mut expanded_families,
                        &mut budget,
                    )?;
                } else {
                    return Err(ResolvedEventAdapterError::InternalArtifactGap(
                        "reachable first-class scoped member has no exact Phase-B callable identity"
                            .into(),
                    ));
                }
            }
        }
    }
    Ok(reachable)
}

type CheckedGroundConstructorIndex =
    BTreeMap<CheckedExploreGroundConstructorSite, Arc<CheckedConstructorIdentity>>;

/// Profile-invariant checked preparation for one accepted Explore query.
///
/// Construct this once, then call [`Self::adapt_profile`] for each outer
/// profile. The producer query view, structural index, reachable closure, exact
/// type facts, and ground-constructor identities are all validated and shared.
pub(in crate::explore) struct PreparedResolvedEventAdapter<'a> {
    artifacts: &'a TypeCheckArtifacts,
    artifact: &'a CheckedExploreQueryArtifact,
    query: &'a ExploreQueryIr,
    index: ProgramIndex<'a>,
    roots: QueryRoots,
    reachable_sites: Box<[ExprSiteId]>,
    type_owners: BTreeMap<CheckedExploreTypeUse, CheckedDataTypeId>,
    ground_constructors: CheckedGroundConstructorIndex,
    limits: ResolvedEventAdapterLimits,
}

fn checked_type_owner_index(
    artifact: &CheckedExploreQueryArtifact,
    query: &ExploreQueryIr,
    limits: ResolvedEventAdapterLimits,
) -> Result<BTreeMap<CheckedExploreTypeUse, CheckedDataTypeId>, ResolvedEventAdapterError> {
    let expected = query
        .universe
        .dimensions
        .len()
        .saturating_add(query.universe.facts.len())
        .saturating_add(query.universe.sliced_inputs.len());
    if expected > limits.max_collection_items.get()
        || artifact.type_facts.len() != expected
        || artifact.type_facts.len() > limits.max_abstract_steps.get()
    {
        return Err(
            ResolvedEventAdapterError::OuterProfileMaterializationLimit {
                dimension: "$checked-type-facts".into(),
                limit: limits.max_collection_items.get(),
            },
        );
    }
    let mut owners = BTreeMap::new();
    for fact in artifact.type_facts.iter() {
        if owners
            .insert(fact.use_site.clone(), fact.owner.clone())
            .is_some()
        {
            return Err(ResolvedEventAdapterError::AcceptedQueryArtifactDiverged);
        }
    }
    let complete = (0..query.universe.dimensions.len())
        .all(|index| owners.contains_key(&CheckedExploreTypeUse::Dimension(index)))
        && (0..query.universe.facts.len())
            .all(|index| owners.contains_key(&CheckedExploreTypeUse::Fact(index)))
        && (0..query.universe.sliced_inputs.len())
            .all(|index| owners.contains_key(&CheckedExploreTypeUse::SlicedInput(index)));
    if !complete {
        return Err(ResolvedEventAdapterError::AcceptedQueryArtifactDiverged);
    }
    Ok(owners)
}

fn checked_ground_constructor_index(
    artifact: &CheckedExploreQueryArtifact,
    limits: ResolvedEventAdapterLimits,
) -> Result<CheckedGroundConstructorIndex, ResolvedEventAdapterError> {
    if artifact.ground_constructors.len() > limits.max_reachable_sites.get() {
        return Err(ResolvedEventAdapterError::ReachabilityLimit {
            limit: limits.max_reachable_sites.get(),
        });
    }
    if artifact.ground_constructors.len() > limits.max_abstract_steps.get() {
        return Err(ResolvedEventAdapterError::ReachabilityWorkLimit {
            limit: limits.max_abstract_steps.get(),
        });
    }
    let mut constructors = BTreeMap::new();
    for fact in artifact.ground_constructors.iter() {
        if constructors
            .insert(fact.site.clone(), Arc::clone(&fact.constructor))
            .is_some()
        {
            return Err(ResolvedEventAdapterError::AcceptedQueryArtifactDiverged);
        }
    }
    Ok(constructors)
}

/// Bound compatibility-vector reads before authoritative checked-query access.
/// Shared private request builders may reuse this guard; it performs no
/// selection and does not change checked-query semantics.
pub(in crate::explore) fn preflight_checked_query_access(
    artifacts: &TypeCheckArtifacts,
    accepted_query_index: usize,
    limits: ResolvedEventAdapterLimits,
) -> Result<(), ResolvedEventAdapterError> {
    // These compatibility-vector reads are resource guards only. Identity and
    // selection still come exclusively from checked_exploration_query below.
    let (Some(query), Some(artifact)) = (
        artifacts.exploration_universes.get(accepted_query_index),
        artifacts
            .checked_exploration_queries
            .get(accepted_query_index),
    ) else {
        return Ok(());
    };
    let width = limits.max_collection_items.get();
    let bounded_widths = [
        query.universe.dimensions.len(),
        query.universe.facts.len(),
        query.universe.sliced_inputs.len(),
        query.query.bounds.len(),
        query.query.inputs.len(),
        query.query.output.key.len(),
        query.query.output.extrema.len(),
        query.query.output.show.len(),
        artifact.type_facts.len(),
        artifact.ground_constructors.len(),
        artifact.finite_plan_facts.len(),
    ];
    let total_width = bounded_widths
        .iter()
        .copied()
        .fold(0_usize, usize::saturating_add);
    if bounded_widths.into_iter().any(|count| count > width) {
        return Err(
            ResolvedEventAdapterError::OuterProfileMaterializationLimit {
                dimension: "$checked-query-access".into(),
                limit: width,
            },
        );
    }
    if total_width > limits.max_abstract_steps.get() {
        return Err(ResolvedEventAdapterError::ReachabilityWorkLimit {
            limit: limits.max_abstract_steps.get(),
        });
    }
    let mut budget = FiniteTypeAccessBudget::new(limits);
    for dimension in query.universe.dimensions.iter() {
        budget
            .admit_width(dimension.name.len())
            .map_err(|failure| finite_access_error("$checked-query-access", failure))?;
        match &dimension.domain {
            ExploreExactDomain::Enumerated { values, .. } => {
                budget
                    .admit_width(values.len())
                    .map_err(|failure| finite_access_error("$checked-query-access", failure))?;
                for value in values {
                    ensure_explore_value_bounded(value, &mut budget)
                        .map_err(|failure| finite_access_error("$checked-query-access", failure))?;
                }
            }
            ExploreExactDomain::FiniteType { plan, .. } => {
                preflight_finite_plan(plan, &mut budget)
                    .map_err(|failure| finite_access_error("$checked-query-access", failure))?;
            }
            ExploreExactDomain::IntRange { .. } => {
                budget
                    .charge(0)
                    .map_err(|failure| finite_access_error("$checked-query-access", failure))?;
            }
        }
    }
    for fact in query.universe.facts.iter() {
        budget
            .admit_width(fact.name.len())
            .map_err(|failure| finite_access_error("$checked-query-access", failure))?;
        if let ExploreFactValue::Fixed(value) = &fact.value {
            ensure_explore_value_bounded(value, &mut budget)
                .map_err(|failure| finite_access_error("$checked-query-access", failure))?;
        }
    }
    Ok(())
}

impl<'a> PreparedResolvedEventAdapter<'a> {
    pub(in crate::explore) fn prepare(
        artifacts: &'a TypeCheckArtifacts,
        accepted_query_index: usize,
        limits: ResolvedEventAdapterLimits,
    ) -> Result<Self, ResolvedEventAdapterError> {
        if !artifacts.diagnostics.is_empty() {
            return Err(ResolvedEventAdapterError::OrdinaryCheckerRejected {
                diagnostics: artifacts.diagnostics.len(),
            });
        }
        if artifacts.analysis_program.declarations.len() > limits.max_reachable_sites.get()
            || artifacts.analysis_program.declarations.len() > limits.max_abstract_steps.get()
        {
            return Err(ResolvedEventAdapterError::StructuralIndexLimit {
                limit: limits
                    .max_reachable_sites
                    .get()
                    .min(limits.max_abstract_steps.get()),
            });
        }

        if artifacts.analysis_program.id != artifacts.checked_resolutions.analysis_program {
            return Err(ResolvedEventAdapterError::AnalysisProgramIdentityMismatch);
        }

        // Bound the full source snapshot before the exact accessor re-hashes
        // its declaration. This index is retained for every adapted profile.
        let index = ProgramIndex::build(artifacts, limits)?;
        preflight_checked_query_access(artifacts, accepted_query_index, limits)?;

        // This is the sole checked-query accessor call for the prepared query.
        let checked = artifacts
            .checked_exploration_query(accepted_query_index)
            .map_err(ResolvedEventAdapterError::CheckedQueryAccess)?;
        let artifact = checked.artifact;
        let query = checked.closed_query;
        if !is_lowercase_sha256(&artifact.identity.digest) {
            return Err(ResolvedEventAdapterError::AcceptedQueryArtifactDiverged);
        }
        let boundary = query
            .universe
            .boundary
            .as_ref()
            .ok_or(ResolvedEventAdapterError::QueryHasNoBoundary)?;
        let dimension = query
            .universe
            .dimensions
            .get(boundary.axis_dimension_index)
            .ok_or(ResolvedEventAdapterError::InvalidBoundaryAxis)?;
        if dimension.name != boundary.axis || boundary.step <= 0 {
            return Err(ResolvedEventAdapterError::InvalidBoundaryAxis);
        }

        let roots = query_roots(query, artifact, limits)?;
        if !index
            .declarations
            .contains_key(&artifact.identity.declaration)
        {
            return Err(ResolvedEventAdapterError::AcceptedQueryArtifactDiverged);
        }
        let reachable = checked_reachable_closure(
            artifacts,
            &index,
            roots.all.iter().cloned(),
            &artifact.question,
            limits,
        )?;
        let issues = artifacts
            .checked_resolutions
            .issues_for_reachable_sites(reachable.iter());
        if !issues.is_empty() {
            return Err(ResolvedEventAdapterError::FatalResolutionIssues(
                issues.into_iter().collect::<Vec<_>>().into_boxed_slice(),
            ));
        }
        let type_owners = checked_type_owner_index(artifact, query, limits)?;
        let ground_constructors = checked_ground_constructor_index(artifact, limits)?;
        Ok(Self {
            artifacts,
            artifact,
            query,
            index,
            roots,
            reachable_sites: reachable.into_iter().collect::<Vec<_>>().into_boxed_slice(),
            type_owners,
            ground_constructors,
            limits,
        })
    }

    pub(in crate::explore) fn analysis_program_hash(&self) -> &str {
        self.artifact.identity.analysis_program.as_str()
    }

    pub(in crate::explore) fn query_hash(&self) -> &str {
        self.artifact.identity.digest.as_ref()
    }

    /// The exact checked query whose producer-owned identities are exposed by
    /// this preparation. Proof consumers take the query through this accessor
    /// instead of accepting an independently supplied, lookalike IR value.
    pub(in crate::explore) fn checked_query(&self) -> &ExploreQueryIr {
        self.query
    }

    /// Adapt one exact outer profile using the profile-invariant checked
    /// preparation. Caller-provided identities are accepted only when they
    /// equal the identities minted by the producer artifact.
    pub(in crate::explore) fn adapt_profile(
        &self,
        analysis_program_hash: &str,
        query_hash: &str,
        outer_ordinals: &[u128],
    ) -> Result<AdaptedBoundaryFragment, ResolvedEventAdapterError> {
        if analysis_program_hash != self.artifact.identity.analysis_program.as_str() {
            return Err(ResolvedEventAdapterError::AnalysisProgramHashMismatch);
        }
        if query_hash != self.artifact.identity.digest.as_ref() {
            return Err(ResolvedEventAdapterError::InvalidQueryHash);
        }

        let mut context = AdapterContext::new(self, outer_ordinals)?;
        let (env, values) = query_profile_env(&mut context, self.query, &self.roots)?;

        for root in &self.roots.validity {
            let value = context.with_role(BoundaryFragmentRootRole::Validity, |context| {
                context.eval_site(root, &env)
            });
            context.mark_quantizers_uncertified(&value);
        }
        for root in &self.roots.requested {
            let value = context.with_role(BoundaryFragmentRootRole::RequestedValue, |context| {
                context.eval_site(root, &env)
            });
            context.mark_quantizers_uncertified(&value);
        }

        let arguments = self
            .query
            .query
            .inputs
            .iter()
            .map(|input| {
                values.get(&input.name).cloned().ok_or_else(|| {
                    ResolvedEventAdapterError::InternalArtifactGap(
                        "accepted query input is absent from its closed bound universe".into(),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let question_site = self
            .artifact
            .sites
            .boundary_step
            .as_ref()
            .or_else(|| self.roots.all.iter().next())
            .cloned()
            .ok_or_else(|| {
                ResolvedEventAdapterError::InternalArtifactGap(
                    "accepted boundary query has no producer-minted expression roots".into(),
                )
            })?;
        let question_expression = context
            .index
            .expression(&question_site)
            .ok_or_else(|| {
                ResolvedEventAdapterError::InternalArtifactGap(
                    "producer-minted boundary-query root is absent from Phase A".into(),
                )
            })?
            .expression
            .clone();
        let question_value = context.with_role(BoundaryFragmentRootRole::Question, |context| {
            context.eval_rule_family(
                &self.artifact.question,
                arguments,
                &AbstractEnv::new(),
                &question_site,
                &question_expression,
            )
        });
        context.mark_quantizers_uncertified(&question_value);
        let classification = resolved_classification_formula(
            &mut context,
            &question_value,
            &question_site,
            &question_expression,
        );

        Ok(AdaptedBoundaryFragment {
            fragment: finalize_fragment(context, classification),
            reachable_sites: self.reachable_sites.clone(),
        })
    }
}

type AbstractEnv = BTreeMap<CheckedBinderSiteId, AbstractValue>;

#[derive(Debug, Clone)]
enum AbstractValue {
    Ground(ExploreValue),
    Int(IntTerm),
    Predicate(PredicateTerm),
    Constructor(AbstractConstructor),
    List(Vec<AbstractValue>),
    Set(Vec<AbstractValue>),
    Tuple(Vec<AbstractValue>),
    Callable(AbstractCallable),
    Unsupported(UnsupportedResidual),
}

#[derive(Debug, Clone)]
struct AbstractConstructor {
    identity: Option<Arc<CheckedConstructorIdentity>>,
    fields: Vec<AbstractValue>,
}

#[derive(Debug, Clone)]
enum AbstractCallable {
    Lambda {
        body_site: ExprSiteId,
        parameters: Box<[CheckedBinderSiteId]>,
        captured: AbstractEnv,
    },
    Function(CheckedCallableId),
    RuleFamily(RuleDispatchKey),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct QuantizerKey {
    source: SourceSiteId,
    numerator: AffineForm,
    divisor: i64,
    nonnegative_numerator: bool,
}

#[derive(Debug, Clone)]
struct SymbolicLinear {
    affine: AffineForm,
    quantizers: BTreeMap<QuantizerKey, i128>,
}

#[derive(Debug, Clone)]
enum IntTerm {
    Linear(SymbolicLinear),
    PositiveAffine {
        affine: AffineForm,
        source: SourceSite,
    },
    Unsupported(UnsupportedResidual),
}

#[derive(Debug, Clone)]
enum PredicateTerm {
    Constant(bool),
    Comparison {
        difference: AffineForm,
        relation: BoundaryRelation,
        source: SourceSite,
    },
    LinearComparison {
        difference: SymbolicLinear,
        relation: BoundaryRelation,
        source: SourceSite,
    },
    Not(Box<PredicateTerm>),
    All(Vec<PredicateTerm>),
    Any(Vec<PredicateTerm>),
    Unsupported(UnsupportedResidual),
}

impl PredicateTerm {
    fn constant(&self) -> Option<bool> {
        match self {
            Self::Constant(value) => Some(*value),
            Self::Not(inner) => inner.constant().map(|value| !value),
            Self::All(parts) => parts
                .iter()
                .map(Self::constant)
                .collect::<Option<Vec<_>>>()
                .map(|parts| parts.into_iter().all(|value| value)),
            Self::Any(parts) => parts
                .iter()
                .map(Self::constant)
                .collect::<Option<Vec<_>>>()
                .map(|parts| parts.into_iter().any(|value| value)),
            Self::Comparison { .. } | Self::LinearComparison { .. } | Self::Unsupported(_) => None,
        }
    }

    fn boundary(&self) -> BoundaryPredicate {
        match self {
            Self::Constant(value) => BoundaryPredicate::Constant(*value),
            Self::Comparison {
                difference,
                relation,
                source,
            } => BoundaryPredicate::Comparison {
                difference: *difference,
                relation: *relation,
                source: source.clone(),
            },
            Self::LinearComparison { source, .. } => BoundaryPredicate::Unsupported(
                UnsupportedResidual {
                    source: Some(source.clone()),
                    kind: UnsupportedResidualKind::NonAffineArithmetic,
                    detail: "predicate over a quantized integer is retained only for liveness normalization"
                        .into(),
                },
            ),
            Self::Not(inner) => BoundaryPredicate::Not(Box::new(inner.boundary())),
            Self::All(parts) => BoundaryPredicate::All(
                parts
                    .iter()
                    .map(Self::boundary)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            ),
            Self::Any(parts) => BoundaryPredicate::Any(
                parts
                    .iter()
                    .map(Self::boundary)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            ),
            Self::Unsupported(residual) => BoundaryPredicate::Unsupported(residual.clone()),
        }
    }

    fn classification_formula(&self) -> ResolvedClassificationFormula {
        match self {
            Self::Constant(value) => ResolvedClassificationFormula::Constant(*value),
            Self::Comparison {
                difference,
                relation,
                source,
            } => ResolvedClassificationFormula::Comparison {
                difference: ResolvedQuasiAffineForm {
                    affine: *difference,
                    quantized_terms: Box::new([]),
                },
                relation: *relation,
                source: source.clone(),
            },
            Self::LinearComparison {
                difference,
                relation,
                source,
            } => ResolvedClassificationFormula::Comparison {
                difference: difference.resolved(),
                relation: *relation,
                source: source.clone(),
            },
            Self::Not(inner) => {
                ResolvedClassificationFormula::Not(Box::new(inner.classification_formula()))
            }
            Self::All(parts) => ResolvedClassificationFormula::All(
                parts
                    .iter()
                    .map(Self::classification_formula)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            ),
            Self::Any(parts) => ResolvedClassificationFormula::Any(
                parts
                    .iter()
                    .map(Self::classification_formula)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            ),
            Self::Unsupported(residual) => {
                ResolvedClassificationFormula::Unsupported(residual.clone())
            }
        }
    }
}

impl SymbolicLinear {
    fn affine(form: AffineForm) -> Self {
        Self {
            affine: form,
            quantizers: BTreeMap::new(),
        }
    }

    fn constant(value: i128) -> Self {
        Self::affine(AffineForm::new(0, value))
    }

    fn checked_add(&self, other: &Self) -> Option<Self> {
        let affine = AffineForm::new(
            self.affine
                .coefficient
                .checked_add(other.affine.coefficient)?,
            self.affine.intercept.checked_add(other.affine.intercept)?,
        );
        let mut quantizers = self.quantizers.clone();
        for (key, coefficient) in &other.quantizers {
            let value = quantizers
                .get(key)
                .copied()
                .unwrap_or(0)
                .checked_add(*coefficient)?;
            if value == 0 {
                quantizers.remove(key);
            } else {
                quantizers.insert(key.clone(), value);
            }
        }
        Some(Self { affine, quantizers })
    }

    fn checked_scale(&self, scale: i128) -> Option<Self> {
        let affine = AffineForm::new(
            self.affine.coefficient.checked_mul(scale)?,
            self.affine.intercept.checked_mul(scale)?,
        );
        let mut quantizers = BTreeMap::new();
        for (key, coefficient) in &self.quantizers {
            let coefficient = coefficient.checked_mul(scale)?;
            if coefficient != 0 {
                quantizers.insert(key.clone(), coefficient);
            }
        }
        Some(Self { affine, quantizers })
    }

    fn plain_affine(&self) -> Option<AffineForm> {
        self.quantizers.is_empty().then_some(self.affine)
    }

    fn ground(&self) -> Option<i128> {
        (self.quantizers.is_empty() && self.affine.coefficient == 0)
            .then_some(self.affine.intercept)
    }

    fn resolved(&self) -> ResolvedQuasiAffineForm {
        ResolvedQuasiAffineForm {
            affine: self.affine,
            quantized_terms: self
                .quantizers
                .iter()
                .map(|(key, coefficient)| ResolvedQuantizedTerm {
                    source: key.source.clone(),
                    numerator: key.numerator,
                    positive_divisor: key.divisor,
                    nonnegative_numerator: key.nonnegative_numerator,
                    coefficient: *coefficient,
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        }
    }
}

impl IntTerm {
    fn linear(form: AffineForm) -> Self {
        Self::Linear(SymbolicLinear::affine(form))
    }

    fn constant(value: i128) -> Self {
        Self::Linear(SymbolicLinear::constant(value))
    }

    fn plain_affine(&self) -> Option<AffineForm> {
        match self {
            Self::Linear(linear) => linear.plain_affine(),
            Self::PositiveAffine { .. } | Self::Unsupported(_) => None,
        }
    }

    fn ground(&self) -> Option<i128> {
        match self {
            Self::Linear(linear) => linear.ground(),
            Self::PositiveAffine { affine, .. } if affine.coefficient == 0 => {
                Some(affine.intercept.max(0))
            }
            Self::PositiveAffine { .. } | Self::Unsupported(_) => None,
        }
    }

    fn quantizers(&self) -> BTreeSet<QuantizerKey> {
        match self {
            Self::Linear(linear) => linear.quantizers.keys().cloned().collect(),
            Self::PositiveAffine { .. } | Self::Unsupported(_) => BTreeSet::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct QuantizerObservation {
    key: QuantizerKey,
    source: SourceSite,
    roles: BTreeSet<BoundaryFragmentRootRole>,
    /// First quantizer cell at which each proven positive clamp is dead.
    cutoff_cells: BTreeSet<i128>,
    uncertified_use: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum RootSpecialization {
    Comparison {
        difference: AffineForm,
        relation: BoundaryRelation,
    },
    Minimum {
        left_minus_right: AffineForm,
        tie_arm: TieArm,
    },
    Maximum {
        left_minus_right: AffineForm,
        tie_arm: TieArm,
    },
    TruncDivision {
        numerator: AffineForm,
        divisor: i64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RootKey {
    role: BoundaryFragmentRootRole,
    site: SourceSiteId,
    specialization: RootSpecialization,
}

struct AdapterContext<'prepared, 'artifacts> {
    artifacts: &'artifacts TypeCheckArtifacts,
    index: &'prepared ProgramIndex<'artifacts>,
    type_owners: &'prepared BTreeMap<CheckedExploreTypeUse, CheckedDataTypeId>,
    ground_constructors: &'prepared CheckedGroundConstructorIndex,
    axis_name: Box<str>,
    step: i64,
    axis_min: i128,
    axis_max: i128,
    analysis_program_hash: &'prepared str,
    query_hash: &'prepared str,
    outer_ordinals: &'prepared [u128],
    limits: ResolvedEventAdapterLimits,
    steps: usize,
    call_stack: Vec<Box<str>>,
    role: BoundaryFragmentRootRole,
    roots: BTreeMap<RootKey, ResolvedBoundaryRoot>,
    quantizers: BTreeMap<QuantizerKey, QuantizerObservation>,
    residuals:
        BTreeMap<(Option<SourceSiteId>, UnsupportedResidualKind, Box<str>), UnsupportedResidual>,
    top_level_cache: BTreeMap<(CheckedTopLevelBindingId, BoundaryFragmentRootRole), AbstractValue>,
    active_rejected_lambda_bodies: BTreeSet<(BoundaryFragmentRootRole, ExprSiteId)>,
    all_quantizers_uncertified: bool,
}

impl<'prepared, 'artifacts> AdapterContext<'prepared, 'artifacts> {
    fn new(
        prepared: &'prepared PreparedResolvedEventAdapter<'artifacts>,
        outer_ordinals: &'prepared [u128],
    ) -> Result<Self, ResolvedEventAdapterError> {
        let query = prepared.query;
        let limits = prepared.limits;
        let boundary = query
            .universe
            .boundary
            .as_ref()
            .ok_or(ResolvedEventAdapterError::QueryHasNoBoundary)?;
        let dimension = query
            .universe
            .dimensions
            .get(boundary.axis_dimension_index)
            .ok_or(ResolvedEventAdapterError::InvalidBoundaryAxis)?;
        if matches!(
            &dimension.domain,
            ExploreExactDomain::Enumerated { values, .. }
                if values.len() > limits.max_collection_items.get()
        ) {
            return Err(ResolvedEventAdapterError::OuterProfileAccessLimit {
                dimension: dimension.name.clone().into_boxed_str(),
                resource: "axis-hull-width",
                limit: limits.max_collection_items.get(),
            });
        }
        let (axis_min, axis_max) = axis_endpoint_hull(&dimension.domain)
            .ok_or(ResolvedEventAdapterError::InvalidBoundaryAxis)?;
        Ok(Self {
            artifacts: prepared.artifacts,
            index: &prepared.index,
            type_owners: &prepared.type_owners,
            ground_constructors: &prepared.ground_constructors,
            axis_name: boundary.axis.clone().into_boxed_str(),
            step: boundary.step,
            axis_min,
            axis_max,
            analysis_program_hash: prepared.artifact.identity.analysis_program.as_str(),
            query_hash: &prepared.artifact.identity.digest,
            outer_ordinals,
            limits,
            steps: 0,
            call_stack: Vec::new(),
            role: BoundaryFragmentRootRole::Question,
            roots: BTreeMap::new(),
            quantizers: BTreeMap::new(),
            residuals: BTreeMap::new(),
            top_level_cache: BTreeMap::new(),
            active_rejected_lambda_bodies: BTreeSet::new(),
            all_quantizers_uncertified: false,
        })
    }

    fn charge(&mut self, source: Option<SourceSite>) -> bool {
        self.steps = self.steps.saturating_add(1);
        if self.steps <= self.limits.max_abstract_steps.get() {
            return true;
        }
        self.residual(
            source,
            UnsupportedResidualKind::AdapterIncomplete,
            format!(
                "abstract adapter exceeded {} evaluation steps",
                self.limits.max_abstract_steps
            ),
        );
        false
    }

    fn residual(
        &mut self,
        source: Option<SourceSite>,
        kind: UnsupportedResidualKind,
        detail: impl Into<Box<str>>,
    ) -> UnsupportedResidual {
        let residual = UnsupportedResidual {
            source,
            kind,
            detail: detail.into(),
        };
        if self.residuals.len() < self.limits.max_residuals.get() {
            let key = (
                residual.source.as_ref().map(|source| source.id.clone()),
                residual.kind.clone(),
                residual.detail.clone(),
            );
            self.residuals
                .entry(key)
                .or_insert_with(|| residual.clone());
        }
        residual
    }

    fn unsupported(
        &mut self,
        site: Option<(&ExprSiteId, &Expr)>,
        kind: UnsupportedResidualKind,
        detail: impl Into<Box<str>>,
    ) -> AbstractValue {
        let source = site.map(|(site, expression)| source_site(site, expression));
        AbstractValue::Unsupported(self.residual(source, kind, detail))
    }

    fn has_evaluation_budget(&self) -> bool {
        self.steps < self.limits.max_abstract_steps.get()
    }

    fn preserve_direct_child_roots(
        &mut self,
        site: &ExprSiteId,
        expression: &Expr,
        env: &AbstractEnv,
    ) {
        let limit = self.limits.max_collection_items.get();
        let (child_count, omitted) = bounded_expression_child_count(expression, limit);
        for child_index in 0..child_count {
            if !self.has_evaluation_budget() {
                break;
            }
            let child_site = self.child_site(site, child_index);
            let value = self.eval_site(&child_site, env);
            // The unsupported parent may consume this child in an unknown way,
            // so a quantizer descendant remains extractable but cannot keep a
            // narrowed liveness certificate.
            self.mark_quantizers_uncertified(&value);
        }
        if omitted {
            let first_omitted = self.child_site(site, child_count);
            self.preserve_indexed_descendant_roots_from(
                site,
                first_omitted,
                env,
                source_site(site, expression),
            );
            self.residual(
                Some(source_site(site, expression)),
                UnsupportedResidualKind::AdapterIncomplete,
                "supported child-root preservation reached the bounded child limit",
            );
        }
    }

    fn preserve_indexed_descendant_roots(
        &mut self,
        root: &ExprSiteId,
        env: &AbstractEnv,
        residual_source: SourceSite,
    ) {
        self.preserve_indexed_descendant_roots_from(root, root.clone(), env, residual_source);
    }

    fn preserve_indexed_descendant_roots_from(
        &mut self,
        root: &ExprSiteId,
        start: ExprSiteId,
        env: &AbstractEnv,
        residual_source: SourceSite,
    ) {
        let limit = self.limits.max_collection_items.get();
        let mut sites = self
            .index
            .descendants_from(root, start)
            .take(limit.saturating_add(1))
            .cloned()
            .collect::<Vec<_>>();
        let omitted = sites.len() > limit;
        sites.truncate(limit);
        let mut stopped_for_budget = false;
        for site in sites {
            if !self.has_evaluation_budget() {
                stopped_for_budget = true;
                break;
            }
            let value = self.eval_site(&site, env);
            self.mark_quantizers_uncertified(&value);
        }
        if omitted {
            self.residual(
                Some(residual_source.clone()),
                UnsupportedResidualKind::AdapterIncomplete,
                "supported descendant-root preservation reached the bounded subtree limit",
            );
        }
        if stopped_for_budget {
            self.residual(
                Some(residual_source),
                UnsupportedResidualKind::AdapterIncomplete,
                "supported descendant-root preservation exhausted the abstract work budget",
            );
        }
    }

    /// Preserve supported descendants when a real lambda value is abandoned.
    /// Captures are exact, but parameters deliberately remain unbound: this
    /// may recover parameter-independent roots while parameter-dependent work
    /// emits a residual instead of being evaluated under an invented value.
    fn preserve_rejected_lambda_body(&mut self, body_site: &ExprSiteId, captured: &AbstractEnv) {
        let Some(indexed) = self.index.expression(body_site) else {
            self.residual(
                None,
                UnsupportedResidualKind::AdapterIncomplete,
                "rejected lambda body is absent from the checked structural index",
            );
            return;
        };
        let source = source_site(body_site, indexed.expression);
        let key = (self.role, body_site.clone());
        if self.active_rejected_lambda_bodies.len() >= self.limits.max_call_depth.get() {
            self.residual(
                Some(source),
                UnsupportedResidualKind::AdapterIncomplete,
                "rejected lambda-body preservation reached the bounded call depth",
            );
            return;
        }
        if !self.active_rejected_lambda_bodies.insert(key.clone()) {
            self.residual(
                Some(source),
                UnsupportedResidualKind::AdapterIncomplete,
                "recursive rejected lambda-body preservation remained open",
            );
            return;
        }
        self.preserve_indexed_descendant_roots(body_site, captured, source);
        self.active_rejected_lambda_bodies.remove(&key);
    }

    fn admit_abandoned_value_node(&mut self, depth: usize, remaining: &mut usize) -> bool {
        if depth >= self.limits.max_call_depth.get()
            || *remaining == 0
            || !self.has_evaluation_budget()
        {
            return false;
        }
        *remaining -= 1;
        self.charge(None)
    }

    fn collect_abandoned_predicate_quantizers(
        &mut self,
        predicate: &PredicateTerm,
        depth: usize,
        remaining: &mut usize,
        result: &mut BTreeSet<QuantizerKey>,
    ) -> bool {
        if !self.admit_abandoned_value_node(depth, remaining) {
            return false;
        }
        match predicate {
            PredicateTerm::LinearComparison { difference, .. } => {
                for key in difference.quantizers.keys() {
                    if !self.admit_abandoned_value_node(depth.saturating_add(1), remaining) {
                        return false;
                    }
                    result.insert(key.clone());
                }
                true
            }
            PredicateTerm::Not(inner) => self.collect_abandoned_predicate_quantizers(
                inner,
                depth.saturating_add(1),
                remaining,
                result,
            ),
            PredicateTerm::All(parts) | PredicateTerm::Any(parts) => {
                for part in parts {
                    if !self.collect_abandoned_predicate_quantizers(
                        part,
                        depth.saturating_add(1),
                        remaining,
                        result,
                    ) {
                        return false;
                    }
                }
                true
            }
            PredicateTerm::Constant(_)
            | PredicateTerm::Comparison { .. }
            | PredicateTerm::Unsupported(_) => true,
        }
    }

    fn collect_abandoned_value_quantizers(
        &mut self,
        value: &AbstractValue,
        depth: usize,
        remaining: &mut usize,
        result: &mut BTreeSet<QuantizerKey>,
    ) -> bool {
        if !self.admit_abandoned_value_node(depth, remaining) {
            return false;
        }
        match value {
            AbstractValue::Int(IntTerm::Linear(linear)) => {
                for key in linear.quantizers.keys() {
                    if !self.admit_abandoned_value_node(depth.saturating_add(1), remaining) {
                        return false;
                    }
                    result.insert(key.clone());
                }
                true
            }
            AbstractValue::Predicate(predicate) => self.collect_abandoned_predicate_quantizers(
                predicate,
                depth.saturating_add(1),
                remaining,
                result,
            ),
            AbstractValue::Callable(AbstractCallable::Lambda {
                body_site,
                captured,
                ..
            }) => {
                for value in captured.values() {
                    if !self.collect_abandoned_value_quantizers(
                        value,
                        depth.saturating_add(1),
                        remaining,
                        result,
                    ) {
                        return false;
                    }
                }
                self.preserve_rejected_lambda_body(body_site, captured);
                true
            }
            AbstractValue::Constructor(constructor) => {
                for field in &constructor.fields {
                    if !self.collect_abandoned_value_quantizers(
                        field,
                        depth.saturating_add(1),
                        remaining,
                        result,
                    ) {
                        return false;
                    }
                }
                true
            }
            AbstractValue::List(values)
            | AbstractValue::Set(values)
            | AbstractValue::Tuple(values) => {
                for value in values {
                    if !self.collect_abandoned_value_quantizers(
                        value,
                        depth.saturating_add(1),
                        remaining,
                        result,
                    ) {
                        return false;
                    }
                }
                true
            }
            AbstractValue::Ground(_)
            | AbstractValue::Int(IntTerm::PositiveAffine { .. })
            | AbstractValue::Int(IntTerm::Unsupported(_))
            | AbstractValue::Callable(AbstractCallable::Function(_))
            | AbstractValue::Callable(AbstractCallable::RuleFamily(_))
            | AbstractValue::Unsupported(_) => true,
        }
    }

    fn unsupported_preserving_children(
        &mut self,
        site: &ExprSiteId,
        expression: &Expr,
        env: &AbstractEnv,
        kind: UnsupportedResidualKind,
        detail: impl Into<Box<str>>,
    ) -> AbstractValue {
        self.preserve_direct_child_roots(site, expression, env);
        self.unsupported(Some((site, expression)), kind, detail)
    }

    fn resolution(&self, site: &ExprSiteId) -> Option<&CheckedExpressionResolution> {
        self.artifacts.checked_resolutions.expressions.get(site)
    }

    fn insert_root(&mut self, key: RootKey, root: ResolvedBoundaryRoot) {
        if let Some(existing) = self.roots.get_mut(&key) {
            if existing.active_support != root.active_support {
                // Identical normalized event nodes reached through different
                // proof contexts share one canonical root.  A disagreement in
                // support is merged conservatively instead of making map
                // insertion order part of extraction semantics.
                existing.active_support = ResolvedAxisSupport::Everywhere;
            }
            return;
        }
        if self.roots.len() >= self.limits.max_reachable_sites.get() {
            self.residual(
                None,
                UnsupportedResidualKind::AdapterIncomplete,
                "normalized root set exceeded the bounded reachable-site limit",
            );
            return;
        }
        self.roots.insert(key, root);
    }

    fn add_predicate_root(&mut self, predicate: PredicateTerm) {
        let (site, specialization) = match &predicate {
            PredicateTerm::Comparison {
                source,
                difference,
                relation,
            } => (
                source.id.clone(),
                RootSpecialization::Comparison {
                    difference: *difference,
                    relation: *relation,
                },
            ),
            _ => return,
        };
        let key = RootKey {
            role: self.role,
            site,
            specialization,
        };
        self.insert_root(
            key,
            ResolvedBoundaryRoot {
                role: self.role,
                guards: vec![SourceGuard::ReachableFrom { role: self.role }].into_boxed_slice(),
                active_support: ResolvedAxisSupport::Everywhere,
                node: ResolvedBoundaryNode::Predicate(predicate.boundary()),
            },
        );
    }

    fn add_int_root(&mut self, site: &SourceSite, expression: BoundaryIntExpr) {
        let specialization = match &expression {
            BoundaryIntExpr::Min {
                left_minus_right,
                tie_arm,
                ..
            } => RootSpecialization::Minimum {
                left_minus_right: *left_minus_right,
                tie_arm: *tie_arm,
            },
            BoundaryIntExpr::Max {
                left_minus_right,
                tie_arm,
                ..
            } => RootSpecialization::Maximum {
                left_minus_right: *left_minus_right,
                tie_arm: *tie_arm,
            },
            _ => return,
        };
        let key = RootKey {
            role: self.role,
            site: site.id.clone(),
            specialization,
        };
        self.insert_root(
            key,
            ResolvedBoundaryRoot {
                role: self.role,
                guards: vec![SourceGuard::ReachableFrom { role: self.role }].into_boxed_slice(),
                active_support: ResolvedAxisSupport::Everywhere,
                node: ResolvedBoundaryNode::Int(expression),
            },
        );
    }

    fn observe_quantizer(
        &mut self,
        site: SourceSite,
        numerator: AffineForm,
        divisor: i64,
        nonnegative_numerator: bool,
    ) -> QuantizerKey {
        let key = QuantizerKey {
            source: site.id.clone(),
            numerator,
            divisor,
            nonnegative_numerator,
        };
        if !self.quantizers.contains_key(&key)
            && self.quantizers.len() >= self.limits.max_reachable_sites.get()
        {
            self.residual(
                Some(site),
                UnsupportedResidualKind::AdapterIncomplete,
                "quantizer observation set exceeded the bounded reachable-site limit",
            );
            return key;
        }
        let globally_uncertified = self.all_quantizers_uncertified;
        self.quantizers
            .entry(key.clone())
            .or_insert_with(|| QuantizerObservation {
                key: key.clone(),
                source: site,
                roles: BTreeSet::new(),
                cutoff_cells: BTreeSet::new(),
                uncertified_use: globally_uncertified,
            })
            .roles
            .insert(self.role);
        key
    }

    fn affine_fits_runtime_int(&self, affine: AffineForm) -> bool {
        let evaluate = |axis: i128| {
            affine
                .coefficient
                .checked_mul(axis)
                .and_then(|value| value.checked_add(affine.intercept))
        };
        [self.axis_min, self.axis_max]
            .into_iter()
            .filter_map(evaluate)
            .all(|value| i64::try_from(value).is_ok())
            && evaluate(self.axis_min).is_some()
            && evaluate(self.axis_max).is_some()
    }

    fn linear_fits_runtime_int(&self, linear: &SymbolicLinear) -> bool {
        let mut bounds = [linear.affine.intercept, linear.affine.intercept];
        let axis_contribution = [self.axis_min, self.axis_max]
            .into_iter()
            .map(|axis| linear.affine.coefficient.checked_mul(axis))
            .collect::<Option<Vec<_>>>();
        let Some(axis_contribution) = axis_contribution else {
            return false;
        };
        let Some(axis_low) = axis_contribution.iter().min().copied() else {
            return false;
        };
        let Some(axis_high) = axis_contribution.iter().max().copied() else {
            return false;
        };
        bounds[0] = match bounds[0].checked_add(axis_low) {
            Some(value) => value,
            None => return false,
        };
        bounds[1] = match bounds[1].checked_add(axis_high) {
            Some(value) => value,
            None => return false,
        };
        for (key, coefficient) in &linear.quantizers {
            let q = |axis: i128| -> Option<i128> {
                let raw = key
                    .numerator
                    .coefficient
                    .checked_mul(axis)?
                    .checked_add(key.numerator.intercept)?;
                let numerator = if key.nonnegative_numerator {
                    raw.max(0)
                } else {
                    raw
                };
                numerator.checked_div(i128::from(key.divisor))
            };
            let values = [q(self.axis_min), q(self.axis_max)];
            let (Some(left), Some(right)) = (values[0], values[1]) else {
                return false;
            };
            let contributions = [left, right]
                .into_iter()
                .map(|value| value.checked_mul(*coefficient))
                .collect::<Option<Vec<_>>>();
            let Some(contributions) = contributions else {
                return false;
            };
            let low = *contributions.iter().min().unwrap();
            let high = *contributions.iter().max().unwrap();
            bounds[0] = match bounds[0].checked_add(low) {
                Some(value) => value,
                None => return false,
            };
            bounds[1] = match bounds[1].checked_add(high) {
                Some(value) => value,
                None => return false,
            };
        }
        bounds.into_iter().all(|value| i64::try_from(value).is_ok())
    }

    fn mark_abandoned_values_uncertified<'value>(
        &mut self,
        values: impl IntoIterator<Item = &'value AbstractValue>,
    ) {
        let mut keys = BTreeSet::new();
        let mut remaining = self.limits.max_collection_items.get();
        let mut complete = true;
        for value in values {
            if !self.collect_abandoned_value_quantizers(value, 0, &mut remaining, &mut keys) {
                complete = false;
                break;
            }
        }
        for key in keys {
            if let Some(observation) = self.quantizers.get_mut(&key) {
                observation.uncertified_use = true;
            }
        }
        if !complete {
            self.residual(
                None,
                UnsupportedResidualKind::AdapterIncomplete,
                "abandoned-value preservation reached its bounded node, depth, or work limit",
            );
        }
    }

    fn mark_quantizers_uncertified(&mut self, value: &AbstractValue) {
        self.mark_abandoned_values_uncertified(std::iter::once(value));
    }

    fn mark_all_quantizers_uncertified(&mut self) {
        self.all_quantizers_uncertified = true;
    }

    fn checked_linear_value(
        &mut self,
        linear: SymbolicLinear,
        site: &ExprSiteId,
        expression: &Expr,
    ) -> AbstractValue {
        if self.linear_fits_runtime_int(&linear) {
            AbstractValue::Int(IntTerm::Linear(linear))
        } else {
            self.mark_quantizers_uncertified(&AbstractValue::Int(IntTerm::Linear(linear)));
            self.unsupported(
                Some((site, expression)),
                UnsupportedResidualKind::RuntimeOverflowNotExcluded,
                "symbolic integer is not proven to remain in Futuruna Int over the declared axis",
            )
        }
    }

    fn with_role<T>(
        &mut self,
        role: BoundaryFragmentRootRole,
        operation: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let prior = self.role;
        self.role = role;
        let value = operation(self);
        self.role = prior;
        value
    }
}

fn axis_endpoint_hull(domain: &ExploreExactDomain) -> Option<(i128, i128)> {
    match domain {
        ExploreExactDomain::IntRange {
            start, cardinality, ..
        } => {
            if *cardinality == 0 {
                return None;
            }
            let end = i128::from(*start).checked_add(i128::from(*cardinality - 1))?;
            Some((i128::from(*start), end))
        }
        ExploreExactDomain::Enumerated { values, .. } => {
            let mut integers = values.iter().map(ExploreValue::int);
            let first = i128::from(integers.next()??);
            let mut minimum = first;
            let mut maximum = first;
            for value in integers {
                let value = i128::from(value?);
                minimum = minimum.min(value);
                maximum = maximum.max(value);
            }
            Some((minimum, maximum))
        }
        ExploreExactDomain::FiniteType { .. } => None,
    }
}

fn exact_ground_value(
    value: &ExploreValue,
    root: &mut impl FnMut(Box<[u32]>) -> CheckedExploreGroundConstructorSite,
    path: &mut Vec<u32>,
    constructors: &CheckedGroundConstructorIndex,
) -> Result<AbstractValue, Box<str>> {
    match value {
        ExploreValue::List(values) => Ok(AbstractValue::List(exact_ground_children(
            values,
            root,
            path,
            constructors,
        )?)),
        ExploreValue::Set(values) => Ok(AbstractValue::Set(exact_ground_children(
            values,
            root,
            path,
            constructors,
        )?)),
        ExploreValue::Tuple(values) => Ok(AbstractValue::Tuple(exact_ground_children(
            values,
            root,
            path,
            constructors,
        )?)),
        ExploreValue::Constructor {
            type_name,
            variant,
            positional,
            fields,
        } => {
            let site = root(path.clone().into_boxed_slice());
            let identity = constructors.get(&site).cloned().ok_or_else(|| {
                format!(
                    "closed constructor `{type_name}.{variant}` has no location-bound checked identity"
                )
                .into_boxed_str()
            })?;
            let layout_matches = matches!(
                (identity.layout, *positional),
                (CheckedConstructorLayout::Positional, true)
                    | (CheckedConstructorLayout::Named, false)
            );
            if !layout_matches || identity.fields.len() != fields.len() {
                return Err(
                    "closed constructor metadata diverges from its checked identity".into(),
                );
            }
            let values = fields.iter().map(|(_, value)| value).collect::<Vec<_>>();
            Ok(AbstractValue::Constructor(AbstractConstructor {
                identity: Some(identity),
                fields: exact_ground_children_refs(&values, root, path, constructors)?,
            }))
        }
        value => Ok(AbstractValue::Ground(value.clone())),
    }
}

fn exact_ground_children(
    values: &[ExploreValue],
    root: &mut impl FnMut(Box<[u32]>) -> CheckedExploreGroundConstructorSite,
    path: &mut Vec<u32>,
    constructors: &CheckedGroundConstructorIndex,
) -> Result<Vec<AbstractValue>, Box<str>> {
    exact_ground_children_refs(&values.iter().collect::<Vec<_>>(), root, path, constructors)
}

fn exact_ground_children_refs(
    values: &[&ExploreValue],
    root: &mut impl FnMut(Box<[u32]>) -> CheckedExploreGroundConstructorSite,
    path: &mut Vec<u32>,
    constructors: &CheckedGroundConstructorIndex,
) -> Result<Vec<AbstractValue>, Box<str>> {
    let mut result = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        let index = u32::try_from(index).map_err(|_| -> Box<str> {
            "closed ground-value path exceeds u32 identity space".into()
        })?;
        path.push(index);
        let child = exact_ground_value(value, root, path, constructors);
        path.pop();
        result.push(child?);
    }
    Ok(result)
}

fn as_int_term(value: &AbstractValue) -> Option<IntTerm> {
    match value {
        AbstractValue::Ground(ExploreValue::Int(value)) => {
            Some(IntTerm::constant(i128::from(*value)))
        }
        AbstractValue::Int(value) => Some(value.clone()),
        _ => None,
    }
}

fn as_predicate(value: &AbstractValue) -> Option<PredicateTerm> {
    match value {
        AbstractValue::Ground(ExploreValue::Boolean(value)) => {
            Some(PredicateTerm::Constant(*value))
        }
        AbstractValue::Predicate(value) => Some(value.clone()),
        _ => None,
    }
}

fn as_ground_bool(value: &AbstractValue) -> Option<bool> {
    as_predicate(value).and_then(|predicate| predicate.constant())
}

fn as_ground_int(value: &AbstractValue) -> Option<i128> {
    as_int_term(value).and_then(|value| value.ground())
}

fn as_plain_affine(value: &AbstractValue) -> Option<AffineForm> {
    as_int_term(value).and_then(|value| value.plain_affine())
}

fn literal_value(literal: &Literal) -> AbstractValue {
    match literal {
        Literal::Int(value) => AbstractValue::Int(IntTerm::constant(i128::from(*value))),
        Literal::Float(value) => AbstractValue::Ground(ExploreValue::FloatBits(value.to_bits())),
        Literal::Str(value) => AbstractValue::Ground(ExploreValue::String(value.clone())),
        Literal::Char(value) => AbstractValue::Ground(ExploreValue::Character(*value)),
        Literal::Bool(value) => AbstractValue::Ground(ExploreValue::Boolean(*value)),
    }
}

fn checked_difference(left: AffineForm, right: AffineForm) -> Option<AffineForm> {
    Some(AffineForm::new(
        left.coefficient.checked_sub(right.coefficient)?,
        left.intercept.checked_sub(right.intercept)?,
    ))
}

fn evaluate_relation(relation: BoundaryRelation, value: i128) -> bool {
    match relation {
        BoundaryRelation::Less => value < 0,
        BoundaryRelation::LessOrEqual => value <= 0,
        BoundaryRelation::Equal => value == 0,
        BoundaryRelation::NotEqual => value != 0,
        BoundaryRelation::GreaterOrEqual => value >= 0,
        BoundaryRelation::Greater => value > 0,
    }
}

fn abstract_ground_equal(left: &AbstractValue, right: &AbstractValue) -> Option<bool> {
    match (left, right) {
        (AbstractValue::Ground(left), AbstractValue::Ground(right)) => Some(left == right),
        (AbstractValue::Int(left), AbstractValue::Int(right)) => {
            Some(left.ground()? == right.ground()?)
        }
        (AbstractValue::Ground(ExploreValue::Int(left)), AbstractValue::Int(right))
        | (AbstractValue::Int(right), AbstractValue::Ground(ExploreValue::Int(left))) => {
            Some(i128::from(*left) == right.ground()?)
        }
        (AbstractValue::Constructor(left), AbstractValue::Constructor(right)) => {
            let (Some(left_identity), Some(right_identity)) = (&left.identity, &right.identity)
            else {
                return None;
            };
            if left_identity != right_identity || left.fields.len() != right.fields.len() {
                return Some(false);
            }
            let fields_equal = left
                .fields
                .iter()
                .zip(right.fields.iter())
                .map(|(left, right)| abstract_ground_equal(left, right))
                .collect::<Option<Vec<_>>>()
                .map(|values| values.into_iter().all(|value| value));
            fields_equal
        }
        (AbstractValue::List(left), AbstractValue::List(right)) => {
            if left.len() != right.len() {
                return Some(false);
            }
            left.iter()
                .zip(right.iter())
                .map(|(left, right)| abstract_ground_equal(left, right))
                .collect::<Option<Vec<_>>>()
                .map(|values| values.into_iter().all(|value| value))
        }
        (AbstractValue::Set(left), AbstractValue::Set(right))
        | (AbstractValue::Tuple(left), AbstractValue::Tuple(right)) => {
            if left.len() != right.len() {
                return Some(false);
            }
            left.iter()
                .zip(right.iter())
                .map(|(left, right)| abstract_ground_equal(left, right))
                .collect::<Option<Vec<_>>>()
                .map(|values| values.into_iter().all(|value| value))
        }
        _ => None,
    }
}

fn reordered_arguments(
    arguments: &[Expr],
    order: Option<&CheckedNamedArgumentOrder>,
) -> Option<Vec<usize>> {
    match order {
        Some(order) => {
            let unique = order
                .canonical_source_indices
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            if order.parameter_names.len() != arguments.len()
                || order.canonical_source_indices.len() != arguments.len()
                || unique.len() != arguments.len()
                || order
                    .canonical_source_indices
                    .iter()
                    .any(|index| *index >= arguments.len())
            {
                None
            } else {
                Some(order.canonical_source_indices.to_vec())
            }
        }
        None if arguments
            .iter()
            .all(|argument| named_arg_parts(argument).is_none()) =>
        {
            Some((0..arguments.len()).collect())
        }
        None => None,
    }
}

fn unwrap_named_value(value: AbstractValue) -> AbstractValue {
    value
}

impl AdapterContext<'_, '_> {
    fn eval_site(&mut self, site: &ExprSiteId, env: &AbstractEnv) -> AbstractValue {
        let Some(indexed) = self.index.expression(site) else {
            return self.unsupported(
                None,
                UnsupportedResidualKind::AdapterIncomplete,
                "Phase-A expression site is absent from the structural index",
            );
        };
        self.eval_expr(indexed.declaration, indexed.expression, site, env)
    }

    fn child_site(&self, site: &ExprSiteId, child: usize) -> ExprSiteId {
        let mut path = site.ast_path.to_vec();
        path.push(child as u32);
        ExprSiteId {
            analysis_program: site.analysis_program.clone(),
            declaration: site.declaration.clone(),
            normalized_declaration_ordinal: site.normalized_declaration_ordinal,
            ast_path: path.into_boxed_slice(),
        }
    }

    fn eval_expr(
        &mut self,
        declaration: &SourcedStmt,
        expression: &Expr,
        site: &ExprSiteId,
        env: &AbstractEnv,
    ) -> AbstractValue {
        let source = source_site(site, expression);
        if !self.charge(Some(source.clone())) {
            return self.unsupported(
                Some((site, expression)),
                UnsupportedResidualKind::AdapterIncomplete,
                "abstract evaluation budget exhausted",
            );
        }
        match &expression.kind {
            ExprKind::Var(_) => self.eval_var(site, expression, env),
            ExprKind::Lit(literal) => literal_value(literal),
            ExprKind::App(function, arguments) => {
                self.eval_app(declaration, expression, site, function, arguments, env)
            }
            ExprKind::Lambda(parameters, _) => {
                if parameters.len() > self.limits.max_collection_items.get()
                    || parameters.len() > u32::MAX as usize
                    || env.len() > self.limits.max_collection_items.get()
                {
                    self.all_quantizers_uncertified = true;
                    return self.unsupported_preserving_children(
                        site,
                        expression,
                        env,
                        UnsupportedResidualKind::NonEnumerableDispatch,
                        "lambda exceeds the bounded parameter limit",
                    );
                }
                let body_site = self.child_site(site, 0);
                let parameter_sites = parameters
                    .iter()
                    .enumerate()
                    .map(|(index, _)| {
                        structural_binder_site(
                            self.index.program_id,
                            declaration,
                            &site.ast_path,
                            vec![BINDER_PARAMETER, index as u32],
                        )
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                AbstractValue::Callable(AbstractCallable::Lambda {
                    body_site,
                    parameters: parameter_sites,
                    captured: env.clone(),
                })
            }
            ExprKind::BinOp(operator, left, right) => {
                let left = self.eval_site(&self.child_site(site, 0), env);
                let right = self.eval_site(&self.child_site(site, 1), env);
                self.eval_binop(operator, left, right, site, expression)
            }
            ExprKind::UnOp(operator, operand) => {
                let value = self.eval_site(&self.child_site(site, 0), env);
                self.eval_unop(operator, value, site, expression)
            }
            ExprKind::If(_, _, _) => self.eval_if(site, expression, env),
            ExprKind::Match(_, _) => self.eval_match(declaration, site, expression, env),
            ExprKind::Block(statements) => {
                self.eval_block(declaration, site, expression, statements, env)
            }
            ExprKind::Field(_, _) => self.eval_field(site, expression, env),
            ExprKind::List(items) => {
                if items.len() > self.limits.max_collection_items.get() {
                    return self.unsupported_preserving_children(
                        site,
                        expression,
                        env,
                        UnsupportedResidualKind::NonEnumerableDispatch,
                        "list literal exceeds abstract collection limit",
                    );
                }
                AbstractValue::List(
                    (0..items.len())
                        .map(|index| self.eval_site(&self.child_site(site, index), env))
                        .collect(),
                )
            }
            ExprKind::Tuple(items) => {
                if items.len() > self.limits.max_collection_items.get() {
                    return self.unsupported_preserving_children(
                        site,
                        expression,
                        env,
                        UnsupportedResidualKind::NonEnumerableDispatch,
                        "tuple literal exceeds abstract collection limit",
                    );
                }
                let values = (0..items.len())
                    .map(|index| self.eval_site(&self.child_site(site, index), env))
                    .collect::<Vec<_>>();
                AbstractValue::Tuple(values)
            }
            ExprKind::Conjunction(parts) | ExprKind::Disjunction(parts) => {
                if parts.len() > self.limits.max_collection_items.get() {
                    return self.unsupported_preserving_children(
                        site,
                        expression,
                        env,
                        UnsupportedResidualKind::NonEnumerableDispatch,
                        "logical aggregate exceeds abstract collection limit",
                    );
                }
                let values = (0..parts.len())
                    .map(|index| self.eval_site(&self.child_site(site, index), env))
                    .collect::<Vec<_>>();
                let predicates = values.iter().map(as_predicate).collect::<Option<Vec<_>>>();
                match (predicates, &expression.kind) {
                    (Some(parts), ExprKind::Conjunction(_)) => {
                        AbstractValue::Predicate(PredicateTerm::All(parts))
                    }
                    (Some(parts), ExprKind::Disjunction(_)) => {
                        AbstractValue::Predicate(PredicateTerm::Any(parts))
                    }
                    _ => {
                        for value in &values {
                            self.mark_quantizers_uncertified(value);
                        }
                        self.unsupported(
                            Some((site, expression)),
                            UnsupportedResidualKind::UnsupportedType,
                            "logical expression has a non-predicate operand",
                        )
                    }
                }
            }
            ExprKind::Unit => AbstractValue::Ground(ExploreValue::Unit),
            ExprKind::Pipe(_, _) => self.unsupported_preserving_children(
                site,
                expression,
                env,
                UnsupportedResidualKind::UnresolvedCallable,
                "pipe/value dispatch is intentionally opaque",
            ),
            ExprKind::Index(_, _) => {
                for child in 0..2 {
                    let value = self.eval_site(&self.child_site(site, child), env);
                    self.mark_quantizers_uncertified(&value);
                }
                self.unsupported(
                    Some((site, expression)),
                    UnsupportedResidualKind::NonEnumerableDispatch,
                    "indexing is not normalized by the first adapter slice",
                )
            }
            ExprKind::Effect(_, arguments) => {
                if arguments.len() > self.limits.max_collection_items.get() {
                    return self.unsupported_preserving_children(
                        site,
                        expression,
                        env,
                        UnsupportedResidualKind::Effect,
                        "effect application exceeds the bounded argument limit",
                    );
                }
                for child in 0..arguments.len() {
                    let value = self.eval_site(&self.child_site(site, child), env);
                    self.mark_quantizers_uncertified(&value);
                }
                self.unsupported(
                    Some((site, expression)),
                    UnsupportedResidualKind::Effect,
                    "effectful expression is outside exact source-event adaptation",
                )
            }
            ExprKind::Handle { .. } => self.unsupported_preserving_children(
                site,
                expression,
                env,
                UnsupportedResidualKind::Effect,
                "effect handler is outside exact source-event adaptation",
            ),
            ExprKind::Try(_) => {
                let value = self.eval_site(&self.child_site(site, 0), env);
                self.mark_quantizers_uncertified(&value);
                self.unsupported(
                    Some((site, expression)),
                    UnsupportedResidualKind::UnsupportedType,
                    "propagating try is outside the first adapter slice",
                )
            }
        }
    }

    fn eval_var(
        &mut self,
        site: &ExprSiteId,
        expression: &Expr,
        env: &AbstractEnv,
    ) -> AbstractValue {
        let Some(resolution) = self.resolution(site).cloned() else {
            return self.unsupported(
                Some((site, expression)),
                UnsupportedResidualKind::UnresolvedCallable,
                "variable has no Phase-B resolution",
            );
        };
        let Some(binding) = resolution.value_binding else {
            return self.unsupported(
                Some((site, expression)),
                UnsupportedResidualKind::UnresolvedCallable,
                "variable has no Phase-B resolution",
            );
        };
        match binding {
            CheckedValueBinding::Binder { site: binder, .. } => {
                env.get(&binder).cloned().unwrap_or_else(|| {
                    self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::AdapterIncomplete,
                        "resolved binder has no value in the specialized environment",
                    )
                })
            }
            CheckedValueBinding::TopLevel(binding) => {
                self.eval_top_level(&binding, site, expression)
            }
            CheckedValueBinding::Callable(callable) => {
                AbstractValue::Callable(AbstractCallable::Function(callable))
            }
            CheckedValueBinding::RuleFamily(family) => {
                AbstractValue::Callable(AbstractCallable::RuleFamily(family))
            }
            CheckedValueBinding::Constructor { .. } => match resolution.exact_constructor {
                Some(identity) if identity.fields.is_empty() => {
                    AbstractValue::Constructor(AbstractConstructor {
                        identity: Some(Arc::new(identity)),
                        fields: Vec::new(),
                    })
                }
                Some(_) => self.unsupported(
                    Some((site, expression)),
                    UnsupportedResidualKind::UnresolvedCallable,
                    "non-nullary constructor value requires an exact checked application",
                ),
                None => self.unsupported(
                    Some((site, expression)),
                    UnsupportedResidualKind::UnresolvedCallable,
                    "constructor value has no producer-minted exact constructor identity",
                ),
            },
            CheckedValueBinding::OpaqueQualifiedOwner(_) => self.unsupported(
                Some((site, expression)),
                UnsupportedResidualKind::UnresolvedCallable,
                "variable is not an exact Phase-B value binding",
            ),
        }
    }

    fn eval_top_level(
        &mut self,
        binding: &CheckedTopLevelBindingId,
        site: &ExprSiteId,
        expression: &Expr,
    ) -> AbstractValue {
        let cache_key = (binding.clone(), self.role);
        if let Some(value) = self.top_level_cache.get(&cache_key) {
            return value.clone();
        }
        let Some(initializer_site) = top_level_initializer_site(self.index, binding) else {
            return self.unsupported(
                Some((site, expression)),
                UnsupportedResidualKind::AdapterIncomplete,
                "top-level binding has no ordinary Phase-A initializer",
            );
        };
        if binding.binder_path.as_ref() != [BINDER_PATTERN] {
            return self.unsupported(
                Some((site, expression)),
                UnsupportedResidualKind::UnsupportedType,
                "destructuring top-level bindings are not normalized by this slice",
            );
        }
        let token = format!(
            "top:{}:{}",
            binding.declaration.declaration.semantic_key(),
            binding.declaration.normalized_ordinal
        )
        .into_boxed_str();
        if self.call_stack.len() >= self.limits.max_call_depth.get()
            || self.call_stack.contains(&token)
        {
            return self.unsupported(
                Some((site, expression)),
                UnsupportedResidualKind::Recursion,
                "recursive top-level initializer",
            );
        }
        self.call_stack.push(token);
        let value = self.eval_site(&initializer_site, &AbstractEnv::new());
        self.call_stack.pop();
        self.top_level_cache.insert(cache_key, value.clone());
        value
    }
}

fn abstract_to_ground(value: &AbstractValue) -> Option<ExploreValue> {
    match value {
        AbstractValue::Ground(value) => Some(value.clone()),
        AbstractValue::Int(value) => Some(ExploreValue::Int(i64::try_from(value.ground()?).ok()?)),
        AbstractValue::Predicate(value) => Some(ExploreValue::Boolean(value.constant()?)),
        AbstractValue::List(values) => Some(ExploreValue::List(
            values
                .iter()
                .map(abstract_to_ground)
                .collect::<Option<Vec<_>>>()?,
        )),
        AbstractValue::Set(values) => Some(ExploreValue::Set(
            values
                .iter()
                .map(abstract_to_ground)
                .collect::<Option<Vec<_>>>()?,
        )),
        AbstractValue::Tuple(values) => Some(ExploreValue::Tuple(
            values
                .iter()
                .map(abstract_to_ground)
                .collect::<Option<Vec<_>>>()?,
        )),
        AbstractValue::Constructor(value) => {
            let identity = value.identity.as_ref()?;
            if identity.fields.len() != value.fields.len() {
                return None;
            }
            Some(ExploreValue::Constructor {
                type_name: identity.owner_type.to_string(),
                variant: identity.variant.to_string(),
                positional: identity.layout == CheckedConstructorLayout::Positional,
                fields: identity
                    .fields
                    .iter()
                    .zip(value.fields.iter())
                    .enumerate()
                    .map(|(index, (field, value))| {
                        Some((
                            if identity.layout == CheckedConstructorLayout::Positional {
                                index.to_string()
                            } else {
                                field.name.to_string()
                            },
                            abstract_to_ground(value)?,
                        ))
                    })
                    .collect::<Option<Vec<_>>>()?,
            })
        }
        AbstractValue::Callable(_) | AbstractValue::Unsupported(_) => None,
    }
}

impl AdapterContext<'_, '_> {
    fn eval_binop(
        &mut self,
        operator: &str,
        left: AbstractValue,
        right: AbstractValue,
        site: &ExprSiteId,
        expression: &Expr,
    ) -> AbstractValue {
        match operator {
            "+" | "-" => {
                let (Some(left), Some(mut right)) = (as_int_term(&left), as_int_term(&right))
                else {
                    self.mark_quantizers_uncertified(&left);
                    self.mark_quantizers_uncertified(&right);
                    return self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::NonAffineArithmetic,
                        "addition/subtraction has a non-integer operand",
                    );
                };
                if operator == "-" {
                    right = match right {
                        IntTerm::Linear(value) => match value.checked_scale(-1) {
                            Some(value) => IntTerm::Linear(value),
                            None => {
                                self.mark_quantizers_uncertified(&AbstractValue::Int(left.clone()));
                                self.mark_quantizers_uncertified(&AbstractValue::Int(
                                    IntTerm::Linear(value),
                                ));
                                return self.unsupported(
                                    Some((site, expression)),
                                    UnsupportedResidualKind::ArithmeticOverflow,
                                    "symbolic subtraction overflowed i128",
                                );
                            }
                        },
                        IntTerm::PositiveAffine { .. } | IntTerm::Unsupported(_) => {
                            return self.unsupported(
                                Some((site, expression)),
                                UnsupportedResidualKind::NonAffineArithmetic,
                                "subtraction from a piecewise-positive operand is non-affine",
                            );
                        }
                    };
                }
                match (left, right) {
                    (IntTerm::Linear(left), IntTerm::Linear(right)) => {
                        match left.checked_add(&right) {
                            Some(value) => self.checked_linear_value(value, site, expression),
                            None => {
                                self.mark_quantizers_uncertified(&AbstractValue::Int(
                                    IntTerm::Linear(left),
                                ));
                                self.mark_quantizers_uncertified(&AbstractValue::Int(
                                    IntTerm::Linear(right),
                                ));
                                self.unsupported(
                                    Some((site, expression)),
                                    UnsupportedResidualKind::ArithmeticOverflow,
                                    "symbolic addition overflowed i128",
                                )
                            }
                        }
                    }
                    (left, right) => {
                        self.mark_quantizers_uncertified(&right_value(left));
                        self.mark_quantizers_uncertified(&right_value(right));
                        self.unsupported(
                            Some((site, expression)),
                            UnsupportedResidualKind::NonAffineArithmetic,
                            "piecewise-positive addition is outside the affine fragment",
                        )
                    }
                }
            }
            "*" => {
                let (Some(left), Some(right)) = (as_int_term(&left), as_int_term(&right)) else {
                    self.mark_quantizers_uncertified(&left);
                    self.mark_quantizers_uncertified(&right);
                    return self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::NonAffineArithmetic,
                        "multiplication has a non-integer operand",
                    );
                };
                let scaled = if let (Some(scale), IntTerm::Linear(value)) = (left.ground(), &right)
                {
                    value.checked_scale(scale)
                } else if let (Some(scale), IntTerm::Linear(value)) = (right.ground(), &left) {
                    value.checked_scale(scale)
                } else {
                    None
                };
                match scaled {
                    Some(value) => self.checked_linear_value(value, site, expression),
                    None => {
                        self.mark_quantizers_uncertified(&left_value_from_terms(&left, &right));
                        self.unsupported(
                            Some((site, expression)),
                            UnsupportedResidualKind::NonAffineArithmetic,
                            "multiplication requires one exact constant integer operand",
                        )
                    }
                }
            }
            "/" => self.eval_division(left, right, site, expression),
            "%" => {
                self.mark_quantizers_uncertified(&left);
                self.mark_quantizers_uncertified(&right);
                self.unsupported(
                    Some((site, expression)),
                    UnsupportedResidualKind::NonAffineArithmetic,
                    "remainder normalization is deferred from the first adapter slice",
                )
            }
            "<" | "<=" | "==" | "!=" | ">=" | ">" => {
                let relation = match operator {
                    "<" => BoundaryRelation::Less,
                    "<=" => BoundaryRelation::LessOrEqual,
                    "==" => BoundaryRelation::Equal,
                    "!=" => BoundaryRelation::NotEqual,
                    ">=" => BoundaryRelation::GreaterOrEqual,
                    ">" => BoundaryRelation::Greater,
                    _ => unreachable!(),
                };
                if let (Some(IntTerm::Linear(left_linear)), Some(IntTerm::Linear(right_linear))) =
                    (as_int_term(&left), as_int_term(&right))
                {
                    let Some(negated_right) = right_linear.checked_scale(-1) else {
                        self.mark_quantizers_uncertified(&left);
                        self.mark_quantizers_uncertified(&right);
                        return self.unsupported(
                            Some((site, expression)),
                            UnsupportedResidualKind::ArithmeticOverflow,
                            "comparison normalization overflowed i128",
                        );
                    };
                    let Some(difference) = left_linear.checked_add(&negated_right) else {
                        self.mark_quantizers_uncertified(&left);
                        self.mark_quantizers_uncertified(&right);
                        return self.unsupported(
                            Some((site, expression)),
                            UnsupportedResidualKind::ArithmeticOverflow,
                            "comparison normalization overflowed i128",
                        );
                    };
                    if let Some(difference) = difference.plain_affine() {
                        if difference.coefficient == 0 {
                            return AbstractValue::Ground(ExploreValue::Boolean(
                                evaluate_relation(relation, difference.intercept),
                            ));
                        }
                        let predicate = PredicateTerm::Comparison {
                            difference,
                            relation,
                            source: source_site(site, expression),
                        };
                        self.add_predicate_root(predicate.clone());
                        AbstractValue::Predicate(predicate)
                    } else {
                        AbstractValue::Predicate(PredicateTerm::LinearComparison {
                            difference,
                            relation,
                            source: source_site(site, expression),
                        })
                    }
                } else if matches!(
                    relation,
                    BoundaryRelation::Equal | BoundaryRelation::NotEqual
                ) {
                    match abstract_ground_equal(&left, &right) {
                        Some(equal) => AbstractValue::Ground(ExploreValue::Boolean(
                            if matches!(relation, BoundaryRelation::Equal) {
                                equal
                            } else {
                                !equal
                            },
                        )),
                        None => {
                            self.mark_quantizers_uncertified(&left);
                            self.mark_quantizers_uncertified(&right);
                            self.unsupported(
                                Some((site, expression)),
                                UnsupportedResidualKind::UnsupportedType,
                                "equality operands are not exact ground values or affine integers",
                            )
                        }
                    }
                } else {
                    self.mark_quantizers_uncertified(&left);
                    self.mark_quantizers_uncertified(&right);
                    self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::NonAffineArithmetic,
                        "ordered comparison operands are not affine integers",
                    )
                }
            }
            "&&" | "||" => {
                let (Some(left), Some(right)) = (as_predicate(&left), as_predicate(&right)) else {
                    self.mark_quantizers_uncertified(&left);
                    self.mark_quantizers_uncertified(&right);
                    return self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::UnsupportedType,
                        "Boolean connective has a non-predicate operand",
                    );
                };
                let predicate = if operator == "&&" {
                    PredicateTerm::All(vec![left, right])
                } else {
                    PredicateTerm::Any(vec![left, right])
                };
                AbstractValue::Predicate(predicate)
            }
            _ => {
                self.mark_quantizers_uncertified(&left);
                self.mark_quantizers_uncertified(&right);
                self.unsupported(
                    Some((site, expression)),
                    UnsupportedResidualKind::NonAffineArithmetic,
                    format!("operator `{operator}` is outside the first adapter fragment"),
                )
            }
        }
    }

    fn eval_division(
        &mut self,
        numerator: AbstractValue,
        divisor: AbstractValue,
        site: &ExprSiteId,
        expression: &Expr,
    ) -> AbstractValue {
        let Some(divisor_value) = as_ground_int(&divisor) else {
            self.mark_quantizers_uncertified(&numerator);
            self.mark_quantizers_uncertified(&divisor);
            return self.unsupported(
                Some((site, expression)),
                UnsupportedResidualKind::VariableDivisor,
                "integer division requires an exact constant divisor",
            );
        };
        let Ok(divisor_i64) = i64::try_from(divisor_value) else {
            self.mark_quantizers_uncertified(&numerator);
            return self.unsupported(
                Some((site, expression)),
                UnsupportedResidualKind::InvalidConstant,
                "integer divisor is outside Futuruna Int",
            );
        };
        if divisor_i64 <= 0 {
            self.mark_quantizers_uncertified(&numerator);
            return self.unsupported(
                Some((site, expression)),
                UnsupportedResidualKind::InvalidConstant,
                "first adapter slice requires a positive integer divisor",
            );
        }
        let Some(numerator_term) = as_int_term(&numerator) else {
            self.mark_quantizers_uncertified(&numerator);
            return self.unsupported(
                Some((site, expression)),
                UnsupportedResidualKind::NonAffineArithmetic,
                "division numerator is not an integer term",
            );
        };
        if let Some(ground) = numerator_term.ground() {
            return AbstractValue::Int(IntTerm::constant(ground / divisor_value));
        }
        let (affine, nonnegative) = match numerator_term {
            IntTerm::Linear(value) => match value.plain_affine() {
                Some(value) => (value, false),
                None => {
                    self.mark_quantizers_uncertified(&numerator);
                    return self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::NonAffineArithmetic,
                        "nested quantized division is outside the first adapter fragment",
                    );
                }
            },
            IntTerm::PositiveAffine { affine, .. } => (affine, true),
            IntTerm::Unsupported(residual) => return AbstractValue::Unsupported(residual),
        };
        let source = source_site(site, expression);
        if !self.affine_fits_runtime_int(affine) {
            return self.unsupported(
                Some((site, expression)),
                UnsupportedResidualKind::RuntimeOverflowNotExcluded,
                "division numerator is not proven to remain in Futuruna Int over the declared axis",
            );
        }
        let key = self.observe_quantizer(source, affine, divisor_i64, nonnegative);
        let mut quantizers = BTreeMap::new();
        quantizers.insert(key, 1);
        self.checked_linear_value(
            SymbolicLinear {
                affine: AffineForm::new(0, 0),
                quantizers,
            },
            site,
            expression,
        )
    }

    fn eval_unop(
        &mut self,
        operator: &str,
        value: AbstractValue,
        site: &ExprSiteId,
        expression: &Expr,
    ) -> AbstractValue {
        match operator {
            "-" => match as_int_term(&value) {
                Some(IntTerm::Linear(value)) => match value.checked_scale(-1) {
                    Some(value) => self.checked_linear_value(value, site, expression),
                    None => {
                        self.mark_quantizers_uncertified(&AbstractValue::Int(IntTerm::Linear(
                            value,
                        )));
                        self.unsupported(
                            Some((site, expression)),
                            UnsupportedResidualKind::ArithmeticOverflow,
                            "unary negation overflowed symbolic i128",
                        )
                    }
                },
                _ => {
                    self.mark_quantizers_uncertified(&value);
                    self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::NonAffineArithmetic,
                        "unary negation requires a linear integer",
                    )
                }
            },
            "!" => match as_predicate(&value) {
                Some(value) => AbstractValue::Predicate(PredicateTerm::Not(Box::new(value))),
                None => {
                    self.mark_quantizers_uncertified(&value);
                    self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::UnsupportedType,
                        "logical negation requires a predicate",
                    )
                }
            },
            _ => {
                self.mark_quantizers_uncertified(&value);
                self.unsupported(
                    Some((site, expression)),
                    UnsupportedResidualKind::NonAffineArithmetic,
                    format!("unary operator `{operator}` is unsupported"),
                )
            }
        }
    }
}

fn right_value(term: IntTerm) -> AbstractValue {
    AbstractValue::Int(term)
}

fn left_value_from_terms(left: &IntTerm, right: &IntTerm) -> AbstractValue {
    let mut values = Vec::new();
    values.push(AbstractValue::Int(left.clone()));
    values.push(AbstractValue::Int(right.clone()));
    AbstractValue::List(values)
}

impl AdapterContext<'_, '_> {
    fn eval_if(
        &mut self,
        site: &ExprSiteId,
        expression: &Expr,
        env: &AbstractEnv,
    ) -> AbstractValue {
        let condition = self.eval_site(&self.child_site(site, 0), env);
        if let Some(condition) = as_ground_bool(&condition) {
            return self.eval_site(&self.child_site(site, if condition { 1 } else { 2 }), env);
        }
        let then_value = self.eval_site(&self.child_site(site, 1), env);
        let else_value = self.eval_site(&self.child_site(site, 2), env);
        let Some(predicate) = as_predicate(&condition) else {
            self.mark_quantizers_uncertified(&condition);
            self.mark_quantizers_uncertified(&then_value);
            self.mark_quantizers_uncertified(&else_value);
            return self.unsupported(
                Some((site, expression)),
                UnsupportedResidualKind::NonEnumerableDispatch,
                "if condition is not an exact predicate",
            );
        };
        if let Some(value) =
            self.recognize_positive(&predicate, &then_value, &else_value, site, expression)
        {
            return value;
        }
        if let Some(value) =
            self.recognize_min_max(&predicate, &then_value, &else_value, site, expression)
        {
            return value;
        }
        if abstract_ground_equal(&then_value, &else_value) == Some(true) {
            return then_value;
        }
        self.mark_quantizers_uncertified(&then_value);
        self.mark_quantizers_uncertified(&else_value);
        self.mark_quantizers_uncertified(&condition);
        self.unsupported(
            Some((site, expression)),
            UnsupportedResidualKind::NonEnumerableDispatch,
            "axis-dependent if is not a supported positive/min/max normalization",
        )
    }

    fn recognize_positive(
        &mut self,
        predicate: &PredicateTerm,
        then_value: &AbstractValue,
        else_value: &AbstractValue,
        site: &ExprSiteId,
        expression: &Expr,
    ) -> Option<AbstractValue> {
        if as_ground_int(else_value) != Some(0) {
            return None;
        }
        let then_term = as_int_term(then_value)?;
        match predicate {
            PredicateTerm::Comparison {
                difference,
                relation: BoundaryRelation::Greater,
                ..
            } if then_term.plain_affine() == Some(*difference) => {
                let source = source_site(site, expression);
                self.add_int_root(
                    &source,
                    BoundaryIntExpr::Max {
                        left_minus_right: *difference,
                        tie_arm: TieArm::Right,
                        source: source.clone(),
                    },
                );
                Some(AbstractValue::Int(IntTerm::PositiveAffine {
                    affine: *difference,
                    source,
                }))
            }
            PredicateTerm::LinearComparison {
                difference,
                relation: BoundaryRelation::Greater,
                ..
            } => {
                let IntTerm::Linear(then_linear) = then_term else {
                    return None;
                };
                if then_linear.affine != difference.affine
                    || then_linear.quantizers != difference.quantizers
                    || difference.affine.coefficient != 0
                    || difference.affine.intercept <= 0
                    || difference.quantizers.len() != 1
                {
                    return None;
                }
                let (quantizer, coefficient) = difference.quantizers.iter().next()?;
                if *coefficient >= 0 {
                    return None;
                }
                let decay = coefficient.checked_neg()?;
                let cutoff = ceil_div_positive(difference.affine.intercept, decay)?;
                if let Some(observation) = self.quantizers.get_mut(quantizer) {
                    observation.cutoff_cells.insert(cutoff);
                } else {
                    return None;
                }
                let residual = self.residual(
                    Some(source_site(site, expression)),
                    UnsupportedResidualKind::NonAffineArithmetic,
                    "positive quantized result supplied a liveness proof but remains piecewise for value normalization",
                );
                Some(AbstractValue::Int(IntTerm::Unsupported(residual)))
            }
            _ => None,
        }
    }

    fn recognize_min_max(
        &mut self,
        predicate: &PredicateTerm,
        then_value: &AbstractValue,
        else_value: &AbstractValue,
        site: &ExprSiteId,
        expression: &Expr,
    ) -> Option<AbstractValue> {
        let then_affine = as_plain_affine(then_value)?;
        let else_affine = as_plain_affine(else_value)?;
        let difference = checked_difference(then_affine, else_affine)?;
        let PredicateTerm::Comparison {
            difference: condition_difference,
            relation,
            ..
        } = predicate
        else {
            return None;
        };
        if *condition_difference != difference {
            return None;
        }
        let source = source_site(site, expression);
        match relation {
            BoundaryRelation::Less | BoundaryRelation::LessOrEqual => self.add_int_root(
                &source,
                BoundaryIntExpr::Min {
                    left_minus_right: difference,
                    tie_arm: if matches!(relation, BoundaryRelation::LessOrEqual) {
                        TieArm::Left
                    } else {
                        TieArm::Right
                    },
                    source: source.clone(),
                },
            ),
            BoundaryRelation::Greater | BoundaryRelation::GreaterOrEqual => self.add_int_root(
                &source,
                BoundaryIntExpr::Max {
                    left_minus_right: difference,
                    tie_arm: if matches!(relation, BoundaryRelation::GreaterOrEqual) {
                        TieArm::Left
                    } else {
                        TieArm::Right
                    },
                    source: source.clone(),
                },
            ),
            BoundaryRelation::Equal | BoundaryRelation::NotEqual => return None,
        }
        let residual = self.residual(
            Some(source),
            UnsupportedResidualKind::NonAffineArithmetic,
            "min/max event was normalized, but its piecewise value remains explicit",
        );
        Some(AbstractValue::Int(IntTerm::Unsupported(residual)))
    }
}

fn ceil_div_positive(numerator: i128, denominator: i128) -> Option<i128> {
    if numerator < 0 || denominator <= 0 {
        return None;
    }
    numerator
        .checked_add(denominator.checked_sub(1)?)?
        .checked_div(denominator)
}

impl AdapterContext<'_, '_> {
    fn eval_app(
        &mut self,
        declaration: &SourcedStmt,
        expression: &Expr,
        site: &ExprSiteId,
        function: &Expr,
        arguments: &[Expr],
        env: &AbstractEnv,
    ) -> AbstractValue {
        if arguments.len() > self.limits.max_collection_items.get() {
            return self.unsupported_preserving_children(
                site,
                expression,
                env,
                UnsupportedResidualKind::NonEnumerableDispatch,
                "application exceeds the bounded argument limit",
            );
        }
        let Some(resolution) = self.resolution(site).cloned() else {
            return self.unsupported_preserving_children(
                site,
                expression,
                env,
                UnsupportedResidualKind::UnresolvedCallable,
                "application has no Phase-B resolution",
            );
        };
        let exact_constructor = resolution.exact_constructor.map(Arc::new);
        let Some(target) = resolution.call_target else {
            return self.unsupported_preserving_children(
                site,
                expression,
                env,
                UnsupportedResidualKind::UnresolvedCallable,
                "application has no exact Phase-B call target",
            );
        };
        let Some(order) = reordered_arguments(arguments, resolution.named_arguments.as_ref())
        else {
            return self.unsupported_preserving_children(
                site,
                expression,
                env,
                UnsupportedResidualKind::UnresolvedCallable,
                "checked named-argument permutation is missing or inconsistent",
            );
        };
        let mut values = Vec::with_capacity(order.len());
        for source_index in order {
            let argument_site = self.child_site(site, source_index + 1);
            values.push(unwrap_named_value(self.eval_site(&argument_site, env)));
        }

        match target {
            CheckedCallTarget::Builtin {
                canonical_name,
                arity,
            } => self.eval_builtin(canonical_name.as_ref(), arity, values, site, expression),
            CheckedCallTarget::Constructor { arity, .. } => {
                let Some(identity) = exact_constructor else {
                    for value in &values {
                        self.mark_quantizers_uncertified(value);
                    }
                    return self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::UnresolvedCallable,
                        "constructor call has no producer-minted exact constructor identity",
                    );
                };
                if arity != values.len() || identity.fields.len() != values.len() {
                    for value in &values {
                        self.mark_quantizers_uncertified(value);
                    }
                    return self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::AdapterIncomplete,
                        "checked constructor arity disagrees with canonical argument order",
                    );
                }
                AbstractValue::Constructor(AbstractConstructor {
                    identity: Some(identity),
                    fields: values,
                })
            }
            CheckedCallTarget::Function { callable, arity } => {
                if arity != values.len() {
                    for value in &values {
                        self.mark_quantizers_uncertified(value);
                    }
                    return self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::AdapterIncomplete,
                        "checked function arity disagrees with canonical argument order",
                    );
                }
                self.eval_function(&callable, values, site, expression)
            }
            CheckedCallTarget::RuleFamily(family) => {
                self.eval_rule_family(&family, values, &AbstractEnv::new(), site, expression)
            }
            CheckedCallTarget::ScopedMember {
                rule_family: Some(family),
                arity,
                ..
            } => {
                if arity != values.len() {
                    for value in &values {
                        self.mark_quantizers_uncertified(value);
                    }
                    return self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::AdapterIncomplete,
                        "checked scoped-rule arity disagrees with canonical argument order",
                    );
                }
                let captures = self.scoped_receiver_captures(
                    declaration,
                    function,
                    site,
                    env,
                    &family,
                    expression,
                );
                let Some(captures) = captures else {
                    for value in &values {
                        self.mark_quantizers_uncertified(value);
                    }
                    return self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::NonEnumerableDispatch,
                        "scoped rule receiver cannot be specialized exactly",
                    );
                };
                self.eval_rule_family(&family, values, &captures, site, expression)
            }
            CheckedCallTarget::ScopedMember {
                rule_family: None, ..
            } => {
                for value in &values {
                    self.mark_quantizers_uncertified(value);
                }
                self.unsupported(
                    Some((site, expression)),
                    UnsupportedResidualKind::UnresolvedCallable,
                    "Phase B does not retain an exact callable identity for this ordinary scoped member",
                )
            }
        }
    }

    fn scoped_receiver_captures(
        &mut self,
        _declaration: &SourcedStmt,
        function: &Expr,
        app_site: &ExprSiteId,
        env: &AbstractEnv,
        family: &RuleDispatchKey,
        expression: &Expr,
    ) -> Option<AbstractEnv> {
        let ExprKind::Field(_, _) = &function.kind else {
            return None;
        };
        let function_site = self.child_site(app_site, 0);
        let receiver_site = self.child_site(&function_site, 0);
        let receiver = self.eval_site(&receiver_site, env);
        let AbstractValue::Constructor(receiver) = receiver else {
            self.mark_quantizers_uncertified(&receiver);
            return None;
        };
        let owner = self
            .artifacts
            .checked_resolutions
            .rule_families
            .get(family)
            .and_then(|resolution| resolution.candidates.first())
            .map(|candidate| candidate.declaration.clone());
        let Some(owner) = owner else {
            self.mark_quantizers_uncertified(&AbstractValue::Constructor(receiver));
            return None;
        };
        let exact_owner = receiver.identity.as_ref().map(|identity| &identity.owner);
        if exact_owner != Some(&CheckedDataTypeId::Declared(owner.clone())) {
            self.mark_quantizers_uncertified(&AbstractValue::Constructor(receiver.clone()));
            self.residual(
                Some(source_site(app_site, expression)),
                UnsupportedResidualKind::NonEnumerableDispatch,
                "scoped receiver constructor occurrence differs from the checked rule owner",
            );
            return None;
        }
        let parameter_count = self.index.declarations.get(&owner).and_then(|declaration| {
            let Stmt::TypeDecl(TypeDecl::RuleScope { params, .. }) = &*declaration.statement else {
                return None;
            };
            Some(params.len())
        });
        let Some(parameter_count) = parameter_count else {
            self.mark_quantizers_uncertified(&AbstractValue::Constructor(receiver));
            return None;
        };
        if parameter_count != receiver.fields.len() {
            self.mark_quantizers_uncertified(&AbstractValue::Constructor(receiver));
            return None;
        }
        if receiver.fields.len() > self.limits.max_collection_items.get()
            || receiver.fields.len() > u32::MAX as usize
        {
            self.mark_quantizers_uncertified(&AbstractValue::Constructor(receiver.clone()));
            self.mark_all_quantizers_uncertified();
            self.residual(
                Some(source_site(app_site, expression)),
                UnsupportedResidualKind::NonEnumerableDispatch,
                "scoped receiver capture exceeds the bounded field limit",
            );
            return None;
        }
        let declaration = *self.index.declarations.get(&owner)?;
        let mut captures = AbstractEnv::new();
        for (index, value) in receiver.fields.into_iter().enumerate() {
            let index = u32::try_from(index).ok()?;
            captures.insert(
                structural_binder_site(
                    self.index.program_id,
                    declaration,
                    &[],
                    vec![BINDER_PARAMETER, index],
                ),
                value,
            );
        }
        Some(captures)
    }

    fn eval_function(
        &mut self,
        callable: &CheckedCallableId,
        arguments: Vec<AbstractValue>,
        site: &ExprSiteId,
        expression: &Expr,
    ) -> AbstractValue {
        let Some(entry) = self.index.callables.get(callable).cloned() else {
            for argument in &arguments {
                self.mark_quantizers_uncertified(argument);
            }
            return self.unsupported(
                Some((site, expression)),
                UnsupportedResidualKind::UnresolvedCallable,
                "checked function body is absent from the Phase-A snapshot",
            );
        };
        if entry.parameter_sites.len() != arguments.len() {
            for argument in &arguments {
                self.mark_quantizers_uncertified(argument);
            }
            return self.unsupported(
                Some((site, expression)),
                UnsupportedResidualKind::AdapterIncomplete,
                "checked function parameter identities disagree with call arity",
            );
        }
        let token = format!(
            "fn:{}:{}:{:?}",
            callable.declaration.declaration.semantic_key(),
            callable.declaration.normalized_ordinal,
            callable.structural_path
        )
        .into_boxed_str();
        if self.call_stack.len() >= self.limits.max_call_depth.get()
            || self.call_stack.contains(&token)
        {
            for argument in &arguments {
                self.mark_quantizers_uncertified(argument);
            }
            return self.unsupported(
                Some((site, expression)),
                UnsupportedResidualKind::Recursion,
                "recursive or over-depth function specialization",
            );
        }
        let env = entry
            .parameter_sites
            .iter()
            .cloned()
            .zip(arguments)
            .collect::<AbstractEnv>();
        self.call_stack.push(token);
        let value = self.eval_site(&entry.body_site, &env);
        self.call_stack.pop();
        value
    }

    fn apply_callable(
        &mut self,
        callable: AbstractValue,
        arguments: Vec<AbstractValue>,
        site: &ExprSiteId,
        expression: &Expr,
    ) -> AbstractValue {
        match callable {
            AbstractValue::Callable(AbstractCallable::Lambda {
                body_site,
                parameters,
                mut captured,
            }) => {
                if captured.len().saturating_add(parameters.len())
                    > self.limits.max_collection_items.get()
                {
                    self.mark_abandoned_values_uncertified(arguments.iter());
                    self.mark_abandoned_values_uncertified(captured.values());
                    self.preserve_rejected_lambda_body(&body_site, &captured);
                    self.mark_all_quantizers_uncertified();
                    return self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::NonEnumerableDispatch,
                        "lambda capture environment exceeds the bounded specialization limit",
                    );
                }
                if parameters.len() != arguments.len() {
                    self.mark_abandoned_values_uncertified(arguments.iter());
                    self.mark_abandoned_values_uncertified(captured.values());
                    self.preserve_rejected_lambda_body(&body_site, &captured);
                    return self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::UnresolvedCallable,
                        "lambda arity mismatch during finite unrolling",
                    );
                }
                let token = format!(
                    "lambda:{}:{}:{:?}",
                    body_site.declaration.semantic_key(),
                    body_site.normalized_declaration_ordinal,
                    body_site.ast_path
                )
                .into_boxed_str();
                if self.call_stack.len() >= self.limits.max_call_depth.get()
                    || self.call_stack.contains(&token)
                    || !self.has_evaluation_budget()
                {
                    self.mark_abandoned_values_uncertified(arguments.iter());
                    self.mark_abandoned_values_uncertified(captured.values());
                    self.preserve_rejected_lambda_body(&body_site, &captured);
                    return self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::Recursion,
                        "recursive, over-depth, or over-budget lambda specialization",
                    );
                }
                for (parameter, argument) in parameters.iter().cloned().zip(arguments) {
                    captured.insert(parameter, argument);
                }
                self.call_stack.push(token);
                let value = self.eval_site(&body_site, &captured);
                self.call_stack.pop();
                value
            }
            AbstractValue::Callable(AbstractCallable::Function(callable)) => {
                self.eval_function(&callable, arguments, site, expression)
            }
            AbstractValue::Callable(AbstractCallable::RuleFamily(family)) => {
                self.eval_rule_family(&family, arguments, &AbstractEnv::new(), site, expression)
            }
            value => {
                self.mark_quantizers_uncertified(&value);
                for argument in &arguments {
                    self.mark_quantizers_uncertified(argument);
                }
                self.unsupported(
                    Some((site, expression)),
                    UnsupportedResidualKind::UnresolvedCallable,
                    "higher-order builtin received a non-callable exact value",
                )
            }
        }
    }

    fn eval_builtin(
        &mut self,
        name: &str,
        arity: usize,
        mut arguments: Vec<AbstractValue>,
        site: &ExprSiteId,
        expression: &Expr,
    ) -> AbstractValue {
        if arity != arguments.len() {
            for argument in &arguments {
                self.mark_quantizers_uncertified(argument);
            }
            return self.unsupported(
                Some((site, expression)),
                UnsupportedResidualKind::AdapterIncomplete,
                "checked builtin arity disagrees with canonical arguments",
            );
        }
        match (name, arity) {
            ("__named_arg", 2) => arguments.pop().unwrap(),
            ("__typed", 2) => arguments.remove(0),
            ("length", 1) => match arguments.remove(0) {
                AbstractValue::List(values) => {
                    AbstractValue::Int(IntTerm::constant(values.len() as i128))
                }
                value => {
                    self.mark_quantizers_uncertified(&value);
                    self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::UnsupportedType,
                        "length requires a finite known list",
                    )
                }
            },
            ("map", 2) => {
                let callable = arguments.pop().unwrap();
                let input = arguments.pop().unwrap();
                let AbstractValue::List(input) = input else {
                    self.mark_quantizers_uncertified(&input);
                    self.mark_quantizers_uncertified(&callable);
                    return self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::NonEnumerableDispatch,
                        "map requires a finite known list",
                    );
                };
                if input.len() > self.limits.max_collection_items.get() {
                    self.mark_quantizers_uncertified(&callable);
                    self.mark_abandoned_values_uncertified(
                        input.iter().take(self.limits.max_collection_items.get()),
                    );
                    self.mark_all_quantizers_uncertified();
                    return self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::NonEnumerableDispatch,
                        "map input exceeds the bounded unrolling limit",
                    );
                }
                let mut output = Vec::with_capacity(input.len());
                for (index, value) in input.iter().cloned().enumerate() {
                    if !self.has_evaluation_budget() {
                        for remaining in &input[index..] {
                            self.mark_quantizers_uncertified(remaining);
                        }
                        self.mark_quantizers_uncertified(&callable);
                        return self.unsupported(
                            Some((site, expression)),
                            UnsupportedResidualKind::AdapterIncomplete,
                            "map unrolling exhausted the abstract work budget",
                        );
                    }
                    output.push(self.apply_callable(
                        callable.clone(),
                        vec![value],
                        site,
                        expression,
                    ));
                }
                AbstractValue::List(output)
            }
            ("foldl", 3) => {
                let callable = arguments.pop().unwrap();
                let mut accumulator = arguments.pop().unwrap();
                let input = arguments.pop().unwrap();
                let AbstractValue::List(input) = input else {
                    self.mark_quantizers_uncertified(&input);
                    self.mark_quantizers_uncertified(&accumulator);
                    self.mark_quantizers_uncertified(&callable);
                    return self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::NonEnumerableDispatch,
                        "foldl requires a finite known list",
                    );
                };
                if input.len() > self.limits.max_collection_items.get() {
                    self.mark_quantizers_uncertified(&callable);
                    self.mark_quantizers_uncertified(&accumulator);
                    self.mark_abandoned_values_uncertified(
                        input.iter().take(self.limits.max_collection_items.get()),
                    );
                    self.mark_all_quantizers_uncertified();
                    return self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::NonEnumerableDispatch,
                        "foldl input exceeds the bounded unrolling limit",
                    );
                }
                for (index, value) in input.iter().cloned().enumerate() {
                    if !self.has_evaluation_budget() {
                        for remaining in &input[index..] {
                            self.mark_quantizers_uncertified(remaining);
                        }
                        self.mark_quantizers_uncertified(&accumulator);
                        self.mark_quantizers_uncertified(&callable);
                        return self.unsupported(
                            Some((site, expression)),
                            UnsupportedResidualKind::AdapterIncomplete,
                            "foldl unrolling exhausted the abstract work budget",
                        );
                    }
                    accumulator = self.apply_callable(
                        callable.clone(),
                        vec![accumulator, value],
                        site,
                        expression,
                    );
                }
                accumulator
            }
            ("all", 2) => {
                let callable = arguments.pop().unwrap();
                let input = arguments.pop().unwrap();
                let AbstractValue::List(input) = input else {
                    self.mark_quantizers_uncertified(&input);
                    self.mark_quantizers_uncertified(&callable);
                    return self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::NonEnumerableDispatch,
                        "all requires a finite known list",
                    );
                };
                if input.len() > self.limits.max_collection_items.get() {
                    self.mark_quantizers_uncertified(&callable);
                    self.mark_abandoned_values_uncertified(
                        input.iter().take(self.limits.max_collection_items.get()),
                    );
                    self.mark_all_quantizers_uncertified();
                    return self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::NonEnumerableDispatch,
                        "all input exceeds the bounded unrolling limit",
                    );
                }
                let mut predicates = Vec::with_capacity(input.len());
                for (index, value) in input.iter().cloned().enumerate() {
                    if !self.has_evaluation_budget() {
                        for remaining in &input[index..] {
                            self.mark_quantizers_uncertified(remaining);
                        }
                        self.mark_quantizers_uncertified(&callable);
                        return self.unsupported(
                            Some((site, expression)),
                            UnsupportedResidualKind::AdapterIncomplete,
                            "all unrolling exhausted the abstract work budget",
                        );
                    }
                    let result =
                        self.apply_callable(callable.clone(), vec![value], site, expression);
                    let Some(predicate) = as_predicate(&result) else {
                        self.mark_quantizers_uncertified(&result);
                        return self.unsupported(
                            Some((site, expression)),
                            UnsupportedResidualKind::UnsupportedType,
                            "all callback did not return a predicate",
                        );
                    };
                    predicates.push(predicate);
                }
                AbstractValue::Predicate(PredicateTerm::All(predicates))
            }
            _ => {
                for value in &arguments {
                    self.mark_quantizers_uncertified(value);
                }
                self.unsupported(
                    Some((site, expression)),
                    UnsupportedResidualKind::UnresolvedCallable,
                    format!("builtin `{name}`/{arity} is outside the bounded adapter whitelist"),
                )
            }
        }
    }

    fn eval_field(
        &mut self,
        site: &ExprSiteId,
        expression: &Expr,
        env: &AbstractEnv,
    ) -> AbstractValue {
        let base = self.eval_site(&self.child_site(site, 0), env);
        let field = self
            .resolution(site)
            .and_then(|resolution| resolution.field.clone());
        let Some(field) = field else {
            self.mark_quantizers_uncertified(&base);
            return self.unsupported(
                Some((site, expression)),
                UnsupportedResidualKind::UnsupportedType,
                "field access has no exact Phase-B field resolution",
            );
        };
        match (field, base) {
            (
                CheckedFieldResolution::Data { fields, .. },
                AbstractValue::Constructor(constructor),
            ) => {
                if fields.len() > self.limits.max_collection_items.get() {
                    self.mark_quantizers_uncertified(&AbstractValue::Constructor(
                        constructor.clone(),
                    ));
                    self.mark_all_quantizers_uncertified();
                    return self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::NonEnumerableDispatch,
                        "checked field alternatives exceed the bounded specialization limit",
                    );
                }
                let Some(identity) = constructor.identity.as_ref() else {
                    self.mark_quantizers_uncertified(&AbstractValue::Constructor(
                        constructor.clone(),
                    ));
                    return self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::UnsupportedType,
                        "field base has no producer-minted exact constructor identity",
                    );
                };
                let mut matching = fields.iter().filter(|field| {
                    field.identity.owner == identity.owner
                        && field.variant_index == identity.variant_index
                        && field.layout == identity.layout
                        && identity.fields.get(field.field_index) == Some(&field.identity)
                });
                let selected = matching.next();
                if matching.next().is_some() {
                    self.mark_quantizers_uncertified(&AbstractValue::Constructor(
                        constructor.clone(),
                    ));
                    return self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::AdapterIncomplete,
                        "checked data-field identity is not unique for the exact constructor",
                    );
                }
                let Some(selected) = selected else {
                    self.mark_quantizers_uncertified(&AbstractValue::Constructor(
                        constructor.clone(),
                    ));
                    return self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::UnsupportedType,
                        "checked data-field identity does not belong to the exact constructor",
                    );
                };
                constructor
                    .fields
                    .get(selected.field_index)
                    .cloned()
                    .unwrap_or_else(|| {
                        self.unsupported(
                            Some((site, expression)),
                            UnsupportedResidualKind::AdapterIncomplete,
                            "exact constructor value is missing its checked data field",
                        )
                    })
            }
            (CheckedFieldResolution::ScopedMember { .. }, value) => {
                self.mark_quantizers_uncertified(&value);
                self.unsupported(
                    Some((site, expression)),
                    UnsupportedResidualKind::UnresolvedCallable,
                    "first-class scoped members need a checked receiver-closure identity",
                )
            }
            (_, value) => {
                self.mark_quantizers_uncertified(&value);
                self.unsupported(
                    Some((site, expression)),
                    UnsupportedResidualKind::UnsupportedType,
                    "field base is not a specialized constructor",
                )
            }
        }
    }

    fn eval_block(
        &mut self,
        declaration: &SourcedStmt,
        site: &ExprSiteId,
        expression: &Expr,
        statements: &[Stmt],
        env: &AbstractEnv,
    ) -> AbstractValue {
        let mut local = env.clone();
        let mut result = AbstractValue::Ground(ExploreValue::Unit);
        let statement_limit = self.limits.max_collection_items.get();
        let oversized = statements.len() > statement_limit;
        for (statement_index, statement) in statements.iter().take(statement_limit).enumerate() {
            let statement_site = self.child_site(site, statement_index);
            match statement {
                Stmt::Bind(pattern, _, _) => {
                    let initializer_site = self.child_site(&statement_site, 0);
                    let value = self.eval_site(&initializer_site, &local);
                    self.bind_local_pattern(
                        declaration,
                        &statement_site,
                        pattern,
                        value,
                        &mut local,
                        expression,
                        0,
                    );
                }
                Stmt::Expr(_) => {
                    result = self.eval_site(&self.child_site(&statement_site, 0), &local);
                }
                Stmt::MonadicBind(_, _, _) => {
                    let initializer = self.eval_site(&self.child_site(&statement_site, 0), &local);
                    self.mark_quantizers_uncertified(&initializer);
                    result = self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::UnsupportedType,
                        "monadic bind is outside the first adapter slice",
                    );
                }
                Stmt::StreamBind(_, _) => {
                    let initializer = self.eval_site(&self.child_site(&statement_site, 0), &local);
                    self.mark_quantizers_uncertified(&initializer);
                    result = self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::Effect,
                        "stream bind is outside exact source-event adaptation",
                    );
                }
                _ => {
                    self.preserve_indexed_descendant_roots(
                        &statement_site,
                        &local,
                        source_site(site, expression),
                    );
                    result = self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::Effect,
                        "non-binding statement in an expression block is unsupported",
                    );
                }
            }
        }
        if oversized {
            let first_omitted = self.child_site(site, statement_limit);
            self.preserve_indexed_descendant_roots_from(
                site,
                first_omitted,
                &local,
                source_site(site, expression),
            );
            self.mark_quantizers_uncertified(&result);
            self.unsupported(
                Some((site, expression)),
                UnsupportedResidualKind::AdapterIncomplete,
                "expression block exceeds the bounded statement limit; supported descendants were retained through the admitted prefix",
            )
        } else {
            result
        }
    }

    fn bind_local_pattern(
        &mut self,
        declaration: &SourcedStmt,
        statement_site: &ExprSiteId,
        pattern: &Pat,
        value: AbstractValue,
        env: &mut AbstractEnv,
        expression: &Expr,
        depth: usize,
    ) {
        let matched = self.match_checked_pattern(
            declaration,
            statement_site,
            pattern,
            &value,
            &[],
            env,
            expression,
            depth,
        );
        if !matches!(matched, HeadMatch::Yes) {
            self.mark_quantizers_uncertified(&value);
            self.residual(
                Some(source_site(statement_site, expression)),
                UnsupportedResidualKind::UnsupportedType,
                "local binding pattern could not be proved against exact checked constructor identities",
            );
        }
    }

    fn eval_match(
        &mut self,
        declaration: &SourcedStmt,
        site: &ExprSiteId,
        expression: &Expr,
        env: &AbstractEnv,
    ) -> AbstractValue {
        let ExprKind::Match(_, arms) = &expression.kind else {
            unreachable!();
        };
        let scrutinee = self.eval_site(&self.child_site(site, 0), env);
        let mut child_index = 1_usize;
        let mut results = Vec::new();
        let arm_limit = self.limits.max_collection_items.get();
        let oversized = arms.len() > arm_limit;
        for (arm_index, arm) in arms.iter().take(arm_limit).enumerate() {
            let mut arm_env = env.clone();
            let Ok(arm_index_u32) = u32::try_from(arm_index) else {
                self.mark_quantizers_uncertified(&scrutinee);
                self.mark_all_quantizers_uncertified();
                return self.unsupported(
                    Some((site, expression)),
                    UnsupportedResidualKind::AdapterIncomplete,
                    "match arm identity exceeds u32 structural space",
                );
            };
            let pattern_match = self.match_checked_pattern(
                declaration,
                site,
                &arm.pat,
                &scrutinee,
                &[arm_index_u32],
                &mut arm_env,
                expression,
                0,
            );
            if matches!(pattern_match, HeadMatch::No) {
                child_index = child_index
                    .saturating_add(arm.guard.is_some() as usize)
                    .saturating_add(1);
                continue;
            }
            let mut definitive = matches!(pattern_match, HeadMatch::Yes);
            if arm.guard.is_some() {
                let guard = self.eval_site(&self.child_site(site, child_index), &arm_env);
                child_index += 1;
                if as_ground_bool(&guard) == Some(false) {
                    child_index += 1;
                    continue;
                }
                if as_ground_bool(&guard).is_none() {
                    self.mark_quantizers_uncertified(&guard);
                    definitive = false;
                }
            }
            let result = self.eval_site(&self.child_site(site, child_index), &arm_env);
            child_index += 1;
            if definitive {
                return result;
            }
            results.push(result);
        }
        if oversized {
            self.preserve_indexed_descendant_roots_from(
                site,
                self.child_site(site, child_index),
                env,
                source_site(site, expression),
            );
            self.mark_quantizers_uncertified(&scrutinee);
            for result in &results {
                self.mark_quantizers_uncertified(result);
            }
            return self.unsupported(
                Some((site, expression)),
                UnsupportedResidualKind::AdapterIncomplete,
                "match exceeds the bounded arm limit; supported descendants were retained through the admitted prefix",
            );
        }
        let joined = join_abstract_values(results.clone());
        joined.unwrap_or_else(|| {
            self.mark_quantizers_uncertified(&scrutinee);
            for result in &results {
                self.mark_quantizers_uncertified(result);
            }
            self.unsupported(
                Some((site, expression)),
                UnsupportedResidualKind::NonEnumerableDispatch,
                "match arms do not produce one exact abstract value",
            )
        })
    }

    fn match_checked_pattern(
        &mut self,
        declaration: &SourcedStmt,
        pattern_site_root: &ExprSiteId,
        pattern: &Pat,
        value: &AbstractValue,
        pattern_path: &[u32],
        env: &mut AbstractEnv,
        expression: &Expr,
        depth: usize,
    ) -> HeadMatch {
        if depth >= self.limits.max_call_depth.get() {
            self.mark_quantizers_uncertified(value);
            self.residual(
                Some(source_site(pattern_site_root, expression)),
                UnsupportedResidualKind::Recursion,
                "checked pattern exceeds the bounded pattern depth",
            );
            return HeadMatch::Unknown;
        }
        match pattern {
            Pat::Var(_) => {
                let mut binder_path = vec![BINDER_PATTERN];
                binder_path.extend_from_slice(pattern_path);
                env.insert(
                    structural_binder_site(
                        self.index.program_id,
                        declaration,
                        &pattern_site_root.ast_path,
                        binder_path,
                    ),
                    value.clone(),
                );
                HeadMatch::Yes
            }
            Pat::As(inner, _) => {
                let matched = self.match_checked_pattern(
                    declaration,
                    pattern_site_root,
                    inner,
                    value,
                    pattern_path,
                    env,
                    expression,
                    depth.saturating_add(1),
                );
                if matches!(matched, HeadMatch::No) {
                    return HeadMatch::No;
                }
                let mut binder_path = vec![BINDER_PATTERN];
                binder_path.extend_from_slice(pattern_path);
                binder_path.push(u32::MAX);
                env.insert(
                    structural_binder_site(
                        self.index.program_id,
                        declaration,
                        &pattern_site_root.ast_path,
                        binder_path,
                    ),
                    value.clone(),
                );
                matched
            }
            Pat::Wild => HeadMatch::Yes,
            Pat::Lit(literal) => match abstract_ground_equal(&literal_value(literal), value) {
                Some(true) => HeadMatch::Yes,
                Some(false) => HeadMatch::No,
                None => HeadMatch::Unknown,
            },
            Pat::Con(_, _) | Pat::NamedCon(_, _) => {
                let checked_site = CheckedPatternSiteId {
                    analysis_program: pattern_site_root.analysis_program.clone(),
                    declaration: pattern_site_root.declaration.clone(),
                    normalized_declaration_ordinal: pattern_site_root
                        .normalized_declaration_ordinal,
                    ast_path: pattern_site_root.ast_path.clone(),
                    pattern_path: pattern_path.to_vec().into_boxed_slice(),
                };
                let Some(checked) = self
                    .artifacts
                    .checked_resolutions
                    .constructor_patterns
                    .get(&checked_site)
                    .cloned()
                else {
                    self.mark_quantizers_uncertified(value);
                    self.residual(
                        Some(source_site(pattern_site_root, expression)),
                        UnsupportedResidualKind::AdapterIncomplete,
                        "constructor pattern has no producer-minted exact identity",
                    );
                    return HeadMatch::Unknown;
                };
                let AbstractValue::Constructor(constructor) = value else {
                    return if matches!(value, AbstractValue::Unsupported(_)) {
                        HeadMatch::Unknown
                    } else {
                        HeadMatch::No
                    };
                };
                let Some(identity) = constructor.identity.as_ref() else {
                    return HeadMatch::Unknown;
                };
                if identity.as_ref() != &checked.constructor {
                    return HeadMatch::No;
                }
                let child_patterns = match pattern {
                    Pat::Con(_, children) => children.iter().collect::<Vec<_>>(),
                    Pat::NamedCon(_, fields) => {
                        fields.iter().map(|(_, child)| child).collect::<Vec<_>>()
                    }
                    _ => unreachable!(),
                };
                if checked.source_fields.len() != child_patterns.len()
                    || identity.fields.len() != constructor.fields.len()
                {
                    return HeadMatch::Unknown;
                }
                let mut unknown = false;
                for (source_index, (field, child_pattern)) in
                    checked.source_fields.iter().zip(child_patterns).enumerate()
                {
                    let Some(canonical_index) = identity
                        .fields
                        .iter()
                        .position(|candidate| candidate == field)
                    else {
                        return HeadMatch::Unknown;
                    };
                    let Some(child_value) = constructor.fields.get(canonical_index) else {
                        return HeadMatch::Unknown;
                    };
                    let Ok(source_index) = u32::try_from(source_index) else {
                        return HeadMatch::Unknown;
                    };
                    let mut child_path = pattern_path.to_vec();
                    child_path.push(source_index);
                    match self.match_checked_pattern(
                        declaration,
                        pattern_site_root,
                        child_pattern,
                        child_value,
                        &child_path,
                        env,
                        expression,
                        depth.saturating_add(1),
                    ) {
                        HeadMatch::No => return HeadMatch::No,
                        HeadMatch::Unknown => unknown = true,
                        HeadMatch::Yes => {}
                    }
                }
                if unknown {
                    HeadMatch::Unknown
                } else {
                    HeadMatch::Yes
                }
            }
        }
    }
}

fn join_abstract_values(mut values: Vec<AbstractValue>) -> Option<AbstractValue> {
    let first = values.pop()?;
    if values
        .iter()
        .all(|value| abstract_ground_equal(&first, value) == Some(true))
    {
        Some(first)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy)]
enum HeadMatch {
    No,
    Yes,
    Unknown,
}

struct PendingRuleBranch {
    condition: PredicateTerm,
    value: AbstractValue,
}

impl AdapterContext<'_, '_> {
    fn eval_rule_family(
        &mut self,
        family: &RuleDispatchKey,
        arguments: Vec<AbstractValue>,
        captures: &AbstractEnv,
        site: &ExprSiteId,
        expression: &Expr,
    ) -> AbstractValue {
        if family.arity != arguments.len() {
            for argument in &arguments {
                self.mark_quantizers_uncertified(argument);
            }
            return self.unsupported(
                Some((site, expression)),
                UnsupportedResidualKind::AdapterIncomplete,
                "checked rule-family arity disagrees with call arguments",
            );
        }
        let token = format!(
            "rule:{}:{}:{}",
            family.scope.as_deref().unwrap_or(""),
            family.name,
            family.arity
        )
        .into_boxed_str();
        if self.call_stack.len() >= self.limits.max_call_depth.get()
            || self.call_stack.contains(&token)
        {
            for argument in &arguments {
                self.mark_quantizers_uncertified(argument);
            }
            return self.unsupported(
                Some((site, expression)),
                UnsupportedResidualKind::Recursion,
                "recursive or over-depth rule specialization",
            );
        }
        let Some(resolution) = self
            .artifacts
            .checked_resolutions
            .rule_families
            .get(family)
            .cloned()
        else {
            for argument in &arguments {
                self.mark_quantizers_uncertified(argument);
            }
            return self.unsupported(
                Some((site, expression)),
                UnsupportedResidualKind::UnresolvedCallable,
                "checked rule-family target is missing",
            );
        };
        self.call_stack.push(token);
        let mut pending: Option<PendingRuleBranch> = None;
        let mut matched_false_clause = false;
        for candidate in resolution.candidates.iter() {
            if !self.charge(Some(source_site(site, expression))) {
                self.call_stack.pop();
                for argument in &arguments {
                    self.mark_quantizers_uncertified(argument);
                }
                return self.unsupported(
                    Some((site, expression)),
                    UnsupportedResidualKind::AdapterIncomplete,
                    "rule specialization exhausted its bounded work budget",
                );
            }
            let mut candidate_env = captures.clone();
            let head_match = self.bind_rule_head(
                &candidate.head_site,
                &arguments,
                &mut candidate_env,
                site,
                expression,
            );
            match head_match {
                HeadMatch::No => continue,
                HeadMatch::Unknown => {
                    self.call_stack.pop();
                    for argument in &arguments {
                        self.mark_quantizers_uncertified(argument);
                    }
                    return self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::NonEnumerableDispatch,
                        "rule-head applicability is not exactly decidable from checked constructor and binder identities",
                    );
                }
                HeadMatch::Yes => {}
            }
            let condition = candidate
                .condition_site
                .as_ref()
                .map(|condition| self.eval_site(condition, &candidate_env));
            if condition.as_ref().and_then(as_ground_bool) == Some(false) {
                continue;
            }
            let value = match &candidate.value_site {
                Some(value) => self.eval_site(value, &candidate_env),
                None => AbstractValue::Ground(ExploreValue::Boolean(true)),
            };

            if let Some(condition) = condition {
                if as_ground_bool(&condition) != Some(true) {
                    let Some(predicate) = as_predicate(&condition) else {
                        self.call_stack.pop();
                        self.mark_quantizers_uncertified(&condition);
                        self.mark_quantizers_uncertified(&value);
                        return self.unsupported(
                            Some((site, expression)),
                            UnsupportedResidualKind::NonEnumerableDispatch,
                            "rule condition is not an exact predicate",
                        );
                    };
                    if let Some(prior) = pending.take() {
                        self.call_stack.pop();
                        self.mark_quantizers_uncertified(&AbstractValue::Predicate(
                            prior.condition,
                        ));
                        self.mark_quantizers_uncertified(&prior.value);
                        self.mark_quantizers_uncertified(&condition);
                        self.mark_quantizers_uncertified(&value);
                        return self.unsupported(
                            Some((site, expression)),
                            UnsupportedResidualKind::NonEnumerableDispatch,
                            "more than one ordered rule condition remains axis-dependent",
                        );
                    }
                    pending = Some(PendingRuleBranch {
                        condition: predicate,
                        value,
                    });
                    continue;
                }
            }

            if matches!(candidate.tier, crate::RuleDispatchTier::Clause) {
                if as_ground_bool(&value) == Some(false) {
                    matched_false_clause = true;
                    continue;
                }
                if matches!(&value, AbstractValue::Predicate(predicate) if predicate.constant().is_none())
                {
                    self.call_stack.pop();
                    if let Some(prior) = pending {
                        self.mark_quantizers_uncertified(&AbstractValue::Predicate(
                            prior.condition,
                        ));
                        self.mark_quantizers_uncertified(&prior.value);
                    }
                    self.mark_quantizers_uncertified(&value);
                    return self.unsupported(
                        Some((site, expression)),
                        UnsupportedResidualKind::NonEnumerableDispatch,
                        "symbolic clause success would require exact backtracking semantics",
                    );
                }
            }

            self.call_stack.pop();
            return match pending {
                Some(branch) => self.combine_rule_branch(
                    branch.condition,
                    branch.value,
                    value,
                    site,
                    expression,
                ),
                None => value,
            };
        }
        self.call_stack.pop();

        if let Some(branch) = pending {
            if matched_false_clause
                || self
                    .artifacts
                    .rule_dispatch_boolean_miss_safe_keys
                    .contains(family)
            {
                return self.combine_rule_branch(
                    branch.condition,
                    branch.value,
                    AbstractValue::Ground(ExploreValue::Boolean(false)),
                    site,
                    expression,
                );
            }
            self.mark_quantizers_uncertified(&AbstractValue::Predicate(branch.condition));
            self.mark_quantizers_uncertified(&branch.value);
            return self.unsupported(
                Some((site, expression)),
                UnsupportedResidualKind::NonEnumerableDispatch,
                "axis-dependent rule has no exact fallback value",
            );
        }
        if matched_false_clause
            || self
                .artifacts
                .rule_dispatch_boolean_miss_safe_keys
                .contains(family)
        {
            AbstractValue::Ground(ExploreValue::Boolean(false))
        } else {
            self.unsupported(
                Some((site, expression)),
                UnsupportedResidualKind::NonEnumerableDispatch,
                "rule family has no exactly applicable candidate or certified Boolean miss",
            )
        }
    }

    fn combine_rule_branch(
        &mut self,
        condition: PredicateTerm,
        then_value: AbstractValue,
        else_value: AbstractValue,
        site: &ExprSiteId,
        expression: &Expr,
    ) -> AbstractValue {
        let predicate = simplify_predicate(condition);
        if let Some(value) =
            self.recognize_positive(&predicate, &then_value, &else_value, site, expression)
        {
            return value;
        }
        if let Some(value) =
            self.recognize_min_max(&predicate, &then_value, &else_value, site, expression)
        {
            return value;
        }
        if abstract_ground_equal(&then_value, &else_value) == Some(true) {
            return then_value;
        }
        self.mark_quantizers_uncertified(&AbstractValue::Predicate(predicate));
        self.mark_quantizers_uncertified(&then_value);
        self.mark_quantizers_uncertified(&else_value);
        self.unsupported(
            Some((site, expression)),
            UnsupportedResidualKind::NonEnumerableDispatch,
            "ordered rule dispatch remains axis-dependent outside positive/min/max normalization",
        )
    }

    fn bind_rule_head(
        &mut self,
        head_site: &ExprSiteId,
        arguments: &[AbstractValue],
        env: &mut AbstractEnv,
        call_site: &ExprSiteId,
        call_expression: &Expr,
    ) -> HeadMatch {
        let Some(indexed) = self.index.expression(head_site) else {
            self.residual(
                Some(source_site(call_site, call_expression)),
                UnsupportedResidualKind::AdapterIncomplete,
                "checked rule candidate head is absent from Phase A",
            );
            return HeadMatch::Unknown;
        };
        let ExprKind::App(_, head_arguments) = &indexed.expression.kind else {
            self.residual(
                Some(source_site(head_site, indexed.expression)),
                UnsupportedResidualKind::AdapterIncomplete,
                "checked rule candidate head is not an application",
            );
            return HeadMatch::Unknown;
        };
        if head_arguments.len() != arguments.len() {
            return HeadMatch::No;
        }
        let mut unknown = false;
        for (index, value) in arguments.iter().enumerate() {
            let argument_site = self.child_site(head_site, index + 1);
            match self.bind_rule_argument(&argument_site, value, env) {
                HeadMatch::No => return HeadMatch::No,
                HeadMatch::Unknown => unknown = true,
                HeadMatch::Yes => {}
            }
        }
        if unknown {
            HeadMatch::Unknown
        } else {
            HeadMatch::Yes
        }
    }

    fn bind_rule_argument(
        &mut self,
        argument_site: &ExprSiteId,
        value: &AbstractValue,
        env: &mut AbstractEnv,
    ) -> HeadMatch {
        let Some(indexed) = self.index.expression(argument_site) else {
            return HeadMatch::Unknown;
        };
        let resolution = self.resolution(argument_site).cloned();
        match &indexed.expression.kind {
            ExprKind::Var(_) => {
                let Some(resolution) = resolution else {
                    return HeadMatch::Unknown;
                };
                match resolution.value_binding {
                    Some(CheckedValueBinding::Binder {
                        kind: CheckedBinderKind::RuleHead,
                        site: binder,
                    }) => {
                        env.insert(binder, value.clone());
                        HeadMatch::Yes
                    }
                    Some(CheckedValueBinding::Constructor { .. }) => {
                        let Some(pattern_identity) = resolution.exact_constructor.as_ref() else {
                            return HeadMatch::Unknown;
                        };
                        let AbstractValue::Constructor(constructor) = value else {
                            return if matches!(value, AbstractValue::Unsupported(_)) {
                                HeadMatch::Unknown
                            } else {
                                HeadMatch::No
                            };
                        };
                        let Some(value_identity) = constructor.identity.as_ref() else {
                            return HeadMatch::Unknown;
                        };
                        if value_identity.as_ref() != pattern_identity {
                            HeadMatch::No
                        } else if constructor.fields.is_empty()
                            && pattern_identity.fields.is_empty()
                        {
                            HeadMatch::Yes
                        } else {
                            HeadMatch::No
                        }
                    }
                    _ => HeadMatch::Unknown,
                }
            }
            ExprKind::Lit(literal) => match abstract_ground_equal(&literal_value(literal), value) {
                Some(true) => HeadMatch::Yes,
                Some(false) => HeadMatch::No,
                None => HeadMatch::Unknown,
            },
            ExprKind::App(_, arguments) => {
                let Some(resolution) = resolution else {
                    return HeadMatch::Unknown;
                };
                let exact_constructor = resolution.exact_constructor.clone();
                match resolution.call_target {
                    Some(CheckedCallTarget::Builtin { canonical_name, .. })
                        if canonical_name.as_ref() == "__typed" =>
                    {
                        self.bind_rule_argument(&self.child_site(argument_site, 1), value, env)
                    }
                    Some(CheckedCallTarget::Builtin { canonical_name, .. })
                        if canonical_name.as_ref() == "__named_arg" =>
                    {
                        self.bind_rule_argument(&self.child_site(argument_site, 2), value, env)
                    }
                    Some(CheckedCallTarget::Constructor { .. }) => {
                        let Some(pattern_identity) = exact_constructor.as_ref() else {
                            return HeadMatch::Unknown;
                        };
                        let AbstractValue::Constructor(constructor) = value else {
                            return if matches!(value, AbstractValue::Unsupported(_)) {
                                HeadMatch::Unknown
                            } else {
                                HeadMatch::No
                            };
                        };
                        let Some(value_identity) = constructor.identity.as_ref() else {
                            return HeadMatch::Unknown;
                        };
                        if value_identity.as_ref() != pattern_identity {
                            return HeadMatch::No;
                        }
                        let Some(order) =
                            reordered_arguments(arguments, resolution.named_arguments.as_ref())
                        else {
                            return HeadMatch::Unknown;
                        };
                        if order.len() != constructor.fields.len() {
                            return HeadMatch::No;
                        }
                        let mut unknown = false;
                        for (canonical_index, source_index) in order.into_iter().enumerate() {
                            match self.bind_rule_argument(
                                &self.child_site(argument_site, source_index + 1),
                                &constructor.fields[canonical_index],
                                env,
                            ) {
                                HeadMatch::No => return HeadMatch::No,
                                HeadMatch::Unknown => unknown = true,
                                HeadMatch::Yes => {}
                            }
                        }
                        if unknown {
                            HeadMatch::Unknown
                        } else {
                            HeadMatch::Yes
                        }
                    }
                    _ => HeadMatch::Unknown,
                }
            }
            ExprKind::Tuple(items) => {
                let AbstractValue::Tuple(values) = value else {
                    return HeadMatch::Unknown;
                };
                if items.len() != values.len() {
                    return HeadMatch::No;
                }
                let mut unknown = false;
                for (index, value) in values.iter().enumerate() {
                    match self.bind_rule_argument(
                        &self.child_site(argument_site, index),
                        value,
                        env,
                    ) {
                        HeadMatch::No => return HeadMatch::No,
                        HeadMatch::Unknown => unknown = true,
                        HeadMatch::Yes => {}
                    }
                }
                if unknown {
                    HeadMatch::Unknown
                } else {
                    HeadMatch::Yes
                }
            }
            _ => HeadMatch::Unknown,
        }
    }
}

fn simplify_predicate(predicate: PredicateTerm) -> PredicateTerm {
    match predicate {
        PredicateTerm::All(parts) => {
            let mut unresolved = parts
                .into_iter()
                .map(simplify_predicate)
                .filter(|part| part.constant() != Some(true))
                .collect::<Vec<_>>();
            if unresolved.iter().any(|part| part.constant() == Some(false)) {
                PredicateTerm::Constant(false)
            } else if unresolved.len() == 1 {
                unresolved.pop().unwrap()
            } else {
                PredicateTerm::All(unresolved)
            }
        }
        PredicateTerm::Any(parts) => {
            let mut unresolved = parts
                .into_iter()
                .map(simplify_predicate)
                .filter(|part| part.constant() != Some(false))
                .collect::<Vec<_>>();
            if unresolved.iter().any(|part| part.constant() == Some(true)) {
                PredicateTerm::Constant(true)
            } else if unresolved.len() == 1 {
                unresolved.pop().unwrap()
            } else {
                PredicateTerm::Any(unresolved)
            }
        }
        PredicateTerm::Not(inner) => {
            let inner = simplify_predicate(*inner);
            match inner.constant() {
                Some(value) => PredicateTerm::Constant(!value),
                None => PredicateTerm::Not(Box::new(inner)),
            }
        }
        predicate => predicate,
    }
}

#[derive(Clone, Copy)]
enum FiniteTypeAccessLimit {
    Work(usize),
    Depth(usize),
    Width(usize),
}

struct FiniteTypeAccessBudget {
    work: usize,
    work_limit: usize,
    depth_limit: usize,
    width_limit: usize,
}

impl FiniteTypeAccessBudget {
    fn new(limits: ResolvedEventAdapterLimits) -> Self {
        Self {
            work: 0,
            work_limit: limits.max_abstract_steps.get(),
            depth_limit: limits.max_call_depth.get(),
            width_limit: limits.max_collection_items.get().min(u32::MAX as usize),
        }
    }

    fn charge(&mut self, depth: usize) -> Result<(), FiniteTypeAccessLimit> {
        if depth >= self.depth_limit {
            return Err(FiniteTypeAccessLimit::Depth(self.depth_limit));
        }
        if self.work >= self.work_limit {
            return Err(FiniteTypeAccessLimit::Work(self.work_limit));
        }
        self.work += 1;
        Ok(())
    }

    fn admit_width(&self, width: usize) -> Result<(), FiniteTypeAccessLimit> {
        if width > self.width_limit {
            Err(FiniteTypeAccessLimit::Width(self.width_limit))
        } else {
            Ok(())
        }
    }

    fn remaining_work(&self) -> usize {
        self.work_limit.saturating_sub(self.work)
    }
}

fn preflight_finite_plan(
    plan: &ExploreFiniteTypePlan,
    budget: &mut FiniteTypeAccessBudget,
) -> Result<(), FiniteTypeAccessLimit> {
    let mut pending = vec![(plan, 0_usize)];
    while let Some((plan, depth)) = pending.pop() {
        budget.charge(depth)?;
        match plan {
            ExploreFiniteTypePlan::Unit | ExploreFiniteTypePlan::Bool => {}
            ExploreFiniteTypePlan::Tuple { elements, .. } => {
                budget.admit_width(elements.len())?;
                if pending.len().saturating_add(elements.len()) > budget.remaining_work() {
                    return Err(FiniteTypeAccessLimit::Work(budget.work_limit));
                }
                pending.extend(
                    elements
                        .iter()
                        .rev()
                        .map(|element| (element, depth.saturating_add(1))),
                );
            }
            ExploreFiniteTypePlan::Sum {
                type_name,
                variants,
                ..
            } => {
                budget.admit_width(type_name.len())?;
                budget.admit_width(variants.len())?;
                let child_count = variants.iter().try_fold(0_usize, |count, variant| {
                    budget.admit_width(variant.name.len())?;
                    budget.admit_width(variant.fields.len())?;
                    for field in variant.fields.iter() {
                        budget.admit_width(field.name.len())?;
                    }
                    Ok::<_, FiniteTypeAccessLimit>(count.saturating_add(variant.fields.len()))
                })?;
                if pending.len().saturating_add(child_count) > budget.remaining_work() {
                    return Err(FiniteTypeAccessLimit::Work(budget.work_limit));
                }
                for variant in variants.iter().rev() {
                    pending.extend(
                        variant
                            .fields
                            .iter()
                            .rev()
                            .map(|field| (&field.plan, depth.saturating_add(1))),
                    );
                }
            }
        }
    }
    Ok(())
}

fn finite_plan_component_ordinals(
    plans: &[&ExploreFiniteTypePlan],
    ordinal: u128,
    budget: &mut FiniteTypeAccessBudget,
    depth: usize,
) -> Result<Vec<u128>, FiniteTypeAccessLimit> {
    budget.admit_width(plans.len())?;
    let mut suffix_products = vec![1_u128; plans.len().saturating_add(1)];
    for index in (0..plans.len()).rev() {
        budget.charge(depth)?;
        let cardinality = plans[index]
            .cardinality()
            .exact()
            .ok_or(FiniteTypeAccessLimit::Work(budget.work_limit))?;
        suffix_products[index] = cardinality
            .checked_mul(suffix_products[index + 1])
            .ok_or(FiniteTypeAccessLimit::Work(budget.work_limit))?;
    }
    if ordinal >= suffix_products[0] {
        return Err(FiniteTypeAccessLimit::Work(budget.work_limit));
    }
    Ok(plans
        .iter()
        .enumerate()
        .map(|(index, plan)| {
            let cardinality = plan.cardinality().exact().unwrap_or(0);
            if cardinality == 0 {
                0
            } else {
                (ordinal / suffix_products[index + 1]) % cardinality
            }
        })
        .collect())
}

fn finite_plan_value_at(
    plan: &ExploreFiniteTypePlan,
    ordinal: u128,
    budget: &mut FiniteTypeAccessBudget,
    depth: usize,
    dimension_index: usize,
    plan_path: &mut Vec<u32>,
    constructors: &CheckedGroundConstructorIndex,
) -> Result<AbstractValue, FiniteTypeAccessLimit> {
    budget.charge(depth)?;
    let cardinality = plan
        .cardinality()
        .exact()
        .ok_or(FiniteTypeAccessLimit::Work(budget.work_limit))?;
    if ordinal >= cardinality {
        return Err(FiniteTypeAccessLimit::Work(budget.work_limit));
    }
    match plan {
        ExploreFiniteTypePlan::Unit => Ok(AbstractValue::Ground(ExploreValue::Unit)),
        ExploreFiniteTypePlan::Bool => {
            Ok(AbstractValue::Ground(ExploreValue::Boolean(ordinal == 1)))
        }
        ExploreFiniteTypePlan::Tuple { elements, .. } => {
            budget.admit_width(elements.len())?;
            let plans = elements.iter().collect::<Vec<_>>();
            let ordinals =
                finite_plan_component_ordinals(&plans, ordinal, budget, depth.saturating_add(1))?;
            let mut values = Vec::with_capacity(elements.len());
            for (element_index, (element, ordinal)) in elements.iter().zip(ordinals).enumerate() {
                let element_index = u32::try_from(element_index)
                    .map_err(|_| FiniteTypeAccessLimit::Width(budget.width_limit))?;
                plan_path.extend([0, element_index]);
                let value = finite_plan_value_at(
                    element,
                    ordinal,
                    budget,
                    depth.saturating_add(1),
                    dimension_index,
                    plan_path,
                    constructors,
                );
                plan_path.truncate(plan_path.len().saturating_sub(2));
                values.push(value?);
            }
            Ok(AbstractValue::Tuple(values))
        }
        ExploreFiniteTypePlan::Sum {
            type_name,
            variants,
            ..
        } => {
            budget.admit_width(type_name.len())?;
            budget.admit_width(variants.len())?;
            let mut remaining = ordinal;
            for (variant_index, variant) in variants.iter().enumerate() {
                budget.charge(depth.saturating_add(1))?;
                budget.admit_width(variant.name.len())?;
                budget.admit_width(variant.fields.len())?;
                if variant
                    .fields
                    .iter()
                    .any(|field| field.name.len() > budget.width_limit)
                {
                    return Err(FiniteTypeAccessLimit::Width(budget.width_limit));
                }
                let plans = variant
                    .fields
                    .iter()
                    .map(|field| &field.plan)
                    .collect::<Vec<_>>();
                let variant_cardinality = plans.iter().try_fold(1_u128, |product, plan| {
                    budget.charge(depth.saturating_add(1))?;
                    let cardinality = plan
                        .cardinality()
                        .exact()
                        .ok_or(FiniteTypeAccessLimit::Work(budget.work_limit))?;
                    product
                        .checked_mul(cardinality)
                        .ok_or(FiniteTypeAccessLimit::Work(budget.work_limit))
                })?;
                if remaining >= variant_cardinality {
                    remaining -= variant_cardinality;
                    continue;
                }
                let ordinals = finite_plan_component_ordinals(
                    &plans,
                    remaining,
                    budget,
                    depth.saturating_add(1),
                )?;
                let identity = constructors
                    .get(&CheckedExploreGroundConstructorSite::FiniteTypeVariant {
                        dimension_index,
                        plan_path: plan_path.clone().into_boxed_slice(),
                        plan_variant_index: variant_index,
                    })
                    .cloned()
                    .filter(|identity| {
                        identity.fields.len() == variant.fields.len()
                            && matches!(
                                (identity.layout, variant.positional),
                                (CheckedConstructorLayout::Positional, true)
                                    | (CheckedConstructorLayout::Named, false)
                            )
                    });
                let mut fields = Vec::with_capacity(variant.fields.len());
                for (field_index, (field, ordinal)) in
                    variant.fields.iter().zip(ordinals).enumerate()
                {
                    let field_index = u32::try_from(field_index)
                        .map_err(|_| FiniteTypeAccessLimit::Width(budget.width_limit))?;
                    let variant_path_index = u32::try_from(variant_index)
                        .map_err(|_| FiniteTypeAccessLimit::Width(budget.width_limit))?;
                    plan_path.extend([1, variant_path_index, field_index]);
                    let value = finite_plan_value_at(
                        &field.plan,
                        ordinal,
                        budget,
                        depth.saturating_add(1),
                        dimension_index,
                        plan_path,
                        constructors,
                    );
                    plan_path.truncate(plan_path.len().saturating_sub(3));
                    fields.push(value?);
                }
                return Ok(AbstractValue::Constructor(AbstractConstructor {
                    identity,
                    fields,
                }));
            }
            Err(FiniteTypeAccessLimit::Work(budget.work_limit))
        }
    }
}

fn finite_access_error(
    dimension: &str,
    failure: FiniteTypeAccessLimit,
) -> ResolvedEventAdapterError {
    let (resource, limit) = match failure {
        FiniteTypeAccessLimit::Work(limit) => ("work", limit),
        FiniteTypeAccessLimit::Depth(limit) => ("nesting-depth", limit),
        FiniteTypeAccessLimit::Width(limit) => ("structural-width", limit),
    };
    ResolvedEventAdapterError::OuterProfileAccessLimit {
        dimension: dimension.to_string().into_boxed_str(),
        resource,
        limit,
    }
}

fn ensure_explore_value_bounded(
    value: &ExploreValue,
    budget: &mut FiniteTypeAccessBudget,
) -> Result<(), FiniteTypeAccessLimit> {
    let mut pending = vec![(value, 0_usize)];
    while let Some((value, depth)) = pending.pop() {
        budget.charge(depth)?;
        let children: &[ExploreValue] = match value {
            ExploreValue::List(values)
            | ExploreValue::Set(values)
            | ExploreValue::Tuple(values) => values,
            ExploreValue::Constructor {
                type_name,
                variant,
                fields,
                ..
            } => {
                if type_name.len() > budget.width_limit
                    || variant.len() > budget.width_limit
                    || fields.len() > budget.width_limit
                    || fields
                        .iter()
                        .any(|(name, _)| name.len() > budget.width_limit)
                {
                    return Err(FiniteTypeAccessLimit::Width(budget.width_limit));
                }
                if pending.len().saturating_add(fields.len()) > budget.remaining_work() {
                    return Err(FiniteTypeAccessLimit::Work(budget.work_limit));
                }
                for (_, value) in fields.iter().rev() {
                    pending.push((value, depth.saturating_add(1)));
                }
                continue;
            }
            ExploreValue::String(value) => {
                if value.len() > budget.width_limit {
                    return Err(FiniteTypeAccessLimit::Width(budget.width_limit));
                }
                continue;
            }
            ExploreValue::Int(_)
            | ExploreValue::FloatBits(_)
            | ExploreValue::Character(_)
            | ExploreValue::Boolean(_)
            | ExploreValue::Unit => continue,
        };
        if children.len() > budget.width_limit {
            return Err(FiniteTypeAccessLimit::Width(budget.width_limit));
        }
        if pending.len().saturating_add(children.len()) > budget.remaining_work() {
            return Err(FiniteTypeAccessLimit::Work(budget.work_limit));
        }
        for child in children.iter().rev() {
            pending.push((child, depth.saturating_add(1)));
        }
    }
    Ok(())
}

enum DomainValueAt {
    Exact(AbstractValue),
    MissingIdentity(Box<str>),
}

fn has_missing_constructor_identity(value: &AbstractValue) -> bool {
    match value {
        AbstractValue::Constructor(constructor) => {
            constructor.identity.is_none()
                || constructor
                    .fields
                    .iter()
                    .any(has_missing_constructor_identity)
        }
        AbstractValue::List(values) | AbstractValue::Set(values) | AbstractValue::Tuple(values) => {
            values.iter().any(has_missing_constructor_identity)
        }
        AbstractValue::Ground(_)
        | AbstractValue::Int(_)
        | AbstractValue::Predicate(_)
        | AbstractValue::Callable(_)
        | AbstractValue::Unsupported(_) => false,
    }
}

fn domain_value_at(
    domain: &ExploreExactDomain,
    ordinal: u128,
    dimension_index: usize,
    dimension: &str,
    budget: &mut FiniteTypeAccessBudget,
    constructors: &CheckedGroundConstructorIndex,
) -> Result<DomainValueAt, ResolvedEventAdapterError> {
    let cardinality = domain.cardinality().exact().ok_or_else(|| {
        ResolvedEventAdapterError::OuterOrdinalOutOfBounds {
            dimension: dimension.to_string().into_boxed_str(),
            ordinal,
        }
    })?;
    if ordinal >= cardinality {
        return Err(ResolvedEventAdapterError::OuterOrdinalOutOfBounds {
            dimension: dimension.to_string().into_boxed_str(),
            ordinal,
        });
    }
    match domain {
        ExploreExactDomain::IntRange { start, .. } => {
            let offset = i128::try_from(ordinal).map_err(|_| {
                ResolvedEventAdapterError::OuterOrdinalOutOfBounds {
                    dimension: dimension.to_string().into_boxed_str(),
                    ordinal,
                }
            })?;
            let value = i128::from(*start).checked_add(offset).ok_or_else(|| {
                ResolvedEventAdapterError::OuterOrdinalOutOfBounds {
                    dimension: dimension.to_string().into_boxed_str(),
                    ordinal,
                }
            })?;
            Ok(DomainValueAt::Exact(AbstractValue::Ground(
                ExploreValue::Int(i64::try_from(value).map_err(|_| {
                    ResolvedEventAdapterError::OuterOrdinalOutOfBounds {
                        dimension: dimension.to_string().into_boxed_str(),
                        ordinal,
                    }
                })?),
            )))
        }
        ExploreExactDomain::Enumerated { values, .. } => {
            let ordinal_index = usize::try_from(ordinal).map_err(|_| {
                ResolvedEventAdapterError::OuterOrdinalOutOfBounds {
                    dimension: dimension.to_string().into_boxed_str(),
                    ordinal,
                }
            })?;
            let value = values.get(ordinal_index).ok_or_else(|| {
                ResolvedEventAdapterError::OuterOrdinalOutOfBounds {
                    dimension: dimension.to_string().into_boxed_str(),
                    ordinal,
                }
            })?;
            ensure_explore_value_bounded(value, budget)
                .map_err(|failure| finite_access_error(dimension, failure))?;
            match exact_ground_value(
                value,
                &mut |value_path| CheckedExploreGroundConstructorSite::EnumeratedDimension {
                    dimension_index,
                    ordinal: ordinal_index,
                    value_path,
                },
                &mut Vec::new(),
                constructors,
            ) {
                Ok(value) => Ok(DomainValueAt::Exact(value)),
                Err(detail) => Ok(DomainValueAt::MissingIdentity(detail)),
            }
        }
        ExploreExactDomain::FiniteType { plan, .. } => {
            let value = finite_plan_value_at(
                plan,
                ordinal,
                budget,
                0,
                dimension_index,
                &mut Vec::new(),
                constructors,
            )
            .map_err(|failure| finite_access_error(dimension, failure))?;
            if has_missing_constructor_identity(&value) {
                Ok(DomainValueAt::MissingIdentity(
                    "finite-type value has no location-bound checked constructor identity".into(),
                ))
            } else {
                Ok(DomainValueAt::Exact(value))
            }
        }
    }
}

fn query_profile_env(
    context: &mut AdapterContext<'_, '_>,
    query: &ExploreQueryIr,
    roots: &QueryRoots,
) -> Result<(AbstractEnv, BTreeMap<String, AbstractValue>), ResolvedEventAdapterError> {
    let boundary = query
        .universe
        .boundary
        .as_ref()
        .ok_or(ResolvedEventAdapterError::QueryHasNoBoundary)?;
    let profile_limit = context.limits.max_collection_items.get();
    if query.universe.dimensions.len() > profile_limit
        || query.universe.facts.len() > profile_limit
        || query.query.bounds.len() > profile_limit
        || query.query.inputs.len() > profile_limit
        || query
            .universe
            .dimensions
            .iter()
            .any(|dimension| dimension.name.len() > profile_limit)
        || query
            .universe
            .facts
            .iter()
            .any(|fact| fact.name.len() > profile_limit)
    {
        return Err(
            ResolvedEventAdapterError::OuterProfileMaterializationLimit {
                dimension: "$query-profile".into(),
                limit: profile_limit,
            },
        );
    }
    let expected_outer = query.universe.dimensions.len().saturating_sub(1);
    if context.outer_ordinals.len() != expected_outer {
        return Err(ResolvedEventAdapterError::OuterOrdinalArityMismatch {
            expected: expected_outer,
            actual: context.outer_ordinals.len(),
        });
    }
    let mut outer_index = 0_usize;
    let mut values = BTreeMap::new();
    let mut finite_budget = FiniteTypeAccessBudget::new(context.limits);
    for (dimension_index, dimension) in query.universe.dimensions.iter().enumerate() {
        let value = if dimension_index == boundary.axis_dimension_index {
            AbstractValue::Int(IntTerm::linear(AffineForm::new(1, 0)))
        } else {
            let ordinal = context.outer_ordinals[outer_index];
            outer_index += 1;
            match domain_value_at(
                &dimension.domain,
                ordinal,
                dimension_index,
                &dimension.name,
                &mut finite_budget,
                context.ground_constructors,
            )? {
                DomainValueAt::Exact(value) => value,
                DomainValueAt::MissingIdentity(detail) => AbstractValue::Unsupported(
                    context.residual(None, UnsupportedResidualKind::AdapterIncomplete, detail),
                ),
            }
        };
        if let AbstractValue::Constructor(constructor) = &value {
            let expected_owner = context
                .type_owners
                .get(&CheckedExploreTypeUse::Dimension(dimension_index));
            if constructor
                .identity
                .as_ref()
                .map(|identity| &identity.owner)
                != expected_owner
            {
                let value = AbstractValue::Unsupported(context.residual(
                    None,
                    UnsupportedResidualKind::AdapterIncomplete,
                    "closed dimension constructor owner diverges from the exact checked type fact",
                ));
                values.insert(dimension.name.clone(), value);
                continue;
            }
        }
        values.insert(dimension.name.clone(), value);
    }

    let mut env = AbstractEnv::new();
    let mut fact_index = 0_usize;
    for (bound_index, bound) in query.query.bounds.iter().enumerate() {
        let (name, value) = match bound {
            TypedExploreBound::Domain { name, .. } => {
                let value = values.get(name).cloned().ok_or_else(|| {
                    ResolvedEventAdapterError::InternalArtifactGap(
                        "Phase-A domain bound is absent from the closed universe".into(),
                    )
                })?;
                (name, value)
            }
            TypedExploreBound::Value { name, .. } => {
                let fact = query.universe.facts.get(fact_index).ok_or_else(|| {
                    ResolvedEventAdapterError::InternalArtifactGap(
                        "Phase-A value bound is absent from the closed fact universe".into(),
                    )
                })?;
                if fact.name != *name {
                    return Err(ResolvedEventAdapterError::InternalArtifactGap(
                        "Phase-A value-bound order diverges from the closed fact universe".into(),
                    ));
                }
                let value = match &fact.value {
                    ExploreFactValue::Fixed(expected) => {
                        ensure_explore_value_bounded(expected, &mut finite_budget)
                            .map_err(|failure| finite_access_error(name, failure))?;
                        let exact_expected = exact_ground_value(
                            expected,
                            &mut |value_path| CheckedExploreGroundConstructorSite::FixedFact {
                                fact_index,
                                value_path,
                            },
                            &mut Vec::new(),
                            context.ground_constructors,
                        );
                        let expression_site = roots
                            .bound_expression_sites
                            .get(&bound_index)
                            .ok_or_else(|| {
                                ResolvedEventAdapterError::InternalArtifactGap(
                                    "fixed fact has no Phase-A expression site".into(),
                                )
                            })?;
                        let specialized = context
                            .with_role(BoundaryFragmentRootRole::Validity, |context| {
                                context.eval_site(expression_site, &env)
                            });
                        let agrees = exact_expected.as_ref().ok().is_some_and(|expected| {
                            abstract_ground_equal(&specialized, expected) == Some(true)
                        });
                        if agrees {
                            specialized
                        } else {
                            context.mark_quantizers_uncertified(&specialized);
                            let source = context
                                .index
                                .expression(expression_site)
                                .map(|indexed| source_site(expression_site, indexed.expression));
                            AbstractValue::Unsupported(context.residual(
                                source,
                                UnsupportedResidualKind::AdapterIncomplete,
                                exact_expected.err().unwrap_or_else(|| {
                                    "Phase-A specialization does not reproduce the identity-bound closed fixed fact exactly".into()
                                }),
                            ))
                        }
                    }
                    ExploreFactValue::Derived { dependencies, .. } => {
                        let expression_site = roots
                            .bound_expression_sites
                            .get(&bound_index)
                            .ok_or_else(|| {
                                ResolvedEventAdapterError::InternalArtifactGap(
                                    "derived fact has no Phase-A expression site".into(),
                                )
                            })?;
                        let role = if dependencies.contains(boundary.axis.as_str()) {
                            BoundaryFragmentRootRole::BoundarySensitiveFact
                        } else {
                            BoundaryFragmentRootRole::Validity
                        };
                        context.with_role(role, |context| context.eval_site(expression_site, &env))
                    }
                };
                fact_index += 1;
                values.insert(name.clone(), value.clone());
                (name, value)
            }
            TypedExploreBound::Where { .. } => continue,
        };
        let binder_site = roots.bound_binder_sites.get(&bound_index).ok_or_else(|| {
            ResolvedEventAdapterError::InternalArtifactGap(
                "Explore value bound has no producer-minted binder site".into(),
            )
        })?;
        env.insert(binder_site.clone(), value.clone());
        values.insert(name.clone(), value);
    }
    if fact_index != query.universe.facts.len() {
        return Err(ResolvedEventAdapterError::AcceptedQueryArtifactDiverged);
    }
    Ok((env, values))
}

fn first_axis_at_or_above(affine: AffineForm, target: i128) -> Option<i128> {
    if affine.coefficient <= 0 {
        return None;
    }
    let numerator = target.checked_sub(affine.intercept)?;
    let quotient = numerator.div_euclid(affine.coefficient);
    if numerator.rem_euclid(affine.coefficient) == 0 {
        Some(quotient)
    } else {
        quotient.checked_add(1)
    }
}

fn liveness_interval(
    observation: &QuantizerObservation,
    declared_step: i64,
) -> Option<BoundaryAxisInterval> {
    if observation.uncertified_use
        || !observation.key.nonnegative_numerator
        || observation.key.divisor <= 0
        || declared_step == 0
    {
        return None;
    }
    let cutoff = observation.cutoff_cells.iter().next_back().copied()?;
    if cutoff <= 0 {
        return None;
    }
    let live_start = first_axis_at_or_above(observation.key.numerator, 0)?;
    let cutoff_raw = cutoff.checked_mul(i128::from(observation.key.divisor))?;
    let first_dead = first_axis_at_or_above(observation.key.numerator, cutoff_raw)?;
    // Support is certified for endpoint pairs, not isolated endpoint values.
    // `contains_pair` requires both endpoints, so retain one full declared-step
    // halo on each side of the scalar live interval.  This keeps a pair whose
    // before endpoint is just outside the live interval and a pair whose after
    // endpoint is just outside it.  Convert through i128 before taking the
    // magnitude so i64::MIN is handled without overflow.
    let dilation = i128::from(declared_step).checked_abs()?;
    let start = live_start.checked_sub(dilation)?;
    let end = first_dead.checked_add(dilation)?;
    let int_min = i128::from(i64::MIN);
    let int_max_exclusive = i128::from(i64::MAX).checked_add(1)?;
    let start = start.max(int_min);
    let end = end.min(int_max_exclusive);
    (start < end).then_some(BoundaryAxisInterval {
        start_inclusive: start,
        end_exclusive: end,
    })
}

fn certificate_id(
    observation: &QuantizerObservation,
    analysis_program_hash: &str,
    query_hash: &str,
    outer_ordinals: &[u128],
    axis_name: &str,
    step: i64,
    interval: BoundaryAxisInterval,
) -> Box<str> {
    let mut hasher = Sha256::new();
    let mut segment = |bytes: &[u8]| {
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    };
    segment(LIVENESS_CERTIFICATE_DOMAIN);
    segment(analysis_program_hash.as_bytes());
    segment(query_hash.as_bytes());
    segment(axis_name.as_bytes());
    segment(&step.to_le_bytes());
    segment(&(outer_ordinals.len() as u64).to_le_bytes());
    for ordinal in outer_ordinals {
        segment(&ordinal.to_le_bytes());
    }
    segment(observation.source.id.declaration_id.as_bytes());
    segment(&(observation.source.id.ast_path.len() as u64).to_le_bytes());
    for child in observation.source.id.ast_path.iter() {
        segment(&child.to_le_bytes());
    }
    segment(&observation.key.numerator.coefficient.to_le_bytes());
    segment(&observation.key.numerator.intercept.to_le_bytes());
    segment(&observation.key.divisor.to_le_bytes());
    segment(&interval.start_inclusive.to_le_bytes());
    segment(&interval.end_exclusive.to_le_bytes());
    format!("{:x}", hasher.finalize()).into_boxed_str()
}

fn resolved_classification_formula(
    context: &mut AdapterContext<'_, '_>,
    value: &AbstractValue,
    site: &ExprSiteId,
    expression: &Expr,
) -> ResolvedClassificationFormula {
    match value {
        AbstractValue::Ground(ExploreValue::Boolean(value)) => {
            ResolvedClassificationFormula::Constant(*value)
        }
        AbstractValue::Predicate(predicate) => predicate.classification_formula(),
        AbstractValue::Unsupported(residual) => {
            ResolvedClassificationFormula::Unsupported(residual.clone())
        }
        _ => ResolvedClassificationFormula::Unsupported(context.residual(
            Some(source_site(site, expression)),
            UnsupportedResidualKind::UnsupportedType,
            "checked Explore question did not normalize to an exact Boolean formula",
        )),
    }
}

fn finalize_fragment(
    mut context: AdapterContext<'_, '_>,
    classification: ResolvedClassificationFormula,
) -> ResolvedBoundaryFragment {
    let observations = std::mem::take(&mut context.quantizers);
    for mut observation in observations.into_values() {
        if context.all_quantizers_uncertified {
            observation.uncertified_use = true;
        }
        let interval = liveness_interval(&observation, context.step);
        let support = match interval {
            Some(interval) => {
                let certificate_id = certificate_id(
                    &observation,
                    context.analysis_program_hash,
                    context.query_hash,
                    context.outer_ordinals,
                    &context.axis_name,
                    context.step,
                    interval,
                );
                ResolvedAxisSupport::ExactIntervals {
                    intervals: vec![interval].into_boxed_slice(),
                    certificate: ResolvedLivenessCertificate {
                        certificate_id,
                        analysis_program_hash: context.analysis_program_hash.into(),
                        query_hash: context.query_hash.into(),
                        outer_ordinals: context.outer_ordinals.to_vec().into_boxed_slice(),
                        axis_name: context.axis_name.clone(),
                        step: context.step,
                        covered_event_sites: vec![observation.source.id.clone()].into_boxed_slice(),
                    },
                }
            }
            // Narrowed liveness is an optional scheduling optimization. The
            // full finite boundary axis is itself exact support, so absence of
            // a narrower proof cannot make the retained formula incomplete.
            None => ResolvedAxisSupport::Everywhere,
        };
        for role in observation.roles {
            let key = RootKey {
                role,
                site: observation.source.id.clone(),
                specialization: RootSpecialization::TruncDivision {
                    numerator: observation.key.numerator,
                    divisor: observation.key.divisor,
                },
            };
            context.insert_root(
                key,
                ResolvedBoundaryRoot {
                    role,
                    guards: vec![SourceGuard::ReachableFrom { role }].into_boxed_slice(),
                    active_support: support.clone(),
                    node: ResolvedBoundaryNode::Int(BoundaryIntExpr::TruncDiv {
                        numerator: observation.key.numerator,
                        divisor: observation.key.divisor,
                        source: observation.source.clone(),
                    }),
                },
            );
        }
    }
    let residuals = context
        .residuals
        .into_values()
        .collect::<Vec<_>>()
        .into_boxed_slice();
    ResolvedBoundaryFragment {
        analysis_program_hash: context.analysis_program_hash.into(),
        query_hash: context.query_hash.into(),
        outer_ordinals: context.outer_ordinals.to_vec().into_boxed_slice(),
        axis_name: context.axis_name,
        step: context.step,
        classification,
        roots: context
            .roots
            .into_values()
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        coverage: if residuals.is_empty() {
            ResolvedFragmentCoverage::Complete
        } else {
            ResolvedFragmentCoverage::Incomplete { residuals }
        },
    }
}

/// Adapt one accepted checked query and one exact outer profile into source
/// events.  Any Phase-B issue in the full transitive reachable closure is
/// fatal.  Semantic gaps after that resolution boundary remain explicit
/// residuals in the returned fragment.
pub(in crate::explore) fn adapt_checked_boundary_fragment(
    request: ResolvedEventAdapterRequest<'_>,
) -> Result<AdaptedBoundaryFragment, ResolvedEventAdapterError> {
    let prepared = PreparedResolvedEventAdapter::prepare(
        request.artifacts,
        request.accepted_query_index,
        request.limits,
    )?;
    prepared.adapt_profile(
        request.analysis_program_hash,
        request.query_hash,
        request.outer_ordinals,
    )
}

#[cfg(test)]
mod source_canaries {
    use super::*;

    fn synthetic_phaseout_observation() -> QuantizerObservation {
        let source = SourceSite {
            id: SourceSiteId {
                declaration_id: "module=synthetic-canary;rule=phaseout".into(),
                ast_path: vec![1, 0, 2].into_boxed_slice(),
            },
            span: crate::Span::dummy(),
        };
        let key = QuantizerKey {
            source: source.id.clone(),
            numerator: AffineForm::new(1, -341_500),
            divisor: 1_000,
            nonnegative_numerator: true,
        };
        QuantizerObservation {
            key,
            source,
            roles: [BoundaryFragmentRootRole::Question].into_iter().collect(),
            cutoff_cells: [50].into_iter().collect(),
            uncertified_use: false,
        }
    }

    #[test]
    fn positive_trunc_div_liveness_is_pair_relative_at_step_one() {
        assert_eq!(
            liveness_interval(&synthetic_phaseout_observation(), 1),
            Some(BoundaryAxisInterval {
                start_inclusive: 341_499,
                end_exclusive: 391_501,
            })
        );
    }

    #[test]
    fn synthetic_fragment_smoke_counts_candidates_without_claiming_adapter_evidence() {
        // This fragment is hand-built test data.  It is deliberately not an
        // accepted-query adapter result and is not canonical model evidence.
        let observation = synthetic_phaseout_observation();
        let interval = liveness_interval(&observation, 1).unwrap();
        let fragment = ResolvedBoundaryFragment {
            analysis_program_hash: "program".into(),
            query_hash: "query".into(),
            outer_ordinals: Box::new([]),
            axis_name: "income".into(),
            step: 1,
            classification: ResolvedClassificationFormula::Constant(false),
            roots: vec![ResolvedBoundaryRoot {
                role: BoundaryFragmentRootRole::Question,
                guards: vec![SourceGuard::ReachableFrom {
                    role: BoundaryFragmentRootRole::Question,
                }]
                .into_boxed_slice(),
                active_support: ResolvedAxisSupport::ExactIntervals {
                    intervals: vec![interval].into_boxed_slice(),
                    certificate: ResolvedLivenessCertificate {
                        certificate_id: "synthetic-canary-not-adapter-evidence".into(),
                        analysis_program_hash: "program".into(),
                        query_hash: "query".into(),
                        outer_ordinals: Box::new([]),
                        axis_name: "income".into(),
                        step: 1,
                        covered_event_sites: vec![observation.source.id.clone()].into_boxed_slice(),
                    },
                },
                node: ResolvedBoundaryNode::Int(BoundaryIntExpr::TruncDiv {
                    numerator: observation.key.numerator,
                    divisor: observation.key.divisor,
                    source: observation.source,
                }),
            }]
            .into_boxed_slice(),
            coverage: ResolvedFragmentCoverage::Incomplete {
                residuals: vec![UnsupportedResidual {
                    source: None,
                    kind: UnsupportedResidualKind::AdapterIncomplete,
                    detail: "canary deliberately does not close the complement".into(),
                }]
                .into_boxed_slice(),
            },
        };
        let axis = super::super::AxisDomain::Dense {
            start: 0,
            end_exclusive: 1_500_001,
        };
        let extracted = super::super::extract_fragment(
            &axis,
            1,
            &fragment,
            super::super::SourceEventExtractionOptions {
                max_candidate_ordinals: NonZeroUsize::new(1_000).unwrap(),
                max_event_cuts: NonZeroUsize::new(1_000).unwrap(),
            },
        );
        let lower_endpoints = extracted
            .candidates
            .iter()
            .map(|candidate| candidate.boundary_value)
            .collect::<Vec<_>>();

        assert_eq!(lower_endpoints.len(), 50);
        assert_eq!(lower_endpoints.first(), Some(&342_499));
        assert_eq!(lower_endpoints.last(), Some(&391_499));
        assert_eq!(
            lower_endpoints,
            (342_499..=391_499).step_by(1_000).collect::<Vec<_>>()
        );
        assert!(!extracted.extraction_complete);
    }

    #[test]
    fn an_uncertified_quantizer_use_cannot_shrink_axis_support() {
        let mut observation = synthetic_phaseout_observation();
        observation.uncertified_use = true;
        assert_eq!(liveness_interval(&observation, 1), None);
    }

    #[test]
    fn liveness_dilates_both_edges_by_the_declared_step_magnitude() {
        let observation = synthetic_phaseout_observation();
        let expected = Some(BoundaryAxisInterval {
            start_inclusive: 341_497,
            end_exclusive: 391_503,
        });
        assert_eq!(liveness_interval(&observation, 3), expected);
        assert_eq!(liveness_interval(&observation, -3), expected);

        let interval = expected.unwrap();
        let support = ResolvedAxisSupport::ExactIntervals {
            intervals: vec![interval].into_boxed_slice(),
            certificate: ResolvedLivenessCertificate {
                certificate_id: "synthetic-step-three".into(),
                analysis_program_hash: "program".into(),
                query_hash: "query".into(),
                outer_ordinals: Box::new([]),
                axis_name: "income".into(),
                step: 3,
                covered_event_sites: vec![observation.source.id].into_boxed_slice(),
            },
        };
        assert!(support.contains_pair(341_497, 341_500));
        assert!(support.contains_pair(391_499, 391_502));
    }

    #[test]
    fn root_key_keeps_distinct_specializations_and_deduplicates_exact_repeats() {
        let site = SourceSiteId {
            declaration_id: "module=synthetic-canary;rule=specialized".into(),
            ast_path: vec![4, 2].into_boxed_slice(),
        };
        let comparison = |intercept| RootKey {
            role: BoundaryFragmentRootRole::Question,
            site: site.clone(),
            specialization: RootSpecialization::Comparison {
                difference: AffineForm::new(1, intercept),
                relation: BoundaryRelation::GreaterOrEqual,
            },
        };
        let maximum = |intercept| RootKey {
            role: BoundaryFragmentRootRole::Question,
            site: site.clone(),
            specialization: RootSpecialization::Maximum {
                left_minus_right: AffineForm::new(1, intercept),
                tie_arm: TieArm::Left,
            },
        };
        let first = comparison(-10);
        let keys = [
            first.clone(),
            first,
            comparison(-20),
            maximum(-10),
            maximum(-20),
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        assert_eq!(keys.len(), 4);
    }

    #[test]
    fn oversized_rule_family_fails_before_a_rejecting_prefix_can_hide_an_event_root() {
        // Model a two-candidate family at a one-candidate preparation cap:
        // candidate zero rejects, while candidate one contains a supported
        // affine comparison. Preparation must fail as a whole; evaluating a
        // source-order prefix and returning a falsely complete fragment is
        // never an admissible outcome.
        assert!(matches!(
            enforce_rule_candidate_limit(2, 1),
            Err(ResolvedEventAdapterError::RuleCandidateLimit { limit: 1 })
        ));
    }
}
