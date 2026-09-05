//! FX option pricer implementation using the Garman-Kohlhagen model.

use crate::instruments::common_impl::parameters::OptionType;
use crate::instruments::fx::fx_option::FxOption;
use crate::instruments::fx::shared::{
    collect_fx_option_inputs as collect_shared_fx_option_inputs,
    collect_fx_option_inputs_no_vol as collect_shared_fx_option_inputs_no_vol,
    FxOptionInputRequest, FxSpotSource,
};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_models::closed_form::vanilla::{bs_greeks_unchecked, bs_price_unchecked};

const STRIKE_ZERO_TOL: f64 = 1e-12;
const THETA_DAYS_PER_YEAR: f64 = 365.0;

pub(crate) fn compute_pv(inst: &FxOption, curves: &MarketContext, as_of: Date) -> Result<Money> {
    inst.validate()?;
    if crate::instruments::fx::shared::event_has_occurred(inst.expiry, as_of) {
        return Ok(Money::from((0_i64, inst.quote_currency)));
    }
    let (spot, r_d, r_f, sigma, t) = collect_inputs(inst, curves, as_of)?;
    if spot <= 0.0 || inst.strike < 0.0 || inst.notional.amount() <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
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
        return Money::new(intrinsic * inst.notional.amount(), inst.quote_currency);
    }

    if !inst.strike.is_finite() {
        return Err(finstack_quant_core::Error::Validation(
            "FX option strike must be finite".to_string(),
        ));
    }

    if inst.strike.abs() < STRIKE_ZERO_TOL {
        let unit_price = match inst.option_type {
            OptionType::Call => spot * (-r_f * t).exp(),
            OptionType::Put => 0.0,
        };
        return Money::new(unit_price * inst.notional.amount(), inst.quote_currency);
    }

    let price = bs_price_unchecked(spot, inst.strike, r_d, r_f, sigma, t, inst.option_type);
    Money::new(price * inst.notional.amount(), inst.quote_currency)
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
        instrument_pricing_overrides: &inst.instrument_pricing_overrides,
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

pub(crate) fn implied_vol(
    inst: &FxOption,
    curves: &MarketContext,
    as_of: Date,
    target_price: f64,
) -> Result<f64> {
    inst.validate()?;
    let (spot, r_d, r_f, t) = collect_inputs_no_vol(inst, curves, as_of)?;
    if t <= 0.0 {
        return Ok(0.0);
    }
    if spot <= 0.0 || inst.strike <= 0.0 || inst.notional.amount() <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
                "Implied vol requires spot > 0, strike > 0, and notional > 0; got spot={spot}, strike={}, notional={}",
                inst.strike,
                inst.notional.amount()
            )));
    }

    let target_unit = target_price / inst.notional.amount();

    finstack_quant_models::bs_implied_vol(
        spot,
        inst.strike,
        r_d,
        r_f,
        t,
        inst.option_type,
        target_unit,
    )
}

