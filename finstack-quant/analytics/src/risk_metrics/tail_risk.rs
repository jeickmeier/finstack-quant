//! Tail-risk and distribution-shape metrics: VaR, ES, skewness, kurtosis.
//!
//! Crate-internal: callers use these through [`crate::Performance`].
//!
//! All functions operate on `&[f64]` return slices and return scalar `f64`.
//!
//! Conventions:
//! - returns are simple decimal returns (`-0.05` for a 5% loss)
//! - VaR / ES / parametric VaR / Cornish-Fisher VaR are reported in **return
//!   space**: a 5% loss reads as `-0.05`. Output is non-positive for losses
//! - confidence levels are in `(0, 1)`, e.g. `0.95` for 95% VaR
//! - sample skewness uses Fisher's `G_1`; sample excess kurtosis uses `G_2`
//!   (matching Excel `SKEW()` / `KURT()`)
//! - empty or invalid VaR / ES / tail-ratio / parametric / Cornish-Fisher
//!   inputs return [`f64::NAN`] (matching Sharpe when `n < 2`)

use crate::math::stats::mean_var;
use finstack_quant_core::math::stats::quantile;

fn has_strict_confidence(confidence: f64) -> bool {
    confidence.is_finite() && confidence > 0.0 && confidence < 1.0
}

fn valid_horizon(ann_factor: Option<f64>) -> bool {
    ann_factor.is_none_or(|af| af.is_finite() && af > 0.0)
}

fn finite_returns_copy(returns: &[f64]) -> Option<Vec<f64>> {
    let mut data = Vec::with_capacity(returns.len());
    for &value in returns {
        if !value.is_finite() {
            tracing::debug!(
                n_returns = returns.len(),
                reason = "non_finite_return",
                "tail-risk metric returning NaN"
            );
            return None;
        }
        data.push(value);
    }
    Some(data)
}

fn historical_sample(
    returns: &[f64],
    confidence: f64,
    invalid_confidence_message: Option<&str>,
) -> Option<Vec<f64>> {
    if returns.is_empty() {
        return None;
    }
    if !has_strict_confidence(confidence) {
        if let Some(message) = invalid_confidence_message {
            tracing::debug!(confidence, reason = "invalid_confidence", "{message}");
        }
        return None;
    }
    finite_returns_copy(returns)
}

fn historical_var_data(
    returns: &[f64],
    confidence: f64,
    invalid_confidence_message: Option<&str>,
) -> Option<(Vec<f64>, f64)> {
    let mut data = historical_sample(returns, confidence, invalid_confidence_message)?;
    let var_threshold = quantile(&mut data, 1.0 - confidence);
    Some((data, var_threshold))
}

fn expected_shortfall_from_data(data: &mut [f64], confidence: f64) -> f64 {
    let mass = (1.0 - confidence) * data.len() as f64;
    let whole = (mass.floor() as usize).min(data.len());
    let boundary = whole.min(data.len() - 1);
    data.select_nth_unstable_by(boundary, f64::total_cmp);
    if whole == 0 {
        return data[0];
    }
    let mut mean = finstack_quant_core::math::summation::NeumaierAccumulator::new();
    for &value in &data[..whole] {
        mean.add(value / mass);
    }
    if whole < data.len() {
        mean.add(data[whole] * ((mass - whole as f64) / mass));
    }
    mean.total()
}

