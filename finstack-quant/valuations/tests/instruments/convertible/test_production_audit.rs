//! Independent convertible exercise, source-resolution and metric contracts.

use super::fixtures::{create_floating_convertible, create_standard_convertible};
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_valuations::instruments::fixed_income::bond::{CallPut, CallPutSchedule};
use finstack_quant_valuations::instruments::fixed_income::convertible::{
    price_convertible_bond, ConversionPolicy, ConvertibleTreeType,
};
use finstack_quant_valuations::instruments::{Instrument, PricingOptions};
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

fn market(as_of: Date, rate: f64, spot: f64, vol: f64) -> MarketContext {
    MarketContext::new()
        .insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                // Flat continuous rate; three pillars let theta roll one day.
                .knots([
                    (0.0, 1.0),
                    (5.0, (-rate * 5.0).exp()),
                    (10.0, (-rate * 10.0).exp()),
                ])
                .build()
                .expect("discount"),
        )
        .insert_price("AAPL", MarketScalar::Unitless(spot))
        .insert_price("AAPL-VOL", MarketScalar::Unitless(vol))
        .insert_price("AAPL-DIVYIELD", MarketScalar::Unitless(0.0))
}

#[test]
fn production_convertible_clean_call_includes_accrued() {
    let as_of = date!(2025 - 04 - 01);
    let mut bond = create_standard_convertible();
    bond.maturity = date!(2026 - 01 - 01);
    bond.conversion.ratio = Some(0.001);
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: as_of,
            end_date: as_of + time::Duration::days(1),
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: vec![],
    });
    let expected = 1000.0 + 50.0 * 90.0 / 365.0;
    for tree in [
        ConvertibleTreeType::Binomial(100),
        ConvertibleTreeType::Trinomial(100),
    ] {
        let actual = price_convertible_bond(&bond, &market(as_of, 0.0, 1.0, 0.2), tree, as_of)
            .expect("callable value")
            .amount();
        assert!(
            (actual - expected).abs() < 1e-8,
            "{tree:?}: {actual} vs {expected}"
        );
    }
}

#[test]
fn production_convertible_clean_put_includes_accrued() {
    let as_of = date!(2025 - 04 - 01);
    let mut bond = create_standard_convertible();
    bond.maturity = date!(2026 - 01 - 01);
    bond.conversion.ratio = Some(0.001);
    bond.call_put = Some(CallPutSchedule {
        calls: vec![],
        puts: vec![CallPut {
            start_date: as_of,
            end_date: as_of + time::Duration::days(1),
            price_pct_of_par: 120.0,
            make_whole: None,
        }],
    });
    let expected = 1200.0 + 50.0 * 90.0 / 365.0;
    for tree in [
        ConvertibleTreeType::Binomial(100),
        ConvertibleTreeType::Trinomial(100),
    ] {
        let actual = price_convertible_bond(&bond, &market(as_of, 0.0, 1.0, 0.2), tree, as_of)
            .expect("puttable value")
            .amount();
        assert!(
            (actual - expected).abs() < 1e-8,
            "{tree:?}: {actual} vs {expected}"
        );
    }
}

#[test]
fn production_convertible_discrete_call_on_valuation_date_is_exercisable() {
    let as_of = date!(2025 - 04 - 01);
    let mut bond = create_standard_convertible();
    bond.maturity = date!(2026 - 01 - 01);
    bond.conversion.ratio = Some(0.001);
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: as_of,
            end_date: as_of,
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: vec![],
    });
    let actual = bond
        .value(&market(as_of, 0.0, 1.0, 0.2), as_of)
        .expect("discrete exercise")
        .amount();
    assert!((actual - (1000.0 + 50.0 * 90.0 / 365.0)).abs() < 1e-8);
}

