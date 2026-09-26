//! Rates projection and index identifiers share one wire vocabulary: the
//! rates projection curve and the commodity price curve are
//! `forward_curve_id`, the rate-index identity is `index_id: IndexId` (the
//! convention-registry key, which replaced the CMS `swap_convention` enum),
//! the observed index tenor is `index_tenor`, the financing curve is
//! `repo_curve_id` and the CMS reference-swap fixed-leg day count is
//! `swap_fixed_day_count`. Each retired spelling is rejected by
//! `deny_unknown_fields`.

use finstack_quant_valuations::instruments::{
    Bond, CmsOption, CmsSpreadOption, CmsSwap, CommodityFuture, CommoditySwap, InterestRateFuture,
    RangeAccrual, Snowball, Tarn, TermLoan,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

/// Serialize `instrument`, insert `retired` with `value` into the object at
/// `pointer` (JSON pointer, `""` for the root), and require a rejection that
/// names the retired key.
fn assert_rejects_at<T: Serialize + DeserializeOwned>(
    instrument: &T,
    pointer: &str,
    retired: &str,
    value: Value,
) {
    let mut json = serde_json::to_value(instrument).expect("serialize");
    json.pointer_mut(pointer)
        .and_then(Value::as_object_mut)
        .unwrap_or_else(|| panic!("{pointer} must be an object"))
        .insert(retired.to_string(), value);
    let err = serde_json::from_value::<T>(json)
        .err()
        .unwrap_or_else(|| panic!("retired key {retired} must be rejected"));
    assert!(err.to_string().contains(retired), "{retired}: {err}");
}

/// JSON pointer of the first object (depth-first) that holds `key`.
fn pointer_of_object_with(value: &Value, key: &str, at: String) -> Option<String> {
    match value {
        Value::Object(map) => {
            if map.contains_key(key) {
                return Some(at);
            }
            map.iter()
                .find_map(|(k, v)| pointer_of_object_with(v, key, format!("{at}/{k}")))
        }
        Value::Array(items) => items
            .iter()
            .enumerate()
            .find_map(|(i, v)| pointer_of_object_with(v, key, format!("{at}/{i}"))),
        _ => None,
    }
}

#[test]
// schema-rejection-test: CMS `swap_convention`, `swap_day_count`
fn cms_retired_convention_keys_are_rejected() {
    for (retired, value) in [
        ("swap_convention", serde_json::json!("usd_sofr")),
        ("swap_day_count", serde_json::json!("act_360")),
    ] {
        assert_rejects_at(&CmsSwap::example(), "", retired, value.clone());
        assert_rejects_at(&CmsOption::example(), "", retired, value.clone());
        assert_rejects_at(&CmsSpreadOption::example(), "", retired, value);
    }
}

#[test]
fn cms_index_id_is_a_registry_key_string() {
    let json = serde_json::to_value(CmsSwap::example()).expect("serialize");
    assert_eq!(json["index_id"], serde_json::json!("USD-SOFR-OIS"));
    let mut unknown = json;
    unknown["index_id"] = serde_json::json!("XXX-NOT-AN-INDEX");
    let swap: CmsSwap = serde_json::from_value(unknown).expect("any registry key deserializes");
    assert!(
        swap.reference_swap().resolved_fixed_frequency().is_err(),
        "an unregistered index_id must fail at convention resolution"
    );
}

#[test]
// schema-rejection-test: Snowball/Tarn `floating_index_id`, `floating_tenor`
fn snowball_and_tarn_retired_index_keys_are_rejected() {
    let tenor = serde_json::json!({"count": 6, "unit": "months"});
    assert_rejects_at(
        &Snowball::example_snowball(),
        "",
        "floating_index_id",
        serde_json::json!("USD-SOFR-6M"),
    );
    assert_rejects_at(
        &Snowball::example_snowball(),
        "",
        "floating_tenor",
        tenor.clone(),
    );
    assert_rejects_at(
        &Tarn::example(),
        "",
        "floating_index_id",
        serde_json::json!("USD-SOFR-6M"),
    );
    assert_rejects_at(&Tarn::example(), "", "floating_tenor", tenor);
}

#[test]
// schema-rejection-test: RangeAccrual `rate_index_id`, `projection_curve_id`, `reference_tenor`
fn range_accrual_retired_rate_keys_are_rejected() {
    let range = RangeAccrual::example();
    assert_rejects_at(&range, "", "rate_index_id", serde_json::json!("SOFR"));
    assert_rejects_at(
        &range,
        "",
        "projection_curve_id",
        serde_json::json!("USD-OIS"),
    );
    assert_rejects_at(
        &range,
        "",
        "reference_tenor",
        serde_json::json!({"count": 3, "unit": "months"}),
    );
}

#[test]
// schema-rejection-test: InterestRateFuture `fixing_index_id`
fn ir_future_retired_fixing_index_key_is_rejected() {
    let future = InterestRateFuture::example().expect("example");
    assert_rejects_at(
        &future,
        "",
        "fixing_index_id",
        serde_json::json!("USD-SOFR"),
    );
}

#[test]
// schema-rejection-test: CommodityFuture `price_curve_id`, CommoditySwap `floating_index_id`
fn commodity_retired_price_curve_keys_are_rejected() {
    let future = CommodityFuture::example().expect("example");
    assert_rejects_at(&future, "", "price_curve_id", serde_json::json!("WTI-FWD"));
    assert_rejects_at(
        &CommoditySwap::example(),
        "",
        "floating_index_id",
        serde_json::json!("NG-SPOT-AVG"),
    );
}

#[test]
// schema-rejection-test: Bond `funding_curve_id`
fn bond_retired_funding_curve_key_is_rejected() {
    let bond = Bond::example().expect("example");
    assert_rejects_at(&bond, "", "funding_curve_id", serde_json::json!("USD-REPO"));
}

#[test]
// schema-rejection-test: FloatingRateSpec `index_id`
fn floating_rate_spec_retired_index_key_is_rejected() {
    let bond = Bond::example_floating().expect("example");
    let json = serde_json::to_value(&bond).expect("serialize");
    let pointer = pointer_of_object_with(&json, "forward_curve_id", String::new())
        .expect("floating rate spec present");
    assert_rejects_at(
        &bond,
        &pointer,
        "index_id",
        serde_json::json!("USD-SOFR-3M"),
    );

    let loan = TermLoan::example_floating_with_ddtl().expect("example");
    let json = serde_json::to_value(&loan).expect("serialize");
    let pointer = pointer_of_object_with(&json, "forward_curve_id", String::new())
        .expect("floating rate spec present");
    assert_rejects_at(
        &loan,
        &pointer,
        "index_id",
        serde_json::json!("USD-SOFR-3M"),
    );
}
