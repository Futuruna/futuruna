//! Bounded correlations for the checked abstract interpreter. Intervals remain
//! the authority for overflow; this optional companion only tightens differences
//! between values derived from the same finite source coordinates.

use sha2::{Digest, Sha256};

pub(super) const MAX_AXES: usize = 8;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct Correlation {
    coefficients: [i128; MAX_AXES],
    constant: i128,
    denominator: i128,
    // Error numerators share `denominator` with the affine part. Preserve
    // fractional error through nested rounding rather than rounding each
    // intermediate error enclosure out to a whole integer again.
    error: (i128, i128),
    // Exact integer congruence: value = residue (mod modulus). Modulus zero
    // denotes a constant. This retains whole-krone rounding after scaling
    // back to øre, without confusing a rational enclosure with real values.
    modulus: i128,
    residue: i128,
    // Equal enclosures alone are not equal values. Only the exact expression
    // identity can cancel the uncertainty introduced by integer rounding.
    expression: [u8; 32],
}

fn identity(operation: &[u8], inputs: &[&[u8]]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"futuruna.checked-abstract-correlation.v1\0");
    hash.update((operation.len() as u64).to_le_bytes());
    hash.update(operation);
    for input in inputs {
        hash.update((input.len() as u64).to_le_bytes());
        hash.update(input);
    }
    hash.finalize().into()
}

fn gcd(mut left: i128, mut right: i128) -> Option<i128> {
    left = left.checked_abs()?;
    right = right.checked_abs()?;
    while right != 0 {
        (left, right) = (right, left.checked_rem(right)?);
    }
    Some(left)
}

fn floor_ratio(value: i128, denominator: i128) -> Option<i128> {
    let quotient = value.checked_div(denominator)?;
    quotient.checked_sub(i128::from(value.checked_rem(denominator)? < 0))
}

fn ceil_ratio(value: i128, denominator: i128) -> Option<i128> {
    let quotient = value.checked_div(denominator)?;
    quotient.checked_add(i128::from(value.checked_rem(denominator)? > 0))
}

impl Correlation {
    /// An otherwise opaque, total pure result. Its interval is represented as
    /// bounded error, not as an invented linear function. The checked caller
    /// may supply an exact identity only when all inputs are symbolically
    /// identified; equal numeric enclosures alone never establish identity.
    pub(super) fn opaque(expression: [u8; 32], bounds: (i128, i128)) -> Option<Self> {
        (bounds.0 <= bounds.1).then_some(Self {
            coefficients: [0; MAX_AXES],
            constant: 0,
            denominator: 1,
            error: bounds,
            modulus: 1,
            residue: 0,
            expression,
        })
    }

    pub(super) fn axis(axis: usize) -> Option<Self> {
        let mut coefficients = [0; MAX_AXES];
        *coefficients.get_mut(axis)? = 1;
        Some(Self {
            coefficients,
            constant: 0,
            denominator: 1,
            error: (0, 0),
            modulus: 1,
            residue: 0,
            expression: identity(b"axis", &[&(axis as u64).to_le_bytes()]),
        })
    }

    pub(super) fn constant(value: i128) -> Self {
        Self {
            coefficients: [0; MAX_AXES],
            constant: value,
            denominator: 1,
            error: (0, 0),
            modulus: 0,
            residue: value,
            expression: identity(b"constant", &[&value.to_le_bytes()]),
        }
    }

    fn normalized(mut self) -> Option<Self> {
        if self.denominator <= 0 || self.error.0 > self.error.1 {
            return None;
        }
        let mut divisor = gcd(self.denominator, self.constant)?;
        for coefficient in self.coefficients {
            divisor = gcd(divisor, coefficient)?;
        }
        divisor = gcd(divisor, self.error.0)?;
        divisor = gcd(divisor, self.error.1)?;
        self.denominator /= divisor;
        self.constant /= divisor;
        self.error.0 /= divisor;
        self.error.1 /= divisor;
        for coefficient in &mut self.coefficients {
            *coefficient /= divisor;
        }
        if self.error == (0, 0) {
            let mut hash = Sha256::new();
            hash.update(b"futuruna.checked-abstract-exact-affine.v1\0");
            for coefficient in self.coefficients {
                hash.update(coefficient.to_le_bytes());
            }
            hash.update(self.constant.to_le_bytes());
            hash.update(self.denominator.to_le_bytes());
            self.expression = hash.finalize().into();
        }
        Some(self)
    }