#[test]
fn production_convertible_coupon_date_call_does_not_duplicate_coupon() {
    let as_of = date!(2025 - 01 - 01);
    let mut bond = create_standard_convertible();
    bond.maturity = date!(2026 - 01 - 01);
    bond.conversion.ratio = Some(0.001);
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: date!(2025 - 07 - 01),
            end_date: date!(2025 - 07 - 01),
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: vec![],
    });
    for (day_count, coupon_fraction) in [
        (DayCount::Act365F, 181.0 / 365.0),
        (DayCount::Thirty360, 0.5),
    ] {
        bond.fixed_coupon
            .as_mut()
            .expect("coupon")
            .schedule
            .day_count = day_count;
        let actual = price_convertible_bond(
            &bond,
            &market(as_of, 0.0, 1.0, 0.2),
            ConvertibleTreeType::Binomial(365),
            as_of,
        )
        .expect("coupon-date exercise")
        .amount();
        assert!(
            (actual - (1000.0 + 50.0 * coupon_fraction)).abs() < 1e-8,
            "{day_count:?}: {actual}"
        );
    }
}

#[test]
fn production_convertible_make_whole_reference_and_floor_pay_accrued_once() {
    use finstack_quant_valuations::instruments::fixed_income::bond::MakeWholeSpec;
    let as_of = date!(2025 - 04 - 01);
    let mut bond = create_standard_convertible();
    bond.maturity = date!(2026 - 01 - 01);
    bond.conversion.ratio = Some(0.001);
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: as_of,
            end_date: as_of,
            price_pct_of_par: 100.0,
            make_whole: Some(MakeWholeSpec {
                reference_curve_id: "REFERENCE".into(),
                spread_bp: 0.0,
            }),
        }],
        puts: vec![],
    });
    for (reference_rate, expected) in [(0.0_f64, 1050.0), (0.3, 1000.0 + 50.0 * 90.0 / 365.0)] {
        let ctx = market(as_of, 0.0, 1.0, 0.2).insert(
            DiscountCurve::builder("REFERENCE")
                .base_date(as_of)
                .knots([(0.0, 1.0), (2.0, (-reference_rate * 2.0).exp())])
                .build()
                .expect("reference discount"),
        );
        let actual = bond.value(&ctx, as_of).expect("make-whole call").amount();
        assert!(
            (actual - expected).abs() < 1e-8,
            "{reference_rate}: {actual} vs {expected}"
        );
    }
}

#[test]
fn production_convertible_volatility_override_drives_price_and_greeks() {
    let as_of = date!(2025 - 01 - 01);
    let bond = create_standard_convertible();
    let mut overridden = bond.clone();
    overridden
        .instrument_pricing_overrides
        .market_quotes
        .implied_volatility = Some(0.4);
    let reference_market = market(as_of, 0.03, 90.0, 0.4);
    let inactive_market = market(as_of, 0.03, 90.0, 0.1);
    let reference = bond
        .greeks(&reference_market, None, None, as_of)
        .expect("reference Greeks");
    let actual = overridden
        .greeks(&inactive_market, None, None, as_of)
        .expect("override Greeks");
    assert!((actual.price - reference.price).abs() < 1e-9);
    assert!((actual.vega - reference.vega).abs() < 1e-9);
    assert!(actual.vega > 0.0);
}

#[test]
fn production_convertible_cross_gamma_follows_scalar_and_override_volatility() {
    let as_of = date!(2025 - 01 - 01);
    let bond = create_standard_convertible();
    let corner = |spot_sign: f64, vol_sign: f64| {
        bond.value(
            &market(
                as_of,
                0.03,
                90.0 * (1.0 + spot_sign * 0.01),
                0.4 + vol_sign * 0.01,
            ),
            as_of,
        )
        .expect("independent cross corner")
        .amount()
    };
    // Both moves are one percentage point, so the mixed stencil divides by 4.
    let expected =
        (corner(1.0, 1.0) - corner(1.0, -1.0) - corner(-1.0, 1.0) + corner(-1.0, -1.0)) / 4.0;
    assert!(expected.abs() > 1e-8);
    for use_override in [false, true] {
        let mut trial = bond.clone();
        if use_override {
            trial
                .instrument_pricing_overrides
                .market_quotes
                .implied_volatility = Some(0.4);
        }
        let ctx = market(as_of, 0.03, 90.0, if use_override { 0.1 } else { 0.4 });
        let result = trial
            .price_with_metrics(
                &ctx,
                as_of,
                &[MetricId::CrossGammaSpotVol],
                PricingOptions::default(),
            )
            .expect("cross gamma");
        let actual = result.measures["cross_gamma_spot_vol"];
        assert!(
            (actual - expected).abs() < 1e-8,
            "override={use_override}: {actual} vs {expected}"
        );
    }
}

