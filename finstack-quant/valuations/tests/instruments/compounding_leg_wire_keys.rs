//! Compounding and swap-leg wire vocabulary: legs are `<role>_leg`
//! (`fixed_leg`, `float_leg`, `premium_leg`, `protection_leg`,
//! `financing_leg`), every floating period rate uses one `compounding:
//! FloatingLegCompounding`, cross-currency legs nest a canonical `FloatLegSpec`
//! under `leg`, and quote bases use core `Compounding`. Each retired spelling
//! is rejected by `deny_unknown_fields` or by the closed enum.

use finstack_quant_cashflows::builder::FloatingRateSpec;
use finstack_quant_valuations::instruments::rates::xccy_swap::XccySwap;
use finstack_quant_valuations::instruments::{
    CdsIndex, CreditDefaultSwap, EquityTotalReturnSwap, FiIndexTotalReturnSwap, InterestRateFuture,
    InterestRateSwap, ModelConfig,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

/// Serialize `value`, rename `from` to `to` in the object at `pointer`, and
/// require a deserialization error that names `to`.
fn assert_renamed_key_rejected<T: Serialize + DeserializeOwned>(
    value: &T,
    pointer: &str,
    from: &str,
    to: &str,
) {
    let mut json = serde_json::to_value(value).expect("serialize");
    let object = json
        .pointer_mut(pointer)
        .and_then(Value::as_object_mut)
        .unwrap_or_else(|| panic!("{pointer} must be an object"));
    let moved = object
        .remove(from)
        .unwrap_or_else(|| panic!("{from} present"));
    object.insert(to.to_string(), moved);
    let err = serde_json::from_value::<T>(json)
        .err()
        .unwrap_or_else(|| panic!("retired key {to} must be rejected"));
    assert!(err.to_string().contains(to), "{to}: {err}");
}

/// Serialize `value`, set `key` in the object at `pointer` to `retired`, and
/// require a deserialization error.
fn assert_value_rejected<T: Serialize + DeserializeOwned>(
    value: &T,
    pointer: &str,
    key: &str,
    retired: Value,
) {
    let mut json = serde_json::to_value(value).expect("serialize");
    json.pointer_mut(pointer)
        .and_then(Value::as_object_mut)
        .unwrap_or_else(|| panic!("{pointer} must be an object"))
        .insert(key.to_string(), retired.clone());
    assert!(
        serde_json::from_value::<T>(json).is_err(),
        "{key} = {retired} must be rejected"
    );
}

#[test]
fn irs_bare_leg_keys_are_rejected() {
    let irs = InterestRateSwap::example().expect("example IRS");
    // schema-rejection-test: `fixed` / `float` are now `fixed_leg` / `float_leg`.
    assert_renamed_key_rejected(&irs, "", "fixed_leg", "fixed");
    assert_renamed_key_rejected(&irs, "", "float_leg", "float");
}

#[test]
fn irs_fixed_leg_compounding_simple_is_rejected() {
    let irs = InterestRateSwap::example().expect("example IRS");
    let mut json = serde_json::to_value(&irs).expect("serialize");
    // schema-rejection-test: the inert `compounding_simple` flag is removed.
    json["fixed_leg"]["compounding_simple"] = Value::Bool(true);
    let err = serde_json::from_value::<InterestRateSwap>(json).expect_err("retired key");
    assert!(err.to_string().contains("compounding_simple"), "{err}");
}

#[test]
fn cds_family_bare_leg_keys_are_rejected() {
    let cds = CreditDefaultSwap::example().expect("example");
    // schema-rejection-test: `premium` / `protection` are now `premium_leg` / `protection_leg`.
    assert_renamed_key_rejected(&cds, "", "premium_leg", "premium");
    assert_renamed_key_rejected(&cds, "", "protection_leg", "protection");
    let index = CdsIndex::example().expect("example");
    assert_renamed_key_rejected(&index, "", "premium_leg", "premium");
    assert_renamed_key_rejected(&index, "", "protection_leg", "protection");
}

#[test]
fn trs_bare_financing_key_and_retired_compounding_are_rejected() {
    let equity = EquityTotalReturnSwap::example().expect("equity TRS");
    // schema-rejection-test: `financing` is now `financing_leg`.
    assert_renamed_key_rejected(&equity, "", "financing_leg", "financing");
    let fi = FiIndexTotalReturnSwap::example().expect("FI TRS");
    assert_renamed_key_rejected(&fi, "", "financing_leg", "financing");
    // schema-rejection-test: FinancingRateCompounding's `term_rate` / `overnight_compounded`.
    for retired in ["term_rate", "overnight_compounded"] {
        assert_value_rejected(
            &equity,
            "/financing_leg",
            "compounding",
            Value::from(retired),
        );
    }
}

#[test]
fn floating_rate_spec_overnight_compounding_is_rejected() {
    let spec = FloatingRateSpec::sofr(rust_decimal::Decimal::ZERO);
    let mut json = serde_json::to_value(&spec).expect("serialize");
    let moved = json
        .as_object_mut()
        .and_then(|o| o.remove("compounding"))
        .expect("compounding present");
    // schema-rejection-test: `overnight_compounding` is now `compounding`.
    json["overnight_compounding"] = moved;
    let err = serde_json::from_value::<FloatingRateSpec>(json).expect_err("retired key");
    assert!(err.to_string().contains("overnight_compounding"), "{err}");
    // The OvernightCompoundingMethod unit spellings are gone too.
    for retired in ["compounded_in_arrears", "compounded_with_lookback"] {
        let mut json = serde_json::to_value(&spec).expect("serialize");
        json["compounding"] = Value::from(retired);
        assert!(
            serde_json::from_value::<FloatingRateSpec>(json).is_err(),
            "{retired}"
        );
    }
}

#[test]
fn ir_future_rate_averaging_is_rejected() {
    let future = InterestRateFuture::example().expect("example future");
    // schema-rejection-test: `rate_averaging` is now `compounding`.
    assert_renamed_key_rejected(&future, "", "compounding", "rate_averaging");
    for retired in ["term", "arithmetic_average", "compounded_overnight"] {
        assert_value_rejected(&future, "", "compounding", Value::from(retired));
    }
}

#[test]
fn xccy_flat_leg_keys_are_rejected() {
    let swap = XccySwap::example().expect("example");
    // schema-rejection-test: the redundant leg `currency` is removed.
    assert_value_rejected(&swap, "/leg1", "currency", Value::from("USD"));
    // schema-rejection-test: flat float-leg fields now live under `leg`.
    assert_value_rejected(
        &swap,
        "/leg1",
        "forward_curve_id",
        Value::from("USD-SOFR-3M"),
    );
    assert_value_rejected(&swap, "/leg1", "spread_bp", Value::from("0"));
}

#[test]
fn oas_quote_compounding_semi_annual_label_is_rejected() {
    let json = serde_json::json!({ "oas_quote_compounding": "semi_annual" });
    // schema-rejection-test: OasQuoteCompounding's `semi_annual` is core `{"periodic": 2}`.
    assert!(serde_json::from_value::<ModelConfig>(json).is_err());
    let canonical = serde_json::json!({ "oas_quote_compounding": { "periodic": 2 } });
    let config: ModelConfig = serde_json::from_value(canonical).expect("core periodic basis");
    config
        .validate()
        .expect("semiannual OAS basis is supported");
    let annual = serde_json::json!({ "oas_quote_compounding": "annual" });
    let config: ModelConfig = serde_json::from_value(annual).expect("core annual basis");
    let err = config
        .validate()
        .expect_err("annual OAS basis is not supported");
    assert!(
        err.to_string()
            .contains("instrument_pricing_overrides.model_config.oas_quote_compounding"),
        "{err}"
    );
}
