//! Curve-fitting and bond-reconstruction helpers shared by the HW1F
//! exotic-rate Monte Carlo pricers.
//!
//! Two responsibilities, both required for an arbitrage-free HW1F simulation:
//!
//! 1. **θ(t) calibration (defect M6).** The Hull-White 1-factor model fits the
//!    initial discount curve only when its mean-reversion *level* θ is
//!    time-dependent (Brigo–Mercurio §3.3.1). [`calibrate_hw1f_params`] wraps
//!    the canonical `finstack_monte_carlo::process::ou::calibrate_theta_from_curve`
//!    bootstrap, presenting the discount curve as the `P(as_of, as_of+t)`
//!    closure the calibrator expects.
//!
//! 2. **Term-forward reconstruction (defect M7).** An exotic coupon indexed to,
//!    e.g., a 6-month rate must use the *term* simple forward, not the
//!    instantaneous short rate r(t). Under HW1F the time-`t` zero-coupon bond
//!    is affine in r(t):
//!
//!    ```text
//!    P(t,T) = A(t,T) · exp(−B(t,T) · r(t))
//!    ```
//!
//!    so the period simple forward over `[t, t+τ]` is
//!
//!    ```text
//!    L(t; t, t+τ) = (1/P(t,t+τ) − 1) / τ
//!                 = (exp(B(t,t+τ)·r(t) − ln A(t,t+τ)) − 1) / τ.
//!    ```
//!
//!    [`Hw1fTermForward`] precomputes the per-event `(B, ln A, τ)` triple from
//!    the *same* initial curve used to calibrate θ(t), so the reconstruction is
//!    consistent with the simulated dynamics.
//!
//! # Reference
//!
//! Brigo & Mercurio (2006) *Interest Rate Models — Theory and Practice*
//! §3.3.1 (HW1F affine bond price, eqs. 3.39–3.40); Hull & White (1990).

use crate::calibration::hull_white::HullWhiteParams;
use finstack_core::dates::{Date, DayCountContext};
use finstack_core::market_data::traits::Discounting;
use finstack_core::Result;
use finstack_monte_carlo::process::ou::{calibrate_theta_from_curve, HullWhite1FParams};

/// Spacing (years) of the piecewise-constant θ(t) bootstrap grid.
///
/// θ(t) is piecewise-constant on intervals of this width, each interval
/// carrying the θ value sampled at its **midpoint** (see [`calibrate_hw1f_params`]).
/// The midpoint rule makes the curve-repricing error O(spacing²) rather than
/// O(spacing) of a left-endpoint rule, so a monthly grid reprices even a
/// steeply-sloped curve to a few bp. Monthly is also fine enough to resolve
/// realistic intra-quarter curve features (e.g. a turn-of-year forward jump).
const THETA_GRID_SPACING_YEARS: f64 = 1.0 / 12.0;

/// Build a `P(as_of, as_of + t)` discount closure from a [`Discounting`] curve.
///
/// The HW1F simulation measures time from `t = 0 ≡ as_of`, but a discount
/// curve is anchored at its own `base_date`. This returns the curve's discount
/// factor *re-based to `as_of`*:
///
/// ```text
/// P(as_of, as_of + t) = DF_curve(t_asof + t) / DF_curve(t_asof)
/// ```
///
/// where `t_asof` is the year fraction from the curve base date to `as_of`.
///
/// # Errors
///
/// Returns an error if the year fraction from the curve base to `as_of`
/// cannot be computed, or if the discount factor at `as_of` is non-positive.
fn rebased_discount_fn<'a>(
    curve: &'a dyn Discounting,
    as_of: Date,
) -> Result<impl Fn(f64) -> f64 + 'a> {
    let base = curve.base_date();
    let dc = curve.day_count();
    let t_asof = if as_of == base {
        0.0
    } else {
        dc.year_fraction(base, as_of, DayCountContext::default())?
    };
    let df_asof = curve.df(t_asof);
    if !df_asof.is_finite() || df_asof <= 0.0 {
        return Err(finstack_core::Error::Validation(format!(
            "HW1F curve fit: discount factor at as_of ({as_of}) is non-positive ({df_asof})"
        )));
    }
    Ok(move |t: f64| {
        let df = curve.df(t_asof + t);
        // Re-base to as_of. Guard the (curve-extrapolation) degenerate case so
        // the calibrator never sees a non-finite discount factor.
        if df.is_finite() && df > 0.0 {
            df / df_asof
        } else {
            0.0
        }
    })
}

