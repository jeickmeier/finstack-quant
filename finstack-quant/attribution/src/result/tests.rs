use super::pnl::PnlAttribution;
use super::types::{AttributionMethod, PnlBucket};
use crate::{
    CarryDetail, CreditFactorAttribution, CrossFactorDetail, LevelPnl, RatesCurvesAttribution,
    SourceLine,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use indexmap::IndexMap;
use std::collections::BTreeMap;
use time::macros::date;

/// A new `Money` field on `PnlAttribution` must be listed as a bucket or
/// named here as a headline value, so it cannot vanish from reports.
#[test]
fn pct_of_total_by_bucket_lists_every_money_field_but_the_headline_values() {
    let mut pnl = PnlAttribution::new(
        Money::from((200_i64, Currency::USD)),
        "TEST",
        date!(2025 - 01 - 15),
        date!(2025 - 01 - 16),
        AttributionMethod::Parallel,
    );
    pnl.mark_to_market_pnl = Some(pnl.total_pnl);
    pnl.pv_t0 = Some(pnl.total_pnl);
    pnl.pv_t1 = Some(pnl.total_pnl);
    pnl.rates_curves_pnl = Money::from((-50_i64, Currency::USD));

    let wire = serde_json::to_value(&pnl).expect("serializes");
    let money_fields: Vec<&str> = wire
        .as_object()
        .expect("object")
        .iter()
        .filter(|(_, value)| serde_json::from_value::<Money>((*value).clone()).is_ok())
        .map(|(key, _)| key.as_str())
        .filter(|key| !["total_pnl", "mark_to_market_pnl", "pv_t0", "pv_t1"].contains(key))
        .collect();
    let buckets = pnl.pct_of_total_by_bucket();
    let mut listed: Vec<&str> = buckets.iter().map(|bucket| bucket.field.as_str()).collect();
    listed.sort_unstable();
    let mut expected = money_fields.clone();
    expected.sort_unstable();
    assert_eq!(listed, expected);

    let rates = &buckets[1];
    assert_eq!(rates.field, PnlBucket::RatesCurvesPnl);
    assert_eq!(rates.amount, pnl.rates_curves_pnl);
    assert_eq!(rates.pct_of_total, Some(-25.0));

    pnl.total_pnl = Money::from((0_i64, Currency::USD));
    assert!(
        pnl.pct_of_total_by_bucket()
            .iter()
            .all(|bucket| bucket.pct_of_total.is_none())
    );
}

#[test]
fn pct_of_total_is_signed_share_and_none_for_zero_total() {
    let mut pnl = PnlAttribution::new(
        Money::new(22_733.88, Currency::USD).expect("finite total"),
        "BOND",
        date!(2025 - 01 - 15),
        date!(2025 - 02 - 15),
        AttributionMethod::Parallel,
    );
    let total = pnl.total_pnl.amount();
    let carry = pnl.pct_of_total(25_112.0).expect("non-zero total");
    assert!((carry - 25_112.0 / total * 100.0).abs() < 1e-12);
    assert_eq!(pnl.pct_of_total(total), Some(100.0));
    pnl.total_pnl = Money::from((0_i64, Currency::USD));
    assert_eq!(pnl.pct_of_total(5.0), None);
    assert_eq!(pnl.pct_of_total(0.0), None);
}

fn attribution_with_all_details() -> serde_json::Value {
    let mut value = serde_json::to_value(PnlAttribution::new(
        Money::from((1_i64, Currency::USD)),
        "ALL-DETAILS",
        date!(2025 - 01 - 15),
        date!(2025 - 02 - 15),
        AttributionMethod::Parallel,
    ))
    .expect("attribution wire payload");
    let money = serde_json::json!({"amount": "1", "currency": "USD"});
    let source = serde_json::json!({
        "total": money, "rates_part": money, "credit_part": money,
    });
    let level = serde_json::json!({
        "level_name": "rating", "total": money, "by_bucket": {"IG": money},
    });
    let details = serde_json::json!({
        "carry_detail": {"total": money, "coupon_income": source,
            "pull_to_par": money, "roll_down": source, "funding_cost": money},
        "rates_detail": {"by_curve": {"USD-OIS": money},
            "by_tenor": {"USD-OIS|1Y": money}, "discount_total": money, "forward_total": money},
        "credit_detail": {"by_curve": {"ACME-HZD": money}, "by_tenor": {"ACME-HZD|1Y": money}},
        "inflation_detail": {"by_curve": {"US-CPI": money}, "by_tenor": {"US-CPI|1Y": money}},
        "correlations_detail": {"by_curve": {"CDX-BC": money}},
        "fx_detail": {"by_pair": {"EUR/USD": money}},
        "vol_detail": {"by_surface": {"SPX-VOL": money}},
        "cross_factor_detail": {"total": money, "by_pair": {"Rates×Credit": money}},
        "model_params_detail": {"prepayment": money, "default_rate": money,
            "recovery_rate": money, "conversion_ratio": money, "other": {"parameter": money}},
        "scalars_detail": {"dividends": {"SPX": money}, "inflation": {"US-CPI": money},
            "equity_prices": {"SPX": money}, "commodity_prices": {"OIL": money}},
        "credit_factor_detail": {"model_id": "test/0", "generic_pnl": money,
            "levels": [level], "adder_pnl_total": money, "curve_shape_pnl": money,
            "adder_pnl_by_issuer": {"ACME": money}, "adder_magnitude": money},
        "credit_carry_decomposition": {"model_id": "test/0", "rates_carry_total": money,
            "credit_carry_total": money, "credit_by_level": {"generic": money,
                "levels": [level], "adder_total": money, "adder_by_issuer": {"ACME": money}}},
    });
    value
        .as_object_mut()
        .expect("attribution object")
        .extend(details.as_object().expect("detail object").clone());
    value
}

// Traverse the public wire shape independently of the implementation's
// monetary visitor, so an omitted nested Money field fails these checks.
fn money_paths(value: &serde_json::Value, prefix: &str, paths: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(object) => {
            if object.contains_key("amount") && object.contains_key("currency") {
                paths.push(prefix.to_owned());
            } else {
                for (key, child) in object {
                    let key = key.replace('~', "~0").replace('/', "~1");
                    money_paths(child, &format!("{prefix}/{key}"), paths);
                }
            }
        }
        serde_json::Value::Array(array) => {
            for (index, child) in array.iter().enumerate() {
                money_paths(child, &format!("{prefix}/{index}"), paths);
            }
        }
        _ => {}
    }
}

