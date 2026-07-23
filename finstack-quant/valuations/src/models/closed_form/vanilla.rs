//! Black–Scholes/Garman–Kohlhagen vanilla option pricing and Greeks.
//!
//! This module provides closed-form pricing and Greeks for European vanilla options
//! using the Black-Scholes-Merton (equity) or Garman-Kohlhagen (FX) framework.
//!
//! # Features
//!
//! - **`bs_price`**: Computes the fair value of a European call or put
//! - **`bs_greeks`**: Computes all first-order Greeks (delta, gamma, vega, theta, rho_r, rho_q)
//! - **`BsGreeks`**: Struct holding per-unit Greeks with both domestic and foreign rho
//!
//! # Relationship to [`super::greeks`]
//!
//! [`super::greeks`] exposes the same Greeks as individual functions
//! (`bs_call_delta`, `bs_gamma`, `bs_vega`, etc.) with identical scaling
//! conventions (vega per 1%, rho per 1%). Those are useful when only a subset
//! of Greeks is needed; [`bs_greeks`] in this module computes all of them in
//! one pass and takes an explicit `theta_days_per_year` for day-count control.
//!
//! # Model
//!
//! The pricing formula uses continuous compounding with dividend yield (or foreign rate for FX):
//! ```text
//! Call = S·e^(-qT)·N(d₁) - K·e^(-rT)·N(d₂)
//! Put  = K·e^(-rT)·N(-d₂) - S·e^(-qT)·N(-d₁)
//! ```
//!
//! where:
//! - `r` is the domestic (risk-free) rate
//! - `q` is the dividend yield (or foreign rate for FX options)
//!
//! # Conventions
//!
//! | Parameter | Convention | Units |
//! |-----------|-----------|-------|
//! | Rates (r, q) | Continuously compounded | Decimal (0.05 = 5%) |
//! | Volatility (σ) | Annualized | Decimal (0.20 = 20%) |
//! | Time (T) | ACT/365-style | Years (1.0 = 1 year) |
//! | Prices | Per unit of underlying | Currency units |
//! | Greeks (vega, rho) | Per 1% move | Scaled by 0.01 |
//!
//! # References
//!
//! - Black, F., & Scholes, M. (1973). "The Pricing of Options and Corporate Liabilities."
//! - Garman, M. B., & Kohlhagen, S. W. (1983). "Foreign Currency Option Values."

use crate::instruments::common_impl::parameters::OptionType;
use crate::models::volatility::black::{d1_d2, d1_d2_black76};
use finstack_quant_core::{Error, Result};
use std::fmt;

/// Conversion constant for per-1% Greeks.
pub const ONE_PERCENT: f64 = 100.0;

/// Black–Scholes/Garman–Kohlhagen Greeks (per unit, not scaled by contract size).
///
/// This struct is suitable for both equity options (with dividend yield) and
/// FX options (with foreign rate), as it includes both `rho_r` (domestic) and
/// `rho_q` (foreign/dividend) sensitivities.
#[derive(Debug, Clone, Copy, Default)]
pub struct BsGreeks {
    /// Delta sensitivity per unit.
    pub delta: f64,
    /// Gamma sensitivity per unit.
    pub gamma: f64,
    /// Vega per 1% volatility move.
    pub vega: f64,
    /// Theta per day (scaled by provided day-count basis).
    pub theta: f64,
    /// Rho to the domestic/risk-free rate per 1%.
    pub rho_r: f64,
    /// Rho to the foreign/dividend yield per 1%.
    pub rho_q: f64,
}

impl fmt::Display for BsGreeks {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Δ={:.4} Γ={:.6} V={:.4} Θ={:.4} ρr={:.4} ρq={:.4}",
            self.delta, self.gamma, self.vega, self.theta, self.rho_r, self.rho_q
        )
    }
}

