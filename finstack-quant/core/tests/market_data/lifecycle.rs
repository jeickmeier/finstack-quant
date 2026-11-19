//! Economic identities across market-data transformations.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::bumps::{BumpMode, BumpSpec, BumpType, BumpUnits, Bumpable};
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::{
    InflationCurve, PriceCurve, PriceCurveKind,
};
use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxQuery};
use std::sync::Arc;

/// Parallel price shock in native price units.
fn par(value: f64) -> BumpSpec {
    BumpSpec {
        mode: BumpMode::Additive,
        units: BumpUnits::Fraction,
        value,
        bump_type: BumpType::Parallel,
    }
}

/// Parallel relative shock; `fraction = 0.1` scales every level by 1.1.
fn pct(fraction: f64) -> BumpSpec {
    BumpSpec {
        mode: BumpMode::Multiplicative,
        units: BumpUnits::Percent,
        value: fraction * 100.0,
        bump_type: BumpType::Parallel,
    }
}

fn base() -> Date {
    Date::from_ordinal_date(2025, 1).unwrap()
}

#[test]
fn signed_price_shocks_preserve_zero_identity_and_native_units() {
    let curve = PriceCurve::builder("POWER")
        .kind(finstack_quant_core::market_data::term_structures::PriceCurveKind::Price)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Flat)
        .base_date(base())
        .knots([(0.0, -10.0), (1.0, -5.0), (2.0, 5.0)])
        .build()
        .unwrap();
    for t in [0.0, 0.25, 1.0, 1.5, 2.0] {
        for bumped in [curve.apply_bump(par(0.0)), curve.apply_bump(pct(0.0))] {
            assert_eq!(bumped.unwrap().price(t).unwrap(), curve.price(t).unwrap());
        }
        assert!(
            (curve.apply_bump(par(-2.0)).unwrap().price(t).unwrap()
                - (curve.price(t).unwrap() - 2.0))
                .abs()
                < 1e-12
        );
        assert!(
            (curve.apply_bump(pct(0.1)).unwrap().price(t).unwrap() - curve.price(t).unwrap() * 1.1)
                .abs()
                < 1e-12
        );
    }
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(curve.apply_bump(par(invalid)).is_err());
        assert!(curve.apply_bump(pct(invalid)).is_err());
    }
    let vol_index = PriceCurve::builder("VIX")
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Flat)
        .kind(PriceCurveKind::VolIndex)
        .base_date(base())
        .knots([(0.0, 10.0), (1.0, 15.0)])
        .build()
        .unwrap();
    assert!(vol_index.apply_bump(par(-11.0)).is_err());
    assert!(vol_index.apply_bump(pct(-2.0)).is_err());
}

#[test]
fn price_and_cpi_condition_keeps_original_clocks_and_levels() {
    use finstack_quant_core::market_data::context::MarketContext;

    let price = PriceCurve::builder("PRICE")
        .kind(finstack_quant_core::market_data::term_structures::PriceCurveKind::Price)
        .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Flat)
        .base_date(base())
        .day_count(DayCount::Act365F)
        .knots([(0.0, 100.0), (1.0, 110.0), (2.0, 120.0)])
        .build()
        .unwrap();
    let cpi = InflationCurve::builder("CPI")
        .base_date(base())
        .day_count(DayCount::Act365F)
        .knots([(0.0, 300.0), (1.0, 306.0), (2.0, 312.0)])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Flat)
        .build()
        .unwrap();
    let context = MarketContext::new()
        .try_insert(price.clone())
        .expect("insert market curve")
        .try_insert(cpi.clone())
        .expect("insert market curve");
    for days in [0, 183, 365, 548] {
        let horizon = base() + time::Duration::days(i64::from(days));
        let conditioned = context.condition(horizon).unwrap();
        let kept_price = conditioned.get_price_curve("PRICE").unwrap();
        let kept_cpi = conditioned.get_inflation_curve("CPI").unwrap();
        assert_eq!(kept_price.base_date(), price.base_date());
        assert_eq!(kept_cpi.base_date(), cpi.base_date());
        for t in [0.0, 1e-8, 0.1, 0.25, 1.5] {
            assert!((kept_price.price(t).unwrap() - price.price(t).unwrap()).abs() < 1e-12);
            assert!((kept_cpi.cpi(t).unwrap() - cpi.cpi(t).unwrap()).abs() < 1e-12);
        }
    }
}

#[test]
fn derived_fx_shocks_preserve_dates_policies_and_reciprocals() {
    // Date- and policy-dependent pivot legs, held as scoped quotes so the
    // state can be frozen (record CORE-CONVENTIONS-013 #11).
    let dates = [base(), Date::from_ordinal_date(2025, 182).unwrap()];
    let policies = [
        FxConversionPolicy::CashflowDate,
        FxConversionPolicy::PeriodEnd,
    ];
    let matrix = FxMatrix::new(
        Arc::new(finstack_quant_core::money::fx::SimpleFxProvider::new()),
        Some(Currency::USD),
    );
    for on in dates {
        for policy in policies {
            let gbp_usd = if policy == FxConversionPolicy::PeriodEnd {
                1.3
            } else {
                1.25
            };
            matrix
                .set_quote_on(
                    Currency::EUR,
                    Currency::USD,
                    on,
                    policy,
                    1.1 + f64::from(on.ordinal()) / 1000.0,
                )
                .unwrap();
            matrix
                .set_quote_on(Currency::GBP, Currency::USD, on, policy, gbp_usd)
                .unwrap();
        }
    }
    let bumped = matrix
        .with_bumped_rate(
            FxQuery::new(
                Currency::EUR,
                Currency::GBP,
                base(),
                FxConversionPolicy::CashflowDate,
            ),
            0.1,
        )
        .unwrap();
    // The frozen scenario holds only quotes, so it captures and restores.
    let restored = FxMatrix::try_from_state(&bumped.get_serializable_state().unwrap()).unwrap();
    for fx in [&bumped, &restored] {
        for on in dates {
            for policy in policies {
                let query = FxQuery::new(Currency::EUR, Currency::GBP, on, policy);
                let reverse = FxQuery::new(Currency::GBP, Currency::EUR, on, policy);
                // Each orientation is its own baseline route, shocked once.
                assert_eq!(
                    fx.rate(query).unwrap().rate,
                    matrix.rate(query).unwrap().rate * 1.1
                );
                assert_eq!(
                    fx.rate(reverse).unwrap().rate,
                    matrix.rate(reverse).unwrap().rate / 1.1
                );
            }
        }
        // No leg is quoted outside those scopes, so the pair stays unpriced.
        let other = Date::from_ordinal_date(2025, 300).unwrap();
        assert!(
            fx.rate(FxQuery::new(
                Currency::EUR,
                Currency::GBP,
                other,
                FxConversionPolicy::CashflowDate
            ))
            .is_err()
        );
    }
}

