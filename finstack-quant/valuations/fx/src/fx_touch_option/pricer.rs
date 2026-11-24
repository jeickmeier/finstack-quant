//! FX touch option pricer implementation.

use crate::fx_touch_option::types::{FxTouchOption, TouchType};
use finstack_quant_core::Result;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{BarrierDirection, PayoutTiming};

pub(crate) fn compute_pv(
    inst: &FxTouchOption,
    curves: &MarketContext,
    as_of: Date,
) -> Result<Money> {
    if as_of > inst.expiry {
        return Ok(Money::from((0_i64, inst.quote_currency)));
    }
    let start = inst.monitoring_start_date.ok_or_else(|| {
        finstack_quant_core::Error::Validation(
            "FxTouchOption requires monitoring_start_date".to_string(),
        )
    })?;
    if as_of < start {
        return Err(finstack_quant_core::Error::Validation(
                "FxTouchOption analytical pricing requires monitoring_start_date on or before as_of; future monitoring windows are unsupported"
                    .to_string(),
            ));
    }
    if as_of > start && as_of <= inst.expiry && inst.observed_barrier_breached.is_none() {
        return Err(finstack_quant_core::Error::Validation(
            "Seasoned FX touch option requires observed_barrier_breached after monitoring starts"
                .to_string(),
        ));
    }
    let crate::market_inputs::FxOptionInputs {
        spot,
        r_domestic: r_d,
        r_foreign: r_f,
        sigma,
        t,
        ..
    } = crate::market_inputs::collect_fx_option_inputs(input_request(inst, curves, as_of))?;

    if t <= 0.0 {
        let observed_barrier_breached = inst.observed_barrier_breached.ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "Expired FX touch option requires explicit observed touch state".to_string(),
            )
        })?;
        let pv = match (
            inst.touch_type,
            observed_barrier_breached,
            inst.payout_timing,
        ) {
            (TouchType::OneTouch, true, PayoutTiming::AtHit)
            | (TouchType::OneTouch, false, _)
            | (TouchType::NoTouch, true, _) => 0.0,
            (TouchType::OneTouch, true, PayoutTiming::AtExpiry)
            | (TouchType::NoTouch, false, _) => inst.payout_amount.amount(),
        };
        return Money::new(pv, inst.quote_currency);
    }

    if inst.observed_barrier_breached == Some(true) {
        let pv = match (inst.touch_type, inst.payout_timing) {
            (TouchType::OneTouch, PayoutTiming::AtHit) | (TouchType::NoTouch, _) => 0.0,
            (TouchType::OneTouch, PayoutTiming::AtExpiry) => {
                (-r_d * t).exp() * inst.payout_amount.amount()
            }
        };
        return Money::from_model_value(pv, inst.quote_currency);
    }

    let price = price_touch(inst, spot, r_d, r_f, sigma, t)?;

    Money::from_model_value(price, inst.quote_currency)
}

/// Resolve the matrix-based spot needed by touch-option metrics.
pub(crate) fn fx_spot(inst: &FxTouchOption, curves: &MarketContext, as_of: Date) -> Result<f64> {
    input_request(inst, curves, as_of).spot()
}

fn input_request<'a>(
    inst: &'a FxTouchOption,
    curves: &'a MarketContext,
    as_of: Date,
) -> crate::market_inputs::FxOptionInputRequest<'a> {
    crate::market_inputs::FxOptionInputRequest {
        market: curves,
        as_of,
        base_currency: inst.base_currency,
        quote_currency: inst.quote_currency,
        expiry: inst.expiry,
        delivery: inst.expiry,
        spot_basis: crate::market_inputs::FxSpotBasis::Cash,
        day_count: inst.day_count,
        domestic_discount_curve_id: &inst.domestic_discount_curve_id,
        foreign_discount_curve_id: &inst.foreign_discount_curve_id,
        vol_surface_id: inst.vol_surface_id.as_str(),
        strike: inst.barrier,
        instrument_pricing_overrides: &inst.instrument_pricing_overrides,
        spot_id: None,
        rate_context: "FxTouchOption",
    }
}

