//! The credit family shares one wire vocabulary: the CDS running coupon is
//! `coupon_bp` (on `premium` for CDS/CDS index, at the root for tranches and
//! CDS options), the roll grid is `roll_rule` (`cds_imm` or `none`), settled
//! index loss is `realized_loss`, the contractual upfront is the dated
//! `upfront` field and structured-credit tranche bounds are
//! `attach_pct`/`detach_pct`. Each retired spelling is rejected by
//! `deny_unknown_fields`, and the new selectors behave as documented.

use finstack_quant_cashflows::builder::specs::RollRule;
use finstack_quant_cashflows::builder::ScheduleParams;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::StubKind;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::{
    CdsOptionParams, CdsOptionStrike,
};
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CdsTrancheParams;
use finstack_quant_valuations::instruments::PayReceive;
use finstack_quant_valuations::instruments::{
    CdsOption, CdsTranche, CreditDefaultSwap, CreditParams, StructuredCredit,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use time::macros::date;

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

#[test]
// schema-rejection-test: PremiumLegSpec `spread_bp`, `standard_imm_dates`
fn cds_premium_retired_coupon_and_imm_keys_are_rejected() {
    let cds = CreditDefaultSwap::example();
    let mut json = serde_json::to_value(&cds).expect("serialize");
    let premium = json["premium_leg"]
        .as_object_mut()
        .expect("premium_leg object");
    let coupon = premium.remove("coupon_bp").expect("coupon_bp present");
    premium.insert("spread_bp".to_string(), coupon);
    let err = serde_json::from_value::<CreditDefaultSwap>(json).expect_err("spread_bp retired");
    assert!(err.to_string().contains("spread_bp"), "{err}");

    assert_rejects_at(
        &cds,
        "/premium_leg",
        "standard_imm_dates",
        serde_json::json!(true),
    );
}

#[test]
// schema-rejection-test: MarketQuoteOverrides `upfront_payment`
fn market_quote_upfront_payment_is_rejected() {
    let cds = CreditDefaultSwap::example();
    let mut json = serde_json::to_value(&cds).expect("serialize");
    json["instrument_pricing_overrides"] = serde_json::json!({
        "market_quotes": {"upfront_payment": {"amount": "25000", "currency": "USD"}}
    });
    let err =
        serde_json::from_value::<CreditDefaultSwap>(json).expect_err("upfront_payment retired");
    assert!(err.to_string().contains("upfront_payment"), "{err}");
}

#[test]
// schema-rejection-test: CdsTranche root `running_coupon_bp`, `accumulated_loss`, `standard_imm_dates`
fn cds_tranche_retired_keys_are_rejected() {
    let tranche = CdsTranche::example();
    assert_rejects_at(&tranche, "", "running_coupon_bp", serde_json::json!(100.0));
    assert_rejects_at(&tranche, "", "accumulated_loss", serde_json::json!(0.0));
    assert_rejects_at(&tranche, "", "standard_imm_dates", serde_json::json!(true));
}

#[test]
// schema-rejection-test: CdsOption root `underlying_cds_coupon`, `realized_index_loss`
fn cds_option_retired_keys_are_rejected() {
    let option = CdsOption::example().expect("example");
    assert_rejects_at(
        &option,
        "",
        "underlying_cds_coupon",
        serde_json::json!("0.01"),
    );
    assert_rejects_at(&option, "", "realized_index_loss", serde_json::json!(0.0));
}

#[test]
// schema-rejection-test: structured-credit Tranche `attachment_point`, `detachment_point`
fn structured_credit_tranche_retired_bounds_are_rejected() {
    let deal = StructuredCredit::example();
    assert_rejects_at(
        &deal,
        "/tranches/tranches/0",
        "attachment_point",
        serde_json::json!(0.0),
    );
    assert_rejects_at(
        &deal,
        "/tranches/tranches/0",
        "detachment_point",
        serde_json::json!(10.0),
    );
}

#[test]
fn cds_premium_rejects_futures_imm_roll_rule() {
    let mut cds = CreditDefaultSwap::example();
    cds.premium_leg.roll_rule = RollRule::Imm;
    let err = cds
        .isda_coupon_schedule()
        .expect_err("imm roll grid is not a CDS grid");
    assert!(err.to_string().contains("premium.roll_rule"), "{err}");
}

#[test]
fn cds_tranche_new_carries_schedule_stub_and_roll_rule() {
    let params = CdsTrancheParams::equity_tranche(
        "CDX.NA.IG",
        42,
        Money::new(10_000_000.0, Currency::USD).expect("money"),
        date!(2029 - 12 - 20),
        100.0,
    );
    let mut schedule = ScheduleParams::quarterly_act360();
    schedule.stub = StubKind::LongFront;
    let tranche = CdsTranche::new(
        "TR-STUB",
        &params,
        &schedule,
        "USD-OIS",
        "CDX.NA.IG.HAZARD",
        PayReceive::Pay,
    )
    .expect("tranche");
    assert_eq!(tranche.stub, StubKind::LongFront);
    assert_eq!(tranche.roll_rule, RollRule::None);

    let standard = CdsTranche::example();
    assert_eq!(standard.roll_rule, RollRule::CdsImm);

    let mut imm = standard;
    imm.roll_rule = RollRule::Imm;
    let err = imm.validate().expect_err("imm roll grid is rejected");
    assert!(err.to_string().contains("roll_rule"), "{err}");
}

#[test]
fn cds_option_accepts_zero_recovery_like_every_credit_instrument() {
    let option_params = CdsOptionParams::call(
        CdsOptionStrike::Spread(rust_decimal::Decimal::new(1, 2)),
        date!(2025 - 06 - 20),
        date!(2030 - 06 - 20),
        Money::new(10_000_000.0, Currency::USD).expect("money"),
    )
    .expect("option params");
    let credit_params = CreditParams::new("ACME", 0.0, "ACME-HAZARD");
    let option = CdsOption::new(
        "CDSO-ZERO-R",
        &option_params,
        &credit_params,
        "USD-OIS",
        "CDSO-VOL",
    )
    .expect("zero recovery is inside the shared [0, 1) domain");
    assert_eq!(option.recovery_rate, 0.0);
    assert_eq!(option.index_factor, 1.0);
    assert_eq!(option.realized_loss, 0.0);

    let one = CreditParams::new("ACME", 1.0, "ACME-HAZARD");
    assert!(
        CdsOption::new("CDSO-FULL-R", &option_params, &one, "USD-OIS", "CDSO-VOL").is_err(),
        "full recovery leaves no protection and is rejected"
    );
}

#[test]
fn cds_tranche_side_uses_pay_receive_like_cds() {
    // A tranche buyer pays the running premium, exactly like a CDS/CDS-index
    // `pay` side: `pay` buys protection, `receive` sells it.
    let tranche = CdsTranche::example();
    assert_eq!(tranche.side, PayReceive::Pay);
    let json = serde_json::to_value(&tranche).expect("serialize");
    assert_eq!(json["side"], "pay");

    for retired in [
        "buy_protection",  // schema-rejection-test: now `pay`
        "sell_protection", // schema-rejection-test: now `receive`
    ] {
        let mut value = json.clone();
        value["side"] = serde_json::json!(retired);
        assert!(
            serde_json::from_value::<CdsTranche>(value).is_err(),
            "retired tranche side {retired} must be rejected"
        );
    }
}