/// Historical Value-at-Risk at the given confidence level.
///
/// Computes the `(1 - confidence)` quantile of the empirical return
/// distribution. For example, at `confidence = 0.95`, the 5th percentile
/// is returned — losses exceeding this threshold occur with 5% probability
/// under the historical distribution.
///
/// Returns a **negative** number representing the loss threshold.
///
/// Historical VaR is reported in the native period of the input return
/// series; for horizon-scaled estimates use [`parametric_var`] or
/// [`cornish_fisher_var`] since sqrt-T scaling is invalid for
/// non-parametric quantiles.
///
/// # Quantile interpolation
///
/// Uses [`finstack_quant_core::math::stats::quantile`]; see that function for
/// the exact interpolation method and cross-tool comparison notes.
///
/// # Arguments
///
/// * `returns` - Slice of period simple returns.
/// * `confidence` - Confidence level in `(0, 1)`, e.g. `0.95` for 95% VaR.
///
/// # Returns
///
/// The VaR as a non-positive scalar. Returns [`f64::NAN`] for an empty
/// slice or an invalid confidence level.
#[must_use]
pub(crate) fn value_at_risk(returns: &[f64], confidence: f64) -> f64 {
    let Some((_, var_threshold)) =
        historical_var_data(returns, confidence, Some("value_at_risk returning NaN"))
    else {
        return f64::NAN;
    };
    var_threshold
}
/// Expected Shortfall (CVaR / ES) at the given confidence level.
///
/// The mean of exactly the worst `(1 - confidence)` empirical probability
/// mass, providing a coherent measure of tail risk:
///
/// ```text
/// ES_c = integral_0^{1-c} empirical_quantile(u) du / (1-c)
/// ```
///
/// ES is always at least as bad (negative) as VaR at the same confidence
/// level. Negating this return-space value gives a sub-additive coherent
/// loss measure.
/// Reported in the native period of the input return series; sqrt-T scaling
/// is invalid for non-parametric tail means.
///
/// # Tail-set convention
///
/// Each observation has mass `1/n`. Include the worst `floor((1-c)*n)`
/// observations and the required fraction of the next observation. Ties do
/// not expand the tail. This is the discrete CVaR definition of Rockafellar
/// & Uryasev (2002); historical VaR retains its documented interpolated
/// quantile convention.
///
/// # Arguments
///
/// * `returns`    - Slice of period simple returns.
/// * `confidence` - Confidence level in `(0, 1)`, e.g. `0.95`.
///
/// # Returns
///
/// The signed mean tail return (negative for losses). Returns [`f64::NAN`]
/// for an empty slice or an invalid confidence level.
///
/// # References
///
/// - Artzner et al. (1999): `docs/REFERENCES.md#artzner1999CoherentRisk`
#[must_use]
pub(crate) fn expected_shortfall(returns: &[f64], confidence: f64) -> f64 {
    let Some(mut data) = historical_sample(
        returns,
        confidence,
        Some("expected_shortfall returning NaN"),
    ) else {
        return f64::NAN;
    };
    expected_shortfall_from_data(&mut data, confidence)
}

/// Compute historical VaR and Expected Shortfall together.
///
/// Shares one validated finite-return copy and one quantile calculation
/// between the two metrics, which is the common case for summary outputs
/// that report both.
///
/// Returns `(value_at_risk, expected_shortfall)`. Sentinel rules match the
/// standalone [`value_at_risk`] / [`expected_shortfall`] functions:
/// `(NaN, NaN)` for an empty slice, non-finite inputs, or out-of-range
/// `confidence`.
#[must_use]
pub(crate) fn value_at_risk_and_es(returns: &[f64], confidence: f64) -> (f64, f64) {
    let Some((mut data, var_threshold)) = historical_var_data(returns, confidence, None) else {
        return (f64::NAN, f64::NAN);
    };
    let es = expected_shortfall_from_data(&mut data, confidence);
    (var_threshold, es)
}
/// Tail ratio = |upper tail| / |lower tail|.
///
/// Computes the ratio of the absolute upper quantile to the absolute lower
/// quantile:
///
/// ```text
/// tail_ratio = |quantile(confidence)| / |quantile(1 - confidence)|
/// ```
///
/// A value greater than 1.0 indicates that the right tail (gains) is larger
/// than the left tail (losses) at the symmetric confidence level.
///
/// # Arguments
///
/// * `returns` - Slice of period simple returns.
/// * `confidence` - Quantile level for the upper tail (e.g., `0.95`).
///   The lower tail uses `1 - confidence`.
///
/// # Returns
///
/// The tail ratio (non-negative). Returns [`f64::NAN`] if `returns` is
/// empty, [`f64::INFINITY`] when the lower tail quantile is zero but the
/// upper tail is positive, and [`f64::NAN`] when both tails are zero.
#[must_use]
pub(crate) fn tail_ratio(returns: &[f64], confidence: f64) -> f64 {
    if returns.is_empty() {
        return f64::NAN;
    }
    if !has_strict_confidence(confidence) {
        return f64::NAN;
    }
    let Some(mut data) = finite_returns_copy(returns) else {
        return f64::NAN;
    };
    let upper = quantile(&mut data, confidence).abs();
    let lower = quantile(&mut data, 1.0 - confidence).abs();
    if lower == 0.0 {
        return if upper > 0.0 { f64::INFINITY } else { f64::NAN };
    }
    upper / lower
}
/// Fisher-corrected sample skewness (G₁) of a return distribution.
///
/// Measures asymmetry: positive skewness indicates a longer right tail
/// (large gains more likely than large losses), negative skewness the
/// opposite. Uses the bias-corrected sample formula matching Bloomberg
/// and Excel `SKEW()`:
///
/// ```text
/// G₁ = [n / ((n-1)(n-2))] × Σ((r_i − x̄) / s)³
/// ```
///
/// where `s` is the sample standard deviation (n-1 denominator).
///
/// # Arguments
///
/// * `returns` - Slice of period simple returns.
///
/// # Returns
///
/// The bias-corrected sample skewness. Returns `0.0` for fewer than 3
/// observations or zero-variance series.
///
/// # References
///
/// - Joanes & Gill (1998): see docs/REFERENCES.md#joanesGill1998
#[must_use]
pub(crate) fn skewness(returns: &[f64]) -> f64 {
    let (_, _, skew, _) = moments4(returns);
    skew
}

