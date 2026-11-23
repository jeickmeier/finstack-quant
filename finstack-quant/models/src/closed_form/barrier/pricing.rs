//! Closed-form barrier option and rebate formulas.

use super::super::vanilla::require_finite_positive;
use crate::gaussian::{log_norm_cdf_or_nan, norm_cdf_or_nan};
use crate::types::OptionType;
use finstack_quant_core::math::integration::adaptive_simpson;
use finstack_quant_core::math::special_functions::{log_norm_cdf, norm_cdf};
use finstack_quant_core::types::{BarrierType, PayoutTiming};

/// Hard limit on integrand evaluations for the negative-rate rebate integral.
///
/// The integrand is smooth on `[0, 1]`, so the `1e-12` budget is met in a few
/// thousand samples; the limit only bounds a run that cannot converge.
pub(super) const REBATE_QUADRATURE_MAX_EVALS: usize = 1_000_000;

/// Parameters for barrier option pricing.
#[derive(Debug, Clone, Copy)]
pub struct BarrierParams {
    /// Current underlying spot price
    pub spot: f64,
    /// Strike price
    pub strike: f64,
    /// Barrier level
    pub barrier: f64,
    /// Time to expiry in years
    pub time: f64,
    /// Risk-free interest rate (annualized)
    pub rate: f64,
    /// Continuous dividend yield (annualized)
    pub div_yield: f64,
    /// Volatility (annualized)
    pub vol: f64,
}

impl BarrierParams {
    /// Create new barrier parameters
    pub fn new(
        spot: f64,
        strike: f64,
        barrier: f64,
        time: f64,
        rate: f64,
        div_yield: f64,
        vol: f64,
    ) -> Self {
        Self {
            spot,
            strike,
            barrier,
            time,
            rate,
            div_yield,
            vol,
        }
    }

    /// Create barrier parameters from a discount factor, deriving the
    /// continuously-compounded rate as `-ln(df)/t`.
    ///
    /// When `time <= 0` the option is expired and discounting is moot, so the
    /// rate is set to `0.0` as a harmless convention.
    ///
    /// # Errors
    ///
    /// Returns a [`finstack_quant_core::Error::Validation`] when `df` is not a
    /// strictly positive finite number. A non-positive discount factor implies
    /// a corrupt or inverted curve; deriving `rate = -ln(df)/t` is undefined
    /// (`ln` of a non-positive value), so the previous behaviour of coercing
    /// such a `df` to `rate = 0.0` silently mispriced the barrier on bad input.
    pub fn with_df(
        spot: f64,
        strike: f64,
        barrier: f64,
        time: f64,
        df: f64,
        div_yield: f64,
        vol: f64,
    ) -> finstack_quant_core::Result<Self> {
        if !df.is_finite() || df <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "BarrierParams::with_df requires a strictly positive finite discount \
                 factor (corrupt/inverted curve otherwise), got df={df}"
            )));
        }
        // `time <= 0` ⇒ expired: discounting is irrelevant, rate is unused.
        let rate = if time > 0.0 { -df.ln() / time } else { 0.0 };
        Ok(Self::new(spot, strike, barrier, time, rate, div_yield, vol))
    }
}

#[inline]
pub(super) fn vanilla_option_price(
    spot: f64,
    strike: f64,
    time: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    eta: f64,
) -> f64 {
    let option_type = if eta > 0.0 {
        OptionType::Call
    } else {
        OptionType::Put
    };
    super::super::vanilla::bs_price_unchecked(spot, strike, rate, div_yield, vol, time, option_type)
}

#[inline]
pub(super) fn deterministic_barrier_crossed(
    spot: f64,
    barrier: f64,
    time: f64,
    rate: f64,
    div_yield: f64,
    is_up: bool,
) -> bool {
    if time <= 0.0 {
        return if is_up {
            spot >= barrier
        } else {
            spot <= barrier
        };
    }

    let forward = spot * ((rate - div_yield) * time).exp();
    if is_up {
        spot.max(forward) >= barrier
    } else {
        spot.min(forward) <= barrier
    }
}