/// Closed-form (Rubinstein-Reiner) touch value per unit of `inst`'s payout,
/// in quote currency, delegated to `models::closed_form::barrier_rebate`.
fn price_touch(
    inst: &FxTouchOption,
    spot: f64,
    r_d: f64,
    r_f: f64,
    sigma: f64,
    t: f64,
) -> Result<f64> {
    use finstack_quant_core::types::BarrierType;
    use finstack_quant_models::closed_form::{BarrierParams, barrier_rebate};

    let (barrier_type, timing) = match (inst.touch_type, inst.barrier_direction) {
        (TouchType::OneTouch, BarrierDirection::Up) => (BarrierType::UpAndOut, inst.payout_timing),
        (TouchType::OneTouch, BarrierDirection::Down) => {
            (BarrierType::DownAndOut, inst.payout_timing)
        }
        // A no-touch pays only if the barrier is never hit, so it settles at expiry.
        (TouchType::NoTouch, BarrierDirection::Up) => {
            (BarrierType::UpAndIn, PayoutTiming::AtExpiry)
        }
        (TouchType::NoTouch, BarrierDirection::Down) => {
            (BarrierType::DownAndIn, PayoutTiming::AtExpiry)
        }
    };
    let params = BarrierParams::new(spot, inst.barrier, inst.barrier, t, r_d, r_f, sigma);
    let value = barrier_rebate(&params, inst.payout_amount.amount(), barrier_type, timing);
    if !value.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "FxTouchOption {}: closed-form touch valuation did not converge \
             (spot={spot}, barrier={}, r_d={r_d}, r_f={r_f}, sigma={sigma}, t={t})",
            inst.id, inst.barrier
        )));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::market_data::surfaces::{
        VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
    };
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::market_data::traits::Discounting;
    use finstack_quant_core::money::Money;
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

    fn build_option(expiry: Date) -> FxTouchOption {
        FxTouchOption::builder()
            .id(InstrumentId::new("FX-TOUCH-TEST"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .barrier(1.10)
            .touch_type(crate::fx_touch_option::TouchType::OneTouch)
            .barrier_direction(finstack_quant_core::types::BarrierDirection::Down)
            .payout_amount(Money::from((100_000_i64, Currency::USD)))
            .payout_timing(finstack_quant_core::types::PayoutTiming::AtExpiry)
            .monitoring_start_date(date!(2024 - 01 - 01))
            .expiry(expiry)
            .day_count(DayCount::Act365F)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .vol_surface_id(CurveId::new("EURUSD-VOL"))
            .attributes(Attributes::new())
            .build()
            .expect("fx touch option")
    }

    #[test]
    fn fx_touch_pricer_compute_pv_matches_instrument_value() {
        let as_of = date!(2024 - 01 - 01);
        let expiry = date!(2025 - 01 - 01);
        let option = build_option(expiry);
        let market = build_market(as_of);

        let via_pricer = compute_pv(&option, &market, as_of).expect("pricer pv");
        let via_instrument = option.value(&market, as_of).expect("instrument pv");

        assert!((via_pricer.amount() - via_instrument.amount()).abs() < 1e-10);
        assert_eq!(via_pricer.currency(), via_instrument.currency());
    }

    fn build_no_touch(
        expiry: Date,
        timing: finstack_quant_core::types::PayoutTiming,
    ) -> finstack_quant_core::Result<FxTouchOption> {
        FxTouchOption::builder()
            .id(InstrumentId::new("FX-NOTOUCH-TEST"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .barrier(1.10)
            .touch_type(crate::fx_touch_option::TouchType::NoTouch)
            .barrier_direction(finstack_quant_core::types::BarrierDirection::Down)
            .payout_amount(Money::from((100_000_i64, Currency::USD)))
            .payout_timing(timing)
            .monitoring_start_date(date!(2024 - 01 - 01))
            .expiry(expiry)
            .day_count(DayCount::Act365F)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .vol_surface_id(CurveId::new("EURUSD-VOL"))
            .attributes(Attributes::new())
            .build()
    }

    #[test]
    fn no_touch_at_hit_is_rejected_during_construction() {
        use finstack_quant_core::types::PayoutTiming;
        let expiry = date!(2025 - 01 - 01);

        build_no_touch(expiry, PayoutTiming::AtExpiry).expect("no-touch at-expiry must construct");
        let error = build_no_touch(expiry, PayoutTiming::AtHit)
            .expect_err("no-touch at-hit must be rejected");
        assert!(error.to_string().contains("must be at_expiry"));
    }

    #[test]
    fn future_touch_monitoring_is_rejected_before_market_lookup() {
        let as_of = date!(2024 - 01 - 01);
        let mut option = build_option(date!(2025 - 01 - 01));
        option.monitoring_start_date = Some(option.expiry);
        for observed in [None, Some(false), Some(true)] {
            option.observed_barrier_breached = observed;
            let error = option
                .value(&MarketContext::new(), as_of)
                .expect_err("future monitoring");
            assert!(matches!(error, finstack_quant_core::Error::Validation(_)));
            assert!(error.to_string().contains("future monitoring windows"));
        }
    }

    #[test]
    fn zero_vol_no_touch_preserves_certain_payment() {
        let as_of = date!(2024 - 01 - 01);
        let expiry = date!(2025 - 01 - 01);
        let market = build_market(as_of)
            .try_insert(
                DiscountCurve::builder("EUR-OIS")
                    .base_date(as_of)
                    .day_count(DayCount::Act365F)
                    .knots([(0.0, 1.0), (1.0, (-0.03_f64).exp())])
                    .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                    .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                    .build()
                    .expect("flat curve"),
            )
            .expect("insert market curve");
        let df = market
            .get_discount("USD-OIS")
            .expect("discount curve")
            .df_between_dates(as_of, expiry)
            .expect("payment DF");
        let mut option = build_option(expiry);
        option
            .instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(0.0);
        for (direction, barrier) in [(BarrierDirection::Down, 1.10), (BarrierDirection::Up, 1.30)] {
            option.barrier_direction = direction;
            option.barrier = barrier;
            option.touch_type = TouchType::NoTouch;
            let no_touch = option.value(&market, as_of).expect("no touch").amount();
            assert!((no_touch - 100_000.0 * df).abs() < 1e-8);
            option.touch_type = TouchType::OneTouch;
            assert_eq!(
                option.value(&market, as_of).expect("one touch").amount(),
                0.0
            );
        }
    }

    #[test]
    fn zero_vol_touch_uses_drift_and_contractual_payment_time() {
        let as_of = date!(2024 - 01 - 01);
        let expiry = date!(2025 - 01 - 01);
        for (direction, domestic, foreign) in [
            (BarrierDirection::Up, 0.03_f64, 0.01_f64),
            (BarrierDirection::Down, 0.01_f64, 0.03_f64),
        ] {
            let mut market = build_market(as_of);
            for (id, rate) in [("USD-OIS", domestic), ("EUR-OIS", foreign)] {
                market = market
                    .try_insert(
                        DiscountCurve::builder(id)
                            .base_date(as_of)
                            .day_count(DayCount::Act365F)
                            .knots([(0.0, 1.0), (1.0, (-rate).exp())])
                            .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                            .extrapolation(
                                finstack_quant_core::math::interp::ExtrapolationPolicy::Extend,
                            )
                            .build()
                            .expect("flat curve"),
                    )
                    .expect("insert market curve");
            }
            let mut option = build_option(expiry);
            option.barrier_direction = direction;
            option.barrier = 1.20 * ((domestic - foreign) * 0.5).exp();
            option
                .instrument_pricing_overrides
                .market_quotes
                .implied_volatility = Some(0.0);
            option.payout_timing = PayoutTiming::AtHit;
            let at_hit = option.value(&market, as_of).expect("hit payment").amount();
            assert!((at_hit - 100_000.0 * (-domestic * 0.5).exp()).abs() < 1e-7);
            option.payout_timing = PayoutTiming::AtExpiry;
            let df = market
                .get_discount("USD-OIS")
                .expect("curve")
                .df_between_dates(as_of, expiry)
                .expect("DF");
            let at_expiry = option
                .value(&market, as_of)
                .expect("expiry payment")
                .amount();
            assert!((at_expiry - 100_000.0 * df).abs() < 1e-8);
            assert!(at_hit > at_expiry);
            option.touch_type = TouchType::NoTouch;
            assert_eq!(
                option.value(&market, as_of).expect("no touch").amount(),
                0.0
            );

            // Move the trigger beyond the expiry horizon: the payout flips
            // back to the no-touch while the two expiry payouts still sum to DF*Q.
            option.barrier = 1.20 * ((domestic - foreign) * 2.0).exp();
            assert!(
                (option.value(&market, as_of).expect("survival").amount() - 100_000.0 * df).abs()
                    < 1e-8
            );
            option.touch_type = TouchType::OneTouch;
            assert_eq!(option.value(&market, as_of).expect("miss").amount(), 0.0);
        }
    }

    #[test]
    fn negative_rate_quadrature_converges_for_at_hit_one_touch() {
        let mut option = build_option(date!(2025 - 01 - 01));
        option.touch_type = TouchType::OneTouch;
        option.barrier_direction = BarrierDirection::Down;
        option.payout_timing = PayoutTiming::AtHit;
        let pv = price_touch(&option, 1.20, -0.05, -0.05, 0.20, 1.0)
            .expect("lambda^2 < 0 quadrature should succeed");
        assert!(
            pv.is_finite() && pv > 0.0,
            "quadrature touch pv={pv} must be positive and finite"
        );
    }
}
