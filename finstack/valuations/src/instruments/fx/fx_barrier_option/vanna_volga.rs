//! Vanna-Volga method for FX barrier option pricing.
//!
//! The Vanna-Volga method (Castagna & Mercurio, 2007) provides a smile-consistent
//! correction to Black-Scholes barrier prices by replicating the option's vanna
//! and volga using three vanilla options at standard market quotes (25Δ put, ATM, 25Δ call).
//!
//! The corrected price is:
//! ```text
//! P_VV = P_BS(σ_ATM) + p₁ × [C₁(σ₁) - C₁(σ_ATM)]
//!                      + p₂ × [C₂(σ₂) - C₂(σ_ATM)]
//!                      + p₃ × [C₃(σ₃) - C₃(σ_ATM)]
//! ```
//!
//! where the weights p₁, p₂, p₃ are determined by matching the vanna and volga
//! of the barrier option to a linear combination of the three vanillas.
//!
//! # The simplified (first-order) Vanna-Volga correction:
//!
//! ```text
//! P_VV ≈ P_BS(σ_ATM) + Vanna_barrier × (Cost_of_Vanna) + Volga_barrier × (Cost_of_Volga)
//! ```
//!
//! where:
//! - `Cost_of_Vanna = x₁(σ₁ - σ_ATM) + x₃(σ₃ - σ_ATM)` — smile cost of vanna
//! - `Cost_of_Volga = x₁(σ₁ - σ_ATM)² + x₃(σ₃ - σ_ATM)²` — smile cost of volga
//!
//! # References
//!
//! - Castagna, A. & Mercurio, F. (2007). "The Vanna-Volga Method for Implied
//!   Volatilities." Risk, January 2007.
//! - Wystup, U. (2006). "FX Options and Structured Products." Wiley.

use crate::instruments::common_impl::parameters::OptionType;
use crate::models::closed_form::barrier::{
    barrier_call_continuous, barrier_put_continuous, barrier_rebate, barrier_touch_probability,
    BarrierParams, BarrierType as AnalyticalBarrierType, RebateTiming,
};
use crate::models::closed_form::vanilla::bs_price;
use crate::models::volatility::black::d1_d2;

/// Optional rebate leg priced alongside the barrier payoff in the
/// Vanna-Volga finite-difference greeks.
///
/// The base BS leg (`bs_barrier_price_per_unit` in the pricer) includes the
/// rebate; the FD vega/vanna/volga matched by the 3×3 system must cover the
/// same payoff or the smile correction hedges a different instrument.
#[derive(Debug, Clone, Copy)]
pub struct VvRebate {
    /// Rebate amount per unit notional.
    pub amount: f64,
    /// Rebate payment timing convention.
    pub timing: RebateTiming,
}

/// Survival-probability weight for the Vanna-Volga barrier correction.
///
/// Castagna & Mercurio (2007) / Wystup (2006, §3) weight the barrier
/// Vanna-Volga correction by a *survival probability* `p ∈ [0, 1]` so the
/// smile correction vanishes as the barrier option degenerates:
///
/// - **Knock-out** (`UpOut`/`DownOut`): the option only survives if the
///   barrier is never touched, so `p = 1 − P(touch)` (the no-touch
///   probability). As the barrier becomes certain to knock out
///   (`P(touch) → 1`), `p → 0` and the correction → 0 — a dead option has no
///   smile exposure.
/// - **Knock-in** (`UpIn`/`DownIn`): the option only comes alive if the
///   barrier *is* touched, so `p = P(touch)`. As knock-in becomes impossible
///   (`P(touch) → 0`), `p → 0`.
///
/// The touch probability is taken under the Black-Scholes dynamics at the ATM
/// volatility (the same vol the BS barrier leg uses).
fn vv_survival_weight(
    params: &BarrierParams,
    barrier_type: AnalyticalBarrierType,
    sigma_atm: f64,
) -> f64 {
    let is_up = matches!(
        barrier_type,
        AnalyticalBarrierType::UpIn | AnalyticalBarrierType::UpOut
    );
    let touch_prob = barrier_touch_probability(
        params.spot,
        params.barrier,
        params.time,
        params.rate,
        params.div_yield,
        sigma_atm,
        is_up,
    );
    let survival = match barrier_type {
        // Knock-out survives only on no-touch.
        AnalyticalBarrierType::UpOut | AnalyticalBarrierType::DownOut => 1.0 - touch_prob,
        // Knock-in activates only on touch.
        AnalyticalBarrierType::UpIn | AnalyticalBarrierType::DownIn => touch_prob,
    };
    survival.clamp(0.0, 1.0)
}