/// Deterministic barrier payoff when volatility is zero.
///
/// With zero vol, the asset follows a deterministic drift path:
/// `S(T) = S * exp((r - q) * T)`. We check whether this path crosses
/// the barrier and compute the vanilla intrinsic accordingly.
pub(super) fn barrier_helper_zero_vol(
    params: &BarrierParams,
    eta: f64, // 1 for call, -1 for put
    phi: f64, // 1 for up, -1 for down
) -> f64 {
    let intrinsic = vanilla_option_price(
        params.spot,
        params.strike,
        params.time,
        params.rate,
        params.div_yield,
        0.0,
        eta,
    );
    let barrier_crossed = deterministic_barrier_crossed(
        params.spot,
        params.barrier,
        params.time,
        params.rate,
        params.div_yield,
        phi > 0.0,
    );

    // barrier_helper computes the knock-IN value. If the barrier was crossed,
    // the knock-in option activates and pays the vanilla intrinsic; otherwise 0.
    if barrier_crossed { intrinsic } else { 0.0 }
}

/// Validate that the price-like barrier inputs are strictly positive finite
/// numbers.
///
/// `spot`, `strike` and `barrier` all flow into `ln(...)` terms; a non-positive
/// or non-finite value would produce a `NaN` price. Release builds previously
/// only had a `debug_assert` here, so such corrupt inputs silently yielded a
/// `NaN` with no diagnostic. This returns an explicit validation error instead.
pub(super) fn validate_barrier_inputs(
    spot: f64,
    strike: f64,
    barrier: f64,
) -> finstack_quant_core::Result<()> {
    require_finite_positive("barrier pricing", "spot", spot)?;
    require_finite_positive("barrier pricing", "strike", strike)?;
    require_finite_positive("barrier pricing", "barrier", barrier)
}

/// Helper function for barrier pricing.
///
/// # Errors
///
/// Returns a [`finstack_quant_core::Error::Validation`] when `spot`, `strike` or
/// `barrier` is not a strictly positive finite number (such inputs would feed a
/// `NaN` into the `ln`-based Reiner-Rubinstein terms).
pub(super) fn barrier_helper(
    params: &BarrierParams,
    eta: f64, // 1 for call, -1 for put
    phi: f64, // 1 for up, -1 for down
) -> finstack_quant_core::Result<f64> {
    let BarrierParams {
        spot,
        strike,
        barrier,
        time,
        rate,
        div_yield,
        vol,
    } = *params;

    validate_barrier_inputs(spot, strike, barrier)?;

    if time <= 0.0 {
        return Ok(0.0);
    }

    if vol <= 0.0 {
        return Ok(barrier_helper_zero_vol(params, eta, phi));
    }

    // Every in-the-money terminal path has already crossed these barriers.
    // Preserve this exact payoff identity independently of reflection numerics.
    if (phi > 0.0 && eta > 0.0 && strike >= barrier)
        || (phi < 0.0 && eta < 0.0 && strike <= barrier)
    {
        return Ok(vanilla_option_price(
            spot, strike, time, rate, div_yield, vol, eta,
        ));
    }

    let mu = (rate - div_yield - 0.5 * vol * vol) / (vol * vol);
    let log_barrier_spot = (barrier / spot).ln();
    let sigma_sqrt_t = vol * time.sqrt();

    // d1/d2 intentionally inline: Merton barrier x,x1,y,y1 terms — not d1/d2
    let x = (spot / strike).ln() / sigma_sqrt_t + (1.0 + mu) * sigma_sqrt_t;
    let x1 = -log_barrier_spot / sigma_sqrt_t + (1.0 + mu) * sigma_sqrt_t;
    let y =
        (2.0 * log_barrier_spot + (spot / strike).ln()) / sigma_sqrt_t + (1.0 + mu) * sigma_sqrt_t;
    let y1 = log_barrier_spot / sigma_sqrt_t + (1.0 + mu) * sigma_sqrt_t;

    let discount = (-rate * time).exp();
    let forward_discount = (-div_yield * time).exp();

    let option_sign = eta;
    let barrier_sign = -phi;

    let a = option_sign * spot * forward_discount * norm_cdf(option_sign * x)?
        - option_sign * strike * discount * norm_cdf(option_sign * (x - vol * time.sqrt()))?;

    let b = option_sign * spot * forward_discount * norm_cdf(option_sign * x1)?
        - option_sign * strike * discount * norm_cdf(option_sign * (x1 - vol * time.sqrt()))?;

    // Evaluate powers and Gaussian tails together in log space. The powers
    // alone can overflow while the complete reflected claims remain finite.
    let log_asset_reflection = spot.ln() - div_yield * time + 2.0 * (mu + 1.0) * log_barrier_spot;
    let log_cash_reflection = strike.ln() - rate * time + 2.0 * mu * log_barrier_spot;
    let c = option_sign
        * ((log_asset_reflection + log_norm_cdf(barrier_sign * y)?).exp()
            - (log_cash_reflection + log_norm_cdf(barrier_sign * (y - sigma_sqrt_t))?).exp());
    let d = option_sign
        * ((log_asset_reflection + log_norm_cdf(barrier_sign * y1)?).exp()
            - (log_cash_reflection + log_norm_cdf(barrier_sign * (y1 - sigma_sqrt_t))?).exp());

    let is_call = eta > 0.0;

    // Combine based on barrier type AND strike-vs-barrier regime.
    //
    // Formula decomposition follows Haug (2007) Table 4.17.
    // This helper computes the knock-IN value; the public knock-OUT wrappers
    // use `vanilla - knock_in`.
    //
    // Notation: A = vanilla, B = vanilla capped at barrier, C = reflected term,
    //           D = reflected capped term.
    let knock_in = if spot > barrier {
        // DOWN barrier
        if is_call {
            if strike >= barrier { c } else { a - b + d }
        } else if strike >= barrier {
            b - c + d
        } else {
            a
        }
    } else {
        // UP barrier
        if is_call {
            if strike <= barrier { b - c + d } else { a }
        } else if strike <= barrier {
            c
        } else {
            a - b + d
        }
    };
    Ok(knock_in)
}

