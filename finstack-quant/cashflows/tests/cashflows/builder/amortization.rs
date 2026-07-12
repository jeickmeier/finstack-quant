//! Tests for amortization and notional functionality.
use finstack_quant_cashflows::builder::{AmortizationSpec, Notional};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;

#[test]
fn test_notional_par() {
    let notional = Notional::par(1_000_000.0, Currency::USD);

    assert_eq!(notional.initial.amount(), 1_000_000.0);
    assert_eq!(notional.initial.currency(), Currency::USD);
    assert!(matches!(notional.amort, AmortizationSpec::None));
}

#[test]
fn test_notional_currency() {
    let notional = Notional::par(500_000.0, Currency::EUR);
    assert_eq!(notional.currency(), Currency::EUR);
}

#[test]
fn test_amortization_spec_none_validation() {
    let notional = Notional {
        initial: Money::new(1_000_000.0, Currency::USD),
        amort: AmortizationSpec::None,
    };

    let result = notional.validate();
    assert!(result.is_ok());
}

#[test]
fn test_amortization_spec_linear_to_validation_ok() {
    let notional = Notional {
        initial: Money::new(1_000_000.0, Currency::USD),
        amort: AmortizationSpec::LinearTo {
            final_notional: Money::new(500_000.0, Currency::USD),
        },
    };

    let result = notional.validate();
    assert!(result.is_ok());
}

#[test]
fn test_amortization_spec_linear_to_validation_currency_mismatch() {
    let notional = Notional {
        initial: Money::new(1_000_000.0, Currency::USD),
        amort: AmortizationSpec::LinearTo {
            final_notional: Money::new(500_000.0, Currency::EUR), // Different currency
        },
    };

    let result = notional.validate();
    assert!(result.is_err());
}

#[test]
fn test_amortization_spec_linear_to_validation_final_exceeds_initial() {
    let notional = Notional {
        initial: Money::new(1_000_000.0, Currency::USD),
        amort: AmortizationSpec::LinearTo {
            final_notional: Money::new(1_500_000.0, Currency::USD), // Exceeds initial
        },
    };

    let result = notional.validate();
    assert!(result.is_err());
}

#[test]
fn test_amortization_spec_step_remaining_validation_ok() {
    let date1 = Date::from_calendar_date(2025, time::Month::March, 1).unwrap();
    let date2 = Date::from_calendar_date(2025, time::Month::June, 1).unwrap();

    let notional = Notional {
        initial: Money::new(1_000_000.0, Currency::USD),
        amort: AmortizationSpec::StepRemaining {
            schedule: vec![
                (date1, Money::new(750_000.0, Currency::USD)),
                (date2, Money::new(500_000.0, Currency::USD)),
            ],
        },
    };

    let result = notional.validate();
    assert!(result.is_ok());
}

#[test]
fn test_amortization_spec_step_remaining_validation_unsorted_dates_rejected() {
    let date1 = Date::from_calendar_date(2025, time::Month::March, 1).unwrap();
    let date2 = Date::from_calendar_date(2025, time::Month::June, 1).unwrap();

    // Intentionally unsorted input (later date first)
    let notional = Notional {
        initial: Money::new(1_000_000.0, Currency::USD),
        amort: AmortizationSpec::StepRemaining {
            schedule: vec![
                (date2, Money::new(500_000.0, Currency::USD)),
                (date1, Money::new(750_000.0, Currency::USD)),
            ],
        },
    };

    let result = notional.validate();
    assert!(result.is_err());
}

#[test]
fn test_amortization_spec_step_remaining_validation_duplicate_dates_rejected() {
    let date1 = Date::from_calendar_date(2025, time::Month::March, 1).unwrap();

    // Duplicate same date
    let notional = Notional {
        initial: Money::new(1_000_000.0, Currency::USD),
        amort: AmortizationSpec::StepRemaining {
            schedule: vec![
                (date1, Money::new(800_000.0, Currency::USD)),
                (date1, Money::new(700_000.0, Currency::USD)),
            ],
        },
    };

    let result = notional.validate();
    assert!(result.is_err());
}

