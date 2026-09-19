//! Requested-metric contract: nothing a caller asked for may be silently absent.
//!
//! Every metric named in the `metrics` argument of the JSON pricing entry
//! points must either appear in `ValuationResult::measures` or cause an error.
//! These tests pin both halves of that contract for the instrument/metric pairs
//! that used to return nothing, and guard against an over-broad rejection by
//! asserting the supported metrics on the same instruments still compute.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
use finstack_quant_valuations::instruments::PricingOptions;
use finstack_quant_valuations::pricer::{
    metric_value, parse_boxed_instrument_from_json, price_instrument,
};
use finstack_quant_valuations::results::ValuationResult;
use std::sync::Arc;

const TERM_LOAN: &str = include_str!("instruments/json_examples/term_loan.json");
const FX_FORWARD: &str = include_str!("instruments/json_examples/fx_forward.json");
const STRUCTURED_CREDIT: &str = include_str!("instruments/json_examples/structured_credit.json");

/// Valuation date for the canonical `term_loan` fixture (matures 2029-01-01).
const TERM_LOAN_AS_OF: &str = "2025-06-16";
/// The canonical `fx_forward` fixture matures 2025-06-15, so the requested
/// 2025-06-16 valuation date is rolled back to a live date for these tests.
const FX_FORWARD_AS_OF: &str = "2025-01-16";
/// The canonical `structured_credit` fixture closes 2024-01-01 and matures
/// 2034-01-01, so any date inside that window exercises the deal's waterfall.
const STRUCTURED_CREDIT_AS_OF: &str = "2025-06-16";
/// `expected_loss` is published by the deal Monte Carlo pass, so the
/// structured-credit cases must run under that model rather than `default`.
const STRUCTURED_CREDIT_MODEL: &str = "structured_credit_stochastic";

fn market(as_of: Date) -> MarketContext {
    let usd = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots(vec![
            (0.0, 1.0),
            (0.5, 0.9753),
            (1.0, 0.9512),
            (5.0, 0.78),
            (10.0, 0.61),
        ])
        .build()
        .expect("USD discount curve fixture");
    let eur = DiscountCurve::builder("EUR-OIS")
        .base_date(as_of)
        .knots(vec![
            (0.0, 1.0),
            (0.5, 0.9851),
            (1.0, 0.9704),
            (5.0, 0.86),
            (10.0, 0.74),
        ])
        .build()
        .expect("EUR discount curve fixture");
    let fx = Arc::new(SimpleFxProvider::new());
    fx.set_quote(Currency::EUR, Currency::USD, 1.10)
        .expect("EURUSD quote fixture");
    MarketContext::new()
        .insert(usd)
        .insert(eur)
        .insert_fx(FxMatrix::new(fx))
}

fn price(
    instrument_json: &str,
    as_of: &str,
    metrics: &[&str],
) -> finstack_quant_core::Result<ValuationResult> {
    price_with_model(instrument_json, as_of, "default", metrics)
}

fn price_with_model(
    instrument_json: &str,
    as_of: &str,
    model: &str,
    metrics: &[&str],
) -> finstack_quant_core::Result<ValuationResult> {
    let parsed = parse_boxed_instrument_from_json(instrument_json, None)
        .expect("canonical fixture parses and validates");
    let owned: Vec<String> = metrics.iter().map(|m| (*m).to_string()).collect();
    price_instrument(
        &parsed,
        &market(
            finstack_quant_core::dates::parse_iso_date(as_of).expect("valid ISO valuation date"),
        ),
        as_of,
        model,
        &owned,
        None,
        PricingOptions::default(),
    )
}

fn single(instrument_json: &str, as_of: &str, metric: &str) -> finstack_quant_core::Result<f64> {
    let parsed = parse_boxed_instrument_from_json(instrument_json, None)
        .expect("canonical fixture parses and validates");
    metric_value(
        &parsed,
        &market(
            finstack_quant_core::dates::parse_iso_date(as_of).expect("valid ISO valuation date"),
        ),
        as_of,
        "default",
        metric,
        PricingOptions::default(),
    )
}

/// Assert the batch path rejects `metric` and the message names both the
/// metric and the instrument type.
fn assert_rejected(instrument_json: &str, as_of: &str, instrument_type: &str, metric: &str) {
    let error = price(instrument_json, as_of, &[metric])
        .expect_err("an unsupported requested metric must be rejected, not silently dropped");
    let message = error.to_string();
    assert!(
        message.contains(metric) && message.contains(instrument_type),
        "rejection for `{metric}` on `{instrument_type}` must name both; got: {message}"
    );
}

