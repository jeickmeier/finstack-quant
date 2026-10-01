//! Coupon, strike and inflation terms share one wire vocabulary:
//!
//! - the margin over a floating or CMS index is `spread_bp` / `cms_spread_bp`
//!   (a `Decimal` in basis points);
//! - the fixing lag is `reset_lag_days`;
//! - a flat contractual rate is `fixed_rate: Decimal`;
//! - the index multiplier is `gearing`;
//! - structured-credit floors are `index_floor_bp` / `all_in_floor_bp`;
//! - explicit schedules are `start_date` plus `payment_dates`, and the CMS swap
//!   schedule vectors carry no `cms_` prefix;
//! - an option strike is `strike`, and a CMS tenor is a `Tenor`;
//! - the commodity swap fixed price is an `f64` per unit;
//! - the base reference CPI is `base_cpi`, and the contractual inflation lag
//!   and interpolation are `lag` / `interpolation`.
//!
//! Each retired spelling is rejected by `deny_unknown_fields`, and the PV of
//! every migrated JSON example is pinned to its reviewed reference value.
//! The CMS and inflation references include subsequent pricing corrections
//! independently reconciled in the September 2026 valuation audit evidence.

use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations::instruments::PricingOptions;
use finstack_quant_valuations::pricer::{parse_boxed_instrument_from_json, price_instrument};
use serde_json::Value;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

/// Read a pricing-case source (`{path, pointer}`) relative to the workspace root.
fn read_source(root: &std::path::Path, source: &Value) -> Value {
    let mut value = read_json(&root.join(source["path"].as_str().expect("path")));
    for seg in source["pointer"].as_array().expect("pointer") {
        value = match seg {
            Value::String(k) => value[k.as_str()].take(),
            Value::Number(i) => {
                value[usize::try_from(i.as_u64().expect("index")).expect("index")].take()
            }
            other => panic!("bad pointer segment {other}"),
        };
    }
    value
}

fn read_json(path: &PathBuf) -> Value {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path:?}: {e}"))
}

fn set_path(root: &mut Value, path: &[Value], value: Value) {
    let mut cur = root;
    for seg in path {
        cur = match seg {
            Value::String(k) => cur
                .as_object_mut()
                .expect("object segment")
                .entry(k.clone())
                .or_insert(Value::Null),
            Value::Number(i) => {
                let idx = usize::try_from(i.as_u64().expect("index")).expect("index");
                &mut cur.as_array_mut().expect("array segment")[idx]
            }
            other => panic!("bad path segment {other}"),
        };
    }
    *cur = value;
}

/// Native PV of each pricing case, including independently checked CMS and
/// inflation pricing corrections after the wire migration.
const PINNED_PV: &[(&str, &str)] = &[
    ("asset_backed_facility", "77045997.819227106393914245443"),
    ("cap_floor", "515716.67931458101"),
    ("cms_option", "153012.6165752798"),
    ("cms_spread_option", "14733.78977571792"),
    ("cms_swap", "-104987.5208795478"),
    ("commodity_swap", "57241.779109393"),
    ("deposit", "725.2194690658727500000"),
    ("forward_rate_agreement", "12414.05804431126"),
    ("inflation_cap_floor", "3742.853205610149"),
    ("inflation_linked_bond", "3359979.7227661326209574555315"),
    ("inflation_swap", "-4080.8031999999000000"),
    ("quanto_option", "257243.9712687773"),
    ("snowball", "994168.490541813"),
    ("structured_credit", "108912295.58406585274007447394"),
    ("tarn", "879899.920584479"),
    ("yoy_inflation_swap", "-13354.31626472641"),
];