pub(crate) fn compute_greeks(
    inst: &FxOption,
    curves: &MarketContext,
    as_of: Date,
) -> Result<FxOptionGreeks> {
    if crate::instruments::fx::shared::event_has_occurred(inst.expiry, as_of) {
        return Ok(FxOptionGreeks::default());
    }
    inst.validate()?;
    let (spot, r_d, r_f, sigma, t) = collect_inputs(inst, curves, as_of)?;
    if spot <= 0.0 || inst.strike < 0.0 || inst.notional.amount() <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
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
        return Err(finstack_quant_core::Error::Validation(
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

    let greeks_unit = bs_greeks_unchecked(
        spot,
        inst.strike,
        r_d,
        r_f,
        sigma,
        t,
        inst.option_type,
        THETA_DAYS_PER_YEAR,
    );
    let d1 = finstack_quant_models::d1(spot, inst.strike, r_d, sigma, t, r_f);
    let d2 = d1 - sigma * t.sqrt();
    let cdf_d1 = finstack_quant_core::math::norm_cdf(d1);
    let cdf_d2 = finstack_quant_core::math::norm_cdf(d2);
    let exp_rd_t = (-r_d * t).exp();
    let delta_forward_unit = match inst.option_type {
        OptionType::Call => cdf_d1,
        OptionType::Put => cdf_d1 - 1.0,
    };
    let signed_cdf_d2 = match inst.option_type {
        OptionType::Call => cdf_d2,
        OptionType::Put => cdf_d2 - 1.0,
    };
    let premium_in_base = inst.delta_convention.premium_currency == inst.base_currency;
    let delta_premium_adjusted_spot_unit = if premium_in_base {
        (inst.strike / spot) * exp_rd_t * signed_cdf_d2
    } else {
        greeks_unit.delta
    };
    let forward = spot * ((r_d - r_f) * t).exp();
    let delta_premium_adjusted_forward_unit = if premium_in_base {
        (inst.strike / forward) * signed_cdf_d2
    } else {
        delta_forward_unit
    };

    let scale = inst.notional.amount();
    Ok(FxOptionGreeks {
        delta: greeks_unit.delta * scale,
        delta_forward: delta_forward_unit * scale,
        delta_premium_adjusted_spot: delta_premium_adjusted_spot_unit * scale,
        delta_premium_adjusted_forward: delta_premium_adjusted_forward_unit * scale,
        gamma: greeks_unit.gamma * scale,
        vega: greeks_unit.vega * scale,
        theta: greeks_unit.theta * scale,
        rho_domestic: greeks_unit.rho_r * scale,
        rho_foreign: greeks_unit.rho_q * scale,
    })
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct FxOptionGreeks {
    /// Garman–Kohlhagen **spot delta** `e^{-r_f·T}·N(d1)` (premium-unadjusted).
    ///
    /// This is one of four explicit FX delta metrics. Consumers should select
    /// the metric named by `FxOption::delta_convention`.
    pub(crate) delta: f64,
    /// Forward delta `N(d1)` — the interbank G10 quoting convention.
    pub(crate) delta_forward: f64,
    /// Premium-adjusted spot delta under the instrument's premium currency.
    pub(crate) delta_premium_adjusted_spot: f64,
    /// Premium-adjusted forward delta under the instrument's premium currency.
    pub(crate) delta_premium_adjusted_forward: f64,
    pub(crate) gamma: f64,
    pub(crate) vega: f64,
    pub(crate) theta: f64,
    pub(crate) rho_domestic: f64,
    pub(crate) rho_foreign: f64,
}

#[cfg(test)]
mod delegation_tests {
    use super::*;
    use crate::instruments::common_impl::traits::{Attributes, Instrument};
    use crate::instruments::fx::fx_option::{FxDeltaConvention, FxDeltaConventionKind};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
    use finstack_quant_core::types::{CurveId, InstrumentId};
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
            .delta_convention(
                FxDeltaConvention::new(FxDeltaConventionKind::Forward, Currency::USD, "test")
                    .expect("valid delta convention"),
            )
            .expiry(expiry)
            .day_count(DayCount::Act365F)
            .notional(Money::from((1_000_000_i64, Currency::EUR)))
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .vol_surface_id(CurveId::new("EURUSD-VOL"))
            .attributes(Attributes::new())
            .build()
            .expect("fx option")
    }

    #[test]
    fn implied_vol_self_seeding_solver_recovers_market_vol() {
        let as_of = date!(2024 - 01 - 01);
        let expiry = date!(2025 - 01 - 01);
        let option = build_option(expiry);
        let market = build_market(as_of);

        // Price the option at sigma=0.15 (the vol surface value), then recover
        // implied vol with the solver's canonical self-seeding path.
        let pv = compute_pv(&option, &market, as_of).expect("pv");
        let target_price = pv.amount();

        let implied = implied_vol(&option, &market, as_of, target_price).expect("implied vol");
        assert!(
            (implied - 0.15).abs() < 1e-6,
            "implied={implied} expected ~0.15"
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

    #[test]
    fn premium_currency_selects_explicit_spot_and_forward_delta_adjustments() {
        let as_of = date!(2024 - 01 - 01);
        let expiry = date!(2025 - 01 - 01);
        let mut base_premium = build_option(expiry);
        base_premium.delta_convention = FxDeltaConvention::new(
            FxDeltaConventionKind::PremiumAdjustedForward,
            Currency::EUR,
            "test",
        )
        .expect("base premium convention");
        let market = build_market(as_of);
        let adjusted = compute_greeks(&base_premium, &market, as_of).expect("adjusted greeks");

        let mut quote_premium = base_premium;
        quote_premium.delta_convention = FxDeltaConvention::new(
            FxDeltaConventionKind::PremiumAdjustedForward,
            Currency::USD,
            "test",
        )
        .expect("quote premium convention");
        let unadjusted = compute_greeks(&quote_premium, &market, as_of).expect("quote greeks");

        assert_eq!(unadjusted.delta_premium_adjusted_spot, unadjusted.delta);
        assert_eq!(
            unadjusted.delta_premium_adjusted_forward,
            unadjusted.delta_forward
        );
        assert_ne!(
            adjusted.delta_premium_adjusted_spot,
            unadjusted.delta_premium_adjusted_spot
        );
        assert_ne!(
            adjusted.delta_premium_adjusted_forward,
            unadjusted.delta_premium_adjusted_forward
        );
    }

    #[test]
    fn post_expiry_value_is_zero_without_live_spot_or_curves() {
        let expiry = date!(2025 - 01 - 01);
        let option = build_option(expiry);
        let empty = MarketContext::new();
        let pv = compute_pv(&option, &empty, date!(2025 - 01 - 02))
            .expect("post-expiry option must be extinguished");
        assert_eq!(pv.amount(), 0.0);
    }
}
