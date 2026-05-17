//! COS method for European option pricing (Fang-Oosterlee 2008).
//!
//! The COS method approximates the option value integral using a cosine
//! series expansion, which converges exponentially for smooth densities.
//! It is the fastest single-strike Fourier method with O(N) complexity
//! where N is the number of cosine terms (typically 64-256).
//!
//! # References
//!
//! - Fang, F. & Oosterlee, C. W. (2008). "A Novel Pricing Method for
//!   European Options Based on Fourier-Cosine Series Expansions."
//!   *SIAM J. Sci. Comput.*, 31(2), 826-848.

use finstack_core::math::characteristic_function::CharacteristicFunction;
use num_complex::Complex64;
use std::f64::consts::PI;

/// COS method configuration.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct CosConfig {
    /// Number of cosine terms (default: 128).
    /// More terms = higher accuracy for non-smooth or heavy-tailed densities.
    pub num_terms: usize,
    /// Truncation range multiplier L (default: 10.0).
    /// Integration domain is [c1 - L*sqrt(c2 + sqrt(|c4|)), c1 + L*sqrt(c2 + sqrt(|c4|))].
    pub truncation_l: f64,
}

impl Default for CosConfig {
    fn default() -> Self {
        Self {
            num_terms: 128,
            truncation_l: 10.0,
        }
    }
}

/// COS method pricer for European options.
///
/// Prices a single European option in O(N) where N = num_terms.
/// For pricing across multiple strikes, characteristic function
/// evaluations are reused across strikes.
///
/// # Algorithm (Fang-Oosterlee 2008)
///
/// Working in the variable x = ln(S_T/K), the call option value is:
///
/// ```text
/// C = K * exp(-r*T) * sum_{k=0}^{N-1}' (2/(b-a))
///     * Re[phi_X(k*pi/(b-a)) * exp(-i*k*pi*a/(b-a))]
///     * (chi_k(0,b) - psi_k(0,b))
/// ```
///
/// where phi_X(u) = exp(i*u*ln(S/K)) * phi(u, t) is the CF of X = ln(S_T/K),
/// and the prime on the sum means the k=0 term is halved.
pub struct CosPricer<'a> {
    cf: &'a dyn CharacteristicFunction,
    config: CosConfig,
}

impl<'a> CosPricer<'a> {
    /// Create a new COS pricer.
    pub fn new(cf: &'a dyn CharacteristicFunction, config: CosConfig) -> Self {
        Self { cf, config }
    }

    /// Price a European call option.
    ///
    /// # Parameters
    ///
    /// * `r` — risk-free rate used for the discount factor `exp(-r*t)`. This
    ///   **must** match the rate encoded in the characteristic function's drift
    ///   term. Any dividend yield is already part of the CF's drift and does
    ///   not appear here — the COS method computes the risk-neutral expectation
    ///   directly from `phi_Y`.
    pub fn price_call(
        &self,
        spot: f64,
        strike: f64,
        r: f64,
        t: f64,
    ) -> crate::pricer::PricingResult<f64> {
        self.price(spot, strike, r, t, true)
    }

    /// Price a European put option. See [`price_call`](Self::price_call) for the
    /// role of `r`.
    pub fn price_put(
        &self,
        spot: f64,
        strike: f64,
        r: f64,
        t: f64,
    ) -> crate::pricer::PricingResult<f64> {
        self.price(spot, strike, r, t, false)
    }

    /// Price a strip of European calls across strikes.
    pub fn price_calls(
        &self,
        spot: f64,
        strikes: &[f64],
        r: f64,
        t: f64,
    ) -> crate::pricer::PricingResult<Vec<f64>> {
        self.price_strip(spot, strikes, r, t, true)
    }

    /// Price a strip of European puts across strikes.
    pub fn price_puts(
        &self,
        spot: f64,
        strikes: &[f64],
        r: f64,
        t: f64,
    ) -> crate::pricer::PricingResult<Vec<f64>> {
        self.price_strip(spot, strikes, r, t, false)
    }