impl BsGreeks {
    /// Validate that Greeks are within their carry-independent bounds.
    ///
    /// Returns `true` if all Greeks satisfy the constraints that hold for
    /// EVERY carry regime:
    /// - Gamma: must be non-negative (≥ 0)
    /// - Vega: must be non-negative (≥ 0)
    /// - All values finite
    ///
    /// Delta is deliberately NOT bounded here: the true bound is
    /// `|Δ| ≤ e^{−qT}`, which exceeds 1 whenever the carry `q` is negative (a
    /// negative dividend yield, or foreign rate above domestic in the
    /// Garman-Kohlhagen reuse of this struct). This type does not know `q`
    /// and `T`, so a ±1 delta check would reject correct values — callers
    /// that want the carry-aware bound must apply `e^{−qT}` themselves.
    /// Theta and rhos have no strict sign constraints.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        // Gamma must be non-negative
        if self.gamma < 0.0 {
            return false;
        }
        // Vega must be non-negative
        if self.vega < 0.0 {
            return false;
        }
        // All values must be finite
        self.delta.is_finite()
            && self.gamma.is_finite()
            && self.vega.is_finite()
            && self.theta.is_finite()
            && self.rho_r.is_finite()
            && self.rho_q.is_finite()
    }

    /// Clamp Greeks to their carry-independent bounds.
    ///
    /// This corrects for minor numerical precision issues near boundaries:
    /// - Gamma: clamped to [0, ∞)
    /// - Vega: clamped to [0, ∞)
    ///
    /// Delta is NOT clamped: its true bound `|Δ| ≤ e^{−qT}` depends on the
    /// carry, which this type does not carry — a ±1 clamp would silently
    /// corrupt correct negative-carry deltas above 1 (see [`Self::is_valid`]).
    /// Theta and rhos are not clamped as they have no theoretical bounds.
    #[must_use]
    pub fn clamped(self) -> Self {
        Self {
            delta: self.delta,
            gamma: self.gamma.max(0.0),
            vega: self.vega.max(0.0),
            theta: self.theta,
            rho_r: self.rho_r,
            rho_q: self.rho_q,
        }
    }
}

/// Convert host-language call/put booleans into the canonical Rust option type.
///
/// # Arguments
///
/// * `is_call` - `true` for a call/payer payoff and `false` for a put/receiver
///   payoff when a host binding supplies the compact boolean convention.
#[must_use]
#[inline]
pub fn option_type_from_bool(is_call: bool) -> OptionType {
    if is_call {
        OptionType::Call
    } else {
        OptionType::Put
    }
}

/// Return a closed-form value when finite, otherwise report a validation error.
///
/// # Arguments
///
/// * `value` - Calculated formula output that must be finite before crossing a
///   checked API boundary.
/// * `what` - Human-readable formula or metric name included in a validation
///   error if `value` is `NaN` or infinite.
pub fn checked_closed_form_value(value: f64, what: &str) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(Error::Validation(format!(
            "{what} is not finite ({value}); check inputs (volatility, time \
             to expiry, spot, strike) are in the model's valid domain"
        )))
    }
}

/// Black–Scholes / Garman–Kohlhagen price (per unit, no contract scaling).
///
/// # Arguments
///
/// * `spot` - Current spot price S
/// * `strike` - Exercise price K in the same units as `spot`.
/// * `r` - Domestic (risk-free) rate, continuously compounded
/// * `q` - Dividend yield or foreign rate, continuously compounded
/// * `sigma` - Volatility σ (annualized)
/// * `t` - Time to expiration T (in years)
/// * `option_type` - Call or put payoff convention used for intrinsic value
///   and the Black-Scholes pricing formula.
///
/// # Returns
///
/// Option price per unit of the underlying. At expiration (t ≤ 0), returns intrinsic value.
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_valuations::models::closed_form::vanilla::bs_price;
/// use finstack_quant_valuations::instruments::OptionType;
///
/// let price = bs_price(100.0, 100.0, 0.05, 0.02, 0.20, 1.0, OptionType::Call);
/// assert!(price > 0.0);
/// ```
#[must_use]
#[inline]
#[allow(clippy::too_many_arguments)]
pub fn bs_price(
    spot: f64,
    strike: f64,
    r: f64,
    q: f64,
    sigma: f64,
    t: f64,
    option_type: OptionType,
) -> f64 {
    if t <= 0.0 {
        return match option_type {
            OptionType::Call => (spot - strike).max(0.0),
            OptionType::Put => (strike - spot).max(0.0),
        };
    }

    // Use combined d1_d2 to avoid redundant computation
    let (d1, d2) = d1_d2(spot, strike, r, sigma, t, q);

    // Compute CDFs - use symmetry N(-x) = 1 - N(x) to reduce calls
    let cdf_d1 = finstack_quant_core::math::norm_cdf(d1);
    let cdf_d2 = finstack_quant_core::math::norm_cdf(d2);

    let exp_q_t = (-q * t).exp();
    let exp_r_t = (-r * t).exp();

    let raw_price = match option_type {
        OptionType::Call => spot * exp_q_t * cdf_d1 - strike * exp_r_t * cdf_d2,
        OptionType::Put => {
            // Use symmetry: N(-x) = 1 - N(x)
            let cdf_m_d1 = 1.0 - cdf_d1;
            let cdf_m_d2 = 1.0 - cdf_d2;
            strike * exp_r_t * cdf_m_d2 - spot * exp_q_t * cdf_m_d1
        }
    };

    // Numerical cancellation can produce tiny negative values for deep OTM options.
    raw_price.max(0.0)
}

