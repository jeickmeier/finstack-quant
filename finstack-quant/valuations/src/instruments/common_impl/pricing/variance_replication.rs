//! Carr-Madan discrete variance replication integral.
//!
//! Provides a shared implementation of the log-contract replication approach
//! (Carr & Madan, 1998) used by both equity and FX variance swap pricers.
//!
//! # Methodology
//!
//! Fair forward variance is replicated from a static portfolio of out-of-the-money
//! options (Demeterfi-Derman-Kamal-Zou, 1999; Carr-Madan, 1998):
//!
//! ```text
//! σ²·T = (2·e^{rT}) · ∫₀^∞ Q(K)/K² dK
//!        − 2·[ (F/K₀ − 1) − ln(F/K₀) ]
//! ```
//!
//! where `Q(K)` is the price of the OTM option struck at `K` (put for `K < F`,
//! call for `K > F`) and `K₀` is the highest strike at or below the forward `F`.
//!
//! The anchor term `−2·[(F/K₀−1) − ln(F/K₀)]` is the **exact** Demeterfi
//! correction for the discrete portfolio being struck at `K₀` rather than
//! exactly at `F`; it is *not* the 2nd-order expansion `−(F/K₀−1)²`.
//!
//! # Wing truncation
//!
//! A real strike grid is finite, so the integral `∫₀^∞` is truncated to
//! `[k_min, k_max]`. The contribution of deep-OTM options outside the grid is
//! recovered with an explicit tail term: the implied volatility is held flat at
//! the edge value and the integrand is integrated over a synthetic wing
//! extension. Without this correction a grid that does not reach far into the
//! wings biases forward variance **low**, silently. When even the synthetic
//! wings are too narrow to capture the tail a diagnostic is emitted.

use crate::instruments::common_impl::parameters::market::OptionType;

/// Minimum implied volatility considered a usable surface value.
///
/// A surface that returns a vol at or below this floor for a strike that the
/// replication integral actually samples is treated as broken: the variance
/// it would produce is plausible-but-wrong, so the integral returns `None`
/// (with a diagnostic) rather than silently clamping.
const MIN_USABLE_VOL: f64 = 1e-6;

/// Minimum forward / strike level treated as economically meaningful.
///
/// A forward or anchor strike at or below this level cannot anchor the
/// log-contract correction; the integral returns `None` rather than clamping.
const MIN_USABLE_LEVEL: f64 = 1e-9;

/// Wing half-width (in log-moneyness) below which the strike grid is considered
/// too narrow to capture the variance tails even after synthetic extension.
///
/// `ln(F/k_min)` and `ln(k_max/F)` are each compared against this; a grid
/// reaching ±~3 forward-vol standard deviations is comfortably wider. A grid
/// narrower than this still produces a number, but the tail correction cannot
/// fully compensate, so a diagnostic is logged.
const MIN_WING_LOG_MONEYNESS: f64 = 0.35;

/// Factor by which the synthetic wing extends past the outermost quoted strike.
///
/// The lower wing runs from `k_min / WING_EXTENSION_FACTOR` up to `k_min`; the
/// upper wing from `k_max` to `k_max * WING_EXTENSION_FACTOR`. Flat-vol
/// extrapolation makes the integrand negligible well before these bounds for
/// any realistic surface, so the exact factor is not delicate.
const WING_EXTENSION_FACTOR: f64 = 3.0;

/// Number of trapezoidal sub-intervals used inside each synthetic wing.
const WING_SUBDIVISIONS: usize = 64;

