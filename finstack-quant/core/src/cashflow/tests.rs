use super::*;
use crate::currency::Currency;
use crate::dates::{Date, DayCount, DayCountContextState};
use crate::money::Money;
use core::mem::size_of;
use time::Month;

fn assert_roundtrip<T>(value: T)
where
    T: Clone + std::fmt::Display + std::str::FromStr + PartialEq,
{
    let label = value.to_string();
    assert!(
        matches!(label.parse::<T>(), Ok(parsed) if parsed == value),
        "roundtrip failed for {label}"
    );
}

#[test]
fn cashflow_size_is_reasonable() {
    let size = size_of::<CashFlow>();
    assert!(size <= 168, "CashFlow grew to {size} bytes");
}

#[test]
fn cashflow_json_rejects_non_finite_rate_metadata() {
    let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid start");
    let end = Date::from_calendar_date(2025, Month::July, 1).expect("valid end");
    for rate in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(
            CashFlowParams::new(
                end,
                None,
                Money::from((25_i64, Currency::USD)),
                CFKind::Fixed,
                0.5,
                Some(rate),
            )
            .build()
            .is_err()
        );
        let mut flow = CashFlowParams::new(
            end,
            None,
            Money::from((25_i64, Currency::USD)),
            CFKind::Fixed,
            0.5,
            Some(0.05),
        )
        .build()
        .expect("valid cashflow");
        // Bypass the constructor to prove validate and serialize still reject.
        flow.rate = Some(rate);
        assert!(flow.validate().is_err());
        assert!(serde_json::to_string(&flow).is_err());
        assert!(serde_json::to_value(&flow).is_err());

        flow.rate = Some(0.05);
        let accrual = CashFlowAccrual {
            start,
            end,
            day_count: DayCount::Act360,
            projected_index_rate: Some(rate),
            context: DayCountContextState::default(),
            nominal_dates: None,
        };
        assert!(serde_json::to_string(&accrual).is_err());
        flow.accrual = Some(accrual);
        assert!(flow.validate().is_err());
        assert!(serde_json::to_string(&flow).is_err());
        assert!(serde_json::to_value(&flow).is_err());
    }
}

#[test]
fn deserialization_validates_rows_and_accrual_slices() {
    // Record CORE-CONVENTIONS-015 #10: no wire route creates an invalid row.
    let settlement_with_accrual = r#"{"date":"2025-07-01","amount":{"amount":"5","currency":"USD"},
        "kind":"derivative_settlement","accrual_factor":0.25}"#;
    assert!(serde_json::from_str::<CashFlow>(settlement_with_accrual).is_err());
    let settlement = serde_json::json!({
        "date": "2025-07-01", "amount": {"amount": "5", "currency": "USD"},
        "kind": "derivative_settlement", "accrual_factor": 0.0
    });
    assert!(serde_json::from_value::<CashFlow>(settlement.clone()).is_ok());
    let mut with_period = settlement;
    with_period["accrual"] = serde_json::json!({
        "start": "2025-01-01", "end": "2025-07-01", "day_count": "act_360",
        "context": {"calendar_id":null,"frequency":null,"bus_basis":null,
            "coupon_period":null,"end_is_termination_date":false}
    });
    assert!(
        serde_json::from_value::<CashFlow>(with_period)
            .unwrap_err()
            .to_string()
            .contains("settlement cannot carry coupon accrual")
    );
    let pik_with_cash = r#"{"date":"2025-07-01","amount":{"amount":"5","currency":"USD"},
        "kind":"pik","accrual_factor":0.0,
        "principal_delta":{"amount":"5","currency":"USD"},"principal_date":"2025-07-01"}"#;
    assert!(serde_json::from_str::<CashFlow>(pik_with_cash).is_err());
    let amortization_without_delta = r#"{"date":"2025-07-01",
        "amount":{"amount":"5","currency":"USD"},"kind":"amortization","accrual_factor":0.0}"#;
    assert!(serde_json::from_str::<CashFlow>(amortization_without_delta).is_err());
    let inverted_accrual = r#"{"start":"2025-07-01","end":"2025-01-01","day_count":"act_360",
        "context":{"calendar_id":null,"frequency":null,"bus_basis":null,
        "coupon_period":null,"end_is_termination_date":false}}"#;
    assert!(serde_json::from_str::<CashFlowAccrual>(inverted_accrual).is_err());
    let valid = r#"{"date":"2025-07-01","amount":{"amount":"0","currency":"USD"},
        "kind":"pik","accrual_factor":0.0,
        "principal_delta":{"amount":"5","currency":"USD"},"principal_date":"2025-07-01"}"#;
    assert!(serde_json::from_str::<CashFlow>(valid).is_ok());
}