/// Checked Black–Scholes / Garman–Kohlhagen price for host-language bindings.
///
/// The raw [`bs_price`] primitive remains an infallible formula for Rust call
/// sites that intentionally handle `NaN` / infinity. Bindings should use this
/// checked wrapper so invalid inputs cross the host boundary as errors.
///
/// # Arguments
///
/// * `spot` - Current underlying spot price in the option's price units.
/// * `strike` - Exercise price in the same units as `spot`.
/// * `r` - Continuously compounded domestic risk-free rate as a decimal.
/// * `q` - Continuously compounded dividend yield or foreign-rate carry as a
///   decimal.
/// * `sigma` - Annualized lognormal volatility as a decimal.
/// * `t` - Remaining time to expiry in years.
/// * `option_type` - Call or put payoff convention for the returned price.
#[allow(clippy::too_many_arguments)]
pub fn bs_price_checked(
    spot: f64,
    strike: f64,
    r: f64,
    q: f64,
    sigma: f64,
    t: f64,
    option_type: OptionType,
) -> Result<f64> {
    checked_closed_form_value(
        bs_price(spot, strike, r, q, sigma, t, option_type),
        "Black-Scholes price",
    )
}

/// Black–Scholes / Garman–Kohlhagen Greeks (per unit, per-1% for vega and rhos).
///
/// Computes all first-order sensitivities for European vanilla options.
///
/// # Arguments
///
/// * `spot` - Current spot price S
/// * `strike` - Exercise price K in the same units as the underlying spot.
/// * `r` - Domestic (risk-free) rate, continuously compounded
/// * `q` - Dividend yield or foreign rate, continuously compounded
/// * `sigma` - Volatility σ (annualized)
/// * `t` - Time to expiration T (in years)
/// * `option_type` - Call or put payoff convention for the reported Greeks.
/// * `theta_days_per_year` - Day-count basis for theta conversion (see below)
///
/// # Returns
///
/// [`BsGreeks`] struct with:
/// - `delta`: ∂V/∂S (per unit)
/// - `gamma`: ∂²V/∂S² (per unit)
/// - `vega`: ∂V/∂σ per 1% vol change
/// - `theta`: ∂V/∂t per day (using specified day-count basis)
/// - `rho_r`: ∂V/∂r per 1% domestic rate change
/// - `rho_q`: ∂V/∂q per 1% foreign/dividend rate change
///
/// # Theta Day-Count Conventions
///
/// The `theta_days_per_year` parameter converts annualized theta to per-day theta.
/// Choose based on your market convention:
///
/// | Convention | Value | Use Case |
/// |------------|-------|----------|
/// | ACT/365 | 365.0 | UK Gilts, GBP options, equity options (US) |
/// | ACT/365.25 | 365.25 | Leap year average, some academic models |
/// | ACT/360 | 360.0 | Money market, most FX, EUR rates |
/// | 30/360 | 360.0 | US corporate bonds, some swaps |
/// | Business days | 252.0 | Trading days only (equity risk systems) |
///
/// **Common choices:**
/// - Equity options: Use 365.0 (calendar days)
/// - FX options: Use 365.0 or 360.0 depending on currency pair
/// - IR options: Match the underlying swap's day count
/// - Risk systems: Often use 252.0 (trading days) for consistency
///
/// # Theta Sign Convention
///
/// Theta is typically **negative** for long options (time decay hurts).
/// The returned value represents the daily P&L impact:
/// - Negative theta: option loses value as time passes
/// - Positive theta: option gains value (rare, e.g., deep ITM puts with high rates)
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_valuations::models::closed_form::vanilla::{bs_greeks, BsGreeks};
/// use finstack_quant_valuations::instruments::OptionType;
///
/// let greeks = bs_greeks(100.0, 100.0, 0.05, 0.02, 0.20, 1.0, OptionType::Call, 365.0);
/// assert!(greeks.delta > 0.0 && greeks.delta < 1.0);
/// assert!(greeks.gamma > 0.0);
/// assert!(greeks.vega > 0.0);
/// ```
#[must_use]
#[inline]
#[allow(clippy::too_many_arguments)]
pub fn bs_greeks(
    spot: f64,
    strike: f64,
    r: f64,
    q: f64,
    sigma: f64,
    t: f64,
    option_type: OptionType,
    theta_days_per_year: f64,
) -> BsGreeks {
    // Theta divides by this basis; a non-positive value would yield inf/NaN.
    // All in-tree callers pass 252/365; a hard assert keeps a degenerate
    // caller from silently producing inf theta in release builds. Bindings
    // validate user-supplied values and return a proper error before reaching
    // this point.
    assert!(
        theta_days_per_year > 0.0,
        "theta_days_per_year must be positive, got {theta_days_per_year}"
    );

    // Use combined d1_d2 to compute both values in one pass (avoids duplicate ln/sqrt)
    let (d1, d2) = d1_d2(spot, strike, r, sigma, t, q);

    // Pre-compute shared exponentials
    let exp_q_t = (-q * t).exp();
    let exp_r_t = (-r * t).exp();
    let sqrt_t = t.sqrt();

    // PDF is always needed for gamma/vega/theta
    let pdf_d1 = finstack_quant_core::math::norm_pdf(d1);

    // Compute CDFs only twice - use symmetry N(-x) = 1 - N(x) for the complements
    let cdf_d1 = finstack_quant_core::math::norm_cdf(d1);
    let cdf_d2 = finstack_quant_core::math::norm_cdf(d2);
    let cdf_m_d1 = 1.0 - cdf_d1; // N(-d1) = 1 - N(d1)
    let cdf_m_d2 = 1.0 - cdf_d2; // N(-d2) = 1 - N(d2)

    let delta = match option_type {
        OptionType::Call => exp_q_t * cdf_d1,
        OptionType::Put => -exp_q_t * cdf_m_d1,
    };

    // Gamma is the same for calls and puts
    let gamma = if spot <= 0.0 || sigma <= 0.0 || sqrt_t <= 0.0 {
        0.0
    } else {
        exp_q_t * pdf_d1 / (spot * sigma * sqrt_t)
    };

    // Vega is the same for calls and puts (per 1% vol)
    let vega = spot * exp_q_t * pdf_d1 * sqrt_t / ONE_PERCENT;

    // Theta differs by option type
    // Common term for both: -S * φ(d1) * σ * e^(-qT) / (2√T)
    let theta_common = if sqrt_t > 0.0 {
        -spot * pdf_d1 * sigma * exp_q_t / (2.0 * sqrt_t)
    } else {
        0.0
    };

    let theta = match option_type {
        OptionType::Call => {
            let term2 = q * spot * cdf_d1 * exp_q_t;
            let term3 = -r * strike * exp_r_t * cdf_d2;
            (theta_common + term2 + term3) / theta_days_per_year
        }
        OptionType::Put => {
            let term2 = -q * spot * cdf_m_d1 * exp_q_t;
            let term3 = r * strike * exp_r_t * cdf_m_d2;
            (theta_common + term2 + term3) / theta_days_per_year
        }
    };

    let rho_r = match option_type {
        OptionType::Call => strike * t * exp_r_t * cdf_d2 / ONE_PERCENT,
        OptionType::Put => -strike * t * exp_r_t * cdf_m_d2 / ONE_PERCENT,
    };

    let rho_q = match option_type {
        OptionType::Call => -spot * t * exp_q_t * cdf_d1 / ONE_PERCENT,
        OptionType::Put => spot * t * exp_q_t * cdf_m_d1 / ONE_PERCENT,
    };

    BsGreeks {
        delta,
        gamma,
        vega,
        theta,
        rho_r,
        rho_q,
    }
}

