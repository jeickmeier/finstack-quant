//! FX option pricer implementation using the Garman-Kohlhagen model.

use crate::instruments::common_impl::parameters::OptionType;
use crate::instruments::fx::fx_option::FxOption;
use crate::instruments::fx::shared::{
    collect_fx_option_inputs as collect_shared_fx_option_inputs,
    collect_fx_option_inputs_no_vol as collect_shared_fx_option_inputs_no_vol,
    FxOptionInputRequest, FxSpotSource,
};
use crate::models::{bs_greeks, bs_price};
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::Result;

const STRIKE_ZERO_TOL: f64 = 1e-12;
const THETA_DAYS_PER_YEAR: f64 = 365.0;

pub(crate) fn compute_pv(inst: &FxOption, curves: &MarketContext, as_of: Date) -> Result<Money> {
    npv(inst, curves, as_of)
}

pub(crate) fn compute_greeks(
    inst: &FxOption,
    curves: &MarketContext,
    as_of: Date,
) -> Result<FxOptionGreeks> {
    compute_greeks_impl(inst, curves, as_of)
}

pub(crate) fn implied_vol(
    inst: &FxOption,
    curves: &MarketContext,
    as_of: Date,
    target_price: f64,
    initial_guess: Option<f64>,
) -> Result<f64> {
    implied_vol_impl(inst, curves, as_of, target_price, initial_guess)
}

fn npv(inst: &FxOption, curves: &MarketContext, as_of: Date) -> Result<Money> {
    validate_exercise_style(inst)?;
    validate_currency(inst)?;
    let (spot, r_d, r_f, sigma, t) = collect_inputs(inst, curves, as_of)?;
    if spot <= 0.0 || inst.strike < 0.0 || inst.notional.amount() <= 0.0 {
        return Err(finstack_core::Error::Validation(format!(
                "FxOption requires spot > 0, strike >= 0, and notional > 0; got spot={spot}, strike={}, notional={}",
                inst.strike,
                inst.notional.amount()
            )));
    }

    if t <= 0.0 {
        let intrinsic = match inst.option_type {
            OptionType::Call => (spot - inst.strike).max(0.0),
            OptionType::Put => (inst.strike - spot).max(0.0),
        };
        return Ok(Money::new(
            intrinsic * inst.notional.amount(),
            inst.quote_currency,
        ));
    }

    if !inst.strike.is_finite() {
        return Err(finstack_core::Error::Validation(
            "FX option strike must be finite".to_string(),
        ));
    }

    if inst.strike.abs() < STRIKE_ZERO_TOL {
        let unit_price = match inst.option_type {
            OptionType::Call => spot * (-r_f * t).exp(),
            OptionType::Put => 0.0,
        };
        return Ok(Money::new(
            unit_price * inst.notional.amount(),
            inst.quote_currency,
        ));
    }

    let price = price_gk_core(spot, inst.strike, r_d, r_f, sigma, t, inst.option_type);
    Ok(Money::new(
        price * inst.notional.amount(),
        inst.quote_currency,
    ))
}

fn input_request<'a>(
    inst: &'a FxOption,
    curves: &'a MarketContext,
    as_of: Date,
) -> FxOptionInputRequest<'a> {
    FxOptionInputRequest {
        market: curves,
        as_of,
        base_currency: inst.base_currency,
        quote_currency: inst.quote_currency,
        expiry: inst.expiry,
        day_count: inst.day_count,
        domestic_discount_curve_id: &inst.domestic_discount_curve_id,
        foreign_discount_curve_id: &inst.foreign_discount_curve_id,
        vol_surface_id: inst.vol_surface_id.as_str(),
        strike: inst.strike,
        pricing_overrides: &inst.pricing_overrides,
        spot_source: FxSpotSource::Matrix,
        rate_context: "FxOption",
    }
}

