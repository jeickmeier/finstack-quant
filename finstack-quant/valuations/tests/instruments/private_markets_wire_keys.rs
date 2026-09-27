//! Private-markets wire keys retired by the naming-consistency remediation.
//!
//! RealEstateAsset prices against the caller's `as_of` (no stored
//! `valuation_date`), acquisition costs have one channel (`acquisition_costs`),
//! the levered exit follows the asset `sale_date`, and PrivateMarketsFund uses
//! `hurdle_irr`, `day_count`, `catch_up_mode` and a presence-switched
//! `clawback`. Each retired key is rejected by `deny_unknown_fields`.

use finstack_quant_valuations::instruments::equity::pe_fund::{
    ClawbackSettle, ClawbackSpec, WaterfallSpec,
};
use finstack_quant_valuations::instruments::equity::{
    LeveredRealEstateEquity, PrivateMarketsFund, RealEstateAsset,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

/// Serialize `value`, apply `edit`, and require deserialization to fail.
fn assert_rejected<T: Serialize + DeserializeOwned>(value: &T, edit: impl FnOnce(&mut Value)) {
    let mut wire = serde_json::to_value(value).expect("serialize");
    edit(&mut wire);
    assert!(
        serde_json::from_value::<T>(wire).is_err(),
        "retired key must be rejected"
    );
}

fn object(value: &mut Value) -> &mut serde_json::Map<String, Value> {
    value.as_object_mut().expect("object")
}

#[test]
// schema-rejection-test: RealEstateAsset `valuation_date` and `acquisition_cost`
fn retired_real_estate_asset_keys_are_rejected() {
    let asset = RealEstateAsset::example().expect("example");
    assert_rejected(&asset, |w| {
        object(w).insert("valuation_date".into(), json!("2025-01-01"));
    });
    assert_rejected(&asset, |w| {
        object(w).insert("acquisition_cost".into(), json!(1000.0));
    });
}

#[test]
// schema-rejection-test: LeveredRealEstateEquity `exit_date` (now `asset.sale_date`)
fn retired_levered_exit_date_is_rejected() {
    let levered = LeveredRealEstateEquity::example().expect("example");
    assert_rejected(&levered, |w| {
        object(w).insert("exit_date".into(), json!("2030-01-01"));
    });
}

#[test]
// schema-rejection-test: WaterfallSpec `irr_basis`, `catchup_mode`, `irr`, `hurdle`
fn retired_waterfall_spec_keys_are_rejected() {
    let fund = PrivateMarketsFund::example().expect("example");
    let spec: &WaterfallSpec = &fund.waterfall_spec;
    assert_rejected(spec, |w| {
        let map = object(w);
        let dc = map.remove("day_count").expect("day_count");
        map.insert("irr_basis".into(), dc);
    });
    assert_rejected(spec, |w| {
        let map = object(w);
        let mode = map.remove("catch_up_mode").expect("catch_up_mode");
        map.insert("catchup_mode".into(), mode);
    });
    assert_rejected(spec, |w| {
        let tranches = w["tranches"].as_array_mut().expect("tranches");
        for t in tranches.iter_mut() {
            if let Some(pref) = t.get_mut("preferred_irr") {
                *pref = json!({ "irr": 0.08 });
            }
        }
    });
    assert_rejected(spec, |w| {
        let tranches = w["tranches"].as_array_mut().expect("tranches");
        for t in tranches.iter_mut() {
            if let Some(tier) = t.get_mut("promote_tier") {
                let map = object(tier);
                map.remove("hurdle_irr");
                map.insert("hurdle".into(), json!({ "irr": { "rate": 0.12 } }));
            }
        }
    });
}

#[test]
// schema-rejection-test: ClawbackSpec `enable` (presence of `clawback` is the switch)
fn retired_clawback_enable_is_rejected() {
    let clawback = ClawbackSpec {
        holdback_decimal: Some(0.2),
        settle_on: ClawbackSettle::FundEnd,
    };
    assert_rejected(&clawback, |w| {
        object(w).insert("enable".into(), json!(true));
    });
}