/// Fisher-corrected sample excess kurtosis (G₂) of a return distribution.
///
/// Measures tail heaviness relative to a normal distribution. A positive
/// value (leptokurtic) indicates fatter tails; negative (platykurtic)
/// indicates thinner tails. Normal returns have excess kurtosis = 0.
///
/// Uses the bias-corrected sample formula matching Bloomberg and
/// Excel `KURT()`:
///
/// ```text
/// G₂ = [n(n+1) / ((n-1)(n-2)(n-3))] × Σ((r_i − x̄) / s)⁴
/// − 3(n-1)² / ((n-2)(n-3))
/// ```
///
/// where `s` is the sample standard deviation (n-1 denominator).
///
/// # Arguments
///
/// * `returns` - Slice of period simple returns.
///
/// # Returns
///
/// Bias-corrected excess kurtosis. Returns `0.0` for fewer than 4
/// observations or zero-variance series.
///
/// # References
///
/// - Joanes & Gill (1998): see docs/REFERENCES.md#joanesGill1998
#[must_use]
pub(crate) fn kurtosis(returns: &[f64]) -> f64 {
    let (_, _, _, kurt) = moments4(returns);
    kurt
}

/// Bias-corrected sample skewness and excess kurtosis in one pass.
///
/// Avoids running `moments4` twice when both metrics are needed (e.g. in
/// summary outputs).
#[must_use]
pub(crate) fn skew_kurt(returns: &[f64]) -> (f64, f64) {
    let (_, _, skew, kurt) = moments4(returns);
    (skew, kurt)
}

