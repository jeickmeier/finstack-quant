//! Contract dates and calendars share one wire vocabulary:
//!
//! - flat contract dates are `start_date` / `maturity`, and bonds and loans
//!   (including revolvers) carry `issue_date`;
//! - an option's underlying contract uses `underlying_start_date` /
//!   `underlying_maturity`;
//! - every option expiry is `expiry`;
//! - Bermudan exercise dates are `exercise_dates`, wrapped in
//!   `exercise_schedule` when a lockout travels with them, and a call lockout
//!   is the date `lockout_end`;
//! - window structs use `start` / `end`;
//! - a single-schedule deal names its calendar `calendar_id` and its
//!   convention `business_day_convention`.
//!
//! Each retired spelling is rejected by `deny_unknown_fields`.

use finstack_quant_valuations::instruments::fixed_income::bond::CallPut;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    ControlledAccumulationSpec, ReinvestmentPeriod,
};
use finstack_quant_valuations::instruments::{
    AssetBackedFacility, BermudanSwaption, CallableRangeAccrual, CdsOption, CdsTranche,
    CmsSpreadOption, CommodityOption, CommoditySwaption, EquityOption, RevolvingCredit,
    StructuredCredit,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

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

/// Require `json` to fail deserializing as `T` with an error naming `retired`.
fn assert_rejects<T: DeserializeOwned>(json: Value, retired: &str) {
    let err = serde_json::from_value::<T>(json)
        .err()
        .unwrap_or_else(|| panic!("retired key {retired} must be rejected"));
    assert!(err.to_string().contains(retired), "{retired}: {err}");
}

#[test]
// schema-rejection-test: RangeAccrual `accrual_start_date`, CdsTranche `effective_date`
fn retired_contract_start_keys_are_rejected() {
    assert_rejects_at(
        &CallableRangeAccrual::example().expect("example"),
        "/range_accrual",
        "accrual_start_date",
        json!("2025-12-31"),
    );
    assert_rejects_at(
        &CdsTranche::example().expect("example"),
        "",
        "effective_date",
        json!("2025-03-20"),
    );
}

#[test]
// schema-rejection-test: CommoditySwaption `swap_start`, `swap_end`; CdsOption `underlying_effective_date`, `cds_maturity`
fn retired_underlying_date_keys_are_rejected() {
    let swaption = CommoditySwaption::example().expect("example");
    assert_rejects_at(&swaption, "", "swap_start", json!("2025-07-01"));
    assert_rejects_at(&swaption, "", "swap_end", json!("2026-06-30"));
    let option = CdsOption::example().expect("cds option");
    assert_rejects_at(
        &option,
        "",
        "underlying_effective_date",
        json!("2025-03-20"),
    );
    assert_rejects_at(&option, "", "cds_maturity", json!("2030-06-20"));
}

#[test]
// schema-rejection-test: CmsSpreadOption `expiry_date`
fn retired_option_expiry_date_key_is_rejected() {
    assert_rejects_at(
        &CmsSpreadOption::example().expect("example"),
        "",
        "expiry_date",
        json!("2027-03-29"),
    );
}

#[test]
// schema-rejection-test: BermudanSwaption `bermudan_schedule`, EquityOption and CommodityOption `exercise_schedule`
fn retired_bermudan_exercise_keys_are_rejected() {
    let schedule = serde_json::to_value(BermudanSwaption::example().expect("example"))
        .expect("serialize")
        .get("exercise_schedule")
        .cloned()
        .expect("exercise_schedule");
    assert_rejects_at(
        &BermudanSwaption::example().expect("example"),
        "",
        "bermudan_schedule",
        schedule,
    );
    assert_rejects_at(
        &EquityOption::example().expect("equity option"),
        "",
        "exercise_schedule",
        json!(["2025-06-20"]),
    );
    assert_rejects_at(
        &CommodityOption::example().expect("example"),
        "",
        "exercise_schedule",
        json!(["2025-06-20"]),
    );
}

#[test]
// schema-rejection-test: CallPut `start_date`, `end_date`; ReinvestmentPeriod `end_date`; ControlledAccumulationSpec `start_date`
fn retired_window_bound_keys_are_rejected() {
    assert_rejects::<CallPut>(
        json!({"start_date": "2027-01-15", "end": "2029-01-15", "price_pct_of_par": 101.0}),
        "start_date",
    );
    assert_rejects::<CallPut>(
        json!({"start": "2027-01-15", "end_date": "2029-01-15", "price_pct_of_par": 101.0}),
        "end_date",
    );
    assert_rejects::<ReinvestmentPeriod>(
        json!({
            "end_date": "2028-01-15",
            "is_active": true,
            "criteria": {"max_price_pct": 100.0, "min_yield": 0.0}
        }),
        "end_date",
    );
    assert_rejects::<ControlledAccumulationSpec>(
        json!({"start_date": "2026-01-15", "bullet_date": "2027-01-15"}),
        "start_date",
    );
}

#[test]
// schema-rejection-test: StructuredCredit `payment_calendar_id`, `payment_business_day_convention`; AssetBackedFacility `payment_calendar_id`; RevolvingCredit `commitment_date`
fn retired_deal_calendar_and_issue_keys_are_rejected() {
    let deal = StructuredCredit::example().expect("example");
    assert_rejects_at(&deal, "", "payment_calendar_id", json!("nyse"));
    assert_rejects_at(
        &deal,
        "",
        "payment_business_day_convention",
        json!("modified_following"),
    );
    assert_rejects_at(
        &AssetBackedFacility::example().expect("facility"),
        "",
        "payment_calendar_id",
        json!("nyse"),
    );
    assert_rejects_at(
        &RevolvingCredit::example().expect("revolver"),
        "",
        "commitment_date",
        json!("2025-01-01"),
    );
}

#[test]
fn canonical_date_and_calendar_keys_round_trip() {
    let deal = serde_json::to_value(
        StructuredCredit::example()
            .expect("example")
            .with_calendar_id("nyse"),
    )
    .expect("serialize");
    assert_eq!(deal["calendar_id"], json!("nyse"));
    let revolver =
        serde_json::to_value(RevolvingCredit::example().expect("revolver")).expect("serialize");
    assert!(revolver.get("issue_date").is_some(), "{revolver}");
    let call: CallPut = serde_json::from_value(
        json!({"start": "2027-01-15", "end": "2029-01-15", "price_pct_of_par": 101.0}),
    )
    .expect("canonical call window");
    assert!(call.start < call.end);
}
