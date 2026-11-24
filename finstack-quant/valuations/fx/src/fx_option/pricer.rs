//! FX option pricer implementation using the Garman-Kohlhagen model.

use crate::fx_option::FxOption;
use crate::market_inputs::{
    FxOptionInputRequest, FxOptionInputs, FxOptionInputsNoVol, collect_fx_option_inputs,
    collect_fx_option_inputs_no_vol,
};
use finstack_quant_core::Result;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_models::closed_form::{bs_greeks_unchecked, bs_price_unchecked};
use finstack_quant_models::types::{OptionMarketParams, OptionType};

const STRIKE_ZERO_TOL: f64 = 1e-12;

#[cfg(test)]
thread_local! {
    pub(crate) static COMPUTE_GREEKS_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(crate) fn compute_pv(inst: &FxOption, curves: &MarketContext, as_of: Date) -> Result<Money> {
    inst.validate()?;
    let signed_notional = inst.notional.amount() * inst.delivery.position_sign();
    if let crate::fx_option::FxOptionDelivery::Physical {
        date, exercised, ..
    } = inst.delivery
        && as_of >= inst.expiry
    {
        if as_of > date || exercised == Some(false) {
            return Ok(Money::from((0_i64, inst.quote_currency)));
        }
        if exercised != Some(true) {
            return Err(finstack_quant_core::Error::Validation(
                "deliverable FX option requires a confirmed exercise outcome at expiry".into(),
            ));
        }
        let settlement = crate::market_inputs::collect_fx_forward_inputs(
            crate::market_inputs::FxForwardRateRequest {
                market: curves,
                as_of,
                maturity: date,
                base_currency: inst.base_currency,
                quote_currency: inst.quote_currency,
                domestic_discount_curve_id: &inst.domestic_discount_curve_id,
                foreign_discount_curve_id: &inst.foreign_discount_curve_id,
                quoted_spot: None,
                spot_basis: inst.spot_basis,
                context: "FxOption physical exercise",
            },
        )?;
        let sign = if inst.option_type == OptionType::Call {
            1.0
        } else {
            -1.0
        };
        let amount = sign
            * signed_notional
            * (settlement.spot * settlement.df_foreign - inst.strike * settlement.df_domestic);
        return Money::from_model_value(amount, inst.quote_currency);
    }
    if crate::market_inputs::event_has_occurred(inst.expiry, as_of) {
        return Ok(Money::from((0_i64, inst.quote_currency)));
    }
    let FxOptionInputs {
        spot,
        r_domestic: r_d,
        r_foreign: r_f,
        sigma,
        t,
        df_domestic,
        df_foreign,
        ..
    } = collect_fx_option_inputs(input_request(inst, curves, as_of))?;

    if t <= 0.0 {
        let intrinsic = inst
            .option_type
            .intrinsic(spot * df_foreign / df_domestic, inst.strike);
        return Money::from_model_value(
            intrinsic * df_domestic * signed_notional,
            inst.quote_currency,
        );
    }

    if inst.strike.abs() < STRIKE_ZERO_TOL {
        let unit_price = match inst.option_type {
            OptionType::Call => spot * (-r_f * t).exp(),
            OptionType::Put => 0.0,
        };
        return Money::from_model_value(unit_price * signed_notional, inst.quote_currency);
    }

    let price = bs_price_unchecked(spot, inst.strike, r_d, r_f, sigma, t, inst.option_type);
    Money::from_model_value(price * signed_notional, inst.quote_currency)
}

pub(crate) fn input_request<'a>(
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
        delivery: inst.delivery.value_date(inst.expiry),
        spot_basis: inst.spot_basis,
        day_count: inst.day_count,
        domestic_discount_curve_id: &inst.domestic_discount_curve_id,
        foreign_discount_curve_id: &inst.foreign_discount_curve_id,
        vol_surface_id: inst.vol_surface_id.as_str(),
        strike: inst.strike,
        instrument_pricing_overrides: &inst.instrument_pricing_overrides,
        spot_id: None,
        rate_context: "FxOption",
    }
}