#[test]
fn test_currency_validation_checks_every_money_leaf() {
    let payload = attribution_with_all_details();
    let attribution: PnlAttribution = serde_json::from_value(payload.clone()).unwrap();
    attribution
        .validate_currencies()
        .expect("all amounts use USD");
    let mut paths = Vec::new();
    money_paths(&payload, "", &mut paths);
    for path in paths {
        let mut mismatched = payload.clone();
        *mismatched.pointer_mut(&format!("{path}/currency")).unwrap() = serde_json::json!("EUR");
        let attribution: PnlAttribution = serde_json::from_value(mismatched).unwrap();
        assert!(attribution.validate_currencies().is_err(), "missed {path}");
        assert!(
            crate::long_rows::pnl_attribution_wide_row(&attribution).is_err(),
            "wide row accepted {path}"
        );
    }
}

#[test]
fn test_scale_rejects_invalid_scalars_without_mutation() {
    for factor in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e100] {
        let payload = attribution_with_all_details();
        let mut attribution: PnlAttribution = serde_json::from_value(payload.clone()).unwrap();
        assert!(attribution.scale(factor).is_err());
        assert_eq!(serde_json::to_value(attribution).unwrap(), payload);
    }
}

#[test]
fn test_scale_updates_every_money_leaf_and_preserves_currencies() {
    let payload = attribution_with_all_details();
    let mut paths = Vec::new();
    money_paths(&payload, "", &mut paths);
    for factor in [-2.0, 0.0, 0.5] {
        let mut attribution: PnlAttribution = serde_json::from_value(payload.clone()).unwrap();
        attribution.scale(factor).expect("representable scale");
        let scaled = serde_json::to_value(attribution).unwrap();
        for path in &paths {
            let amount = |value: &serde_json::Value| {
                value
                    .pointer(&format!("{path}/amount"))
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .parse::<f64>()
                    .unwrap()
            };
            let multiplier = if path == "/credit_factor_detail/adder_magnitude" {
                factor.abs()
            } else {
                factor
            };
            assert_eq!(
                amount(&scaled),
                amount(&payload) * multiplier,
                "missed {path}"
            );
            assert_eq!(
                scaled.pointer(&format!("{path}/currency")),
                payload.pointer(&format!("{path}/currency"))
            );
        }
        assert_eq!(scaled["meta"], payload["meta"]);
    }
}

