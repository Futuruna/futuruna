//! Exact Cartesian restrictions of a mixed-radix page.
//!
//! Ranks are local to the ordered factor basis, while factor coordinates keep
//! their original values. Filtering one factor preserves tuple order, so the
//! image of a parent rank interval is another rank interval in the restricted
//! basis. Prefix counting translates the endpoints without enumerating tuples.
//! These are coordinate counts only; extensional case counts still require the
//! checked materializer's injectivity and admission/selection proofs.

use super::{SupportCellError, SupportExpr, SupportExprKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RankedProductBox {
    factors: Box<[(u128, u128)]>,
    rank_start: u128,
    rank_end: u128,
}

impl RankedProductBox {
    pub(crate) fn from_expr(expression: &SupportExpr) -> Result<Self, SupportCellError> {
        expression.validate()?;
        let (factors, rank_start, rank_end) = match expression.kind() {
            SupportExprKind::Product(factors) => (
                factors,
                0,
                expression.intrinsic_cardinality().exact().ok_or(
                    SupportCellError::InvalidProductRankInterval("product is not exact"),
                )?,
            ),
            SupportExprKind::ProductRankInterval {
                factors,
                rank_start,
                rank_end_exclusive,
            } => (factors, *rank_start, *rank_end_exclusive),
            _ => return Err(SupportCellError::ParentNotProduct),
        };
        let factors = factors
            .iter()
            .map(|factor| match factor.kind() {
                SupportExprKind::OrdinalInterval {
                    start,
                    end_exclusive,
                } => Ok((*start, *end_exclusive)),
                _ => Err(SupportCellError::InvalidProductRankInterval(
                    "ranked box requires ordinal interval factors",
                )),
            })
            .collect::<Result<Box<[_]>, _>>()?;
        Ok(Self {
            factors,
            rank_start,
            rank_end,
        })
    }

    pub(crate) fn expression(&self) -> Result<SupportExpr, SupportCellError> {
        SupportExpr::product_rank_interval(
            self.factors
                .iter()
                .map(|&(low, high)| SupportExpr::ordinal_interval(low, high))
                .collect::<Result<_, _>>()?,
            self.rank_start,
            self.rank_end,
        )
    }

    pub(crate) fn factors(&self) -> &[(u128, u128)] {
        &self.factors
    }

    pub(crate) const fn coordinate_count(&self) -> u128 {
        self.rank_end - self.rank_start
    }

    /// Exact overlap with a restriction in this parent's rank basis. Used to
    /// reconcile already checked concrete runs without enumerating coordinates.
    pub(crate) fn restricted_rank_count(
        &self,
        child: &Self,
        start: u128,
        end: u128,
    ) -> Result<u128, SupportCellError> {
        if start > end
            || start < self.rank_start
            || end > self.rank_end
            || self.factors.len() != child.factors.len()
            || self
                .factors
                .iter()
                .zip(&child.factors)
                .any(|(&(a, b), &(c, d))| c < a || d > b)
        {
            return Err(SupportCellError::InvalidProductRankInterval(
                "invalid restriction overlap",
            ));
        }
        let low = self
            .restricted_prefix_count(start, &child.factors)?
            .max(child.rank_start);
        let high = self
            .restricted_prefix_count(end, &child.factors)?
            .min(child.rank_end);
        Ok(high.saturating_sub(low))
    }

    /// Intersect with one factor interval. Empty intersections have no child,
    /// not an invented point or a zero-weight ordinary support cell.
    pub(crate) fn restrict_factor(
        &self,
        axis: usize,
        low: u128,
        high: u128,
    ) -> Result<Option<Self>, SupportCellError> {
        let &(parent_low, parent_high) =
            self.factors
                .get(axis)
                .ok_or(SupportCellError::ProductFactorOutOfBounds {
                    factor_index: axis,
                    factor_count: self.factors.len(),
                })?;
        if low > high || low < parent_low || high > parent_high {
            return Err(SupportCellError::InvalidProductRankInterval(
                "factor restriction is reversed or outside its parent",
            ));
        }
        if low == high {
            return Ok(None);
        }
        let mut factors = self.factors.clone();
        factors[axis] = (low, high);
        let rank_start = self.restricted_prefix_count(self.rank_start, &factors)?;
        let rank_end = self.restricted_prefix_count(self.rank_end, &factors)?;
        Ok((rank_start < rank_end).then_some(Self {
            factors,
            rank_start,
            rank_end,
        }))
    }

    /// Count restricted tuples preceding a rank in this box's full basis.
    /// Once a prefix digit lies outside the child, no equal-prefix term remains.
    fn restricted_prefix_count(
        &self,
        rank: u128,
        restricted: &[(u128, u128)],
    ) -> Result<u128, SupportCellError> {
        let overflow = || SupportCellError::CardinalityOverflow("ranked product prefix");
        let mut parent_suffix = vec![1u128; self.factors.len() + 1];
        let mut child_suffix = parent_suffix.clone();
        for axis in (0..self.factors.len()).rev() {
            parent_suffix[axis] = parent_suffix[axis + 1]
                .checked_mul(self.factors[axis].1 - self.factors[axis].0)
                .ok_or_else(overflow)?;
            child_suffix[axis] = child_suffix[axis + 1]
                .checked_mul(restricted[axis].1 - restricted[axis].0)
                .ok_or_else(overflow)?;
        }
        if rank == parent_suffix[0] {
            return Ok(child_suffix[0]);
        }
        if rank > parent_suffix[0] {
            return Err(SupportCellError::InvalidProductRankInterval(
                "prefix rank exceeds its basis",
            ));
        }
        let mut count = 0u128;
        for axis in 0..self.factors.len() {
            let (parent_low, parent_high) = self.factors[axis];
            let digit = rank / parent_suffix[axis + 1] % (parent_high - parent_low);
            let coordinate = parent_low.checked_add(digit).ok_or_else(overflow)?;
            let (low, high) = restricted[axis];
            let earlier = coordinate.min(high).saturating_sub(low);
            count = count
                .checked_add(
                    earlier
                        .checked_mul(child_suffix[axis + 1])
                        .ok_or_else(overflow)?,
                )
                .ok_or_else(overflow)?;
            if coordinate < low || coordinate >= high {
                break;
            }
        }
        Ok(count)
    }

    /// Inclusive tuple-coordinate enclosure, respecting nonzero factor origins.
    /// It can include extra tuples; a classifier must prove the whole enclosure.
    pub(crate) fn enclosure(&self) -> Result<Vec<(u128, u128)>, SupportCellError> {
        let overflow = || SupportCellError::CardinalityOverflow("ranked product enclosure");
        let mut stride = 1u128;
        let mut bounds = vec![(0, 0); self.factors.len()];
        for axis in (0..self.factors.len()).rev() {
            let (origin, end) = self.factors[axis];
            let radix = end - origin;
            let period = stride.checked_mul(radix).ok_or_else(overflow)?;
            bounds[axis] = if self.rank_start / period == (self.rank_end - 1) / period {
                (
                    origin
                        .checked_add(self.rank_start / stride % radix)
                        .ok_or_else(overflow)?,
                    origin
                        .checked_add((self.rank_end - 1) / stride % radix)
                        .ok_or_else(overflow)?,
                )
            } else {
                (origin, end - 1)
            };
            stride = period;
        }
        Ok(bounds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(factors: &[(u128, u128)], low: u128, high: u128) -> RankedProductBox {
        RankedProductBox::from_expr(
            &SupportExpr::product_rank_interval(
                factors
                    .iter()
                    .map(|&(a, b)| SupportExpr::ordinal_interval(a, b).unwrap())
                    .collect(),
                low,
                high,
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn tuples(page: &RankedProductBox) -> Vec<[u128; 3]> {
        let mut result = vec![];
        let mut rank = 0;
        for a in page.factors[0].0..page.factors[0].1 {
            for b in page.factors[1].0..page.factors[1].1 {
                for c in page.factors[2].0..page.factors[2].1 {
                    if rank >= page.rank_start && rank < page.rank_end {
                        result.push([a, b, c]);
                    }
                    rank += 1;
                }
            }
        }
        result
    }

    #[test]
    fn ranked_box_restrictions_match_tuple_oracle_at_every_small_page_boundary() {
        let factors = [(10, 13), (20, 24), (30, 32)];
        for start in 0..24 {
            for end in start + 1..=24 {
                let parent = page(&factors, start, end);
                let original = tuples(&parent);
                assert_eq!(
                    RankedProductBox::from_expr(&parent.expression().unwrap()).unwrap(),
                    parent
                );
                for (axis, &(low, high)) in factors.iter().enumerate() {
                    for a in low..=high {
                        for b in a..=high {
                            let child = parent.restrict_factor(axis, a, b).unwrap();
                            let expected: Vec<_> = original
                                .iter()
                                .copied()
                                .filter(|t| t[axis] >= a && t[axis] < b)
                                .collect();
                            assert_eq!(child.as_ref().map(tuples).unwrap_or_default(), expected);
                            if let Some(child) = child {
                                assert_eq!(child.coordinate_count(), expected.len() as u128);
                                assert_eq!(
                                    RankedProductBox::from_expr(&child.expression().unwrap())
                                        .unwrap(),
                                    child
                                );
                                let enclosure = child.enclosure().unwrap();
                                assert!(expected.iter().all(|t| t
                                    .iter()
                                    .zip(&enclosure)
                                    .all(|(v, (a, b))| v >= a && v <= b)));
                                let other = (axis + 1) % 3;
                                let nested = child
                                    .restrict_factor(other, factors[other].0, factors[other].0 + 1)
                                    .unwrap();
                                assert_eq!(
                                    nested.as_ref().map(tuples).unwrap_or_default(),
                                    expected
                                        .into_iter()
                                        .filter(|t| t[other] == factors[other].0)
                                        .collect::<Vec<_>>()
                                );
                            }
                        }
                    }
                    assert!(parent.restrict_factor(axis, low - 1, high).is_err());
                    assert!(parent.restrict_factor(axis, low, high + 1).is_err());
                }
            }
        }
    }

    #[test]
    fn ranked_box_counts_full_income_commute_boundary_slabs_without_enumeration() {
        let factors = [(0, 400001), (0, 201), (0, 2)];
        let first = page(&factors, 0, 65536);
        let salary = first.restrict_factor(2, 0, 1).unwrap().unwrap();
        let commute = first.restrict_factor(2, 1, 2).unwrap().unwrap();
        let inward = commute.restrict_factor(1, 0, 200).unwrap().unwrap();
        let outward = commute.restrict_factor(1, 200, 201).unwrap().unwrap();
        assert_eq!(salary.coordinate_count(), 32768);
        assert_eq!(inward.coordinate_count(), 32605);
        assert_eq!(outward.coordinate_count(), 163);
        assert_eq!(
            salary.coordinate_count() + inward.coordinate_count() + outward.coordinate_count(),
            65536
        );
        assert!(first.restrict_factor(0, 400000, 400001).unwrap().is_none());
        let full = page(&factors, 0, 160800402);
        let outward_salary = full
            .restrict_factor(2, 0, 1)
            .unwrap()
            .unwrap()
            .restrict_factor(0, 400000, 400001)
            .unwrap()
            .unwrap();
        let outward_commute = full
            .restrict_factor(2, 1, 2)
            .unwrap()
            .unwrap()
            .restrict_factor(1, 200, 201)
            .unwrap()
            .unwrap();
        assert_eq!(outward_salary.coordinate_count(), 201);
        assert_eq!(outward_commute.coordinate_count(), 400001);
        assert_eq!(
            full.coordinate_count()
                - outward_salary.coordinate_count()
                - outward_commute.coordinate_count(),
            160400200
        );
    }
}
