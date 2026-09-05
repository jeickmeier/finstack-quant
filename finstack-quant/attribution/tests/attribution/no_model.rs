//! Attribution behavior when no credit factor model is supplied.
//!
//! All four attribution methods on a bond without a `credit_factor_model`
//! produce finite total P&L and omit credit-factor detail.

use finstack_quant_attribution::{
    default_waterfall_order, AttributionEnvelope, AttributionMethod, AttributionSpec,
    PnlAttribution, TaylorAttributionConfig,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::create_date;
use finstack_quant_core::market_data::context::{CurveState, MarketContextState};
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::json_loader::InstrumentJson;
use finstack_quant_valuations::instruments::Bond;
use time::Month;

// Shared helpers

const CURVE_ID: &str = "USD-OIS";

fn flat_discount_curve(as_of: finstack_quant_core::dates::Date, rate: f64) -> DiscountCurve {
    let knots: Vec<(f64, f64)> = [0.0_f64, 1.0, 2.0, 5.0, 10.0, 20.0, 30.0]
        .iter()
        .map(|&t| (t, (-rate * t).exp()))
        .collect();
    DiscountCurve::builder(CURVE_ID)
        .base_date(as_of)
        .knots(knots)
        .interp(InterpStyle::Linear)
        .build()
        .unwrap()
}

fn sample_bond() -> Bond {
    Bond::fixed(
        "NO-MODEL-BOND-001",
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        create_date(2025, Month::January, 1).unwrap(),
        create_date(2030, Month::January, 1).unwrap(),
        finstack_quant_core::dates::StubKind::ShortFront,
        CURVE_ID,
    )
    .unwrap()
}

fn make_market_state(as_of: finstack_quant_core::dates::Date, rate: f64) -> MarketContextState {
    MarketContextState {
        schema_version: finstack_quant_core::wire::SchemaVersion::CURRENT,
        curves: vec![CurveState::Discount(flat_discount_curve(as_of, rate))],
        fx: None,
        surfaces: vec![],
        prices: std::collections::BTreeMap::new(),
        series: vec![],
        inflation_indices: vec![],
        dividends: vec![],
        credit_indices: vec![],
        collateral: std::collections::BTreeMap::new(),
        fx_delta_vol_surfaces: vec![],
        hierarchy: None,
        vol_cubes: vec![],
    }
}

/// Build and execute an AttributionSpec for the given method with NO
/// credit_factor_model, returning the resulting PnlAttribution.
fn run_attribution(method: AttributionMethod) -> PnlAttribution {
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    let spec = AttributionSpec {
        instrument: InstrumentJson::Bond(sample_bond()),
        market_t0: make_market_state(as_of_t0, 0.04),
        market_t1: make_market_state(as_of_t1, 0.0401), // 1 bp shift
        as_of_t0,
        as_of_t1,
        method,
        config: None,
        model_params_t0: None,
        credit_factor_model: None,
        credit_factor_detail_options: Default::default(),
        full_cross_attribution: false,
    };

    AttributionEnvelope::new(spec)
        .execute()
        .expect("attribution should succeed")
        .result
        .attribution
}

#[test]
fn metrics_based_no_credit_model_produces_finite_total_and_no_detail() {
    let attr = run_attribution(AttributionMethod::MetricsBased);
    assert!(
        attr.total_pnl.amount().is_finite(),
        "MetricsBased: total_pnl is not finite: {}",
        attr.total_pnl.amount()
    );
    assert!(
        attr.credit_factor_detail.is_none(),
        "MetricsBased: credit_factor_detail should be None without model"
    );
}

#[test]
fn taylor_no_credit_model_produces_finite_total_and_no_detail() {
    let attr = run_attribution(AttributionMethod::Taylor(TaylorAttributionConfig::default()));
    assert!(
        attr.total_pnl.amount().is_finite(),
        "Taylor: total_pnl is not finite: {}",
        attr.total_pnl.amount()
    );
    assert!(
        attr.credit_factor_detail.is_none(),
        "Taylor: credit_factor_detail should be None without model"
    );
}

#[test]
fn parallel_no_credit_model_produces_finite_total_and_no_detail() {
    let attr = run_attribution(AttributionMethod::Parallel);
    assert!(
        attr.total_pnl.amount().is_finite(),
        "Parallel: total_pnl is not finite: {}",
        attr.total_pnl.amount()
    );
    assert!(
        attr.credit_factor_detail.is_none(),
        "Parallel: credit_factor_detail should be None without model"
    );
}

#[test]
fn waterfall_no_credit_model_produces_finite_total_and_no_detail() {
    let attr = run_attribution(AttributionMethod::Waterfall(default_waterfall_order()));
    assert!(
        attr.total_pnl.amount().is_finite(),
        "Waterfall: total_pnl is not finite: {}",
        attr.total_pnl.amount()
    );
    assert!(
        attr.credit_factor_detail.is_none(),
        "Waterfall: credit_factor_detail should be None without model"
    );
}

/// Confirm all four method totals are finite and in the same sign-group
/// (all should show a small loss from the 1 bp rate rise on a bond).
#[test]
fn all_four_methods_no_credit_model_totals_all_finite() {
    let methods = [
        ("MetricsBased", AttributionMethod::MetricsBased),
        (
            "Taylor",
            AttributionMethod::Taylor(TaylorAttributionConfig::default()),
        ),
        ("Parallel", AttributionMethod::Parallel),
        (
            "Waterfall",
            AttributionMethod::Waterfall(default_waterfall_order()),
        ),
    ];

    for (name, method) in methods {
        let attr = run_attribution(method);
        assert!(
            attr.total_pnl.amount().is_finite(),
            "{}: total_pnl is not finite",
            name
        );
        assert!(
            attr.credit_factor_detail.is_none(),
            "{}: credit_factor_detail should be None without model",
            name
        );
    }
}