/// Carr-Madan discrete variance replication integral.
///
/// Computes forward variance from a discrete strike grid using the
/// log-contract replication approach (Carr & Madan, 1998; Demeterfi et al., 1999).
///
/// `vol_fn(t, k)` returns implied volatility at time `t` and strike `k`.
/// `bs_price_fn(strike, vol, option_type)` returns the Black-Scholes option price.
/// All other parameters (spot, rates, etc.) should be captured by the closures.
///
/// # Arguments
///
/// * `strikes` - At least three finite, strictly increasing positive strikes
///   in the same price units as `forward`; the grid must contain levels below
///   and above the forward.
/// * `forward` - Positive forward price for the option expiry, expressed in
///   the same units as `strikes`.
/// * `risk_free_rate` - Continuously compounded annual risk-free rate used to
///   convert the discounted option-price integral to forward variance.
/// * `time_to_expiry` - Positive option expiry in years.
/// * `vol_fn` - Implied-volatility surface queried as `(expiry_years, strike)`
///   and returning a finite positive annualized decimal volatility.
/// * `bs_price_fn` - Pricing callback receiving `(strike, volatility,
///   option_type)` and returning the discounted call or put value in the same
///   monetary units as the forward.
///
/// Returns `None` if:
/// - the strike grid is too short, non-monotone, or non-finite,
/// - the forward is non-finite or non-positive,
/// - the surface returns a non-positive / sub-floor volatility for a sampled
///   strike (a broken surface is reported, not clamped — see the internal MIN_USABLE_VOL constant),
/// - the result is non-finite or non-positive.
pub fn carr_madan_forward_variance(
    strikes: &[f64],
    forward: f64,
    risk_free_rate: f64,
    time_to_expiry: f64,
    vol_fn: impl Fn(f64, f64) -> f64,
    bs_price_fn: impl Fn(f64, f64, OptionType) -> f64,
) -> Option<f64> {
    if strikes.len() < 3
        || !forward.is_finite()
        || forward <= MIN_USABLE_LEVEL
        || !risk_free_rate.is_finite()
        || !time_to_expiry.is_finite()
        || time_to_expiry <= 0.0
    {
        return None;
    }

    // Strike grid must be strictly increasing and finite. Carr-Madan integration
    // assumes a monotone integration domain; non-monotonic or duplicate strikes
    // produce silently nonsensical variance because `dk` (the trapezoidal width)
    // becomes zero or negative.
    if !strikes.iter().all(|k| k.is_finite() && *k > 0.0) {
        return None;
    }
    if !strikes.windows(2).all(|w| w[0] < w[1]) {
        return None;
    }
    if strikes.first().is_none_or(|k| *k >= forward) || strikes.last().is_none_or(|k| *k <= forward)
    {
        return None;
    }

    // Find the highest strike at or below the forward.
    let k0_idx = {
        let mut idx = 0usize;
        for (i, &k) in strikes.iter().enumerate() {
            if k <= forward {
                idx = i;
            } else {
                break;
            }
        }
        idx
    };
    // The anchor strike must be a genuine market level. A broken grid whose
    // anchor collapses to ~0 is reported rather than clamped into a
    // plausible-but-wrong variance.
    let k0 = *strikes.get(k0_idx)?;
    if !k0.is_finite() || k0 <= MIN_USABLE_LEVEL {
        return None;
    }

    // --- Replication integral ∫ Q(K)/K² dK over the quoted grid ---------------
    //
    // The integrand spans a wide dynamic range (deep-OTM option prices are tiny,
    // near-ATM prices are O(F·σ·√T)), so the trapezoidal accumulation uses
    // Neumaier compensation per the workspace summation invariant.
    //
    // Trapezoidal weights: interior points get the midpoint cell
    // `0.5·(k[i+1]−k[i−1])`; the two **endpoints** get a genuine HALF cell
    // (`0.5·(k₁−k₀)` and `0.5·(kₙ−k_{n-1})`). The endpoint sits exactly on the
    // grid boundary, so half its cell belongs to this core integral and the
    // other half is the first sub-interval of the wing-truncation correction
    // below. (The earlier code used a *full* endpoint cell — an ad-hoc implicit
    // tail; combined with an explicit wing term that would double-count the
    // boundary integrand.)
    let mut integral = finstack_quant_core::math::NeumaierAccumulator::new();
    for i in 0..strikes.len() {
        // Strike positivity already verified above; index is in range.
        let k = *strikes.get(i)?;
        let dk = if i == 0 {
            // First endpoint: half of the first cell.
            0.5 * finite_diff(*strikes.get(1)?, *strikes.first()?)?
        } else if i + 1 == strikes.len() {
            // Last endpoint: half of the last cell.
            0.5 * finite_diff(*strikes.get(i)?, *strikes.get(i - 1)?)?
        } else {
            0.5 * finite_diff(*strikes.get(i + 1)?, *strikes.get(i - 1)?)?
        };

        let vol = vol_fn(time_to_expiry, k);
        // A non-positive or sub-floor vol at a sampled strike means the surface
        // is broken. Surface it as `None` instead of clamping to 1e-8 and
        // returning a plausible-but-wrong variance.
        if !vol.is_finite() || vol <= MIN_USABLE_VOL {
            tracing::debug!(
                target = "finstack_quant.variance_replication",
                strike = k,
                vol,
                "Carr-Madan replication: surface returned non-usable volatility; \
                 reporting failure rather than clamping"
            );
            return None;
        }

        let qk = otm_option_price(k, k0_idx, i, vol, &bs_price_fn);
        integral.add((dk / (k * k)) * qk);
    }

    // --- Wing-truncation tail correction --------------------------------------
    //
    // The quoted grid stops at `k_min`/`k_max`; deep-OTM options outside it
    // still carry variance. Extend the integrand into both wings with the edge
    // implied vol held flat and integrate the synthetic extension. Skipping
    // this term biases forward variance low for a narrow grid.
    let k_min = *strikes.first()?;
    let k_max = *strikes.last()?;
    let vol_low = vol_fn(time_to_expiry, k_min);
    let vol_high = vol_fn(time_to_expiry, k_max);
    if !vol_low.is_finite()
        || vol_low <= MIN_USABLE_VOL
        || !vol_high.is_finite()
        || vol_high <= MIN_USABLE_VOL
    {
        return None;
    }

    let lower_tail = wing_tail_integral(
        k_min / WING_EXTENSION_FACTOR,
        k_min,
        vol_low,
        OptionType::Put,
        &bs_price_fn,
    )?;
    let upper_tail = wing_tail_integral(
        k_max,
        k_max * WING_EXTENSION_FACTOR,
        vol_high,
        OptionType::Call,
        &bs_price_fn,
    )?;
    integral.add(lower_tail);
    integral.add(upper_tail);

    // Diagnostic: even with the synthetic wings, a grid that barely straddles
    // the forward cannot reproduce the full tail. Warn so a too-narrow surface
    // is visible rather than silently biasing variance low.
    let lower_wing_lm = (forward / k_min).ln();
    let upper_wing_lm = (k_max / forward).ln();
    if lower_wing_lm < MIN_WING_LOG_MONEYNESS || upper_wing_lm < MIN_WING_LOG_MONEYNESS {
        tracing::warn!(
            target = "finstack_quant.variance_replication",
            k_min,
            k_max,
            forward,
            lower_wing_log_moneyness = lower_wing_lm,
            upper_wing_log_moneyness = upper_wing_lm,
            min_wing_log_moneyness = MIN_WING_LOG_MONEYNESS,
            "Carr-Madan replication: strike grid is narrow; the wing-truncation \
             correction may not fully capture the variance tails (forward variance \
             biased low)"
        );
    }

    // Exact Demeterfi anchor term (see `demeterfi_anchor`).
    let anchor = demeterfi_anchor(forward, k0, time_to_expiry);

    let variance = (2.0 * (risk_free_rate * time_to_expiry).exp() / time_to_expiry)
        * integral.total()
        - anchor;

    if variance.is_finite() && variance > 0.0 {
        Some(variance)
    } else {
        None
    }
}

