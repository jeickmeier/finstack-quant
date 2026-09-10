//! Public regressions for the September 2026 production audit.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::{MarketContext, MarketContextState};
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxQuery, SimpleFxProvider};
use std::sync::Arc;
use time::macros::date;

#[test]
fn m9_global_source_precedes_orientation_and_pinned_fixing() {
    let on = date!(2025 - 01 - 02);
    let matrix = FxMatrix::new(Arc::new(SimpleFxProvider::new()));
    matrix
        .set_quote(Currency::USD, Currency::EUR, 0.8)
        .expect("global");
    matrix
        .set_quote_on(
            Currency::EUR,
            Currency::USD,
            on,
            FxConversionPolicy::CashflowDate,
            1.3,
        )
        .expect("pinned");
    let forward = matrix
        .rate(FxQuery::new(Currency::EUR, Currency::USD, on))
        .expect("forward")
        .rate;
    let reverse = matrix
        .rate(FxQuery::new(Currency::USD, Currency::EUR, on))
        .expect("reverse")
        .rate;
    assert_eq!(forward, 1.25);
    assert!((forward * reverse - 1.0).abs() < 1e-12);
    matrix
        .validate_triangular(1e-6)
        .expect("effective quote snapshot is reciprocal");
    matrix
        .set_quote(Currency::USD, Currency::GBP, 0.8)
        .expect("pivot leg");
    let cross = matrix
        .rate(FxQuery::new(Currency::EUR, Currency::GBP, on))
        .expect("cross");
    assert!(
        (cross.rate - 1.0).abs() < 1e-12,
        "pivot legs must use global source first: {}",
        cross.rate
    );
    assert!(cross.triangulated);
}

#[test]
fn m9_snapshot_preserves_provider_below_pinned_source() {
    let on = date!(2025 - 01 - 02);
    let provider = SimpleFxProvider::new();
    provider
        .set_quote(Currency::USD, Currency::EUR, 0.8)
        .expect("provider quote");
    let matrix = FxMatrix::new(Arc::new(provider));
    matrix
        .set_quote_on(
            Currency::EUR,
            Currency::USD,
            on,
            FxConversionPolicy::CashflowDate,
            1.3,
        )
        .expect("pinned");
    let market = MarketContext::new().insert_fx(matrix);
    let state = MarketContextState::from(&market);
    let restored = MarketContext::try_from(state).expect("restore");
    for (from, to, expected) in [
        (Currency::EUR, Currency::USD, 1.3),
        (Currency::USD, Currency::EUR, 1.0 / 1.3),
    ] {
        let rate = restored
            .fx_required()
            .expect("fx")
            .rate(FxQuery::new(from, to, on))
            .expect("restored rate")
            .rate;
        assert!((rate - expected).abs() < 1e-12, "{from}/{to}: {rate}");
    }
    let other = restored
        .fx_required()
        .expect("fx")
        .rate(FxQuery::new(
            Currency::USD,
            Currency::EUR,
            date!(2025 - 01 - 03),
        ))
        .expect("provider remains available")
        .rate;
    assert_eq!(other, 0.8);
}

#[test]
fn m10_negative_rate_stress_keeps_mathematical_validation_and_roundtrips() {
    let base = date!(2025 - 01 - 02);
    let curve = DiscountCurve::builder("STRICT")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, (-0.01_f64).exp()),
            (2.0, (-0.02_f64).exp()),
        ])
        .build()
        .expect("positive-rate construction");
    let shocked = curve
        .with_parallel_bump(-200.0)
        .expect("negative-rate stress");
    assert!((shocked.df(2.0) - 0.02_f64.exp()).abs() < 1e-12);
    let roundtrip: DiscountCurve =
        serde_json::from_str(&serde_json::to_string(&shocked).expect("serialize"))
            .expect("stress roundtrip");
    assert_eq!(roundtrip.df(2.0), shocked.df(2.0));
    assert!(curve.with_parallel_bump(f64::NAN).is_err());
    assert!(curve.with_parallel_bump(-1e308).is_err());
    assert!((curve.df(2.0) - (-0.02_f64).exp()).abs() < 1e-12);
    assert!(DiscountCurve::builder("STRICT")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, 1.01)])
        .build()
        .is_err());
}