#[test]
fn production_convertible_cross_gamma_uses_risky_discount_curve() {
    let as_of = date!(2025 - 01 - 01);
    let mut bond = create_standard_convertible();
    bond.credit_curve_id = Some("CREDIT".into());
    bond.recovery_rate = Some(0.4);
    let make_market = |spread_sign: f64, vol_sign: f64| {
        market(as_of, 0.03, 90.0, 0.4 + vol_sign * 0.01).insert(
            DiscountCurve::builder("CREDIT")
                .base_date(as_of)
                .knots([
                    (0.0, 1.0),
                    (10.0, (-(0.08 + spread_sign * 0.0001) * 10.0).exp()),
                ])
                .build()
                .expect("risky discount"),
        )
    };
    let corner = |spread_sign, vol_sign| {
        bond.value(&make_market(spread_sign, vol_sign), as_of)
            .expect("independent credit/vol corner")
            .amount()
    };
    let expected =
        (corner(1.0, 1.0) - corner(1.0, -1.0) - corner(-1.0, 1.0) + corner(-1.0, -1.0)) / 4.0;
    assert!(expected.abs() > 1e-8);
    let result = bond
        .price_with_metrics(
            &make_market(0.0, 0.0),
            as_of,
            &[MetricId::CrossGammaCreditVol],
            PricingOptions::default(),
        )
        .expect("credit/vol cross gamma");
    let actual = result.measures["cross_gamma_credit_vol"];
    assert!((actual - expected).abs() < 1e-8, "{actual} vs {expected}");
}

#[test]
fn production_convertible_surface_uses_conversion_strike() {
    let as_of = date!(2025 - 01 - 01);
    let bond = create_standard_convertible();
    let reference = market(as_of, 0.03, 80.0, 0.4);
    let surface = MarketContext::new()
        .insert(
            reference
                .get_discount("USD-OIS")
                .expect("discount")
                .as_ref()
                .clone(),
        )
        .insert_price("AAPL", MarketScalar::Unitless(80.0))
        .insert_price("AAPL-DIVYIELD", MarketScalar::Unitless(0.0))
        .insert_surface(
            VolSurface::builder("AAPL-VOL")
                .expiries(&[1.0, 6.0])
                .strikes(&[80.0, 100.0])
                .row(&[0.1, 0.4])
                .row(&[0.1, 0.4])
                .build()
                .expect("smile"),
        );
    let expected = bond
        .value(&reference, as_of)
        .expect("scalar price")
        .amount();
    let actual = bond.value(&surface, as_of).expect("surface price").amount();
    assert!((actual - expected).abs() < 1e-9, "{actual} vs {expected}");
}

#[test]
fn production_convertible_floating_coupons_use_the_forward_curve() {
    let as_of = date!(2025 - 01 - 01);
    let mut bond = create_floating_convertible();
    let floating = bond.floating_coupon.as_mut().expect("floating coupon");
    floating.rate_spec.reset_lag_days = 0;
    floating.rate_spec.fallback = finstack_quant_cashflows::builder::FloatingRateFallback::Error;
    let ctx = market(as_of, 0.0, 0.001, 0.2).insert(
        ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(as_of)
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.04), (2.0, 0.04)])
            .build()
            .expect("forward"),
    );
    let expected = 1000.0 + 1000.0 * 0.04 * 365.0 / 360.0;
    let actual = bond
        .value(&ctx, as_of)
        .expect("project floating coupons")
        .amount();
    assert!((actual - expected).abs() < 1e-8, "{actual} vs {expected}");
}

