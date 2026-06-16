//! Return computation utilities: simple returns, excess returns, price conversion,
//! compounded returns. Delegates to `math::stats::log_returns` for log variants
//! and `math::summation` for numerically stable accumulation.
//!
//! Crate-internal: callers use these through [`crate::Performance`]; the
//! `///` doc examples target crate developers and are marked `ignore` because
//! the functions are not part of the public API.

use crate::math::summation::NeumaierAccumulator;

/// Pairwise simple (percentage-change) returns from a price series.
///
/// For prices `[p0, p1, p2, ...]` returns `[p1/p0 - 1, p2/p1 - 1, ...]`
/// (length `prices.len() - 1`). Unlike the Python "prepend a zero before
/// first valid" convention, no leading `0.0` is emitted, so the output
/// pairs 1:1 with the return-aligned date grid used by
/// [`crate::Performance::new`].
///
/// Non-positive or non-finite prices produce `NaN` for that element.
///
/// # Arguments
///
/// * `prices` - Slice of asset prices in chronological order.
///
/// # Returns
///
/// A `Vec<f64>` of length `prices.len() - 1`. Returns an empty vector for
/// fewer than two prices.
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_analytics::returns::pairwise_returns;
///
/// let r = pairwise_returns(&[100.0, 110.0, 99.0]);
/// assert!((r[0] - 0.1).abs() < 1e-12);   // +10%
/// assert!((r[1] - (-0.1)).abs() < 1e-12); // −10%
/// ```
pub(crate) fn pairwise_returns(prices: &[f64]) -> Vec<f64> {
    if prices.len() < 2 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(prices.len() - 1);
    push_pairwise_returns(prices, &mut out);
    out
}

#[inline]
fn push_pairwise_returns(prices: &[f64], out: &mut Vec<f64>) {
    for w in prices.windows(2) {
        let p0 = w[0];
        let p1 = w[1];
        if p0 <= 0.0 || !p0.is_finite() || !p1.is_finite() {
            out.push(f64::NAN);
        } else {
            let ratio = p1 / p0;
            if !ratio.is_finite() || ratio <= 0.0 {
                out.push(f64::NAN);
            } else {
                out.push(ratio - 1.0);
            }
        }
    }
}

/// Excess returns = portfolio returns minus risk-free returns.
///
/// When `nperiods` is provided, the risk-free rate is de-compounded to the
/// observation frequency before subtraction:
///
/// ```text
/// rf_adj = (1 + rf)^(1/nperiods) - 1
/// ```ignore
///
/// For example, if `rf` is an annualized rate and observations are monthly,
/// pass `nperiods = 12.0`.
///
/// # Arguments
///
/// * `returns` - Portfolio return series.
/// * `rf` - Risk-free rate series, aligned with `returns`. If longer, the
///   excess length is ignored.
/// * `nperiods` - Optional compounding periods per year. `None` uses `rf`
///   values directly without adjustment. Negative, zero, or non-finite
///   values yield an all-`NaN` output to flag invalid input (negative
///   values would invert the decompounding direction).
///
/// # Returns
///
/// A `Vec<f64>` of length `min(returns.len(), rf.len())` containing
/// `returns[i] - rf_adj[i]` for each observation.
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_analytics::returns::excess_returns;
///
/// // Monthly returns, annualized risk-free rate of 10%.
/// let ret = [0.05, 0.03, -0.02];
/// let rf  = [0.10, 0.10,  0.10];
/// let ex  = excess_returns(&ret, &rf, Some(12.0));
/// // rf_adj ≈ (1.10)^(1/12) − 1 ≈ 0.00797
/// let rf_adj = 1.1_f64.powf(1.0 / 12.0) - 1.0;
/// assert!((ex[0] - (0.05 - rf_adj)).abs() < 1e-10);
/// ```
pub(crate) fn excess_returns(returns: &[f64], rf: &[f64], nperiods: Option<f64>) -> Vec<f64> {
    let n = returns.len().min(rf.len());
    if let Some(np) = nperiods {
        if !np.is_finite() || np <= 0.0 {
            return vec![f64::NAN; n];
        }
    }
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let rf_adj = match nperiods {
            Some(np) if np.abs() > f64::EPSILON && (np - 1.0).abs() > f64::EPSILON => {
                (1.0 + rf[i]).powf(1.0 / np) - 1.0
            }
            _ => rf[i],
        };
        out.push(returns[i] - rf_adj);
    }
    out
}