/// Market quotes for the Vanna-Volga method (three-point smile).
#[derive(Debug, Clone, Copy)]
pub struct VannaVolgaQuotes {
    /// 25-delta put volatility
    pub vol_25d_put: f64,
    /// ATM (delta-neutral straddle) volatility
    pub vol_atm: f64,
    /// 25-delta call volatility
    pub vol_25d_call: f64,
    /// 25-delta put strike
    pub strike_25d_put: f64,
    /// ATM strike
    pub strike_atm: f64,
    /// 25-delta call strike
    pub strike_25d_call: f64,
}

/// Compute vanilla BS price for a given strike, vol, and option parameters.
///
/// For the VV method we always price as calls for the upper strikes and puts
/// for the lower strikes, but the put-call parity means the smile cost is the
/// same regardless. We use call prices throughout for simplicity.
fn vanilla_call(spot: f64, strike: f64, r_d: f64, r_f: f64, vol: f64, t: f64) -> f64 {
    bs_price(spot, strike, r_d, r_f, vol, t, OptionType::Call)
}

/// Compute BS vega for a vanilla option (∂C/∂σ).
///
/// vega = S × e^{-r_f × T} × φ(d₁) × √T
fn bs_vega(spot: f64, strike: f64, r_d: f64, r_f: f64, vol: f64, t: f64) -> f64 {
    if t <= 0.0 || vol <= 0.0 {
        return 0.0;
    }
    let (d1, _d2) = d1_d2(spot, strike, r_d, vol, t, r_f);
    let pdf_d1 = finstack_core::math::norm_pdf(d1);
    spot * (-r_f * t).exp() * pdf_d1 * t.sqrt()
}

/// Compute BS vanna for a vanilla option (∂²C/∂S∂σ).
///
/// vanna = -e^{-r_f × T} × φ(d₁) × d₂ / σ
fn bs_vanna(spot: f64, strike: f64, r_d: f64, r_f: f64, vol: f64, t: f64) -> f64 {
    if t <= 0.0 || vol <= 0.0 {
        return 0.0;
    }
    let (d1, d2) = d1_d2(spot, strike, r_d, vol, t, r_f);
    let pdf_d1 = finstack_core::math::norm_pdf(d1);
    -(-r_f * t).exp() * pdf_d1 * d2 / vol
}

/// Compute BS volga for a vanilla option (∂²C/∂σ²).
///
/// volga = vega × d₁ × d₂ / σ
fn bs_volga(spot: f64, strike: f64, r_d: f64, r_f: f64, vol: f64, t: f64) -> f64 {
    if t <= 0.0 || vol <= 0.0 {
        return 0.0;
    }
    let (d1, d2) = d1_d2(spot, strike, r_d, vol, t, r_f);
    let vega = bs_vega(spot, strike, r_d, r_f, vol, t);
    vega * d1 * d2 / vol
}

/// Compute barrier option price using BS model at a given vol, including the
/// optional rebate leg so FD greeks cover the same payoff as the base leg.
fn barrier_bs_price(
    params: &BarrierParams,
    barrier_type: AnalyticalBarrierType,
    is_call: bool,
    rebate: Option<VvRebate>,
) -> f64 {
    let option_leg = if is_call {
        barrier_call_continuous(params, barrier_type)
    } else {
        barrier_put_continuous(params, barrier_type)
    };
    let rebate_leg = rebate
        .map(|r| barrier_rebate(params, r.amount, barrier_type, r.timing))
        .unwrap_or(0.0);
    option_leg + rebate_leg
}

/// Finite-difference bump sizes for the barrier FD greeks, guarded so the
/// bumped scenarios stay in the same regime as the base point:
///
/// - the vol bump shrinks near zero vol so `vol - h_vol` cannot go negative
///   (a negative vol input would silently price garbage);
/// - the spot bump is capped at half the distance to the barrier so
///   `spot ± h_spot` cannot cross it — an FD straddling the barrier mixes
///   knocked and alive regimes and produces a meaningless cross-derivative.
fn fd_bumps(params: &BarrierParams) -> (f64, f64) {
    let h_vol = 0.001_f64.min(0.5 * params.vol).max(1e-8);
    let barrier_dist = (params.spot - params.barrier).abs();
    let h_spot = (params.spot * 0.001)
        .min(0.5 * barrier_dist)
        .max(params.spot * 1e-7);
    (h_spot, h_vol)
}