#[test]
fn test_amortization_spec_step_remaining_validation_currency_mismatch() {
    let date1 = Date::from_calendar_date(2025, time::Month::March, 1).unwrap();

    let notional = Notional {
        initial: Money::new(1_000_000.0, Currency::USD),
        amort: AmortizationSpec::StepRemaining {
            schedule: vec![
                (date1, Money::new(750_000.0, Currency::EUR)), // Different currency
            ],
        },
    };

    let result = notional.validate();
    assert!(result.is_err());
}

#[test]
fn test_amortization_spec_step_remaining_validation_increasing() {
    let date1 = Date::from_calendar_date(2025, time::Month::March, 1).unwrap();
    let date2 = Date::from_calendar_date(2025, time::Month::June, 1).unwrap();

    let notional = Notional {
        initial: Money::new(1_000_000.0, Currency::USD),
        amort: AmortizationSpec::StepRemaining {
            schedule: vec![
                (date1, Money::new(500_000.0, Currency::USD)),
                (date2, Money::new(750_000.0, Currency::USD)), // Increasing - invalid
            ],
        },
    };

    let result = notional.validate();
    assert!(result.is_err());
}

#[test]
fn test_amortization_spec_percent_per_period_validation() {
    let notional = Notional {
        initial: Money::new(1_000_000.0, Currency::USD),
        amort: AmortizationSpec::PercentOfOriginalPerPeriod { pct: 0.05 },
    };

    let result = notional.validate();
    assert!(result.is_ok());
}

#[test]
fn test_amortization_spec_custom_principal_validation_ok() {
    let date1 = Date::from_calendar_date(2025, time::Month::March, 1).unwrap();

    let notional = Notional {
        initial: Money::new(1_000_000.0, Currency::USD),
        amort: AmortizationSpec::CustomPrincipal {
            items: vec![(date1, Money::new(100_000.0, Currency::USD))],
        },
    };

    let result = notional.validate();
    assert!(result.is_ok());
}

#[test]
fn test_amortization_spec_custom_principal_validation_currency_mismatch() {
    let date1 = Date::from_calendar_date(2025, time::Month::March, 1).unwrap();

    let notional = Notional {
        initial: Money::new(1_000_000.0, Currency::USD),
        amort: AmortizationSpec::CustomPrincipal {
            items: vec![
                (date1, Money::new(100_000.0, Currency::EUR)), // Different currency
            ],
        },
    };

    let result = notional.validate();
    assert!(result.is_err());
}

#[test]
fn test_amortization_spec_default() {
    let default_spec = AmortizationSpec::default();
    assert!(matches!(default_spec, AmortizationSpec::None));
}

// =============================================================================
// Edge Case Tests (Market Standards Review - Safety)
// =============================================================================

#[test]
fn test_amortization_spec_percent_nan_rejected() {
    let notional = Notional {
        initial: Money::new(1_000_000.0, Currency::USD),
        amort: AmortizationSpec::PercentOfOriginalPerPeriod { pct: f64::NAN },
    };

    let result = notional.validate();
    assert!(result.is_err(), "NaN percentage should be rejected");
}

#[test]
fn test_amortization_spec_percent_infinity_rejected() {
    let notional = Notional {
        initial: Money::new(1_000_000.0, Currency::USD),
        amort: AmortizationSpec::PercentOfOriginalPerPeriod { pct: f64::INFINITY },
    };

    let result = notional.validate();
    assert!(result.is_err(), "Infinite percentage should be rejected");
}

#[test]
fn test_amortization_spec_percent_neg_infinity_rejected() {
    let notional = Notional {
        initial: Money::new(1_000_000.0, Currency::USD),
        amort: AmortizationSpec::PercentOfOriginalPerPeriod {
            pct: f64::NEG_INFINITY,
        },
    };

    let result = notional.validate();
    assert!(
        result.is_err(),
        "Negative infinite percentage should be rejected"
    );
}

#[test]
fn test_amortization_spec_percent_negative_rejected() {
    let notional = Notional {
        initial: Money::new(1_000_000.0, Currency::USD),
        amort: AmortizationSpec::PercentOfOriginalPerPeriod { pct: -0.05 },
    };

    let result = notional.validate();
    assert!(result.is_err(), "Negative percentage should be rejected");
}

