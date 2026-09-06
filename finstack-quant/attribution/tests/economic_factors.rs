//! Economic factor isolation and coupon-window carry regressions.
use finstack_quant_attribution::{
    attribute_pnl, attribute_pnl_metrics_based, default_waterfall_order, AttributionFactor,
    AttributionMethod, AttributionRequest,
};
use finstack_quant_core::{
    config::FinstackConfig,
    currency::Currency,
    dates::{DayCount, StubKind},
    market_data::{
        context::MarketContext,
        scalars::{InflationIndex, InflationInterpolation, MarketScalar},
        term_structures::DiscountCurve,
    },
    money::Money,
    types::Rate,
};
use finstack_quant_valuations::{
    instruments::{
        fixed_income::{convertible::ConvertibleBond, inflation_linked_bond::InflationLinkedBond},
        Bond, Instrument, PricingOptions,
    },
    metrics::MetricId,
};
use std::sync::Arc;
use time::macros::date;

fn discount(as_of: time::Date) -> DiscountCurve {
    DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([0.0_f64, 0.5, 1.0, 2.0, 5.0, 10.0].map(|t| (t, (-0.04 * t).exp())))
        .build()
        .unwrap()
}

fn cpi_index(latest: f64) -> InflationIndex {
    let mut observations = Vec::new();
    for year in 2023..=2025 {
        for month in 1..=12 {
            let date =
                time::Date::from_calendar_date(year, time::Month::try_from(month).unwrap(), 1)
                    .unwrap();
            if date > date!(2025 - 05 - 01) {
                continue;
            }
            let value = if date >= date!(2025 - 04 - 01) {
                latest
            } else {
                100.0
            };
            observations.push((date, value));
        }
    }
    InflationIndex::new("US-CPI", observations, Currency::USD)
        .unwrap()
        .with_interpolation(InflationInterpolation::Linear)
}

#[test]
fn published_cpi_move_must_be_inflation_pnl() {
    let as_of = date!(2025 - 07 - 14);
    let market = |cpi| {
        MarketContext::new()
            .insert(discount(as_of))
            .insert_inflation_index("US-CPI", cpi_index(cpi))
    };
    let mut bond = InflationLinkedBond::example();
    bond.maturity = date!(2025 - 07 - 15);
    let instrument: Arc<dyn Instrument> = Arc::new(bond);
    let m0 = market(100.0);
    let m1 = market(101.0);
    let config = FinstackConfig::default();
    for (method, full_cross) in repricing_methods() {
        let request = AttributionRequest {
            full_cross_attribution: full_cross,
            ..AttributionRequest::new(&instrument, &m0, &m1, as_of, as_of, &config)
        };
        let a = attribute_pnl(&method, &request).unwrap();
        assert!(a.total_pnl.amount() > 10_000.0);
        assert!(
            (a.inflation_curves_pnl.amount() - a.total_pnl.amount()).abs() < 0.01,
            "{method:?}: pure CPI P&L must be inflation P&L"
        );
        assert!(a.market_scalars_pnl.amount().abs() < 0.01);
        assert!(a.residual.amount().abs() < 0.01);
        assert!(!a.result_invalid);
    }
}

#[test]
fn carry_horizon_scaling_must_not_scale_ex_coupon_price_drop() {
    let t0 = date!(2025 - 07 - 14);
    let t1 = date!(2025 - 07 - 18);
    let mut bond = Bond::fixed(
        "COUPON",
        Money::from((1_000_000_i64, Currency::USD)),
        Rate::from_decimal(0.05).unwrap(),
        date!(2024 - 01 - 15),
        date!(2030 - 01 - 15),
        StubKind::ShortFront,
        "USD-OIS",
    )
    .unwrap();
    bond.metric_pricing_overrides.theta_period = Some("2D".into());
    let instrument: Arc<dyn Instrument> = Arc::new(bond.clone());
    let market = MarketContext::new().insert(discount(t0));
    let metrics = [
        MetricId::CarryTotal,
        MetricId::CouponIncome,
        MetricId::PullToPar,
        MetricId::RollDown,
        MetricId::FundingCost,
        MetricId::ThetaPeriodDays,
    ];
    let v0 = instrument
        .price_with_metrics(&market, t0, &metrics, PricingOptions::default())
        .unwrap();
    let v1 = instrument
        .price_with_metrics(&market, t1, &[], PricingOptions::default())
        .unwrap();
    assert!(v0.measures[MetricId::CouponIncome.as_str()] > 24_000.0);
    assert!(v0.measures[MetricId::PullToPar.as_str()] < -24_000.0);
    let error =
        attribute_pnl_metrics_based(&instrument, &market, &market, &v0, &v1, t0, t1).unwrap_err();
    assert!(error.to_string().contains("matching theta_period_days"));

    bond.metric_pricing_overrides.theta_period = Some("4D".into());
    let instrument: Arc<dyn Instrument> = Arc::new(bond);
    let v0 = instrument
        .price_with_metrics(&market, t0, &metrics, PricingOptions::default())
        .unwrap();
    let a = attribute_pnl_metrics_based(&instrument, &market, &market, &v0, &v1, t0, t1).unwrap();
    assert!(a.total_pnl.amount() > 400.0);
    assert!((a.carry.amount() - a.total_pnl.amount()).abs() < 0.01);
    assert!(a.residual.amount().abs() < 0.01);
    assert!(!a.result_invalid);
}