/// Calibrate time-dependent HW1F parameters θ(t) to a discount curve.
///
/// Bootstraps a piecewise-constant θ(t) (defect M6 fix) so the simulated short
/// rate reprices the initial curve. The θ(t) formula itself is the canonical
/// `finstack_monte_carlo::process::ou::calibrate_theta_from_curve` bootstrap —
/// this does **not** reinvent it.
///
/// # Midpoint rule
///
/// `calibrate_theta_from_curve` evaluates θ exactly at the breakpoints it is
/// given, and the resulting [`HullWhite1FParams::theta_at_time`] is *left*-
/// continuous: the value at breakpoint `tᵢ` applies on `[tᵢ, tᵢ₊₁)`. Sampling
/// θ at the *left* edge of each interval biases the drift by O(spacing) on a
/// sloped curve. Instead this function samples θ at each interval's **midpoint**
/// and re-pairs those values with the interval *boundaries*, yielding the
/// piecewise-constant midpoint rule (curve-repricing error O(spacing²)).
///
/// # Arguments
///
/// * `hw_params` - Validated HW1F κ, σ.
/// * `discount_curve` - Initial discount curve (any [`Discounting`] curve).
/// * `as_of` - Valuation date; `t = 0` of the simulation.
/// * `horizon` - Last event time (years from `as_of`); the θ(t) grid covers
///   `[0, horizon]`.
///
/// # Errors
///
/// Returns an error if the curve cannot be re-based to `as_of`.
pub fn calibrate_hw1f_params(
    hw_params: HullWhiteParams,
    discount_curve: &dyn Discounting,
    as_of: Date,
    horizon: f64,
) -> Result<HullWhite1FParams> {
    let discount_fn = rebased_discount_fn(discount_curve, as_of)?;

    // Piecewise-constant θ(t) on `n_steps` intervals of width
    // `THETA_GRID_SPACING_YEARS` covering `[0, horizon]` (one extra interval so
    // `theta_at_time` never extrapolates past its last knot at the horizon).
    let n_steps = (horizon / THETA_GRID_SPACING_YEARS).ceil().max(1.0) as usize;

    // Interval midpoints — where θ is sampled for the O(spacing²) midpoint rule.
    let midpoints: Vec<f64> = (0..n_steps)
        .map(|i| (i as f64 + 0.5) * THETA_GRID_SPACING_YEARS)
        .collect();
    // Interval left boundaries — the breakpoints the piecewise-constant θ
    // actually switches on (`theta_at_time` is left-continuous).
    let boundaries: Vec<f64> = (0..n_steps)
        .map(|i| i as f64 * THETA_GRID_SPACING_YEARS)
        .collect();

    // `calibrate_theta_from_curve` evaluates θ at the times it is handed, so
    // passing the midpoints yields the midpoint-sampled θ *values*; re-pair
    // them with the interval boundaries to realise the midpoint rule.
    let midpoint_fit =
        calibrate_theta_from_curve(hw_params.kappa, hw_params.sigma, discount_fn, &midpoints);

    Ok(HullWhite1FParams::with_time_dependent_theta(
        hw_params.kappa,
        hw_params.sigma,
        midpoint_fit.theta_curve,
        boundaries,
    ))
}

/// Initial short rate `r(0)` for a HW1F simulation that reprices `discount_curve`.
///
/// For the HW1F affine bond price `P(t,T) = A(t,T)·exp(−B(t,T)·r(t))` to equal
/// the market price `P(0,T)` at `t = 0`, the short rate **must** start at the
/// curve's instantaneous forward `f(0,0) = −∂/∂t ln P(0,t)|₀` — `A(0,T)` is
/// constructed so that `B(0,T)·f(0,0) − B(0,T)·r(0)` cancels. Seeding `r(0)`
/// from a *projection* (e.g. a separate forward curve's period rate) leaves the
/// simulated short rate offset from `f(0,0)` and the model no longer reprices
/// the discount curve — the defect this M6 fix removes.
///
/// `f(0,0)` is taken by an instantaneous-forward finite difference of `−ln P`
/// on the `as_of`-rebased curve — a one-sided forward difference at `t = 0`.
/// This is the *same kind* of estimator the θ-bootstrap relies on, so `r(0)`
/// and the bootstrapped θ(t) are consistent in construction; the finite-
/// difference step sizes are chosen independently and need not coincide.
///
/// # Errors
///
/// Returns an error if the curve cannot be re-based to `as_of`.
pub fn initial_short_rate_from_curve(discount_curve: &dyn Discounting, as_of: Date) -> Result<f64> {
    let discount_fn = rebased_discount_fn(discount_curve, as_of)?;
    Ok(fd_instantaneous_forward(&discount_fn, 0.0))
}

