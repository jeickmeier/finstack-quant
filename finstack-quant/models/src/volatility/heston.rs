//! Heston (1993) stochastic volatility model.
//!
//! Implements the Heston model for European option pricing and global
//! calibration to market-implied volatilities. Uses the Gil-Pelaez / P1-P2
//! Fourier inversion with the "Little Heston Trap" formulation from
//! Albrecher et al. (2007) for numerical stability.
//!
//! # Mathematical Foundation
//!
//! The Heston model describes the joint dynamics of an asset price and its
//! instantaneous variance:
//!
//! ```text
//! dS = (r - q) S dt + √v S dW₁
//! dv = κ(θ - v) dt + σ√v dW₂
//! E[dW₁ dW₂] = ρ dt
//!
//! where:
//!   S = asset price
//!   v = instantaneous variance
//!   κ = mean reversion speed of variance
//!   θ = long-run variance level
//!   σ = volatility of variance (vol-of-vol)
//!   ρ = correlation between asset and variance processes
//! ```
//!
//! # Parameters
//!
//! | Parameter | Symbol | Range | Market Role |
//! |-----------|--------|-------|-------------|
//! | v0 | v₀ | > 0 | Initial variance |
//! | kappa | κ | > 0 | Mean reversion speed |
//! | theta | θ | > 0 | Long-run variance |
//! | sigma_v | σ | > 0 | Vol-of-vol (smile curvature) |
//! | rho | ρ | (-1, 1) | Skew direction |
//!
//! # Feller Condition
//!
//! The condition 2κθ > σ² ensures the variance process remains strictly
//! positive. When violated, the process can hit zero, potentially causing
//! numerical instability. The constructor warns but does not reject.
//!
//! # References
//!
//! - Heston, S. L. (1993). "A Closed-Form Solution for Options with Stochastic
//!   Volatility with Applications to Bond and Currency Options."
//!   *Review of Financial Studies*, 6(2), 327-343. `docs/REFERENCES.md#heston-1993`
//!
//! - Albrecher, H., Mayer, P., Schoutens, W., & Tistaert, J. (2007).
//!   "The Little Heston Trap." *Wilmott Magazine*, January 2007. `docs/REFERENCES.md#albrecher-2007-little-heston-trap`
//! - Gatheral, J. (2006). *The Volatility Surface: A Practitioner's Guide*.
//!   Wiley Finance. `docs/REFERENCES.md#gatheral-volatility-surface`
//! - Kahl, C., & Jäckel, P. (2005). "Not-so-complex logarithms in the Heston
//!   model." *Wilmott Magazine*, September 2005. (Characteristic-function
//!   tail decay rate used for the quadrature truncation bound.)

use num_complex::Complex64;

const HESTON_G_DENOM_EPS: f64 = 1e-8;
const HESTON_EXPONENT_REAL_LIMIT: f64 = 700.0;

/// Heston stochastic volatility model parameters.
///
/// # Examples
///
/// ```rust,no_run
/// use finstack_quant_models::volatility::heston::HestonParams;
///
/// let params = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.5).unwrap();
/// assert!(params.satisfies_feller_condition());
///
/// let call = params.price_european(100.0, 100.0, 0.05, 0.0, 1.0, true);
/// assert!(call > 0.0 && call < 100.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "RawHestonParams")]
pub struct HestonParams {
    /// Initial variance (v₀ > 0).
    pub v0: f64,
    /// Mean reversion speed (κ > 0).
    pub kappa: f64,
    /// Long-run variance (θ > 0).
    pub theta: f64,
    /// Vol-of-vol (σ > 0).
    pub sigma_v: f64,
    /// Correlation between spot and variance (-1 < ρ < 1).
    pub rho: f64,
}

impl Default for HestonParams {
    fn default() -> Self {
        Self {
            v0: 0.04,
            kappa: 2.0,
            theta: 0.04,
            sigma_v: 0.3,
            rho: -0.5,
        }
    }
}

/// Raw deserialization state of [`HestonParams`].
///
/// Mirrors the current serialized field layout; conversion runs
/// [`HestonParams::new`] validation and rejects unknown fields.
#[derive(Debug, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct RawHestonParams {
    /// Initial variance.
    v0: f64,
    /// Mean reversion speed.
    kappa: f64,
    /// Long-run variance.
    theta: f64,
    /// Vol-of-vol.
    sigma_v: f64,
    /// Spot-variance correlation.
    rho: f64,
}

