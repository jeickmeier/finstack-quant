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

use super::PricingError;
use finstack_quant_core::math::characteristic_function::{
    BlackScholesCf, CharacteristicFunction, MertonJumpCf, VarianceGammaCf,
};
use finstack_quant_core::math::NeumaierAccumulator;
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

/// Inputs for Black-Scholes pricing via the COS method.
#[derive(Debug, Clone, Copy)]
pub struct BlackScholesCosParams {
    /// Current spot price of the underlying.
    pub spot: f64,
    /// Option strike.
    pub strike: f64,
    /// Continuously-compounded risk-free rate.
    pub rate: f64,
    /// Continuous dividend yield.
    pub dividend: f64,
    /// Annualized volatility.
    pub vol: f64,
    /// Time to expiry in years.
    pub maturity: f64,
    /// `true` for call, `false` for put.
    pub is_call: bool,
    /// Optional COS term count; defaults to [`CosConfig::default`].
    pub n_terms: Option<usize>,
}

/// Inputs for Variance Gamma pricing via the COS method.
#[derive(Debug, Clone, Copy)]
pub struct VarianceGammaCosParams {
    /// Current spot price of the underlying.
    pub spot: f64,
    /// Option strike.
    pub strike: f64,
    /// Continuously-compounded risk-free rate.
    pub rate: f64,
    /// Continuous dividend yield.
    pub dividend: f64,
    /// Volatility of the subordinated Brownian motion.
    pub sigma: f64,
    /// Drift of the subordinated Brownian motion.
    pub theta: f64,
    /// Variance rate of the Gamma subordinator.
    pub nu: f64,
    /// Time to expiry in years.
    pub maturity: f64,
    /// `true` for call, `false` for put.
    pub is_call: bool,
    /// Optional COS term count; defaults to [`CosConfig::default`].
    pub n_terms: Option<usize>,
}

/// Inputs for Merton jump-diffusion pricing via the COS method.
#[derive(Debug, Clone, Copy)]
pub struct MertonJumpCosParams {
    /// Current spot price of the underlying.
    pub spot: f64,
    /// Option strike.
    pub strike: f64,
    /// Continuously-compounded risk-free rate.
    pub rate: f64,
    /// Continuous dividend yield.
    pub dividend: f64,
    /// Diffusion volatility.
    pub sigma: f64,
    /// Mean of log-jump size.
    pub mu_jump: f64,
    /// Standard deviation of log-jump size.
    pub sigma_jump: f64,
    /// Jump intensity, in expected jumps per year.
    pub lambda: f64,
    /// Time to expiry in years.
    pub maturity: f64,
    /// `true` for call, `false` for put.
    pub is_call: bool,
    /// Optional COS term count; defaults to [`CosConfig::default`].
    pub n_terms: Option<usize>,
}

/// Price a European option under Black-Scholes using the COS method.
///
/// # Arguments
///
/// * `params` - Black-Scholes COS input bag containing spot, strike, maturity,
///   continuous rates/carry, volatility, payoff direction, and optional term
///   count; `None` uses [`CosConfig::default`].
pub fn bs_cos_price(params: BlackScholesCosParams) -> std::result::Result<f64, PricingError> {
    let cf = BlackScholesCf {
        r: params.rate,
        q: params.dividend,
        sigma: params.vol,
    };
    price_from_cf(
        &cf,
        params.spot,
        params.strike,
        params.rate,
        params.maturity,
        params.is_call,
        params.n_terms,
    )
}

/// Price a European option under Variance Gamma using the COS method.
///
/// # Arguments
///
/// * `params` - Variance-Gamma COS input bag containing option data, continuous
///   rates/carry, process parameters, payoff direction, and optional term
///   count; `None` uses [`CosConfig::default`].
pub fn vg_cos_price(params: VarianceGammaCosParams) -> std::result::Result<f64, PricingError> {
    let cf = VarianceGammaCf {
        r: params.rate,
        q: params.dividend,
        sigma: params.sigma,
        nu: params.nu,
        theta: params.theta,
    };
    price_from_cf(
        &cf,
        params.spot,
        params.strike,
        params.rate,
        params.maturity,
        params.is_call,
        params.n_terms,
    )
}