/// Calculate probability of hitting the barrier before T (Touch Probability).
///
/// Returns P(min S < H) for down barrier, or P(max S > H) for up barrier.
///
/// Returns NaN when the input is non-finite, the levels are non-positive,
/// or the Gaussian reflection cannot be evaluated.
///
/// # Arguments
///
/// * `spot` - Current underlying spot price in the barrier's quote units.
/// * `barrier` - Continuously monitored trigger level in the same units as
///   `spot`.
/// * `time` - Remaining monitoring horizon in years.
/// * `rate` - Continuously compounded domestic risk-free rate as a decimal.
/// * `div_yield` - Continuously compounded dividend yield or foreign-rate
///   carry as a decimal.
/// * `vol` - Annualized lognormal volatility as a decimal.
/// * `is_up` - `true` for an upper barrier and `false` for a lower barrier.
pub fn barrier_touch_probability(
    spot: f64,
    barrier: f64,
    time: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    is_up: bool,
) -> f64 {
    if ![spot, barrier, time, rate, div_yield, vol]
        .iter()
        .all(|v| v.is_finite())
        || spot <= 0.0
        || barrier <= 0.0
    {
        return f64::NAN;
    }
    if (is_up && spot >= barrier) || (!is_up && spot <= barrier) {
        return 1.0;
    }

    if time <= 0.0 {
        return if deterministic_barrier_crossed(spot, barrier, time, rate, div_yield, is_up) {
            1.0
        } else {
            0.0
        };
    }

    if vol <= 0.0 {
        return if deterministic_barrier_crossed(spot, barrier, time, rate, div_yield, is_up) {
            1.0
        } else {
            0.0
        };
    }

    let sigma_sqrt_t = vol * time.sqrt();
    let nu = rate - div_yield - 0.5 * vol * vol;
    let log_barrier_ratio = (barrier / spot).ln();
    let log_power = 2.0 * nu * log_barrier_ratio / (vol * vol);

    let (d1, d2) = if is_up {
        // Up barrier (H > S)
        // P(max S > H)
        let x = log_barrier_ratio; // Positive
        (
            (-x + nu * time) / sigma_sqrt_t,
            (-x - nu * time) / sigma_sqrt_t,
        )
    } else {
        // Down barrier (H < S)
        // P(min S < H) — first-passage formula (Shreve, Theorem 7.2.1):
        //   P = N((h − νT)/(σ√T)) + e^{2νh/σ²} N((h + νT)/(σ√T))
        // Note: the ±ν signs differ from the up-barrier case because
        // P(min ≤ h) and P(max ≥ h) are structurally different formulas.
        let log_h_s = log_barrier_ratio; // Negative
        (
            (log_h_s - nu * time) / sigma_sqrt_t,
            (log_h_s + nu * time) / sigma_sqrt_t,
        )
    };
    (norm_cdf_or_nan(d1) + (log_power + log_norm_cdf_or_nan(d2)).exp()).clamp(0.0, 1.0)
}

