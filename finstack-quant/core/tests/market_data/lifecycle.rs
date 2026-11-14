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
        mode: BumpMode::Additive,
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
        .base_date(base())
        .knots([(0.0, -10.0), (1.0, -5.0), (2.0, 5.0)])
        .build()
        .unwrap();
    for t in [0.0, 0.25, 1.0, 1.5, 2.0] {
        for bumped in [curve.apply_bump(par(0.0)), curve.apply_bump(pct(0.0))] {
            assert_eq!(bumped.unwrap().price(t), curve.price(t));
        }
        assert!(
            (curve.apply_bump(par(-2.0)).unwrap().price(t) - (curve.price(t) - 2.0)).abs() < 1e-12
        );
        assert!(
            (curve.apply_bump(pct(0.1)).unwrap().price(t) - curve.price(t) * 1.1).abs() < 1e-12
        );
    }
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(curve.apply_bump(par(invalid)).is_err());
        assert!(curve.apply_bump(pct(invalid)).is_err());
    }
    let vol_index = PriceCurve::builder("VIX")
        .kind(PriceCurveKind::VolIndex)
        .base_date(base())
        .knots([(0.0, 10.0), (1.0, 15.0)])
        .build()
        .unwrap();
    assert!(vol_index.apply_bump(par(-11.0)).is_err());
    assert!(vol_index.apply_bump(pct(-2.0)).is_err());
}

#[test]
fn price_and_cpi_rolls_preserve_the_new_origin_and_remaining_segment() {
    let price = PriceCurve::builder("PRICE")
        .base_date(base())
        .day_count(DayCount::Act365F)
        .knots([(0.0, 100.0), (1.0, 110.0), (2.0, 120.0)])
        .build()
        .unwrap();
    let cpi = InflationCurve::builder("CPI")
        .base_date(base())
        .day_count(DayCount::Act365F)
        .base_cpi(300.0)
        .knots([(0.0, 300.0), (1.0, 306.0), (2.0, 312.0)])
        .build()
        .unwrap();
    for days in [0, 183, 365, 548] {
        let dt = f64::from(days) / 365.0;
        let rolled_price = price.roll_forward(i64::from(days)).unwrap();
        let rolled_cpi = cpi.roll_forward(i64::from(days)).unwrap();
        for t in [0.0, 1e-8, 0.1, 0.25] {
            assert!((rolled_price.price(t) - price.price(t + dt)).abs() < 1e-10);
            assert!((rolled_cpi.cpi(t) - cpi.cpi(t + dt)).abs() < 1e-10);
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
        BasisSpreadCurve, DiscountCurve, ForwardCurve, HazardCurve,
    };

    let discount = DiscountCurve::builder("DISCOUNT")
        .base_date(base())
        .day_count(DayCount::Act360)
        .knots([(0.0, 1.0), (1.0, 0.95), (2.0, 0.9)])
        .build()
        .expect("discount curve");
    let forward = ForwardCurve::builder("FORWARD", 0.25)
        .base_date(base())
        .day_count(DayCount::Act365F)
        .knots([(0.0, 0.03), (1.0, 0.04), (2.0, 0.05)])
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
        .base_cpi(300.0)
        .knots([(0.0, 300.0), (1.0, 306.0), (2.0, 312.0)])
        .build()
        .expect("inflation curve");
    let price = PriceCurve::builder("PRICE")
        .base_date(base())
        .knots([(0.0, 100.0), (1.0, 110.0), (2.0, 120.0)])
        .build()
        .expect("price curve");
    let basis = BasisSpreadCurve::builder("BASIS")
        .base_date(base())
        .knots([(0.0, 0.001), (1.0, 0.002), (2.0, 0.003)])
        .build()
        .expect("basis curve");

    assert!(discount.roll_forward(-30).is_err());
    assert!(forward.roll_forward(-30).is_err());
    assert!(hazard.roll_forward(-30).is_err());
    assert!(inflation.roll_forward(-30).is_err());
    assert!(price.roll_forward(-30).is_err());
    assert!(basis.roll_forward(-30).is_err());

    let context = MarketContext::new()
        .insert(discount)
        .insert(forward)
        .insert(hazard)
        .insert(inflation)
        .insert(price)
        .insert(basis);
    assert!(context.roll_forward(-30).is_err());
}

#[test]
fn cpi_validation_survives_serialization() {
    let builder = |base_cpi| {
        InflationCurve::builder("CPI")
            .base_date(base())
            .base_cpi(base_cpi)
            .knots([(0.0, 300.0), (1.0, 306.0)])
    };
    for invalid in [f64::NAN, f64::INFINITY, 0.0, -300.0, 301.0] {
        assert!(builder(invalid).build().is_err());
    }
    let curve = builder(300.0).build().unwrap();
    for invalid in [0.0, -300.0, 301.0] {
        let mut state = serde_json::to_value(&curve).unwrap();
        state["base_cpi"] = serde_json::json!(invalid);
        assert!(serde_json::from_value::<InflationCurve>(state).is_err());
    }
    assert!((curve.inflation_rate(0.0, 1.0).unwrap() - 0.02).abs() < 1e-12);
}

#[test]
fn volatility_mutations_reject_invalid_values_without_changing_the_source() {
    let mut surface = VolSurface::builder("VOL")
        .expiries(&[1.0, 2.0])
        .strikes(&[90.0, 100.0])
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
