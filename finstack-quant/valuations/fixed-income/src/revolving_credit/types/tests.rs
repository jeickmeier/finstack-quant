use super::*;
use finstack_quant_cashflows::builder::RateSpec;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use time::macros::date;

#[test]
fn date_offset_rejects_wrapping_settlement_days() {
    let mut facility = RevolvingCredit::example().expect("example facility");
    facility.settlement_days = u32::MAX;
    for calendar in [None, Some("weekends_only".into())] {
        facility.calendar_id = calendar;
        assert!(facility.settlement_date(date!(2025 - 01 - 01)).is_err());
    }
}

#[test]
fn hull_white_process_uses_canonical_acronym_spelling() {
    let process = InterestRateProcessSpec::HullWhite1F {
        hw1f_mean_reversion: 0.1,
        hw1f_sigma: 0.01,
        initial: 0.03,
        theta: 0.03,
    };
    let value = serde_json::to_value(process).expect("serialize Hull-White process");
    assert!(value.get("hull_white_1f").is_some());

    assert!(
        serde_json::from_value::<InterestRateProcessSpec>(serde_json::json!({
            "hull_white_1f": {
                // schema-rejection-test
                "kappa": 0.1,
                "sigma": 0.01,
                "initial": 0.03,
                "theta": 0.03
            }
        }))
        .is_err(),
        "retired kappa/sigma keys must not parse"
    );

    // schema-rejection-test
    assert!(
        serde_json::from_value::<InterestRateProcessSpec>(serde_json::json!({
            "hull_white1_f": {
                "kappa": 0.1,
                "sigma": 0.01,
                "initial": 0.03,
                "theta": 0.03
            }
        }))
        .is_err()
    );
}

#[test]
fn floating_revolver_uses_the_canonical_fixing_series_id() {
    let facility = RevolvingCredit::example().expect("floating example");
    let RateSpec::Floating(spec) = &facility.rate else {
        unreachable!("example must use a floating base rate");
    };
    let expected =
        finstack_quant_core::market_data::fixings::fixing_series_id(spec.index_id.as_str());

    let deps =
        finstack_quant_valuations_core::instruments::Instrument::market_dependencies(&facility)
            .expect("dependencies");
    assert_eq!(deps.series_ids, vec![expected]);
}

#[test]
fn deterministic_events_are_validated_and_replayed_chronologically() {
    let mut facility = RevolvingCredit::example().expect("example");
    facility.draw_repay_spec = DrawRepaySpec::Deterministic(vec![
        DrawRepayEvent {
            date: date!(2025 - 06 - 01),
            amount: Money::from((20_000_000_i64, Currency::USD)),
            is_draw: false,
        },
        DrawRepayEvent {
            date: date!(2025 - 01 - 01),
            amount: Money::from((20_000_000_i64, Currency::USD)),
            is_draw: true,
        },
    ]);

    facility
        .validate()
        .expect("chronologically valid events must not depend on input order");
    let balance = crate::revolving_credit::cashflow_engine::calculate_drawn_balance_at_date(
        &facility,
        facility.issue_date,
        date!(2025 - 12 - 31),
    )
    .expect("balance");
    assert_eq!(balance.amount(), 10_000_000.0);
}

#[test]
fn validation_rejects_events_outside_life() {
    let mut facility = RevolvingCredit::example().expect("example");
    facility.draw_repay_spec = DrawRepaySpec::Deterministic(vec![DrawRepayEvent {
        date: date!(2028 - 01 - 01),
        amount: Money::from((1_000_000_i64, Currency::USD)),
        is_draw: true,
    }]);
    assert!(
        facility
            .validate()
            .expect_err("post-maturity draw must fail")
            .to_string()
            .contains("maturity")
    );
}

#[test]
fn validation_rejects_invalid_floating_rate_economics() {
    let mut facility = RevolvingCredit::example().expect("example");
    let RateSpec::Floating(spec) = &mut facility.rate else {
        unreachable!("example must be floating");
    };
    spec.gearing = rust_decimal::Decimal::ZERO;

    let err = facility
        .validate()
        .expect_err("non-positive floating gearing must fail validation");
    assert!(
        err.to_string().contains("gearing"),
        "unexpected validation error: {err}"
    );
}

#[test]
fn h1_and_h24_serde_rejects_invalid_facility_fees_and_non_finite_mc_config() {
    let facility = RevolvingCredit::example().expect("example");
    let mut json = serde_json::to_value(&facility).expect("serialize facility");
    json["recovery_rate"] = serde_json::json!(1.5);
    assert!(
        serde_json::from_value::<RevolvingCredit>(json).is_err(),
        "RevolvingCredit serde must reject out-of-range recovery_rate"
    );

    let mut bad_fees_json =
        serde_json::to_value(RevolvingCreditFees::default()).expect("serialize fees");
    bad_fees_json["facility_fee_bp"] = serde_json::json!(-10.0);
    assert!(
        serde_json::from_value::<RevolvingCreditFees>(bad_fees_json).is_err(),
        "RevolvingCreditFees serde must reject negative facility_fee_bp"
    );

    let bad_mc_json = serde_json::json!({
        "credit_spread_process": {
            "cir": {
                "kappa": 0.0,
                "theta": 0.02,
                "sigma": 0.01,
                "initial": 0.02
            }
        }
    });
    assert!(
        serde_json::from_value::<McConfig>(bad_mc_json).is_err(),
        "McConfig serde must reject non-positive CIR kappa"
    );

    let non_finite_cases = [
        McConfig {
            credit_spread_process: CreditSpreadProcessSpec::Constant(f64::INFINITY),
            interest_rate_process: None,
            correlation_matrix: None,
            util_credit_corr: None,
        },
        McConfig {
            credit_spread_process: CreditSpreadProcessSpec::Cir {
                kappa: 0.5,
                theta: f64::INFINITY,
                sigma: 0.01,
                initial: 0.02,
            },
            interest_rate_process: None,
            correlation_matrix: None,
            util_credit_corr: None,
        },
        McConfig {
            credit_spread_process: CreditSpreadProcessSpec::MarketAnchored {
                credit_curve_id: "ACME-HZD".into(),
                kappa: f64::NAN,
                implied_vol: 0.2,
                tenor_years: None,
            },
            interest_rate_process: None,
            correlation_matrix: None,
            util_credit_corr: None,
        },
        McConfig {
            credit_spread_process: CreditSpreadProcessSpec::Constant(0.01),
            interest_rate_process: Some(InterestRateProcessSpec::HullWhite1F {
                hw1f_mean_reversion: 0.1,
                hw1f_sigma: f64::INFINITY,
                initial: 0.03,
                theta: 0.03,
            }),
            correlation_matrix: None,
            util_credit_corr: None,
        },
        McConfig {
            credit_spread_process: CreditSpreadProcessSpec::Constant(0.01),
            interest_rate_process: None,
            correlation_matrix: None,
            util_credit_corr: Some(f64::INFINITY),
        },
    ];
    for cfg in non_finite_cases {
        assert!(
            cfg.validate().is_err(),
            "McConfig::validate must reject non-finite parameters: {cfg:?}"
        );
    }
}