/// Pricing inputs without volatility lookup. Used by IV solver and as a base
/// for the full input collection. Returns `(spot, r_d, r_f, t_vol)` and
/// short-circuits to `(spot, 0, 0, 0)` when `as_of >= expiry`.
fn collect_inputs_no_vol(
    inst: &FxOption,
    curves: &MarketContext,
    as_of: Date,
) -> Result<(f64, f64, f64, f64)> {
    let inputs = collect_shared_fx_option_inputs_no_vol(input_request(inst, curves, as_of))?;
    Ok((inputs.spot, inputs.r_domestic, inputs.r_foreign, inputs.t))
}

/// Full pricing inputs including volatility. Returns `(spot, r_d, r_f, sigma,
/// t_vol)` and short-circuits to `(spot, 0, 0, 0, 0)` when expired.
fn collect_inputs(
    inst: &FxOption,
    curves: &MarketContext,
    as_of: Date,
) -> Result<(f64, f64, f64, f64, f64)> {
    let inputs = collect_shared_fx_option_inputs(input_request(inst, curves, as_of))?;
    Ok((
        inputs.spot,
        inputs.r_domestic,
        inputs.r_foreign,
        inputs.sigma,
        inputs.t,
    ))
}

fn implied_vol_impl(
    inst: &FxOption,
    curves: &MarketContext,
    as_of: Date,
    target_price: f64,
    initial_guess: Option<f64>,
) -> Result<f64> {
    validate_currency(inst)?;
    let (spot, r_d, r_f, t) = collect_inputs_no_vol(inst, curves, as_of)?;
    if t <= 0.0 {
        return Ok(0.0);
    }
    if spot <= 0.0 || inst.strike <= 0.0 || inst.notional.amount() <= 0.0 {
        return Err(finstack_core::Error::Validation(format!(
                "Implied vol requires spot > 0, strike > 0, and notional > 0; got spot={spot}, strike={}, notional={}",
                inst.strike,
                inst.notional.amount()
            )));
    }

    // `initial_guess` is accepted in the public API for forward-compatibility
    // but the underlying Newton-Raphson/bisection solver (`bs_implied_vol`)
    // self-seeds its bracket from [MIN_VOL, 0.3] and does not consume an
    // external starting point. The parameter is intentionally ignored here;
    // callers should not rely on it affecting convergence.
    let _ = initial_guess;
    let target_unit = target_price / inst.notional.amount();

    crate::models::bs_implied_vol(
        spot,
        inst.strike,
        r_d,
        r_f,
        t,
        inst.option_type,
        target_unit,
    )
}

fn compute_greeks_impl(
    inst: &FxOption,
    curves: &MarketContext,
    as_of: Date,
) -> Result<FxOptionGreeks> {
    validate_currency(inst)?;
    let (spot, r_d, r_f, sigma, t) = collect_inputs(inst, curves, as_of)?;
    if spot <= 0.0 || inst.strike < 0.0 || inst.notional.amount() <= 0.0 {
        return Err(finstack_core::Error::Validation(format!(
                "FxOption greeks require spot > 0, strike >= 0, and notional > 0; got spot={spot}, strike={}, notional={}",
                inst.strike,
                inst.notional.amount()
            )));
    }

    if t <= 0.0 {
        let spot_gt_strike = spot > inst.strike;
        let delta_unit = match inst.option_type {
            OptionType::Call => {
                if spot_gt_strike {
                    1.0
                } else {
                    0.0
                }
            }
            OptionType::Put => {
                if !spot_gt_strike {
                    -1.0
                } else {
                    0.0
                }
            }
        };
        let scale = inst.notional.amount();
        return Ok(FxOptionGreeks {
            delta: delta_unit * scale,
            ..Default::default()
        });
    }

    if !inst.strike.is_finite() {
        return Err(finstack_core::Error::Validation(
            "FX option strike must be finite".to_string(),
        ));
    }

    if inst.strike.abs() < STRIKE_ZERO_TOL {
        let scale = inst.notional.amount();
        let exp_rf_t = (-r_f * t).exp();
        let delta_unit = match inst.option_type {
            OptionType::Call => exp_rf_t,
            OptionType::Put => 0.0,
        };
        return Ok(FxOptionGreeks {
            delta: delta_unit * scale,
            ..Default::default()
        });
    }

    let greeks_unit = bs_greeks(
        spot,
        inst.strike,
        r_d,
        r_f,
        sigma,
        t,
        inst.option_type,
        THETA_DAYS_PER_YEAR,
    );
    let d1 = crate::models::d1(spot, inst.strike, r_d, sigma, t, r_f);
    let d2 = d1 - sigma * t.sqrt();
    let cdf_d1 = finstack_core::math::norm_cdf(d1);
    let cdf_d2 = finstack_core::math::norm_cdf(d2);
    let exp_rd_t = (-r_d * t).exp();
    let delta_forward_unit = match inst.option_type {
        OptionType::Call => cdf_d1,
        OptionType::Put => cdf_d1 - 1.0,
    };
    let delta_premium_adjusted_unit = match inst.option_type {
        OptionType::Call => (inst.strike / spot) * exp_rd_t * cdf_d2,
        OptionType::Put => (inst.strike / spot) * exp_rd_t * (cdf_d2 - 1.0),
    };

    let scale = inst.notional.amount();
    Ok(FxOptionGreeks {
        delta: greeks_unit.delta * scale,
        delta_forward: delta_forward_unit * scale,
        delta_premium_adjusted: delta_premium_adjusted_unit * scale,
        gamma: greeks_unit.gamma * scale,
        vega: greeks_unit.vega * scale,
        theta: greeks_unit.theta * scale,
        rho_domestic: greeks_unit.rho_r * scale,
        rho_foreign: greeks_unit.rho_q * scale,
    })
}

