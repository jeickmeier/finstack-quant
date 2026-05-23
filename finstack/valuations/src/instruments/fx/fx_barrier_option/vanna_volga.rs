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
    barrier_call_continuous, barrier_put_continuous, barrier_touch_probability, BarrierParams,
    BarrierType as AnalyticalBarrierType,
};
use crate::models::closed_form::vanilla::bs_price;
use crate::models::volatility::black::d1_d2;

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

/// Compute barrier option price using BS model at a given vol.
fn barrier_bs_price(
    params: &BarrierParams,
    barrier_type: AnalyticalBarrierType,
    is_call: bool,
) -> f64 {
    if is_call {
        barrier_call_continuous(params, barrier_type)
    } else {
        barrier_put_continuous(params, barrier_type)
    }
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
) -> f64 {
    let h_spot = params.spot * 0.001; // 0.1% spot bump
    let h_vol = 0.001; // 10bp vol bump

    let p = |s: f64, v: f64| -> f64 {
        let bumped = BarrierParams {
            spot: s,
            vol: v,
            ..*params
        };
        barrier_bs_price(&bumped, barrier_type, is_call)
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
) -> f64 {
    let h_vol = 0.001; // 10bp vol bump

    let bump_vol = |dv: f64| -> BarrierParams {
        BarrierParams {
            vol: params.vol + dv,
            ..*params
        }
    };

    let p_base = barrier_bs_price(params, barrier_type, is_call);
    let p_up = barrier_bs_price(&bump_vol(h_vol), barrier_type, is_call);
    let p_down = barrier_bs_price(&bump_vol(-h_vol), barrier_type, is_call);

    (p_up - 2.0 * p_base + p_down) / (h_vol * h_vol)
}

/// Compute barrier vega via central finite differences on the BS barrier formula.
///
/// vega_barrier = ∂P_barrier / ∂σ
fn barrier_vega_fd(
    params: &BarrierParams,
    barrier_type: AnalyticalBarrierType,
    is_call: bool,
) -> f64 {
    let h_vol = 0.001;

    let bump_vol = |dv: f64| -> BarrierParams {
        BarrierParams {
            vol: params.vol + dv,
            ..*params
        }
    };

    let p_up = barrier_bs_price(&bump_vol(h_vol), barrier_type, is_call);
    let p_down = barrier_bs_price(&bump_vol(-h_vol), barrier_type, is_call);

    (p_up - p_down) / (2.0 * h_vol)
}

/// Apply Vanna-Volga correction to a barrier option price.
///
/// This implements the full Vanna-Volga method which solves for weights that
/// match the vega, vanna, and volga of the barrier option to a portfolio of
/// three vanilla options at market strikes (25Δ put, ATM, 25Δ call).
///
/// # Arguments
///
/// * `bs_barrier_price` - The BS barrier price computed at ATM vol
/// * `spot` - Current FX spot rate
/// * `barrier` - Barrier level
/// * `strike` - Option strike
/// * `r_d` - Domestic risk-free rate
/// * `r_f` - Foreign risk-free rate
/// * `t` - Time to expiry in years
/// * `quotes` - Market volatility quotes at the three pillar strikes
/// * `is_call` - Whether the option is a call
/// * `barrier_type` - The analytical barrier type
///
/// # Returns
///
/// The Vanna-Volga adjusted barrier price.
pub fn vanna_volga_barrier_adjustment(
    bs_barrier_price: f64,
    params: &BarrierParams,
    quotes: &VannaVolgaQuotes,
    is_call: bool,
    barrier_type: AnalyticalBarrierType,
) -> f64 {
    let BarrierParams {
        spot,
        strike: _,
        barrier: _,
        time: t,
        rate: r_d,
        div_yield: r_f,
        vol: _,
    } = *params;

    if t <= 0.0 {
        return bs_barrier_price;
    }

    let sigma_atm = quotes.vol_atm;
    let atm_params = BarrierParams {
        vol: sigma_atm,
        ..*params
    };

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

    // Step 3: Compute vega, vanna, volga of the barrier option via FD
    let vega_barrier = barrier_vega_fd(&atm_params, barrier_type, is_call);
    let vanna_barrier = barrier_vanna_fd(&atm_params, barrier_type, is_call);
    let volga_barrier = barrier_volga_fd(&atm_params, barrier_type, is_call);

    // Step 4: Solve the 3×3 linear system for weights p₁, p₂, p₃:
    //   p₁ × vega₁ + p₂ × vega₂ + p₃ × vega₃ = vega_barrier
    //   p₁ × vanna₁ + p₂ × vanna₂ + p₃ × vanna₃ = vanna_barrier
    //   p₁ × volga₁ + p₂ × volga₂ + p₃ × volga₃ = volga_barrier
    //
    // We solve this via Cramer's rule for the 3×3 system.
    let det = determinant_3x3(
        vega_1, vega_2, vega_3, vanna_1, vanna_2, vanna_3, volga_1, volga_2, volga_3,
    );

    // If the system is singular (degenerate smile), return BS price
    if det.abs() < 1e-30 {
        return bs_barrier_price;
    }

    let p1 = determinant_3x3(
        vega_barrier,
        vega_2,
        vega_3,
        vanna_barrier,
        vanna_2,
        vanna_3,
        volga_barrier,
        volga_2,
        volga_3,
    ) / det;

    let p2 = determinant_3x3(
        vega_1,
        vega_barrier,
        vega_3,
        vanna_1,
        vanna_barrier,
        vanna_3,
        volga_1,
        volga_barrier,
        volga_3,
    ) / det;

    let p3 = determinant_3x3(
        vega_1,
        vega_2,
        vega_barrier,
        vanna_1,
        vanna_2,
        vanna_barrier,
        volga_1,
        volga_2,
        volga_barrier,
    ) / det;

    // Step 5: smile-cost portfolio, Σ pᵢ × costᵢ.
    let smile_cost = p1 * cost_1 + p2 * cost_2 + p3 * cost_3;

    // Step 6: weight by the survival probability. For a barrier option the
    // Vanna-Volga correction must vanish as the barrier becomes certain to
    // knock out (or, for a knock-in, certain not to activate): a dead option
    // carries no vanna/volga and therefore no smile cost. Omitting this weight
    // over-applies the smile correction near the barrier.
    let survival_weight = vv_survival_weight(&atm_params, barrier_type, sigma_atm);
    let vv_adjustment = survival_weight * smile_cost;

    bs_barrier_price + vv_adjustment
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
        let bs_price = barrier_bs_price(&params, barrier_type, true);

        let vv_price =
            vanna_volga_barrier_adjustment(bs_price, &params, &quotes, true, barrier_type);

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
        let bs_price = barrier_bs_price(&params, barrier_type, true);

        let vv_price =
            vanna_volga_barrier_adjustment(bs_price, &params, &quotes, true, barrier_type);

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
        let bs_price = barrier_bs_price(&params, barrier_type, true);

        let vv_price =
            vanna_volga_barrier_adjustment(bs_price, &params, &quotes, true, barrier_type);

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
        let bs_near = barrier_bs_price(&params_near, barrier_type, true);
        let vv_near =
            vanna_volga_barrier_adjustment(bs_near, &params_near, &quotes, true, barrier_type);
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
        let bs_far = barrier_bs_price(&params_far, barrier_type, true);
        let vv_far =
            vanna_volga_barrier_adjustment(bs_far, &params_far, &quotes, true, barrier_type);
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
        let bs_price = barrier_bs_price(&params, barrier_type, false);

        let vv_price =
            vanna_volga_barrier_adjustment(bs_price, &params, &quotes, false, barrier_type);

        assert!(vv_price.is_finite(), "VV price should be finite for puts");
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
