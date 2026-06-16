//! Integration tests for cross-factor gamma metrics.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::{CrossFactorPair, MetricId};
use time::macros::date;
use time::Date;

fn build_discount_curve(id: &str, as_of: Date, rate: f64) -> DiscountCurve {
    DiscountCurve::builder(id)
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0f64, 1.0f64),
            (1.0f64, (-rate).exp()),
            (5.0f64, (-rate * 5.0).exp()),
            (10.0f64, (-rate * 10.0).exp()),
        ])
        .build()
        .expect("discount curve should build")
}

fn build_hazard_curve(id: &str, as_of: Date, hazard_rate: f64) -> HazardCurve {
    HazardCurve::builder(id)
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .recovery_rate(0.4)
        .knots([(0.0f64, hazard_rate), (5.0f64, hazard_rate)])
        .build()
        .expect("hazard curve should build")
}

fn build_test_bond(as_of: Date) -> Bond {
    let mut bond = Bond::fixed(
        "CROSS-GAMMA-BOND",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        date!(2030 - 01 - 01),
        "USD-OIS",
    )
    .expect("bond construction should succeed");
    bond.credit_curve_id = Some(CurveId::new("USD-CREDIT"));
    bond
}

fn build_market(as_of: Date, rate: f64, hazard_rate: f64) -> MarketContext {
    MarketContext::new()
        .insert(build_discount_curve("USD-OIS", as_of, rate))
        .insert(build_hazard_curve("USD-CREDIT", as_of, hazard_rate))
}

fn repriced_bond_value(
    bond: &Bond,
    market: &MarketContext,
    as_of: Date,
    rate_direction: f64,
    credit_direction: f64,
) -> f64 {
    let bumped = market
        .bump([
            MarketBump::Curve {
                id: CurveId::new("USD-OIS"),
                spec: BumpSpec::parallel_bp(rate_direction),
            },
            MarketBump::Curve {
                id: CurveId::new("USD-CREDIT"),
                spec: BumpSpec::parallel_bp(credit_direction),
            },
        ])
        .expect("market bump should succeed");
    bond.value(&bumped, as_of)
        .expect("bond repricing should succeed")
        .amount()
}

#[test]
fn cross_gamma_rates_credit_matches_manual_four_corner_repricing() {
    let as_of = date!(2025 - 01 - 01);
    let bond = build_test_bond(as_of);
    let market = build_market(as_of, 0.04, 0.02);

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::CrossGammaRatesCredit],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("cross gamma metric should compute");
    let metric_value = result.measures[&MetricId::CrossGammaRatesCredit];

    let manual_value = (repriced_bond_value(&bond, &market, as_of, 1.0, 1.0)
        - repriced_bond_value(&bond, &market, as_of, 1.0, -1.0)
        - repriced_bond_value(&bond, &market, as_of, -1.0, 1.0)
        + repriced_bond_value(&bond, &market, as_of, -1.0, -1.0))
        / 4.0;

    assert!(
        (metric_value - manual_value).abs() < 1e-8,
        "registry metric {metric_value} should match manual four-corner value {manual_value}",
    );
}

#[test]
fn cross_factor_pair_covers_all_metric_ids() {
    for pair in CrossFactorPair::ALL {
        let id = pair.metric_id();
        let parsed = MetricId::parse_strict(id.as_str()).expect("metric id should parse");
        assert_eq!(parsed, id);
    }
}