    pub(super) fn add(self, other: Self) -> Option<Self> {
        if other.modulus == 0 && other.residue == 0 {
            return Some(self);
        }
        if self.modulus == 0 && self.residue == 0 {
            return Some(other);
        }
        let common = gcd(self.denominator, other.denominator)?;
        let left_scale = other.denominator.checked_div(common)?;
        let right_scale = self.denominator.checked_div(common)?;
        let mut coefficients = [0; MAX_AXES];
        for (axis, result) in coefficients.iter_mut().enumerate() {
            *result = self.coefficients[axis]
                .checked_mul(left_scale)?
                .checked_add(other.coefficients[axis].checked_mul(right_scale)?)?;
        }
        let modulus = gcd(self.modulus, other.modulus)?;
        let residue = self.residue.checked_add(other.residue)?;
        Self {
            coefficients,
            constant: self
                .constant
                .checked_mul(left_scale)?
                .checked_add(other.constant.checked_mul(right_scale)?)?,
            denominator: self.denominator.checked_mul(left_scale)?,
            error: (
                self.error
                    .0
                    .checked_mul(left_scale)?
                    .checked_add(other.error.0.checked_mul(right_scale)?)?,
                self.error
                    .1
                    .checked_mul(left_scale)?
                    .checked_add(other.error.1.checked_mul(right_scale)?)?,
            ),
            modulus,
            residue: if modulus == 0 {
                residue
            } else {
                residue.checked_rem_euclid(modulus)?
            },
            expression: identity(b"add", &[&self.expression, &other.expression]),
        }
        .normalized()
    }

    pub(super) fn scale(self, factor: i128) -> Option<Self> {
        if factor == 0 {
            return Some(Self::constant(0));
        }
        if factor == 1 {
            return Some(self);
        }
        let mut result = self;
        result.modulus = result.modulus.checked_mul(factor.checked_abs()?)?;
        result.residue = result.residue.checked_mul(factor)?;
        if result.modulus != 0 {
            result.residue = result.residue.checked_rem_euclid(result.modulus)?;
        }
        for coefficient in &mut result.coefficients {
            *coefficient = coefficient.checked_mul(factor)?;
        }
        result.constant = result.constant.checked_mul(factor)?;
        let a = self.error.0.checked_mul(factor)?;
        let b = self.error.1.checked_mul(factor)?;
        result.error = (a.min(b), a.max(b));
        result.expression = identity(b"scale", &[&self.expression, &factor.to_le_bytes()]);
        result.normalized()
    }

    pub(super) fn divide(self, divisor: i128, numerator_bounds: (i128, i128)) -> Option<Self> {
        if divisor == 0 {
            return None;
        }
        if divisor == 1 {
            return Some(self);
        }
        let magnitude = divisor.checked_abs()?;
        let mut result = if divisor < 0 { self.scale(-1)? } else { self };
        result.denominator = result.denominator.checked_mul(magnitude)?;
        // Truncation is exact only when the unrounded affine value is already
        // integral, or the actual integer's congruence proves divisibility.
        let divisible = result.modulus.checked_rem(magnitude)? == 0
            && result.residue.checked_rem(magnitude)? == 0;
        let exact = divisible
            || (result.error == (0, 0)
                && result.constant.checked_rem(result.denominator)? == 0
                && result
                    .coefficients
                    .iter()
                    .all(|coefficient| coefficient.checked_rem(result.denominator) == Some(0)));
        if !exact {
            let bounds = if divisor < 0 {
                (
                    numerator_bounds.1.checked_neg()?,
                    numerator_bounds.0.checked_neg()?,
                )
            } else {
                numerator_bounds
            };
            if bounds.1 > 0 {
                result.error.0 = result.error.0.checked_sub(result.denominator)?;
            }
            if bounds.0 < 0 {
                result.error.1 = result.error.1.checked_add(result.denominator)?;
            }
        }
        result.expression = identity(
            b"divide-truncate",
            &[&self.expression, &divisor.to_le_bytes()],
        );
        // Truncation through zero is not generally congruence-preserving.
        // Divisible residues and moduli need no truncation and are exact.
        if divisible {
            result.modulus /= magnitude;
            result.residue /= magnitude;
        } else {
            result.modulus = 1;
            result.residue = 0;
        }
        result.normalized()
    }

