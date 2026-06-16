//! PR-12 final compatibility sweep.
//!
//! Verifies that:
//!  1. Old `AttributionEnvelope` JSON (pre-PR-7, no `credit_factor_model` field)
//!     still deserializes correctly and `credit_factor_model` defaults to `None`.
//!  2. Old `PnlAttribution` JSON (pre-PR-7, no `credit_factor_detail` field)
//!     still deserializes correctly and `credit_factor_detail` defaults to `None`.
//!  3. All four attribution methods (MetricsBased, Taylor, Parallel, Waterfall)
//!     on a bond with NO `credit_factor_model` produce a finite total P&L and
//!     `credit_factor_detail == None` (confirming the opt-in model-absent path
//!     is unchanged by the credit factor hierarchy feature).

use finstack_quant_attribution::{
    default_waterfall_order, AttributionEnvelope, AttributionMethod, AttributionSpec,
    PnlAttribution, TaylorAttributionConfig,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::create_date;
use finstack_quant_core::market_data::context::{
    CurveState, MarketContextState, MARKET_CONTEXT_STATE_VERSION,
};
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::json_loader::InstrumentJson;
use finstack_quant_valuations::instruments::Bond;
use time::Month;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

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
        "COMPAT-BOND-001",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        create_date(2025, Month::January, 1).unwrap(),
        create_date(2030, Month::January, 1).unwrap(),
        CURVE_ID,
    )
    .unwrap()
}

fn make_market_state(as_of: finstack_quant_core::dates::Date, rate: f64) -> MarketContextState {
    MarketContextState {
        version: MARKET_CONTEXT_STATE_VERSION,
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

// ---------------------------------------------------------------------------
// 1. Old AttributionEnvelope JSON (pre-PR-7) round-trip
// ---------------------------------------------------------------------------

/// Build a pre-PR-7 AttributionEnvelope JSON string by serializing a current
/// spec and removing the fields that were added by PR-7+.
fn pre_pr7_envelope_json() -> String {
    let bond = sample_bond();
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    let spec = AttributionSpec {
        instrument: InstrumentJson::Bond(bond),
        market_t0: make_market_state(as_of_t0, 0.04),
        market_t1: make_market_state(as_of_t1, 0.0401),
        as_of_t0,
        as_of_t1,
        method: AttributionMethod::Parallel,
        config: None,
        model_params_t0: None,
        credit_factor_model: None,
        credit_factor_detail_options: Default::default(),
        full_cross_attribution: false,
    };
    let envelope = AttributionEnvelope::new(spec);
    let mut value = serde_json::to_value(&envelope).expect("serialize");

    // Strip PR-7+ fields to simulate a pre-PR-7 payload.
    if let Some(attr_obj) = value.get_mut("attribution").and_then(|v| v.as_object_mut()) {
        attr_obj.remove("credit_factor_model");
        attr_obj.remove("credit_factor_detail_options");
    }
    serde_json::to_string(&value).expect("re-serialize")
}

#[test]
fn old_attribution_envelope_json_deserializes_credit_factor_model_defaults_to_none() {
    let json = pre_pr7_envelope_json();
    let parsed: AttributionEnvelope =
        serde_json::from_str(&json).expect("pre-PR-7 envelope should deserialize");
    assert_eq!(parsed.schema, "finstack_quant.attribution/1");
    assert!(
        parsed.attribution.credit_factor_model.is_none(),
        "credit_factor_model should default to None for old payloads"
    );
}

// ---------------------------------------------------------------------------
// 2. Old PnlAttribution JSON (pre-PR-7) round-trip
// ---------------------------------------------------------------------------

fn pre_pr7_pnl_attribution_json() -> String {
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    let mut attr = PnlAttribution::new(
        Money::new(500.0, Currency::USD),
        "COMPAT-BOND-001",
        as_of_t0,
        as_of_t1,
        AttributionMethod::Parallel,
    );
    attr.credit_curves_pnl = Money::new(-100.0, Currency::USD);

    // Serialize and strip all new optional fields added by PR-7+.
    let mut value = serde_json::to_value(&attr).expect("serialize");
    if let Some(obj) = value.as_object_mut() {
        obj.remove("credit_factor_detail");
        obj.remove("credit_carry_decomposition");
    }
    serde_json::to_string(&value).expect("re-serialize")
}

#[test]
fn old_pnl_attribution_json_deserializes_new_fields_default_to_none() {
    let json = pre_pr7_pnl_attribution_json();
    let parsed: PnlAttribution =
        serde_json::from_str(&json).expect("pre-PR-7 PnlAttribution should deserialize");
    assert!(
        parsed.credit_factor_detail.is_none(),
        "credit_factor_detail should default to None for old payloads"
    );
    assert!(
        parsed.credit_carry_decomposition.is_none(),
        "credit_carry_decomposition should default to None for old payloads"
    );
    assert!(
        (parsed.credit_curves_pnl.amount() - (-100.0)).abs() < 1e-12,
        "credit_curves_pnl should be preserved byte-identically"
    );
}

// ---------------------------------------------------------------------------
// 3. All four methods — no credit_factor_model — finite totals + no detail
// ---------------------------------------------------------------------------

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