/// Calculate value of a rebate paid at maturity.
///
/// - For Knock-Out: Paid if barrier is hit (Hit Rebate).
/// - For Knock-In: Paid if barrier is NOT hit (No-Hit Rebate).
///
/// `params.strike` is unused by this function; callers can pass any value
/// (conventionally `params.barrier`) when building the parameter bag.
///
/// # Arguments
///
/// * `params` - Barrier market parameters; only spot, barrier, time, rates,
///   carry, and volatility affect the rebate value.
/// * `rebate` - Cash amount paid under the rebate condition in option-price
///   currency units.
/// * `barrier_type` - Knock-in or knock-out direction and activation
///   convention controlling whether hit or no-hit earns the rebate.
pub(super) fn barrier_rebate_at_expiry(
    params: &BarrierParams,
    rebate: f64,
    barrier_type: BarrierType,
) -> f64 {
    let is_up = barrier_type.is_up();
    let p_hit = barrier_touch_probability(
        params.spot,
        params.barrier,
        params.time,
        params.rate,
        params.div_yield,
        params.vol,
        is_up,
    );

    let df = if params.time <= 0.0 {
        1.0
    } else {
        (-params.rate * params.time).exp()
    };

    match barrier_type {
        BarrierType::UpAndIn | BarrierType::DownAndIn => {
            // Knock-In: Option activates if Hit.
            // Rebate paid if it fails to activate (Not Hit).
            rebate * df * (1.0 - p_hit)
        }
        BarrierType::UpAndOut | BarrierType::DownAndOut => {
            // Knock-Out: Option deactivates if Hit.
            // Rebate paid if it deactivates (Hit).
            rebate * df * p_hit
        }
    }
}

/// Calculate value of a barrier rebate with explicit payment timing.
///
/// - Knock-in (no-hit) rebates are determined only at expiry, so the only
///   supported timing is [`PayoutTiming::AtExpiry`]. Knock-in with
///   [`PayoutTiming::AtHit`] is unsupported and returns a `NaN` sentinel; it
///   is never silently repriced at expiry.
/// - Knock-out rebates pay when the barrier is hit. With
///   [`PayoutTiming::AtExpiry`] the payment is deferred to expiry; with
///   [`PayoutTiming::AtHit`] the rebate is valued as
///   `rebate · E[e^{-r·τ} 1{τ≤T}]`
///   via the Rubinstein–Reiner discounted first-passage value.
///
/// Negative rates are supported, including the region where the usual
/// real-valued closed form has an imaginary square root. In that region,
/// the discounted first-passage expectation is evaluated by quadrature.
/// Returns a `NaN` sentinel if that quadrature fails to converge, or for the
/// unsupported knock-in/`AtHit` combination.
///
/// # Arguments
///
/// * `params` - Barrier market parameters used for hit probability and
///   discounting; strike is not used for the standalone rebate.
/// * `rebate` - Cash amount paid under the rebate condition in option-price
///   currency units.
/// * `barrier_type` - Knock-in or knock-out direction and activation
///   convention controlling whether hit or no-hit earns the rebate.
/// * `timing` - Whether a knock-out rebate is paid at expiry or at the first
///   hit. Knock-in rebates require [`PayoutTiming::AtExpiry`].
pub fn barrier_rebate(
    params: &BarrierParams,
    rebate: f64,
    barrier_type: BarrierType,
    timing: PayoutTiming,
) -> f64 {
    match (barrier_type, timing) {
        (BarrierType::UpAndIn | BarrierType::DownAndIn, PayoutTiming::AtHit) => f64::NAN,
        (_, PayoutTiming::AtExpiry) => barrier_rebate_at_expiry(params, rebate, barrier_type),
        (BarrierType::UpAndOut | BarrierType::DownAndOut, PayoutTiming::AtHit) => {
            let is_up = barrier_type.is_up();
            rebate * discounted_touch_value(params, is_up)
        }
    }
}