#[test]
fn test_amortization_spec_percent_over_100_rejected() {
    let notional = Notional {
        initial: Money::new(1_000_000.0, Currency::USD),
        amort: AmortizationSpec::PercentOfOriginalPerPeriod { pct: 1.5 },
    };

    let result = notional.validate();
    assert!(result.is_err(), "Percentage over 100% should be rejected");
}

#[test]
fn test_amortization_spec_percent_zero_ok() {
    let notional = Notional {
        initial: Money::new(1_000_000.0, Currency::USD),
        amort: AmortizationSpec::PercentOfOriginalPerPeriod { pct: 0.0 },
    };

    let result = notional.validate();
    assert!(result.is_ok(), "0% percentage should be valid (edge case)");
}

#[test]
fn test_amortization_spec_percent_100_ok() {
    let notional = Notional {
        initial: Money::new(1_000_000.0, Currency::USD),
        amort: AmortizationSpec::PercentOfOriginalPerPeriod { pct: 1.0 },
    };

    let result = notional.validate();
    assert!(
        result.is_ok(),
        "100% percentage should be valid (edge case)"
    );
}

#[test]
fn test_amortization_spec_step_remaining_empty_ok() {
    // Empty schedule means no amortization events - should be valid
    let notional = Notional {
        initial: Money::new(1_000_000.0, Currency::USD),
        amort: AmortizationSpec::StepRemaining { schedule: vec![] },
    };

    let result = notional.validate();
    assert!(
        result.is_ok(),
        "Empty step schedule should be valid (no amortization)"
    );
}

#[test]
fn test_amortization_spec_custom_principal_empty_ok() {
    // Empty custom principal means no exchanges - should be valid
    let notional = Notional {
        initial: Money::new(1_000_000.0, Currency::USD),
        amort: AmortizationSpec::CustomPrincipal { items: vec![] },
    };

    let result = notional.validate();
    assert!(
        result.is_ok(),
        "Empty custom principal should be valid (no exchanges)"
    );
}

#[test]
fn test_amortization_spec_linear_to_zero_ok() {
    // Full amortization to zero is a valid scenario (e.g., fully amortizing loan)
    let notional = Notional {
        initial: Money::new(1_000_000.0, Currency::USD),
        amort: AmortizationSpec::LinearTo {
            final_notional: Money::new(0.0, Currency::USD),
        },
    };

    let result = notional.validate();
    assert!(
        result.is_ok(),
        "Linear amortization to zero should be valid"
    );
}

// =============================================================================
// Amortization Computation Tests (Golden Values)
// =============================================================================

mod computation {
    use super::*;
    use finstack_quant_cashflows::builder::specs::{CouponType, FixedCouponSpec};
    use finstack_quant_cashflows::builder::CashFlowSchedule;
    use finstack_quant_core::cashflow::CFKind;
    use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
    use rust_decimal::Decimal;
    use time::Month;