/// Price a European option under Merton jump-diffusion using the COS method.
///
/// # Arguments
///
/// * `params` - Merton jump-diffusion COS input bag containing option data,
///   continuous rates/carry, diffusion and jump parameters, payoff direction,
///   and optional term count; `None` uses [`CosConfig::default`].
pub fn merton_jump_cos_price(
    params: MertonJumpCosParams,
) -> std::result::Result<f64, PricingError> {
    let cf = MertonJumpCf {
        r: params.rate,
        q: params.dividend,
        sigma: params.sigma,
        lambda: params.lambda,
        mu_j: params.mu_jump,
        sigma_j: params.sigma_jump,
    };
    price_from_cf(
        &cf,
        params.spot,
        params.strike,
        params.rate,
        params.maturity,
        params.is_call,
        params.n_terms,
    )
}

fn cos_config(n_terms: Option<usize>) -> CosConfig {
    let default = CosConfig::default();
    CosConfig {
        num_terms: n_terms.unwrap_or(default.num_terms),
        ..default
    }
}

fn price_from_cf(
    cf: &dyn CharacteristicFunction,
    spot: f64,
    strike: f64,
    rate: f64,
    maturity: f64,
    is_call: bool,
    n_terms: Option<usize>,
) -> std::result::Result<f64, PricingError> {
    let pricer = CosPricer::new(cf, cos_config(n_terms));
    if is_call {
        pricer.price_call(spot, strike, rate, maturity)
    } else {
        pricer.price_put(spot, strike, rate, maturity)
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
    ) -> std::result::Result<f64, PricingError> {
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
    ) -> std::result::Result<f64, PricingError> {
        self.price(spot, strike, r, t, false)
    }

    /// Price a strip of European calls across strikes.
    pub fn price_calls(
        &self,
        spot: f64,
        strikes: &[f64],
        r: f64,
        t: f64,
    ) -> Result<Vec<f64>, PricingError> {
        self.price_strip(spot, strikes, r, t, true)
    }

    /// Price a strip of European puts across strikes.
    pub fn price_puts(
        &self,
        spot: f64,
        strikes: &[f64],
        r: f64,
        t: f64,
    ) -> Result<Vec<f64>, PricingError> {
        self.price_strip(spot, strikes, r, t, false)
    }

    fn price(
        &self,
        spot: f64,
        strike: f64,
        r: f64,
        t: f64,
        is_call: bool,
    ) -> std::result::Result<f64, PricingError> {
        // `price_strip` returns one price per input strike; with a single
        // strike the result is a one-element vector. Use a non-panicking
        // accessor — a panicking index has no place in library code.
        self.price_strip(spot, &[strike], r, t, is_call)?
            .into_iter()
            .next()
            .ok_or_else(|| {
                crate::pricer::PricingError::model_failure_with_context(
                    "COS method: price_strip returned no price for a single strike",
                    crate::pricer::PricingErrorContext::default(),
                )
            })
    }

    fn price_strip(
        &self,
        spot: f64,
        strikes: &[f64],
        r: f64,
        t: f64,
        is_call: bool,
    ) -> Result<Vec<f64>, PricingError> {
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
        let (a, b) = truncation_range(&cumulants, self.config.truncation_l)?;

        if !(a.is_finite() && b.is_finite()) || b <= a {
            return Err(crate::pricer::PricingError::model_failure_with_context(
                "COS method: invalid truncation range from cumulants",
                crate::pricer::PricingErrorContext::default(),
            ));
        }

        let n = self.config.num_terms;
        let bma = b - a;
        let df = (-r * t).exp();

        // Pre-compute the strike-independent COS coefficients
        //   a_k = Re[phi_Y(u_k) * exp(-i*u_k*a)],   u_k = k*pi/(b-a)
        // for the whole strip. Only the payoff coefficient `v_k` depends on the
        // strike, so the characteristic-function evaluation and the complex
        // `exp(-i*u_k*a)` phase factor (the dominant per-term cost) are computed
        // once here rather than once per strike inside `put_price`.
        //
        // A non-finite CF value (NaN/inf) — e.g. a model parameterised
        // outside its domain of validity — would otherwise propagate into
        // `raw` and then be silently swallowed by a `max(0.0)` clamp,
        // reporting a fully-failed pricing as a benign `$0`. Reject it here.
        let mut aks: Vec<f64> = Vec::with_capacity(n);
        for k in 0..n {
            let u_k = k as f64 * PI / bma;
            let cf_val = self.cf.cf(Complex64::new(u_k, 0.0), t);
            if !(cf_val.re.is_finite() && cf_val.im.is_finite()) {
                return Err(crate::pricer::PricingError::model_failure_with_context(
                    format!(
                        "COS method: characteristic function returned a non-finite \
                         value ({cf_val}) at frequency u_{k}={u_k}; the model is \
                         likely parameterised outside its domain of validity"
                    ),
                    crate::pricer::PricingErrorContext::default(),
                ));
            }
            let phase = Complex64::new(0.0, -u_k * a).exp();
            aks.push((cf_val * phase).re);
        }

        // The COS cosine-series sum is evaluated on the *put* payoff for every
        // option. The put payoff `(1 - e^x)^+` is supported on `x <= 0`, where
        // `e^x <= 1`, so the payoff coefficient `chi_k` never carries a large
        // `exp(b)` term. The call payoff `(e^x - 1)^+` lives on `x >= 0`, and
        // for long-dated / high-drift regimes the Fang-Oosterlee window is wide
        // enough that `exp(b)` overflows the usable f64 dynamic range — pricing
        // the call directly then collapses. We instead always price the put and
        // recover the call by put-call parity.
        //
        // Put-call parity in the COS framework:
        //   C - P = e^{-rt} * E[S_T - K]
        //         = e^{-rt} * (S_0 * E[e^Y] - K)
        // where E[e^Y] = phi_Y(-i) is the CF evaluated at u = -i. For a
        // martingale-consistent CF this equals exp((r-q)*t), so the term is the
        // dividend-discounted spot S_0 * exp(-q*t). Using the CF avoids needing
        // the dividend yield q explicitly. `phi_Y(-i)` is strike-independent,
        // so it is evaluated once here for the whole strip.
        let fwd_moment_re = if is_call {
            let fwd_moment = self.cf.cf(Complex64::new(0.0, -1.0), t);
            if !(fwd_moment.re.is_finite() && fwd_moment.im.is_finite()) {
                return Err(crate::pricer::PricingError::model_failure_with_context(
                    format!(
                        "COS method: characteristic function returned a non-finite \
                         forward moment phi(-i) ({fwd_moment}); cannot apply \
                         put-call parity"
                    ),
                    crate::pricer::PricingErrorContext::default(),
                ));
            }
            fwd_moment.re
        } else {
            // Unused on the put path.
            0.0
        };

        strikes
            .iter()
            .map(|&strike| {
                let put = self.put_price(strike, r, t, spot, a, b, bma, df, &aks)?;
                if !is_call {
                    return Ok(put);
                }
                let call = put + df * (spot * fwd_moment_re - strike);
                cos_finite_price(call, "call")
            })
            .collect()
    }

    /// COS price of a European put for a single strike.
    ///
    /// The put payoff coefficients integrate `e^x` only over `x <= 0`, so the
    /// coefficient `chi_k` stays bounded for arbitrarily wide truncation
    /// windows — this is the numerically stable building block from which the
    /// call is recovered by put-call parity.
    #[allow(clippy::too_many_arguments)]
    fn put_price(
        &self,
        strike: f64,
        r: f64,
        t: f64,
        spot: f64,
        a: f64,
        b: f64,
        bma: f64,
        df: f64,
        aks: &[f64],
    ) -> std::result::Result<f64, PricingError> {
        // x0 = ln(S/K): shift from Y to X = Y + x0.
        // Integration window in X-space, following the moneyness shift.
        let x0 = (spot / strike).ln();
        let a_x = a + x0;
        let b_x = b + x0;

        // The COS cosine series accumulates up to `num_terms` (configurably
        // hundreds) terms whose magnitudes span a wide dynamic range — the
        // payoff coefficients `v_k` decay only algebraically while the CF
        // weights decay (sub-)exponentially. A naive `+=` accumulator loses
        // low-order bits; use Neumaier compensated summation.
        let mut sum = NeumaierAccumulator::new();
        for (k, &ak) in aks.iter().enumerate() {
            // Put payoff (1 - e^x)^+ is supported on X <= 0. The payoff
            // sub-interval is the intersection of that half-line with the
            // X-centred window [a_x, b_x] — for deep OTM/ITM strikes the
            // whole window can lie on one side of zero, so the upper limit
            // must be clamped, not fixed at 0.
            let hi = 0.0_f64.clamp(a_x, b_x);
            let v_k = -chi_k(k, a_x, b_x, a_x, hi) + psi_k(k, a_x, b_x, a_x, hi);

            // `ak = Re[phi_Y(u_k) * exp(-i*u_k*a)]` is strike-independent and was
            // precomputed for the whole strip; only `v_k` varies with the strike.
            // (The phase identity uses `a_x - x0 = a`.)
            let weight = if k == 0 { 0.5 } else { 1.0 };
            sum.add(weight * (2.0 / bma) * ak * v_k);
        }

        let raw = strike * df * sum.total();
        // A negative result here is always a truncation / discretisation
        // artefact for a vanilla put — the mathematical price is non-negative.
        // Surface it as a warning so callers can tune `num_terms` or
        // `truncation_l` instead of silently accepting a misleading zero.
        if raw < -numerical_negativity_tolerance(strike) {
            tracing::warn!(
                strike,
                spot,
                r,
                t,
                raw,
                num_terms = self.config.num_terms,
                truncation_l = self.config.truncation_l,
                "COS method returned negative raw put price; clamping to zero. \
                 Consider increasing num_terms or truncation_l, or check \
                 that the CF's drift matches the supplied r."
            );
        }
        cos_finite_price(raw, "put")
    }
}