/// Exact Demeterfi log-contract anchor correction.
///
/// The discrete replication portfolio is struck at `K₀` (the highest grid
/// strike at or below the forward `F`), not exactly at `F`. The correction for
/// that offset is the **exact** log-contract expression
///
/// ```text
/// (2/T) · [ (F/K₀ − 1) − ln(F/K₀) ]
/// ```
///
/// from Demeterfi-Derman-Kamal-Zou (1999), eq. for the "cash flow from
/// rebalancing". The earlier code used the 2nd-order Taylor expansion
/// `(1/T)·(F/K₀ − 1)²`, which under-corrects (the omitted 3rd-order term is
/// `−(1/3T)(F/K₀−1)³`) and biases forward variance whenever the forward does
/// not land exactly on a grid strike.
///
/// `forward` and `k0` are pre-validated by the caller to be finite and
/// `> MIN_USABLE_LEVEL`, and `t > 0`, so `F/K₀` is finite, positive, and the
/// logarithm is well defined.
#[inline]
fn demeterfi_anchor(forward: f64, k0: f64, t: f64) -> f64 {
    let moneyness = forward / k0;
    (2.0 / t) * ((moneyness - 1.0) - moneyness.ln())
}

/// Price of the out-of-the-money option used at grid index `i`.
///
/// The Demeterfi replication portfolio splits the put/call legs at the **anchor
/// strike** `K₀` (`= strikes[k0_idx]`), not at the forward `F`:
///
/// - `i < k0_idx` (`K < K₀`): put,
/// - `i > k0_idx` (`K > K₀`): call,
/// - `i == k0_idx` (`K = K₀`): the boundary cell straddles `K₀`, so the put and
///   call are averaged.
///
/// Splitting at `F` instead of `K₀` (the prior behavior) priced every strike in
/// `(K₀, F)` as a put when the replication portfolio requires a call there. By
/// put-call parity that mis-prices each such strike by `e^{-rT}(F − K)`, and
/// the resulting integral error `(2e^{rT}/T)∫_{K₀}^{F}(F-K)/K² dK` is a *fixed*
/// bias that does **not** vanish as the strike grid is refined — i.e. the
/// replication converged to the wrong variance.
#[inline]
fn otm_option_price(
    k: f64,
    k0_idx: usize,
    i: usize,
    vol: f64,
    bs_price_fn: &impl Fn(f64, f64, OptionType) -> f64,
) -> f64 {
    use std::cmp::Ordering;
    match i.cmp(&k0_idx) {
        Ordering::Equal => {
            0.5 * (bs_price_fn(k, vol, OptionType::Put) + bs_price_fn(k, vol, OptionType::Call))
        }
        Ordering::Less => bs_price_fn(k, vol, OptionType::Put),
        Ordering::Greater => bs_price_fn(k, vol, OptionType::Call),
    }
}