    /// Helper to create a standard fixed coupon spec for amortization tests
    fn standard_fixed_spec() -> FixedCouponSpec {
        FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: Decimal::try_from(0.05).expect("valid"),
            freq: Tenor::quarterly(),
            dc: DayCount::Act365F,
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
        }
    }

    /// Golden value test: Linear amortization produces correct remaining balances
    ///
    /// For a 1-year term loan with $1M notional amortizing linearly to zero
    /// over 4 quarters, each quarter should reduce principal by $250K.
    #[test]
    fn linear_amortization_golden_values() {
        let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();

        let init = Money::new(1_000_000.0, Currency::USD);

        let mut builder = CashFlowSchedule::builder();
        let _ = builder
            .principal(init, issue, maturity)
            .amortization(AmortizationSpec::LinearTo {
                final_notional: Money::new(0.0, Currency::USD),
            })
            .fixed_cf(standard_fixed_spec());

        let schedule = builder.build_with_curves(None).unwrap();

        // Get amortization flows
        let amort_flows: Vec<_> = schedule
            .flows
            .iter()
            .filter(|cf| cf.kind == CFKind::Amortization)
            .collect();

        // Should have 4 quarterly amortization payments
        assert_eq!(
            amort_flows.len(),
            4,
            "Should have 4 quarterly amortization flows"
        );

        // Each payment should be approximately $250K (1M / 4 quarters)
        let expected_payment = 250_000.0;
        for (i, flow) in amort_flows.iter().enumerate() {
            assert!(
                (flow.amount.amount() - expected_payment).abs() < 1000.0, // $1K tolerance
                "Amortization payment {} should be ~${:.0}, got ${:.2}",
                i + 1,
                expected_payment,
                flow.amount.amount()
            );
        }

        // Verify sum of amortization equals initial notional
        let total_amort: f64 = amort_flows.iter().map(|cf| cf.amount.amount()).sum();
        assert!(
            (total_amort - init.amount()).abs() < 1.0,
            "Sum of amortization should equal initial notional: ${:.2} vs ${:.2}",
            total_amort,
            init.amount()
        );
    }

    #[test]
    fn custom_principal_on_issue_date_is_emitted_once() {
        let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let maturity = Date::from_calendar_date(2025, Month::April, 15).unwrap();
        let init = Money::new(1_000_000.0, Currency::USD);

        let mut builder = CashFlowSchedule::builder();
        let _ = builder
            .principal(init, issue, maturity)
            .amortization(AmortizationSpec::CustomPrincipal {
                items: vec![(issue, Money::new(100_000.0, Currency::USD))],
            })
            .fixed_cf(standard_fixed_spec());

        let schedule = builder
            .build_with_curves(None)
            .expect("issue-dated amortization should build");
        let issue_amort: Vec<_> = schedule
            .flows
            .iter()
            .filter(|cf| cf.kind == CFKind::Amortization && cf.date == issue)
            .collect();
        assert_eq!(
            issue_amort.len(),
            1,
            "issue amortization must emit exactly once"
        );
        assert_eq!(issue_amort[0].amount.amount(), 100_000.0);

        let coupon = schedule
            .flows
            .iter()
            .find(|cf| cf.kind == CFKind::Fixed)
            .expect("coupon");
        let expected_coupon = 900_000.0 * 0.05 * 90.0 / 365.0;
        assert!(
            (coupon.amount.amount() - expected_coupon).abs() < 1e-6,
            "coupon should accrue on 900k after issue amortization, got {}",
            coupon.amount.amount()
        );
    }

    /// Golden value test: Percent-per-period amortization
    ///
    /// For 25% per period on $1M:
    /// - Period 1: pay $250K, remaining = $750K
    /// - Period 2: pay $187.5K (25% of $750K), remaining = $562.5K
    /// - etc.
    #[test]
    fn percent_per_period_amortization_golden_values() {
        let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();

        let init = Money::new(1_000_000.0, Currency::USD);

        let mut builder = CashFlowSchedule::builder();
        let _ = builder
            .principal(init, issue, maturity)
            .amortization(AmortizationSpec::PercentOfOriginalPerPeriod { pct: 0.25 })
            .fixed_cf(standard_fixed_spec());

        let schedule = builder.build_with_curves(None).unwrap();
        let path = schedule.outstanding_by_date().unwrap();

        // Filter to get outstanding after each amortization event
        let amort_flows: Vec<_> = schedule
            .flows
            .iter()
            .filter(|cf| cf.kind == CFKind::Amortization)
            .collect();

        // First amortization: 25% of 1M = $250K, remaining = $750K
        if !amort_flows.is_empty() {
            let first_payment = amort_flows[0].amount.amount();
            assert!(
                (first_payment - 250_000.0).abs() < 100.0,
                "First payment should be ~$250K, got ${:.2}",
                first_payment
            );
        }

        // Outstanding should decrease each period
        let outstanding_values: Vec<f64> = path
            .iter()
            .filter(|(_, _)| true)
            .map(|(_, m)| m.amount())
            .collect();

        // Verify monotonically decreasing (ignoring PIK effects if any)
        for window in outstanding_values.windows(2) {
            // Allow for small increases due to PIK if present
            if window[1] > window[0] + 1000.0 {
                // Only fail if significant increase without PIK
                let has_pik = schedule.flows.iter().any(|cf| cf.kind == CFKind::PIK);
                assert!(
                    has_pik,
                    "Outstanding should decrease: ${:.2} -> ${:.2}",
                    window[0], window[1]
                )
            }
        }
    }

    /// Golden value test: Percent-per-period outstanding path
    ///
    /// For 25% per period on $1M (of original notional), remaining balances after each quarter:
    /// Q1: 750,000; Q2: 500,000; Q3: 250,000
    #[test]
    fn percent_per_period_outstanding_path_golden_values() {
        let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let q1 = Date::from_calendar_date(2025, Month::April, 15).unwrap();
        let q2 = Date::from_calendar_date(2025, Month::July, 15).unwrap();
        let q3 = Date::from_calendar_date(2025, Month::October, 15).unwrap();
        let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();

        let init = Money::new(1_000_000.0, Currency::USD);

        let mut builder = CashFlowSchedule::builder();
        let _ = builder
            .principal(init, issue, maturity)
            .amortization(AmortizationSpec::PercentOfOriginalPerPeriod { pct: 0.25 })
            .fixed_cf(standard_fixed_spec());

        let schedule = builder.build_with_curves(None).unwrap();
        let outstanding = schedule.outstanding_by_date().unwrap();

        let expected_remaining = [(q1, 750_000.0), (q2, 500_000.0), (q3, 250_000.0)];

        for (date, expected) in expected_remaining {
            let actual = outstanding
                .iter()
                .find(|(d, _)| *d == date)
                .map(|(_, m)| m.amount())
                .unwrap();
            assert!(
                (actual - expected).abs() < 1.0,
                "Outstanding at {:?} should be ${:.2}, got ${:.2}",
                date,
                expected,
                actual
            );
        }

        // At maturity, redemption should set outstanding to zero
        let maturity_outstanding = outstanding
            .iter()
            .find(|(d, _)| *d == maturity)
            .map(|(_, m)| m.amount())
            .unwrap();
        assert!(
            maturity_outstanding.abs() < 0.01,
            "Outstanding at maturity should be 0 after redemption, got {}",
            maturity_outstanding
        );
    }

    /// Invariant test: Sum of all principal-related flows equals initial notional
    ///
    /// For a fully amortizing loan (linear to zero), all principal is returned
    /// via amortization flows. For partial amortization, the remaining balance
    /// is returned at maturity via a positive notional flow.
    #[test]
    fn amortization_sum_invariant() {
        let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();

        let init = Money::new(1_000_000.0, Currency::USD);

        // Test with full amortization to zero
        let mut builder = CashFlowSchedule::builder();
        let _ = builder
            .principal(init, issue, maturity)
            .amortization(AmortizationSpec::LinearTo {
                final_notional: Money::new(0.0, Currency::USD),
            })
            .fixed_cf(standard_fixed_spec());

        let schedule = builder.build_with_curves(None).unwrap();

        // Sum of amortization flows
        let amort_sum: f64 = schedule
            .flows
            .iter()
            .filter(|cf| cf.kind == CFKind::Amortization)
            .map(|cf| cf.amount.amount())
            .sum();

        // Sum of redemption flows (positive notional flows at maturity)
        let redemption_sum: f64 = schedule
            .flows
            .iter()
            .filter(|cf| cf.kind == CFKind::Notional && cf.amount.amount() > 0.0)
            .map(|cf| cf.amount.amount())
            .sum();

        // Invariant: total principal returned (amort + redemption) = initial
        let total_principal = amort_sum + redemption_sum;
        assert!(
            (total_principal - init.amount()).abs() < 100.0,
            "Total principal returned should equal initial: ${:.0} vs ${:.2}",
            init.amount(),
            total_principal
        );

        // For full amortization to zero, either:
        // - All principal returned via amortization (redemption = 0), or
        // - Some via amortization and remainder at maturity
        // The key invariant is total = initial
        assert!(
            amort_sum > 0.0 || redemption_sum > 0.0,
            "At least some principal must be returned"
        );
    }

    #[test]
    fn custom_principal_at_maturity_emits_requested_amortization_and_residual_notional() {
        let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
        let init = Money::new(1_000_000.0, Currency::USD);
        let custom_payment = Money::new(250_000.0, Currency::USD);

        let mut builder = CashFlowSchedule::builder();
        let _ = builder.principal(init, issue, maturity).amortization(
            AmortizationSpec::CustomPrincipal {
                items: vec![(maturity, custom_payment)],
            },
        );

        let schedule = builder.build_with_curves(None).unwrap();

        let amortization: Vec<_> = schedule
            .flows
            .iter()
            .filter(|cf| cf.date == maturity && cf.kind == CFKind::Amortization)
            .collect();
        assert_eq!(
            amortization.len(),
            1,
            "custom maturity payment should emit one amortization flow"
        );
        assert!(
            (amortization[0].amount.amount() - custom_payment.amount()).abs() < 1e-9,
            "custom maturity amortization should honor the configured amount"
        );

        let residual_notional: Vec<_> = schedule
            .flows
            .iter()
            .filter(|cf| {
                cf.date == maturity && cf.kind == CFKind::Notional && cf.amount.amount() > 0.0
            })
            .collect();
        assert_eq!(
            residual_notional.len(),
            1,
            "residual maturity balance should emit one positive notional flow"
        );
        assert!(
            (residual_notional[0].amount.amount() - 750_000.0).abs() < 1e-9,
            "residual maturity notional should redeem the remaining balance"
        );
    }

    #[test]
    fn custom_principal_same_date_negative_item_does_not_offset_repayment() {
        let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let pay_date = Date::from_calendar_date(2025, Month::July, 15).unwrap();
        let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
        let init = Money::new(1_000_000.0, Currency::USD);

        let mut builder = CashFlowSchedule::builder();
        let _ = builder.principal(init, issue, maturity).amortization(
            AmortizationSpec::CustomPrincipal {
                items: vec![
                    (pay_date, Money::new(100_000.0, Currency::USD)),
                    (pay_date, Money::new(-100_000.0, Currency::USD)),
                ],
            },
        );

        let err = builder
            .build_with_curves(None)
            .expect_err("negative custom principal entries must be rejected");
        assert!(err.to_string().contains("non-negative"));
    }

    /// Maturity classification: StepRemaining emits only the scheduled
    /// step-down as `Amortization` at maturity; the residual (final target)
    /// is redeemed as `Notional`, consistent with `CustomPrincipal`.
    ///
    /// Previously the entire outstanding was emitted as `Amortization` at
    /// maturity. Total principal cash is identical — only the KIND
    /// classification of the residual changed (Amortization -> Notional).
    #[test]
    fn step_remaining_at_maturity_emits_scheduled_amortization_and_residual_notional() {
        let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let q2 = Date::from_calendar_date(2025, Month::July, 15).unwrap();
        let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
        let init = Money::new(1_000_000.0, Currency::USD);

        // Step down to 800K mid-life, then a final 200K target at maturity.
        let schedule_pairs = vec![
            (q2, Money::new(800_000.0, Currency::USD)),
            (maturity, Money::new(200_000.0, Currency::USD)),
        ];

        let mut builder = CashFlowSchedule::builder();
        let _ = builder
            .principal(init, issue, maturity)
            .amortization(AmortizationSpec::StepRemaining {
                schedule: schedule_pairs,
            })
            .fixed_cf(standard_fixed_spec());

        let schedule = builder.build_with_curves(None).unwrap();

        // At maturity: scheduled step-down 800K -> 200K = 600K Amortization.
        let maturity_amort: f64 = schedule
            .flows
            .iter()
            .filter(|cf| cf.date == maturity && cf.kind == CFKind::Amortization)
            .map(|cf| cf.amount.amount())
            .sum();
        assert!(
            (maturity_amort - 600_000.0).abs() < 1e-6,
            "maturity amortization should be the scheduled step-down (600K), got {maturity_amort}"
        );

        // Residual 200K (the final target) is redeemed as Notional.
        let maturity_notional: f64 = schedule
            .flows
            .iter()
            .filter(|cf| {
                cf.date == maturity && cf.kind == CFKind::Notional && cf.amount.amount() > 0.0
            })
            .map(|cf| cf.amount.amount())
            .sum();
        assert!(
            (maturity_notional - 200_000.0).abs() < 1e-6,
            "residual balance should be redeemed as Notional (200K), got {maturity_notional}"
        );

        // Economic invariant: total principal returned equals initial.
        let total_amort: f64 = schedule
            .flows
            .iter()
            .filter(|cf| cf.kind == CFKind::Amortization)
            .map(|cf| cf.amount.amount())
            .sum();
        assert!(
            (total_amort + maturity_notional - init.amount()).abs() < 1e-6,
            "total principal cash must equal initial notional"
        );
    }

    /// Maturity classification: PercentOfOriginalPerPeriod emits only the
    /// configured percentage as `Amortization` at maturity; the residual is
    /// redeemed as `Notional`, consistent with `CustomPrincipal`.
    ///
    /// Previously the entire outstanding was emitted as `Amortization` at
    /// maturity. Total principal cash is identical — only the KIND
    /// classification of the residual changed (Amortization -> Notional).
    #[test]
    fn percent_per_period_at_maturity_emits_scheduled_amortization_and_residual_notional() {
        let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
        let init = Money::new(1_000_000.0, Currency::USD);

        // 10% of original per quarter: 100K on each of 4 quarterly dates,
        // leaving 600K residual at maturity.
        let mut builder = CashFlowSchedule::builder();
        let _ = builder
            .principal(init, issue, maturity)
            .amortization(AmortizationSpec::PercentOfOriginalPerPeriod { pct: 0.10 })
            .fixed_cf(standard_fixed_spec());

        let schedule = builder.build_with_curves(None).unwrap();

        let maturity_amort: f64 = schedule
            .flows
            .iter()
            .filter(|cf| cf.date == maturity && cf.kind == CFKind::Amortization)
            .map(|cf| cf.amount.amount())
            .sum();
        assert!(
            (maturity_amort - 100_000.0).abs() < 1e-6,
            "maturity amortization should be the configured 10% (100K), got {maturity_amort}"
        );

        let maturity_notional: f64 = schedule
            .flows
            .iter()
            .filter(|cf| {
                cf.date == maturity && cf.kind == CFKind::Notional && cf.amount.amount() > 0.0
            })
            .map(|cf| cf.amount.amount())
            .sum();
        assert!(
            (maturity_notional - 600_000.0).abs() < 1e-6,
            "residual balance should be redeemed as Notional (600K), got {maturity_notional}"
        );

        let total_amort: f64 = schedule
            .flows
            .iter()
            .filter(|cf| cf.kind == CFKind::Amortization)
            .map(|cf| cf.amount.amount())
            .sum();
        assert!(
            (total_amort + maturity_notional - init.amount()).abs() < 1e-6,
            "total principal cash must equal initial notional"
        );
    }

    /// Test step-remaining amortization produces correct outstanding path
    #[test]
    fn step_remaining_amortization_golden_values() {
        let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
        let q1_date = Date::from_calendar_date(2025, Month::April, 15).unwrap();
        let q2_date = Date::from_calendar_date(2025, Month::July, 15).unwrap();
        let q3_date = Date::from_calendar_date(2025, Month::October, 15).unwrap();

        let init = Money::new(1_000_000.0, Currency::USD);

        // Step schedule: 1M -> 800K -> 500K -> 200K -> 0
        let schedule_pairs = vec![
            (q1_date, Money::new(800_000.0, Currency::USD)),
            (q2_date, Money::new(500_000.0, Currency::USD)),
            (q3_date, Money::new(200_000.0, Currency::USD)),
            (maturity, Money::new(0.0, Currency::USD)),
        ];

        let mut builder = CashFlowSchedule::builder();
        let _ = builder
            .principal(init, issue, maturity)
            .amortization(AmortizationSpec::StepRemaining {
                schedule: schedule_pairs,
            })
            .fixed_cf(standard_fixed_spec());

        let schedule = builder.build_with_curves(None).unwrap();
        let outstanding = schedule.outstanding_by_date().unwrap();

        // Verify outstanding at each step date
        let expected_remaining = [
            (q1_date, 800_000.0),
            (q2_date, 500_000.0),
            (q3_date, 200_000.0),
        ];

        for (expected_date, expected_balance) in expected_remaining {
            let actual = outstanding
                .iter()
                .find(|(d, _)| *d == expected_date)
                .map(|(_, m)| m.amount());

            assert!(
                actual.is_some(),
                "Should have outstanding at {:?}",
                expected_date
            );

            assert!(
                (actual.unwrap() - expected_balance).abs() < 100.0,
                "Outstanding at {:?} should be ${:.0}, got ${:.2}",
                expected_date,
                expected_balance,
                actual.unwrap()
            );
        }
    }
}