/// Smallest growth factor allowed before taking the log.
///
/// Returns of exactly −100% (total wipeout) or worse would produce −∞ or NaN
/// in log-space. Clamping to this floor keeps the accumulator valid while
/// still representing an effectively total loss.
const MIN_GROWTH_FACTOR: f64 = 1e-18;

/// Cumulative compounded returns: `(1+r).cumprod() - 1`.
///
/// At each step `i` the cumulative return is:
///
/// ```text
/// comp_sum[i] = Π_{j=0}^{i} (1 + r[j]) - 1
/// ```ignore
///
/// Uses a Neumaier accumulator in log-space for numerical stability on
/// long series. Growth factors are clamped to `MIN_GROWTH_FACTOR` so
/// that returns ≤ −1.0 produce a near-total-loss (≈ −100 %) rather than NaN.
/// Non-finite returns (NaN, ±Inf) mark the path invalid from that point
/// forward, so the current and subsequent outputs become `NaN`.
///
/// # Arguments
///
/// * `returns` - Slice of simple period returns.
///
/// # Returns
///
/// A `Vec<f64>` of the same length as `returns`. Returns an empty vector
/// if `returns` is empty.
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_analytics::returns::comp_sum;
///
/// let r = [0.01, 0.02, -0.005];
/// let cs = comp_sum(&r);
/// // Final value ≈ (1.01 × 1.02 × 0.995) − 1
/// let expected = 1.01 * 1.02 * 0.995 - 1.0;
/// assert!((cs[2] - expected).abs() < 1e-12);
/// ```
pub(crate) fn comp_sum(returns: &[f64]) -> Vec<f64> {
    let mut acc = NeumaierAccumulator::new();
    let mut out = Vec::with_capacity(returns.len());
    let mut invalid = false;
    for &r in returns {
        if invalid || !r.is_finite() {
            invalid = true;
            out.push(f64::NAN);
            continue;
        }
        let g = (1.0 + r).max(MIN_GROWTH_FACTOR);
        acc.add(g.ln());
        out.push(acc.total().exp() - 1.0);
    }
    out
}