impl TryFrom<RawHestonParams> for HestonParams {
    type Error = finstack_quant_core::Error;

    fn try_from(raw: RawHestonParams) -> finstack_quant_core::Result<Self> {
        Self::new(raw.v0, raw.kappa, raw.theta, raw.sigma_v, raw.rho)
    }
}

impl HestonParams {
    /// Construct validated Heston parameters.
    ///
    /// # Arguments
    ///
    /// * `v0` - Positive initial variance level.
    /// * `kappa` - Positive annual mean-reversion speed of variance.
    /// * `theta` - Positive long-run variance level.
    /// * `sigma_v` - Positive annualized volatility of variance.
    /// * `rho` - Instantaneous spot-variance correlation in the open interval
    ///   `(-1, 1)`.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `v0 <= 0` or non-finite
    /// - `kappa <= 0` or non-finite
    /// - `theta <= 0` or non-finite
    /// - `sigma_v <= 0` or non-finite
    /// - `rho` not in `(-1, 1)` or non-finite
    ///
    /// # Feller Condition
    ///
    /// If 2κθ ≤ σ², a warning is emitted (but the parameters are still accepted).
    pub fn new(
        v0: f64,
        kappa: f64,
        theta: f64,
        sigma_v: f64,
        rho: f64,
    ) -> finstack_quant_core::Result<Self> {
        if v0 <= 0.0 || !v0.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Heston v0 (initial variance) must be positive, got {v0}"
            )));
        }
        if kappa <= 0.0 || !kappa.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Heston kappa (mean reversion) must be positive, got {kappa}"
            )));
        }
        if theta <= 0.0 || !theta.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Heston theta (long-run variance) must be positive, got {theta}"
            )));
        }
        if sigma_v <= 0.0 || !sigma_v.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Heston sigma_v (vol-of-vol) must be positive, got {sigma_v}"
            )));
        }
        if rho <= -1.0 || rho >= 1.0 || !rho.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Heston rho (correlation) must be in (-1, 1), got {rho}"
            )));
        }

        let params = Self {
            v0,
            kappa,
            theta,
            sigma_v,
            rho,
        };

        if !params.satisfies_feller_condition() {
            tracing::warn!(
                v0 = v0,
                kappa = kappa,
                theta = theta,
                sigma_v = sigma_v,
                rho = rho,
                feller_lhs = 2.0 * kappa * theta,
                feller_rhs = sigma_v * sigma_v,
                "Heston Feller condition violated (2*kappa*theta <= sigma_v^2). \
                 Variance process can reach zero. This is acceptable for Fourier pricing \
                 but may cause issues in Monte Carlo simulation.",
            );
        }

        Ok(params)
    }

    /// Check whether the inclusive Feller condition (2κθ ≥ σ²) is satisfied.
    ///
    /// At equality the CIR variance boundary remains unattainable. When the
    /// condition is violated, variance can reach zero, which causes numerical
    /// issues in Monte Carlo simulation (though Fourier pricing remains valid).
    #[must_use]
    pub fn satisfies_feller_condition(&self) -> bool {
        2.0 * self.kappa * self.theta >= self.sigma_v * self.sigma_v
    }

    /// Return deterministic average variance over `[0, t]`.
    ///
    /// This integrates the mean variance path
    /// `E[v_s] = theta + (v0 - theta) exp(-kappa s)`. A Taylor branch handles
    /// `kappa * t` near zero; non-positive `t` returns the initial variance.
    ///
    /// # Arguments
    ///
    /// * `t` - Time horizon in years.
    #[must_use]
    pub fn deterministic_avg_variance(&self, t: f64) -> f64 {
        if t <= 0.0 {
            return self.v0;
        }
        let kt = self.kappa * t;
        let decay_avg = if kt.abs() < 1e-6 {
            1.0 - 0.5 * kt + kt * kt / 6.0
        } else {
            (1.0 - (-kt).exp()) / kt
        };
        self.theta + (self.v0 - self.theta) * decay_avg
    }

    /// Return these parameters unchanged if the Feller condition holds,
    /// otherwise return an error.
    ///
    /// Use this when the parameters will be used downstream in Monte Carlo
    /// simulation where variance must stay strictly positive. Fourier-based
    /// pricing does not require the Feller condition.
    ///
    /// # Errors
    ///
    /// Returns an error if `2κθ ≤ σ²`.
    pub fn require_feller(self) -> finstack_quant_core::Result<Self> {
        if self.satisfies_feller_condition() {
            Ok(self)
        } else {
            Err(finstack_quant_core::Error::Validation(format!(
                "Heston Feller condition violated: 2*kappa*theta ({:.6}) <= sigma_v^2 ({:.6}). \
                 Variance process can reach zero, causing numerical issues in Monte Carlo. \
                 Use satisfies_feller_condition() to check, or omit require_feller() for \
                 Fourier-only pricing.",
                2.0 * self.kappa * self.theta,
                self.sigma_v * self.sigma_v,
            )))
        }
    }

    /// Price a European option using Fourier integration.
    ///
    /// Uses the Gil-Pelaez / P1-P2 formulation:
    /// ```text
    /// Call = S × exp(-qT) × P₁ - K × exp(-rT) × P₂
    /// Put  = Call - S × exp(-qT) + K × exp(-rT)   (put-call parity)
    /// ```
    ///
    /// where P₁ and P₂ are computed via numerical integration of the
    /// Heston characteristic function using composite Gauss-Legendre quadrature.
    ///
    /// # Arguments
    ///
    /// * `spot` - Current spot price
    /// * `strike` - Strike price
    /// * `r` - Risk-free rate (continuous compounding)
    /// * `q` - Dividend yield (continuous compounding)
    /// * `t` - Time to expiry in years
    /// * `is_call` - `true` for call, `false` for put
    ///
    /// # Returns
    ///
    /// Option price (non-negative), or `NaN` if inputs are invalid or numerical
    /// convergence cannot be established.
    #[must_use]
    pub fn price_european(
        &self,
        spot: f64,
        strike: f64,
        r: f64,
        q: f64,
        t: f64,
        is_call: bool,
    ) -> f64 {
        let params = crate::closed_form::heston::HestonPricingParams { r, q, model: *self };
        let result = if is_call {
            crate::closed_form::heston::heston_call_price_fourier(spot, strike, t, &params, None)
        } else {
            crate::closed_form::heston::heston_put_price_fourier(spot, strike, t, &params, None)
        };
        result.unwrap_or(f64::NAN)
    }

    /// Price a strip of European options sharing the same expiry and model inputs.
    ///
    /// Reuses the canonical checked Fourier driver and its strike-independent
    /// characteristic-function evaluations. Returns `NaN` in every entry if input
    /// validation or convergence fails.
    ///
    /// # Arguments
    ///
    /// * `spot` - Positive finite current underlying price in quote units.
    /// * `strikes` - Positive finite exercise prices, in result order and spot units.
    /// * `r` - Finite continuously compounded domestic rate, annual decimal.
    /// * `q` - Finite continuous dividend yield or foreign rate, annual decimal.
    /// * `t` - Finite time to expiry in years; non-positive values use expiry payoff.
    /// * `is_call` - Whether to price calls (`true`) or puts (`false`).
    #[must_use]
    pub fn price_european_strip(
        &self,
        spot: f64,
        strikes: &[f64],
        r: f64,
        q: f64,
        t: f64,
        is_call: bool,
    ) -> Vec<f64> {
        let params = crate::closed_form::heston::HestonPricingParams { r, q, model: *self };
        let result = if is_call {
            crate::closed_form::heston::heston_call_prices_fourier(spot, strikes, t, &params, None)
        } else {
            crate::closed_form::heston::heston_put_prices_fourier(spot, strikes, t, &params, None)
        };
        result.unwrap_or_else(|_| vec![f64::NAN; strikes.len()])
    }
}