#[test]
fn test_scale_is_atomic_when_any_money_leaf_overflows() {
    let payload = attribution_with_all_details();
    let mut paths = Vec::new();
    money_paths(&payload, "", &mut paths);
    for path in paths {
        let mut overflowing = payload.clone();
        *overflowing.pointer_mut(&format!("{path}/amount")).unwrap() =
            serde_json::json!(rust_decimal::Decimal::MAX.to_string());
        let mut attribution: PnlAttribution = serde_json::from_value(overflowing).unwrap();
        let original = serde_json::to_value(&attribution).unwrap();
        assert!(attribution.scale(2.0).is_err(), "missed {path}");
        assert_eq!(
            serde_json::to_value(attribution).unwrap(),
            original,
            "mutated {path}"
        );
    }
}

#[test]
fn test_metrics_based_defaults_include_bucketed_rates() {
    use finstack_quant_valuations::metrics::MetricId;
    let metrics = crate::default_attribution_metrics();
    assert!(metrics.contains(&MetricId::BucketedDv01));
    assert!(metrics.contains(&MetricId::BucketedCs01));
    assert_eq!(metrics, AttributionMethod::MetricsBased.required_metrics());
}

#[test]
fn test_carry_detail_scale_and_explain_include_decomposition_fields() {
    let mut attribution = PnlAttribution::new(
        Money::from((10_i64, Currency::USD)),
        "BOND-1",
        date!(2025 - 01 - 15),
        date!(2025 - 02 - 15),
        AttributionMethod::MetricsBased,
    );
    attribution.carry = Money::from((10_i64, Currency::USD));
    attribution.carry_detail = Some(CarryDetail {
        total: Money::from((10_i64, Currency::USD)),
        coupon_income: Some(SourceLine::scalar(Money::from((3_i64, Currency::USD)))),
        pull_to_par: Some(Money::from((4_i64, Currency::USD))),
        roll_down: Some(SourceLine::scalar(Money::from((5_i64, Currency::USD)))),
        funding_cost: Some(Money::from((2_i64, Currency::USD))),
    });

    attribution.scale(0.5).expect("finite representable scale");

    let detail = attribution.carry_detail.clone().expect("carry detail");
    assert_eq!(detail.total.amount(), 5.0);
    assert_eq!(
        detail
            .coupon_income
            .as_ref()
            .expect("coupon income")
            .total
            .amount(),
        1.5
    );
    assert_eq!(
        detail.pull_to_par.as_ref().expect("pull to par").amount(),
        2.0
    );
    assert_eq!(
        detail.roll_down.as_ref().expect("roll down").total.amount(),
        2.5
    );
    assert_eq!(
        detail.funding_cost.as_ref().expect("funding cost").amount(),
        1.0
    );

    attribution.carry_detail = Some(detail);
    let explanation = attribution.explain(true);
    assert!(explanation.contains("Coupon Income"));
    assert!(explanation.contains("Pull-to-Par"));
    assert!(explanation.contains("Roll-Down"));
    assert!(explanation.contains("Funding Cost"));
    assert!(!explanation.contains("Theta"));
}