/// Trapezoidal integral of `Q(K)/K²` over a synthetic wing `[lo, hi]`.
///
/// The implied volatility is held flat at `vol` (the edge value) — consistent
/// with the flat-strike extrapolation a volatility surface applies past its
/// outermost quote anyway. `option_type` is `Put` for the lower wing and
/// `Call` for the upper wing (both deep OTM throughout the wing).
///
/// Returns `None` if the wing is degenerate (`hi <= lo`).
fn wing_tail_integral(
    lo: f64,
    hi: f64,
    vol: f64,
    option_type: OptionType,
    bs_price_fn: &impl Fn(f64, f64, OptionType) -> f64,
) -> Option<f64> {
    if !lo.is_finite() || !hi.is_finite() || hi <= lo || lo <= 0.0 {
        // A degenerate wing contributes no tail; treat it as zero rather than
        // failing the whole replication (the diagnostic above covers the
        // narrow-grid case).
        return Some(0.0);
    }

    let n = WING_SUBDIVISIONS;
    let step = (hi - lo) / (n as f64);
    let mut acc = finstack_quant_core::math::NeumaierAccumulator::new();
    for j in 0..=n {
        let k = lo + step * (j as f64);
        if k <= 0.0 {
            continue;
        }
        let integrand = bs_price_fn(k, vol, option_type) / (k * k);
        // Trapezoidal weights: endpoints get half weight.
        let weight = if j == 0 || j == n { 0.5 } else { 1.0 };
        acc.add(weight * step * integrand);
    }
    Some(acc.total())
}