/// Compute barrier vanna via central finite differences on the BS barrier formula.
///
/// vanna_barrier = ∂²P_barrier / (∂S × ∂σ)
///
/// We use a cross-derivative finite difference:
/// vanna ≈ [P(S+h, σ+k) - P(S+h, σ-k) - P(S-h, σ+k) + P(S-h, σ-k)] / (4 h k)
fn barrier_vanna_fd(
    params: &BarrierParams,
    barrier_type: AnalyticalBarrierType,
    is_call: bool,
    rebate: Option<VvRebate>,
) -> f64 {
    let (h_spot, h_vol) = fd_bumps(params);

    let p = |s: f64, v: f64| -> f64 {
        let bumped = BarrierParams {
            spot: s,
            vol: v,
            ..*params
        };
        barrier_bs_price(&bumped, barrier_type, is_call, rebate)
    };

    let ppp = p(params.spot + h_spot, params.vol + h_vol);
    let ppm = p(params.spot + h_spot, params.vol - h_vol);
    let pmp = p(params.spot - h_spot, params.vol + h_vol);
    let pmm = p(params.spot - h_spot, params.vol - h_vol);

    (ppp - ppm - pmp + pmm) / (4.0 * h_spot * h_vol)
}

/// Compute barrier volga via central finite differences on the BS barrier formula.
///
/// volga_barrier = ∂²P_barrier / ∂σ²
fn barrier_volga_fd(
    params: &BarrierParams,
    barrier_type: AnalyticalBarrierType,
    is_call: bool,
    rebate: Option<VvRebate>,
) -> f64 {
    let (_, h_vol) = fd_bumps(params);

    let bump_vol = |dv: f64| -> BarrierParams {
        BarrierParams {
            vol: params.vol + dv,
            ..*params
        }
    };

    let p_base = barrier_bs_price(params, barrier_type, is_call, rebate);
    let p_up = barrier_bs_price(&bump_vol(h_vol), barrier_type, is_call, rebate);
    let p_down = barrier_bs_price(&bump_vol(-h_vol), barrier_type, is_call, rebate);

    (p_up - 2.0 * p_base + p_down) / (h_vol * h_vol)
}

/// Compute barrier vega via central finite differences on the BS barrier formula.
///
/// vega_barrier = ∂P_barrier / ∂σ
fn barrier_vega_fd(
    params: &BarrierParams,
    barrier_type: AnalyticalBarrierType,
    is_call: bool,
    rebate: Option<VvRebate>,
) -> f64 {
    let (_, h_vol) = fd_bumps(params);

    let bump_vol = |dv: f64| -> BarrierParams {
        BarrierParams {
            vol: params.vol + dv,
            ..*params
        }
    };

    let p_up = barrier_bs_price(&bump_vol(h_vol), barrier_type, is_call, rebate);
    let p_down = barrier_bs_price(&bump_vol(-h_vol), barrier_type, is_call, rebate);

    (p_up - p_down) / (2.0 * h_vol)
}

/// Target vega/vanna/volga to be replicated by the three-pillar portfolio.
#[derive(Debug, Clone, Copy)]
struct VvTargetGreeks {
    /// Target vega (∂P/∂σ).
    vega: f64,
    /// Target vanna (∂²P/∂S∂σ).
    vanna: f64,
    /// Target volga (∂²P/∂σ²).
    volga: f64,
}