/// Total compounded return over the full slice: `Π(1 + r_i) - 1`.
///
/// Equivalent to `comp_sum(returns).last()`, but computed in a single pass
/// without allocating an intermediate vector.
///
/// Uses a Neumaier accumulator in log-space for numerical stability
/// (matching [`comp_sum`]). Growth factors are clamped to
/// `MIN_GROWTH_FACTOR` so that returns ≤ −1.0 produce a near-total-loss
/// rather than NaN. Non-finite returns (NaN, ±Inf) immediately propagate
/// invalidity by returning `NaN`.
///
/// # Arguments
///
/// * `returns` - Slice of simple period returns.
///
/// # Returns
///
/// The total compounded return as a scalar. Returns `0.0` for an empty slice.
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_analytics::returns::comp_total;
///
/// let r = [0.01, 0.02, -0.005];
/// let ct = comp_total(&r);
/// let expected = 1.01 * 1.02 * 0.995 - 1.0;
/// assert!((ct - expected).abs() < 1e-12);
///
/// // Handles total wipeout without producing NaN.
/// let ct_wipeout = comp_total(&[0.05, -1.0, 0.10]);
/// assert!(ct_wipeout.is_finite());
/// assert!(ct_wipeout < -0.99);
/// ```
#[must_use]
pub(crate) fn comp_total(returns: &[f64]) -> f64 {
    if returns.is_empty() {
        return 0.0;
    }
    let mut acc = NeumaierAccumulator::new();
    for &r in returns {
        if !r.is_finite() {
            return f64::NAN;
        }
        acc.add((1.0 + r).max(MIN_GROWTH_FACTOR).ln());
    }
    acc.total().exp() - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairwise_returns_basic() {
        let prices = [100.0, 110.0, 99.0];
        let r = pairwise_returns(&prices);
        assert_eq!(r.len(), 2);
        assert!((r[0] - 0.1).abs() < 1e-12);
        assert!((r[1] - (-0.1)).abs() < 1e-12);
    }

    #[test]
    fn pairwise_returns_empty() {
        assert!(pairwise_returns(&[]).is_empty());
    }

    #[test]
    fn pairwise_returns_single() {
        assert!(pairwise_returns(&[100.0]).is_empty());
    }

    #[test]
    fn excess_returns_defect_fix() {
        let ret = [0.05, 0.03, -0.02];
        let rf = [0.10, 0.10, 0.10];
        let ex = excess_returns(&ret, &rf, Some(12.0));
        // rf_adj = (1.10)^(1/12) - 1 ≈ 0.00797
        let rf_adj = 1.1_f64.powf(1.0 / 12.0) - 1.0;
        assert!((ex[0] - (0.05 - rf_adj)).abs() < 1e-10);
    }

    #[test]
    fn excess_returns_invalid_nperiods_returns_nan_series() {
        let ret = [0.05, 0.03, -0.02];
        let rf = [0.10, 0.10, 0.10];
        let ex = excess_returns(&ret, &rf, Some(-12.0));
        assert_eq!(ex.len(), 3);
        assert!(ex.iter().all(|v| v.is_nan()));
    }

    #[test]
    fn comp_sum_and_total() {
        let r = [0.01, 0.02, -0.005];
        let cs = comp_sum(&r);
        assert_eq!(cs.len(), 3);
        let ct = comp_total(&r);
        assert!((cs[2] - ct).abs() < 1e-12);
        // manual: (1.01 * 1.02 * 0.995) - 1
        let expected = 1.01 * 1.02 * 0.995 - 1.0;
        assert!((ct - expected).abs() < 1e-12);
    }

    #[test]
    fn comp_total_matches_comp_sum_on_long_mixed_sign_series() {
        let r: Vec<f64> = (0..5000)
            .map(|i| (((i % 17) as f64) - 8.0) * 0.0003)
            .collect();
        let cs = comp_sum(&r);
        let ct = comp_total(&r);
        assert!((cs.last().copied().unwrap_or(0.0) - ct).abs() < 1e-12);
    }

    #[test]
    fn comp_total_handles_total_wipeout() {
        let r = [0.05, -1.0, 0.10];
        let ct = comp_total(&r);
        assert!(ct.is_finite(), "comp_total must not produce NaN/Inf");
        assert!(ct < -0.99, "total wipeout should be near −100%");
    }

    #[test]
    fn comp_sum_handles_return_below_minus_one() {
        let r = [0.05, -1.5, 0.10];
        let cs = comp_sum(&r);
        assert!(
            cs.iter().all(|v| v.is_finite()),
            "all values must be finite"
        );
    }

    #[test]
    fn comp_total_propagates_nan_returns() {
        let ct = comp_total(&[0.05, f64::NAN, 0.10]);
        assert!(ct.is_nan(), "NaN inputs should remain invalid");
    }

    #[test]
    fn comp_sum_propagates_nan_returns() {
        let cs = comp_sum(&[0.05, f64::NAN, 0.10]);
        assert!(cs[1].is_nan(), "NaN period should mark the path invalid");
        assert!(
            cs[2].is_nan(),
            "invalid compounding should propagate forward"
        );
    }
}