/// Outcome of a single Heston characteristic-function evaluation.
///
/// Distinguishes the two ways ψ_j(φ) can legitimately come back as zero:
///
/// - [`HestonCfStatus::Overflow`] — an intermediate was non-finite or the
///   exponent guard tripped. The value is *corrupt*; callers that track
///   integration health should count the node.
/// - [`HestonCfStatus::Underflow`] — every intermediate was well-formed but
///   |ψ| underflowed to exactly zero deep in the decayed tail. Contributing
///   zero is the *correct* value there, so such nodes must not trip a
///   corruption fallback.
///
/// Conflating the two makes long-dated / high-κθ surfaces fall back to a
/// Black-Scholes price unnecessarily.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HestonCfStatus {
    /// Finite, non-zero value.
    Ok,
    /// Well-formed inputs; |ψ| underflowed to exactly zero (legitimate).
    Underflow,
    /// Non-finite intermediate or exponent guard hit; value zeroed (corrupt).
    Overflow,
}

/// Heston probability characteristic function ψ_j(φ) for j ∈ {1, 2}.
///
/// This is the single canonical implementation of the "Little Heston Trap"
/// formulation (Albrecher et al. 2007) for the workspace. It places −d in the
/// numerator of g and uses `exp(−dT)`, which avoids both the branch-cut
/// discontinuity and the `exp(+dT)` overflow of the original Heston (1993)
/// formulation.
///
/// Integration strategy is deliberately *not* part of this function: callers
/// supply their own quadrature over φ and differ in how they truncate.
///
/// # Arguments
///
/// * `j` — probability index (1 for the stock numeraire, 2 for money market)
/// * `phi` — Fourier variable
/// * `log_spot` — natural log of the spot price
/// * `r` — continuously compounded risk-free rate as a decimal (0.05 = 5%)
/// * `q` — continuously compounded dividend yield (or foreign rate for FX) as
///   a decimal (0.02 = 2%)
/// * `t` — time to maturity in years
/// * `params` — Heston parameters (v0, κ, θ, σ, ρ)
///
/// # Returns
///
/// `(ψ_j(φ), status)` — the value (zeroed on overflow/underflow) plus a
/// [`HestonCfStatus`] telling the caller whether a zero is legitimate.
///
/// # References
///
/// - Albrecher, H., Mayer, P., Schoutens, W., & Tistaert, J. (2007).
///   "The Little Heston Trap." *Wilmott Magazine*, January 2007. `docs/REFERENCES.md#albrecher-2007-little-heston-trap`
/// - Heston, S. L. (1993). "A Closed-Form Solution for Options with
///   Stochastic Volatility." *Review of Financial Studies*, 6(2), 327-343. `docs/REFERENCES.md#heston-1993`
#[must_use]
pub fn heston_pj_characteristic_function(
    j: u8,
    phi: f64,
    log_spot: f64,
    r: f64,
    q: f64,
    t: f64,
    params: &HestonParams,
) -> (Complex64, HestonCfStatus) {
    let kappa = params.kappa;
    let theta = params.theta;
    let sigma_v = params.sigma_v;
    let rho = params.rho;
    let v0 = params.v0;

    let i = Complex64::i();
    let one = Complex64::new(1.0, 0.0);
    let zero = Complex64::new(0.0, 0.0);

    if phi == 0.0 {
        return (one, HestonCfStatus::Ok);
    }

    // For P₁: u = 0.5, b = κ − ρσ (stock numeraire)
    // For P₂: u = −0.5, b = κ (money market numeraire)
    let (u_j, b_j) = if j == 1 {
        (0.5, kappa - rho * sigma_v)
    } else {
        (-0.5, kappa)
    };

    let a = kappa * theta;
    let sigma_sq = sigma_v * sigma_v;

    // d = sqrt((ρσiφ − b)² − σ²(2u_j iφ − φ²))
    let rsi_phi = Complex64::new(0.0, rho * sigma_v * phi);
    let b = Complex64::new(b_j, 0.0);
    let d_sq = (rsi_phi - b).powi(2) - sigma_sq * (Complex64::new(-phi * phi, 2.0 * u_j * phi));
    let d = d_sq.sqrt();

    // Little Heston Trap: g = (b − ρσiφ − d)/(b − ρσiφ + d)
    let bm = b - rsi_phi;
    let g_denom = bm + d;
    let g_denom_limit = HESTON_G_DENOM_EPS * (1.0 + bm.norm() + d.norm());
    if !g_denom.is_finite() || g_denom.norm() <= g_denom_limit {
        return (zero, HestonCfStatus::Overflow);
    }
    // Rationalize (bm-d)/sigma² before subtraction loses its significant
    // digits. This remains well conditioned as vol-of-vol tends to zero.
    let scaled_difference = Complex64::new(-phi * phi, 2.0 * u_j * phi) / g_denom;
    let g = sigma_sq * scaled_difference / g_denom;
    if !g.is_finite() {
        return (zero, HestonCfStatus::Overflow);
    }

    let exp_minus_dt = (-d * t).exp();
    if !exp_minus_dt.is_finite() {
        return (zero, HestonCfStatus::Overflow);
    }

    let one_minus_exp = if (d * t).norm() < 1e-4 {
        let z = -d * t;
        // -expm1(z), evaluated without cancellation for small complex z.
        -z * (one + z * (0.5 + z * (1.0 / 6.0 + z * (1.0 / 24.0 + z / 120.0))))
    } else {
        one - exp_minus_dt
    };
    // log((1-g exp(-dT))/(1-g))/sigma² = log1p(w)/w * w/sigma².
    // Computing w/sigma² directly preserves the deterministic limit even if
    // sigma² underflows. No Black-Scholes substitution is needed.
    let scaled_w = scaled_difference / g_denom * one_minus_exp / (one - g);
    let w = sigma_sq * scaled_w;
    let log1p_ratio = if w.norm() < 1e-4 {
        one + w * (-0.5 + w * (1.0 / 3.0 + w * (-0.25 + w / 5.0)))
    } else {
        (one + w).ln() / w
    };
    let c_val = i * phi * (r - q) * t + a * (scaled_difference * t - 2.0 * scaled_w * log1p_ratio);
    let d_val = scaled_difference * one_minus_exp / (one - g * exp_minus_dt);
    if !c_val.is_finite() || !d_val.is_finite() {
        return (zero, HestonCfStatus::Overflow);
    }

    let exponent = c_val + d_val * v0 + i * phi * log_spot;
    if !exponent.is_finite() || exponent.re > HESTON_EXPONENT_REAL_LIMIT {
        return (zero, HestonCfStatus::Overflow);
    }

    // ψ_j(φ) = exp(C + D v₀ + iφx)
    let psi = exponent.exp();
    if !psi.is_finite() {
        return (zero, HestonCfStatus::Overflow);
    }
    if psi.norm_sqr() == 0.0 {
        return (zero, HestonCfStatus::Underflow);
    }
    (psi, HestonCfStatus::Ok)
}

