//! Historical-simulation VaR / Expected Shortfall quantile-estimator tests.
//!
//! These tests pin the historical-simulation VaR to the R type-7 / numpy-default
//! linear-interpolated empirical quantile and verify that Expected Shortfall is
//! derived from the SAME quantile definition, so the two measures are mutually
//! consistent.
//!
//! Every reference value below is hand-computed from a known P&L sample whose
//! sorted order statistics form a uniform arithmetic progression. For a uniform
//! step the R type-7 quantile function is globally linear,
//!
//!   Q(u) = pnl[0] + (n - 1) * d * u,
//!
//! where `d` is the step. Hence, with `h = (n - 1) * p`:
//!
//!   VaR quantile  Q(p)            = pnl[0] + h * d
//!   ES (tail mean of Q over [0,p]) = (1/p) * integral_0^p Q du
//!                                  = pnl[0] + (h / 2) * d
//!
//! Both samples use a non-integer `(1 - alpha) * n`, the regime where the old
//! ceil-index estimator rounded the tail outward and produced a VaR / ES pair
//! that disagreed with the true interpolated tail.

use finstack_valuations::metrics::risk::VarResult;

/// A uniform arithmetic progression of `n` losses with step `d` starting at
/// `start`, returned in DESCENDING order so `VarResult::from_distribution`
/// performs a genuine sort. The ascending (sorted) sequence is therefore
/// `start, start + d, start + 2d, ...`.
fn descending_progression(start: f64, step: f64, n: usize) -> Vec<f64> {
    (0..n)
        .rev()
        .map(|i| start + step * i as f64)
        .collect::<Vec<_>>()
}

/// Case 1 - single linear segment.
///
/// `n = 90`, `alpha = 0.99` => `p = 0.01`, `(1 - alpha) * n = 0.9` (non-integer).
/// Sorted ascending P&L: `pnl[i] = -1000 + 10 * i`, so `pnl[0] = -1000`,
/// `pnl[1] = -990`, ... all losses.
///
/// type-7: `h = (n - 1) * p = 89 * 0.01 = 0.89`, `lo = 0`, `frac = 0.89`.
///   VaR quantile Q(p) = -1000 + 0.89 * 10 = -991.1  => VaR = 991.1
/// The tail `[0, p]` lies inside the first segment (knot at 1/89 ~= 0.01124),
/// so ES is one trapezoid:
///   ES = pnl[0] + (h / 2) * d = -1000 + 0.445 * 10 = -995.55  => ES = 995.55
///
/// Old ceil-index estimator (parent f78c6228e):
///   var_index = ceil(0.01 * 90) - 1 = ceil(0.9) - 1 = 0  => VaR = -pnl[0] = 1000.0
///   tail_size = var_index + 1 = 1                        => ES  = mean{pnl[0]} = 1000.0
/// Both buggy values (1000.0) differ from the hand-computed 991.1 / 995.55.
#[test]
fn historical_var_single_segment_linear_interpolation() {
    let pnls = descending_progression(-1000.0, 10.0, 90);
    let result = VarResult::from_distribution(pnls, 0.99).expect("finite P&L distribution");

    assert_eq!(result.num_scenarios, 90);

    let expected_var = 991.1;
    let expected_es = 995.55;

    assert!(
        (result.var - expected_var).abs() < 1e-9,
        "interpolated 99% VaR should be {expected_var} (hand-computed Q(0.01)), got {} \
         (the ceil-index estimator wrongly reported 1000.0)",
        result.var,
    );
    assert!(
        (result.expected_shortfall - expected_es).abs() < 1e-9,
        "ES should be {expected_es} (hand-computed mean of Q over [0, 0.01]), got {} \
         (the var_index+1 estimator wrongly reported 1000.0)",
        result.expected_shortfall,
    );

    // VaR and ES are derived from one quantile definition: ES is the mean of
    // the quantile function over [0, p] and VaR is its least-extreme endpoint
    // Q(p), so ES >= VaR holds exactly.
    assert!(
        result.expected_shortfall >= result.var,
        "ES ({}) must be >= VaR ({})",
        result.expected_shortfall,
        result.var,
    );
}