/// Per-event HW1F bond-reconstruction coefficients for a coupon period.
///
/// Precomputed once per coupon/observation event; the payoff turns a simulated
/// short rate `r(t)` into the period simple forward with a single `exp`.
#[derive(Debug, Clone, Copy)]
pub struct PeriodForwardCoeffs {
    /// `B(t, t+τ) = (1 − e^{−κτ}) / κ`.
    b: f64,
    /// `ln A(t, t+τ)` for the HW1F affine bond price.
    ln_a: f64,
    /// Accrual fraction τ of the coupon period (years).
    tau: f64,
}

impl PeriodForwardCoeffs {
    /// Reconstruct the period simple forward rate from a simulated short rate.
    ///
    /// ```text
    /// L = (1/P(t,t+τ) − 1) / τ,   P(t,t+τ) = A·exp(−B·r)
    ///   = (exp(B·r − ln A) − 1) / τ
    /// ```
    #[inline]
    #[must_use]
    pub fn simple_forward(&self, short_rate: f64) -> f64 {
        if self.tau <= 0.0 {
            return 0.0;
        }
        let inv_p = (self.b * short_rate - self.ln_a).exp();
        (inv_p - 1.0) / self.tau
    }

    /// Degenerate coefficients that reproduce a *fixed* simple forward `rate`
    /// over an accrual `tau`, independent of the short rate (`B = 0`).
    ///
    /// Used by unit tests that exercise payoff *mechanics* (coupon capping,
    /// knock-out, redemption) with a known floating rate, and as a safe
    /// fallback when no HW1F curve is available.
    #[must_use]
    pub fn from_flat_rate(rate: f64, tau: f64) -> Self {
        // simple_forward(r) = (exp(0·r − ln_a) − 1)/τ = (exp(−ln_a) − 1)/τ.
        // Set exp(−ln_a) = 1 + rate·τ ⇒ ln_a = −ln(1 + rate·τ).
        let one_plus = (1.0 + rate * tau).max(f64::MIN_POSITIVE);
        Self {
            b: 0.0,
            ln_a: -one_plus.ln(),
            tau,
        }
    }
}

/// Builds HW1F term-forward reconstructions from the calibrated process and the
/// initial discount curve (defect M7 fix).
///
/// Holds κ, σ, and the `P(as_of, as_of+·)` closure. For each coupon period the
/// caller asks for [`Self::period_coeffs`], which yields a [`PeriodForwardCoeffs`]
/// that the payoff stores and evaluates per path.
pub struct Hw1fTermForward<'a> {
    kappa: f64,
    sigma: f64,
    discount_fn: Box<dyn Fn(f64) -> f64 + 'a>,
}

impl<'a> Hw1fTermForward<'a> {
    /// Construct from the HW1F parameters and the same discount curve used to
    /// calibrate θ(t).
    ///
    /// # Errors
    ///
    /// Returns an error if the curve cannot be re-based to `as_of`.
    pub fn new(
        hw_params: HullWhiteParams,
        discount_curve: &'a dyn Discounting,
        as_of: Date,
    ) -> Result<Self> {
        let discount_fn = rebased_discount_fn(discount_curve, as_of)?;
        Ok(Self {
            kappa: hw_params.kappa,
            sigma: hw_params.sigma,
            discount_fn: Box::new(discount_fn),
        })
    }