/// Independent Black-Scholes reference for the deterministic-variance limit.
#[cfg(test)]
fn bs_reference(spot: f64, strike: f64, r: f64, q: f64, t: f64, vol: f64, is_call: bool) -> f64 {
    use finstack_quant_core::math::special_functions::norm_cdf;

    if vol <= 0.0 || t <= 0.0 {
        return if is_call {
            (spot * (-q * t).exp() - strike * (-r * t).exp()).max(0.0)
        } else {
            (strike * (-r * t).exp() - spot * (-q * t).exp()).max(0.0)
        };
    }

    let sqrt_t = t.sqrt();
    // d1/d2 intentionally inline: In finstack_quant_core, cannot import from valuations
    let d1 = ((spot / strike).ln() + (r - q + 0.5 * vol * vol) * t) / (vol * sqrt_t);
    let d2 = d1 - vol * sqrt_t;

    let call = spot * (-q * t).exp() * norm_cdf(d1) - strike * (-r * t).exp() * norm_cdf(d2);

    if is_call {
        call.max(0.0)
    } else {
        (call - spot * (-q * t).exp() + strike * (-r * t).exp()).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heston_params_validation() {
        assert!(HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.5).is_ok());
        assert!(HestonParams::new(0.0, 2.0, 0.04, 0.3, -0.5).is_err()); // v0 = 0
        assert!(HestonParams::new(-0.01, 2.0, 0.04, 0.3, -0.5).is_err()); // v0 < 0
        assert!(HestonParams::new(0.04, 0.0, 0.04, 0.3, -0.5).is_err()); // kappa = 0
        assert!(HestonParams::new(0.04, 2.0, 0.0, 0.3, -0.5).is_err()); // theta = 0
        assert!(HestonParams::new(0.04, 2.0, 0.04, 0.0, -0.5).is_err()); // sigma_v = 0
        assert!(HestonParams::new(0.04, 2.0, 0.04, 0.3, -1.0).is_err()); // rho = -1
        assert!(HestonParams::new(0.04, 2.0, 0.04, 0.3, 1.0).is_err()); // rho = 1
    }

    #[test]
    fn feller_condition() {
        // Satisfies: 2*2*0.04 = 0.16 > 0.09 = 0.3²
        let p = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.5).expect("valid");
        assert!(p.satisfies_feller_condition());

        // Violates: 2*0.5*0.04 = 0.04 < 0.25 = 0.5²
        let p2 = HestonParams::new(0.04, 0.5, 0.04, 0.5, -0.5).expect("valid");
        assert!(!p2.satisfies_feller_condition());
    }

    #[test]
    fn require_feller_accepts_only_strict_feller_parameters() {
        let ok = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.5).expect("valid");
        assert_eq!(ok.require_feller().expect("satisfies Feller"), ok);

        let violates = HestonParams::new(0.04, 0.5, 0.04, 0.5, -0.5).expect("valid");
        let err = violates
            .require_feller()
            .expect_err("violates Feller condition");
        assert!(
            err.to_string().contains("Feller condition violated"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn call_price_positive_and_bounded() {
        let p = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.5).expect("valid");
        let call = p.price_european(100.0, 100.0, 0.05, 0.0, 1.0, true);
        assert!(call > 0.0, "Call should be positive, got {call}");
        assert!(call < 100.0, "Call should be < spot, got {call}");
    }

    #[test]
    fn put_call_parity() {
        let p = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.7).expect("valid");
        let s = 100.0;
        let k = 100.0;
        let r = 0.05;
        let q = 0.02;
        let t = 1.0;

        let call = p.price_european(s, k, r, q, t, true);
        let put = p.price_european(s, k, r, q, t, false);

        let lhs = call - put;
        let rhs = s * (-q * t).exp() - k * (-r * t).exp();

        assert!(
            (lhs - rhs).abs() < 0.01,
            "Put-call parity: C−P = {lhs:.4}, S·e^{{-qT}} − K·e^{{-rT}} = {rhs:.4}"
        );
    }

    #[test]
    fn moneyness_ordering() {
        let p = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.5).expect("valid");
        let itm = p.price_european(100.0, 90.0, 0.05, 0.0, 1.0, true);
        let atm = p.price_european(100.0, 100.0, 0.05, 0.0, 1.0, true);
        let otm = p.price_european(100.0, 110.0, 0.05, 0.0, 1.0, true);

        assert!(itm > atm, "ITM > ATM: {itm:.4} vs {atm:.4}");
        assert!(atm > otm, "ATM > OTM: {atm:.4} vs {otm:.4}");
    }

    #[test]
    fn black_scholes_limit() {
        let vol = 0.2;
        let var = vol * vol;
        // sigma_v → 0: Heston degenerates to Black-Scholes
        let p = HestonParams::new(var, 2.0, var, 1e-12, 0.0).expect("valid");
        let heston = p.price_european(100.0, 100.0, 0.05, 0.0, 1.0, true);
        let bs = bs_reference(100.0, 100.0, 0.05, 0.0, 1.0, vol, true);

        assert!(
            (heston - bs).abs() < 0.01,
            "Heston → BS limit: Heston={heston:.4}, BS={bs:.4}"
        );
    }

    #[test]
    fn sigma_v_zero_limit_uses_time_averaged_variance() {
        // v0 ≠ θ: the σ_v → 0 limit is the time-averaged deterministic CIR
        // variance, NOT v0. v̄ = θ + (v0−θ)(1−e^{−κT})/(κT).
        let v0 = 0.01;
        let theta = 0.09;
        let kappa = 2.0;
        let t = 1.0;
        let p = HestonParams::new(v0, kappa, theta, 1e-12, 0.0).expect("valid");

        // Closed form
        let v_bar = theta + (v0 - theta) * (1.0 - (-kappa * t).exp()) / (kappa * t);
        assert!(
            (v_bar - 0.0554134).abs() < 1e-6,
            "expected v̄ ≈ 0.0554, got {v_bar:.6}"
        );

        // Brute-force average of the deterministic variance path
        // v(s) = θ + (v0−θ)e^{−κs} via fine trapezoidal integration.
        let n = 100_000;
        let h = t / n as f64;
        let v = |s: f64| theta + (v0 - theta) * (-kappa * s).exp();
        let mut integral = 0.0;
        for j in 0..n {
            integral += 0.5 * (v(j as f64 * h) + v((j + 1) as f64 * h)) * h;
        }
        let v_bar_numeric = integral / t;
        assert!(
            (v_bar - v_bar_numeric).abs() < 1e-9,
            "closed form v̄={v_bar:.10} vs brute force {v_bar_numeric:.10}"
        );

        // The limiting price must match Black-Scholes at σ = √v̄ (≈ 23.5%),
        // not at √v0 = 10%.
        let heston = p.price_european(100.0, 100.0, 0.05, 0.0, t, true);
        let bs_avg = bs_reference(100.0, 100.0, 0.05, 0.0, t, v_bar.sqrt(), true);
        let bs_v0 = bs_reference(100.0, 100.0, 0.05, 0.0, t, v0.sqrt(), true);
        assert!(
            (heston - bs_avg).abs() < 1e-10,
            "limit should use √v̄: heston={heston:.6}, bs(√v̄)={bs_avg:.6}"
        );
        assert!(
            (heston - bs_v0).abs() > 1.0,
            "limit must not collapse to √v0: heston={heston:.6}, bs(√v0)={bs_v0:.6}"
        );
    }

    #[test]
    fn put_call_parity_survives_clamping_in_extreme_region() {
        // Deep-OTM call / deep-ITM put region (~5-8 stddevs OTM) where the
        // raw call is ≈ 0 and quadrature noise can push it slightly negative.
        // The put is derived from the UNclamped call, so parity holds up to
        // the clamped noise rather than drifting by the full clamped amount.
        let p = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.5).expect("valid");
        let spot: f64 = 100.0;
        let r: f64 = 0.05;
        let q: f64 = 0.0;
        let t: f64 = 0.5;

        for &strike in &[220.0_f64, 260.0, 300.0] {
            let call = p.price_european(spot, strike, r, q, t, true);
            let put = p.price_european(spot, strike, r, q, t, false);
            // call clamps to ~0 here; the put is derived from the UNclamped
            // call, so the parity residual is bounded by the clamped
            // quadrature noise on the raw call (~1e-7 in this region).
            let parity_residual = call - put - (spot * (-q * t).exp() - strike * (-r * t).exp());
            assert!(
                parity_residual.abs() < 1e-5,
                "K={strike}: parity residual {parity_residual:.2e} too large \
                 (call={call:.10}, put={put:.10})"
            );
            let intrinsic_fwd = strike * (-r * t).exp() - spot * (-q * t).exp();
            assert!(
                (put - intrinsic_fwd).abs() < 1e-5,
                "K={strike}: deep ITM put should be ≈ forward intrinsic: \
                 put={put:.8}, intrinsic={intrinsic_fwd:.8}"
            );
        }
    }

    #[test]
    fn heston_params_serde_validates_on_deserialize() {
        // Valid JSON round-trips.
        let p = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.5).expect("valid");
        let json = serde_json::to_string(&p).expect("serialize");
        let back: HestonParams = serde_json::from_str(&json).expect("round-trip");
        assert_eq!(p, back);

        // Out-of-range rho rejected.
        let bad = r#"{"v0":0.04,"kappa":2.0,"theta":0.04,"sigma_v":0.3,"rho":1.5}"#;
        assert!(serde_json::from_str::<HestonParams>(bad).is_err());

        // Unknown field rejected.
        let unknown =
            r#"{"v0":0.04,"kappa":2.0,"theta":0.04,"sigma_v":0.3,"rho":-0.5,"extra":1.0}"#;
        assert!(serde_json::from_str::<HestonParams>(unknown).is_err());
    }

    #[test]
    fn expired_option() {
        let p = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.5).expect("valid");
        let itm_call = p.price_european(100.0, 90.0, 0.05, 0.0, 0.0, true);
        assert!((itm_call - 10.0).abs() < 1e-10, "Expired ITM call");

        let otm_call = p.price_european(100.0, 110.0, 0.05, 0.0, 0.0, true);
        assert!(otm_call.abs() < 1e-10, "Expired OTM call");

        let itm_put = p.price_european(100.0, 110.0, 0.05, 0.0, 0.0, false);
        assert!((itm_put - 10.0).abs() < 1e-10, "Expired ITM put");
    }

    #[test]
    fn invalid_inputs_return_nan() {
        let p = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.5).expect("valid");
        let price = p.price_european(100.0, 0.0, 0.05, 0.0, 1.0, true);
        assert!(price.is_nan());
    }

    #[test]
    fn heston_characteristic_function_is_normalized_with_positive_correlation() {
        let p = HestonParams::new(0.04, 0.1, 0.04, 1.0, 0.9).expect("valid");
        for j in [1, 2] {
            let (psi, status) =
                heston_pj_characteristic_function(j, 0.0, 100.0_f64.ln(), 0.05, 0.0, 1.0, &p);
            assert_eq!(status, HestonCfStatus::Ok);
            assert_eq!(psi, Complex64::new(1.0, 0.0));
        }
    }

    #[test]
    fn public_heston_paths_share_short_expiry_convergence() {
        let params = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.7).expect("valid");
        for (t, expected) in [(0.001, 0.254803543342), (0.0001, 0.080038149868)] {
            let scalar = params.price_european(100.0, 100.0, 0.05, 0.0, t, true);
            let strip = params.price_european_strip(100.0, &[100.0], 0.05, 0.0, t, true);
            assert!((scalar - expected).abs() < 1e-9, "t={t}, price={scalar}");
            assert!((strip[0] - scalar).abs() < 1e-12);
        }
    }

    #[test]
    fn price_european_strip_matches_single_strike_prices() {
        let params = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.5).expect("valid");
        let strikes = [80.0, 90.0, 100.0, 110.0, 120.0];

        let strip_prices = params.price_european_strip(100.0, &strikes, 0.05, 0.02, 1.0, true);

        assert_eq!(strip_prices.len(), strikes.len());
        for (idx, &strike) in strikes.iter().enumerate() {
            let single_price = params.price_european(100.0, strike, 0.05, 0.02, 1.0, true);
            assert!(
                (strip_prices[idx] - single_price).abs() < 1e-5,
                "strip price {} should match single-strike price {} for K={}",
                strip_prices[idx],
                single_price,
                strike
            );
        }
    }

    #[test]
    fn price_european_strip_put_call_parity_holds_per_strike() {
        let params = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.7).expect("valid");
        let spot: f64 = 100.0;
        let r: f64 = 0.05;
        let q: f64 = 0.02;
        let t: f64 = 1.0;
        let strikes = [85.0, 95.0, 100.0, 105.0, 115.0];

        let calls = params.price_european_strip(spot, &strikes, r, q, t, true);
        let puts = params.price_european_strip(spot, &strikes, r, q, t, false);

        for ((&strike, &call), &put) in strikes.iter().zip(calls.iter()).zip(puts.iter()) {
            let parity = call - put - (spot * (-q * t).exp() - strike * (-r * t).exp());
            assert!(
                parity.abs() < 1e-12,
                "put-call parity should hold for K={strike}: residual={parity}"
            );
        }
    }

    #[test]
    fn price_european_strip_matches_single_strike_for_short_dated_params() {
        let params = HestonParams::new(0.01, 3.0, 0.01, 0.02, -0.5).expect("valid");
        let spot = 100.0;
        let r = 0.01;
        let q = 0.0;
        let t = 0.005;
        let strikes = [95.0, 100.0, 105.0];

        let strip_prices = params.price_european_strip(spot, &strikes, r, q, t, true);

        for (idx, &strike) in strikes.iter().enumerate() {
            let single_price = params.price_european(spot, strike, r, q, t, true);
            assert!(
                (strip_prices[idx] - single_price).abs() < 1e-5,
                "strip price {} should match refined single-strike price {} for K={}",
                strip_prices[idx],
                single_price,
                strike
            );
        }
    }
}
