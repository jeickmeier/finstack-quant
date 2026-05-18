//! CS01 calculator tests.

use finstack_core::currency::Currency;
use finstack_core::market_data::term_structures::HazardCurve;
use finstack_core::money::Money;
use finstack_core::types::CurveId;
use finstack_valuations::instruments::fixed_income::bond::Bond;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;
use time::macros::date;

#[test]
fn test_cs01_negative_for_long_bond() {
    let as_of = date!(2025 - 01 - 01);
    let mut bond = Bond::fixed(
        "CS1",
        Money::new(100.0, Currency::USD),
        0.05,
        as_of,
        date!(2030 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();

    bond.credit_curve_id = Some(CurveId::new("USD-CREDIT"));

    let disc = finstack_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (5.0, 0.80)])
        .build()
        .unwrap();

    let hazard = HazardCurve::builder("USD-CREDIT")
        .base_date(as_of)
        .recovery_rate(0.4)
        .knots([(0.0, 0.02), (5.0, 0.02)])
        .build()
        .unwrap();

    let market = finstack_core::market_data::context::MarketContext::new()
        .insert(disc)
        .insert(hazard);

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Cs01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let cs01 = *result.measures.get("cs01").unwrap();
    assert!(
        cs01 < 0.0,
        "Long bond CS01 should be negative (wider spreads reduce PV), got {}",
        cs01
    );
    assert!(
        cs01.abs() < 1.0,
        "Bond CS01 should be small for $100 notional, got {}",
        cs01.abs()
    );
}

/// Item 1 regression: the CS01 z-spread fallback (used when a bond has **no**
/// credit curve) must reprice on the same settlement-anchored discounting basis
/// the Z-spread was calibrated on.
///
/// `ZSpreadCalculator` solves the spread with time/discount-factors measured
/// from the bond's settlement (`quote_date`) and a compounding-aware spread
/// shift. The CS01 fallback must use the *identical* basis; otherwise
/// `base_npv` is not the dirty price the spread was solved to and CS01 is
/// computed against the wrong curve. CS01 must therefore equal the
/// settlement-anchored finite difference
/// `price_from_z_spread(z + 1bp) - price_from_z_spread(z)`.
#[test]
fn test_cs01_zspread_fallback_uses_settlement_anchored_basis() {
    use finstack_core::dates::{DayCount, Tenor};
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_valuations::instruments::fixed_income::bond::pricing::quote_conversions::price_from_z_spread;
    use finstack_valuations::instruments::fixed_income::bond::CashflowSpec;
    use finstack_valuations::instruments::PricingOverrides;

    let as_of = date!(2025 - 01 - 06);
    let notional = Money::new(1_000_000.0, Currency::USD);

    // `Bond::fixed` carries a 2-business-day settlement, so quote_date != as_of
    // and != the discount-curve base date.
    let mut bond = Bond::fixed(
        "CS01-SETTLE",
        notional,
        0.05,
        date!(2023 - 01 - 06),
        date!(2030 - 01 - 06),
        "USD-OIS",
    )
    .unwrap();
    bond.cashflow_spec = CashflowSpec::fixed_rate(0.05.into(), Tenor::annual(), DayCount::Act365F)
        .expect("finite test coupon");
    // No credit curve => CS01 takes the z-spread fallback.
    bond.credit_curve_id = None;
    // Quote off-par so the Z-spread (and hence CS01) is non-trivial.
    bond.pricing_overrides = PricingOverrides::default().with_quoted_clean_price(96.5);

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (7.0, 0.80)])
        .build()
        .unwrap();
    let market = MarketContext::new().insert(disc);

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ZSpread, MetricId::Cs01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let z = *result.measures.get("z_spread").expect("z_spread measure");
    let cs01 = *result.measures.get("cs01").expect("cs01 measure");

    // Expected CS01: settlement-anchored finite difference, identical basis to
    // the Z-spread calibration.
    let one_bp = 1e-4;
    let base = price_from_z_spread(&bond, &market, as_of, z).expect("base reprice");
    let bumped = price_from_z_spread(&bond, &market, as_of, z + one_bp).expect("bumped reprice");
    let expected_cs01 = bumped - base;

    let err = (cs01 - expected_cs01).abs() / notional.amount();
    assert!(
        err < 1e-7,
        "CS01 z-spread fallback must be on the settlement-anchored basis: \
         cs01={cs01:.6}, expected={expected_cs01:.6}, relative_error={err:.3e}"
    );
    // Sanity: a long bond's CS01 is negative.
    assert!(cs01 < 0.0, "long-bond CS01 should be negative, got {cs01}");
}