/// Solve the Vanna-Volga 3×3 system and return the smile cost `Σ pᵢ·costᵢ`
/// for a target instrument with the given vega/vanna/volga.
///
/// The weights `p₁, p₂, p₃` replicate the target's vega, vanna and volga with
/// the three pillar vanillas (25Δ put, ATM, 25Δ call), all valued at the ATM
/// vol; the cost of each pillar is its market-vs-ATM price difference.
/// Returns `0.0` (no adjustment) when the system is singular (degenerate
/// smile).
fn vv_smile_cost(
    spot: f64,
    r_d: f64,
    r_f: f64,
    t: f64,
    quotes: &VannaVolgaQuotes,
    target: VvTargetGreeks,
) -> f64 {
    let VvTargetGreeks {
        vega: vega_target,
        vanna: vanna_target,
        volga: volga_target,
    } = target;
    let sigma_atm = quotes.vol_atm;

    // Step 1: Compute vanilla costs for the three pillar instruments
    // Cost_i = C_i(σ_i) - C_i(σ_ATM) for each pillar strike
    let k1 = quotes.strike_25d_put;
    let k2 = quotes.strike_atm;
    let k3 = quotes.strike_25d_call;

    let cost_1 = vanilla_call(spot, k1, r_d, r_f, quotes.vol_25d_put, t)
        - vanilla_call(spot, k1, r_d, r_f, sigma_atm, t);
    // ATM cost is zero by definition: C₂(σ_ATM) - C₂(σ_ATM) = 0.
    // The p₂ weight still participates in the 3×3 system (matching vega/vanna/volga),
    // but its contribution to the final adjustment is zero since cost_2 = 0.
    let cost_2 = 0.0;
    let cost_3 = vanilla_call(spot, k3, r_d, r_f, quotes.vol_25d_call, t)
        - vanilla_call(spot, k3, r_d, r_f, sigma_atm, t);

    // Step 2: Compute vega, vanna, volga of the three vanillas at ATM vol
    let vega_1 = bs_vega(spot, k1, r_d, r_f, sigma_atm, t);
    let vega_2 = bs_vega(spot, k2, r_d, r_f, sigma_atm, t);
    let vega_3 = bs_vega(spot, k3, r_d, r_f, sigma_atm, t);

    let vanna_1 = bs_vanna(spot, k1, r_d, r_f, sigma_atm, t);
    let vanna_2 = bs_vanna(spot, k2, r_d, r_f, sigma_atm, t);
    let vanna_3 = bs_vanna(spot, k3, r_d, r_f, sigma_atm, t);

    let volga_1 = bs_volga(spot, k1, r_d, r_f, sigma_atm, t);
    let volga_2 = bs_volga(spot, k2, r_d, r_f, sigma_atm, t);
    let volga_3 = bs_volga(spot, k3, r_d, r_f, sigma_atm, t);

    // Step 3: Solve the 3×3 linear system for weights p₁, p₂, p₃:
    //   p₁ × vega₁ + p₂ × vega₂ + p₃ × vega₃ = vega_target
    //   p₁ × vanna₁ + p₂ × vanna₂ + p₃ × vanna₃ = vanna_target
    //   p₁ × volga₁ + p₂ × volga₂ + p₃ × volga₃ = volga_target
    //
    // We solve this via Cramer's rule for the 3×3 system.
    let det = determinant_3x3(
        vega_1, vega_2, vega_3, vanna_1, vanna_2, vanna_3, volga_1, volga_2, volga_3,
    );

    // If the system is singular (degenerate smile), apply no adjustment.
    if det.abs() < 1e-30 {
        return 0.0;
    }

    let p1 = determinant_3x3(
        vega_target,
        vega_2,
        vega_3,
        vanna_target,
        vanna_2,
        vanna_3,
        volga_target,
        volga_2,
        volga_3,
    ) / det;

    let p2 = determinant_3x3(
        vega_1,
        vega_target,
        vega_3,
        vanna_1,
        vanna_target,
        vanna_3,
        volga_1,
        volga_target,
        volga_3,
    ) / det;

    let p3 = determinant_3x3(
        vega_1,
        vega_2,
        vega_target,
        vanna_1,
        vanna_2,
        vanna_target,
        volga_1,
        volga_2,
        volga_target,
    ) / det;

    // Smile-cost portfolio, Σ pᵢ × costᵢ.
    p1 * cost_1 + p2 * cost_2 + p3 * cost_3
}

/// Vanna-Volga price of a **vanilla** FX option.
///
/// Base leg is the Black-Scholes vanilla at the **ATM** vol; the smile cost
/// (same 3×3 pillar replication as the barrier method) is added with weight 1
/// — a vanilla has no knock-out attenuation. This is the reference leg used
/// to price knock-in barriers via in–out parity.
///
/// Only `spot`, `strike`, `time`, `rate` and `div_yield` of `params` are
/// used; the ambient `vol` is ignored in favour of `quotes.vol_atm`.
pub fn vanna_volga_vanilla_adjustment(
    params: &BarrierParams,
    quotes: &VannaVolgaQuotes,
    is_call: bool,
) -> f64 {
    let BarrierParams {
        spot,
        strike,
        time: t,
        rate: r_d,
        div_yield: r_f,
        ..
    } = *params;

    let sigma_atm = quotes.vol_atm;
    let option_type = if is_call {
        OptionType::Call
    } else {
        OptionType::Put
    };
    let base = bs_price(spot, strike, r_d, r_f, sigma_atm, t, option_type);
    if t <= 0.0 {
        return base;
    }

    // Analytic BS greeks of the vanilla at ATM vol (call/put share vega,
    // vanna and volga).
    let target = VvTargetGreeks {
        vega: bs_vega(spot, strike, r_d, r_f, sigma_atm, t),
        vanna: bs_vanna(spot, strike, r_d, r_f, sigma_atm, t),
        volga: bs_volga(spot, strike, r_d, r_f, sigma_atm, t),
    };

    base + vv_smile_cost(spot, r_d, r_f, t, quotes, target)
}