#[test]
fn explain_renders_nonzero_factors_with_their_children() {
    let usd = |amount: i64| Money::from((amount, Currency::USD));
    let mut attribution = PnlAttribution::new(
        usd(100),
        "BOND-1",
        date!(2025 - 01 - 15),
        date!(2025 - 02 - 15),
        AttributionMethod::Parallel,
    );
    attribution.carry = usd(40);
    attribution.carry_detail = Some(CarryDetail {
        total: usd(40),
        coupon_income: Some(SourceLine::scalar(usd(30))),
        pull_to_par: None,
        roll_down: None,
        funding_cost: Some(usd(5)),
    });
    attribution.rates_curves_pnl = usd(50);
    attribution.rates_detail = Some(RatesCurvesAttribution {
        by_curve: IndexMap::from([(finstack_quant_core::types::CurveId::new("USD-OIS"), usd(50))]),
        by_tenor: IndexMap::new(),
        discount_total: usd(50),
        forward_total: usd(0),
    });
    attribution
        .compute_residual()
        .expect("same-currency residual");

    let expected = [
        format!("Total P&L: {}", usd(100)),
        format!("  ├─ Carry: {} (40.0%)", usd(40)),
        format!("  │   ├─ Coupon Income: {}", usd(30)),
        format!("  │   └─ Funding Cost: {}", usd(5)),
        format!("  ├─ Rates Curves: {} (50.0%)", usd(50)),
        format!("  │   ├─ USD-OIS: {}", usd(50)),
        format!("  └─ Residual: {} (10.0%)", usd(10)),
    ]
    .join("\n");
    assert_eq!(attribution.explain(false), expected);
    assert!(attribution.explain(true).contains("  ├─ Market Scalars: "));
}

#[test]
fn test_cross_factor_detail_serde_roundtrip() {
    let detail = CrossFactorDetail {
        total: Money::from((500_i64, Currency::USD)),
        by_pair: {
            let mut map = IndexMap::new();
            map.insert(
                "Rates×Credit".to_string(),
                Money::from((300_i64, Currency::USD)),
            );
            map.insert(
                "Spot×Vol".to_string(),
                Money::from((200_i64, Currency::USD)),
            );
            map
        },
    };

    let json = serde_json::to_string(&detail).unwrap();
    let parsed: CrossFactorDetail = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.total.amount(), 500.0);
    assert_eq!(parsed.by_pair.len(), 2);
}

#[test]
fn test_compute_residual_includes_cross_factor() {
    let total = Money::from((1000_i64, Currency::USD));
    let mut attr = PnlAttribution::new(
        total,
        "TEST",
        date!(2025 - 01 - 01),
        date!(2025 - 01 - 02),
        AttributionMethod::Parallel,
    );
    attr.rates_curves_pnl = Money::from((600_i64, Currency::USD));
    attr.credit_curves_pnl = Money::from((200_i64, Currency::USD));
    attr.cross_factor_pnl = Money::from((150_i64, Currency::USD));

    attr.compute_residual().unwrap();

    assert!((attr.residual.amount() - 50.0).abs() < 1e-10);
}

/// `mark_to_market_pnl` must capture the raw `val_t1 − val_t0`
/// supplied at construction time and stay frozen even if `total_pnl` is
/// later mutated by `apply_total_return_carry`. This is what lets a
/// consumer reconcile against their own computation of the price change.
#[test]
fn test_mark_to_market_pnl_captured_at_construction_and_preserved() {
    use crate::pnl_support::apply_total_return_carry;
    let raw_pnl = Money::from((1000_i64, Currency::USD));
    let mut attr = PnlAttribution::new(
        raw_pnl,
        "TEST-MTM",
        date!(2025 - 01 - 01),
        date!(2025 - 01 - 02),
        AttributionMethod::Parallel,
    );

    // Sanity: mark_to_market_pnl mirrors total_pnl at construction.
    assert_eq!(attr.mark_to_market_pnl.map(|m| m.amount()), Some(1000.0));

    // Simulate total-return adjustment (coupon income paid in period).
    let coupon = Money::from((50_i64, Currency::USD));
    let theta = Money::from((30_i64, Currency::USD));
    let carry_inputs = crate::pnl_support::TotalReturnCarryInputs {
        cash_paid: coupon,
        income_cash_paid: coupon,
        delta_accrued: None,
        flat_window_diff: None,
        funding_cost: None,
        num_repricings: 0,
        warnings: Vec::new(),
    };
    apply_total_return_carry(&mut attr, theta, carry_inputs).expect("carry add");

    assert_eq!(attr.total_pnl.amount(), 1050.0);
    assert_eq!(
        attr.mark_to_market_pnl.map(|m| m.amount()),
        Some(1000.0),
        "mark_to_market_pnl must not absorb coupon adjustment"
    );
}

