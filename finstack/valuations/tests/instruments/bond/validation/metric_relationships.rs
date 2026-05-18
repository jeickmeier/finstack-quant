//! Cross-metric validation tests.
//!
//! Tests fundamental relationships between bond metrics:
//! - Modified Duration = Macaulay Duration / (1 + YTM/m)
//! - DV01 ≈ Price × Modified Duration × 0.0001 (approximate relationship)
//! - Convexity and duration approximations
//!
//! ## Note on DV01 vs Duration
//!
//! The relationship `DV01 ≈ -Price × ModDur × 0.0001` is an **approximation** that
//! assumes both metrics use the same rate sensitivity. In practice:
//!
//! - **DV01**: Computed via parallel bump of the discount curve (continuous
//!   compounding on zero rates). Captures actual curve-based price change.
//!
//! - **Modified Duration**: Derived from YTM (periodic compounding, typically
//!   semi-annual). Represents yield-based sensitivity.
//!
//! For a par bond on a flat curve (coupon = yield), the difference is minimal
//! (typically 0.5-1.5%) because both methods are measuring the same underlying
//! rate sensitivity. The `BUMP_VS_ANALYTICAL` tolerance (1.5%) accounts for:
//! - Compounding convention differences (~0.6% for continuous vs semi-annual)
//! - Numerical precision in bump-and-reprice vs analytical formula

use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount, Tenor};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::money::Money;
use finstack_valuations::instruments::fixed_income::bond::{Bond, CashflowSpec};
use finstack_valuations::instruments::Instrument;
use finstack_valuations::instruments::PricingOverrides;
use finstack_valuations::metrics::MetricId;
use time::macros::date;

fn create_flat_curve(rate: f64, base_date: Date, curve_id: &str) -> DiscountCurve {
    DiscountCurve::builder(curve_id)
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (1.0, (-rate).exp()),
            (5.0, (-rate * 5.0).exp()),
            (10.0, (-rate * 10.0).exp()),
            (30.0, (-rate * 30.0).exp()),
        ])
        .build()
        .unwrap()
}

#[test]
fn test_modified_macaulay_duration_relationship() {
    let as_of = date!(2025 - 01 - 01);
    let bond = Bond::fixed(
        "DUR_REL",
        Money::new(100.0, Currency::USD),
        0.06,
        as_of,
        date!(2030 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();

    let curve = create_flat_curve(0.06, as_of, "USD-OIS");
    let market = MarketContext::new().insert(curve);

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::DurationMac, MetricId::DurationMod, MetricId::Ytm],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let mac_dur = *result.measures.get("duration_mac").unwrap();
    let mod_dur = *result.measures.get("duration_mod").unwrap();
    let ytm = *result.measures.get("ytm").unwrap();

    // ModDur = MacDur / (1 + ytm/m) for semi-annual
    let m = 2.0; // Semi-annual
    let expected_mod_dur = mac_dur / (1.0 + ytm / m);

    assert!((mod_dur - expected_mod_dur).abs() < 0.01);
}

#[test]
fn test_yield_dv01_duration_price_relationship() {
    // Test the direct relationship: Yield DV01 ≈ −Price × ModDur × 0.0001.
    //
    // This validates the new bond-specific yield-basis DV01 metric, which should
    // align tightly with modified duration because both are defined on the same
    // yield compounding basis.
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    // Use pricing override to ensure bond is at par (clean price = 100)
    let pricing_overrides = PricingOverrides::default().with_quoted_clean_price(100.0);

    let bond = Bond::builder()
        .id("DV01_REL".into())
        .notional(Money::new(100.0, Currency::USD))
        .cashflow_spec(CashflowSpec::fixed(
            0.08,
            Tenor::annual(),
            DayCount::Act365F,
        ).expect("finite test coupon"))
        .issue_date(as_of)
        .maturity(maturity)
        .discount_curve_id("USD_DISC".into())
        .pricing_overrides(pricing_overrides)
        .call_put_opt(None)
        .custom_cashflows_opt(None)
        .attributes(Default::default())
        .build()
        .unwrap();

    let curve = create_flat_curve(0.08, as_of, "USD_DISC");
    let market = MarketContext::new().insert(curve);

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::DurationMod, MetricId::YieldDv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let mod_dur = *result.measures.get("duration_mod").unwrap();
    let yield_dv01 = *result.measures.get("yield_dv01").unwrap();
    let price = result.value.amount();

    assert!(
        yield_dv01 < 0.0,
        "Yield DV01 should be negative for fixed-rate bond"
    );

    // Approximate relationship: Yield DV01 ≈ −Price × ModDur × 0.0001
    let approx_dv01 = -(price * mod_dur * 0.0001);
    let relative_diff = ((yield_dv01 - approx_dv01) / approx_dv01).abs();

    assert!(
        relative_diff < 0.015,
        "Yield DV01={:.6} differs from duration estimate {:.6} by {:.2}% (max 1.5%)",
        yield_dv01,
        approx_dv01,
        relative_diff * 100.0,
    );
}