    /// Compute the reconstruction coefficients for a coupon period that *fixes*
    /// at `fixing_t` (years from `as_of`) and *accrues* for `tau` years.
    ///
    /// The simple forward applies to the bond `P(fixing_t, fixing_t + tau)`.
    #[must_use]
    pub fn period_coeffs(&self, fixing_t: f64, tau: f64) -> PeriodForwardCoeffs {
        let t = fixing_t.max(0.0);
        let big_t = t + tau.max(0.0);
        let b = hw_b(self.kappa, t, big_t);
        let ln_a = hw_ln_a(self.kappa, self.sigma, t, big_t, self.discount_fn.as_ref());
        PeriodForwardCoeffs { b, ln_a, tau }
    }
}

/// `B(t₁, t₂) = (1 − e^{−κ(t₂−t₁)}) / κ`, with the κ→0 Taylor limit.
///
/// Mirrors `calibration::hull_white::hw_b`; duplicated (a few lines) to keep
/// that calibration helper private to its module.
#[inline]
fn hw_b(kappa: f64, t1: f64, t2: f64) -> f64 {
    let tau = t2 - t1;
    if kappa.abs() < 1e-10 {
        tau
    } else {
        (1.0 - (-kappa * tau).exp()) / kappa
    }
}

/// `ln A(t, T)` for the HW1F affine zero-coupon bond price.
///
/// ```text
/// ln A(t,T) = ln(P(0,T)/P(0,t)) + B(t,T)·f(0,t) − (σ²/4κ)·(1−e^{−2κt})·B(t,T)²
/// ```
///
/// `f(0,t) = −∂/∂t ln P(0,t)` is the market instantaneous forward, taken by
/// central finite difference — the same approximation
/// `calibrate_theta_from_curve` uses for the θ(t) bootstrap, so the
/// reconstructed bond is consistent with the simulated dynamics.
fn hw_ln_a(kappa: f64, sigma: f64, t: f64, big_t: f64, df: &dyn Fn(f64) -> f64) -> f64 {
    let p0t = df(t);
    let p0_big_t = df(big_t);
    let b = hw_b(kappa, t, big_t);
    let f0t = fd_instantaneous_forward(df, t);

    let var_term = if kappa.abs() < 1e-10 {
        sigma * sigma * t * b * b / 2.0
    } else {
        sigma * sigma / (4.0 * kappa) * (1.0 - (-2.0 * kappa * t).exp()) * b * b
    };

    // P(0,t) / P(0,T) are both positive on a well-formed curve; the rebased
    // closure already floors degenerate extrapolation to 0.0, guarded here.
    if p0t > 0.0 && p0_big_t > 0.0 {
        (p0_big_t / p0t).ln() + b * f0t - var_term
    } else {
        // Degenerate curve: ln A → 0 ⇒ reconstruction collapses to the
        // driftless P = exp(−B·r). Better than a non-finite forward.
        b * f0t - var_term
    }
}

