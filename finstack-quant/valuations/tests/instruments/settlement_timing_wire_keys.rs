//! Settlement and payment timing use one spelling per concept on the wire:
//! `settlement_date` for a date, `settlement_days` for the T+N lag,
//! `payment_lag_days` for the business-day payment lag, `stated_delay_days`
//! for the agency MBS calendar-day delay, `premium_settlement_date` for an
//! option premium payment date and `fixing` for a commodity future's
//! final-settlement price rule. Each retired spelling is rejected by
//! `deny_unknown_fields`.

use finstack_quant_valuations::instruments::rates::cap_floor::OvernightCouponConvention;
use finstack_quant_valuations::instruments::{
    AgencyMbsPassthrough, CDSOption, CommodityForward, CommodityFuture, Deposit,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

/// Serialize `instrument`, drop the canonical top-level key when present,
/// insert the retired spelling with `value`, and require rejection.
fn assert_rejects<T: Serialize + DeserializeOwned>(
    instrument: &T,
    canonical: &str,
    retired: &str,
    value: Value,
) {
    let mut json = serde_json::to_value(instrument).expect("serialize");
    let map = json
        .as_object_mut()
        .expect("instrument serializes as an object");
    map.remove(canonical);
    map.insert(retired.to_string(), value);
    let err = serde_json::from_value::<T>(json)
        .err()
        .unwrap_or_else(|| panic!("retired key {retired} must be rejected"));
    assert!(err.to_string().contains(retired), "{retired}: {err}");
}

#[test]
// schema-rejection-test: CDSOption `cash_settlement_date`
fn cds_option_retired_cash_settlement_date_is_rejected() {
    let option = CDSOption::example().expect("example");
    assert_rejects(
        &option,
        "premium_settlement_date",
        "cash_settlement_date",
        serde_json::json!("2026-05-12"),
    );
}

#[test]
// schema-rejection-test: CommodityFuture root `settlement` (now `fixing`)
fn commodity_future_retired_settlement_rule_is_rejected() {
    let future = CommodityFuture::example().expect("example");
    let rule = serde_json::to_value(&future).expect("serialize")["fixing"].clone();
    assert!(rule.is_object(), "fixing rule must serialize");
    assert_rejects(&future, "fixing", "settlement", rule);
}

#[test]
// schema-rejection-test: CommodityForward `settlement_lag_days`
fn commodity_forward_retired_settlement_lag_days_is_rejected() {
    assert_rejects(
        &CommodityForward::example(),
        "settlement_days",
        "settlement_lag_days",
        serde_json::json!(2),
    );
}

#[test]
// schema-rejection-test: Deposit `spot_lag_days`
fn deposit_retired_spot_lag_days_is_rejected() {
    let deposit = Deposit::example().expect("example");
    assert_rejects(
        &deposit,
        "settlement_days",
        "spot_lag_days",
        serde_json::json!(2),
    );
}

#[test]
fn deposit_negative_settlement_days_fail_deserialization() {
    // `settlement_days` is an unsigned T+N lag; a negative lag cannot be expressed.
    let mut json = serde_json::to_value(Deposit::example().expect("example")).expect("ser");
    json["settlement_days"] = serde_json::json!(-1);
    serde_json::from_value::<Deposit>(json).expect_err("negative lag must be rejected");
}

#[test]
// schema-rejection-test: AgencyMbsPassthrough `payment_lag_days`
fn mbs_retired_payment_lag_days_is_rejected() {
    let mbs = AgencyMbsPassthrough::example().expect("example");
    assert_rejects(
        &mbs,
        "stated_delay_days",
        "payment_lag_days",
        serde_json::json!(55),
    );
}

#[test]
// schema-rejection-test: CapFloor `overnight_coupon.payment_delay_days`
fn cap_floor_retired_payment_delay_days_is_rejected() {
    let canonical = serde_json::json!({
        "compounding": {"compounded_in_arrears": {"lookback_days": 0}},
        "payment_lag_days": 2
    });
    let convention: OvernightCouponConvention =
        serde_json::from_value(canonical).expect("canonical key deserializes");
    assert_eq!(convention.payment_lag_days, 2);
    assert_rejects(
        &convention,
        "payment_lag_days",
        "payment_delay_days",
        serde_json::json!(2),
    );
}
