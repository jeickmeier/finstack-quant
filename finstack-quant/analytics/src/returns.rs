//! Return computation utilities: simple returns, excess returns, price conversion,
//! compounded returns. Delegates to `math::stats::log_returns` for log variants
//! and `math::summation` for numerically stable accumulation.
//!
//! Crate-internal: callers use these through [`crate::Performance`].

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

/// Geometric decompounding of an annualized risk-free rate to one period.
///
/// ```text
/// rf_period = (1 + rf_annual)^{1/N} − 1
/// ```
///
/// This is the crate-wide compounding rule for Sharpe-family excess returns
/// and for per-period excess-return series.
///
/// # Arguments
///
/// * `risk_free_rate` - Annualized risk-free rate in decimal form (e.g. `0.02`
///   for 2%).
/// * `ann_factor` - Periods per year `N` used to decompound (e.g. `252` daily,
///   `12` monthly).
///
/// # Returns
///
/// The equivalent one-period simple rate. Returns [`f64::NAN`] when either
/// input is non-finite or `ann_factor` is not strictly positive.
#[must_use]
pub(crate) fn periodic_risk_free_rate(risk_free_rate: f64, ann_factor: f64) -> f64 {
    if !risk_free_rate.is_finite() || !ann_factor.is_finite() || ann_factor <= 0.0 {
        return f64::NAN;
    }
    if (ann_factor - 1.0).abs() <= f64::EPSILON {
        return risk_free_rate;
    }
    (1.0 + risk_free_rate).powf(1.0 / ann_factor) - 1.0
}

/// Linearly annualized excess return after geometric rf decompounding.
///
/// `ann_return` is the arithmetic mean scaled by `N` (`μ × N`). The
/// annualized risk-free rate is first converted to a period rate, then
/// subtracted from the period mean and rescaled:
///
/// ```text
/// excess_ann = (μ − rf_period) × N = ann_return − rf_period × N
/// ```
///
/// # Arguments
///
/// * `ann_return` - Linearly annualized arithmetic mean return (`μ × N`).
/// * `risk_free_rate` - Annualized risk-free rate in decimal form.
/// * `ann_factor` - Periods per year `N` used to decompound `risk_free_rate`.
///
/// # Returns
///
/// Annualized excess return. Propagates [`f64::NAN`] when decompounding
/// is undefined.
#[must_use]
pub(crate) fn annualized_excess_return(
    ann_return: f64,
    risk_free_rate: f64,
    ann_factor: f64,
) -> f64 {
    ann_return - periodic_risk_free_rate(risk_free_rate, ann_factor) * ann_factor
}

/// Excess returns = portfolio returns minus risk-free returns.
///
/// When `nperiods` is provided, the risk-free rate is de-compounded to the
/// observation frequency before subtraction:
///
/// ```text
/// rf_adj = (1 + rf)^(1/nperiods) - 1
/// ```
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
            Some(np) => periodic_risk_free_rate(rf[i], np),
            None => rf[i],
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
pub(crate) const MIN_GROWTH_FACTOR: f64 = 1e-18;

/// Log-space wealth accumulator shared by compounding and drawdown.
///
/// Each step multiplies wealth by `max(1 + r, MIN_GROWTH_FACTOR)` via a
/// Neumaier sum of log growth factors. Non-finite returns mark the path
/// invalid from that point forward.
pub(crate) struct WealthEngine {
    acc: NeumaierAccumulator,
    invalid: bool,
}

impl WealthEngine {
    pub(crate) fn new() -> Self {
        Self {
            acc: NeumaierAccumulator::new(),
            invalid: false,
        }
    }

    /// Apply one simple return and return the reconstructed wealth level.
    ///
    /// Starting wealth is `1`. The returned value is `exp(Σ ln g)` or
    /// [`f64::NAN`] once a non-finite return has been seen.
    pub(crate) fn step(&mut self, r: f64) -> f64 {
        if self.invalid || !r.is_finite() {
            self.invalid = true;
            return f64::NAN;
        }
        let g = (1.0 + r).max(MIN_GROWTH_FACTOR);
        self.acc.add(g.ln());
        self.acc.total().exp()
    }
}

/// Cumulative compounded returns: `(1+r).cumprod() - 1`.
///
/// At each step `i` the cumulative return is:
///
/// ```text
/// comp_sum[i] = Π_{j=0}^{i} (1 + r[j]) - 1
/// ```
///
/// Uses the shared [`WealthEngine`] (Neumaier log-space, `MIN_GROWTH_FACTOR`
/// clamp). Returns ≤ −1.0 produce a near-total-loss (≈ −100 %) rather than
/// NaN. Non-finite returns mark the path invalid from that point forward.
///
/// # Arguments
///
/// * `returns` - Slice of simple period returns.
///
/// # Returns
///
/// A `Vec<f64>` of the same length as `returns`. Returns an empty vector
/// if `returns` is empty.
pub(crate) fn comp_sum(returns: &[f64]) -> Vec<f64> {
    let mut engine = WealthEngine::new();
    let mut out = Vec::with_capacity(returns.len());
    for &r in returns {
        out.push(engine.step(r) - 1.0);
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
#[must_use]
pub(crate) fn comp_total(returns: &[f64]) -> f64 {
    if returns.is_empty() {
        return 0.0;
    }
    let mut engine = WealthEngine::new();
    let mut last = 1.0;
    for &r in returns {
        last = engine.step(r);
    }
    last - 1.0
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
    fn periodic_risk_free_rate_geometric_decompound() {
        let rf_period = periodic_risk_free_rate(0.02, 252.0);
        assert!((rf_period - (1.02_f64.powf(1.0 / 252.0) - 1.0)).abs() < 1e-15);
        assert!(periodic_risk_free_rate(0.02, 0.0).is_nan());
        assert!(periodic_risk_free_rate(f64::NAN, 252.0).is_nan());
    }

    #[test]
    fn annualized_excess_return_differs_from_linear_subtraction() {
        let mu = 0.0004_f64;
        let ann = 252.0;
        let geometric = annualized_excess_return(mu * ann, 0.02, ann);
        let linear = mu * ann - 0.02;
        let rf_period = 1.02_f64.powf(1.0 / ann) - 1.0;
        assert!((geometric - (mu - rf_period) * ann).abs() < 1e-12);
        assert!((geometric - linear).abs() > 1e-6);
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