/// Validate a barrier product's rebate and its payment timing together.
///
/// A rebate requires an explicit timing, and a timing requires a rebate: no
/// timing is invented for cash that does not exist. A knock-in (no-hit)
/// rebate is only determined at expiry, so it must use
/// [`PayoutTiming::AtExpiry`]; [`PayoutTiming::AtHit`] is rejected rather
/// than silently repriced.
///
/// # Arguments
///
/// * `product` - Product name used to prefix the error message, for example
///   `"BarrierOption"`.
/// * `barrier_type` - Contractual barrier classification; only its in/out
///   sense matters here.
/// * `has_rebate` - Whether the product carries a rebate amount (including an
///   explicit zero amount).
/// * `timing` - The product's selected rebate payment timing, or `None` when
///   the product has no rebate.
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] when a rebate has no
/// timing, a timing has no rebate, or a knock-in rebate selects `AtHit`.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::types::{BarrierType, PayoutTiming};
/// use finstack_quant_models::closed_form::validate_rebate_timing;
///
/// assert!(validate_rebate_timing("Example", BarrierType::UpAndOut, true, Some(PayoutTiming::AtHit)).is_ok());
/// assert!(validate_rebate_timing("Example", BarrierType::DownAndIn, true, Some(PayoutTiming::AtHit)).is_err());
/// assert!(validate_rebate_timing("Example", BarrierType::UpAndOut, true, None).is_err());
/// assert!(validate_rebate_timing("Example", BarrierType::UpAndOut, false, None).is_ok());
/// ```
pub fn validate_rebate_timing(
    product: &str,
    barrier_type: BarrierType,
    has_rebate: bool,
    timing: Option<PayoutTiming>,
) -> finstack_quant_core::Result<()> {
    match (has_rebate, timing) {
        (true, None) => Err(finstack_quant_core::Error::Validation(format!(
            "{product} rebate requires an explicit rebate_timing (at_hit or at_expiry)"
        ))),
        (false, Some(_)) => Err(finstack_quant_core::Error::Validation(format!(
            "{product} rebate_timing requires a rebate"
        ))),
        (true, Some(PayoutTiming::AtHit)) if barrier_type.is_knock_in() => {
            Err(finstack_quant_core::Error::Validation(format!(
                "{product} knock-in rebates are paid only if no hit occurred, so rebate_timing must be at_expiry"
            )))
        }
        (false, None) | (true, Some(_)) => Ok(()),
    }
}

/// Expiry-slice payoff of a barrier option whose monitoring window has closed.
///
/// The result covers only the amount payable at expiry. An at-hit knock-out
/// rebate is excluded because it was payable at the hit; whether that cash
/// was actually settled belongs to the product's cash-flow and settlement
/// records, not to this helper.
///
/// | Barrier | Breached | Payoff |
/// |---|---|---|
/// | knock-out | no | `intrinsic` |
/// | knock-out | yes | rebate amount at [`PayoutTiming::AtExpiry`]; `0.0` at [`PayoutTiming::AtHit`] (payable at the hit, outside the expiry slice) or with no rebate |
/// | knock-in | yes | `intrinsic` |
/// | knock-in | no | rebate amount at [`PayoutTiming::AtExpiry`]; `0.0` with no rebate; `NaN` sentinel at [`PayoutTiming::AtHit`] (unsupported) |
///
/// # Arguments
///
/// * `barrier_type` - Knock-in or knock-out direction of the contract.
/// * `breached` - Whether the barrier was observed hit during monitoring.
/// * `intrinsic` - Exercise value at expiry (`max(S - K, 0)` or
///   `max(K - S, 0)`), on the caller's scale (per unit or total).
/// * `rebate` - The contract's rebate as `(amount, timing)`, with the amount
///   on the same scale as `intrinsic`; `None` when the contract has no
///   rebate. Knock-in (no-hit) rebates require [`PayoutTiming::AtExpiry`].
///
/// # Returns
///
/// The undiscounted amount the holder receives at expiry, on the scale of
/// the inputs.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::types::{BarrierType, PayoutTiming};
/// use finstack_quant_models::closed_form::expired_barrier_payoff;
///
/// let knocked_out = expired_barrier_payoff(
///     BarrierType::UpAndOut,
///     true,
///     5.0,
///     Some((1.0, PayoutTiming::AtExpiry)),
/// );
/// assert_eq!(knocked_out, 1.0);
/// let knocked_in = expired_barrier_payoff(BarrierType::DownAndIn, true, 5.0, None);
/// assert_eq!(knocked_in, 5.0);
/// ```
pub fn expired_barrier_payoff(
    barrier_type: BarrierType,
    breached: bool,
    intrinsic: f64,
    rebate: Option<(f64, PayoutTiming)>,
) -> f64 {
    match (barrier_type.is_knock_out(), breached, rebate) {
        (true, false, _) | (false, true, _) => intrinsic,
        (_, _, None) | (true, true, Some((_, PayoutTiming::AtHit))) => 0.0,
        (_, _, Some((amount, PayoutTiming::AtExpiry))) => amount,
        (false, false, Some((_, PayoutTiming::AtHit))) => f64::NAN,
    }
}