#[test]
fn scalar_convertible_vol_must_be_volatility_pnl() {
    let as_of = date!(2025 - 07 - 14);
    let mut bond = ConvertibleBond::example().unwrap();
    bond.notional = Money::from((1000_i64, Currency::USD));
    bond.conversion.ratio = Some(10.0);
    bond.discount_curve_id = "USD-OIS".into();
    bond.credit_curve_id = None;
    let instrument: Arc<dyn Instrument> = Arc::new(bond);
    let market = |spot, vol| {
        MarketContext::new()
            .insert(discount(as_of))
            .insert_price("TECH", MarketScalar::Unitless(spot))
            .insert_price("TECH-VOL", MarketScalar::Unitless(vol))
            .insert_price("TECH-DIVYIELD", MarketScalar::Unitless(0.0))
    };
    let m0 = market(80.0, 0.25);
    let m1 = market(80.0, 0.26);
    let config = FinstackConfig::default();
    for (method, full_cross) in repricing_methods() {
        let request = AttributionRequest {
            full_cross_attribution: full_cross,
            ..AttributionRequest::new(&instrument, &m0, &m1, as_of, as_of, &config)
        };
        let a = attribute_pnl(&method, &request).unwrap();
        assert!(a.total_pnl.amount() > 1.0);
        assert!(
            (a.vol_pnl.amount() - a.total_pnl.amount()).abs() < 0.01,
            "{method:?}: pure scalar-vol P&L must be volatility P&L"
        );
        assert!(a.market_scalars_pnl.amount().abs() < 0.01);
        assert!(a.residual.amount().abs() < 0.01);
        assert!(!a.result_invalid);
    }

    let spot_only = market(81.0, 0.25);
    let both = market(81.0, 0.26);
    let v00 = instrument.value(&m0, as_of).unwrap().amount();
    let v01 = instrument.value(&m1, as_of).unwrap().amount();
    let v10 = instrument.value(&spot_only, as_of).unwrap().amount();
    let v11 = instrument.value(&both, as_of).unwrap().amount();
    for (method, full_cross) in repricing_methods() {
        let request = AttributionRequest {
            full_cross_attribution: full_cross,
            ..AttributionRequest::new(&instrument, &m0, &both, as_of, as_of, &config)
        };
        let a = attribute_pnl(&method, &request).unwrap();
        let (expected_vol, expected_spot) = match &method {
            AttributionMethod::Parallel => (v11 - v10, v11 - v01),
            AttributionMethod::Waterfall(order) => {
                if order
                    .iter()
                    .position(|f| *f == AttributionFactor::Volatility)
                    .unwrap()
                    < order
                        .iter()
                        .position(|f| *f == AttributionFactor::MarketScalars)
                        .unwrap()
                {
                    (v01 - v00, v11 - v01)
                } else {
                    (v11 - v10, v10 - v00)
                }
            }
            AttributionMethod::MetricsBased | AttributionMethod::Taylor(_) => {
                unreachable!("only full-revaluation methods are tested")
            }
        };
        assert!((a.vol_pnl.amount() - expected_vol).abs() < 0.01);
        assert!((a.market_scalars_pnl.amount() - expected_spot).abs() < 0.01);
        assert!(a.residual.amount().abs() < 0.01);
        assert!(!a.result_invalid);
    }
}

fn repricing_methods() -> [(AttributionMethod, bool); 4] {
    let mut scalars_first = default_waterfall_order();
    let scalars = scalars_first.pop().unwrap();
    scalars_first.insert(1, scalars);
    [
        (AttributionMethod::Parallel, false),
        (AttributionMethod::Parallel, true),
        (
            AttributionMethod::Waterfall(default_waterfall_order()),
            false,
        ),
        (AttributionMethod::Waterfall(scalars_first), false),
    ]
}
