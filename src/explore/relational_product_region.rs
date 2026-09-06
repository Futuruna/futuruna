//! Verified no-finding regions over independent finite integer products.
//!
//! A canonical rank interval is enclosed in a conservative source-coordinate
//! box. Classification must be total and uniformly admitted/not-selected on
//! that entire box, including any extra points introduced by enclosure. The
//! emitted weight is ONLY the exact rank-interval cardinality. Constructor
//! paths and pure call frames preserve affine correlations, so Before/After
//! cancellation can prove a delta without enumerating every coordinate.
//! Unsupported graphs and uncertain branches remain concrete residual work.

use super::*;
use crate::explore::support_cell::SupportExprKind;

const MAX_VALUE_NODES: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Number {
    constant: i128,
    coefficients: BTreeMap<u32, i128>,
    error: (i128, i128),
}

impl Number {
    fn constant(value: i128) -> Self {
        Self {
            constant: value,
            coefficients: BTreeMap::new(),
            error: (0, 0),
        }
    }

    fn bounds(&self, domains: &BTreeMap<u32, (i128, i128)>) -> Option<(i128, i128)> {
        let mut low = self.constant.checked_add(self.error.0)?;
        let mut high = self.constant.checked_add(self.error.1)?;
        for (axis, coefficient) in &self.coefficients {
            let (a, b) = *domains.get(axis)?;
            let a = a.checked_mul(*coefficient)?;
            let b = b.checked_mul(*coefficient)?;
            low = low.checked_add(a.min(b))?;
            high = high.checked_add(a.max(b))?;
        }
        Some((low, high))
    }

    fn scale(mut self, scale: i128) -> Option<Self> {
        self.constant = self.constant.checked_mul(scale)?;
        for coefficient in self.coefficients.values_mut() {
            *coefficient = coefficient.checked_mul(scale)?;
        }
        self.coefficients.retain(|_, coefficient| *coefficient != 0);
        let a = self.error.0.checked_mul(scale)?;
        let b = self.error.1.checked_mul(scale)?;
        self.error = (a.min(b), a.max(b));
        Some(self)
    }

    fn add(mut self, other: Self) -> Option<Self> {
        self.constant = self.constant.checked_add(other.constant)?;
        self.error = (
            self.error.0.checked_add(other.error.0)?,
            self.error.1.checked_add(other.error.1)?,
        );
        for (axis, coefficient) in other.coefficients {
            let old = self.coefficients.entry(axis).or_default();
            *old = old.checked_add(coefficient)?;
        }
        self.coefficients.retain(|_, coefficient| *coefficient != 0);
        Some(self)
    }

    fn exact_constant(&self) -> Option<i128> {
        (self.coefficients.is_empty() && self.error == (0, 0)).then_some(self.constant)
    }