/// Parametric (Gaussian) Value-at-Risk.
///
/// Assumes normally distributed returns:
///
/// ```text
/// VaR = μ + z_(1−α) × σ
/// ```
///
/// where `z_(1−α)` is the standard normal quantile at `(1 - confidence)`.
///
/// # Arguments
///
/// * `returns`    - Slice of period simple returns.
/// * `confidence` - Confidence level in `(0, 1)`, e.g. `0.95`.
/// * `ann_factor` - If `Some(f)`, scales the mean term by `f` and the
///   volatility term by `sqrt(f)`.
///
/// # Returns
///
/// The parametric VaR (typically negative). Returns [`f64::NAN`] for an
/// empty slice or an invalid confidence / horizon.
///
/// This is equal-weight Gaussian VaR (sample mean and sample volatility),
/// not an EWMA / RiskMetrics estimator.
#[must_use]
pub(crate) fn parametric_var(returns: &[f64], confidence: f64, ann_factor: Option<f64>) -> f64 {
    if returns.is_empty() {
        return f64::NAN;
    }
    if !has_strict_confidence(confidence) {
        return f64::NAN;
    }
    if !valid_horizon(ann_factor) {
        return f64::NAN;
    }
    let (m, var) = mean_var(returns);
    let vol = var.sqrt();
    let z = crate::math::special_functions::standard_normal_inv_cdf(1.0 - confidence);
    match ann_factor {
        Some(af) => m * af + z * vol * af.sqrt(),
        None => m + z * vol,
    }
}
/// Cornish-Fisher Value-at-Risk: adjusts Gaussian VaR for skewness and kurtosis.
///
/// Uses the Cornish-Fisher expansion to produce a more accurate VaR
/// estimate when the return distribution departs from normality:
///
/// ```text
/// z_cf = z + (z² − 1)S/6 + (z³ − 3z)K/24 − (2z³ − 5z)S²/36
/// VaR_CF = μ + z_cf × σ
/// ```
///
/// where `S` is skewness and `K` is excess kurtosis.
///
/// # Arguments
///
/// * `returns`    - Slice of period simple returns.
/// * `confidence` - Confidence level in `(0, 1)`, e.g. `0.95`.
/// * `ann_factor` - If `Some(f)`, scales the mean term by `f`, the
///   volatility term by `sqrt(f)`, and the shape moments per i.i.d.
///   aggregation: skewness by `1/sqrt(f)` and excess kurtosis by `1/f`
///   (the central-limit decay rates), so the horizon-`f` quantile uses
///   the horizon-`f` distribution rather than per-period tail shape.
///
/// # Returns
///
/// The Cornish-Fisher adjusted VaR (typically negative). Returns
/// [`f64::NAN`] for an empty slice. Falls back to parametric VaR if
/// skewness and kurtosis are both zero.
///
/// # Caution
///
/// The Cornish-Fisher expansion is a polynomial approximation valid for
/// moderate departures from normality. For extreme skewness or kurtosis
/// (e.g., |S| > 2 or |K| > 6), the adjusted quantile `z_cf` can become
/// non-monotonic in confidence level, producing paradoxical results.
/// Always cross-check against historical VaR for heavily-tailed series.
///
/// # Monotonicity guard (Maillard, 2012)
///
/// The implementation evaluates `dz_cf/dz` at the requested `z`. If the
/// derivative is non-positive at that point, the local Cornish-Fisher
/// quantile function is non-monotonic and the expansion is unreliable, so
/// the function falls back to the parametric (Gaussian) VaR and logs a
/// `tracing::warn!` diagnostic with the offending `(skewness, kurtosis,
/// confidence)` triple.
///
/// # References
///
/// - Cornish & Fisher (1937): see docs/REFERENCES.md#cornishFisher1937
#[must_use]
pub(crate) fn cornish_fisher_var(returns: &[f64], confidence: f64, ann_factor: Option<f64>) -> f64 {
    if returns.is_empty() {
        return f64::NAN;
    }
    if !has_strict_confidence(confidence) {
        return f64::NAN;
    }
    if !valid_horizon(ann_factor) {
        return f64::NAN;
    }
    let (m, vol, s, k) = moments4(returns);
    // Scale the shape moments to the aggregation horizon before building the
    // expansion: under i.i.d. aggregation of h periods, skewness decays as
    // S/√h and excess kurtosis as K/h (CLT). Evaluating z_cf with the raw
    // per-period moments while scaling mean/vol to the horizon would
    // overstate the non-normality adjustment by up to √h / h.
    let (s, k) = match ann_factor {
        Some(af) => (s / af.sqrt(), k / af),
        None => (s, k),
    };
    let z = crate::math::special_functions::standard_normal_inv_cdf(1.0 - confidence);
    let z2 = z * z;
    let z3 = z2 * z;
    // Maillard (2012) monotonicity guard: d z_cf / d z at the target z must
    // be positive for the expansion to behave like a valid quantile function
    // locally. If it isn't, the polynomial inversion is unreliable; fall back
    // to parametric VaR and emit a structured warning.
    let dz_cf =
        1.0 + (2.0 * z) * s / 6.0 + (3.0 * z2 - 3.0) * k / 24.0 - (6.0 * z2 - 5.0) * s * s / 36.0;
    if dz_cf <= 0.0 {
        tracing::warn!(
            skewness = s,
            excess_kurtosis = k,
            confidence,
            dz_cf,
            "Cornish-Fisher expansion non-monotonic at requested confidence; \
             falling back to parametric (Gaussian) VaR"
        );
        return match ann_factor {
            Some(af) => m * af + z * vol * af.sqrt(),
            None => m + z * vol,
        };
    }
    let z_cf =
        z + (z2 - 1.0) * s / 6.0 + (z3 - 3.0 * z) * k / 24.0 - (2.0 * z3 - 5.0 * z) * s * s / 36.0;
    match ann_factor {
        Some(af) => m * af + z_cf * vol * af.sqrt(),
        None => m + z_cf * vol,
    }
}
/// Compute mean, standard deviation, skewness (G₁), and excess kurtosis (G₂) in a single pass.
///
/// Uses a one-pass algorithm accumulating central moments (Pebay 2008, Terriberry 2007).
/// Returns `(mean, std_dev, skewness, excess_kurtosis)` matching the bias-corrected
/// formulas used by `skewness()` and `kurtosis()`.
pub(super) fn moments4(returns: &[f64]) -> (f64, f64, f64, f64) {
    let n = returns.len();
    if n == 0 {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let nf = n as f64;

    let mut m1 = 0.0_f64;
    let mut m2 = 0.0_f64;
    let mut m3 = 0.0_f64;
    let mut m4 = 0.0_f64;
    let mut k = 0.0_f64;

    for &r in returns {
        k += 1.0;
        let d = r - m1;
        let d_over_k = d / k;
        let d_over_k2 = d_over_k * d_over_k;
        let term1 = d * d_over_k * (k - 1.0);
        m4 += term1 * d_over_k2 * (k * k - 3.0 * k + 3.0) + 6.0 * d_over_k2 * m2
            - 4.0 * d_over_k * m3;
        m3 += term1 * d_over_k * (k - 2.0) - 3.0 * d_over_k * m2;
        m2 += term1;
        m1 += d_over_k;
    }

    let sample_var = if n < 2 { 0.0 } else { m2 / (nf - 1.0) };
    let vol = sample_var.sqrt();

    let skew = if n < 3 || sample_var == 0.0 {
        0.0
    } else {
        let s3 = vol * vol * vol;
        let adj = nf / ((nf - 1.0) * (nf - 2.0));
        adj * (m3 / s3)
    };

    let kurt = if n < 4 || sample_var == 0.0 {
        0.0
    } else {
        let s4 = sample_var * sample_var;
        let sum_z4 = m4 / s4;
        let a = (nf * (nf + 1.0)) / ((nf - 1.0) * (nf - 2.0) * (nf - 3.0));
        let b = (3.0 * (nf - 1.0) * (nf - 1.0)) / ((nf - 2.0) * (nf - 3.0));
        a * sum_z4 - b
    };

    (m1, vol, skew, kurt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::stats::{mean, mean_var, variance};

    fn assert_f64_eq_nan_aware(actual: f64, expected: f64) {
        if expected.is_nan() {
            assert!(actual.is_nan(), "expected NaN, got {actual}");
        } else {
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn var_basic() {
        let data: Vec<f64> = (-100..=100).map(|i| i as f64 / 100.0).collect();
        let var = value_at_risk(&data, 0.95);
        assert!(var < -0.8);
    }

    #[test]
    fn tail_ratio_returns_infinity_when_lower_tail_is_zero_but_upper_is_positive() {
        let returns = [0.0, 0.0, 0.02, 0.03];
        assert_eq!(tail_ratio(&returns, 0.75), f64::INFINITY);
    }

    #[test]
    fn tail_ratio_returns_nan_when_both_tails_are_zero() {
        let returns = [0.0, 0.0, 0.0, 0.0];
        assert!(tail_ratio(&returns, 0.75).is_nan());
    }

    #[test]
    fn expected_shortfall_weights_only_the_requested_tail_mass() {
        let data = [-3.0, -1.0, -1.0, 10.0];
        let es = expected_shortfall(&data, 0.5);
        let expected = (-3.0 - 1.0) / 2.0;
        assert!((es - expected).abs() < 1e-12);
    }

    #[test]
    fn combined_var_and_es_exactly_match_standalone_metrics() {
        let finite = [-0.08, 0.03, -0.02, 0.01, -0.05, 0.04];
        let tied_threshold = [-3.0, -1.0, -1.0, 10.0];
        let non_finite_nan = [-0.03, f64::NAN, 0.01];
        let non_finite_infinity = [-0.03, f64::INFINITY, 0.01];
        let cases: &[(&[f64], f64)] = &[
            (&finite, 0.95),
            (&tied_threshold, 0.5),
            (&finite, 1.0),
            (&non_finite_nan, 0.95),
            (&non_finite_infinity, 0.95),
            (&[], 0.95),
        ];

        for &(returns, confidence) in cases {
            let (combined_var, combined_es) = value_at_risk_and_es(returns, confidence);
            assert_f64_eq_nan_aware(combined_var, value_at_risk(returns, confidence));
            assert_f64_eq_nan_aware(combined_es, expected_shortfall(returns, confidence));
        }
    }

    #[test]
    fn tail_risk_metrics_reject_non_finite_inputs() {
        let data = [-0.03, f64::NAN, 0.01, 0.02];

        assert!(value_at_risk(&data, 0.95).is_nan());
        assert!(expected_shortfall(&data, 0.95).is_nan());
        assert!(tail_ratio(&data, 0.95).is_nan());
    }

    #[test]
    fn skewness_symmetric_zero() {
        let r: Vec<f64> = (-50..=50).map(|i| i as f64 * 0.01).collect();
        assert!(skewness(&r).abs() < 1e-10);
    }

    #[test]
    fn skewness_hand_calc() {
        let r = [0.0, 0.0, 0.0, 0.0, 5.0];
        assert!((skewness(&r) - 5.0_f64.sqrt()).abs() < 1e-12);
    }

    #[test]
    fn skewness_negative() {
        let r = [0.0, 0.0, 0.0, 0.0, -5.0];
        assert!((skewness(&r) - (-5.0_f64.sqrt())).abs() < 1e-12);
    }

    #[test]
    fn skewness_edge_cases() {
        assert_eq!(skewness(&[]), 0.0);
        assert_eq!(skewness(&[1.0, 2.0]), 0.0);
        assert_eq!(skewness(&[3.0, 3.0, 3.0]), 0.0);
    }

    #[test]
    fn kurtosis_hand_calc() {
        let r = [0.0, 0.0, 0.0, 0.0, 5.0];
        assert!((kurtosis(&r) - 5.0).abs() < 1e-12);
    }

    #[test]
    fn kurtosis_uniform_analytical() {
        let r: Vec<f64> = (0..10000).map(|i| i as f64 / 10000.0).collect();
        let k = kurtosis(&r);
        assert!((k - (-1.2)).abs() < 0.005);
    }

    #[test]
    fn kurtosis_edge_cases() {
        assert_eq!(kurtosis(&[]), 0.0);
        assert_eq!(kurtosis(&[1.0, 2.0, 3.0]), 0.0);
        assert_eq!(kurtosis(&[1.0, 1.0, 1.0, 1.0]), 0.0);
    }

    #[test]
    fn parametric_var_formula_verification() {
        let r = [-0.02, -0.01, 0.0, 0.01, 0.02];
        let pvar = parametric_var(&r, 0.95, None);
        let m = mean(&r);
        let vol = variance(&r).sqrt();
        let z = crate::math::special_functions::standard_normal_inv_cdf(0.05);
        let expected = m + z * vol;
        assert!((pvar - expected).abs() < 1e-14);
    }

    #[test]
    fn parametric_var_annualized() {
        let r = [-0.02, -0.01, 0.0, 0.01, 0.02];
        let pvar_raw = parametric_var(&r, 0.95, None);
        let pvar_ann = parametric_var(&r, 0.95, Some(252.0));
        assert!((pvar_ann - pvar_raw * 252.0_f64.sqrt()).abs() < 1e-12);
    }

    #[test]
    fn parametric_var_empty() {
        assert!(parametric_var(&[], 0.95, None).is_nan());
    }

    #[test]
    fn parametric_var_horizon_ten_matches_formula() {
        let returns = [0.01, -0.02, 0.03, -0.01, 0.02, -0.005];
        let actual = parametric_var(&returns, 0.95, Some(10.0));
        let (m, var) = mean_var(&returns);
        let z = crate::math::special_functions::standard_normal_inv_cdf(0.05);
        let expected = m * 10.0 + z * var.sqrt() * 10.0_f64.sqrt();
        assert!((actual - expected).abs() < 1e-14);
        let period = parametric_var(&returns, 0.95, None);
        assert!((actual - period).abs() > 1e-6);
    }

    #[test]
    fn cornish_fisher_var_converges_to_parametric_for_normal_like() {
        let r: Vec<f64> = (-500..=500).map(|i| i as f64 / 5000.0).collect();
        let pvar = parametric_var(&r, 0.95, None);
        let cfvar = cornish_fisher_var(&r, 0.95, None);
        assert!((cfvar - pvar).abs() < 0.02);
    }

    #[test]
    fn cornish_fisher_var_differs_from_parametric_for_skewed() {
        // Mildly skewed normal-ish sample, well within the Maillard
        // monotonicity region so the expansion is applied (not falling back
        // to parametric).
        let mut r: Vec<f64> = (0..200).map(|i| (i as f64 - 100.0) * 0.001).collect();
        // Inject a handful of mildly larger losses to create modest skew.
        for v in r.iter_mut().take(5) {
            *v -= 0.005;
        }
        let pvar = parametric_var(&r, 0.95, None);
        let cfvar = cornish_fisher_var(&r, 0.95, None);
        assert!(pvar < 0.0);
        assert!(cfvar < 0.0);
        assert!((cfvar - pvar).abs() > 1e-6);
    }

    #[test]
    fn cornish_fisher_var_falls_back_to_parametric_when_non_monotonic() {
        // Heavily skewed bimodal sample: 80 small losses and 20 large gains.
        // dz_cf/dz at z ≈ −1.645 is negative for this (S, K), so the
        // Maillard guard should fire and the function should fall back to
        // parametric VaR.
        let mut r = vec![-0.01_f64; 80];
        r.extend(vec![0.05; 20]);
        let pvar = parametric_var(&r, 0.95, None);
        let cfvar = cornish_fisher_var(&r, 0.95, None);
        assert!((cfvar - pvar).abs() < 1e-12);
    }

    #[test]
    fn cornish_fisher_var_empty() {
        assert!(cornish_fisher_var(&[], 0.95, None).is_nan());
    }

    #[test]
    fn expected_shortfall_is_at_most_var_across_distributions() {
        let datasets: Vec<Vec<f64>> = vec![
            (-100..=100).map(|i| i as f64 / 100.0).collect(),
            vec![0.05, -0.10, 0.03, -0.15, 0.07, -0.02, 0.01, -0.08],
            vec![-0.01; 50],
        ];
        for data in &datasets {
            let var = value_at_risk(data, 0.95);
            let es = expected_shortfall(data, 0.95);
            assert!(es <= var, "ES must be ≤ VaR: es={es}, var={var}");
        }
    }

    #[test]
    fn cornish_fisher_formula_verification() {
        let r = [
            0.05, -0.10, 0.03, -0.15, 0.07, -0.02, 0.01, -0.08, -0.05, 0.02,
        ];
        let cf = cornish_fisher_var(&r, 0.95, None);
        let m = mean(&r);
        let vol = variance(&r).sqrt();
        let s = skewness(&r);
        let k = kurtosis(&r);
        let z = crate::math::special_functions::standard_normal_inv_cdf(0.05);
        let z2 = z * z;
        let z3 = z2 * z;
        let z_cf = z + (z2 - 1.0) * s / 6.0 + (z3 - 3.0 * z) * k / 24.0
            - (2.0 * z3 - 5.0 * z) * s * s / 36.0;
        let expected = m + z_cf * vol;
        assert!((cf - expected).abs() < 1e-14);
    }

    #[test]
    fn cornish_fisher_var_scales_mean_vol_and_moments_by_horizon() {
        let returns = [
            0.05, -0.10, 0.03, -0.15, 0.07, -0.02, 0.01, -0.08, -0.05, 0.02,
        ];
        let ann_factor = 12.0_f64;
        let m = mean(&returns);
        let vol = variance(&returns).sqrt();
        // Under i.i.d. aggregation over h periods, S_h = S/√h and K_h = K/h.
        let s = skewness(&returns) / ann_factor.sqrt();
        let k = kurtosis(&returns) / ann_factor;
        let z = crate::math::special_functions::standard_normal_inv_cdf(0.05);
        let z2 = z * z;
        let z3 = z2 * z;
        let z_cf = z + (z2 - 1.0) * s / 6.0 + (z3 - 3.0 * z) * k / 24.0
            - (2.0 * z3 - 5.0 * z) * s * s / 36.0;
        let expected = m * ann_factor + z_cf * vol * ann_factor.sqrt();
        let actual = cornish_fisher_var(&returns, 0.95, Some(ann_factor));
        assert!((actual - expected).abs() < 1e-14, "{actual} vs {expected}");
    }

    #[test]
    fn cornish_fisher_var_converges_to_parametric_at_long_horizons() {
        // Central-limit behavior: as the aggregation horizon grows the
        // scaled moments vanish, so CF-VaR must approach Gaussian VaR
        // relative to the (growing) scale of the annualized quantities.
        let returns = [
            0.05, -0.10, 0.03, -0.15, 0.07, -0.02, 0.01, -0.08, -0.05, 0.02,
        ];
        let cf_1 = cornish_fisher_var(&returns, 0.99, Some(1.0));
        let p_1 = parametric_var(&returns, 0.99, Some(1.0));
        let cf_252 = cornish_fisher_var(&returns, 0.99, Some(252.0));
        let p_252 = parametric_var(&returns, 0.99, Some(252.0));
        let rel_gap_1 = (cf_1 - p_1).abs() / p_1.abs();
        let rel_gap_252 = (cf_252 - p_252).abs() / p_252.abs();
        assert!(
            rel_gap_252 < rel_gap_1,
            "relative CF adjustment must shrink with horizon: {rel_gap_252} vs {rel_gap_1}"
        );
    }

    #[test]
    fn empty_input_consistency() {
        let empty: Vec<f64> = vec![];
        assert!(value_at_risk(&empty, 0.95).is_nan());
        assert!(expected_shortfall(&empty, 0.95).is_nan());
        assert!(tail_ratio(&empty, 0.95).is_nan());
        let (var, es) = value_at_risk_and_es(&empty, 0.95);
        assert!(var.is_nan() && es.is_nan());
    }

    #[test]
    fn value_at_risk_requires_strict_confidence_bounds() {
        let returns = [-0.03, -0.01, 0.01, 0.02];
        assert!(value_at_risk(&returns, 0.0).is_nan());
        assert!(value_at_risk(&returns, 1.0).is_nan());
    }

    #[test]
    fn parametric_var_rejects_non_positive_or_non_finite_horizon() {
        let returns = [-0.03, -0.01, 0.01, 0.02];
        assert!(parametric_var(&returns, 0.95, Some(0.0)).is_nan());
        assert!(parametric_var(&returns, 0.95, Some(-12.0)).is_nan());
        assert!(parametric_var(&returns, 0.95, Some(f64::INFINITY)).is_nan());
    }

    #[test]
    fn cornish_fisher_var_rejects_non_positive_or_non_finite_horizon() {
        let returns = [-0.03, -0.01, 0.01, 0.02];
        assert!(cornish_fisher_var(&returns, 0.95, Some(0.0)).is_nan());
        assert!(cornish_fisher_var(&returns, 0.95, Some(-12.0)).is_nan());
        assert!(cornish_fisher_var(&returns, 0.95, Some(f64::INFINITY)).is_nan());
    }
}