pub(crate) fn implied_vol(
    inst: &FxOption,
    curves: &MarketContext,
    as_of: Date,
    target_price: f64,
) -> Result<f64> {
    inst.validate()?;
    let FxOptionInputsNoVol {
        spot,
        r_domestic: r_d,
        r_foreign: r_f,
        t,
        ..
    } = collect_fx_option_inputs_no_vol(input_request(inst, curves, as_of))?;
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

    let target_unit = target_price / (inst.notional.amount() * inst.delivery.position_sign());

    finstack_quant_models::closed_form::bs_implied_vol(
        spot,
        inst.strike,
        r_d,
        r_f,
        t,
        target_unit,
        inst.option_type,
    )
}

pub(crate) fn compute_greeks(
    inst: &FxOption,
    curves: &MarketContext,
    as_of: Date,
) -> Result<FxOptionGreeks> {
    #[cfg(test)]
    COMPUTE_GREEKS_CALLS.with(|count| count.set(count.get() + 1));
    if crate::market_inputs::event_has_occurred(inst.expiry, as_of) {
        return Ok(FxOptionGreeks::default());
    }
    inst.validate()?;
    let FxOptionInputs {
        spot,
        r_domestic: r_d,
        r_foreign: r_f,
        sigma,
        t,
        spot_conversion,
        ..
    } = collect_fx_option_inputs(input_request(inst, curves, as_of))?;
    if t <= 0.0 || inst.strike.abs() < STRIKE_ZERO_TOL {
        // At the expiry kink use half of each one-sided delta, consistently
        // across all four quoting conventions. Zero-strike puts are worthless.
        let call_probability = if t > 0.0 || spot > inst.strike {
            1.0
        } else if spot < inst.strike {
            0.0
        } else {
            0.5
        };
        let signed_probability = if inst.option_type == OptionType::Call {
            call_probability
        } else {
            call_probability - 1.0
        };
        let foreign_df = (-r_f * t).exp();
        let domestic_df = (-r_d * t).exp();
        let delta = foreign_df * signed_probability;
        let forward = spot * foreign_df / domestic_df;
        let premium_in_base = inst.delta_convention.premium_currency == inst.base_currency;
        let scale = inst.notional.amount() * inst.delivery.position_sign();
        let pv = if t > 0.0 {
            scale * spot * foreign_df * signed_probability
        } else {
            0.0
        };
        return Ok(FxOptionGreeks {
            delta: delta * scale * spot_conversion,
            delta_forward: signed_probability * scale,
            delta_premium_adjusted_spot: if premium_in_base {
                inst.strike / spot * domestic_df * signed_probability * scale
            } else {
                delta * scale
            },
            delta_premium_adjusted_forward: if premium_in_base {
                inst.strike / forward * signed_probability * scale
            } else {
                signed_probability * scale
            },
            rho_foreign_bp: -t * pv * 0.0001,
            theta: r_f * pv / inst.metric_pricing_overrides.theta_days_per_year(),
            ..Default::default()
        });
    }

    let greeks_unit = bs_greeks_unchecked(
        &OptionMarketParams {
            spot,
            strike: inst.strike,
            rate: r_d,
            volatility: sigma,
            time_to_expiry: t,
            dividend_yield: r_f,
            option_type: inst.option_type,
        },
        inst.metric_pricing_overrides.theta_days_per_year(),
    );
    let (d1, d2) = finstack_quant_models::closed_form::d1_d2(spot, inst.strike, r_d, sigma, t, r_f);
    let cdf_d1 = finstack_quant_core::math::special_functions::norm_cdf(d1)?;
    let cdf_d2 = finstack_quant_core::math::special_functions::norm_cdf(d2)?;
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

    let scale = inst.notional.amount() * inst.delivery.position_sign();
    let day_count = inst.day_count;
    let delivery_scale = day_count.year_fraction(
        as_of,
        inst.delivery.value_date(inst.expiry),
        Default::default(),
    )? / t;
    let spot_time = match inst.spot_basis {
        crate::FxSpotBasis::Cash => 0.0,
        crate::FxSpotBasis::ValueDate { date } => {
            day_count.year_fraction(as_of, date, Default::default())?
        }
    };
    let bridge_rho = spot_time * spot * greeks_unit.delta * scale * 0.0001;
    Ok(FxOptionGreeks {
        delta: greeks_unit.delta * scale * spot_conversion,
        delta_forward: delta_forward_unit * scale,
        delta_premium_adjusted_spot: delta_premium_adjusted_spot_unit * scale,
        delta_premium_adjusted_forward: delta_premium_adjusted_forward_unit * scale,
        gamma: greeks_unit.gamma * scale * spot_conversion * spot_conversion,
        vega: greeks_unit.vega * scale,
        theta: greeks_unit.theta * scale,
        rho_domestic_bp: greeks_unit.rho_r * scale * delivery_scale / 100.0 - bridge_rho,
        rho_foreign_bp: greeks_unit.rho_q * scale * delivery_scale / 100.0 + bridge_rho,
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
    /// Quote PV per positive domestic basis point on the option model clock.
    pub(crate) rho_domestic_bp: f64,
    /// Quote PV per positive foreign basis point on the option model clock.
    pub(crate) rho_foreign_bp: f64,
}

#[cfg(test)]
mod delegation_tests {
    use super::*;
    use crate::fx_option::{FxDeltaConvention, FxDeltaConventionKind};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::market_data::surfaces::{
        VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
    };
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
    use finstack_quant_core::types::Attributes;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use finstack_quant_valuations_core::instruments::Instrument;
    use std::sync::Arc;
    use time::macros::date;

    fn build_market(as_of: Date) -> MarketContext {
        let usd_curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, (-0.03_f64).exp())])
            .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("usd curve");
        let eur_curve = DiscountCurve::builder("EUR-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, (-0.01_f64).exp())])
            .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("eur curve");
        let vol_surface = VolSurface::builder("EURUSD-VOL")
            .expiries(&[0.25, 0.5, 1.0, 2.0])
            .secondary_values(&[0.9, 1.0, 1.1, 1.2, 1.3])
            .secondary_axis(VolSurfaceAxis::Strike)
            .quote_type(VolQuoteType::BlackLognormal)
            .interpolation_mode(VolInterpolationMode::Vol)
            .row(&[0.15; 5])
            .row(&[0.15; 5])
            .row(&[0.15; 5])
            .row(&[0.15; 5])
            .build()
            .expect("vol surface");
        let provider = SimpleFxProvider::new();
        provider
            .set_quotes(&[(Currency::EUR, Currency::USD, 1.20)])
            .expect("valid rate");
        let fx_matrix = FxMatrix::new(Arc::new(provider), None);

        MarketContext::new()
            .try_insert(usd_curve)
            .expect("insert market curve")
            .try_insert(eur_curve)
            .expect("insert market curve")
            .insert_surface(vol_surface)
            .insert_fx(fx_matrix)
    }

    fn build_option(expiry: Date) -> FxOption {
        FxOption::builder()
            .spot_basis(crate::FxSpotBasis::Cash)
            .delivery(crate::fx_option::FxOptionDelivery::CashAtExpiry)
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
    fn deliverable_put_call_parity_and_local_rho_use_booked_dates() {
        use crate::FxSpotBasis;
        use crate::fx_option::{FxOptionDelivery, FxOptionPosition};
        use finstack_quant_core::market_data::bumps::BumpSpec;
        use finstack_quant_core::market_data::traits::Discounting;

        let as_of = date!(2025 - 01 - 02);
        let spot_date = date!(2025 - 01 - 06);
        let expiry = date!(2025 - 07 - 02);
        let delivery = date!(2025 - 07 - 07);
        let mut market = build_market(as_of);
        for (id, knots) in [
            ("USD-OIS", [(0.0, 1.0), (0.25, 0.98), (1.0, 0.94)]),
            ("EUR-OIS", [(0.0, 1.0), (0.25, 0.995), (1.0, 0.97)]),
        ] {
            let curve = DiscountCurve::builder(id)
                .base_date(as_of)
                .day_count(DayCount::Act365F)
                .knots(knots)
                .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .expect("sloped curve");
            market.try_insert_mut(curve).expect("curve");
        }
        let mut call = build_option(expiry);
        call.spot_basis = FxSpotBasis::ValueDate { date: spot_date };
        call.delivery = FxOptionDelivery::Physical {
            date: delivery,
            position: FxOptionPosition::Long,
            exercised: None,
        };
        let mut put = call.clone();
        put.option_type = OptionType::Put;
        let pv = |option: &FxOption, market: &MarketContext| {
            compute_pv(option, market, as_of)
                .expect("option PV")
                .amount()
        };

        let domestic = market.get_discount("USD-OIS").expect("discount curve");
        let foreign = market.get_discount("EUR-OIS").expect("base curve");
        let cash_spot = 1.2 * domestic.df_between_dates(as_of, spot_date).unwrap()
            / foreign.df_between_dates(as_of, spot_date).unwrap();
        let parity = call.notional.amount()
            * (cash_spot * foreign.df_between_dates(as_of, delivery).unwrap()
                - call.strike * domestic.df_between_dates(as_of, delivery).unwrap());
        assert!((pv(&call, &market) - pv(&put, &market) - parity).abs() < 1e-7);

        let mut sold = call.clone();
        sold.delivery = FxOptionDelivery::Physical {
            date: delivery,
            position: FxOptionPosition::Short,
            exercised: None,
        };
        assert_eq!(pv(&call, &market), -pv(&sold, &market));

        let greeks = compute_greeks(&call, &market, as_of).expect("option Greeks");
        let sold_greeks = compute_greeks(&sold, &market, as_of).expect("short option Greeks");
        assert_eq!(greeks.delta, -sold_greeks.delta);
        assert_eq!(greeks.rho_domestic_bp, -sold_greeks.rho_domestic_bp);

        for (curve, analytic) in [
            (&call.domestic_discount_curve_id, greeks.rho_domestic_bp),
            (&call.foreign_discount_curve_id, greeks.rho_foreign_bp),
        ] {
            let shocked_pv = |bp| {
                let mut shocked = market.clone();
                shocked
                    .apply_curve_bump_in_place(curve, BumpSpec::parallel_bp(bp))
                    .expect("rate bump");
                pv(&call, &shocked)
            };
            let central = (shocked_pv(1.0) - shocked_pv(-1.0)) / 2.0;
            assert!(
                (analytic - central).abs() < 1e-4,
                "{curve}: {analytic} versus {central}"
            );
        }
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
    #[test]
    fn limiting_payoffs_populate_all_delta_conventions_and_foreign_rho() {
        let as_of = date!(2025 - 01 - 01);
        let market = build_market(as_of);
        let mut option = build_option(as_of);
        option.strike = 1.0;
        let g = compute_greeks(&option, &market, as_of).unwrap();
        assert_eq!(g.delta_forward, 1_000_000.0);
        assert_eq!(g.delta_premium_adjusted_forward, 1_000_000.0);
        option.delta_convention.premium_currency = Currency::EUR;
        let g = compute_greeks(&option, &market, as_of).unwrap();
        assert!((g.delta_premium_adjusted_forward - 1_000_000.0 / 1.2).abs() < 1e-9);
        option.expiry = date!(2026 - 01 - 01);
        option.strike = 0.0;
        let g = compute_greeks(&option, &market, as_of).unwrap();
        let pv = compute_pv(&option, &market, as_of).unwrap().amount();
        assert_eq!(g.delta_forward, 1_000_000.0);
        assert_eq!(g.delta_premium_adjusted_forward, 0.0);
        assert_eq!(g.delta_premium_adjusted_spot, 0.0);
        assert!((g.rho_foreign_bp + pv * 0.0001).abs() < 1e-8);
        option.option_type = OptionType::Put;
        let g = compute_greeks(&option, &market, as_of).unwrap();
        assert_eq!(g.delta_forward, 0.0);
        assert_eq!(g.rho_foreign_bp, 0.0);
    }

    #[test]
    fn fx_delta_vol_surface_propagates_cirp_forward_instead_of_expiry() {
        use finstack_quant_core::dates::{DayCountContext, DayCountContextState, VolatilityClock};
        use finstack_quant_core::market_data::surfaces::{
            FxAtmConvention, FxButterflyConvention, FxDeltaConvention, FxDeltaVolSurface,
        };
        use finstack_quant_core::money::fx::CurrencyPair;
        use finstack_quant_models::volatility::{VolSource, get_fx_delta_vol};

        let as_of = date!(2024 - 01 - 01);
        let expiry = date!(2024 - 04 - 02);
        let mut option = build_option(expiry);
        option.strike = 1.25;
        option.vol_surface_id = CurveId::new("EURUSD-DELTA");

        let eurusd = CurrencyPair::try_new(Currency::EUR, Currency::USD).expect("eur/usd");
        let clock =
            VolatilityClock::try_new(as_of, DayCount::Act365F, DayCountContextState::default())
                .expect("clock");
        let fx_delta = FxDeltaVolSurface::builder("EURUSD-DELTA", eurusd)
            .delta_convention(FxDeltaConvention::ForwardUnadjusted)
            .atm_convention(FxAtmConvention::DeltaNeutralStraddle)
            .butterfly_convention(FxButterflyConvention::Smile)
            .expiries(&[0.10, 0.25, 0.50, 1.0])
            .atm_vols(&[0.09, 0.10, 0.11, 0.12])
            .rr_25d(&[0.025, 0.025, 0.025, 0.025])
            .bf_25d(&[0.008, 0.008, 0.008, 0.008])
            .expiry_clock(clock)
            .build()
            .expect("fx delta surface");
        let source = VolSource::FxDelta(Arc::new(fx_delta.clone()));
        assert!(source.get_vol(0.25, 1.20, option.strike).is_err());

        let market = build_market(as_of).insert_fx_delta_vol_surface(fx_delta.clone());
        let t = option
            .day_count
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("year fraction");
        let df_dom = market
            .get_discount("USD-OIS")
            .expect("usd")
            .df(t)
            .expect("df");
        let df_for = market
            .get_discount("EUR-OIS")
            .expect("eur")
            .df(t)
            .expect("df");
        let cirp_fwd = 1.20 * df_for / df_dom;

        let expected_vol = get_fx_delta_vol(&fx_delta, t, option.strike, cirp_fwd)
            .expect("expected fx delta vol at CIRP forward");
        let wrong_expiry_as_fwd_vol = get_fx_delta_vol(&fx_delta, t, option.strike, t)
            .expect("vol if expiry were passed as forward");
        assert!(
            (expected_vol - wrong_expiry_as_fwd_vol).abs() > 1e-3,
            "test smile must distinguish forward={cirp_fwd} from expiry={t}"
        );
        assert!(
            (source
                .get_fx_vol(t, option.strike, cirp_fwd)
                .expect("VolSource::get_fx_vol")
                - expected_vol)
                .abs()
                < 1e-12
        );

        let pv = compute_pv(&option, &market, as_of).expect("fx option pv from fx delta surface");
        let recovered = implied_vol(&option, &market, as_of, pv.amount()).expect("implied vol");
        assert!((recovered - expected_vol).abs() < 1e-6);
    }

    #[test]
    fn fx_delta_vol_surface_checks_pair_clock_and_variance_bridge() {
        use finstack_quant_core::dates::{DayCountContextState, VolatilityClock};
        use finstack_quant_core::market_data::surfaces::{
            FxAtmConvention, FxButterflyConvention, FxDeltaConvention, FxDeltaVolSurface,
        };
        use finstack_quant_core::money::fx::CurrencyPair;
        use finstack_quant_valuations_core::instruments::pricing_overrides::{
            InstrumentPricingOverrides, MarketQuoteOverrides,
        };

        let as_of = date!(2025 - 01 - 02);
        let expiry = date!(2025 - 02 - 01); // 30 calendar days
        let mut option = build_option(expiry);
        option.vol_surface_id = CurveId::new("EURUSD-DELTA");
        option.day_count = DayCount::Act360;

        let eurusd = CurrencyPair::try_new(Currency::EUR, Currency::USD).expect("eur/usd");
        let usdeur = CurrencyPair::try_new(Currency::USD, Currency::EUR).expect("usd/eur");
        let matching_clock =
            VolatilityClock::try_new(as_of, DayCount::Act365F, DayCountContextState::default())
                .expect("clock");
        let wrong_date_clock = VolatilityClock::try_new(
            date!(2025 - 01 - 03),
            DayCount::Act365F,
            DayCountContextState::default(),
        )
        .expect("wrong date clock");

        // 1. Pair mismatch errors.
        let reversed = FxDeltaVolSurface::builder("EURUSD-DELTA", usdeur)
            .delta_convention(FxDeltaConvention::ForwardUnadjusted)
            .atm_convention(FxAtmConvention::DeltaNeutralStraddle)
            .butterfly_convention(FxButterflyConvention::Smile)
            .expiries(&[0.05, 0.50, 1.0])
            .atm_vols(&[0.20, 0.20, 0.20])
            .rr_25d(&[0.0, 0.0, 0.0])
            .bf_25d(&[0.0, 0.0, 0.0])
            .expiry_clock(matching_clock.clone())
            .build()
            .expect("reversed");
        let market_rev = build_market(as_of).insert_fx_delta_vol_surface(reversed);
        assert!(compute_pv(&option, &market_rev, as_of).is_err());

        // 2. Missing expiry_clock errors.
        let clockless = FxDeltaVolSurface::builder("EURUSD-DELTA", eurusd)
            .delta_convention(FxDeltaConvention::ForwardUnadjusted)
            .atm_convention(FxAtmConvention::DeltaNeutralStraddle)
            .butterfly_convention(FxButterflyConvention::Smile)
            .expiries(&[0.05, 0.50, 1.0])
            .atm_vols(&[0.20, 0.20, 0.20])
            .rr_25d(&[0.0, 0.0, 0.0])
            .bf_25d(&[0.0, 0.0, 0.0])
            .build()
            .expect("clockless");
        let market_clockless = build_market(as_of).insert_fx_delta_vol_surface(clockless);
        assert!(compute_pv(&option, &market_clockless, as_of).is_err());

        // 3. Stale reference_date != as_of errors.
        let stale = FxDeltaVolSurface::builder("EURUSD-DELTA", eurusd)
            .delta_convention(FxDeltaConvention::ForwardUnadjusted)
            .atm_convention(FxAtmConvention::DeltaNeutralStraddle)
            .butterfly_convention(FxButterflyConvention::Smile)
            .expiries(&[0.05, 0.50, 1.0])
            .atm_vols(&[0.20, 0.20, 0.20])
            .rr_25d(&[0.0, 0.0, 0.0])
            .bf_25d(&[0.0, 0.0, 0.0])
            .expiry_clock(wrong_date_clock)
            .build()
            .expect("stale");
        let market_stale = build_market(as_of).insert_fx_delta_vol_surface(stale);
        assert!(compute_pv(&option, &market_stale, as_of).is_err());

        // 4. Active Act/365F clock bridges variance to Act/360 option clock.
        let bridged = FxDeltaVolSurface::builder("EURUSD-DELTA", eurusd)
            .delta_convention(FxDeltaConvention::ForwardUnadjusted)
            .atm_convention(FxAtmConvention::DeltaNeutralStraddle)
            .butterfly_convention(FxButterflyConvention::Smile)
            .expiries(&[0.05, 0.50, 1.0])
            .atm_vols(&[0.20, 0.20, 0.20])
            .rr_25d(&[0.0, 0.0, 0.0])
            .bf_25d(&[0.0, 0.0, 0.0])
            .expiry_clock(matching_clock)
            .build()
            .expect("bridged");
        let market_ok = build_market(as_of).insert_fx_delta_vol_surface(bridged);
        let pv_bridged = compute_pv(&option, &market_ok, as_of).expect("bridged pv");

        let mut override_opt = option;
        override_opt.instrument_pricing_overrides = InstrumentPricingOverrides {
            market_quotes: MarketQuoteOverrides {
                implied_volatility: Some(0.20 * (360.0_f64 / 365.0).sqrt()),
                ..Default::default()
            },
            ..Default::default()
        };
        let pv_override = compute_pv(&override_opt, &market_ok, as_of).expect("override pv");
        assert!((pv_bridged.amount() - pv_override.amount()).abs() < 1e-10);
    }

    #[test]
    fn deep_otm_pv_flushes_subnormal_without_conversion_overflow() {
        use crate::fx_option::FxOptionDelivery;
        use finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides;

        let as_of = date!(2025 - 01 - 01);
        let expiry = date!(2025 - 02 - 01);
        let option = FxOption::builder()
            .spot_basis(crate::FxSpotBasis::Cash)
            .delivery(FxOptionDelivery::CashAtExpiry)
            .id(InstrumentId::new("FXOPT-DEEP-OTM"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .strike(10.0)
            .option_type(OptionType::Call)
            .delta_convention(
                FxDeltaConvention::new(FxDeltaConventionKind::Forward, Currency::USD, "test")
                    .expect("delta convention"),
            )
            .expiry(expiry)
            .notional(Money::from((1_i64, Currency::EUR)))
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .vol_surface_id(CurveId::new("EURUSD-VOL"))
            .instrument_pricing_overrides(
                InstrumentPricingOverrides::default().with_implied_volatility(0.05),
            )
            .attributes(Attributes::new())
            .build()
            .expect("deep OTM option");

        let market = build_market(as_of);
        let pv = compute_pv(&option, &market, as_of).expect("deep OTM PV should not overflow");
        assert_eq!(pv, Money::from((0_i64, Currency::USD)));
    }
}