    fn price(
        &self,
        spot: f64,
        strike: f64,
        r: f64,
        t: f64,
        is_call: bool,
    ) -> crate::pricer::PricingResult<f64> {
        let prices = self.price_strip(spot, &[strike], r, t, is_call)?;
        Ok(prices[0])
    }

    fn price_strip(
        &self,
        spot: f64,
        strikes: &[f64],
        r: f64,
        t: f64,
        is_call: bool,
    ) -> crate::pricer::PricingResult<Vec<f64>> {
        if strikes.is_empty() {
            return Ok(Vec::new());
        }

        // Cumulants of Y = ln(S_T/S_0) for truncation range.
        let cumulants = self.cf.cumulants(t);

        // The Fang-Oosterlee derivation is in the moneyness variable
        // X = ln(S_T/K) = Y + x0, with x0 = ln(S/K). The cumulants describe
        // the *shape* of the distribution (it differs from Y's only by the
        // deterministic shift x0), so the truncation half-width is the same;
        // but the *centre* of the support of X is x0 to the right of Y's.
        //
        // `truncation_range` returns the Y-centred window [a, b]. For each
        // strike we must integrate the payoff over the X-centred window
        // [a + x0, b + x0], otherwise deep ITM/OTM strikes (large |x0|) push
        // the true payoff region outside the integration window and prices
        // collapse. The width b - a is unchanged, so u_k and the CF values
        // stay strike-independent; only the payoff coefficients shift.
        let (a, b) = truncation_range(&cumulants, self.config.truncation_l);

        if !(a.is_finite() && b.is_finite()) || b <= a {
            return Err(crate::pricer::PricingError::ModelFailure {
                message: "COS method: invalid truncation range from cumulants".to_string(),
                context: crate::pricer::PricingErrorContext::default(),
            });
        }

        let n = self.config.num_terms;
        let bma = b - a;
        let df = (-r * t).exp();

        // Pre-compute the CF values (strike-independent).
        // phi(u_k, t) for u_k = k*pi/(b-a)
        let mut cf_vals: Vec<Complex64> = Vec::with_capacity(n);
        for k in 0..n {
            let u_k = k as f64 * PI / bma;
            cf_vals.push(self.cf.cf(Complex64::new(u_k, 0.0), t));
        }

        let prices = strikes
            .iter()
            .map(|&strike| {
                // x0 = ln(S/K): shift from Y to X = Y + x0.
                // Integration window in X-space, following the moneyness shift.
                let x0 = (spot / strike).ln();
                let a_x = a + x0;
                let b_x = b + x0;

                let mut sum = 0.0;
                for (k, &cf_val) in cf_vals.iter().enumerate() {
                    let k_f = k as f64;
                    let u_k = k_f * PI / bma;

                    // Payoff coefficient over the X-centred window [a_x, b_x].
                    // Call payoff (e^x - 1)^+ is supported on X >= 0; put
                    // payoff (1 - e^x)^+ on X <= 0. The payoff sub-interval is
                    // the intersection of that half-line with [a_x, b_x] — for
                    // deep OTM/ITM strikes the whole window can lie on one side
                    // of zero, so the limits must be clamped, not fixed at 0.
                    let v_k = if is_call {
                        let lo = 0.0_f64.clamp(a_x, b_x);
                        chi_k(k, a_x, b_x, lo, b_x) - psi_k(k, a_x, b_x, lo, b_x)
                    } else {
                        let hi = 0.0_f64.clamp(a_x, b_x);
                        -chi_k(k, a_x, b_x, a_x, hi) + psi_k(k, a_x, b_x, a_x, hi)
                    };

                    // phi_X(u_k) = exp(i*u_k*x0) * phi_Y(u_k)
                    // We need: Re[phi_X(u_k) * exp(-i*u_k*a_x)]
                    //        = Re[phi_Y(u_k) * exp(-i*u_k*a)]   since a_x - x0 = a.
                    let phase = Complex64::new(0.0, -u_k * a).exp();
                    let ak = (cf_val * phase).re;

                    let weight = if k == 0 { 0.5 } else { 1.0 };
                    sum += weight * (2.0 / bma) * ak * v_k;
                }

                let raw = strike * df * sum;
                // A negative result here is always a truncation / discretisation
                // artefact for vanilla calls and puts — the mathematical price
                // is non-negative. Surface it as a warning so callers can tune
                // `num_terms` or `truncation_l` instead of silently accepting a
                // misleading zero.
                if raw < -numerical_negativity_tolerance(strike) {
                    tracing::warn!(
                        strike,
                        spot,
                        r,
                        t,
                        raw,
                        num_terms = self.config.num_terms,
                        truncation_l = self.config.truncation_l,
                        "COS method returned negative raw price; clamping to zero. \
                         Consider increasing num_terms or truncation_l, or check \
                         that the CF's drift matches the supplied r."
                    );
                }
                raw.max(0.0)
            })
            .collect();

        Ok(prices)
    }
}