#[test]
fn cashflow_json_preserves_absent_and_finite_negative_rates() {
    let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid start");
    let end = Date::from_calendar_date(2025, Month::July, 1).expect("valid end");
    for rate in [None, Some(-0.05), Some(0.0), Some(0.05)] {
        let flow = CashFlowParams::new(
            end,
            None,
            Money::from((-25_i64, Currency::USD)),
            CFKind::Fixed,
            0.5,
            rate,
        )
        .with_accrual(CashFlowAccrual {
            start,
            end,
            day_count: DayCount::Act360,
            projected_index_rate: rate,
            context: DayCountContextState::default(),
            nominal_dates: None,
        })
        .build()
        .expect("valid cashflow");
        let json = serde_json::to_value(&flow).expect("finite rates serialize");
        if rate.is_none() {
            assert!(json.get("rate").is_none());
            assert!(json["accrual"].get("projected_index_rate").is_none());
        }
        let restored: CashFlow = serde_json::from_value(json).expect("valid cashflow JSON");
        assert_eq!(restored, flow);
        assert!(restored.validate().is_ok());
    }
}

#[test]
fn cashflow_validation_works() {
    let date = Date::from_calendar_date(2025, Month::January, 15).expect("Valid test date");
    let amount = Money::from((100_i64, Currency::USD));

    let cf = CashFlow {
        date,
        reset_date: None,
        amount,
        kind: CFKind::Fixed,
        accrual_factor: 0.0,
        rate: None,
        accrual: None,
        principal_delta: None,
        principal_date: None,
    };
    assert_eq!(cf.date, date);
    assert_eq!(cf.amount, amount);
    assert_eq!(cf.kind, CFKind::Fixed);
    assert!(cf.reset_date.is_none());
    assert_eq!(cf.accrual_factor, 0.0);
    assert!(cf.validate().is_ok());
}

#[test]
fn cashflow_rejects_unknown_json_fields() {
    let json = serde_json::json!({
        "date": Date::from_calendar_date(2025, Month::January, 15).expect("valid date"),
        "reset_date": null,
        "amount": Money::from((100_i64, Currency::USD)),
        "kind": "fixed",
        "accrual_factor": 0.25,
        "rate": 0.05,
        "extra": true
    });

    let err = serde_json::from_value::<CashFlow>(json)
        .expect_err("unknown cashflow fields must be rejected");
    assert!(err.to_string().contains("extra"));
}

#[test]
fn cashflow_kinds_construct_correctly() {
    let date = Date::from_calendar_date(2025, Month::March, 1).expect("Valid test date");
    let amt = Money::from((1_000_i64, Currency::EUR));

    let princ = CashFlow {
        date,
        reset_date: None,
        amount: amt,
        kind: CFKind::Notional,
        accrual_factor: 0.0,
        rate: None,
        accrual: None,
        principal_delta: Some(amt.checked_neg()),
        principal_date: Some(date),
    };
    assert_eq!(princ.kind, CFKind::Notional);
    assert!(princ.validate().is_ok());

    let fee = CashFlow {
        date,
        reset_date: None,
        amount: amt,
        kind: CFKind::Fee,
        accrual_factor: 0.0,
        rate: None,
        accrual: None,
        principal_delta: None,
        principal_date: None,
    };
    assert_eq!(fee.kind, CFKind::Fee);
    assert!(fee.validate().is_ok());

    // PIK is a noncash balance event: zero cash, explicit capitalization.
    let pik = CashFlow {
        date,
        reset_date: None,
        amount: Money::zero(amt.currency()),
        kind: CFKind::Pik,
        accrual_factor: 0.0,
        rate: None,
        accrual: None,
        principal_delta: Some(amt),
        principal_date: Some(date),
    };
    assert_eq!(pik.kind, CFKind::Pik);
    assert!(pik.validate().is_ok());

    let amort = CashFlow {
        date,
        reset_date: None,
        amount: amt,
        kind: CFKind::Amortization,
        accrual_factor: 0.0,
        rate: None,
        accrual: None,
        principal_delta: Some(amt.checked_neg()),
        principal_date: Some(date),
    };
    assert_eq!(amort.kind, CFKind::Amortization);
    assert!(amort.validate().is_ok());

    // Zero amounts validate (floored coupons are legitimate).
    let zero = Money::from((0_i64, Currency::EUR));
    let zero_cf = CashFlow {
        date,
        reset_date: None,
        amount: zero,
        kind: CFKind::Fixed,
        accrual_factor: 0.0,
        rate: None,
        accrual: None,
        principal_delta: None,
        principal_date: None,
    };
    assert!(zero_cf.validate().is_ok());
}