    fn runtime(self, domains: &BTreeMap<u32, (i128, i128)>) -> Option<Self> {
        let (low, high) = self.bounds(domains)?;
        (low >= i128::from(i64::MIN) && high <= i128::from(i64::MAX)).then_some(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Value {
    Int(Number),
    Bool(bool),
    Constant(ClassificationConstant),
    Construct {
        owner: [u8; 32],
        variant: u32,
        fields: Vec<Value>,
    },
}

impl Value {
    fn node_count(&self) -> usize {
        match self {
            Self::Construct { fields, .. } => {
                1 + fields.iter().map(Self::node_count).sum::<usize>()
            }
            _ => 1,
        }
    }

    fn integer(self) -> Option<Number> {
        if let Self::Int(value) = self {
            Some(value)
        } else {
            None
        }
    }
    fn boolean(self) -> Option<bool> {
        if let Self::Bool(value) = self {
            Some(value)
        } else {
            None
        }
    }
}

struct Evaluator<'a> {
    capsule: &'a RelationalClassificationCapsule,
    domains: BTreeMap<u32, (i128, i128)>,
    source: BTreeMap<u32, Value>,
    inputs: BTreeMap<ClassificationInputSlot, Value>,
    frames: Vec<(ClassificationCallableId, Vec<Value>)>,
    work: usize,
    depth: usize,
}

impl Evaluator<'_> {
    // Bound retained trees as well as evaluation work: repeated constructor
    // duplication can otherwise expand exponentially without visiting many
    // graph nodes. Arguments and fields share the same conservative cap.
    fn values(&mut self, nodes: &[ClassificationNodeId]) -> Option<Vec<Value>> {
        if nodes.len() >= MAX_VALUE_NODES {
            return None;
        }
        let mut result = Vec::with_capacity(nodes.len());
        let mut retained = 1;
        for node in nodes {
            let value = self.eval(*node)?;
            retained += value.node_count();
            if retained > MAX_VALUE_NODES {
                return None;
            }
            result.push(value);
        }
        Some(result)
    }

    fn eval(&mut self, id: ClassificationNodeId) -> Option<Value> {
        if self.work == 100_000 || self.depth == 128 {
            return None;
        }
        self.work += 1;
        self.depth += 1;
        let value = self.eval_node(id);
        self.depth -= 1;
        value
    }

    fn eval_node(&mut self, id: ClassificationNodeId) -> Option<Value> {
        let node = classification_node(self.capsule.graph(), id).ok()?.clone();
        match node.kind {
            ClassificationNodeKind::Constant(ClassificationConstant::Integer(value)) => {
                Some(Value::Int(Number::constant(i128::from(value))))
            }
            ClassificationNodeKind::Constant(ClassificationConstant::Boolean(value)) => {
                Some(Value::Bool(value))
            }
            ClassificationNodeKind::Constant(value) => Some(Value::Constant(value)),
            ClassificationNodeKind::Input(slot) => self.inputs.get(&slot).cloned(),
            ClassificationNodeKind::SourceParameter(binding) => self.source.get(&binding).cloned(),
            ClassificationNodeKind::CallableParameter {
                callable_id,
                ordinal,
            } => {
                let (owner, arguments) = self.frames.last()?;
                (*owner == callable_id)
                    .then(|| arguments.get(ordinal as usize).cloned())
                    .flatten()
            }
            ClassificationNodeKind::Construct {
                constructor_id,
                fields,
            } => {
                let shape = self
                    .capsule
                    .runtime_shapes()
                    .shape_for_constructor(constructor_id)?;
                let owner = shape.owner_id;
                let variant = shape.variant_ordinal;
                let fields = self.values(&fields)?;
                Some(Value::Construct {
                    owner,
                    variant,
                    fields,
                })
            }
            ClassificationNodeKind::Project {
                owner_id,
                variant_ordinal,
                field_ordinal,
                base,
            } => {
                let Value::Construct {
                    owner,
                    variant,
                    fields,
                } = self.eval(base)?
                else {
                    return None;
                };
                (owner == owner_id && variant == variant_ordinal)
                    .then(|| fields.get(field_ordinal as usize).cloned())
                    .flatten()
            }
            ClassificationNodeKind::IsVariant {
                owner_id,
                variant_ordinal,
                base,
            } => {
                let Value::Construct { owner, variant, .. } = self.eval(base)? else {
                    return None;
                };
                (owner == owner_id).then_some(Value::Bool(variant == variant_ordinal))
            }
            ClassificationNodeKind::Unary { op, operand } => {
                let value = self.eval(operand)?;
                match op {
                    ClassificationUnaryOp::BooleanNot => Some(Value::Bool(!value.boolean()?)),
                    ClassificationUnaryOp::IntegerNegateChecked => Some(Value::Int(
                        value.integer()?.scale(-1)?.runtime(&self.domains)?,
                    )),
                }
            }
            ClassificationNodeKind::If {
                condition,
                then_node,
                else_node,
            } => {
                let condition = self.eval(condition)?.boolean()?;
                self.eval(if condition { then_node } else { else_node })
            }
            ClassificationNodeKind::Call {
                callable_id,
                arguments,
            } => {
                if self.frames.iter().any(|(active, _)| *active == callable_id) {
                    return None;
                }
                let body = classification_callable(self.capsule.graph(), callable_id)
                    .ok()?
                    .body;
                let arguments = self.values(&arguments)?;
                self.frames.push((callable_id, arguments));
                let result = self.eval(body);
                self.frames.pop();
                result
            }
            ClassificationNodeKind::Binary { op, left, right } => {
                let left = self.eval(left)?;
                match op {
                    ClassificationBinaryOp::BooleanAndShortCircuit => {
                        return Some(Value::Bool(left.boolean()? && self.eval(right)?.boolean()?))
                    }
                    ClassificationBinaryOp::BooleanOrShortCircuit => {
                        return Some(Value::Bool(left.boolean()? || self.eval(right)?.boolean()?))
                    }
                    _ => {}
                }
                let right = self.eval(right)?;
                if matches!(
                    op,
                    ClassificationBinaryOp::Equal | ClassificationBinaryOp::NotEqual
                ) {
                    let equal = self.equal(&left, &right)?;
                    return Some(Value::Bool(equal == (op == ClassificationBinaryOp::Equal)));
                }
                let left = left.integer()?;
                let right = right.integer()?;
                use ClassificationBinaryOp::*;
                if matches!(
                    op,
                    LessThan | LessThanOrEqual | GreaterThan | GreaterThanOrEqual
                ) {
                    let (low, high) = left.add(right.scale(-1)?)?.bounds(&self.domains)?;
                    let (yes, no) = match op {
                        LessThan => (high < 0, low >= 0),
                        LessThanOrEqual => (high <= 0, low > 0),
                        GreaterThan => (low > 0, high <= 0),
                        GreaterThanOrEqual => (low >= 0, high < 0),
                        _ => unreachable!(),
                    };
                    return (yes || no).then_some(Value::Bool(yes));
                }
                let value = match op {
                    IntegerAddChecked => left.add(right)?,
                    IntegerSubtractChecked => left.add(right.scale(-1)?)?,
                    IntegerMultiplyChecked => {
                        if let Some(scale) = right.exact_constant() {
                            left.scale(scale)?
                        } else if let Some(scale) = left.exact_constant() {
                            right.scale(scale)?
                        } else {
                            return None;
                        }
                    }
                    IntegerDivideChecked => {
                        let divisor = right.exact_constant()?;
                        if divisor == 0 {
                            return None;
                        }
                        let (low, high) = left.bounds(&self.domains)?;
                        if divisor == -1
                            && low <= i128::from(i64::MIN)
                            && high >= i128::from(i64::MIN)
                        {
                            return None;
                        }
                        if left.error == (0, 0)
                            && left.constant.checked_rem(divisor) == Some(0)
                            && left
                                .coefficients
                                .values()
                                .all(|coefficient| coefficient.checked_rem(divisor) == Some(0))
                        {
                            Number {
                                constant: left.constant.checked_div(divisor)?,
                                coefficients: left
                                    .coefficients
                                    .into_iter()
                                    .map(|(axis, coefficient)| {
                                        Some((axis, coefficient.checked_div(divisor)?))
                                    })
                                    .collect::<Option<_>>()?,
                                error: (0, 0),
                            }
                        } else {
                            let a = low.checked_div(divisor)?;
                            let b = high.checked_div(divisor)?;
                            Number {
                                constant: 0,
                                coefficients: BTreeMap::new(),
                                error: (a.min(b), a.max(b)),
                            }
                        }
                    }
                    IntegerRemainderChecked => {
                        let a = left.exact_constant()?;
                        let b = right.exact_constant()?;
                        if b == 0 || (a == i128::from(i64::MIN) && b == -1) {
                            return None;
                        }
                        Number::constant(a.checked_rem(b)?)
                    }
                    _ => return None,
                };
                Some(Value::Int(value.runtime(&self.domains)?))
            }
        }
    }

    fn equal(&self, left: &Value, right: &Value) -> Option<bool> {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => {
                let (low, high) = a.clone().add(b.clone().scale(-1)?)?.bounds(&self.domains)?;
                if low == 0 && high == 0 {
                    Some(true)
                } else if high < 0 || low > 0 {
                    Some(false)
                } else {
                    None
                }
            }
            (Value::Bool(a), Value::Bool(b)) => Some(a == b),
            // Unit's direct/structural equality is not uniformly reflexive
            // across runtime consumers. Do not acquire a pruning theorem
            // from the canonical-value identity used to carry a Unit input.
            (Value::Constant(ClassificationConstant::Unit), _)
            | (_, Value::Constant(ClassificationConstant::Unit)) => None,
            (Value::Constant(a), Value::Constant(b)) => Some(a == b),
            (
                Value::Construct {
                    owner: a,
                    variant: av,
                    fields: af,
                },
                Value::Construct {
                    owner: b,
                    variant: bv,
                    fields: bf,
                },
            ) => {
                if a != b {
                    return None;
                }
                if av != bv {
                    return Some(false);
                }
                if af.len() != bf.len() {
                    return None;
                }
                for (a, b) in af.iter().zip(bf) {
                    if !self.equal(a, b)? {
                        return Some(false);
                    }
                }
                Some(true)
            }
            _ => None,
        }
    }
}

