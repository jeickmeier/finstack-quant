//! Dense count probabilities for independent trials with a common success probability.

use crate::math::interp::exact_f64_eq;

/// Compute the full binomial PMF `P(K = k)` for every `k` in `0..=n` in one pass.
///
/// Returns a vector `pmf` of length `n + 1` where `pmf[k]` is the probability of
/// exactly `k` successes in `n` independent trials with common probability
/// `p`. The O(n) recurrence works outward from the upper mode and normalizes
/// once. Probabilities too small to represent in binary64 can round to zero.
///
/// # Arguments
///
/// * `n` - Number of independent trials, at most 1,000,000.
/// * `p` - Per-trial success probability in the closed unit interval.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::distributions::binomial_pmf_all;
///
/// let pmf = binomial_pmf_all(10, 0.5)?;
/// assert_eq!(pmf.len(), 11);
/// assert!((pmf[5] - 0.24609375).abs() < 1e-12);
/// assert!((pmf.iter().sum::<f64>() - 1.0).abs() < 1e-12);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
///
/// # Errors
///
/// Returns an error on invalid inputs, allocation failure or unresolved probability mass.
///
/// # References
///
/// - Johnson, N. L., Kotz, S., & Kemp, A. W. (1993). *Univariate Discrete Distributions*
///   (2nd ed.). Wiley. Chapter 3. `docs/REFERENCES.md#press-numerical-recipes`
pub fn binomial_pmf_all(n: usize, p: f64) -> crate::Result<Vec<f64>> {
    let mut pmf = Vec::new();
    binomial_pmf_all_into(&mut pmf, n, p)?;
    Ok(pmf)
}

/// In-place variant of [`binomial_pmf_all`] that writes the PMF into `out`
/// (cleared and resized to `n + 1`) rather than allocating a fresh vector.
///
/// Hot loops that evaluate the binomial PMF many times — e.g. a CDS-tranche
/// factor-quadrature integrand, which calls this once per Gauss-Hermite node
/// per payment date — can pass a reusable scratch buffer to avoid an
/// allocation on every evaluation.
///
/// # Arguments
///
/// * `out` - Reusable output buffer cleared and resized to `n + 1`, then filled
///   with probabilities for zero through `n` successes.
/// * `n` - Number of independent trials, at most 1,000,000.
/// * `p` - Per-trial success probability in the closed unit interval.
///
/// # Errors
///
/// Invalid arguments and reservation failures leave the output buffer intact.
/// Discard its contents if a numerical error occurs during the recurrence.
pub fn binomial_pmf_all_into(out: &mut Vec<f64>, n: usize, p: f64) -> crate::Result<()> {
    if !(0.0..=1.0).contains(&p) || n > 1_000_000 {
        return Err(crate::Error::Validation(
            "binomial PMF requires p in [0, 1] and at most 1,000,000 trials".into(),
        ));
    }
    let len = n + 1;
    out.try_reserve_exact(len.saturating_sub(out.len()))
        .map_err(|err| {
            crate::Error::Validation(format!("cannot reserve {len} binomial cells: {err}"))
        })?;
    out.resize(len, 0.0);
    if n == 0 || p == 0.0 || exact_f64_eq(p, 1.0) {
        out.fill(0.0);
        out[if exact_f64_eq(p, 1.0) { n } else { 0 }] = 1.0;
        return Ok(());
    }

    let mode = binomial_mode(n, p);
    out[mode] = 1.0;
    for k in (1..=mode).rev() {
        let count = k as f64;
        let numerator = (-p).mul_add(count, count);
        let denominator = (n - k + 1) as f64 * p;
        out[k - 1] = outward_binomial_weight(out[k], numerator, denominator)?;
    }
    for k in mode..n {
        let next = (k + 1) as f64;
        let numerator = (n - k) as f64 * p;
        let denominator = (-p).mul_add(next, next);
        out[k + 1] = outward_binomial_weight(out[k], numerator, denominator)?;
    }

    let total = crate::math::summation::neumaier_sum(out.iter().copied())?;
    if !(1.0..=len as f64).contains(&total) {
        return Err(crate::Error::Internal(
            "invalid binomial relative mass".into(),
        ));
    }
    for value in &mut *out {
        *value /= total;
    }
    let mass = crate::math::summation::neumaier_sum(out.iter().copied())?;
    if (mass - 1.0).abs() > 8.0 * f64::EPSILON {
        return Err(crate::Error::Internal(
            "binomial PMF lost probability mass".into(),
        ));
    }
    Ok(())
}