#[inline]
fn validate_exercise_style(inst: &FxOption) -> Result<()> {
    use crate::instruments::ExerciseStyle;
    if inst.exercise_style != ExerciseStyle::European {
        return Err(finstack_core::Error::Validation(format!(
            "FxOption only supports European exercise style. \
                 Got {:?}. American and Bermudan options require \
                 specialized pricers not yet implemented.",
            inst.exercise_style
        )));
    }
    Ok(())
}

#[inline]
fn validate_currency(inst: &FxOption) -> Result<()> {
    inst.validate()
}

#[derive(Debug, Clone, Copy, Default)]
#[allow(dead_code)]
pub(crate) struct FxOptionGreeks {
    pub(crate) delta: f64,
    pub(crate) delta_forward: f64,
    pub(crate) delta_premium_adjusted: f64,
    pub(crate) gamma: f64,
    pub(crate) vega: f64,
    pub(crate) theta: f64,
    pub(crate) rho_domestic: f64,
    pub(crate) rho_foreign: f64,
}

#[inline]
fn price_gk_core(
    spot: f64,
    strike: f64,
    r_d: f64,
    r_f: f64,
    sigma: f64,
    t: f64,
    option_type: OptionType,
) -> f64 {
    bs_price(spot, strike, r_d, r_f, sigma, t, option_type)
}

#[cfg(test)]
mod delegation_tests {
    use super::*;
    use crate::instruments::common_impl::traits::{Attributes, Instrument};
    use crate::instruments::{ExerciseStyle, PricingOverrides, SettlementType};
    use finstack_core::currency::Currency;
    use finstack_core::dates::{Date, DayCount};
    use finstack_core::market_data::surfaces::VolSurface;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::money::fx::{FxMatrix, SimpleFxProvider};
    use finstack_core::types::{CurveId, InstrumentId};
    use std::sync::Arc;
    use time::macros::date;