/// Threshold below which a negative COS price is surfaced as a warning.
///
/// Clamping at exactly zero would fire the warning for benign rounding
/// noise near out-of-the-money strikes. We allow a small relative slack
/// (1e-10 of the strike) before warning.
fn numerical_negativity_tolerance(strike: f64) -> f64 {
    strike.abs() * 1e-10 + 1e-12
}

/// Compute the truncation range [a, b] from cumulants.
///
/// Uses the Fang-Oosterlee (2008) formula:
///   a = c1 - L * sqrt(c2 + sqrt(|c4|))
///   b = c1 + L * sqrt(c2 + sqrt(|c4|))
fn truncation_range(
    c: &finstack_core::math::characteristic_function::Cumulants,
    l: f64,
) -> (f64, f64) {
    let width = l * (c.c2 + c.c4.abs().sqrt()).max(1e-8).sqrt();
    (c.c1 - width, c.c1 + width)
}

/// Cosine series coefficient chi_k for the exponential payoff.
///
/// chi_k(a, b, c, d) = integral from c to d of exp(x) * cos(k*pi*(x-a)/(b-a)) dx
fn chi_k(k: usize, a: f64, b: f64, c: f64, d: f64) -> f64 {
    let bma = b - a;
    let k_pi_bma = k as f64 * PI / bma;

    let denom = 1.0 + k_pi_bma * k_pi_bma;

    let cos_d = (k as f64 * PI * (d - a) / bma).cos();
    let sin_d = (k as f64 * PI * (d - a) / bma).sin();
    let cos_c = (k as f64 * PI * (c - a) / bma).cos();
    let sin_c = (k as f64 * PI * (c - a) / bma).sin();

    (d.exp() * (cos_d + k_pi_bma * sin_d) - c.exp() * (cos_c + k_pi_bma * sin_c)) / denom
}