/// Validate a raw COS price and clamp small negative noise to zero.
///
/// A non-finite `raw` (NaN/inf) means the cosine series or characteristic
/// function diverged; returning `raw.max(0.0)` would turn that into a silent
/// `$0`, since `f64::max(NaN, 0.0) == 0.0` and the negativity check
/// `raw < -tol` is `false` for `NaN`. Surface it as an explicit error instead.
fn cos_finite_price(raw: f64, side: &str) -> std::result::Result<f64, PricingError> {
    if !raw.is_finite() {
        return Err(crate::pricer::PricingError::model_failure_with_context(
            format!(
                "COS method: {side} price is non-finite ({raw}); the cosine \
                 series or characteristic function diverged — increase \
                 num_terms / truncation_l or check the model parameters"
            ),
            crate::pricer::PricingErrorContext::default(),
        ));
    }
    Ok(raw.max(0.0))
}

/// Threshold below which a negative COS price is surfaced as a warning.
///
/// Clamping at exactly zero would fire the warning for benign rounding
/// noise near out-of-the-money strikes. We allow a small relative slack
/// (1e-10 of the strike) before warning.
fn numerical_negativity_tolerance(strike: f64) -> f64 {
    strike.abs() * 1e-10 + 1e-12
}

/// Minimum truncation half-width below which the cumulant set is treated as
/// degenerate.
///
/// `c2 + sqrt(|c4|)` is the radicand of the Fang-Oosterlee half-width. A value
/// at or below this threshold describes a (near-)deterministic log-price
/// (`c2 -> 0`, e.g. `t -> 0` or `sigma -> 0`), for which the cosine-series
/// expansion has no meaningful integration window.
const DEGENERATE_CUMULANT_RADICAND: f64 = 1e-12;

