//! Tests for the surrounding crate component and its documented behavior.
//!
use finstack_quant_attribution::{
    AttributionConfig, AttributionEnvelope, AttributionMethod, AttributionSpec,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{create_date, Date};
use finstack_quant_core::market_data::context::MarketContextState;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::json_loader::InstrumentJson;
use finstack_quant_valuations::instruments::Bond;
use std::collections::BTreeMap;
use time::Month;

fn empty_market_state() -> MarketContextState {
    MarketContextState {
        schema_version: finstack_quant_core::wire::SchemaVersion::CURRENT,
        curves: vec![],
        fx: None,
        surfaces: vec![],
        prices: BTreeMap::new(),
        series: vec![],
        inflation_indices: vec![],
        dividends: vec![],
        credit_indices: vec![],
        collateral: BTreeMap::new(),
        fx_delta_vol_surfaces: vec![],
        hierarchy: None,
        vol_cubes: vec![],
    }
}

fn test_dates() -> (Date, Date) {
    (
        create_date(2025, Month::January, 1).expect("Valid date"),
        create_date(2025, Month::January, 2).expect("Valid date"),
    )
}

#[test]
fn spec_rejects_unknown_metrics() {
    let bond = Bond::fixed(
        "TEST-BOND",
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        create_date(2024, Month::January, 1).expect("Valid issue date"),
        create_date(2034, Month::January, 1).expect("Valid maturity"),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("Bond construction should succeed");

    let (as_of_t0, as_of_t1) = test_dates();

    let spec = AttributionSpec {
        instrument: InstrumentJson::Bond(bond),
        market_t0: empty_market_state(),
        market_t1: empty_market_state(),
        as_of_t0,
        as_of_t1,
        method: AttributionMethod::MetricsBased,
        model_params_t0: None,
        credit_factor_model: None,
        credit_factor_detail_options: Default::default(),
        config: Some(AttributionConfig {
            tolerance_abs: None,
            tolerance_pct: None,
            metrics: Some(vec!["dv01".to_string(), "unknown_metric".to_string()]),
            strict_validation: None,
            rounding_scale: None,
            rate_bump_bp: None,
            target_currency: None,
            execution_policy: None,
        }),
        full_cross_attribution: false,
    };

    let envelope = AttributionEnvelope::new(spec);
    let result = envelope.execute();
    assert!(
        result.is_err(),
        "Unknown metrics should trigger validation error"
    );

    let err = result.unwrap_err();
    if let finstack_quant_core::Error::Validation(msg) = err {
        assert!(
            msg.contains("unknown_metric"),
            "Error message should include unknown metric name: {msg}"
        );
    } else {
        panic!("Expected validation error, got {:?}", err);
    }
}