/// Discounted first-passage value `E[e^{-r·τ} 1{τ≤T}]` for a continuously
/// monitored barrier (Rubinstein–Reiner one-touch, pay-at-hit).
///
/// Uses `μ = (r − q − σ²/2)/σ²` and `λ = √(μ² + 2r/σ²)`:
///
/// ```text
/// V = (S/H)^{−(μ+λ)} N(ηz) + (S/H)^{−(μ−λ)} N(ηz′)
/// z  = ln(H/S)/(σ√T) + λσ√T,   z′ = ln(H/S)/(σ√T) − λσ√T
/// η  = +1 (down barrier), −1 (up barrier)
/// ```
///
/// Already-breached barriers return `1.0` (immediate payment). Zero volatility
/// discounts at the deterministic crossing time. When `μ² + 2r/σ² < 0`,
/// integrate the first-passage distribution instead of using a complex root.
pub(super) fn discounted_touch_value(params: &BarrierParams, is_up: bool) -> f64 {
    let BarrierParams {
        spot,
        barrier,
        time,
        rate,
        div_yield,
        vol,
        ..
    } = *params;

    let already_breached = if is_up {
        spot >= barrier
    } else {
        spot <= barrier
    };
    if already_breached {
        return 1.0;
    }
    if time <= 0.0 {
        return 0.0;
    }
    let log_hs = (barrier / spot).ln();
    if vol <= 0.0 {
        if !deterministic_barrier_crossed(spot, barrier, time, rate, div_yield, is_up) {
            return 0.0;
        }
        let hit_time = log_hs / (rate - div_yield);
        return (-rate * hit_time).exp();
    }

    let sigma_sqrt_t = vol * time.sqrt();
    let sigma2 = vol * vol;
    let nu = rate - div_yield - sigma2 / 2.0;
    let discriminant = nu * nu + 2.0 * rate * sigma2;
    if discriminant < 0.0 {
        // For r < 0, integration by parts gives the nonnegative decomposition
        // V = F(T) + (-r) ∫₀ᵀ exp(-rt) [F(T) - F(t)] dt.
        // Normalize by F(T) and integrate over [0,1] for relative accuracy.
        let hit_probability =
            barrier_touch_probability(spot, barrier, time, rate, div_yield, vol, is_up);
        if hit_probability == 0.0 {
            return 0.0;
        }
        let integral = adaptive_simpson(
            |u| {
                let p =
                    barrier_touch_probability(spot, barrier, time * u, rate, div_yield, vol, is_up);
                Ok((-rate * time * u).exp() * (1.0 - p / hit_probability).clamp(0.0, 1.0))
            },
            0.0,
            1.0,
            1e-12,
            48,
            REBATE_QUADRATURE_MAX_EVALS,
        );
        return integral
            .map(|value| hit_probability * (1.0 - rate * time * value))
            .unwrap_or(f64::NAN);
    }
    let root = discriminant.sqrt();

    let z = (log_hs + root * time) / sigma_sqrt_t;
    let z_prime = (log_hs - root * time) / sigma_sqrt_t;
    let eta = if is_up { -1.0 } else { 1.0 };
    // Rationalize the cancelling root when volatility is small: ν² - root²
    // equals -2rσ², so the small exponent retains its deterministic limit.
    let power_plus = if nu < 0.0 {
        -2.0 * rate / (nu - root)
    } else {
        (nu + root) / sigma2
    };
    let power_minus = if nu > 0.0 {
        -2.0 * rate / (nu + root)
    } else {
        (nu - root) / sigma2
    };
    let value = (power_plus * log_hs + log_norm_cdf_or_nan(eta * z)).exp()
        + (power_minus * log_hs + log_norm_cdf_or_nan(eta * z_prime)).exp();
    if value.is_finite() {
        value.clamp(0.0, 1.0_f64.max((-rate * time).exp()))
    } else {
        f64::NAN
    }
}