#[test]
fn term_loan_rejects_unsupported_requested_metrics() {
    for metric in ["duration_mod", "accrued", "clean_price"] {
        assert_rejected(TERM_LOAN, TERM_LOAN_AS_OF, "term_loan", metric);
    }
}

#[test]
fn fx_forward_rejects_unsupported_requested_metrics() {
    for metric in ["spot_rate", "base_amount"] {
        assert_rejected(FX_FORWARD, FX_FORWARD_AS_OF, "fx_forward", metric);
    }
}

#[test]
fn term_loan_still_computes_its_supported_metrics() {
    let metrics = ["ytm", "ytw", "dv01"];
    let result = price(TERM_LOAN, TERM_LOAN_AS_OF, &metrics).expect("supported metrics must price");
    for metric in metrics {
        let value = result
            .metric_str(metric)
            .unwrap_or_else(|| panic!("supported metric `{metric}` must be present"));
        assert!(value.is_finite(), "`{metric}` must be finite, got {value}");
    }
}

#[test]
fn fx_forward_still_computes_its_supported_metrics() {
    let metrics = ["dv01", "fx01"];
    let result =
        price(FX_FORWARD, FX_FORWARD_AS_OF, &metrics).expect("supported metrics must price");
    for metric in metrics {
        let value = result
            .metric_str(metric)
            .unwrap_or_else(|| panic!("supported metric `{metric}` must be present"));
        assert!(value.is_finite(), "`{metric}` must be finite, got {value}");
    }
}

#[test]
fn single_metric_path_agrees_with_batch_path() {
    let cases: [(&str, &str, &str, bool); 7] = [
        (TERM_LOAN, TERM_LOAN_AS_OF, "duration_mod", false),
        (TERM_LOAN, TERM_LOAN_AS_OF, "accrued", false),
        (TERM_LOAN, TERM_LOAN_AS_OF, "clean_price", false),
        (FX_FORWARD, FX_FORWARD_AS_OF, "spot_rate", false),
        (FX_FORWARD, FX_FORWARD_AS_OF, "base_amount", false),
        (TERM_LOAN, TERM_LOAN_AS_OF, "ytm", true),
        (FX_FORWARD, FX_FORWARD_AS_OF, "fx01", true),
    ];
    for (json, as_of, metric, expected_ok) in cases {
        let batch = price(json, as_of, &[metric]).map(|result| result.metric_str(metric));
        let scalar = single(json, as_of, metric);
        match (batch, scalar) {
            (Ok(Some(batch_value)), Ok(scalar_value)) => {
                assert!(expected_ok, "`{metric}` was expected to be rejected");
                assert_eq!(
                    batch_value, scalar_value,
                    "batch and single-metric paths disagree on `{metric}`"
                );
            }
            (Ok(None), _) => panic!("`{metric}` must never be silently absent from the batch path"),
            (Err(batch_error), Err(_)) => {
                assert!(
                    !expected_ok,
                    "`{metric}` was expected to compute, got: {batch_error}"
                );
            }
            (batch, scalar) => panic!(
                "batch and single-metric paths disagree on `{metric}`: \
                 batch={batch:?}, scalar={scalar:?}"
            ),
        }
    }
}

/// A standard metric a model publishes as a side output of its own pricing
/// pass is owned by the registry for that instrument type and answered with
/// the model's own number.
///
/// Structured credit's Monte Carlo pass publishes `expected_loss` directly
/// into the result envelope; no calculator derives it. Requesting it must
/// therefore return that published value, not be rejected as inapplicable and
/// not be recomputed into a second, disagreeing number.
#[test]
fn structured_credit_expected_loss_comes_from_the_pricer() {
    let published = price_with_model(
        STRUCTURED_CREDIT,
        STRUCTURED_CREDIT_AS_OF,
        STRUCTURED_CREDIT_MODEL,
        &[],
    )
    .expect("structured credit prices under its stochastic model")
    .metric_str("expected_loss")
    .expect("the stochastic pricer publishes `expected_loss` without being asked");

    let requested = price_with_model(
        STRUCTURED_CREDIT,
        STRUCTURED_CREDIT_AS_OF,
        STRUCTURED_CREDIT_MODEL,
        &["expected_loss"],
    )
    .expect("requesting `expected_loss` on structured credit must not be rejected")
    .metric_str("expected_loss")
    .expect("a requested metric is never silently absent");

    assert_eq!(
        requested, published,
        "requesting `expected_loss` must return the pricer's value verbatim"
    );
}

/// The registry still rejects the same metric on an instrument type whose
/// pricers neither publish nor calculate it.
#[test]
fn term_loan_still_rejects_expected_loss() {
    assert_rejected(TERM_LOAN, TERM_LOAN_AS_OF, "term_loan", "expected_loss");
}
