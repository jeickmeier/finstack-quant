use crate::inflation_linked_bond::InflationLinkedBond;
use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, InflationCurve};
use finstack_quant_core::math::units::ONE_BASIS_POINT;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::instruments::PricingOptions;
use finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides;
use finstack_quant_valuations_core::metrics::MetricId;
use time::macros::date;

fn market(as_of: finstack_quant_core::dates::Date) -> MarketContext {
    let discount = DiscountCurve::builder("US-CPI-DISC")
        .base_date(as_of)
        .knots([(0.0, 1.0), (10.0, 0.7)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve");
    let inflation = InflationCurve::builder("US-CPI")
        .base_date(as_of)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Flat)
        .knots([(0.0, 100.0), (5.0, 112.0), (10.0, 125.0)])
        .build()
        .expect("inflation curve");
    MarketContext::new()
        .try_insert(discount)
        .expect("insert market curve")
        .try_insert(inflation)
        .expect("insert market curve")
        .insert_inflation_index(
            finstack_quant_valuations_test_support::inflation::simple_index(
                "US-CPI",
                as_of,
                100.0,
                finstack_quant_core::currency::Currency::USD,
            ),
        )
}

fn sample_bond() -> InflationLinkedBond {
    let mut bond = InflationLinkedBond::example().expect("example");
    // `example()` uses ActActIsma which needs an explicit period frequency;
    // use a self-contained day count for the metric test.
    bond.deflation_protection = crate::inflation_linked_bond::DeflationProtection::None;
    bond.day_count = finstack_quant_core::dates::DayCount::Thirty360;
    bond.discount_curve_id = CurveId::new("US-CPI-DISC");
    bond.inflation_index_id = CurveId::new("US-CPI");
    bond
}

/// the reported `InflationConvexity` is the **raw dollar
/// second derivative** `d²PV/dπ²` ($ per decimal²) — the same convention
/// as the inflation swap producer and the attribution consumer. The
/// former `(1/P)`-normalized figure understated ILB inflation convexity
/// P&L by a factor of ~P₀.
#[test]
fn inflation_convexity_is_raw_dollar_second_derivative() {
    let as_of = date!(2024 - 01 - 15);
    let bond = sample_bond();
    let market = market(as_of);

    let base_pv = bond.value(&market, as_of).expect("base pv").amount();

    let bump = ONE_BASIS_POINT;
    let curves_up = market
        .bump([MarketBump::InflationCurve {
            id: bond.inflation_index_id.clone(),
            spec: BumpSpec::inflation_shift_pct(bump * 100.0),
        }])
        .expect("bump up");
    let curves_down = market
        .bump([MarketBump::InflationCurve {
            id: bond.inflation_index_id.clone(),
            spec: BumpSpec::inflation_shift_pct(-bump * 100.0),
        }])
        .expect("bump down");
    let pv_up = bond.value(&curves_up, as_of).expect("pv up").amount();
    let pv_down = bond.value(&curves_down, as_of).expect("pv down").amount();

    // Raw dollar second derivative ($ per decimal²) — the MetricId unit.
    let expected = (pv_up + pv_down - 2.0 * base_pv) / (bump * bump);
    // The (1/P)-normalized figure would be ~base_pv× smaller and break
    // the consumer formula `½ × C × Δi²` (no P₀ factor).
    let wrong_dimensionless = (pv_up + pv_down - 2.0 * base_pv) / (bump * bump) / base_pv;

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::InflationConvexity],
            PricingOptions::default(),
        )
        .expect("price with metrics");
    let reported = *result
        .measures
        .get("inflation_convexity")
        .expect("inflation_convexity measure");

    assert!(
        (reported - expected).abs() < 1e-6 * expected.abs().max(1.0),
        "reported {reported} should match the raw dollar figure {expected}"
    );
    assert!(
        (reported - wrong_dimensionless).abs() > 1.0,
        "reported {reported} must NOT equal the (1/P)-normalized figure {wrong_dimensionless}"
    );
}

/// W-28(a): a near-zero base PV is handled via a tolerance, not an exact
/// `== 0.0` compare. A bond whose PV is a tiny non-zero value still returns 0.0.
#[test]
fn inflation_convexity_near_zero_base_pv_uses_tolerance() {
    let as_of = date!(2024 - 01 - 15);
    let mut bond = sample_bond();
    // Force a tiny non-zero base PV via a tiny notional.
    bond.notional = finstack_quant_core::money::Money::new(1e-12, bond.notional.currency())
        .expect("valid money fixture");
    bond.instrument_pricing_overrides = InstrumentPricingOverrides::default();
    let market = market(as_of);

    let base_pv = bond.value(&market, as_of).expect("base pv").amount();
    assert!(
        base_pv.abs() < finstack_quant_core::math::ZERO_TOLERANCE && base_pv.abs() > 0.0,
        "test precondition: base PV {base_pv} is tiny but non-zero"
    );

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::InflationConvexity],
            PricingOptions::default(),
        )
        .expect("price with metrics");
    let reported = *result
        .measures
        .get("inflation_convexity")
        .expect("inflation_convexity measure");

    assert_eq!(
        reported, 0.0,
        "near-zero base PV must be caught by the tolerance guard"
    );
}
