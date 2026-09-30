//! Loan and facility terms share one wire vocabulary:
//!
//! - a facility's size is `commitment` and its drawn balance `drawn`
//!   (RevolvingCredit, DDTL, AssetBackedFacility; the cashflow envelope's
//!   `CashFlowMeta.commitment` and `FeeBase::Undrawn.commitment`);
//! - the coupon is one `RateSpec { fixed { rate }, floating }` with a decimal
//!   fixed rate (TermLoan `rate`, RevolvingCredit `rate`, AssetBackedFacility
//!   `rate`, structured-credit tranche `coupon`);
//! - term-loan amortization is the shared cashflows `AmortizationSpec`;
//! - a make-whole call carries the shared `MakeWholeSpec`;
//! - contractual margin and fee bp are `Decimal`s (no `i32` bp);
//! - commitment steps carry `reduction_fee_bp`, the undrawn fee is
//!   `commitment_fee_bp`, the upfront fee is `Option<UpfrontFee>`, and
//!   scheduled draws are `draws: Vec<DrawEvent>`;
//! - the utilization OU uses `kappa` / `theta` / `sigma`;
//! - a convertible's coupon is `cashflow_spec`, and both of its share-price
//!   triggers are one `PriceTrigger`.
//!
//! Each retired spelling is rejected by `deny_unknown_fields`, and the PV of
//! each migrated input is pinned bit-for-bit (exact `Money` amount string) to
//! the value the equivalent pre-migration input gave.

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