pub(super) fn prove(
    checked: &CheckedExploreQueryView<'_>,
    plan: &RelationalSupportPlan,
    capsule: &RelationalClassificationCapsule,
    target: &RelationalRegionProofTarget<'_>,
    replay_authority_id: [u8; 32],
) -> Result<RelationalRegionProofOutcome, RelationalRegionProofError> {
    let unsupported =
        || fallback(RelationalRegionProofResidual::CaseImageCardinalityLiftUnavailable);
    let inventory = RelationalProofStrategyInventory::from_checked(checked, plan)?;
    let SupportExprKind::ProductRankInterval {
        factors,
        rank_start: start,
        rank_end_exclusive: end_exclusive,
    } = target.cell.expression().kind()
    else {
        return Ok(unsupported());
    };
    if start >= end_exclusive
        || *start != target.coordinate_start
        || *end_exclusive != target.coordinate_end_exclusive
        || factors.len() != inventory.finite_binding_indices().len()
        || inventory.axes().len() != factors.len()
        || factors.len() > 16
    {
        return Ok(unsupported());
    }
    let Some(first_axis) = inventory
        .axes()
        .iter()
        .find(|axis| Some(&axis.binding_index()) == inventory.finite_binding_indices().first())
    else {
        return Ok(unsupported());
    };
    let mut domains = BTreeMap::new();
    let mut stride = 1u128;
    for (binding, factor) in inventory.finite_binding_indices().iter().zip(factors).rev() {
        let SupportExprKind::OrdinalInterval {
            start: 0,
            end_exclusive: radix,
        } = factor.kind()
        else {
            return Ok(unsupported());
        };
        if *radix == 0 {
            return Ok(unsupported());
        }
        let Some(axis) = inventory
            .axes()
            .iter()
            .find(|axis| axis.binding_index() == *binding)
        else {
            return Ok(unsupported());
        };
        if axis.cardinality() != *radix || axis.coordinate_start() != 0 {
            return Ok(unsupported());
        }
        let Some(period) = stride.checked_mul(*radix) else {
            return Ok(unsupported());
        };
        let (low, high) = if start / period == (end_exclusive - 1) / period {
            (
                (start / stride) % radix,
                ((end_exclusive - 1) / stride) % radix,
            )
        } else {
            (0, radix - 1)
        };
        let low = i128::from(axis.value_start())
            + i128::try_from(low).map_err(|_| RelationalRegionProofError::InvalidArtifactShape)?;
        let high = i128::from(axis.value_start())
            + i128::try_from(high).map_err(|_| RelationalRegionProofError::InvalidArtifactShape)?;
        domains.insert(*binding, (low, high));
        stride = period;
    }
    if stride != target.root_coordinate_end_exclusive || target.root_coordinate_start != 0 {
        return Ok(unsupported());
    }
    let mut evaluator = Evaluator {
        capsule,
        domains,
        source: BTreeMap::new(),
        inputs: BTreeMap::new(),
        frames: Vec::new(),
        work: 0,
        depth: 0,
    };
    let graph = capsule.graph();
    let mut retained_sources = 0;
    for binding in &checked.closed_query.source.bindings {
        let index = binding.binding_index as u32;
        let value = if let Some((low, high)) = evaluator.domains.get(&index) {
            if low == high {
                Value::Int(Number::constant(*low))
            } else {
                Value::Int(Number {
                    constant: 0,
                    coefficients: BTreeMap::from([(index, 1)]),
                    error: (0, 0),
                })
            }
        } else {
            let Ok(root) =
                required_lane_root(graph, ClassificationSemanticLane::SourceBinding(index))
            else {
                return Ok(unsupported());
            };
            let Some(value) = evaluator.eval(root) else {
                return Ok(unsupported());
            };
            value
        };
        retained_sources += value.node_count();
        if retained_sources > MAX_VALUE_NODES {
            return Ok(unsupported());
        }
        evaluator.source.insert(index, value.clone());
        if binding.binding_index == checked.closed_query.source.before_binding_index {
            evaluator
                .inputs
                .insert(ClassificationInputSlot::BEFORE, value.clone());
        }
        if binding.binding_index == checked.closed_query.source.context_binding_index {
            evaluator
                .inputs
                .insert(ClassificationInputSlot::CONTEXT, value);
        }
    }
    if !matches!(
        checked.closed_query.successor.kind,
        ExploreSuccessorKindIr::Singleton { .. }
    ) {
        return Ok(unsupported());
    }
    let Ok(successor_root_id) = required_lane_root(graph, ClassificationSemanticLane::Successor)
    else {
        return Ok(unsupported());
    };
    let Some(after) = evaluator.eval(successor_root_id) else {
        return Ok(unsupported());
    };
    evaluator
        .inputs
        .insert(ClassificationInputSlot::AFTER, after);
    for entry in graph.lane_manifest() {
        if matches!(entry.lane, ClassificationSemanticLane::Admission { .. }) {
            let Ok(root) = required_lane_root(graph, entry.lane) else {
                return Ok(unsupported());
            };
            if evaluator.eval(root).and_then(Value::boolean) != Some(true) {
                return Ok(unsupported());
            }
        }
    }
    let question_id = checked.question_ids()[0];
    let Ok(find_root_id) = required_lane_root(graph, ClassificationSemanticLane::Find(question_id))
    else {
        return Ok(unsupported());
    };
    if evaluator.eval(find_root_id).and_then(Value::boolean) != Some(false) {
        return Ok(unsupported());
    }
    let (Some(assignment), Some(source), Some(successor), Some(root_cell_id)) = (
        plan.source_assignments().cell(),
        plan.source_rows().cell(),
        plan.successor_coordinates().cell(),
        plan.root_cell_id(),
    ) else {
        return Ok(unsupported());
    };
    let mut digest = CanonicalProofHasher::new(b"futuruna.explore.product-region.affine-box.v1");
    digest.digest(capsule.id().bytes());
    digest.u128(*start);
    digest.u128(*end_exclusive);
    for (binding, (low, high)) in &evaluator.domains {
        digest.u32(*binding);
        digest.i128(*low);
        digest.i128(*high);
    }
    let artifact = RelationalRegionProofArtifact {
        schema_version: RELATIONAL_REGION_PROOF_VERSION,
        product_rank: true,
        certificate_id: [0; 32],
        replay_authority_id,
        classification_capsule_id: capsule.id(),
        successor_root_id,
        find_root_id,
        relation_id: checked.relation_id(),
        admission_id: checked.admission_id(),
        question_id,
        plan_root: plan.root(),
        root_cell_id,
        subject: target.subject,
        conclusion: RelationalCertifiedRegionConclusion::AdmittedNotSelected,
        starter_region_id: RelationalStarterRegionId([0; 32]),
        source_assignment_cell_id: assignment.id(),
        source_row_cell_id: source.id(),
        successor_coordinate_cell_id: successor.id(),
        axis_stage_id: first_axis.stage_id(),
        axis_dimension_id: first_axis.dimension_id(),
        axis_cell_id: first_axis.cell().id(),
        value_start: i64::try_from(*start)
            .map_err(|_| RelationalRegionProofError::InvalidArtifactShape)?,
        value_end_exclusive: i64::try_from(*end_exclusive)
            .map_err(|_| RelationalRegionProofError::InvalidArtifactShape)?,
        coordinate_start: *start,
        coordinate_end_exclusive: *end_exclusive,
        case_cardinality: end_exclusive - start,
        selected_formula_digest: digest.finish(),
    };
    seal_region_proof(
        artifact,
        SupportCellObligation::new(target.cell, ExactCardinalityClaim)?,
        SupportCellObligation::new(
            target.cell,
            AdmissionClassificationClaim::new(checked.admission_id()),
        )?,
        SupportCellObligation::new(target.cell, SelectionClassificationClaim::new(question_id))?,
        target.cell,
        false,
    )
}