/// Price a continuously monitored European barrier option (Reiner-Rubinstein).
///
/// Covers all eight up/down, knock-in/knock-out, call/put combinations. A
/// knock-out is priced as `vanilla - knock-in` and floored at zero, because
/// catastrophic cancellation near the barrier can produce tiny negative
/// values. When `spot` is already on or beyond the barrier, a knock-in is the
/// vanilla option and a knock-out is worth zero (rebates are priced
/// separately by [`barrier_rebate`]).
///
/// Returns a non-finite (`NaN`) sentinel — never a plausible finite price — if
/// `spot`, `strike` or `barrier` is not strictly positive and finite. Such
/// corrupt inputs cannot produce a meaningful barrier price; the `NaN` is
/// rejected by downstream `is_finite()` guards rather than silently mispriced.
///
/// # Arguments
///
/// * `params` - Spot, strike and barrier level in the same quote units, time
///   to expiry in years, continuously compounded risk-free rate and dividend
///   yield (or foreign-rate carry) as decimals, and annualized lognormal
///   volatility as a decimal.
/// * `barrier_type` - Up/down and knock-in/knock-out convention. "Up" means the
///   barrier is hit when spot rises to it, "down" when spot falls to it.
/// * `option_type` - Call or put payoff at expiry.
///
/// # Examples
///
/// ```
/// use finstack_quant_core::types::BarrierType;
/// use finstack_quant_models::closed_form::{barrier_price, BarrierParams};
/// use finstack_quant_models::types::OptionType;
///
/// // spot 100, strike 100, barrier 90, 1y, r = 5%, q = 2%, vol = 20%
/// let params = BarrierParams::new(100.0, 100.0, 90.0, 1.0, 0.05, 0.02, 0.20);
/// let knock_out = barrier_price(&params, BarrierType::DownAndOut, OptionType::Call);
/// let knock_in = barrier_price(&params, BarrierType::DownAndIn, OptionType::Call);
///
/// // The knock-out feature reduces value relative to the vanilla call.
/// assert!(knock_out >= 0.0 && knock_in >= 0.0);
/// ```
pub fn barrier_price(
    params: &BarrierParams,
    barrier_type: BarrierType,
    option_type: OptionType,
) -> f64 {
    let BarrierParams {
        spot,
        strike,
        barrier,
        time,
        rate,
        div_yield,
        vol,
    } = *params;
    if validate_barrier_inputs(spot, strike, barrier).is_err() {
        return f64::NAN;
    }
    // eta: +1 call / -1 put. phi: +1 upper barrier / -1 lower barrier.
    let eta = match option_type {
        OptionType::Call => 1.0,
        OptionType::Put => -1.0,
    };
    let (is_up, is_in) = match barrier_type {
        BarrierType::UpAndIn => (true, true),
        BarrierType::UpAndOut => (true, false),
        BarrierType::DownAndIn => (false, true),
        BarrierType::DownAndOut => (false, false),
    };
    let phi = if is_up { 1.0 } else { -1.0 };
    let already_hit = if is_up {
        spot >= barrier
    } else {
        spot <= barrier
    };

    if is_in {
        if already_hit {
            // Already knocked in, price as vanilla
            return vanilla_option_price(spot, strike, time, rate, div_yield, vol, eta);
        }
        return barrier_helper(params, eta, phi).unwrap_or(f64::NAN);
    }

    if already_hit {
        return 0.0; // Already knocked out
    }
    // Knock-out = vanilla - knock-in, clamped to zero.
    let vanilla = vanilla_option_price(spot, strike, time, rate, div_yield, vol, eta);
    match barrier_helper(params, eta, phi) {
        Ok(knock_in) => (vanilla - knock_in).clamp(0.0, f64::INFINITY),
        Err(_) => f64::NAN,
    }
}

//
// When the discount factor is known directly (e.g., from date-based curve
// lookup), callers should build the parameter bag via
// [`BarrierParams::with_df`], which derives `rate = -ln(df)/t`. The
// resulting `BarrierParams` can be passed to
// [`barrier_price`] or [`barrier_rebate`].