// Compute floor((n+1)*p) for the exact bits of p. With n <= 1,000,000 the
// integer product needs at most 73 bits. Tiny probabilities shift it to zero.
fn binomial_mode(n: usize, p: f64) -> usize {
    let (significand, exponent, _) = num_traits::Float::integer_decode(p);
    let product = (n + 1) as u128 * u128::from(significand);
    product
        .checked_shr(u32::from(exponent.unsigned_abs()))
        .unwrap_or(0) as usize
}

fn outward_binomial_weight(weight: f64, numerator: f64, denominator: f64) -> crate::Result<f64> {
    // The exact positive denominator corresponds to a nonempty side of the
    // mode. Each product was rounded once, so an outward ratio cannot exceed 1.
    let ratio = numerator / denominator;
    if !(0.0..=1.0).contains(&ratio) {
        return Err(crate::Error::Internal(
            "invalid outward binomial ratio".into(),
        ));
    }
    Ok(weight * ratio)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binomial_pmf_all() {
        // Test basic distribution
        let dist = binomial_pmf_all(10, 0.5).unwrap();
        assert_eq!(dist.len(), 11);

        // Test P(X=5) for fair coin
        assert!(
            (dist[5] - 0.24609375).abs() < 1e-6,
            "P(X=5) = {}, expected 0.24609375",
            dist[5]
        );

        // Test normalization
        let sum: f64 = dist.iter().sum();
        assert!(
            (sum - 1.0).abs() < 1e-10,
            "Distribution sum = {}, expected 1.0",
            sum
        );

        // Test symmetry for p=0.5
        for k in 0..=5 {
            assert!(
                (dist[k] - dist[10 - k]).abs() < 1e-10,
                "P({}) = {} should equal P({}) = {}",
                k,
                dist[k],
                10 - k,
                dist[10 - k]
            );
        }
    }

    #[test]
    fn test_binomial_distribution_edge_cases() {
        // p = 0: all probability on k=0
        let dist_zero = binomial_pmf_all(5, 0.0).unwrap();
        assert!((dist_zero[0] - 1.0).abs() < 1e-10);
        for val in dist_zero.iter().skip(1) {
            assert!(*val < 1e-10);
        }

        // p = 1: all probability on k=n
        let dist_one = binomial_pmf_all(5, 1.0).unwrap();
        assert!((dist_one[5] - 1.0).abs() < 1e-10);
        for val in dist_one.iter().take(5) {
            assert!(*val < 1e-10);
        }

        // n = 0: single element
        let dist_n0 = binomial_pmf_all(0, 0.5).unwrap();
        assert_eq!(dist_n0.len(), 1);
        assert!((dist_n0[0] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_binomial_distribution_rejects_nan_p() {
        assert!(binomial_pmf_all(10, f64::NAN).is_err());
    }

    #[test]
    fn test_binomial_distribution_credit_portfolio() {
        // Typical credit portfolio: 100 names with 5% PD
        let dist = binomial_pmf_all(100, 0.05).unwrap();
        assert_eq!(dist.len(), 101);

        // Expected number of defaults = n * p = 5
        // Most probability mass should be around 5
        let expected_mean: f64 = (0..=100).map(|k| k as f64 * dist[k]).sum();
        assert!(
            (expected_mean - 5.0).abs() < 0.01,
            "Mean = {}, expected ~5.0",
            expected_mean
        );

        // Variance = n * p * (1-p) = 4.75
        let expected_var: f64 = (0..=100)
            .map(|k| (k as f64 - expected_mean).powi(2) * dist[k])
            .sum();
        assert!(
            (expected_var - 4.75).abs() < 0.01,
            "Variance = {}, expected ~4.75",
            expected_var
        );
    }

    #[test]
    fn binomial_pmf_all_matches_statrs() {
        use statrs::distribution::{Binomial, Discrete};

        for &(n, p) in &[(10usize, 0.3f64), (50, 0.05), (126, 0.5), (20, 0.9)] {
            let reference = Binomial::new(p, n as u64).expect("valid binomial");
            let pmf = binomial_pmf_all(n, p).expect("valid probability should produce a PMF");
            assert_eq!(pmf.len(), n + 1);
            for (k, &prob) in pmf.iter().enumerate() {
                let expected = reference.pmf(k as u64);
                assert!(
                    (prob - expected).abs() < 1e-10,
                    "n={n}, k={k}, p={p}: recurrence {prob} vs statrs {expected}"
                );
            }
            let total: f64 = pmf.iter().sum();
            assert!((total - 1.0).abs() < 1e-9, "n={n}, p={p}: pmf sum={total}");
        }
    }

    #[test]
    fn binomial_pmf_all_large_n_keeps_probability_mass() {
        let pmf = binomial_pmf_all(2_000, 0.5).expect("valid probability should produce a PMF");
        let sum: f64 = pmf.iter().sum();
        let mode = pmf[1_000];

        assert!(
            (sum - 1.0).abs() < 1e-10,
            "large-n PMF must sum to 1, got {sum}"
        );
        assert!(
            mode > 0.0,
            "mode probability should be positive for n=2000, p=0.5"
        );
    }

    #[test]
    fn binomial_pmf_matches_independent_distribution_at_extreme_probabilities() {
        use statrs::distribution::{Binomial, Discrete};
        for (n, p) in [(2_000, 0.001), (2_000, 0.5), (2_000, 0.999)] {
            let reference = Binomial::new(p, n as u64).expect("valid distribution");
            let values = binomial_pmf_all(n, p).expect("valid probability");
            assert!((values.iter().sum::<f64>() - 1.0).abs() < 1e-10);
            for (k, actual) in values.into_iter().enumerate() {
                assert!((actual - reference.pmf(k as u64)).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn binomial_pmf_all_handles_degenerate_p() {
        let lo = binomial_pmf_all(5, 0.0).expect("zero should produce a degenerate PMF");
        assert_eq!(lo[0], 1.0);
        assert!(lo[1..].iter().all(|&x| x == 0.0));

        let hi = binomial_pmf_all(5, 1.0).expect("one should produce a degenerate PMF");
        assert_eq!(hi[5], 1.0);
        assert!(hi[..5].iter().all(|&x| x == 0.0));
    }

    #[test]
    fn binomial_pmf_all_rejects_nan_without_mutating_scratch() {
        let error = binomial_pmf_all(5, f64::NAN).expect_err("NaN must be rejected");
        assert!(matches!(error, crate::Error::Validation(_)));

        let mut scratch = vec![0.25, 0.75];
        let error = binomial_pmf_all_into(&mut scratch, 5, f64::NAN)
            .expect_err("in-place NaN must be rejected");
        assert!(matches!(error, crate::Error::Validation(_)));
        assert_eq!(scratch, vec![0.25, 0.75]);
    }

    // Chi-Squared Distribution Tests

    #[test]
    fn binomial_recurrence_preserves_modes_and_representable_tails() {
        assert_eq!(binomial_mode(2, 1.0 / 3.0), 0);
        assert_eq!(binomial_mode(9, 0.3), 2);
        assert_eq!(binomial_mode(3, 0.5), 2);
        for (n, p, cell, bits) in [
            (1, f64::from_bits(1), 1, 1),
            (2, f64::from_bits(1), 1, 2),
            (1_000, 2.0_f64.powi(-276), 4, 39),
        ] {
            assert_eq!(binomial_pmf_all(n, p).unwrap()[cell].to_bits(), bits);
        }
    }

    #[test]
    fn binomial_input_and_resource_errors_preserve_scratch() {
        let mut scratch = vec![0.25, 0.75];
        for (n, p) in [
            (0, -0.5),
            (1, 1.01),
            (4, f64::NEG_INFINITY),
            (4, f64::INFINITY),
            (1_000_001, 0.0),
            (usize::MAX, 1.0),
        ] {
            assert!(binomial_pmf_all_into(&mut scratch, n, p).is_err());
            assert_eq!(scratch, [0.25, 0.75]);
        }
        binomial_pmf_all_into(&mut scratch, 1_000_000, 0.5).unwrap();
        assert_eq!(scratch.len(), 1_000_001);
        assert!(scratch[500_000] > 0.0);
        assert!(
            (crate::math::summation::neumaier_sum(scratch).unwrap() - 1.0).abs()
                <= 8.0 * f64::EPSILON
        );
    }
}