#[test]
fn cfkind_labels_retire_float_reset_and_classify_protection_payment() {
    // Record CORE-CONVENTIONS-015 #2: `float_reset` is retired without an
    // alias; protection cash is its own cash-capable kind.
    assert!("float_reset".parse::<CFKind>().is_err());
    assert!(serde_json::from_str::<CFKind>("\"float_reset\"").is_err());
    assert_eq!("floating".parse::<CFKind>().unwrap(), CFKind::Floating);
    assert_eq!(
        serde_json::to_string(&CFKind::ProtectionPayment).unwrap(),
        "\"protection_payment\""
    );
    let protection = CFKind::ProtectionPayment;
    assert!(protection.is_cash_settlement());
    assert!(!protection.is_coupon() && !protection.is_principal_like());
    assert!(!protection.is_repayment_like());
}

#[test]
fn cfkind_is_coupon_classifies_coupon_flows_only() {
    for kind in [
        CFKind::Fixed,
        CFKind::Floating,
        CFKind::InflationCoupon,
        CFKind::Stub,
    ] {
        assert!(kind.is_coupon(), "{kind:?} should be a coupon");
    }

    for kind in [
        CFKind::Fee,
        CFKind::Notional,
        CFKind::Pik,
        CFKind::Amortization,
        CFKind::Recovery,
        CFKind::MarginInterest,
    ] {
        assert!(!kind.is_coupon(), "{kind:?} should not be interest-like");
    }
}

#[test]
fn cfkind_is_principal_like_classifies_balance_changing_flows() {
    for kind in [
        CFKind::Notional,
        CFKind::Pik,
        CFKind::Amortization,
        CFKind::PrePayment,
        CFKind::RevolvingDraw,
        CFKind::RevolvingRepayment,
        CFKind::DefaultedNotional,
    ] {
        assert!(
            kind.is_principal_like(),
            "{kind:?} should be principal-like"
        );
    }

    for kind in [
        CFKind::Fixed,
        CFKind::Fee,
        CFKind::Recovery,
        CFKind::MarginInterest,
    ] {
        assert!(
            !kind.is_principal_like(),
            "{kind:?} should not be principal-like"
        );
    }
}

#[test]
fn cfkind_fromstr_accepts_canonical_labels_and_mixed_case() {
    assert!("coupon".parse::<CFKind>().is_err());
    assert!("principal".parse::<CFKind>().is_err());
    assert_eq!(
        " Fixed ".parse::<CFKind>().expect("canonical label"),
        CFKind::Fixed
    );
    assert_eq!(
        "Notional".parse::<CFKind>().expect("mixed case"),
        CFKind::Notional
    );
    assert!("not-a-kind".parse::<CFKind>().is_err());
    // Display and serde keep the canonical label.
    assert_eq!(CFKind::Fixed.to_string(), "fixed");
    assert_eq!(CFKind::Notional.to_string(), "notional");
}