/// Instantaneous forward `f(0,t) = −d/dt ln P(0,t)` by central finite difference.
#[inline]
fn fd_instantaneous_forward(df: &dyn Fn(f64) -> f64, t: f64) -> f64 {
    let h = (t * 1e-3).clamp(1e-6, 1e-3);
    if t > h {
        let dfp = df(t + h);
        let dfm = df(t - h);
        if dfp > 0.0 && dfm > 0.0 {
            -(dfp.ln() - dfm.ln()) / (2.0 * h)
        } else {
            0.0
        }
    } else {
        // Near t = 0: one-sided forward difference against P(0,0) = 1.
        let dfh = df(h);
        if dfh > 0.0 {
            -dfh.ln() / h
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::dates::DayCount;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use time::Month;

    fn date(y: i32, m: Month, d: u8) -> Date {
        Date::from_calendar_date(y, m, d).expect("valid date")
    }

    /// Flat-curve discount factor closure for analytical cross-checks.
    fn flat_curve(as_of: Date, rate: f64) -> DiscountCurve {
        DiscountCurve::builder("FLAT")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([
                (0.0, 1.0),
                (1.0, (-rate).exp()),
                (5.0, (-rate * 5.0).exp()),
                (10.0, (-rate * 10.0).exp()),
            ])
            .build()
            .expect("flat discount curve")
    }

    #[test]
    fn rebased_discount_fn_is_one_at_zero() {
        let as_of = date(2025, Month::January, 1);
        let curve = flat_curve(as_of, 0.03);
        let f = rebased_discount_fn(&curve, as_of).expect("rebased");
        // P(as_of, as_of) = 1.
        assert!((f(0.0) - 1.0).abs() < 1e-12);
        // P(as_of, as_of + 1y) ≈ e^{-0.03}.
        assert!((f(1.0) - (-0.03_f64).exp()).abs() < 1e-6);
    }

    #[test]
    fn calibrate_hw1f_params_grid_covers_horizon() {
        let as_of = date(2025, Month::January, 1);
        let curve = flat_curve(as_of, 0.03);
        let hw = HullWhiteParams::new(0.15, 0.01).expect("hw");
        let horizon = 2.6_f64;
        let params = calibrate_hw1f_params(hw, &curve, as_of, horizon).expect("calibrated");

        // Monthly grid: ceil(2.6 · 12) = 32 intervals, one θ knot per interval.
        let n_steps = (horizon / THETA_GRID_SPACING_YEARS).ceil() as usize;
        assert_eq!(params.theta_times.len(), 32);
        assert_eq!(params.theta_curve.len(), params.theta_times.len());

        // Knot times are the interval *left boundaries* (`theta_at_time` is
        // left-continuous); the last interval covers `[last_boundary, ∞)`, so
        // the horizon — and every event before it — lands in a defined θ knot.
        assert!((params.theta_times[0] - 0.0).abs() < 1e-12);
        assert!(n_steps as f64 * THETA_GRID_SPACING_YEARS >= horizon);
        let last_boundary = *params.theta_times.last().expect("last");
        assert!(last_boundary <= horizon && last_boundary + THETA_GRID_SPACING_YEARS >= horizon);
        // θ(t) at the horizon resolves to the final knot, not an extrapolation.
        let theta_h = params.theta_at_time(horizon);
        assert!((theta_h - *params.theta_curve.last().expect("θ")).abs() < 1e-12);
    }

    /// On a *flat* curve the HW1F term forward must equal the curve's own
    /// simple forward (≈ the continuously-compounded flat rate, up to the
    /// simple-vs-continuous and HW1F-convexity corrections), when evaluated at
    /// the short rate equal to that flat rate.
    #[test]
    fn term_forward_on_flat_curve_matches_flat_rate() {
        let as_of = date(2025, Month::January, 1);
        let rate = 0.03_f64;
        let curve = flat_curve(as_of, rate);
        let hw = HullWhiteParams::new(0.15, 0.01).expect("hw");
        let recon = Hw1fTermForward::new(hw, &curve, as_of).expect("recon");

        // 6-month period fixing at t = 1y. On a flat curve r(t) ≡ rate keeps
        // the bond at its forward value; the simple forward over [1, 1.5] is
        // (e^{rate·0.5} − 1)/0.5 ≈ 3.02%.
        let tau = 0.5_f64;
        let coeffs = recon.period_coeffs(1.0, tau);
        let fwd = coeffs.simple_forward(rate);
        let expected_simple = ((rate * tau).exp() - 1.0) / tau;
        assert!(
            (fwd - expected_simple).abs() < 5e-4,
            "flat-curve term forward {fwd:.6} should match simple forward {expected_simple:.6}"
        );
    }

    #[test]
    fn term_forward_zero_tau_is_zero() {
        let as_of = date(2025, Month::January, 1);
        let curve = flat_curve(as_of, 0.03);
        let hw = HullWhiteParams::new(0.15, 0.01).expect("hw");
        let recon = Hw1fTermForward::new(hw, &curve, as_of).expect("recon");
        let coeffs = recon.period_coeffs(1.0, 0.0);
        assert!((coeffs.simple_forward(0.05)).abs() < 1e-12);
    }

    /// The reconstructed forward rises with the short rate (B(t,T) > 0 ⇒
    /// 1/P is monotone increasing in r).
    #[test]
    fn term_forward_is_increasing_in_short_rate() {
        let as_of = date(2025, Month::January, 1);
        let curve = flat_curve(as_of, 0.03);
        let hw = HullWhiteParams::new(0.15, 0.01).expect("hw");
        let recon = Hw1fTermForward::new(hw, &curve, as_of).expect("recon");
        let coeffs = recon.period_coeffs(1.0, 0.5);
        let lo = coeffs.simple_forward(0.01);
        let hi = coeffs.simple_forward(0.06);
        assert!(hi > lo, "forward must increase with r: lo={lo}, hi={hi}");
    }
}