    fn build_market(as_of: Date) -> MarketContext {
        let usd_curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, (-0.03_f64).exp())])
            .build()
            .expect("usd curve");
        let eur_curve = DiscountCurve::builder("EUR-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, (-0.01_f64).exp())])
            .build()
            .expect("eur curve");
        let vol_surface = VolSurface::builder("EURUSD-VOL")
            .expiries(&[0.25, 0.5, 1.0, 2.0])
            .strikes(&[0.9, 1.0, 1.1, 1.2, 1.3])
            .row(&[0.15; 5])
            .row(&[0.15; 5])
            .row(&[0.15; 5])
            .row(&[0.15; 5])
            .build()
            .expect("vol surface");
        let provider = SimpleFxProvider::new();
        provider
            .set_quote(Currency::EUR, Currency::USD, 1.20)
            .expect("valid rate");
        let fx_matrix = FxMatrix::new(Arc::new(provider));

        MarketContext::new()
            .insert(usd_curve)
            .insert(eur_curve)
            .insert_surface(vol_surface)
            .insert_fx(fx_matrix)
    }

    fn build_option(expiry: Date) -> FxOption {
        FxOption::builder()
            .id(InstrumentId::new("FX-OPTION-TEST"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .strike(1.20)
            .option_type(OptionType::Call)
            .exercise_style(ExerciseStyle::European)
            .expiry(expiry)
            .day_count(DayCount::Act365F)
            .notional(Money::new(1_000_000.0, Currency::EUR))
            .settlement(SettlementType::Cash)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .vol_surface_id(CurveId::new("EURUSD-VOL"))
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("fx option")
    }

    /// W-45: implied_vol initial_guess parameter was silently discarded via
    /// `let _ = initial_guess.unwrap_or(IV_INITIAL_GUESS)`.
    ///
    /// The underlying `bs_implied_vol` solver uses Newton-Raphson + bisection
    /// and self-seeds its bracket — it does not consume an external initial
    /// guess. This test verifies:
    /// 1. Convergence is unconditional regardless of the supplied `initial_guess`.
    /// 2. Wildly-wrong guesses (e.g. 10.0 = 1000% vol) do not cause divergence.
    /// 3. Both `None` and `Some(x)` return the same result (parameter is inert).
    #[test]
    fn w45_implied_vol_initial_guess_is_inert_solver_always_converges() {
        let as_of = date!(2024 - 01 - 01);
        let expiry = date!(2025 - 01 - 01);
        let option = build_option(expiry);
        let market = build_market(as_of);

        // Price the option at sigma=0.15 (the vol surface value), then recover
        // implied vol. Use a deliberately-bad initial guess (1000% vol) to
        // confirm the solver does not rely on it.
        let pv = compute_pv(&option, &market, as_of).expect("pv");
        let target_price = pv.amount();

        let iv_no_guess =
            implied_vol(&option, &market, as_of, target_price, None).expect("iv with None guess");
        let iv_bad_guess = implied_vol(&option, &market, as_of, target_price, Some(10.0))
            .expect("iv with bad guess (10.0 = 1000% vol)");
        let iv_zero_guess = implied_vol(&option, &market, as_of, target_price, Some(0.0001))
            .expect("iv with near-zero guess");

        // All three must recover ~0.15 (the market vol).
        assert!(
            (iv_no_guess - 0.15).abs() < 1e-6,
            "iv_no_guess={iv_no_guess} expected ~0.15"
        );
        assert!(
            (iv_bad_guess - 0.15).abs() < 1e-6,
            "iv_bad_guess={iv_bad_guess} expected ~0.15 — solver must be unconditional"
        );
        assert!(
            (iv_zero_guess - 0.15).abs() < 1e-6,
            "iv_zero_guess={iv_zero_guess} expected ~0.15"
        );

        // All three must agree with each other (initial_guess is inert).
        assert!(
            (iv_no_guess - iv_bad_guess).abs() < 1e-10,
            "initial_guess must not affect the result: no_guess={iv_no_guess} bad_guess={iv_bad_guess}"
        );
    }

    #[test]
    fn fx_option_pricer_compute_pv_matches_instrument_value() {
        let as_of = date!(2024 - 01 - 01);
        let expiry = date!(2025 - 01 - 01);
        let option = build_option(expiry);
        let market = build_market(as_of);

        let via_pricer = compute_pv(&option, &market, as_of).expect("pricer pv");
        let via_instrument = option.value(&market, as_of).expect("instrument pv");

        assert!((via_pricer.amount() - via_instrument.amount()).abs() < 1e-10);
        assert_eq!(via_pricer.currency(), via_instrument.currency());
    }
}