/// Case 2 - tail spans two linear segments, and the old ceil index lands on a
/// NON-extreme order statistic.
///
/// `n = 150`, `alpha = 0.99` => `p = 0.01`, `(1 - alpha) * n = 1.5` (non-integer).
/// Sorted ascending P&L: `pnl[i] = -1000 + 10 * i`.
///
/// type-7: `h = (n - 1) * p = 149 * 0.01 = 1.49`, `lo = 1`, `frac = 0.49`.
///   VaR quantile Q(p) = pnl[1] + 0.49 * (pnl[2] - pnl[1])
///                     = -990 + 0.49 * 10 = -985.1            => VaR = 985.1
/// The tail `[0, p]` crosses the knot at `u = 1/149 ~= 0.006711`, so the ES
/// integral spans two linear pieces of Q; for the uniform progression this
/// still evaluates to the closed form
///   ES = pnl[0] + (h / 2) * d = -1000 + 0.745 * 10 = -992.55  => ES = 992.55
///
/// Old ceil-index estimator (parent f78c6228e):
///   var_index = ceil(0.01 * 150) - 1 = ceil(1.5) - 1 = 1  => VaR = -pnl[1] = 990.0
///   tail_size = var_index + 1 = 2 => ES = mean{-pnl[0], -pnl[1]} = mean{1000, 990} = 995.0
/// Both buggy values differ from the hand-computed 985.1 / 992.55: the ceil
/// estimator rounds the tail outward to the more-extreme order statistic.
#[test]
fn historical_var_two_segment_interpolation_with_nonextreme_ceil_index() {
    let pnls = descending_progression(-1000.0, 10.0, 150);
    let result = VarResult::from_distribution(pnls, 0.99).expect("finite P&L distribution");

    assert_eq!(result.num_scenarios, 150);

    let expected_var = 985.1;
    let expected_es = 992.55;

    assert!(
        (result.var - expected_var).abs() < 1e-9,
        "interpolated 99% VaR should be {expected_var} (hand-computed Q(0.01)), got {} \
         (the ceil-index estimator wrongly reported 990.0)",
        result.var,
    );
    assert!(
        (result.expected_shortfall - expected_es).abs() < 1e-9,
        "ES should be {expected_es} (hand-computed mean of Q over [0, 0.01]), got {} \
         (the var_index+1 estimator wrongly reported 995.0)",
        result.expected_shortfall,
    );

    assert!(
        result.expected_shortfall >= result.var,
        "ES ({}) must be >= VaR ({})",
        result.expected_shortfall,
        result.var,
    );
}

/// VaR / ES consistency: the ES tail mass corresponds to the SAME quantile the
/// VaR uses. Compare a calculated ES against an independent reconstruction of
/// the tail integral built only from the VaR-defining quantile function.
///
/// `n = 150`, `alpha = 0.99` as above. The tail `[0, p]` is integrated as a
/// fine Riemann/trapezoid sum of the type-7 quantile function evaluated only
/// via VaR-style quantiles; this must converge to the engine's ES.
#[test]
fn historical_var_es_uses_same_quantile_as_var() {
    let n = 150usize;
    let alpha = 0.99;
    let p = 1.0 - alpha;

    let pnls = descending_progression(-1000.0, 10.0, n);
    let result = VarResult::from_distribution(pnls.clone(), alpha).expect("finite distribution");

    // Reconstruct the type-7 quantile function from the sorted sample exactly
    // as the VaR path defines it (linear interpolation between order stats).
    let mut sorted = pnls;
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n_minus_1 = (n - 1) as f64;
    let quantile_at = |u: f64| -> f64 {
        let h = (n_minus_1 * u).clamp(0.0, n_minus_1);
        let lo = (h.floor() as usize).min(n - 2);
        let frac = h - lo as f64;
        sorted[lo] + frac * (sorted[lo + 1] - sorted[lo])
    };

    // The VaR the engine reports is exactly Q(p) under this quantile function.
    let q_p = quantile_at(p);
    assert!(
        (result.var - (-q_p)).abs() < 1e-9,
        "engine VaR ({}) must equal -Q(p) of the type-7 quantile ({})",
        result.var,
        -q_p,
    );

    // ES = mean of the SAME Q over [0, p]. Approximate the tail integral with
    // a fine trapezoid sum over Q; it must match the engine's ES, confirming
    // both measures share one quantile definition.
    let steps = 200_000usize;
    let du = p / steps as f64;
    let mut integral = 0.0;
    for k in 0..steps {
        let u0 = k as f64 * du;
        let u1 = (k + 1) as f64 * du;
        integral += 0.5 * (quantile_at(u0) + quantile_at(u1)) * du;
    }
    let es_reconstructed = -(integral / p);

    assert!(
        (result.expected_shortfall - es_reconstructed).abs() < 1e-4,
        "engine ES ({}) must match the tail mean of the VaR-defining quantile ({})",
        result.expected_shortfall,
        es_reconstructed,
    );

    // And the tail-count consistency check: ES averages strictly more of the
    // loss distribution than the single VaR point, so ES is at least as
    // extreme as VaR.
    assert!(
        result.expected_shortfall >= result.var,
        "ES ({}) >= VaR ({}) under one consistent quantile definition",
        result.expected_shortfall,
        result.var,
    );
}