#[test]
fn margin_cashflow_kinds_construct_correctly() {
    let date = Date::from_calendar_date(2025, Month::March, 1).expect("Valid test date");
    let amt = Money::from((1_000_000_i64, Currency::USD));

    // Initial margin posting
    let im_post = CashFlow {
        date,
        reset_date: None,
        amount: amt,
        kind: CFKind::InitialMarginPost,
        accrual_factor: 0.0,
        rate: None,
        accrual: None,
        principal_delta: None,
        principal_date: None,
    };
    assert_eq!(im_post.kind, CFKind::InitialMarginPost);
    assert!(im_post.validate().is_ok());

    // Initial margin return
    let im_return = CashFlow {
        date,
        reset_date: None,
        amount: amt,
        kind: CFKind::InitialMarginReturn,
        accrual_factor: 0.0,
        rate: None,
        accrual: None,
        principal_delta: None,
        principal_date: None,
    };
    assert_eq!(im_return.kind, CFKind::InitialMarginReturn);
    assert!(im_return.validate().is_ok());

    // Variation margin received
    let vm_receive = CashFlow {
        date,
        reset_date: None,
        amount: amt,
        kind: CFKind::VariationMarginReceive,
        accrual_factor: 0.0,
        rate: None,
        accrual: None,
        principal_delta: None,
        principal_date: None,
    };
    assert_eq!(vm_receive.kind, CFKind::VariationMarginReceive);
    assert!(vm_receive.validate().is_ok());

    // Variation margin paid
    let vm_pay = CashFlow {
        date,
        reset_date: None,
        amount: amt,
        kind: CFKind::VariationMarginPay,
        accrual_factor: 0.0,
        rate: None,
        accrual: None,
        principal_delta: None,
        principal_date: None,
    };
    assert_eq!(vm_pay.kind, CFKind::VariationMarginPay);
    assert!(vm_pay.validate().is_ok());

    // Margin interest
    let margin_int = CashFlow {
        date,
        reset_date: None,
        amount: Money::from((5_000_i64, Currency::USD)),
        kind: CFKind::MarginInterest,
        accrual_factor: 0.25,
        rate: Some(0.05),
        accrual: None,
        principal_delta: None,
        principal_date: None,
    };
    assert_eq!(margin_int.kind, CFKind::MarginInterest);
    assert!(margin_int.validate().is_ok());

    // Collateral substitution in
    let sub_in = CashFlow {
        date,
        reset_date: None,
        amount: amt,
        kind: CFKind::CollateralSubstitutionIn,
        accrual_factor: 0.0,
        rate: None,
        accrual: None,
        principal_delta: None,
        principal_date: None,
    };
    assert_eq!(sub_in.kind, CFKind::CollateralSubstitutionIn);
    assert!(sub_in.validate().is_ok());

    // Collateral substitution out
    let sub_out = CashFlow {
        date,
        reset_date: None,
        amount: amt,
        kind: CFKind::CollateralSubstitutionOut,
        accrual_factor: 0.0,
        rate: None,
        accrual: None,
        principal_delta: None,
        principal_date: None,
    };
    assert_eq!(sub_out.kind, CFKind::CollateralSubstitutionOut);
    assert!(sub_out.validate().is_ok());
}

// CFKind FromStr / Display roundtrip tests

#[test]
fn cfkind_display_roundtrip() {
    let all = [
        CFKind::Fixed,
        CFKind::Floating,
        CFKind::InflationCoupon,
        CFKind::Fee,
        CFKind::CommitmentFee,
        CFKind::UsageFee,
        CFKind::FacilityFee,
        CFKind::LcFee,
        CFKind::FrontingFee,
        CFKind::Notional,
        CFKind::Pik,
        CFKind::Amortization,
        CFKind::PrePayment,
        CFKind::RevolvingDraw,
        CFKind::RevolvingRepayment,
        CFKind::DefaultedNotional,
        CFKind::Recovery,
        CFKind::AccruedOnDefault,
        CFKind::Stub,
        CFKind::InitialMarginPost,
        CFKind::InitialMarginReturn,
        CFKind::VariationMarginReceive,
        CFKind::VariationMarginPay,
        CFKind::MarginInterest,
        CFKind::CollateralSubstitutionIn,
        CFKind::CollateralSubstitutionOut,
    ];

    for kind in &all {
        assert_roundtrip(*kind);
        assert_eq!(
            serde_json::to_value(kind).expect("CFKind serializes"),
            serde_json::Value::String(kind.to_string()),
            "Display and serde labels must agree for {kind:?}"
        );
    }
}

#[test]
fn cfkind_from_str_rejects_noncanonical_spellings() {
    // Only surrounding whitespace and ASCII case are normalized.
    // The wire label is `pre_payment`, never `prepayment`.
    assert_eq!(
        "FIXED".parse::<CFKind>().expect("case-folded"),
        CFKind::Fixed
    );
    for rejected in [
        "amort",
        "prepayment",
        "Float-Reset",
        "float reset",
        "principal",
        "coupon",
    ] {
        assert!(rejected.parse::<CFKind>().is_err(), "{rejected:?}");
    }
}

#[test]
fn cfkind_from_str_unknown() {
    assert!("garbage".parse::<CFKind>().is_err());
}
