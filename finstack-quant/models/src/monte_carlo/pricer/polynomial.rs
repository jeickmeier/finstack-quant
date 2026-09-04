//! Standardized multivariate polynomial regression for Monte Carlo policies.
//!
//! The fitted policy owns its centering, scaling, basis selection, and
//! coefficients, so it can be frozen after training and replayed on an
//! independent pricing sample. Polynomial degree is deliberately bounded to
//! quadratic production fits and cubic validation fits.

use super::lsq::solve_least_squares;
use finstack_quant_core::{Error, Result};

const STANDARD_DEVIATION_EPSILON: f64 = 1.0e-12;

/// Maximum multivariate polynomial degree supported by the policy fitter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolynomialDegree {
    /// Linear, squared, and pairwise-interaction terms.
    Quadratic,
    /// All total-degree-three terms in addition to the quadratic basis.
    Cubic,
}

/// Frozen standardized multivariate polynomial regression policy.
///
/// Constant input dimensions are omitted from the fitted basis. When there
/// are too few observations for the requested basis, fitting falls back to a
/// smaller polynomial basis and ultimately to an intercept-only policy. This
/// keeps exercise-policy replay independent of realized future values while
/// handling small or degenerate training samples deterministically.
#[derive(Debug, Clone)]
pub struct StandardizedPolynomialPolicy<const N: usize> {
    means: [f64; N],
    scales: [f64; N],
    terms: Vec<BasisTerm>,
    coefficients: Vec<f64>,
}

impl<const N: usize> StandardizedPolynomialPolicy<N> {
    /// Fit a bounded-degree polynomial policy to paired observations.
    ///
    /// The fitter standardizes each active feature with its sample mean and
    /// standard deviation. A dimension whose standard deviation is negligible
    /// relative to its mean is treated as constant and removed. If the full
    /// requested basis cannot be fit, the candidate sequence is reduced in a
    /// deterministic order: cubic, full quadratic, quadratic without
    /// interactions, linear, then intercept-only.
    ///
    /// # Arguments
    ///
    /// * `features` - Paired multivariate state observations. Every scalar must
    ///   be finite and each row has the compile-time dimension `N`.
    /// * `responses` - Finite regression targets paired one-for-one with
    ///   `features`, in the value units the policy should predict.
    /// * `degree` - Maximum total polynomial degree to attempt. Quadratic is
    ///   intended for production policies; cubic supports refinement checks.
    ///
    /// # Returns
    ///
    /// A frozen policy containing the selected reduced basis, standardization,
    /// and least-squares coefficients.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] for empty, mismatched, or non-finite
    /// samples. Returns an internal error if even the intercept-only
    /// least-squares fit cannot be produced.
    pub fn fit(features: &[[f64; N]], responses: &[f64], degree: PolynomialDegree) -> Result<Self> {
        if features.len() != responses.len() || features.is_empty() {
            return Err(Error::Validation(
                "standardized polynomial regression requires equal non-empty feature and response sets"
                    .to_string(),
            ));
        }
        if features
            .iter()
            .flatten()
            .chain(responses)
            .any(|value| !value.is_finite())
        {
            return Err(Error::Validation(
                "standardized polynomial regression inputs must be finite".to_string(),
            ));
        }

        let (means, scales, active) = standardization(features);
        let candidates = candidate_bases(&active, degree);
        let mut last_error = None;
        for terms in candidates {
            if features.len() < terms.len() {
                continue;
            }
            let mut design = Vec::with_capacity(features.len() * terms.len());
            for row in features {
                let standardized = standardize(row, &means, &scales);
                design.extend(terms.iter().map(|term| evaluate_term(*term, &standardized)));
            }
            match solve_least_squares(&design, responses, features.len(), terms.len()) {
                Ok(coefficients) if coefficients.iter().all(|value| value.is_finite()) => {
                    return Ok(Self {
                        means,
                        scales,
                        terms,
                        coefficients,
                    });
                }
                Ok(_) => {
                    last_error = Some(Error::internal(
                        "standardized polynomial regression returned non-finite coefficients",
                    ));
                }
                Err(error) => last_error = Some(error),
            }
        }

        Err(last_error.unwrap_or_else(|| {
            Error::internal(
                "standardized polynomial regression could not fit even an intercept-only policy",
            )
        }))
    }

