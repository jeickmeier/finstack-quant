//! Closed-form Black-76 analytical Greeks shared across European commodity options
//! and swaptions.

use finstack_quant_core::Result;
use finstack_quant_core::math::special_functions::norm_pdf;
use finstack_quant_models::closed_form::{black76_greeks, d1_d2_black76};
use finstack_quant_models::types::OptionType;
use finstack_quant_valuations_core::instruments::OptionGreeks;

/// Compute analytical Black-76 cash Greeks (delta, gamma, vega, vanna, volga) in one pass.
///
/// Unit conventions match [`finstack_quant_valuations_core::instruments::OptionGreeks`]:
/// - `delta`: cash sensitivity per 1.0 unit forward move (`0.0` at expiry or zero vol when `F == K`)
/// - `gamma`: second derivative with respect to the forward (`0.0` at expiry or zero vol)
/// - `vega`: cash sensitivity per 1 vol point (`+0.01` absolute lognormal vol)
/// - `vanna`: mixed derivative per 1.0 forward move and 1 vol point
/// - `volga`: second derivative per `(1 vol point)^2`
///
/// # Arguments
///
/// * `forward` - Forward price; finite and strictly positive.
/// * `strike` - Strike or fixed price; finite and strictly positive.
/// * `discount_or_annuity` - Discount factor (or swap annuity) applied to undiscounted Greeks.
/// * `sigma` - Annualized lognormal Black volatility as a decimal; finite and non-negative.
/// * `time` - Time to expiry in years; finite and non-negative.
/// * `option_type` - Call or put payoff direction.
/// * `scale` - Contract notional multiplier (`quantity * multiplier`).
pub(crate) fn black76_option_greeks(
    forward: f64,
    strike: f64,
    discount_or_annuity: f64,
    sigma: f64,
    time: f64,
    option_type: OptionType,
    scale: f64,
) -> Result<OptionGreeks> {
    if time <= 0.0 || sigma <= 0.0 {
        let intrinsic = match option_type {
            OptionType::Call => f64::from(forward > strike),
            OptionType::Put => -f64::from(forward < strike),
        };
        let df_scale = if time <= 0.0 {
            1.0
        } else {
            discount_or_annuity
        };
        return Ok(OptionGreeks {
            delta: Some(intrinsic * df_scale * scale),
            gamma: Some(0.0),
            vega: Some(0.0),
            theta: None,
            rho_bp: None,
            foreign_rho_bp: None,
            vanna: Some(0.0),
            volga: Some(0.0),
        });
    }

    let g = black76_greeks(forward, strike, sigma, time, option_type)?;
    let (d1, d2) = d1_d2_black76(forward, strike, sigma, time);
    let pdf_d1 = norm_pdf(d1)?;
    let total_scale = discount_or_annuity * scale;

    Ok(OptionGreeks {
        delta: Some(total_scale * g.delta),
        gamma: Some(total_scale * g.gamma),
        vega: Some(total_scale * g.vega * 0.01),
        theta: None,
        rho_bp: None,
        foreign_rho_bp: None,
        vanna: Some(-total_scale * pdf_d1 * (d2 / sigma) * 0.01),
        volga: Some(total_scale * g.vega * (d1 * d2 / sigma) * 1e-4),
    })
}