#[test]
fn production_convertible_seasoned_floater_uses_fixings_and_current_accrual() {
    use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
    let as_of = date!(2025 - 04 - 02);
    let mut bond = create_floating_convertible();
    let floating = bond.floating_coupon.as_mut().expect("floating coupon");
    floating.rate_spec.reset_lag_days = 0;
    floating.rate_spec.fallback = finstack_quant_cashflows::builder::FloatingRateFallback::Error;
    let ctx = market(as_of, 0.0, 0.001, 0.2).insert(
        ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(as_of)
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.07), (2.0, 0.07)])
            .build()
            .expect("projection"),
    );
    assert!(
        bond.value(&ctx, as_of).is_err(),
        "past fixings must be supplied"
    );
    let ctx = ctx.insert_series(
        ScalarTimeSeries::new(
            "FIXING:USD-SOFR-3M",
            vec![(date!(2025 - 01 - 01), 0.03), (date!(2025 - 04 - 01), 0.05)],
            None,
        )
        .expect("fixings"),
    );
    let result = bond
        .price_with_metrics(&ctx, as_of, &[MetricId::Accrued], PricingOptions::default())
        .expect("seasoned floater");
    let expected = 1000.0 + 1000.0 * (0.05 * 91.0 + 0.07 * 184.0) / 360.0;
    assert!((result.value.amount() - expected).abs() < 1e-8);
    assert!((result.measures["accrued"] - 1000.0 * 0.05 / 360.0).abs() < 1e-10);
}

#[test]
fn production_convertible_bond_floor_uses_the_pricing_recovery_blend() {
    let as_of = date!(2025 - 01 - 01);
    let mut bond = create_standard_convertible();
    bond.fixed_coupon = None;
    bond.maturity = as_of + time::Duration::days(200);
    bond.credit_curve_id = Some("CREDIT".into());
    bond.recovery_rate = Some(0.4);
    bond.metric_pricing_overrides = bond.metric_pricing_overrides.with_rate_bump(1.0);
    let ctx = market(as_of, 0.03, 0.001, 0.2).insert(
        DiscountCurve::builder("CREDIT")
            .base_date(as_of)
            .knots([(0.0, 1.0), (10.0, (-0.08_f64 * 10.0).exp())])
            .build()
            .expect("zero-recovery risky discount"),
    );
    let result = bond
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::custom("bond_floor"), MetricId::Dv01],
            PricingOptions::default(),
        )
        .expect("floor metric");
    let years = (bond.maturity - as_of).whole_days() as f64 / 365.0;
    // 200 daily steps: independent product of the specified forward-DF blend.
    let expected =
        1000.0 * (0.6 * (-0.08_f64 / 365.0).exp() + 0.4 * (-0.03_f64 / 365.0).exp()).powi(200);
    assert!((result.value.amount() - expected).abs() < 1e-8);
    assert!((result.measures["bond_floor"] - expected).abs() < 1e-8);
    let expected_dv01 = 0.5 * expected * ((-0.0001 * years).exp() - (0.0001 * years).exp());
    assert!(
        (result.measures["dv01"] - expected_dv01).abs() < 1e-8,
        "full parallel rate risk {} vs {expected_dv01}",
        result.measures["dv01"]
    );
}

#[test]
fn production_convertible_risky_discount_is_a_rate_dependency() {
    let mut bond = create_standard_convertible();
    bond.credit_curve_id = Some("CREDIT".into());
    bond.recovery_rate = Some(0.4);
    let deps = bond.market_dependencies().expect("dependencies");
    assert!(deps
        .curves
        .discount_curves
        .iter()
        .any(|id| id.as_str() == "CREDIT"));
}

#[test]
fn production_convertible_mandatory_parity_uses_variable_delivery() {
    let mut bond = create_standard_convertible();
    bond.conversion.policy = ConversionPolicy::MandatoryVariable {
        conversion_date: bond.maturity,
        lower_conversion_price: 80.0,
        upper_conversion_price: 120.0,
    };
    for (spot, expected) in [(40.0, 0.5), (90.0, 1.0), (180.0, 1.5)] {
        let actual = bond
            .parity(&market(date!(2025 - 01 - 01), 0.03, spot, 0.2))
            .expect("parity");
        assert!(
            (actual - expected).abs() < 1e-12,
            "{spot}: {actual} vs {expected}"
        );
    }
}