/// Vanna-Volga price of an FX barrier option (per unit notional).
///
/// The base Black-Scholes leg — barrier payoff plus optional rebate — is
/// valued at the **ATM** volatility `quotes.vol_atm` (the ambient `params.vol`
/// is ignored): the Vanna-Volga construction prices everything off the ATM
/// leg and lets the 3×3 pillar replication carry the smile, so building the
/// base leg at the strike vol would double-count the smile.
///
/// - **Knock-out** (`UpOut`/`DownOut`): `BS(σ_ATM)` plus the smile cost of
///   the barrier's FD vega/vanna/volga, attenuated by the no-touch survival
///   probability (Wystup 2006, §3).
/// - **Knock-in** (`UpIn`/`DownIn`): priced via in–out parity,
///   `VV(KI) = VV(vanilla) − VV(paired KO)`, with the rebate handled
///   per-leg: the knock-in rebate (paid at expiry when the barrier is never
///   touched) is valued at the ATM vol and added to the parity result, while
///   the paired knock-out option leg carries no rebate. Pricing a KI
///   directly with survival-weighted attenuation mis-states the smile cost
///   because a knock-in's smile exposure *grows* with touch probability.
///
/// # Arguments
///
/// * `params` - Spot/strike/barrier/time/rates; `vol` is ignored (ATM is used)
/// * `quotes` - Market volatility quotes at the three pillar strikes
/// * `is_call` - Whether the option is a call
/// * `barrier_type` - The analytical barrier type
/// * `rebate` - Optional rebate leg, priced with the same conventions as the
///   base BS leg
///
/// # Returns
///
/// The Vanna-Volga barrier price per unit notional.
pub fn vanna_volga_barrier_price(
    params: &BarrierParams,
    quotes: &VannaVolgaQuotes,
    is_call: bool,
    barrier_type: AnalyticalBarrierType,
    rebate: Option<VvRebate>,
) -> f64 {
    let sigma_atm = quotes.vol_atm;
    let atm_params = BarrierParams {
        vol: sigma_atm,
        ..*params
    };

    match barrier_type {
        AnalyticalBarrierType::UpOut | AnalyticalBarrierType::DownOut => {
            // Base BS leg (option + rebate) at ATM vol.
            let bs_barrier_price = barrier_bs_price(&atm_params, barrier_type, is_call, rebate);
            if atm_params.time <= 0.0 {
                return bs_barrier_price;
            }

            // FD vega/vanna/volga of the barrier (including any rebate leg,
            // matching the base BS price).
            let target = VvTargetGreeks {
                vega: barrier_vega_fd(&atm_params, barrier_type, is_call, rebate),
                vanna: barrier_vanna_fd(&atm_params, barrier_type, is_call, rebate),
                volga: barrier_volga_fd(&atm_params, barrier_type, is_call, rebate),
            };

            let smile_cost = vv_smile_cost(
                atm_params.spot,
                atm_params.rate,
                atm_params.div_yield,
                atm_params.time,
                quotes,
                target,
            );

            // Weight by the survival probability. The Vanna-Volga correction
            // must vanish as the knock-out becomes certain: a dead option
            // carries no vanna/volga and therefore no smile cost. Omitting
            // this weight over-applies the smile correction near the barrier.
            let survival_weight = vv_survival_weight(&atm_params, barrier_type, sigma_atm);
            bs_barrier_price + survival_weight * smile_cost
        }
        AnalyticalBarrierType::UpIn | AnalyticalBarrierType::DownIn => {
            // In–out parity on the option leg: KI = vanilla − paired KO.
            let ko_type = if matches!(barrier_type, AnalyticalBarrierType::UpIn) {
                AnalyticalBarrierType::UpOut
            } else {
                AnalyticalBarrierType::DownOut
            };
            let vv_vanilla = vanna_volga_vanilla_adjustment(&atm_params, quotes, is_call);
            let vv_ko = vanna_volga_barrier_price(&atm_params, quotes, is_call, ko_type, None);

            // Per-leg rebate: a knock-in rebate pays when the barrier is
            // never touched, which is not part of the parity identity; value
            // it at the ATM vol alongside the base legs.
            let rebate_leg = rebate
                .map(|r| barrier_rebate(&atm_params, r.amount, barrier_type, r.timing))
                .unwrap_or(0.0);

            vv_vanilla - vv_ko + rebate_leg
        }
    }
}