/// Instrument envelope and market of one UI pricing case, with the case's own
/// instrument and market patches applied.
pub(crate) fn pricing_case_inputs(case: &Value) -> (Value, MarketContext) {
    let root = workspace_root();
    let mut instrument = read_source(&root, &case["instrumentSource"]);
    for patch in case["instrumentPatches"].as_array().expect("patches") {
        set_path(
            &mut instrument,
            patch["path"].as_array().expect("path"),
            patch["value"].clone(),
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
    (instrument, market)
}

/// Price the UI pricing case for `ty` after applying the case's own patches and
/// then `extra` (`(path, value)` pairs on the instrument envelope). Returns the
/// exact `Money` amount string.
fn price_case(cases: &Value, ty: &str, extra: &[(Value, Value)]) -> String {
    let case = cases["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|c| c["type"] == ty)
        .unwrap_or_else(|| panic!("no pricing case for {ty}"));
    let (mut instrument, market) = pricing_case_inputs(case);
    for (path, value) in extra {
        set_path(
            &mut instrument,
            path.as_array().expect("path"),
            value.clone(),
        );
    }
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

pub(crate) fn pricing_cases() -> Value {
    read_json(
        &workspace_root().join("finstack-quant-ui/tests/valuations/instruments/pricing-cases.json"),
    )
}

/// Case name, UI pricing-case type, instrument patches (new wire form) and
/// the PV the same economics gave before the migration.
type Check = (
    &'static str,
    &'static str,
    Vec<(Value, Value)>,
    &'static str,
);

fn checks() -> Vec<Check> {
    use serde_json::json;
    let s = |k: &str| json!(["instrument", "spec", k]);
    let usd = |a: &str| json!({"amount": a, "currency": "USD"});
    vec![
        (
            "tl_base",
            "term_loan",
            vec![],
            "1129270.412017839474630499983",
        ),
        (
            "tl_fixed_333",
            "term_loan",
            vec![(s("rate"), json!({"fixed": {"rate": 0.0333}}))],
            "128039.185672074002489866217",
        ),
        (
            "tl_amort_orig",
            "term_loan",
            vec![(
                s("amortization"),
                json!({"percent_of_original_per_period": {"pct": 0.01}}),
            )],
            "1282141.293427806641662734845",
        ),
        (
            "tl_amort_linear",
            "term_loan",
            vec![(
                s("amortization"),
                json!({"linear_between": {"start": "2025-01-01", "end": "2029-01-01"}}),
            )],
            "900310.269035339719196530634",
        ),
        (
            "tl_amort_custom",
            "term_loan",
            vec![(
                s("amortization"),
                json!({"custom_principal": {"items": [["2025-01-01", usd("1000000")], ["2026-01-01", usd("2000000")]]}}),
            )],
            "1136890.884855133304481358402",
        ),
        (
            "tl_upfront",
            "term_loan",
            vec![(s("upfront_fee"), json!({"amount": usd("150000")}))],
            "1279220.350443457524630499983",
        ),
        (
            "tl_margin_steps",
            "term_loan",
            vec![(
                s("covenants"),
                json!({"margin_steps": [{"date": "2025-06-01", "delta_bp": "125"}], "pik_toggles": [], "cash_sweeps": [], "draw_stop_dates": []}),
            )],
            "1424178.973550816938735374326",
        ),
        (
            "tl_makewhole",
            "term_loan",
            vec![(
                s("call_schedule"),
                json!({"calls": [{"date": "2026-01-01", "price_pct_of_par": 101.0, "call_type": {"make_whole": {"reference_curve_id": "USD-OIS", "spread_bp": 50.0}}}]}),
            )],
            "1129270.412017839474630499983",
        ),
        (
            "tl_ddtl",
            "term_loan",
            vec![(
                s("ddtl"),
                json!({
                    "commitment": usd("12000000"),
                    "availability_start": "2024-01-01",
                    "availability_end": "2025-01-01",
                    "draws": [{"date": "2024-06-03", "amount": usd("2000000")}],
                    "commitment_steps": [{"date": "2024-09-03", "amount": usd("11000000"), "reduction_fee_bp": "0"}],
                    "usage_fee_bp": "12.5",
                    "commitment_fee_bp": "37.5",
                    "fee_base": "undrawn",
                    "oid_policy": {"withheld_bp": "150"}
                }),
            )],
            "283521.10698740233784858648805",
        ),
        (
            "tl_ddtl_sep",
            "term_loan",
            vec![(
                s("ddtl"),
                json!({
                    "commitment": usd("12000000"),
                    "availability_start": "2024-01-01",
                    "availability_end": "2025-01-01",
                    "draws": [{"date": "2024-06-03", "amount": usd("2000000")}],
                    "commitment_steps": [],
                    "usage_fee_bp": "25",
                    "commitment_fee_bp": "50",
                    "fee_base": "commitment_minus_outstanding",
                    "oid_policy": {"separate_bp": "200"}
                }),
            )],
            "316852.54901668496318353932995",
        ),
        ("rc_base", "revolving_credit", vec![], "1712846.279446133"),
        (
            "rc_fixed",
            "revolving_credit",
            vec![(s("rate"), json!({"fixed": {"rate": 0.0575}}))],
            "1426733.85989398",
        ),
        (
            "rc_steps",
            "revolving_credit",
            vec![
                (
                    s("commitment_steps"),
                    json!([{"date": "2025-01-01", "amount": usd("40000000"), "reduction_fee_bp": "25"}]),
                ),
                (
                    s("margin_steps"),
                    json!([{"date": "2025-03-01", "delta_bp": "-25"}]),
                ),
            ],
            "1625928.952566946",
        ),
        (
            "abf_base",
            "asset_backed_facility",
            vec![],
            "77045997.819227106393914245443",
        ),
        (
            "abf_draws",
            "asset_backed_facility",
            vec![(
                s("draws"),
                json!([{"date": "2024-06-15", "amount": usd("5000000")}]),
            )],
            "77527928.198651577739903836763",
        ),
        (
            "abf_float",
            "asset_backed_facility",
            vec![(
                s("rate"),
                json!({"floating": {"forward_curve_id": "USD-SOFR-3M", "spread_bp": "275", "gearing": "1", "gearing_includes_spread": true, "reset_frequency": {"count": 3, "unit": "months"}, "reset_lag_days": 0}}),
            )],
            "78803034.559481118830452616678",
        ),
        ("cb_base", "convertible_bond", vec![], "1012.328767123288"),
        (
            "cb_zero",
            "convertible_bond",
            vec![(
                json!(["instrument", "spec", "cashflow_spec", "fixed", "rate"]),
                json!("0"),
            )],
            "1000",
        ),
        (
            "cb_trigger",
            "convertible_bond",
            vec![
                (s("call_put"), Value::Null),
                (
                    s("conversion"),
                    json!({"anti_dilution": "none", "dilution_events": [], "dividend_adjustment": "none", "policy": {"upon_event": {"price_trigger": {"threshold_pct": 130.0, "observation_days": 20, "required_days_above": 20}}}, "price": 1.0, "ratio": null}),
                ),
            ],
            "1079.933110532801",
        ),
        (
            "cb_trigger_lo",
            "convertible_bond",
            vec![
                (s("call_put"), Value::Null),
                (
                    s("conversion"),
                    json!({"anti_dilution": "none", "dilution_events": [], "dividend_adjustment": "none", "policy": {"upon_event": {"price_trigger": {"threshold_pct": 110.0, "observation_days": 20, "required_days_above": 20}}}, "price": 1.0, "ratio": null}),
                ),
            ],
            "1111.288221171224",
        ),
        (
            "cb_cp100",
            "convertible_bond",
            vec![
                (s("call_put"), Value::Null),
                (
                    s("conversion"),
                    json!({"anti_dilution": "none", "dilution_events": [], "dividend_adjustment": "none", "policy": "voluntary", "price": 1.0, "ratio": null}),
                ),
            ],
            // Last-digit move from the Tsiveriotis-Zhang terminal-split
            // smoothing (481dd75f3); the rename itself is still bit-exact.
            "1119.083135256321",
        ),
        (
            "cb_zero_cp",
            "convertible_bond",
            vec![
                (s("call_put"), Value::Null),
                (
                    json!(["instrument", "spec", "cashflow_spec", "fixed", "rate"]),
                    json!("0"),
                ),
                (
                    s("conversion"),
                    json!({"anti_dilution": "none", "dilution_events": [], "dividend_adjustment": "none", "policy": "voluntary", "price": 1.0, "ratio": null}),
                ),
            ],
            "1069.083135256321",
        ),
        (
            "sc_base",
            "structured_credit",
            vec![],
            "108912295.58406585274007447394",
        ),
        (
            "lre_base",
            "levered_real_estate_equity",
            vec![],
            "1636404.266927017",
        ),
    ]
}

/// Every migrated input reprices, from the UI pricing-case catalogue plus the
/// case's new-form patches, to the PV the equivalent pre-migration input gave
/// (captured natively before the rename). The reference is the exact `Money`
/// amount string, so the check is bit-for-bit.
#[test]
fn migrated_inputs_reprice_to_recorded_reference() {
    let cases = pricing_cases();
    let mismatches: Vec<String> = checks()
        .into_iter()
        .filter_map(|(name, ty, extra, expected)| {
            let actual = price_case(&cases, ty, &extra);
            (actual != expected).then(|| format!("{name}: {actual} (was {expected})"))
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
// schema-rejection-test: RevolvingCredit `commitment_amount`, `drawn_amount`, `base_rate_spec`
fn retired_revolver_keys_are_rejected() {
    use finstack_quant_valuations::instruments::RevolvingCredit;
    use serde_json::json;
    let rc = RevolvingCredit::example().expect("revolver");
    let usd = json!({"amount": "1", "currency": "USD"});
    assert_rejects_at(&rc, "", "commitment_amount", usd.clone());
    assert_rejects_at(&rc, "", "drawn_amount", usd);
    assert_rejects_at(&rc, "", "base_rate_spec", json!({"fixed": {"rate": 0.05}}));
}

#[test]
// schema-rejection-test: CommitmentStep `fee_bp`, DdtlSpec `commitment_limit`
fn retired_commitment_keys_are_rejected() {
    use finstack_quant_valuations::instruments::fixed_income::loan_terms::CommitmentStep;
    use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
    use serde_json::json;
    let step =
        json!({"date": "2025-01-01", "amount": {"amount": "1", "currency": "USD"}, "fee_bp": 25.0});
    let err = serde_json::from_value::<CommitmentStep>(step).expect_err("fee_bp is retired");
    assert!(err.to_string().contains("fee_bp"), "{err}");
    let loan = TermLoan::example_floating_with_ddtl().expect("ddtl loan");
    assert_rejects_at(
        &loan,
        "/ddtl",
        "commitment_limit",
        json!({"amount": "1", "currency": "USD"}),
    );
}

#[test]
// schema-rejection-test: CashFlowMeta `facility_limit`, FeeBase::Undrawn `facility_limit`
fn retired_facility_limit_keys_are_rejected() {
    use finstack_quant_cashflows::builder::{CashFlowMeta, FeeBase};
    use serde_json::json;
    assert_rejects_at(
        &CashFlowMeta::default(),
        "",
        "facility_limit",
        json!({"amount": "1", "currency": "USD"}),
    );
    let err = serde_json::from_value::<FeeBase>(
        json!({"undrawn": {"facility_limit": {"amount": "1", "currency": "USD"}}}),
    )
    .expect_err("facility_limit is retired");
    assert!(err.to_string().contains("facility_limit"), "{err}");
}

#[test]
// schema-rejection-test: RateSpec fixed `rate_bp`, TermLoan amortization `bp`, LoanCallType make_whole `treasury_spread_bp`
fn retired_term_loan_keys_are_rejected() {
    use finstack_quant_valuations::instruments::fixed_income::loan_terms::RateSpec;
    use finstack_quant_valuations::instruments::fixed_income::term_loan::{
        AmortizationSpec, LoanCallType,
    };
    use serde_json::json;
    let err = serde_json::from_value::<RateSpec>(json!({"fixed": {"rate_bp": 600}}))
        .expect_err("rate_bp is retired");
    assert!(err.to_string().contains("rate_bp"), "{err}");
    for (retired, old) in [
        (
            "percent_per_period",
            json!({"percent_per_period": {"bp": 250}}),
        ),
        (
            "percent_of_original_notional",
            json!({"percent_of_original_notional": {"bp": 100}}),
        ),
        (
            "linear",
            json!({"linear": {"start": "2025-01-01", "end": "2026-01-01"}}),
        ),
        ("custom", json!({"custom": []})),
        ("bp", json!({"percent_of_original_per_period": {"bp": 100}})),
    ] {
        let err = serde_json::from_value::<AmortizationSpec>(old)
            .expect_err("retired amortization spelling");
        assert!(err.to_string().contains(retired), "{retired}: {err}");
    }
    let err =
        serde_json::from_value::<LoanCallType>(json!({"make_whole": {"treasury_spread_bp": 50}}))
            .expect_err("treasury_spread_bp is retired");
    assert!(err.to_string().contains("treasury_spread_bp"), "{err}");
}

#[test]
// schema-rejection-test: AssetBackedFacility `unused_fee_bp`, `spread_bp`, `forward_curve_id`, `draw_schedule`; FacilityProjection `unused_fees`
fn retired_facility_keys_are_rejected() {
    use finstack_quant_valuations::instruments::AssetBackedFacility;
    use serde_json::json;
    let abf = AssetBackedFacility::example().expect("abf");
    assert_rejects_at(&abf, "", "unused_fee_bp", json!(50.0));
    assert_rejects_at(&abf, "", "spread_bp", json!(600.0));
    assert_rejects_at(&abf, "", "forward_curve_id", json!("USD-SOFR-3M"));
    assert_rejects_at(&abf, "", "draw_schedule", json!([]));
    let err = serde_json::from_value::<
        finstack_quant_valuations::instruments::fixed_income::asset_backed_facility::FacilityProjection,
    >(json!({"unused_fees": []}))
    .expect_err("unused_fees is retired");
    assert!(err.to_string().contains("unused_fees"), "{err}");
}

#[test]
// schema-rejection-test: UtilizationProcess mean_reverting `target_rate`, `speed`, `volatility`
fn retired_utilization_keys_are_rejected() {
    use finstack_quant_valuations::instruments::fixed_income::revolving_credit::UtilizationProcess;
    use serde_json::json;
    for retired in ["target_rate", "speed", "volatility"] {
        let mut process = json!({"mean_reverting": {"theta": 0.5, "kappa": 1.0, "sigma": 0.1}});
        process["mean_reverting"][retired] = json!(0.5);
        let err = serde_json::from_value::<UtilizationProcess>(process)
            .expect_err("retired utilization key");
        assert!(err.to_string().contains(retired), "{retired}: {err}");
    }
}

#[test]
// schema-rejection-test: ConvertibleBond `fixed_coupon`, `floating_coupon`; ConversionEvent price_trigger `threshold`, `lookback_days`
fn retired_convertible_keys_are_rejected() {
    use finstack_quant_valuations::instruments::fixed_income::convertible::ConversionEvent;
    use finstack_quant_valuations::instruments::ConvertibleBond;
    use serde_json::json;
    let cb = ConvertibleBond::example().expect("convertible");
    assert_rejects_at(&cb, "", "fixed_coupon", Value::Null);
    assert_rejects_at(&cb, "", "floating_coupon", Value::Null);
    for retired in ["threshold", "lookback_days"] {
        let mut event = json!({"price_trigger": {"threshold_pct": 130.0, "observation_days": 30, "required_days_above": 20}});
        event["price_trigger"][retired] = json!(20);
        let err =
            serde_json::from_value::<ConversionEvent>(event).expect_err("retired trigger key");
        assert!(err.to_string().contains(retired), "{retired}: {err}");
    }
}

/// `reduction_fee_bp` and the facility `commitment_fee_bp` default to zero
/// when omitted, as the retired `fee_bp` / `unused_fee_bp` did.
#[test]
fn omitted_contractual_fees_default_to_zero() {
    use finstack_quant_valuations::instruments::fixed_income::loan_terms::CommitmentStep;
    use finstack_quant_valuations::instruments::AssetBackedFacility;
    use serde_json::json;
    let step: CommitmentStep = serde_json::from_value(
        json!({"date": "2025-01-01", "amount": {"amount": "1", "currency": "USD"}}),
    )
    .expect("reduction_fee_bp is optional");
    assert!(step.reduction_fee_bp.is_zero());
    let mut abf =
        serde_json::to_value(AssetBackedFacility::example().expect("abf")).expect("serialize");
    abf.as_object_mut()
        .expect("object")
        .remove("commitment_fee_bp");
    let abf: AssetBackedFacility =
        serde_json::from_value(abf).expect("commitment_fee_bp is optional");
    assert!(abf.commitment_fee_bp.is_zero());
}