#[test]
fn production_convertible_implied_vol_uses_a_valid_lattice_bracket() {
    let as_of = date!(2025 - 01 - 01);
    for volatility in [0.1, 0.8] {
        let mut bond = create_standard_convertible();
        bond.fixed_coupon = None;
        let ctx = market(as_of, 0.05, 90.0, volatility);
        let target = bond.value(&ctx, as_of).expect("target PV").amount();
        bond.instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price = Some(target / 10.0);
        let result = bond
            .price_with_metrics(
                &ctx,
                as_of,
                &[MetricId::ImpliedVol],
                PricingOptions::default(),
            )
            .expect("implied volatility with nonzero drift");
        assert!((result.measures["implied_vol"] - volatility).abs() < 1e-6);
    }
}

#[test]
fn production_convertible_implied_vol_preserves_selected_engine() {
    use finstack_quant_valuations::pricer::{
        expect_inst, InstrumentType, ModelKey, Pricer, PricerKey, PricerRegistry, PricingError,
        PricingErrorContext,
    };
    use finstack_quant_valuations::results::ValuationResult;
    struct SelectedTree;
    impl Pricer for SelectedTree {
        fn key(&self) -> PricerKey {
            PricerKey::new(InstrumentType::Convertible, ModelKey::Tree)
        }
        fn price_dyn(
            &self,
            instrument: &dyn Instrument,
            market: &MarketContext,
            as_of: Date,
        ) -> Result<ValuationResult, PricingError> {
            let bond = expect_inst::<finstack_quant_valuations::instruments::ConvertibleBond>(
                instrument,
                InstrumentType::Convertible,
            )?;
            let value =
                price_convertible_bond(bond, market, ConvertibleTreeType::Trinomial(17), as_of)
                    .map_err(|error| {
                        PricingError::model_failure_with_context(
                            error.to_string(),
                            PricingErrorContext::default(),
                        )
                    })?;
            Ok(ValuationResult::stamped(bond.id(), as_of, value))
        }
    }
    let as_of = date!(2025 - 01 - 01);
    let mut bond = create_standard_convertible();
    bond.fixed_coupon = None;
    let ctx = market(as_of, 0.0, 90.0, 0.4);
    let target = price_convertible_bond(&bond, &ctx, ConvertibleTreeType::Trinomial(17), as_of)
        .expect("selected target")
        .amount();
    bond.instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price = Some(target / 10.0);
    let mut registry = PricerRegistry::new();
    registry
        .register(SelectedTree)
        .expect("register selected engine");
    let result = registry
        .price_with_metrics(
            &bond,
            ModelKey::Tree,
            &ctx,
            as_of,
            &[MetricId::ImpliedVol],
            PricingOptions::default(),
        )
        .expect("selected engine inversion");
    assert!(
        (result.measures["implied_vol"] - 0.4).abs() < 1e-6,
        "inversion switched engines: {}",
        result.measures["implied_vol"]
    );
}

#[test]
fn production_convertible_implied_vol_is_independent_of_trade_scale() {
    let as_of = date!(2025 - 01 - 01);
    for notional in [1_000.0, 1_000_000.0, 1_000_000_000.0] {
        let mut bond = create_standard_convertible();
        bond.fixed_coupon = None;
        bond.notional = finstack_quant_core::money::Money::new(notional, bond.notional.currency())
            .expect("notional");
        bond.conversion.ratio = Some(notional / 100.0);
        let ctx = market(as_of, 0.05, 90.0, 0.4);
        let target = bond.value(&ctx, as_of).expect("scaled PV").amount();
        bond.instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price = Some(100.0 * target / notional);
        let result = bond
            .price_with_metrics(
                &ctx,
                as_of,
                &[MetricId::ImpliedVol],
                PricingOptions::default(),
            )
            .expect("scaled implied volatility");
        assert!((result.measures["implied_vol"] - 0.4).abs() < 1e-6);
    }
}