/// Compute 3×3 determinant.
///
/// | a11 a12 a13 |
/// | a21 a22 a23 |
/// | a31 a32 a33 |
#[inline]
#[allow(clippy::too_many_arguments)]
fn determinant_3x3(
    a11: f64,
    a12: f64,
    a13: f64,
    a21: f64,
    a22: f64,
    a23: f64,
    a31: f64,
    a32: f64,
    a33: f64,
) -> f64 {
    a11 * (a22 * a33 - a23 * a32) - a12 * (a21 * a33 - a23 * a31) + a13 * (a21 * a32 - a22 * a31)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// For a flat vol surface, the VV correction should be zero since all
    /// three pillar vols equal ATM vol, making all vanilla costs zero.
    #[test]
    fn test_vv_correction_zero_for_flat_vol() {
        let spot = 1.10;
        let strike = 1.10;
        let barrier = 1.25;
        let r_d = 0.05;
        let r_f = 0.03;
        let t = 0.5;
        let vol = 0.10;

        let quotes = VannaVolgaQuotes {
            vol_25d_put: vol,  // Same as ATM
            vol_atm: vol,      // ATM vol
            vol_25d_call: vol, // Same as ATM
            strike_25d_put: 1.05,
            strike_atm: 1.10,
            strike_25d_call: 1.15,
        };

        let barrier_type = AnalyticalBarrierType::UpOut;
        let params = BarrierParams::new(spot, strike, barrier, t, r_d, r_f, vol);
        let bs_price = barrier_bs_price(&params, barrier_type, true, None);

        let vv_price = vanna_volga_barrier_price(&params, &quotes, true, barrier_type, None);

        // With flat vol, VV price should equal BS price
        let diff = (vv_price - bs_price).abs();
        assert!(
            diff < 1e-10,
            "VV correction should be zero for flat vol, got diff = {diff}"
        );
    }

    /// VV barrier price should be between 0 and notional (per-unit: between 0 and spot).
    #[test]
    fn test_vv_price_within_bounds() {
        let spot = 1.10;
        let strike = 1.10;
        let barrier = 1.25;
        let r_d = 0.05;
        let r_f = 0.03;
        let t = 0.5;

        // Typical FX smile: puts have higher vol than calls
        let quotes = VannaVolgaQuotes {
            vol_25d_put: 0.12, // Risk reversal: higher vol for puts
            vol_atm: 0.10,
            vol_25d_call: 0.11,
            strike_25d_put: 1.02,
            strike_atm: 1.10,
            strike_25d_call: 1.18,
        };

        let barrier_type = AnalyticalBarrierType::UpOut;
        let params = BarrierParams::new(spot, strike, barrier, t, r_d, r_f, quotes.vol_atm);

        let vv_price = vanna_volga_barrier_price(&params, &quotes, true, barrier_type, None);

        assert!(
            vv_price >= -1e-10,
            "VV price should be non-negative, got {vv_price}"
        );
        // Upper bound: for a call, price < spot (per unit)
        assert!(
            vv_price < spot * 2.0,
            "VV price should be bounded, got {vv_price}"
        );
    }

    /// VV adjustment should produce a nonzero correction when there is a smile.
    #[test]
    fn test_vv_nonzero_correction_with_smile() {
        let spot = 1.10;
        let strike = 1.10;
        let barrier = 1.25;
        let r_d = 0.05;
        let r_f = 0.03;
        let t = 0.5;

        let quotes = VannaVolgaQuotes {
            vol_25d_put: 0.14, // Significant smile
            vol_atm: 0.10,
            vol_25d_call: 0.12,
            strike_25d_put: 1.02,
            strike_atm: 1.10,
            strike_25d_call: 1.18,
        };

        let barrier_type = AnalyticalBarrierType::UpOut;
        let params = BarrierParams::new(spot, strike, barrier, t, r_d, r_f, quotes.vol_atm);
        let bs_price = barrier_bs_price(&params, barrier_type, true, None);

        let vv_price = vanna_volga_barrier_price(&params, &quotes, true, barrier_type, None);

        let adjustment = (vv_price - bs_price).abs();
        assert!(
            adjustment > 1e-6,
            "VV adjustment should be nonzero with smile, got {adjustment}"
        );
    }

    /// Item 9 regression: the Vanna-Volga barrier correction must be weighted
    /// by the no-touch survival probability, so it vanishes as the barrier
    /// becomes certain to knock out. A knock-out call whose barrier sits just
    /// above spot with high vol and long maturity is almost surely knocked out;
    /// the smile correction must collapse toward zero even though the smile is
    /// strong.
    #[test]
    fn test_vv_correction_vanishes_as_knockout_becomes_certain() {
        let r_d = 0.05;
        let r_f = 0.03;
        let t = 3.0; // long maturity
        let vol = 0.40; // high vol => near-certain touch
        let spot = 1.10;

        let quotes = VannaVolgaQuotes {
            vol_25d_put: 0.50, // strong smile
            vol_atm: vol,
            vol_25d_call: 0.46,
            strike_25d_put: 1.02,
            strike_atm: 1.10,
            strike_25d_call: 1.18,
        };

        let barrier_type = AnalyticalBarrierType::UpOut;

        // Barrier essentially at spot: P(touch) ~ 1, survival ~ 0.
        let barrier_near = 1.1001;
        let params_near = BarrierParams::new(spot, 1.10, barrier_near, t, r_d, r_f, vol);
        let survival_near = vv_survival_weight(&params_near, barrier_type, vol);
        assert!(
            survival_near < 1e-3,
            "fixture must make knock-out near-certain (survival≈0), got {survival_near}"
        );
        let bs_near = barrier_bs_price(&params_near, barrier_type, true, None);
        let vv_near = vanna_volga_barrier_price(&params_near, &quotes, true, barrier_type, None);
        assert!(
            (vv_near - bs_near).abs() < 1e-3,
            "VV correction must vanish as knock-out becomes certain: \
             vv={vv_near} bs={bs_near}"
        );

        // A distant barrier survives with meaningful probability, so the same
        // smile produces a materially larger correction — confirming the
        // weighting is what suppresses the near-barrier case (not a degenerate
        // smile or zero greeks).
        let barrier_far = 1.60;
        let params_far = BarrierParams::new(spot, 1.10, barrier_far, t, r_d, r_f, vol);
        let bs_far = barrier_bs_price(&params_far, barrier_type, true, None);
        let vv_far = vanna_volga_barrier_price(&params_far, &quotes, true, barrier_type, None);
        assert!(
            (vv_far - bs_far).abs() > (vv_near - bs_near).abs() + 1e-6,
            "distant barrier (higher survival) must carry a larger VV correction: \
             near={} far={}",
            (vv_near - bs_near).abs(),
            (vv_far - bs_far).abs()
        );
    }

    /// Test that the VV method works for put options as well.
    #[test]
    fn test_vv_put_option() {
        let spot = 1.10;
        let strike = 1.10;
        let barrier = 0.95;
        let r_d = 0.05;
        let r_f = 0.03;
        let t = 0.5;

        let quotes = VannaVolgaQuotes {
            vol_25d_put: 0.13,
            vol_atm: 0.10,
            vol_25d_call: 0.11,
            strike_25d_put: 1.02,
            strike_atm: 1.10,
            strike_25d_call: 1.18,
        };

        let barrier_type = AnalyticalBarrierType::DownOut;
        let params = BarrierParams::new(spot, strike, barrier, t, r_d, r_f, quotes.vol_atm);

        let vv_price = vanna_volga_barrier_price(&params, &quotes, false, barrier_type, None);

        assert!(vv_price.is_finite(), "VV price should be finite for puts");
    }

    /// In–out parity on a smiled surface: `VV(KO) + VV(KI) = VV(vanilla)`
    /// must hold exactly for both up and down barriers (option legs only —
    /// no rebates), because the knock-in is priced via parity off the
    /// ATM-based vanilla and paired knock-out legs.
    #[test]
    fn test_vv_in_out_parity_on_smiled_surface() {
        let spot = 1.10;
        let strike = 1.10;
        let r_d = 0.05;
        let r_f = 0.03;
        let t = 0.5;

        let quotes = VannaVolgaQuotes {
            vol_25d_put: 0.14, // pronounced smile
            vol_atm: 0.10,
            vol_25d_call: 0.12,
            strike_25d_put: 1.02,
            strike_atm: 1.10,
            strike_25d_call: 1.18,
        };

        let cases = [
            (
                1.25,
                AnalyticalBarrierType::UpOut,
                AnalyticalBarrierType::UpIn,
                true,
            ),
            (
                1.25,
                AnalyticalBarrierType::UpOut,
                AnalyticalBarrierType::UpIn,
                false,
            ),
            (
                0.95,
                AnalyticalBarrierType::DownOut,
                AnalyticalBarrierType::DownIn,
                true,
            ),
            (
                0.95,
                AnalyticalBarrierType::DownOut,
                AnalyticalBarrierType::DownIn,
                false,
            ),
        ];

        for (barrier, ko, ki, is_call) in cases {
            let params = BarrierParams::new(spot, strike, barrier, t, r_d, r_f, quotes.vol_atm);
            let vv_ko = vanna_volga_barrier_price(&params, &quotes, is_call, ko, None);
            let vv_ki = vanna_volga_barrier_price(&params, &quotes, is_call, ki, None);
            let vv_vanilla = vanna_volga_vanilla_adjustment(&params, &quotes, is_call);

            let gap = (vv_ko + vv_ki - vv_vanilla).abs();
            assert!(
                gap < 1e-12,
                "in-out parity violated for {ko:?}/{ki:?} is_call={is_call}: \
                 KO={vv_ko} KI={vv_ki} vanilla={vv_vanilla} gap={gap}"
            );
        }
    }

    /// Flat smile: every VV leg collapses to its Black-Scholes counterpart,
    /// including the knock-in priced via parity (the BS barrier formulas
    /// satisfy in–out parity analytically).
    #[test]
    fn test_vv_knock_in_flat_smile_equals_bs() {
        let spot = 1.10;
        let strike = 1.10;
        let r_d = 0.05;
        let r_f = 0.03;
        let t = 0.5;
        let vol = 0.10;

        let quotes = VannaVolgaQuotes {
            vol_25d_put: vol,
            vol_atm: vol,
            vol_25d_call: vol,
            strike_25d_put: 1.05,
            strike_atm: 1.10,
            strike_25d_call: 1.15,
        };

        let cases = [
            (1.25, AnalyticalBarrierType::UpIn, true),
            (0.95, AnalyticalBarrierType::DownIn, false),
        ];

        for (barrier, barrier_type, is_call) in cases {
            let params = BarrierParams::new(spot, strike, barrier, t, r_d, r_f, vol);
            let bs = barrier_bs_price(&params, barrier_type, is_call, None);
            let vv = vanna_volga_barrier_price(&params, &quotes, is_call, barrier_type, None);
            let diff = (vv - bs).abs();
            assert!(
                diff < 1e-9,
                "flat-smile VV knock-in must equal BS for {barrier_type:?} \
                 is_call={is_call}: vv={vv} bs={bs} diff={diff}"
            );
        }
    }

    /// The vanilla VV leg with a flat smile equals the plain BS vanilla.
    #[test]
    fn test_vv_vanilla_flat_smile_equals_bs() {
        let spot = 1.10;
        let strike = 1.08;
        let r_d = 0.05;
        let r_f = 0.03;
        let t = 0.5;
        let vol = 0.10;

        let quotes = VannaVolgaQuotes {
            vol_25d_put: vol,
            vol_atm: vol,
            vol_25d_call: vol,
            strike_25d_put: 1.05,
            strike_atm: 1.10,
            strike_25d_call: 1.15,
        };

        // Barrier field is unused by the vanilla leg.
        let params = BarrierParams::new(spot, strike, 1.50, t, r_d, r_f, vol);
        for is_call in [true, false] {
            let vv = vanna_volga_vanilla_adjustment(&params, &quotes, is_call);
            let option_type = if is_call {
                OptionType::Call
            } else {
                OptionType::Put
            };
            let bs = bs_price(spot, strike, r_d, r_f, vol, t, option_type);
            assert!(
                (vv - bs).abs() < 1e-12,
                "flat-smile VV vanilla must equal BS: vv={vv} bs={bs}"
            );
        }
    }

    /// Verify that BS greeks (vanna, volga) are computed correctly for a vanilla option
    /// by checking against known relationships.
    #[test]
    fn test_vanilla_vanna_volga_consistency() {
        let spot = 1.10;
        let strike = 1.10;
        let r_d = 0.05;
        let r_f = 0.03;
        let vol = 0.10;
        let t = 1.0;

        let vega = bs_vega(spot, strike, r_d, r_f, vol, t);
        let vanna = bs_vanna(spot, strike, r_d, r_f, vol, t);
        let volga = bs_volga(spot, strike, r_d, r_f, vol, t);

        // Vega should be positive
        assert!(vega > 0.0, "Vega should be positive, got {vega}");

        // Vanna should be finite and bounded (not necessarily small for ATM
        // when r_d != r_f, since the forward moneyness shift affects d2)
        assert!(vanna.is_finite(), "Vanna should be finite, got {vanna}");

        // Volga at ATM should be finite
        // For exact ATM: d1*d2 ≈ (r-q+σ²/2)T / σ * ((r-q+σ²/2)T / σ - σ√T)
        assert!(volga.is_finite(), "Volga should be finite, got {volga}");

        // Cross-check: volga = vega * d1 * d2 / σ
        // Compute d1, d2 manually and verify
        let sqrt_t = t.sqrt();
        let d1 = ((spot / strike).ln() + (r_d - r_f + 0.5 * vol * vol) * t) / (vol * sqrt_t);
        let d2 = d1 - vol * sqrt_t;
        let expected_volga = vega * d1 * d2 / vol;
        let volga_err = (volga - expected_volga).abs();
        assert!(
            volga_err < 1e-10,
            "Volga should match analytical formula. Got {volga}, expected {expected_volga}, diff {volga_err}"
        );
    }
}