#[test]
fn backward_roll_is_rejected_across_context_and_curves() {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{
        DiscountCurve, ForwardCurve, HazardCurve,
    };

    let discount = DiscountCurve::builder("DISCOUNT")
        .base_date(base())
        .day_count(DayCount::Act360)
        .knots([(0.0, 1.0), (1.0, 0.95), (2.0, 0.9)])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve");
    let forward = ForwardCurve::builder("FORWARD")
        .base_date(base())
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, 0.96), (2.0, 0.91)])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("forward curve");
    let hazard = HazardCurve::builder("HAZARD")
        .base_date(base())
        .day_count(DayCount::Act360)
        .recovery_rate(0.4)
        .knots([(1.0, 0.01), (2.0, 0.02)])
        .build()
        .expect("hazard curve");
    let inflation = InflationCurve::builder("INFLATION")
        .base_date(base())
        .day_count(DayCount::Act365F)
        .knots([(0.0, 300.0), (1.0, 306.0), (2.0, 312.0)])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Flat)
        .build()
        .expect("inflation curve");
    let price = PriceCurve::builder("PRICE")
        .kind(finstack_quant_core::market_data::term_structures::PriceCurveKind::Price)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Flat)
        .base_date(base())
        .knots([(0.0, 100.0), (1.0, 110.0), (2.0, 120.0)])
        .build()
        .expect("price curve");

    let back_date = base() - time::Duration::days(30);
    assert!(discount.condition(back_date).is_err());
    assert!(forward.condition(back_date).is_err());
    assert!(hazard.condition(back_date).is_err());

    let context = MarketContext::new()
        .try_insert(discount)
        .expect("insert market curve")
        .try_insert(forward)
        .expect("insert market curve")
        .try_insert(hazard)
        .expect("insert market curve")
        .try_insert(inflation)
        .expect("insert market curve")
        .try_insert(price)
        .expect("insert market curve");
    assert!(context.condition(back_date).is_err());
}

#[test]
fn cpi_validation_survives_serialization() {
    let builder = |origin_cpi| {
        InflationCurve::builder("CPI")
            .base_date(base())
            .day_count(DayCount::Act365F)
            .knots([(0.0, origin_cpi), (1.0, 306.0)])
            .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Flat)
    };
    for invalid in [f64::NAN, f64::INFINITY, 0.0, -300.0] {
        assert!(builder(invalid).build().is_err());
    }
    let curve = builder(300.0).build().unwrap();
    for invalid in [0.0, -300.0] {
        let mut state = serde_json::to_value(&curve).unwrap();
        state["knot_points"][0][1] = serde_json::json!(invalid);
        assert!(serde_json::from_value::<InflationCurve>(state).is_err());
    }
    let mut nonzero_origin = serde_json::to_value(&curve).unwrap();
    nonzero_origin["knot_points"][0][0] = serde_json::json!(0.1);
    assert!(serde_json::from_value::<InflationCurve>(nonzero_origin).is_err());
    assert!((curve.inflation_rate(0.0, 1.0).unwrap() - 0.02).abs() < 1e-12);
}

#[test]
fn volatility_mutations_reject_invalid_values_without_changing_the_source() {
    use finstack_quant_core::market_data::surfaces::{
        VolInterpolationMode, VolQuoteType, VolSurfaceAxis,
    };
    let mut surface = VolSurface::builder("VOL")
        .expiries(&[1.0, 2.0])
        .secondary_values(&[90.0, 100.0])
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(VolInterpolationMode::Vol)
        .row(&[0.2, 0.3])
        .row(&[0.25, 0.35])
        .build()
        .unwrap();
    let original = surface.vols().to_vec();
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(surface.scaled(invalid).is_err());
        assert!(
            surface
                .bump_point_absolute_in_place(1.0, 90.0, invalid)
                .is_err()
        );
        assert!(
            surface
                .bump_point_absolute_in_place(invalid, 90.0, 0.01)
                .is_err()
        );
        assert!(
            surface
                .bump_point_absolute_in_place(1.0, invalid, 0.01)
                .is_err()
        );
    }
    assert!(surface.scaled(-1.0).is_err());
    assert_eq!(surface.scaled(1.0).unwrap().vols(), original);
    assert!(
        surface
            .scaled(0.0)
            .unwrap()
            .vols()
            .iter()
            .all(|v| *v == 0.0)
    );
    assert_eq!(surface.vols(), original);
    let enormous = surface.scaled(f64::MAX).unwrap();
    assert!(enormous.scaled(10.0).is_err());
}