/// Checked Black–Scholes / Garman–Kohlhagen Greeks for host boundaries.
///
/// Unlike the raw formula, this rejects non-finite or economically invalid
/// inputs and verifies every returned sensitivity is finite.
///
/// # Arguments
///
/// * `spot` - Current underlying spot price in the option's price units.
/// * `strike` - Exercise price in the same units as `spot`.
/// * `r` - Continuously compounded domestic risk-free rate as a decimal.
/// * `q` - Continuously compounded dividend yield or foreign-rate carry as a
///   decimal.
/// * `sigma` - Positive annualized lognormal volatility as a decimal.
/// * `t` - Positive remaining time to expiry in years.
/// * `option_type` - Call or put payoff convention for the reported Greeks.
/// * `theta_days_per_year` - Positive calendar or trading-day basis used to
///   convert annual theta into the returned per-day amount.
#[allow(clippy::too_many_arguments)]
pub fn bs_greeks_checked(
    spot: f64,
    strike: f64,
    r: f64,
    q: f64,
    sigma: f64,
    t: f64,
    option_type: OptionType,
    theta_days_per_year: f64,
) -> Result<BsGreeks> {
    for (name, value) in [
        ("spot", spot),
        ("strike", strike),
        ("r", r),
        ("q", q),
        ("sigma", sigma),
        ("t", t),
        ("theta_days_per_year", theta_days_per_year),
    ] {
        if !value.is_finite() {
            return Err(Error::Validation(format!(
                "Black-Scholes Greeks input '{name}' must be finite, got {value}"
            )));
        }
    }
    if spot <= 0.0 || strike <= 0.0 {
        return Err(Error::Validation(format!(
            "Black-Scholes Greeks require positive spot and strike, got spot={spot}, strike={strike}"
        )));
    }
    if sigma <= 0.0 || t <= 0.0 || theta_days_per_year <= 0.0 {
        return Err(Error::Validation(format!(
            "Black-Scholes Greeks require positive sigma, t, and theta_days_per_year, got sigma={sigma}, t={t}, theta_days_per_year={theta_days_per_year}"
        )));
    }

    let greeks = bs_greeks(
        spot,
        strike,
        r,
        q,
        sigma,
        t,
        option_type,
        theta_days_per_year,
    );
    for (name, value) in [
        ("delta", greeks.delta),
        ("gamma", greeks.gamma),
        ("vega", greeks.vega),
        ("theta", greeks.theta),
        ("rho_r", greeks.rho_r),
        ("rho_q", greeks.rho_q),
    ] {
        if !value.is_finite() {
            return Err(Error::Validation(format!(
                "Black-Scholes Greek '{name}' is non-finite for the supplied inputs"
            )));
        }
    }
    Ok(greeks)
}