/// Compute the truncation range [a, b] from cumulants.
///
/// Uses the Fang-Oosterlee (2008) §3.3 formula:
///   a = c1 - L * sqrt(c2 + sqrt(|c4|))
///   b = c1 + L * sqrt(c2 + sqrt(|c4|))
///
/// The radicand `c2 + sqrt(|c4|)` is the distribution's spread proxy. A
/// (near-)degenerate cumulant set — `c2` and `c4` both collapsing to zero, as
/// happens for a deterministic payoff (`t -> 0`, `sigma -> 0`) — leaves no
/// meaningful integration window. The previous implementation floored the
/// radicand at `1e-8`, silently producing a tiny non-zero window and
/// mis-pricing such an input rather than rejecting it; this returns an
/// explicit error instead.
fn truncation_range(
    c: &finstack_quant_core::math::characteristic_function::Cumulants,
    l: f64,
) -> std::result::Result<(f64, f64), PricingError> {
    let radicand = c.c2 + c.c4.abs().sqrt();
    if !radicand.is_finite() || radicand <= DEGENERATE_CUMULANT_RADICAND {
        return Err(crate::pricer::PricingError::model_failure_with_context(
            format!(
                "COS method: degenerate cumulant set (c2={}, c4={}); the \
                 log-price distribution has effectively zero spread, so no \
                 meaningful truncation window exists — the COS method is not \
                 applicable to a (near-)deterministic payoff",
                c.c2, c.c4
            ),
            crate::pricer::PricingErrorContext::default(),
        ));
    }
    let width = l * radicand.sqrt();
    Ok((c.c1 - width, c.c1 + width))
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
    use finstack_quant_core::math::characteristic_function::{
        BlackScholesCf, Cumulants, MertonJumpCf,
    };

    /// Test CF whose `cf` evaluation returns a non-finite value.
    ///
    /// Used to verify that the COS pricer surfaces a non-finite
    /// characteristic-function result as an explicit error rather than
    /// silently clamping the resulting price to zero.
    struct NanCf;

    impl CharacteristicFunction for NanCf {
        fn cf(&self, _u: Complex64, _t: f64) -> Complex64 {
            Complex64::new(f64::NAN, 0.0)
        }
        fn cumulants(&self, t: f64) -> Cumulants {
            // Well-formed, finite cumulants so the failure is isolated to the CF.
            Cumulants {
                c1: 0.0,
                c2: 0.04 * t,
                c3: 0.0,
                c4: 0.0,
            }
        }
    }

    /// Test CF reporting a degenerate (zero-spread) cumulant set.
    ///
    /// `c2 = c4 = 0` describes a deterministic (point-mass) log-price, for
    /// which the COS truncation half-width collapses to zero.
    struct DegenerateCf;

    impl CharacteristicFunction for DegenerateCf {
        fn cf(&self, _u: Complex64, _t: f64) -> Complex64 {
            Complex64::new(1.0, 0.0)
        }
        fn cumulants(&self, _t: f64) -> Cumulants {
            Cumulants {
                c1: 0.0,
                c2: 0.0,
                c3: 0.0,
                c4: 0.0,
            }
        }
    }

    /// Reference Black-Scholes call price for validation.
    fn bs_call_price(spot: f64, strike: f64, r: f64, q: f64, t: f64, sigma: f64) -> f64 {
        use finstack_quant_core::math::special_functions::norm_cdf;
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
        use finstack_quant_core::math::characteristic_function::VarianceGammaCf;
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
    fn cos_deep_itm_otm_low_vol_short_dated() -> std::result::Result<(), Box<dyn std::error::Error>>
    {
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
        use finstack_quant_core::math::characteristic_function::MertonJumpCf;
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

    /// Item 1: a non-finite characteristic-function result must surface as an
    /// explicit error, not a silently-clamped `$0` price.
    ///
    /// With `raw = NaN`, the old guard `raw < -tol` is `false` (no warning
    /// fires) and `raw.max(0.0)` returns `0.0`, so a totally failed pricing
    /// was indistinguishable from a deep-OTM zero.
    #[test]
    fn cos_non_finite_cf_returns_error_not_silent_zero() {
        let pricer = CosPricer::new(&NanCf, CosConfig::default());

        let call = pricer.price_call(100.0, 100.0, 0.05, 1.0);
        assert!(
            call.is_err(),
            "non-finite CF must yield an error, got {call:?}"
        );
        let msg = format!("{}", call.unwrap_err());
        assert!(
            msg.contains("non-finite") || msg.contains("not finite"),
            "error should explain the non-finite failure, got: {msg}"
        );

        // The strip path must fail the same way (it is the shared core).
        let strip = pricer.price_calls(100.0, &[90.0, 100.0, 110.0], 0.05, 1.0);
        assert!(
            strip.is_err(),
            "non-finite CF must yield an error on the strip path too, got {strip:?}"
        );
    }

    /// Item 3: a degenerate cumulant set (`c2 = c4 = 0`, i.e. a deterministic
    /// log-price) must be rejected with an error rather than silently producing
    /// a tiny, mis-priced truncation window via the old `√1e-8` floor.
    #[test]
    fn cos_degenerate_cumulants_return_error() {
        let pricer = CosPricer::new(&DegenerateCf, CosConfig::default());
        let call = pricer.price_call(100.0, 100.0, 0.05, 1.0);
        assert!(
            call.is_err(),
            "degenerate cumulants must yield an error, got {call:?}"
        );
        let msg = format!("{}", call.unwrap_err());
        assert!(
            msg.contains("degenerate") || msg.contains("cumulant"),
            "error should explain the degenerate-cumulant failure, got: {msg}"
        );
    }

    /// Item 3: a near-degenerate but genuinely non-trivial cumulant set (very
    /// short maturity, low vol) must still price — the rejection only fires
    /// for a truly collapsed window, not for legitimately small ones.
    #[test]
    fn cos_tiny_but_valid_cumulants_still_price(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let sigma = 0.01;
        let r = 0.01;
        let t = 1.0 / 365.0;
        let cf = BlackScholesCf { r, q: 0.0, sigma };
        let pricer = CosPricer::new(&cf, CosConfig::default());
        let cos = pricer.price_call(100.0, 100.0, r, t)?;
        let bs = bs_call_price(100.0, 100.0, r, 0.0, t, sigma);
        assert!(
            (cos - bs).abs() < 1e-6,
            "tiny-but-valid case should still price accurately: COS={cos}, BS={bs}"
        );
        Ok(())
    }

    /// Item 2: COS accuracy must hold for long-dated / high-drift regimes at
    /// the default `num_terms`.
    ///
    /// A strongly-skewed Merton process accumulates variance, skew, and
    /// kurtosis linearly in `t`. By `t = 20`+ the Fang-Oosterlee truncation
    /// window is wide enough that the call payoff coefficient `chi_k` — which
    /// carries an `exp(b)` term — overflows the usable f64 dynamic range,
    /// producing a wildly wrong direct-call COS price. Pricing the call from
    /// the bounded put payoff via put-call parity keeps `exp(x) <= 1` on the
    /// integration support and stays accurate.
    #[test]
    fn cos_long_dated_high_drift_accuracy() -> std::result::Result<(), Box<dyn std::error::Error>> {
        for t in [10.0, 20.0, 30.0] {
            let r = 0.05;
            let merton = MertonJumpCf {
                r,
                q: 0.0,
                sigma: 0.2,
                lambda: 2.0,
                mu_j: -0.3,
                sigma_j: 0.2,
            };
            let pricer = CosPricer::new(&merton, CosConfig::default());

            for k in [80.0, 100.0, 120.0] {
                let call = pricer.price_call(100.0, k, r, t)?;
                let put = pricer.price_put(100.0, k, r, t)?;

                // Sanity bounds: a call is in [intrinsic_fwd_discounted, spot].
                assert!(
                    call.is_finite() && (0.0..=100.0).contains(&call),
                    "Merton call out of range at t={t}, K={k}: {call}"
                );
                assert!(
                    put.is_finite() && put >= 0.0,
                    "Merton put invalid at t={t}, K={k}: {put}"
                );

                // Put-call parity must hold to tight tolerance for q = 0:
                //   C - P = S - K * exp(-r t)
                let parity = call - put - (100.0 - k * (-r * t).exp());
                assert!(
                    parity.abs() < 1e-6,
                    "put-call parity broken at t={t}, K={k}: residual={parity:.3e} \
                     (call={call}, put={put})"
                );
            }
        }
        Ok(())
    }
}
