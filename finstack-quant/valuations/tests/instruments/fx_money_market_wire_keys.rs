//! FX and money-market instruments share one wire vocabulary:
//!
//! - the quote-currency discount curve is `domestic_discount_curve_id`
//!   (FxSpot joins FxForward, FxSwap, Ndf and the FX options);
//! - the base-currency notional is `notional` (FxSwap);
//! - Ndf's settlement-currency calendar is `settlement_calendar_id` and its
//!   fixing benchmark is `fixing_source`;
//! - an agreed forward rate is optional, `None` meaning at-market (Ndf joins
//!   FxForward);
//! - every FX option-style instrument defaults `day_count` to ACT/365F;
//! - repo collateral is identified once, by `CollateralSpec.instrument_id`.
//!
//! Each retired spelling is rejected by `deny_unknown_fields`.

use finstack_quant_core::dates::DayCount;
use finstack_quant_valuations::instruments::fx::fx_digital_option::FxDigitalOption;
use finstack_quant_valuations::instruments::fx::fx_spot::FxSpot;
use finstack_quant_valuations::instruments::fx::fx_swap::FxSwap;
use finstack_quant_valuations::instruments::fx::fx_touch_option::FxTouchOption;
use finstack_quant_valuations::instruments::fx::fx_variance_swap::FxVarianceSwap;
use finstack_quant_valuations::instruments::fx::ndf::Ndf;
use finstack_quant_valuations::instruments::fx::quanto_option::QuantoOption;
use finstack_quant_valuations::instruments::rates::repo::CollateralType;
use finstack_quant_valuations::instruments::rates::repo::Repo;
use serde_json::{json, Value};

fn rejects<T: serde::de::DeserializeOwned>(mut value: Value, new_key: &str, old_key: &str) {
    let moved = value[new_key].take();
    let obj = value.as_object_mut().expect("object");
    obj.remove(new_key);
    obj.insert(old_key.to_string(), moved);
    let Err(err) = serde_json::from_value::<T>(value) else {
        panic!("retired key `{old_key}` must be rejected");
    };
    assert!(
        err.to_string().contains(old_key),
        "error should name the retired key `{old_key}`: {err}"
    );
}

#[test]
fn retired_fx_keys_are_rejected() {
    let mut spot = serde_json::to_value(FxSpot::example().expect("example")).expect("ser");
    spot["domestic_discount_curve_id"] = json!("USD-OIS");
    // schema-rejection-test
    rejects::<FxSpot>(spot, "domestic_discount_curve_id", "discount_curve_id");

    let swap = serde_json::to_value(FxSwap::example()).expect("ser");
    // schema-rejection-test
    rejects::<FxSwap>(swap, "notional", "base_notional");

    let mut ndf = serde_json::to_value(Ndf::example()).expect("ser");
    ndf["settlement_calendar_id"] = json!("nyse");
    ndf["fixing_source"] = json!("pboc");
    // schema-rejection-test
    rejects::<Ndf>(ndf.clone(), "settlement_calendar_id", "quote_calendar_id");
    // schema-rejection-test
    rejects::<Ndf>(ndf, "fixing_source", "fixing_source_enum");
}

#[test]
fn retired_repo_special_security_id_is_rejected() {
    let mut repo = serde_json::to_value(Repo::example()).expect("ser");
    repo["collateral"]["collateral_type"] = json!({"special": {"rate_adjustment_bp": -25.0}});
    let parsed: Repo = serde_json::from_value(repo.clone()).expect("special collateral parses");
    assert_eq!(
        parsed.collateral.collateral_type,
        CollateralType::Special {
            rate_adjustment_bp: Some(-25.0)
        }
    );
    // schema-rejection-test
    repo["collateral"]["collateral_type"]["special"]["security_id"] = json!("ON_THE_RUN_10Y");
    let err = serde_json::from_value::<Repo>(repo).expect_err("security_id is retired");
    assert!(err.to_string().contains("security_id"), "{err}");
}

#[test]
fn ndf_contract_rate_is_optional_on_the_wire() {
    let mut ndf = Ndf::example();
    ndf.contract_rate = None;
    let json = serde_json::to_value(&ndf).expect("ser");
    assert!(json.get("contract_rate").is_none());
    let parsed: Ndf = serde_json::from_value(json).expect("at-market NDF parses");
    assert_eq!(parsed.contract_rate, None);
}

#[test]
fn fx_option_style_instruments_default_day_count_to_act365f() {
    fn check<T: serde::Serialize + serde::de::DeserializeOwned>(
        example: T,
        get: fn(&T) -> DayCount,
    ) {
        let mut value = serde_json::to_value(&example).expect("ser");
        value.as_object_mut().expect("object").remove("day_count");
        let parsed: T = serde_json::from_value(value).expect("day_count is optional");
        assert_eq!(get(&parsed), DayCount::Act365F);
    }
    check(FxDigitalOption::example().expect("example"), |i| {
        i.day_count
    });
    check(FxTouchOption::example().expect("example"), |i| i.day_count);
    check(QuantoOption::example(), |i| i.day_count);
    check(FxVarianceSwap::example(), |i| i.day_count);
}