/// Price the UI pricing case for `ty` after applying the case's own patches and
/// then `extra` (`(path, value)` pairs on the instrument envelope). Returns the
/// exact `Money` amount string.
fn price_case(cases: &Value, ty: &str, extra: &[(Value, Value)]) -> String {
    let root = workspace_root();
    let case = cases["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|c| c["type"] == ty)
        .unwrap_or_else(|| panic!("no pricing case for {ty}"));
    let mut instrument = read_source(&root, &case["instrumentSource"]);
    for patch in case["instrumentPatches"].as_array().expect("patches") {
        set_path(
            &mut instrument,
            patch["path"].as_array().expect("path"),
            patch["value"].clone(),
        );
    }
    for (path, value) in extra {
        set_path(
            &mut instrument,
            path.as_array().expect("path"),
            value.clone(),
        );
    }
    let mut market = read_source(&root, &case["marketSource"]);
    for patch in case["marketPatches"].as_array().expect("patches") {
        set_path(
            &mut market,
            patch["path"].as_array().expect("path"),
            patch["value"].clone(),
        );
    }
    let market: MarketContext = serde_json::from_value(market).expect("market");
    let parsed = parse_boxed_instrument_from_json(&instrument.to_string(), None)
        .unwrap_or_else(|e| panic!("{ty}: {e}"));
    let request = &case["request"];
    let result = price_instrument(
        &parsed,
        &market,
        request["asOf"].as_str().expect("asOf"),
        request["model"].as_str().expect("model"),
        &[],
        None,
        PricingOptions::default(),
    )
    .unwrap_or_else(|e| panic!("{ty}: {e}"));
    let value = serde_json::to_value(result.value).expect("money");
    value["amount"].as_str().expect("amount").to_string()
}

fn pricing_cases() -> Value {
    read_json(
        &workspace_root().join("finstack-quant-ui/tests/valuations/instruments/pricing-cases.json"),
    )
}

/// Every instrument this slice migrates reprices, from the UI pricing-case
/// catalogue inputs, to the reviewed native PV (`PINNED_PV`). The reference is
/// the exact `Money` amount string, so any unreviewed pricing change fails here.
#[test]
fn migrated_examples_reprice_to_recorded_reference() {
    let cases = pricing_cases();
    let mismatches: Vec<String> = PINNED_PV
        .iter()
        .filter_map(|(ty, expected)| {
            let actual = price_case(&cases, ty, &[]);
            (actual != *expected).then(|| format!("(\"{ty}\", \"{actual}\"),"))
        })
        .collect();
    assert!(
        mismatches.is_empty(),
        "PV moved:\n{}",
        mismatches.join("\n")
    );
}

/// A pricing case, the instrument patches that set a non-zero input, and the
/// independently reviewed PV for that economic input.
type RescaleCheck = (&'static str, Vec<(Value, Value)>, &'static str);

/// Inputs whose unit changed (a decimal margin or floor now quoted in basis
/// points) reprice to the reviewed PV for the same economic input, when the
/// example's zero default is replaced by a non-zero value.
#[test]
fn rescaled_inputs_reprice_to_recorded_reference() {
    use serde_json::json;
    let cases = pricing_cases();
    let spec = |key: &str| json!(["instrument", "spec", key]);
    let checks: Vec<RescaleCheck> = vec![
        (
            // 15bp margin over the cap index (was the decimal `spread` 0.0015).
            "cap_floor",
            vec![(spec("spread_bp"), json!("15"))],
            "578605.83223393675",
        ),
        (
            // 25bp over the CMS rate and a 10bp floating funding margin (were
            // the decimal `cms_spread` 0.0025 and funding `spread` 0.001).
            "cms_swap",
            vec![
                (spec("cms_spread_bp"), json!("25")),
                (
                    spec("funding_leg"),
                    json!({
                        "type": "floating",
                        "spread_bp": "10",
                        "periods": [
                            {"accrual_start": "2025-03-20", "accrual_end": "2025-06-20", "payment_date": "2025-06-20", "reset_date": "2025-03-20", "accrual_year_fraction": 0.25},
                            {"accrual_start": "2025-06-20", "accrual_end": "2025-09-20", "payment_date": "2025-09-22", "reset_date": "2025-06-20", "accrual_year_fraction": 0.25},
                            {"accrual_start": "2025-09-20", "accrual_end": "2025-12-20", "payment_date": "2025-12-22", "reset_date": "2025-09-20", "accrual_year_fraction": 0.25},
                            {"accrual_start": "2025-12-20", "accrual_end": "2026-03-20", "payment_date": "2026-03-20", "reset_date": "2025-12-20", "accrual_year_fraction": 0.25}
                        ],
                        "day_count": "act_360",
                        "forward_curve_id": "USD-SOFR-3M"
                    }),
                ),
            ],
            "-24634.80936387088",
        ),
        (
            // A floating pool asset at SOFR + 450bp with a 5000bp index floor
            // (was the decimal `index_floor` 0.5); the floor binds every period.
            "structured_credit",
            vec![
                (
                    json!([
                        "instrument",
                        "spec",
                        "pool",
                        "assets",
                        0,
                        "forward_curve_id"
                    ]),
                    json!("USD-SOFR-3M"),
                ),
                (
                    json!(["instrument", "spec", "pool", "assets", 0, "spread_bp"]),
                    json!(450.0),
                ),
                (
                    json!(["instrument", "spec", "pool", "assets", 0, "index_floor_bp"]),
                    json!(5000.0),
                ),
            ],
            "109049787.08034628570527971583",
        ),
    ];
    let mismatches: Vec<String> = checks
        .iter()
        .filter_map(|(ty, extra, expected)| {
            let actual = price_case(&cases, ty, extra);
            (actual != *expected).then(|| format!("{ty}: {actual}"))
        })
        .collect();
    assert!(
        mismatches.is_empty(),
        "PV moved:\n{}",
        mismatches.join("\n")
    );
}

/// Serialize `instrument`, insert `retired` with `value` into the object at
/// `pointer` (JSON pointer, `""` for the root), and require a rejection that
/// names the retired key.
fn assert_rejects_at<T: serde::Serialize + serde::de::DeserializeOwned>(
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

#[test]
// schema-rejection-test: CapFloor `spread`, CmsSwap `cms_spread`, AssetBackedFacility `margin_bp`
fn retired_margin_keys_are_rejected() {
    use finstack_quant_valuations::instruments::{AssetBackedFacility, CapFloor, CmsSwap};
    use serde_json::json;
    assert_rejects_at(
        &CapFloor::example().expect("cap"),
        "",
        "spread",
        json!("0.001"),
    );
    assert_rejects_at(
        &CmsSwap::example().expect("example"),
        "",
        "cms_spread",
        json!(0.001),
    );
    assert_rejects_at(
        &AssetBackedFacility::example().expect("facility"),
        "",
        "margin_bp",
        json!(600.0),
    );
}

#[test]
// schema-rejection-test: CmsSwap funding-leg `spread`
fn retired_cms_funding_spread_is_rejected() {
    use finstack_quant_valuations::instruments::CmsSwap;
    use serde_json::json;
    let mut json = serde_json::to_value(CmsSwap::example().expect("example")).expect("serialize");
    let leg = json["funding_leg"].as_object_mut().expect("funding leg");
    leg.insert("type".into(), json!("floating"));
    leg.remove("rate");
    leg.insert("spread_bp".into(), json!("10"));
    leg.insert("spread".into(), json!(0.001));
    leg.insert("forward_curve_id".into(), json!("USD-SOFR-3M"));
    let err = serde_json::from_value::<CmsSwap>(json).expect_err("spread is retired");
    assert!(err.to_string().contains("unknown field `spread`"), "{err}");
}

#[test]
// schema-rejection-test: CmsSwap `cms_fixing_dates`, `cms_payment_dates`, `cms_accrual_fractions`, `cms_day_count`
fn retired_cms_schedule_prefix_is_rejected() {
    use finstack_quant_valuations::instruments::CmsSwap;
    use serde_json::json;
    for (retired, value) in [
        ("cms_fixing_dates", json!(["2025-03-20"])),
        ("cms_payment_dates", json!(["2025-06-20"])),
        ("cms_accrual_fractions", json!([0.25])),
        ("cms_day_count", json!("act_365f")),
    ] {
        assert_rejects_at(&CmsSwap::example().expect("example"), "", retired, value);
    }
}

#[test]
// schema-rejection-test: ForwardRateAgreement `reset_lag`, Deposit `quote_rate`
fn retired_lag_and_rate_keys_are_rejected() {
    use finstack_quant_valuations::instruments::{Deposit, ForwardRateAgreement};
    use serde_json::json;
    assert_rejects_at(
        &ForwardRateAgreement::example().expect("fra"),
        "",
        "reset_lag",
        json!(2),
    );
    assert_rejects_at(
        &Deposit::example().expect("deposit"),
        "",
        "quote_rate",
        json!("0.045"),
    );
}

#[test]
// schema-rejection-test: Snowball `leverage`, `coupon_dates`; Tarn `coupon_dates`
fn retired_structured_note_keys_are_rejected() {
    use finstack_quant_valuations::instruments::{Snowball, Tarn};
    use serde_json::json;
    let dates = json!(["2026-06-30", "2026-12-31"]);
    assert_rejects_at(
        &Snowball::example().expect("example"),
        "",
        "leverage",
        json!(1.0),
    );
    assert_rejects_at(
        &Snowball::example().expect("example"),
        "",
        "coupon_dates",
        dates.clone(),
    );
    assert_rejects_at(
        &Tarn::example().expect("example"),
        "",
        "coupon_dates",
        dates,
    );
}

#[test]
// schema-rejection-test: PoolAsset/RepLine `index_floor`, ReinvestmentAssumptions `coupon_floor`
fn retired_structured_credit_floor_keys_are_rejected() {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
        AssetType, PoolAsset, ReinvestmentAssumptions, RepLine,
    };
    use serde_json::json;
    let maturity = time::macros::date!(2030 - 01 - 15);
    let balance = Money::new(1_000_000.0, Currency::USD).expect("money");
    let asset = PoolAsset::floating_rate_loan(
        "L",
        balance,
        "USD-SOFR-3M",
        400.0,
        maturity,
        DayCount::Act360,
    );
    assert_rejects_at(&asset, "", "index_floor", json!(0.01));
    let line = RepLine::new(
        "R",
        balance,
        0.07,
        maturity,
        DayCount::Act360,
        AssetType::FirstLienLoan {},
    );
    assert_rejects_at(&line, "", "index_floor", json!(0.01));
    let assumptions = ReinvestmentAssumptions {
        spread_bp: 350.0,
        price_pct: 99.0,
        maturity_months: 60,
        forward_curve_id: Some("USD-SOFR-3M".into()),
        all_in_floor_bp: Some(100.0),
    };
    assert_rejects_at(&assumptions, "", "coupon_floor", json!(0.01));
}

#[test]
// schema-rejection-test: QuantoOption `equity_strike`, InflationLinkedBond `base_index`
fn retired_strike_and_base_index_keys_are_rejected() {
    use finstack_quant_valuations::instruments::{InflationLinkedBond, QuantoOption};
    use serde_json::json;
    assert_rejects_at(
        &QuantoOption::example().expect("example"),
        "",
        "equity_strike",
        json!({"amount": "35000", "currency": "JPY"}),
    );
    assert_rejects_at(
        &InflationLinkedBond::example().expect("example"),
        "",
        "base_index",
        json!(100.0),
    );
}

#[test]
// schema-rejection-test: `lag_override`, `interpolation_override` on the inflation derivatives
fn retired_inflation_override_keys_are_rejected() {
    use finstack_quant_valuations::instruments::{
        InflationCapFloor, InflationSwap, YoYInflationSwap,
    };
    use serde_json::json;
    for (retired, value) in [
        ("lag_override", json!({"months": 3})),
        ("interpolation_override", json!("linear")),
    ] {
        assert_rejects_at(
            &InflationSwap::example().expect("example"),
            "",
            retired,
            value.clone(),
        );
        assert_rejects_at(
            &YoYInflationSwap::example().expect("example"),
            "",
            retired,
            value.clone(),
        );
        assert_rejects_at(
            &InflationCapFloor::example().expect("example"),
            "",
            retired,
            value,
        );
    }
}

/// A CMS reference tenor must be a whole number of months: a week-based
/// tenor has no reference swap and is rejected with the field's wire path.
#[test]
fn cms_tenor_must_be_month_based() {
    use finstack_quant_core::dates::{Tenor, TenorUnit};
    use finstack_quant_valuations::instruments::CmsSwap;
    let mut swap = CmsSwap::example().expect("example");
    swap.cms_tenor = Tenor::new(2, TenorUnit::Weeks).expect("tenor");
    let err = swap.validate().expect_err("week tenor is rejected");
    assert!(err.to_string().contains("CmsSwap cms_tenor"), "{err}");
}
