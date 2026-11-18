//! Comprehensive unit tests for common instruments module.
//!
//! Organized by:
//! - parameters: Parameter types and conventions
//! - pricer: Registry and instrument-type contract tests
//!
//! Curve builders, tolerances and parity helpers live in
//! `finstack_quant_valuations_test_support`.

pub mod parameters;
pub mod pricer;
pub mod test_callable_credit_baseline;
pub mod test_callable_floating_resets;
pub mod test_discountable;

/// Build a USD deposit curve and its replay record through the public calibration engine.
pub(super) fn calibrated_discount(
    id: &str,
    base: finstack_quant_core::dates::Date,
    rate: f64,
) -> finstack_quant_core::market_data::term_structures::DiscountCurve {
    use finstack_quant_core::market_data::context::MarketContext;
    use serde_json::json;
    let maturities = [1, 5, 10];
    let deposits: Vec<_> = maturities.iter().map(|&years| json!({
        "kind":"rate_quote", "type":"deposit", "id":format!("{id}-{years}Y"), "index":"USD-SOFR-OIS",
        "pillar":{"tenor":{"count":years, "unit":"years"}}, "rate":rate,
    })).collect();
    let ids: Vec<_> = deposits.iter().map(|quote| &quote["id"]).collect();
    let envelope = json!({
        "schema":"finstack_quant.calibration/1", "plan":{"id":"cds-test-curve",
          "quote_sets":{"deposits":ids}, "steps":[{"id":id, "quote_set":"deposits", "kind":"discount",
          "curve_id":id, "currency":"USD", "base_date":base.to_string(), "method":"bootstrap",
          "interpolation":"log_linear", "extrapolation":"extend", "conventions":{"curve_day_count":"act_365f"}}]},
        "market_data":deposits
    });
    let result =
        finstack_quant_calibration::api::engine::calibrate_from_json(&envelope.to_string())
            .unwrap();
    let market = MarketContext::try_from(result.result.final_market).unwrap();
    market.get_discount(id).unwrap().as_ref().clone()
}