/// Cosine series coefficient psi_k for the constant payoff.
///
/// psi_k(a, b, c, d) = integral from c to d of cos(k*pi*(x-a)/(b-a)) dx
fn psi_k(k: usize, a: f64, b: f64, c: f64, d: f64) -> f64 {
    if k == 0 {
        return d - c;
    }
    let bma = b - a;
    let k_pi = k as f64 * PI;
    (bma / k_pi) * ((k_pi * (d - a) / bma).sin() - (k_pi * (c - a) / bma).sin())
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::math::characteristic_function::BlackScholesCf;

    /// Reference Black-Scholes call price for validation.
    fn bs_call_price(spot: f64, strike: f64, r: f64, q: f64, t: f64, sigma: f64) -> f64 {
        use finstack_core::math::special_functions::norm_cdf;
        let fwd = spot * ((r - q) * t).exp();
        let sqrt_t = t.sqrt();
        let d1 = ((fwd / strike).ln() + 0.5 * sigma * sigma * t) / (sigma * sqrt_t);
        let d2 = d1 - sigma * sqrt_t;
        (-r * t).exp() * (fwd * norm_cdf(d1) - strike * norm_cdf(d2))
    }

    fn bs_put_price(spot: f64, strike: f64, r: f64, q: f64, t: f64, sigma: f64) -> f64 {
        let call = bs_call_price(spot, strike, r, q, t, sigma);
        call - spot * (-q * t).exp() + strike * (-r * t).exp()
    }

    #[test]
    fn cos_matches_bs_call_atm() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let sigma = 0.2;
        let cf = BlackScholesCf {
            r: 0.05,
            q: 0.0,
            sigma,
        };
        let config = CosConfig {
            num_terms: 128,
            truncation_l: 10.0,
        };
        let pricer = CosPricer::new(&cf, config);
        let cos_price = pricer.price_call(100.0, 100.0, 0.05, 1.0)?;
        let bs_price = bs_call_price(100.0, 100.0, 0.05, 0.0, 1.0, sigma);

        assert!(
            (cos_price - bs_price).abs() < 1e-6,
            "COS={cos_price:.8}, BS={bs_price:.8}, diff={}",
            (cos_price - bs_price).abs()
        );
        Ok(())
    }

    #[test]
    fn cos_matches_bs_call_itm_otm() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let sigma = 0.25;
        let cf = BlackScholesCf {
            r: 0.05,
            q: 0.02,
            sigma,
        };
        let config = CosConfig::default();
        let pricer = CosPricer::new(&cf, config);

        for strike in [80.0, 90.0, 100.0, 110.0, 120.0] {
            let cos_price = pricer.price_call(100.0, strike, 0.05, 1.0)?;
            let bs_price = bs_call_price(100.0, strike, 0.05, 0.02, 1.0, sigma);
            assert!(
                (cos_price - bs_price).abs() < 1e-4,
                "K={strike}: COS={cos_price:.8}, BS={bs_price:.8}, diff={}",
                (cos_price - bs_price).abs()
            );
        }
        Ok(())
    }

    #[test]
    fn cos_put_call_parity() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let sigma = 0.2;
        let cf = BlackScholesCf {
            r: 0.05,
            q: 0.02,
            sigma,
        };
        let config = CosConfig::default();
        let pricer = CosPricer::new(&cf, config);
        let spot = 100.0;
        let strike = 105.0;
        let r = 0.05;
        let q = 0.02;
        let t = 1.0;

        let call = pricer.price_call(spot, strike, r, t)?;
        let put = pricer.price_put(spot, strike, r, t)?;
        let parity = call - put - (spot * (-q * t).exp() - strike * (-r * t).exp());

        assert!(
            parity.abs() < 1e-6,
            "Put-call parity residual: {parity:.10}"
        );
        Ok(())
    }

    #[test]
    fn cos_put_matches_bs() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let sigma = 0.2;
        let cf = BlackScholesCf {
            r: 0.05,
            q: 0.0,
            sigma,
        };
        let config = CosConfig::default();
        let pricer = CosPricer::new(&cf, config);
        let cos_put = pricer.price_put(100.0, 100.0, 0.05, 1.0)?;
        let bs_put = bs_put_price(100.0, 100.0, 0.05, 0.0, 1.0, sigma);
        assert!(
            (cos_put - bs_put).abs() < 1e-6,
            "COS put={cos_put:.8}, BS put={bs_put:.8}"
        );
        Ok(())
    }

    #[test]
    fn cos_strip_matches_singles() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let sigma = 0.2;
        let cf = BlackScholesCf {
            r: 0.05,
            q: 0.0,
            sigma,
        };
        let config = CosConfig::default();
        let pricer = CosPricer::new(&cf, config);
        let strikes = vec![90.0, 95.0, 100.0, 105.0, 110.0];

        let strip = pricer.price_calls(100.0, &strikes, 0.05, 1.0)?;
        for (i, &k) in strikes.iter().enumerate() {
            let single = pricer.price_call(100.0, k, 0.05, 1.0)?;
            assert!(
                (strip[i] - single).abs() < 1e-12,
                "Strip[{i}]={}, single={}",
                strip[i],
                single
            );
        }
        Ok(())
    }

    #[test]
    fn cos_variance_gamma_prices_are_positive(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        use finstack_core::math::characteristic_function::VarianceGammaCf;
        let vg = VarianceGammaCf {
            r: 0.05,
            q: 0.0,
            sigma: 0.12,
            nu: 0.2,
            theta: -0.14,
        };
        let config = CosConfig {
            num_terms: 256,
            truncation_l: 12.0,
        };
        let pricer = CosPricer::new(&vg, config);
        let call = pricer.price_call(100.0, 100.0, 0.05, 1.0)?;
        assert!(call > 0.0, "VG call should be positive: {call}");
        assert!(call < 100.0, "VG call should be < spot: {call}");
        Ok(())
    }

    /// W-42 regression: deep ITM/OTM strikes on a low-vol, short-dated
    /// underlying. The cumulant half-width of `Y = ln(S_T/S_0)` is tiny, so
    /// the moneyness shift `x0 = ln(S/K)` pushes the true support of
    /// `X = ln(S_T/K)` entirely outside a `Y`-centred truncation window.
    /// The integration window must follow the per-strike moneyness shift.
    #[test]
    fn cos_deep_itm_otm_low_vol_short_dated(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let sigma = 0.05;
        let r = 0.05;
        let q = 0.0;
        let t = 0.05;
        let cf = BlackScholesCf { r, q, sigma };
        let config = CosConfig::default();
        let pricer = CosPricer::new(&cf, config);
        let spot = 100.0;

        // Strikes span deep ITM through deep OTM. |x0| reaches ~0.69, far
        // beyond the ~0.11 cumulant half-width at this vol/maturity.
        for strike in [50.0, 70.0, 90.0, 100.0, 110.0, 130.0, 150.0, 200.0] {
            let cos_call = pricer.price_call(spot, strike, r, t)?;
            let bs_call = bs_call_price(spot, strike, r, q, t, sigma);
            assert!(
                (cos_call - bs_call).abs() < 1e-6,
                "call K={strike}: COS={cos_call:.10}, BS={bs_call:.10}, diff={}",
                (cos_call - bs_call).abs()
            );

            let cos_put = pricer.price_put(spot, strike, r, t)?;
            let bs_put = bs_put_price(spot, strike, r, q, t, sigma);
            assert!(
                (cos_put - bs_put).abs() < 1e-6,
                "put K={strike}: COS={cos_put:.10}, BS={bs_put:.10}, diff={}",
                (cos_put - bs_put).abs()
            );
        }
        Ok(())
    }

    #[test]
    fn cos_merton_prices_are_reasonable() -> std::result::Result<(), Box<dyn std::error::Error>> {
        use finstack_core::math::characteristic_function::MertonJumpCf;
        let merton = MertonJumpCf {
            r: 0.05,
            q: 0.0,
            sigma: 0.2,
            lambda: 1.0,
            mu_j: -0.05,
            sigma_j: 0.1,
        };
        let config = CosConfig {
            num_terms: 256,
            truncation_l: 12.0,
        };
        let pricer = CosPricer::new(&merton, config);
        let call = pricer.price_call(100.0, 100.0, 0.05, 1.0)?;
        assert!(call > 0.0, "Merton call should be positive: {call}");
        assert!(call < 100.0, "Merton call should be < spot: {call}");

        // Should be somewhat close to BS but different due to jumps
        let bs_price = bs_call_price(100.0, 100.0, 0.05, 0.0, 1.0, 0.2);
        assert!(
            (call - bs_price).abs() < 5.0,
            "Merton should be in BS neighborhood: merton={call}, bs={bs_price}"
        );
        Ok(())
    }
}