    /// Domain-independent difference when all varying coefficients cancel.
    /// The result bounds an integer, hence ceil/floor tighten rational bounds.
    pub(super) fn difference(self, other: Self) -> Option<(i128, i128)> {
        if self.expression == other.expression {
            return Some((0, 0));
        }
        let difference = self.add(other.scale(-1)?)?;
        if difference
            .coefficients
            .iter()
            .any(|coefficient| *coefficient != 0)
        {
            return None;
        }
        let mut low = ceil_ratio(
            difference.constant.checked_add(difference.error.0)?,
            difference.denominator,
        )?;
        let mut high = floor_ratio(
            difference.constant.checked_add(difference.error.1)?,
            difference.denominator,
        )?;
        if difference.modulus == 0 {
            low = low.max(difference.residue);
            high = high.min(difference.residue);
        } else {
            low = low.checked_add(
                difference
                    .residue
                    .checked_sub(low)?
                    .checked_rem_euclid(difference.modulus)?,
            )?;
            high = high.checked_sub(
                high.checked_sub(difference.residue)?
                    .checked_rem_euclid(difference.modulus)?,
            )?;
        }
        (low <= high).then_some((low, high))
    }

    pub(super) fn digest(self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"futuruna.checked-abstract-correlation-value.v1\0");
        for coefficient in self.coefficients {
            hash.update(coefficient.to_le_bytes());
        }
        hash.update(self.constant.to_le_bytes());
        hash.update(self.denominator.to_le_bytes());
        hash.update(self.error.0.to_le_bytes());
        hash.update(self.error.1.to_le_bytes());
        hash.update(self.modulus.to_le_bytes());
        hash.update(self.residue.to_le_bytes());
        hash.update(self.expression);
        hash.finalize().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_rounding_retains_fractional_error_and_exact_rescaling() {
        let axis = Correlation::axis(0).unwrap();
        let base = axis
            .scale(46284)
            .unwrap()
            .divide(100, (0, 2_000_000))
            .unwrap();
        let supplement = base.scale(64).unwrap().divide(100, (0, 2_000_000)).unwrap();
        let contribution = base.add(supplement).unwrap().scale(2339).unwrap();
        let before = contribution.divide(100, (0, 100_000_000)).unwrap();
        let after = contribution
            .add(Correlation::constant(6461))
            .unwrap()
            .divide(100, (6461, 100_006_461))
            .unwrap();
        let (low, high) = after.difference(before).unwrap();
        assert!(
            low > 0,
            "fractional error must not grow to a false loss: {low}..{high}"
        );
        for x in 0..=30 {
            let base = x * 46284 / 100;
            let contribution = (base + base * 64 / 100) * 2339;
            let delta = (contribution + 6461) / 100 - contribution / 100;
            assert!(low <= delta && delta <= high);
        }
        // Scaling an uncertain integer and exactly undoing that scaling
        // introduces no new truncation error, including negative divisors.
        for factor in [-100, 100] {
            let restored = base
                .scale(factor)
                .unwrap()
                .divide(factor, (-2_000_000, 2_000_000))
                .unwrap();
            assert_eq!(restored.coefficients, base.coefficients);
            assert_eq!(restored.constant, base.constant);
            assert_eq!(restored.denominator, base.denominator);
            assert_eq!(restored.error, base.error);
        }
    }

    #[test]
    fn adjacent_integer_rounding_differences_enclose_concrete_edges() {
        let axis = Correlation::axis(0).unwrap();
        for coefficient in [-7, -1, 1, 7, 37] {
            for divisor in [-11, -3, 2, 9] {
                let before = axis.scale(coefficient).unwrap();
                let after = before.add(Correlation::constant(coefficient)).unwrap();
                let before = before.divide(divisor, (-1000, 1000)).unwrap();
                let after = after.divide(divisor, (-1000, 1000)).unwrap();
                let (low, high) = after.difference(before).unwrap();
                for x in -20..20 {
                    let delta = (coefficient * (x + 1)) / divisor - (coefficient * x) / divisor;
                    assert!(
                        low <= delta && delta <= high,
                        "{coefficient} {divisor} {x}: {low}..{high}, {delta}"
                    );
                }
                assert_eq!(before.difference(before), Some((0, 0)));
            }
        }
        assert!(Correlation::axis(MAX_AXES).is_none());
        assert!(axis.divide(0, (0, 10)).is_none());
        assert!(axis.scale(i128::MAX).unwrap().scale(2).is_none());
        let before = axis
            .scale(8)
            .unwrap()
            .divide(100, (0, 8000))
            .unwrap()
            .scale(100)
            .unwrap();
        let after = axis
            .add(Correlation::constant(1))
            .unwrap()
            .scale(8)
            .unwrap()
            .divide(100, (8, 8008))
            .unwrap()
            .scale(100)
            .unwrap();
        assert_eq!(after.difference(before), Some((0, 100)));
        assert_eq!(
            axis.add(Correlation::constant(0)).unwrap().difference(axis),
            Some((0, 0))
        );
    }
}