/// When compute_residual fails because a factor is in a
/// different currency from total_pnl, the attribution must report itself
/// as invalid, and `residual_within_tolerance` must refuse to claim
/// success even though `residual == 0` was set by the failure path.
#[test]
fn test_failed_residual_marks_invalid_and_blocks_tolerance_check() {
    let total = Money::from((1000_i64, Currency::USD));
    let mut attr = PnlAttribution::new(
        total,
        "TEST",
        date!(2025 - 01 - 01),
        date!(2025 - 01 - 02),
        AttributionMethod::Parallel,
    );
    // Deliberately mismatched currency so validate_currencies errors out.
    attr.rates_curves_pnl = Money::from((600_i64, Currency::EUR));

    let err = attr.compute_residual();
    assert!(err.is_err(), "currency mismatch must error");
    assert!(attr.result_invalid, "result_invalid must be set on failure");
    // residual was reset to zero — without the result_invalid guard the
    // tolerance check below would falsely succeed.
    assert_eq!(attr.residual.amount(), 0.0);
    assert!(
        !attr.residual_within_tolerance(Some(0.1), Some(1.0)),
        "tolerance check MUST fail when result_invalid is set, regardless of residual value"
    );
    assert!(
        !attr.residual_within_tolerance(None, None),
        "stored-tolerance check MUST also fail when result_invalid is set"
    );
}

/// Scaling must include every `credit_factor_detail` component, including
/// `curve_shape_pnl` and the `adder_magnitude` diagnostic, while preserving
/// `generic + Σlevels + adder + curve_shape ≡ credit_curves_pnl`.
#[test]
fn test_scale_preserves_credit_factor_detail_reconciliation() {
    let usd = Currency::USD;
    let mut attr = PnlAttribution::new(
        Money::from((100_i64, usd)),
        "BOND-SCALE",
        date!(2025 - 01 - 15),
        date!(2025 - 02 - 15),
        AttributionMethod::Parallel,
    );
    attr.credit_curves_pnl = Money::from((100_i64, usd));
    attr.credit_factor_detail = Some(CreditFactorAttribution {
        model_id: "test/0".to_string(),
        generic_pnl: Money::from((40_i64, usd)),
        levels: vec![LevelPnl {
            level_name: "rating".to_string(),
            total: Money::from((30_i64, usd)),
            by_bucket: BTreeMap::new(),
        }],
        adder_pnl_total: Money::from((20_i64, usd)),
        curve_shape_pnl: Money::from((10_i64, usd)),
        adder_pnl_by_issuer: None,
        adder_magnitude: Some(Money::from((20_i64, usd))),
    });

    let reconciles = |a: &PnlAttribution| {
        let d = a.credit_factor_detail.as_ref().expect("detail");
        let sum = d.generic_pnl.amount()
            + d.levels.iter().map(|l| l.total.amount()).sum::<f64>()
            + d.adder_pnl_total.amount()
            + d.curve_shape_pnl.amount();
        (sum - a.credit_curves_pnl.amount()).abs() < 1e-9
    };
    assert!(reconciles(&attr), "fixture must reconcile before scaling");

    attr.scale(0.5).expect("finite representable scale");
    let d = attr.credit_factor_detail.as_ref().expect("detail");
    assert_eq!(
        d.curve_shape_pnl.amount(),
        5.0,
        "curve_shape_pnl must scale"
    );
    assert_eq!(
        d.adder_magnitude.map(|m| m.amount()),
        Some(10.0),
        "adder_magnitude must scale"
    );
    assert!(
        reconciles(&attr),
        "reconciliation invariant must survive scaling"
    );

    // A negative scale (short position) must keep adder_magnitude non-negative.
    attr.scale(-1.0).expect("finite representable scale");
    assert_eq!(
        attr.credit_factor_detail
            .as_ref()
            .and_then(|d| d.adder_magnitude)
            .map(|m| m.amount()),
        Some(10.0),
        "adder_magnitude is a |.| diagnostic — stays non-negative under negative scale"
    );
    assert!(
        reconciles(&attr),
        "reconciliation survives a negative scale"
    );
}