/// Black-76 undiscounted call price (option on a forward).
///
/// Returns `(forward - strike).max(0.0)` for degenerate inputs
/// (`t <= 0`, `sigma <= 0`, `forward <= 0`, or `strike <= 0`).
///
/// # Arguments
///
/// * `forward` - Forward price or rate F at option expiry.
/// * `strike`  - Exercise price or rate K in the same units as `forward`.
/// * `sigma`   - Implied volatility σ (annualized)
/// * `t`       - Time to expiration T (in years)
///
/// # Returns
///
/// Undiscounted call price: `F·N(d1) - K·N(d2)`.
/// Multiply by a discount factor at the call site to obtain a present value.
#[must_use]
#[inline]
pub fn black76_call(forward: f64, strike: f64, sigma: f64, t: f64) -> f64 {
    if t <= 0.0 || sigma <= 0.0 || forward <= 0.0 || strike <= 0.0 {
        return (forward - strike).max(0.0);
    }
    let (d1, d2) = d1_d2_black76(forward, strike, sigma, t);
    forward * finstack_quant_core::math::norm_cdf(d1)
        - strike * finstack_quant_core::math::norm_cdf(d2)
}

/// Black-76 undiscounted put price (option on a forward).
///
/// Returns `(strike - forward).max(0.0)` for degenerate inputs
/// (`t <= 0`, `sigma <= 0`, `forward <= 0`, or `strike <= 0`).
///
/// # Arguments
///
/// * `forward` - Forward price or rate F at option expiry.
/// * `strike`  - Exercise price or rate K in the same units as `forward`.
/// * `sigma`   - Implied volatility σ (annualized)
/// * `t`       - Time to expiration T (in years)
///
/// # Returns
///
/// Undiscounted put price: `K·N(-d2) - F·N(-d1)`.
/// Multiply by a discount factor at the call site to obtain a present value.
#[must_use]
#[inline]
pub fn black76_put(forward: f64, strike: f64, sigma: f64, t: f64) -> f64 {
    if t <= 0.0 || sigma <= 0.0 || forward <= 0.0 || strike <= 0.0 {
        return (strike - forward).max(0.0);
    }
    let (d1, d2) = d1_d2_black76(forward, strike, sigma, t);
    strike * finstack_quant_core::math::norm_cdf(-d2)
        - forward * finstack_quant_core::math::norm_cdf(-d1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bs_price_call_atm() {
        let price = bs_price(100.0, 100.0, 0.05, 0.02, 0.20, 1.0, OptionType::Call);
        // ATM call with these params should be around 9-10
        assert!(price > 8.0 && price < 12.0, "price = {}", price);
    }

    #[test]
    fn option_type_from_bool_maps_binding_flags() {
        assert!(matches!(option_type_from_bool(true), OptionType::Call));
        assert!(matches!(option_type_from_bool(false), OptionType::Put));
    }

    #[test]
    fn checked_closed_form_value_rejects_non_finite_result() {
        let err = checked_closed_form_value(f64::NAN, "Black-Scholes price")
            .expect_err("non-finite closed-form values must error");
        let message = err.to_string();
        assert!(
            message.contains("Black-Scholes price is not finite"),
            "unexpected error: {message}"
        );
    }

    #[test]
    fn test_bs_price_put_atm() {
        let price = bs_price(100.0, 100.0, 0.05, 0.02, 0.20, 1.0, OptionType::Put);
        // Put-call parity check
        let call = bs_price(100.0, 100.0, 0.05, 0.02, 0.20, 1.0, OptionType::Call);
        let parity = call - price - 100.0 * (-0.02_f64).exp() + 100.0 * (-0.05_f64).exp();
        assert!(parity.abs() < 1e-10, "Put-call parity violated: {}", parity);
    }

    #[test]
    fn test_bs_price_expired() {
        // ITM call at expiration
        assert!(
            (bs_price(110.0, 100.0, 0.05, 0.0, 0.2, 0.0, OptionType::Call) - 10.0).abs() < 1e-10
        );
        // OTM call at expiration
        assert!(bs_price(90.0, 100.0, 0.05, 0.0, 0.2, 0.0, OptionType::Call).abs() < 1e-10);
        // ITM put at expiration
        assert!((bs_price(90.0, 100.0, 0.05, 0.0, 0.2, 0.0, OptionType::Put) - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_bs_price_put_is_non_negative_for_deep_otm_case() {
        let price = bs_price(
            141.855_852_889_058_4,
            58.709_489_081_432_6,
            0.0,
            0.0,
            0.367_806_872_430_263_44,
            31.0 / 365.0,
            OptionType::Put,
        );
        assert!(price >= 0.0, "deep OTM put price = {}", price);
    }

    #[test]
    fn test_bs_greeks_call() {
        let greeks = bs_greeks(100.0, 100.0, 0.05, 0.02, 0.20, 1.0, OptionType::Call, 365.0);
        // ATM call delta should be around 0.5-0.6
        assert!(
            greeks.delta > 0.4 && greeks.delta < 0.7,
            "delta = {}",
            greeks.delta
        );
        // Gamma always positive
        assert!(greeks.gamma > 0.0);
        // Vega always positive
        assert!(greeks.vega > 0.0);
    }

    #[test]
    fn test_bs_greeks_put() {
        let greeks = bs_greeks(100.0, 100.0, 0.05, 0.02, 0.20, 1.0, OptionType::Put, 365.0);
        // ATM put delta should be negative, around -0.4 to -0.5
        assert!(
            greeks.delta < 0.0 && greeks.delta > -0.7,
            "delta = {}",
            greeks.delta
        );
        // Gamma same for calls and puts
        let call_greeks = bs_greeks(100.0, 100.0, 0.05, 0.02, 0.20, 1.0, OptionType::Call, 365.0);
        assert!((greeks.gamma - call_greeks.gamma).abs() < 1e-10);
    }

    #[test]
    fn test_bs_greeks_display() {
        let greeks = bs_greeks(100.0, 100.0, 0.05, 0.02, 0.20, 1.0, OptionType::Call, 365.0);
        let s = format!("{}", greeks);
        assert!(s.contains("Δ="));
        assert!(s.contains("Γ="));
        assert!(s.contains("V="));
    }

    #[test]
    fn test_bs_greeks_is_valid() {
        // Normal ATM call should be valid
        let greeks = bs_greeks(100.0, 100.0, 0.05, 0.02, 0.20, 1.0, OptionType::Call, 365.0);
        assert!(greeks.is_valid(), "ATM call Greeks should be valid");

        // Normal ATM put should be valid
        let put_greeks = bs_greeks(100.0, 100.0, 0.05, 0.02, 0.20, 1.0, OptionType::Put, 365.0);
        assert!(put_greeks.is_valid(), "ATM put Greeks should be valid");

        // Deep ITM call should still be valid
        let deep_itm = bs_greeks(
            200.0,
            100.0,
            0.05,
            0.02,
            0.20,
            0.01,
            OptionType::Call,
            365.0,
        );
        assert!(deep_itm.is_valid(), "Deep ITM call Greeks should be valid");

        // Deep OTM put should still be valid
        let deep_otm = bs_greeks(200.0, 100.0, 0.05, 0.02, 0.20, 0.01, OptionType::Put, 365.0);
        assert!(deep_otm.is_valid(), "Deep OTM put Greeks should be valid");
    }

    #[test]
    fn test_bs_greeks_clamped() {
        // Gamma/vega noise is floored; delta is untouched (its true bound
        // depends on the carry, which the struct does not know).
        let greeks = BsGreeks {
            delta: 1.0000001,  // Legitimate under negative carry; left as-is
            gamma: -0.0000001, // Slightly negative
            vega: -0.0000001,  // Slightly negative
            theta: -0.05,
            rho_r: 0.5,
            rho_q: -0.3,
        };

        let clamped = greeks.clamped();
        assert_eq!(clamped.delta, 1.0000001); // Unchanged
        assert_eq!(clamped.gamma, 0.0);
        assert_eq!(clamped.vega, 0.0);
        assert_eq!(clamped.theta, -0.05); // Unchanged
        assert_eq!(clamped.rho_r, 0.5); // Unchanged
        assert_eq!(clamped.rho_q, -0.3); // Unchanged
        assert!(clamped.is_valid());
    }

    /// Independent finite-difference cross-check of every analytic Greek
    /// against `bs_price`. The price formula and the Greek formulas are
    /// separate derivations, so central differences of the price are a
    /// non-circular oracle — a sign flip or scaling error (×100, missing √T)
    /// in any Greek fails here even though put-call parity would not notice.
    #[test]
    fn analytic_greeks_match_finite_differences_of_price() {
        let (s, k, r, q, sigma, t) = (100.0, 105.0, 0.03, 0.02, 0.25, 0.75);
        for option_type in [OptionType::Call, OptionType::Put] {
            let p = |s: f64, r: f64, q: f64, sigma: f64, t: f64| {
                bs_price(s, k, r, q, sigma, t, option_type)
            };
            let g = bs_greeks(s, k, r, q, sigma, t, option_type, 365.0);

            // Delta: ∂V/∂S
            let hs = 1e-4 * s;
            let fd_delta = (p(s + hs, r, q, sigma, t) - p(s - hs, r, q, sigma, t)) / (2.0 * hs);
            assert!(
                (g.delta - fd_delta).abs() < 1e-7,
                "{option_type:?} delta {} vs FD {fd_delta}",
                g.delta
            );

            // Gamma: ∂²V/∂S² (0.1%-of-spot bump: truncation ~1e-8, roundoff ~1e-13)
            let hg = 1e-3 * s;
            let fd_gamma = (p(s + hg, r, q, sigma, t) - 2.0 * p(s, r, q, sigma, t)
                + p(s - hg, r, q, sigma, t))
                / (hg * hg);
            assert!(
                (g.gamma - fd_gamma).abs() < 1e-6,
                "{option_type:?} gamma {} vs FD {fd_gamma}",
                g.gamma
            );

            // Vega: ∂V/∂σ, reported per 1% vol.
            let hv = 1e-5;
            let fd_vega =
                (p(s, r, q, sigma + hv, t) - p(s, r, q, sigma - hv, t)) / (2.0 * hv) * 0.01;
            assert!(
                (g.vega - fd_vega).abs() < 1e-7,
                "{option_type:?} vega {} vs FD {fd_vega}",
                g.vega
            );

            // Theta: ∂V/∂calendar-time = −∂V/∂T; reported per day (365 basis).
            let ht = 1e-6;
            let fd_theta_annual =
                -(p(s, r, q, sigma, t + ht) - p(s, r, q, sigma, t - ht)) / (2.0 * ht);
            assert!(
                (g.theta - fd_theta_annual / 365.0).abs() < 1e-9,
                "{option_type:?} theta {} vs FD {}",
                g.theta,
                fd_theta_annual / 365.0
            );

            // Rho_r and rho_q, reported per 1% rate move.
            let hr = 1e-6;
            let fd_rho_r =
                (p(s, r + hr, q, sigma, t) - p(s, r - hr, q, sigma, t)) / (2.0 * hr) * 0.01;
            assert!(
                (g.rho_r - fd_rho_r).abs() < 1e-8,
                "{option_type:?} rho_r {} vs FD {fd_rho_r}",
                g.rho_r
            );
            let fd_rho_q =
                (p(s, r, q + hr, sigma, t) - p(s, r, q - hr, sigma, t)) / (2.0 * hr) * 0.01;
            assert!(
                (g.rho_q - fd_rho_q).abs() < 1e-8,
                "{option_type:?} rho_q {} vs FD {fd_rho_q}",
                g.rho_q
            );
        }
    }

    /// Literal textbook anchor: Hull, *Options, Futures, and Other
    /// Derivatives* (10th ed., Ch. 19 worked examples) — S=49, K=50, r=5%,
    /// σ=20%, T=0.3846, q=0. Hull reports (rounded): Δ=0.522, Γ=0.066,
    /// vega=12.1 (per 100% vol), Θ=−4.31/yr, ρ=8.91 (per 100% rate).
    /// Tolerances cover Hull's rounding only — a scaling or sign error is far
    /// outside them.
    #[test]
    fn hull_chapter19_worked_example_anchor() {
        let g = bs_greeks(49.0, 50.0, 0.05, 0.0, 0.20, 0.3846, OptionType::Call, 365.0);

        assert!((g.delta - 0.522).abs() < 0.001, "delta {}", g.delta);
        assert!((g.gamma - 0.066).abs() < 0.001, "gamma {}", g.gamma);
        // vega is per 1% here; Hull's 12.1 is per 100%.
        assert!((g.vega * 100.0 - 12.1).abs() < 0.05, "vega {}", g.vega);
        // theta is per day (365 basis); Hull's −4.31 is per year.
        assert!(
            (g.theta * 365.0 - (-4.31)).abs() < 0.01,
            "theta/yr {}",
            g.theta * 365.0
        );
        // rho_r is per 1%; Hull's 8.91 is per 100%.
        assert!((g.rho_r * 100.0 - 8.91).abs() < 0.05, "rho {}", g.rho_r);
    }

    /// Under NEGATIVE carry (q < 0 — negative dividend yield, or the foreign
    /// rate above domestic in the Garman-Kohlhagen reuse of this struct), the
    /// call delta `e^{−qT}·N(d1)` legitimately exceeds 1. `clamped()` must not
    /// corrupt such a delta to 1.0, and `is_valid()` must not reject it.
    #[test]
    fn negative_carry_delta_above_one_is_neither_clamped_nor_invalid() {
        // Deep ITM call, q = −5%, T = 2y: delta = e^{0.1}·N(d1) ≈ 1.105·~1.
        let greeks = bs_greeks(
            200.0,
            100.0,
            0.03,
            -0.05,
            0.20,
            2.0,
            OptionType::Call,
            365.0,
        );
        assert!(
            greeks.delta > 1.0,
            "test premise: negative-carry deep-ITM call delta must exceed 1, got {}",
            greeks.delta
        );

        assert!(
            greeks.is_valid(),
            "a correct negative-carry delta > 1 must not be flagged invalid"
        );
        let clamped = greeks.clamped();
        assert_eq!(
            clamped.delta, greeks.delta,
            "clamped() must not corrupt a legitimate delta > 1 down to 1.0"
        );
    }

    #[test]
    fn test_bs_greeks_delta_bounds() {
        // Test that delta stays in [-1, 1] for extreme cases
        let cases = [
            // (spot, strike, option_type, expected_delta_sign)
            (1000.0, 100.0, OptionType::Call, 1), // Deep ITM call → delta ≈ 1
            (10.0, 100.0, OptionType::Call, 1),   // Deep OTM call → delta ≈ 0
            (1000.0, 100.0, OptionType::Put, -1), // Deep OTM put → delta ≈ 0
            (10.0, 100.0, OptionType::Put, -1),   // Deep ITM put → delta ≈ -1
        ];

        for (spot, strike, opt_type, expected_sign) in cases {
            let greeks = bs_greeks(spot, strike, 0.05, 0.02, 0.20, 1.0, opt_type, 365.0);
            assert!(
                greeks.is_valid(),
                "Greeks should be valid for spot={}, strike={}, type={:?}",
                spot,
                strike,
                opt_type
            );
            if expected_sign > 0 {
                assert!(greeks.delta >= 0.0);
            } else {
                assert!(greeks.delta <= 0.0);
            }
        }
    }
}