/// `a - b`, but `None` if the result is non-finite.
///
/// `f64` subtraction never panics, but a non-finite difference (e.g. from an
/// infinite strike that slipped past validation) must not silently poison the
/// integral.
#[inline]
fn finite_diff(a: f64, b: f64) -> Option<f64> {
    let d = a - b;
    d.is_finite().then_some(d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::closed_form::vanilla::bs_price;

    /// A wide, dense strike grid spanning deep into both wings. With this grid
    /// the discrete replication should reproduce flat-vol variance tightly.
    fn wide_grid() -> Vec<f64> {
        (10..=400).map(|k| k as f64).collect()
    }

    #[test]
    fn test_carr_madan_atm_flat_vol() {
        let vol = 0.20;
        let t = 1.0;
        let fwd = 100.0;
        let r = 0.05;
        let spot = fwd;
        let strikes = wide_grid();
        let vol_fn = |_t: f64, _k: f64| vol;
        let bs_fn =
            |k: f64, v: f64, opt: OptionType| -> f64 { bs_price(spot, k, r, 0.0, v, t, opt) };
        let variance = carr_madan_forward_variance(&strikes, fwd, r, t, vol_fn, bs_fn)
            .expect("Expected Some variance");
        assert!(
            (variance - vol * vol).abs() < 0.01,
            "Expected ~{}, got {}",
            vol * vol,
            variance
        );
    }

    #[test]
    fn one_sided_strike_grids_are_rejected() {
        let vol_fn = |_t: f64, _k: f64| 0.2;
        let bs_fn = |k: f64, v: f64, opt: OptionType| bs_price(100.0, k, 0.0, 0.0, v, 1.0, opt);
        assert!(carr_madan_forward_variance(
            &[110.0, 120.0, 130.0],
            100.0,
            0.0,
            1.0,
            vol_fn,
            bs_fn,
        )
        .is_none());
        assert!(
            carr_madan_forward_variance(&[70.0, 80.0, 90.0], 100.0, 0.0, 1.0, vol_fn, bs_fn,)
                .is_none()
        );
    }

    /// Regression for [P5-6](c): the anchor term must be the EXACT Demeterfi
    /// log-contract correction `(2/T)[(F/K₀−1)−ln(F/K₀)]`, not its 2nd-order
    /// Taylor expansion `(1/T)(F/K₀−1)²`.
    ///
    /// Failure mode locked in: with the forward sitting away from a grid strike
    /// (so `F/K₀ ≠ 1`), the Taylor anchor under-corrects by the omitted
    /// 3rd-order term `(1/3T)(F/K₀−1)³`. The two forms must NOT be equal, and
    /// `demeterfi_anchor` must return the exact value. Tested directly on the
    /// anchor helper so the assertion is not muddied by the trapezoidal
    /// discretization error of the replication integral itself.
    #[test]
    fn test_demeterfi_anchor_is_exact_not_taylor() {
        let t = 0.75;
        // A range of moneyness offsets, including a coarse-grid-sized 9% gap
        // where the exact/Taylor discrepancy is clearly material.
        for &(forward, k0) in &[
            (100.6, 100.0), // ~0.6% — fine grid
            (102.5, 100.0), // 2.5%
            (109.0, 100.0), // 9% — coarse grid
            (95.0, 100.0),  // forward below K₀ (last strike <= F can equal F-side)
        ] {
            let m = forward / k0;
            let exact = demeterfi_anchor(forward, k0, t);
            let exact_ref = (2.0 / t) * ((m - 1.0) - m.ln());
            let taylor = (1.0 / t) * (m - 1.0).powi(2);

            assert!(
                (exact - exact_ref).abs() < 1e-14,
                "demeterfi_anchor must return the exact log-contract form: \
                 got {exact}, expected {exact_ref}"
            );
            // Exact and Taylor must differ — if `demeterfi_anchor` were the
            // Taylor form this is where the regression would catch it.
            let third_order = ((m - 1.0).powi(3) / 3.0 / t).abs();
            assert!(
                (exact - taylor).abs() > 0.4 * third_order,
                "exact anchor must differ from the Taylor expansion at m={m}: \
                 exact={exact}, taylor={taylor}"
            );
        }
    }

    /// The exact anchor must drive the replicated variance to vol² for a flat
    /// surface when the grid is fine (discretization error negligible) and the
    /// forward is non-trivial (`r ≠ 0`). This is the end-to-end counterpart to
    /// `test_demeterfi_anchor_is_exact_not_taylor`.
    ///
    /// `spot` is chosen so the Black-Scholes forward `spot·e^{rT}` equals the
    /// `forward` argument — otherwise the option prices would encode a
    /// different forward than the anchor uses and the test would conflate that
    /// mismatch with the anchor it is meant to pin down.
    #[test]
    fn test_carr_madan_flat_vol_with_exact_anchor_fine_grid() {
        let vol = 0.25_f64;
        let t = 0.75_f64;
        let r = 0.03_f64;
        let fwd = 100.6_f64;
        let spot = fwd * (-r * t).exp();
        // Fine grid: 0.1 spacing — trapezoidal error well below 1e-4 in variance.
        let strikes: Vec<f64> = (1..=4000).map(|i| 0.1 * (i as f64)).collect();
        let vol_fn = |_t: f64, _k: f64| vol;
        let bs_fn =
            |k: f64, v: f64, opt: OptionType| -> f64 { bs_price(spot, k, r, 0.0, v, t, opt) };

        let variance = carr_madan_forward_variance(&strikes, fwd, r, t, vol_fn, bs_fn)
            .expect("Expected Some variance");
        assert!(
            (variance - vol * vol).abs() < 5e-4,
            "fine-grid flat-vol variance with the exact anchor should match \
             vol²={}, got {}",
            vol * vol,
            variance
        );
    }

    /// Regression for [P5-6](b): a strike grid that does not reach into the
    /// wings biases forward variance LOW unless a tail correction is applied.
    ///
    /// Failure mode locked in: without the wing-truncation term a narrow grid
    /// systematically under-reports flat-vol variance. With the correction the
    /// narrow-grid result is materially closer to vol².
    #[test]
    fn test_carr_madan_wing_truncation_recovers_low_bias() {
        let vol = 0.20;
        let t = 1.0;
        let fwd = 100.0;
        let r = 0.0;
        let spot = fwd;
        let vol_fn = |_t: f64, _k: f64| vol;
        let bs_fn =
            |k: f64, v: f64, opt: OptionType| -> f64 { bs_price(spot, k, r, 0.0, v, t, opt) };

        // Narrow grid: only ±~18% around the forward — well inside the wings.
        let narrow: Vec<f64> = (82..=118).map(|k| k as f64).collect();
        let narrow_var = carr_madan_forward_variance(&narrow, fwd, r, t, vol_fn, bs_fn)
            .expect("narrow-grid variance");

        // The corrected narrow-grid variance should be close to vol². Before
        // the fix the same grid under-reported by several percent of variance.
        let target = vol * vol;
        assert!(
            (narrow_var - target).abs() < 0.10 * target,
            "wing-truncation correction should keep narrow-grid variance near \
             vol²={target}, got {narrow_var}"
        );
        // And it must not collapse to a grossly low value.
        assert!(
            narrow_var > 0.5 * target,
            "narrow-grid variance must not be biased far below vol²: got {narrow_var}"
        );
    }

    /// Regression for [P5-6](a): the replication integral must accumulate with
    /// compensated (Neumaier) summation over a wide-dynamic-range strike grid.
    ///
    /// Failure mode locked in: a very fine, very wide grid mixes O(1e1) ATM
    /// integrand cells with O(1e-12) deep-wing cells; naive `+=` loses the
    /// small terms. The compensated result must match flat-vol variance.
    #[test]
    fn test_carr_madan_compensated_summation_on_fine_wide_grid() {
        let vol = 0.30;
        let t = 2.0;
        let fwd = 100.0;
        let r = 0.0;
        let spot = fwd;
        let vol_fn = |_t: f64, _k: f64| vol;
        let bs_fn =
            |k: f64, v: f64, opt: OptionType| -> f64 { bs_price(spot, k, r, 0.0, v, t, opt) };

        // ~8000 strikes from deep ITM to deep OTM: a wide dynamic range.
        let fine: Vec<f64> = (1..=8000).map(|i| 1.0 + 0.25 * (i as f64)).collect();
        let variance = carr_madan_forward_variance(&fine, fwd, r, t, vol_fn, bs_fn)
            .expect("fine-grid variance");
        assert!(
            (variance - vol * vol).abs() < 5e-4,
            "compensated summation should reproduce vol²={}, got {}",
            vol * vol,
            variance
        );
    }

    /// Regression for [P5-6](low) / item 5: a broken vol surface returning a
    /// near-zero volatility for a sampled strike must surface as `None`, not be
    /// silently clamped to 1e-8 and turned into a plausible-but-wrong variance.
    #[test]
    fn test_carr_madan_reports_broken_surface_instead_of_clamping() {
        let t = 1.0;
        let fwd = 100.0;
        let r = 0.0;
        let spot = fwd;
        let bs_fn =
            |k: f64, v: f64, opt: OptionType| -> f64 { bs_price(spot, k, r, 0.0, v, t, opt) };
        let strikes = wide_grid();

        // Surface returns zero vol everywhere: broken.
        let zero_vol = |_t: f64, _k: f64| 0.0;
        assert!(
            carr_madan_forward_variance(&strikes, fwd, r, t, zero_vol, bs_fn).is_none(),
            "a zero-volatility surface must be reported as None, not clamped"
        );

        // Surface returns a NaN vol for one strike: broken.
        let nan_at_120 = |_t: f64, k: f64| {
            if (k - 120.0).abs() < 1e-9 {
                f64::NAN
            } else {
                0.2
            }
        };
        assert!(
            carr_madan_forward_variance(&strikes, fwd, r, t, nan_at_120, bs_fn).is_none(),
            "a NaN volatility at a sampled strike must be reported as None"
        );
    }

    #[test]
    fn test_carr_madan_returns_none_for_too_few_strikes() {
        let vol_fn = |_t: f64, _k: f64| 0.2;
        let bs_fn = |_k: f64, _v: f64, _opt: OptionType| -> f64 { 1.0 };
        assert!(
            carr_madan_forward_variance(&[100.0, 101.0], 100.0, 0.05, 1.0, vol_fn, bs_fn).is_none()
        );
    }

    #[test]
    fn test_carr_madan_returns_none_for_invalid_forward() {
        let vol_fn = |_t: f64, _k: f64| 0.2;
        let bs_fn = |_k: f64, _v: f64, _opt: OptionType| -> f64 { 1.0 };
        let strikes: Vec<f64> = (50..=150).map(|k| k as f64).collect();
        assert!(
            carr_madan_forward_variance(&strikes, f64::NAN, 0.05, 1.0, vol_fn, bs_fn).is_none()
        );
        assert!(carr_madan_forward_variance(&strikes, -1.0, 0.05, 1.0, vol_fn, bs_fn).is_none());
    }

    #[test]
    fn test_carr_madan_returns_none_for_invalid_time() {
        let vol_fn = |_t: f64, _k: f64| 0.2;
        let bs_fn = |_k: f64, _v: f64, _opt: OptionType| -> f64 { 1.0 };
        let strikes: Vec<f64> = (50..=150).map(|k| k as f64).collect();
        assert!(
            carr_madan_forward_variance(&strikes, 100.0, 0.05, 0.0, vol_fn, bs_fn).is_none(),
            "zero time to expiry must be rejected"
        );
        assert!(
            carr_madan_forward_variance(&strikes, 100.0, 0.05, f64::NAN, vol_fn, bs_fn).is_none(),
            "non-finite time to expiry must be rejected"
        );
    }

    #[test]
    fn test_carr_madan_rejects_non_monotonic_strike_grid() {
        let vol_fn = |_t: f64, _k: f64| 0.2;
        let bs_fn = |_k: f64, _v: f64, _opt: OptionType| -> f64 { 1.0 };
        // Non-monotonic — third entry breaks the ordering
        let strikes = vec![80.0, 100.0, 95.0, 120.0];
        assert!(
            carr_madan_forward_variance(&strikes, 100.0, 0.05, 1.0, vol_fn, bs_fn).is_none(),
            "non-monotonic strike grid must be rejected"
        );
        // Duplicate strike — the dk for the duplicate is zero
        let dup_strikes = vec![80.0, 100.0, 100.0, 120.0];
        assert!(
            carr_madan_forward_variance(&dup_strikes, 100.0, 0.05, 1.0, vol_fn, bs_fn).is_none(),
            "duplicate strikes must be rejected"
        );
    }

    #[test]
    fn test_carr_madan_rejects_non_finite_strike() {
        let vol_fn = |_t: f64, _k: f64| 0.2;
        let bs_fn = |_k: f64, _v: f64, _opt: OptionType| -> f64 { 1.0 };
        let strikes = vec![80.0, f64::NAN, 120.0];
        assert!(carr_madan_forward_variance(&strikes, 100.0, 0.05, 1.0, vol_fn, bs_fn).is_none());
    }
}