    /// Predict one response with the frozen training transformation and basis.
    ///
    /// # Arguments
    ///
    /// * `features` - State vector in the same units and dimension used for
    ///   fitting. Every scalar must be finite.
    ///
    /// # Returns
    ///
    /// The fitted response in the training response units.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when an input is non-finite and an
    /// internal error if polynomial evaluation produces a non-finite result.
    pub fn predict(&self, features: &[f64; N]) -> Result<f64> {
        if features.iter().any(|value| !value.is_finite()) {
            return Err(Error::Validation(
                "standardized polynomial prediction inputs must be finite".to_string(),
            ));
        }
        let standardized = standardize(features, &self.means, &self.scales);
        let prediction = self
            .terms
            .iter()
            .zip(&self.coefficients)
            .map(|(term, coefficient)| coefficient * evaluate_term(*term, &standardized))
            .sum::<f64>();
        if prediction.is_finite() {
            Ok(prediction)
        } else {
            Err(Error::internal(
                "standardized polynomial regression produced a non-finite prediction",
            ))
        }
    }

    /// Number of basis terms retained by the fitted policy.
    #[must_use]
    pub fn num_terms(&self) -> usize {
        self.terms.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BasisTerm {
    Constant,
    Linear(usize),
    Square(usize),
    Interaction(usize, usize),
    Cube(usize),
    SquareLinear(usize, usize),
    TripleInteraction(usize, usize, usize),
}

fn standardization<const N: usize>(features: &[[f64; N]]) -> ([f64; N], [f64; N], Vec<usize>) {
    let mut means = [0.0; N];
    for row in features {
        for (mean, value) in means.iter_mut().zip(row) {
            *mean += value;
        }
    }
    for mean in &mut means {
        *mean /= features.len() as f64;
    }

    let mut scales = [0.0; N];
    for row in features {
        for ((scale, value), mean) in scales.iter_mut().zip(row).zip(means) {
            *scale += (value - mean).powi(2);
        }
    }
    let denominator = features.len().saturating_sub(1).max(1) as f64;
    let mut active = Vec::new();
    for index in 0..N {
        scales[index] = (scales[index] / denominator).sqrt();
        let threshold = STANDARD_DEVIATION_EPSILON * means[index].abs().max(1.0);
        if scales[index] > threshold {
            active.push(index);
        } else {
            scales[index] = 1.0;
        }
    }
    (means, scales, active)
}

fn standardize<const N: usize>(
    features: &[f64; N],
    means: &[f64; N],
    scales: &[f64; N],
) -> [f64; N] {
    let mut standardized = [0.0; N];
    for index in 0..N {
        standardized[index] = (features[index] - means[index]) / scales[index];
    }
    standardized
}

fn candidate_bases(active: &[usize], degree: PolynomialDegree) -> Vec<Vec<BasisTerm>> {
    let mut candidates = Vec::with_capacity(5);
    if degree == PolynomialDegree::Cubic {
        candidates.push(cubic_terms(active));
    }
    candidates.push(quadratic_terms(active, true));
    candidates.push(quadratic_terms(active, false));
    candidates.push(linear_terms(active));
    candidates.push(vec![BasisTerm::Constant]);
    candidates.dedup();
    candidates
}

fn linear_terms(active: &[usize]) -> Vec<BasisTerm> {
    let mut terms = Vec::with_capacity(1 + active.len());
    terms.push(BasisTerm::Constant);
    terms.extend(active.iter().copied().map(BasisTerm::Linear));
    terms
}

fn quadratic_terms(active: &[usize], interactions: bool) -> Vec<BasisTerm> {
    let mut terms = linear_terms(active);
    terms.extend(active.iter().copied().map(BasisTerm::Square));
    if interactions {
        for (position, &left) in active.iter().enumerate() {
            for &right in &active[position + 1..] {
                terms.push(BasisTerm::Interaction(left, right));
            }
        }
    }
    terms
}

fn cubic_terms(active: &[usize]) -> Vec<BasisTerm> {
    let mut terms = quadratic_terms(active, true);
    terms.extend(active.iter().copied().map(BasisTerm::Cube));
    for &squared in active {
        for &linear in active {
            if squared != linear {
                terms.push(BasisTerm::SquareLinear(squared, linear));
            }
        }
    }
    for (left_position, &left) in active.iter().enumerate() {
        for (middle_offset, &middle) in active[left_position + 1..].iter().enumerate() {
            for &right in &active[left_position + middle_offset + 2..] {
                terms.push(BasisTerm::TripleInteraction(left, middle, right));
            }
        }
    }
    terms
}

fn evaluate_term<const N: usize>(term: BasisTerm, row: &[f64; N]) -> f64 {
    match term {
        BasisTerm::Constant => 1.0,
        BasisTerm::Linear(index) => row[index],
        BasisTerm::Square(index) => row[index] * row[index],
        BasisTerm::Interaction(left, right) => row[left] * row[right],
        BasisTerm::Cube(index) => row[index] * row[index] * row[index],
        BasisTerm::SquareLinear(squared, linear) => row[squared] * row[squared] * row[linear],
        BasisTerm::TripleInteraction(left, middle, right) => row[left] * row[middle] * row[right],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quadratic_policy_recovers_interactions_and_drops_constant_dimensions() {
        let mut features = Vec::new();
        let mut responses = Vec::new();
        for left in -3..=3 {
            for right in -3..=3 {
                let x = left as f64 / 2.0;
                let y = right as f64 / 3.0;
                features.push([x, y, 7.0]);
                responses.push(4.0 + 2.0 * x - 3.0 * y + 0.5 * x * x + 1.25 * x * y);
            }
        }
        let policy =
            StandardizedPolynomialPolicy::fit(&features, &responses, PolynomialDegree::Quadratic)
                .expect("quadratic policy");
        // Two active dimensions: intercept + 2 linear + 2 squares + interaction.
        assert_eq!(policy.num_terms(), 6);
        let actual = policy.predict(&[0.7, -0.4, 7.0]).expect("prediction");
        let expected = 4.0 + 2.0 * 0.7 - 3.0 * -0.4 + 0.5 * 0.7_f64.powi(2) + 1.25 * 0.7 * -0.4;
        assert!((actual - expected).abs() < 1.0e-10);
    }

    #[test]
    fn cubic_policy_recovers_total_degree_three_surface() {
        let mut features = Vec::new();
        let mut responses = Vec::new();
        for first in -2..=2 {
            for second in -2..=2 {
                for third in -2..=2 {
                    let x = first as f64 / 2.0;
                    let y = second as f64 / 2.5;
                    let z = third as f64 / 3.0;
                    features.push([x, y, z]);
                    responses.push(
                        2.0 + x - 0.5 * y + 0.25 * z + 0.7 * x * x * y - 0.4 * y * z * z
                            + 1.1 * x * y * z
                            + 0.3 * z * z * z,
                    );
                }
            }
        }
        let policy =
            StandardizedPolynomialPolicy::fit(&features, &responses, PolynomialDegree::Cubic)
                .expect("cubic policy");
        // C(3 + 3, 3) total monomials of degree <= 3.
        assert_eq!(policy.num_terms(), 20);
        let [x, y, z] = [0.35, -0.22, 0.41];
        let expected = 2.0 + x - 0.5 * y + 0.25 * z + 0.7 * x * x * y - 0.4 * y * z * z
            + 1.1 * x * y * z
            + 0.3 * z * z * z;
        let actual = policy.predict(&[x, y, z]).expect("prediction");
        assert!((actual - expected).abs() < 1.0e-10);
    }

    #[test]
    fn undersampled_cubic_fit_reduces_to_linear_without_future_value_fallback() {
        let features = [[-2.0, -1.0], [-1.0, 1.0], [0.0, -1.0], [1.0, 1.0]];
        let responses = [0.0, -4.0, 4.0, 0.0];
        let policy =
            StandardizedPolynomialPolicy::fit(&features, &responses, PolynomialDegree::Cubic)
                .expect("reduced policy");
        assert_eq!(policy.num_terms(), 3);
        assert!((policy.predict(&[2.0, 0.5]).expect("prediction") - 3.5).abs() < 1.0e-10);
    }

    #[test]
    fn constant_sample_reduces_to_intercept() {
        let features = [[1.0, 2.0]; 8];
        let responses = [2.0, 4.0, 3.0, 5.0, 1.0, 7.0, 6.0, 4.0];
        let policy =
            StandardizedPolynomialPolicy::fit(&features, &responses, PolynomialDegree::Quadratic)
                .expect("intercept policy");
        assert_eq!(policy.num_terms(), 1);
        assert!((policy.predict(&[1.0, 2.0]).expect("prediction") - 4.0).abs() < 1.0e-12);
    }
}
