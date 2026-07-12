// Tests for emission functions.

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod accrual_context_tests {
    use crate::cashflow::builder::compiler::{FixedSchedule, FloatSchedule};
    use crate::cashflow::builder::emission::coupons::{
        emit_fixed_coupons_on, emit_float_coupons_on,
    };
    use crate::cashflow::builder::specs::{
        CouponType, FixedCouponSpec, FloatingCouponSpec, FloatingRateSpec,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind};
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use finstack_quant_core::types::CurveId;
    use rust_decimal::Decimal;
    use time::Month;

    #[test]
    fn fixed_accrual_with_actact_isma_full_period() {
        // Test that Act/Act ISMA with frequency context gives correct year fraction
        let start = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::July, 15).expect("valid date");

        let spec = FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: Decimal::try_from(0.05).expect("valid rate"),
            freq: finstack_quant_core::dates::Tenor::new(6, finstack_quant_core::dates::TenorUnit::Months), // Semi-annual
            dc: DayCount::ActActIsma,
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
        };

        let dates = vec![end];
        let mut prev_map = finstack_quant_core::HashMap::default();
        prev_map.insert(
            end,
            crate::cashflow::builder::periods::SchedulePeriod {
                accrual_start: start,
                accrual_end: end,
                payment_date: end,
                reset_date: None,
                accrual_year_fraction: 0.0,
            },
        );
        let first_last = finstack_quant_core::HashSet::default();

        let schedule: FixedSchedule = (spec, dates, prev_map, first_last);
        let outstanding_after = finstack_quant_core::HashMap::default();
        let outstanding_fallback = Decimal::new(1_000_000, 0);

        let mut flows = Vec::new();
        let pik = emit_fixed_coupons_on(
            end,
            &[schedule],
            &outstanding_after,
            outstanding_fallback,
            Currency::USD,
            &mut flows,
        )
        .expect("should emit fixed coupons");

        assert_eq!(pik, 0.0);
        assert_eq!(flows.len(), 1);

        // Accrual factor should be 0.5 for full semi-annual period (6 months / 12 months)
        assert!(
            (flows[0].accrual_factor - 0.5).abs() < 1e-6,
            "Expected accrual ~0.5, got {}",
            flows[0].accrual_factor
        );

        // Coupon amount: 1M × 5% × 0.5 = 25K
        assert!((flows[0].amount.amount() - 25_000.0).abs() < 1.0);
    }

    #[test]
    fn float_accrual_with_actact_isma_quarterly() {
        // Test quarterly floating with Act/Act ISMA
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 1).expect("valid date");

        let spec = FloatingCouponSpec {
            rate_spec: FloatingRateSpec {
                index_id: CurveId::new("USD-SOFR-3M"),
                spread_bp: Decimal::try_from(200.0).expect("valid spread"),
                gearing: Decimal::ONE,
                gearing_includes_spread: true,
                index_floor_bp: None,
                all_in_cap_bp: None,
                all_in_floor_bp: None,
                index_cap_bp: None,
                reset_freq: finstack_quant_core::dates::Tenor::new(
                    3,
                    finstack_quant_core::dates::TenorUnit::Months,
                ),
                reset_lag_days: 2,
                dc: DayCount::ActActIsma,
                bdc: BusinessDayConvention::Following,
                calendar_id: "weekends_only".to_string(),
                fixing_calendar_id: None,
                end_of_month: false,
                overnight_compounding: None,
                overnight_basis: None,
                fallback: finstack_quant_cashflows::builder::FloatingRateFallback::SpreadOnly,
                payment_lag_days: 0,
            },
            coupon_type: CouponType::Cash,
            freq: finstack_quant_core::dates::Tenor::new(3, finstack_quant_core::dates::TenorUnit::Months),
            stub: StubKind::None,
        };

        let dates = vec![end];
        let mut prev_map = finstack_quant_core::HashMap::default();
        prev_map.insert(
            end,
            crate::cashflow::builder::periods::SchedulePeriod {
                accrual_start: start,
                accrual_end: end,
                payment_date: end,
                reset_date: None,
                accrual_year_fraction: 0.0,
            },
        );

        let schedule: FloatSchedule = (spec, dates, prev_map);
        let outstanding_after = finstack_quant_core::HashMap::default();
        let outstanding_fallback = Decimal::new(1_000_000, 0);

        let mut flows = Vec::new();
        let resolved: [Option<std::sync::Arc<ForwardCurve>>; 1] = [None];
        let pik = emit_float_coupons_on(
            end,
            &[schedule],
            &outstanding_after,
            outstanding_fallback,
            Currency::USD,
            &resolved, // One resolved curve slot (None) to match the one float schedule
            &mut flows,
        )
        .expect("should emit float coupons");

        assert_eq!(pik, 0.0);
        assert_eq!(flows.len(), 1);

        // For full quarterly period, ISMA accrual should be 0.25 (3 months / 12 months)
        assert!(
            (flows[0].accrual_factor - 0.25).abs() < 1e-6,
            "Expected accrual ~0.25 for full ISMA quarter, got {}",
            flows[0].accrual_factor
        );
    }

    #[test]
    fn bus252_accrual_requires_calendar() {
        // Bus/252 with calendar should calculate business days

        let start = Date::from_calendar_date(2025, Month::January, 6).expect("valid date"); // Monday
        let end = Date::from_calendar_date(2025, Month::January, 13).expect("valid date"); // Next Monday (5 biz days)

        // This test verifies that the calendar lookup happens in coupons.rs
        // The actual year fraction calculation is tested in core's day-count tests
        // Here we just verify no panic/error when using Bus/252 with a valid calendar ID

        let spec = FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: Decimal::try_from(0.05).expect("valid rate"),
            freq: finstack_quant_core::dates::Tenor::new(7, finstack_quant_core::dates::TenorUnit::Days),
            dc: DayCount::Bus252,
            bdc: BusinessDayConvention::Following,
            calendar_id: "NYSE".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
        };

        let dates = vec![end];
        let mut prev_map = finstack_quant_core::HashMap::default();
        prev_map.insert(
            end,
            crate::cashflow::builder::periods::SchedulePeriod {
                accrual_start: start,
                accrual_end: end,
                payment_date: end,
                reset_date: None,
                accrual_year_fraction: 0.0,
            },
        );
        let first_last = finstack_quant_core::HashSet::default();

        let schedule: FixedSchedule = (spec, dates, prev_map, first_last);
        let outstanding_after = finstack_quant_core::HashMap::default();
        let outstanding_fallback = Decimal::new(1_000_000, 0);

        let mut flows = Vec::new();
        let result = emit_fixed_coupons_on(
            end,
            &[schedule],
            &outstanding_after,
            outstanding_fallback,
            Currency::USD,
            &mut flows,
        );

        // Should succeed with calendar available
        assert!(result.is_ok());
        let pik = result.expect("should succeed with calendar");
        assert_eq!(pik, 0.0);
        assert_eq!(flows.len(), 1);

        // Year fraction should be roughly 5 business days / 252
        let expected_yf = 5.0 / 252.0;
        assert!(
            (flows[0].accrual_factor - expected_yf).abs() < 0.01,
            "Expected accrual ~{}, got {}",
            expected_yf,
            flows[0].accrual_factor
        );
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod credit_emission_tests {
    use crate::cashflow::builder::emission::credit::{emit_default_on, emit_prepayment_on};
    use crate::cashflow::builder::specs::DefaultEvent;
    use crate::cashflow::primitives::CFKind;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::dates::DateExt;
    use time::Month;

    #[test]
    fn test_default_and_recovery_mechanics() {
        let d = Date::from_calendar_date(2025, Month::March, 1).expect("valid date");
        let event = DefaultEvent {
            default_date: d,
            defaulted_amount: 400_000.0,
            recovery_rate: 0.40,
            recovery_lag: 12,
            recovery_bdc: None,
            recovery_calendar_id: None,
            accrued_on_default: None,
        };

        let mut outstanding: f64 = 1_000_000.0;
        let mut flows = Vec::new();
        emit_default_on(d, &[event], &mut outstanding, Currency::USD, &mut flows)
            .expect("should emit default");

        // Outstanding is reduced by the full defaulted amount.
        // Recovery is a future cash inflow, not an immediate outstanding increase.
        // This ensures interest between default and recovery uses the correct base.
        assert_eq!(outstanding, 600_000.0);
        assert_eq!(flows.len(), 2);

        // First flow: default
        assert_eq!(flows[0].kind, CFKind::DefaultedNotional);
        assert_eq!(flows[0].amount.amount(), 400_000.0);
        assert_eq!(flows[0].date, d);

        // Second flow: recovery (future cash inflow)
        assert_eq!(flows[1].kind, CFKind::Recovery);
        assert_eq!(flows[1].amount.amount(), 160_000.0);
        let expected_recovery_date = d.add_months(12);
        assert_eq!(flows[1].date, expected_recovery_date);
    }

    #[test]
    fn test_coupon_on_reduced_outstanding_after_default() {
        // CRITICAL TEST: Verify coupon uses reduced outstanding after default
        use crate::cashflow::builder::emission::coupons::emit_fixed_coupons_on;
        use crate::cashflow::builder::specs::{CouponType, FixedCouponSpec};
        use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};

        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let mat = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");
        let default_date = Date::from_calendar_date(2025, Month::July, 1).expect("valid date");
        let coupon_date = Date::from_calendar_date(2025, Month::October, 1).expect("valid date");

        // Setup: 1M notional, 5% coupon, quarterly payments
        let spec = FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid rate"),
            freq: Tenor::quarterly(),
            dc: DayCount::Act360,
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
        };

        // Generate coupon dates
        let period_schedule = crate::cashflow::builder::periods::build_periods(
            crate::cashflow::builder::periods::BuildPeriodsParams {
                start: issue,
                end: mat,
                frequency: spec.freq,
                stub: spec.stub,
                bdc: spec.bdc,
                calendar_id: &spec.calendar_id,
                end_of_month: spec.end_of_month,
                day_count: spec.dc,
                payment_lag_days: spec.payment_lag_days,
                reset_lag_days: None,
                adjust_accrual_dates: false,
            },
        )
        .expect("schedule should build");

        let mut outstanding_after = finstack_quant_core::HashMap::default();
        outstanding_after.insert(issue, Decimal::new(1_000_000, 0));

        // Apply default on July 1: 400K defaults, 40% recovery
        let default_event = DefaultEvent {
            default_date,
            defaulted_amount: 400_000.0,
            recovery_rate: 0.40,
            recovery_lag: 12,
            recovery_bdc: None,
            recovery_calendar_id: None,
            accrued_on_default: None,
        };

        let mut outstanding: f64 = 1_000_000.0;
        let mut _default_flows = Vec::new();
        emit_default_on(
            default_date,
            &[default_event],
            &mut outstanding,
            Currency::USD,
            &mut _default_flows,
        )
        .expect("should emit default");

        // Outstanding now 600K (1M - 400K). Recovery is future inflow, not outstanding increase.
        outstanding_after.insert(
            default_date,
            Decimal::try_from(outstanding).expect("outstanding converts to Decimal"),
        );

        // Generate coupon on Oct 1 using reduced outstanding
        let mut period_map = finstack_quant_core::HashMap::default();
        period_map.reserve(period_schedule.len());
        for p in &period_schedule {
            period_map.insert(p.payment_date, *p);
        }
        let payment_dates = period_schedule
            .iter()
            .map(|period| period.payment_date)
            .collect();
        let schedule = (
            spec,
            payment_dates,
            period_map,
            finstack_quant_core::HashSet::default(),
        );
        let mut coupons = Vec::new();
        let pik = emit_fixed_coupons_on(
            coupon_date,
            &[schedule],
            &outstanding_after,
            Decimal::new(1_000_000, 0),
            Currency::USD,
            &mut coupons,
        )
        .expect("should emit fixed coupons");

        assert_eq!(pik, 0.0);
        assert_eq!(coupons.len(), 1);

        // Coupon should be on 600K (not 1M, not the old 760K)
        // Year fraction for Jul 1 - Oct 1 = 92 days
        let yf = 92.0 / 360.0;
        let expected_coupon = 600_000.0 * 0.05 * yf;

        assert!(
            (coupons[0].amount.amount() - expected_coupon).abs() < 1.0,
            "Expected coupon ~{}, got {}",
            expected_coupon,
            coupons[0].amount.amount()
        );
    }

    #[test]
    fn test_prepayment_reduces_outstanding() {
        let d = Date::from_calendar_date(2025, Month::March, 1).expect("valid date");
        let mut outstanding: f64 = 1_000_000.0;

        let mut flows = Vec::new();
        emit_prepayment_on(d, 50_000.0, &mut outstanding, Currency::USD, &mut flows);

        assert_eq!(outstanding, 950_000.0);
        assert_eq!(flows.len(), 1);
        assert_eq!(flows[0].kind, CFKind::PrePayment);
        assert_eq!(flows[0].amount.amount(), 50_000.0);
    }

    #[test]
    fn test_prepayment_capped_by_outstanding() {
        let d = Date::from_calendar_date(2025, Month::March, 1).expect("valid date");
        let mut outstanding = 30_000.0;

        let mut flows = Vec::new();
        emit_prepayment_on(d, 50_000.0, &mut outstanding, Currency::USD, &mut flows);

        // Can only prepay what's outstanding
        assert_eq!(outstanding, 0.0);
        assert_eq!(flows[0].amount.amount(), 30_000.0);
    }

    #[test]
    fn test_zero_recovery_rate() {
        let d = Date::from_calendar_date(2025, Month::March, 1).expect("valid date");
        let event = DefaultEvent {
            default_date: d,
            defaulted_amount: 100_000.0,
            recovery_rate: 0.0, // Total loss
            recovery_lag: 12,
            recovery_bdc: None,
            recovery_calendar_id: None,
            accrued_on_default: None,
        };

        let mut outstanding = 1_000_000.0;
        let mut flows = Vec::new();
        emit_default_on(d, &[event], &mut outstanding, Currency::USD, &mut flows)
            .expect("should emit default");

        // Net loss is 100% of defaulted amount
        assert_eq!(outstanding, 900_000.0);
        assert_eq!(flows.len(), 1); // Only default, no recovery
    }

    #[test]
    fn test_full_recovery_rate() {
        let d = Date::from_calendar_date(2025, Month::March, 1).expect("valid date");
        let event = DefaultEvent {
            default_date: d,
            defaulted_amount: 100_000.0,
            recovery_rate: 1.0, // Full recovery
            recovery_lag: 12,
            recovery_bdc: None,
            recovery_calendar_id: None,
            accrued_on_default: None,
        };

        let mut outstanding = 1_000_000.0;
        let mut flows = Vec::new();
        emit_default_on(d, &[event], &mut outstanding, Currency::USD, &mut flows)
            .expect("should emit default");

        // Outstanding is reduced by full defaulted amount at default time.
        // Recovery (100K) is a future cash inflow, not an immediate balance change.
        assert_eq!(outstanding, 900_000.0);
        assert_eq!(flows.len(), 2); // Default + full recovery
    }

    #[test]
    fn test_multiple_defaults_same_date() {
        let d = Date::from_calendar_date(2025, Month::March, 1).expect("valid date");
        let events = vec![
            DefaultEvent {
                default_date: d,
                defaulted_amount: 50_000.0,
                recovery_rate: 0.40,
                recovery_lag: 12,
                recovery_bdc: None,
                recovery_calendar_id: None,
                accrued_on_default: None,
            },
            DefaultEvent {
                default_date: d,
                defaulted_amount: 30_000.0,
                recovery_rate: 0.50,
                recovery_lag: 6,
                recovery_bdc: None,
                recovery_calendar_id: None,
                accrued_on_default: None,
            },
        ];

        let mut outstanding = 1_000_000.0;
        let mut flows = Vec::new();
        emit_default_on(d, &events, &mut outstanding, Currency::USD, &mut flows)
            .expect("should emit multiple defaults");

        // Outstanding is reduced by total defaulted amounts: 50K + 30K = 80K
        // Recoveries are future cash inflows, not immediate balance changes.
        assert_eq!(outstanding, 1_000_000.0 - 50_000.0 - 30_000.0);
        assert_eq!(flows.len(), 4); // 2 defaults + 2 recoveries
    }

    #[test]
    fn test_non_matching_dates_return_empty() {
        let d = Date::from_calendar_date(2025, Month::March, 1).expect("valid date");
        let other_date = Date::from_calendar_date(2025, Month::April, 1).expect("valid date");

        let event = DefaultEvent {
            default_date: other_date,
            defaulted_amount: 100_000.0,
            recovery_rate: 0.40,
            recovery_lag: 12,
            recovery_bdc: None,
            recovery_calendar_id: None,
            accrued_on_default: None,
        };

        let mut outstanding = 1_000_000.0;
        let mut flows = Vec::new();
        emit_default_on(d, &[event], &mut outstanding, Currency::USD, &mut flows)
            .expect("should emit default");

        assert_eq!(outstanding, 1_000_000.0); // Unchanged
        assert_eq!(flows.len(), 0);
    }

    #[test]
    fn test_recovery_lag_calculation() {
        let d = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let event = DefaultEvent {
            default_date: d,
            defaulted_amount: 100_000.0,
            recovery_rate: 0.40,
            recovery_lag: 6,
            recovery_bdc: None,
            recovery_calendar_id: None,
            accrued_on_default: None,
        };

        let mut outstanding = 1_000_000.0;
        let mut flows = Vec::new();
        emit_default_on(d, &[event], &mut outstanding, Currency::USD, &mut flows)
            .expect("should emit default");

        let expected_recovery_date = d.add_months(6);
        assert_eq!(flows[1].date, expected_recovery_date);
    }

    #[test]
    fn test_prepayment_model_psa_curve() {
        use crate::cashflow::builder::credit_rates::smm_to_cpr;
        use crate::cashflow::builder::specs::PrepaymentModelSpec;

        let model = PrepaymentModelSpec::psa(1.5); // 150% PSA

        // Month 15: should be 4.5% CPR (halfway to 9%)
        let smm = model.smm(15);
        assert!(smm > 0.0);
        let cpr = smm_to_cpr(smm);
        assert!((cpr - 0.045).abs() < 0.001);

        // Month 30: should be 9% CPR = ~0.77% SMM
        let smm = model.smm(30);
        let cpr = smm_to_cpr(smm);
        assert!((cpr - 0.09).abs() < 0.001);

        // Month 60: should still be 9% CPR (flat after month 30)
        let smm = model.smm(60);
        let cpr = smm_to_cpr(smm);
        assert!((cpr - 0.09).abs() < 0.001);
    }

    #[test]
    fn test_default_model_sda_curve() {
        use crate::cashflow::builder::specs::DefaultModelSpec;

        let model = DefaultModelSpec::sda(2.0); // 200% SDA

        // Should ramp up then decline
        let early = model.mdr(10);
        let peak = model.mdr(30);
        let late = model.mdr(70);

        assert!(peak > early, "Peak should be greater than early");
        assert!(peak > late, "Peak should be greater than late");
        assert!(early > 0.0, "Early should be positive");
    }

    #[test]
    fn test_invalid_recovery_rate_too_high() {
        let d = Date::from_calendar_date(2025, Month::March, 1).expect("valid date");
        let event = DefaultEvent {
            default_date: d,
            defaulted_amount: 100_000.0,
            recovery_rate: 1.5, // Invalid: > 1.0
            recovery_lag: 12,
            recovery_bdc: None,
            recovery_calendar_id: None,
            accrued_on_default: None,
        };

        let mut outstanding = 1_000_000.0;
        let mut flows = Vec::new();
        let result = emit_default_on(d, &[event], &mut outstanding, Currency::USD, &mut flows);
        assert!(result.is_err(), "Should reject recovery_rate > 1.0");
    }

    #[test]
    fn test_invalid_recovery_rate_negative() {
        let d = Date::from_calendar_date(2025, Month::March, 1).expect("valid date");
        let event = DefaultEvent {
            default_date: d,
            defaulted_amount: 100_000.0,
            recovery_rate: -0.1, // Invalid: < 0.0
            recovery_lag: 12,
            recovery_bdc: None,
            recovery_calendar_id: None,
            accrued_on_default: None,
        };

        let mut outstanding = 1_000_000.0;
        let mut flows = Vec::new();
        let result = emit_default_on(d, &[event], &mut outstanding, Currency::USD, &mut flows);
        assert!(result.is_err(), "Should reject recovery_rate < 0.0");
    }

    #[test]
    fn test_invalid_defaulted_amount_negative() {
        let d = Date::from_calendar_date(2025, Month::March, 1).expect("valid date");
        let event = DefaultEvent {
            default_date: d,
            defaulted_amount: -100_000.0, // Invalid: negative
            recovery_rate: 0.4,
            recovery_lag: 12,
            recovery_bdc: None,
            recovery_calendar_id: None,
            accrued_on_default: None,
        };

        let mut outstanding = 1_000_000.0;
        let mut flows = Vec::new();
        let result = emit_default_on(d, &[event], &mut outstanding, Currency::USD, &mut flows);
        assert!(result.is_err(), "Should reject negative defaulted_amount");
    }

    #[test]
    fn test_default_amount_clamped_to_outstanding() {
        // If defaulted_amount exceeds outstanding, it should be clamped
        // to prevent negative outstanding balances.
        let d = Date::from_calendar_date(2025, Month::March, 1).expect("valid date");
        let event = DefaultEvent {
            default_date: d,
            defaulted_amount: 1_500_000.0, // More than outstanding
            recovery_rate: 0.40,
            recovery_lag: 12,
            recovery_bdc: None,
            recovery_calendar_id: None,
            accrued_on_default: None,
        };

        let mut outstanding = 1_000_000.0;
        let mut flows = Vec::new();
        emit_default_on(d, &[event], &mut outstanding, Currency::USD, &mut flows)
            .expect("should emit clamped default");

        // Outstanding should be 0 (clamped to not go negative)
        assert!(
            outstanding.abs() < 1e-9,
            "Outstanding should be 0 after clamped default, got {}",
            outstanding
        );

        // Default cashflow should be clamped to original outstanding
        assert_eq!(flows.len(), 2); // Default + Recovery
        assert_eq!(flows[0].kind, CFKind::DefaultedNotional);
        assert!(
            (flows[0].amount.amount() - 1_000_000.0).abs() < 1e-9,
            "Default amount should be clamped to outstanding (1M), got {}",
            flows[0].amount.amount()
        );

        // Recovery should also be based on clamped amount
        assert_eq!(flows[1].kind, CFKind::Recovery);
        assert!(
            (flows[1].amount.amount() - 400_000.0).abs() < 1e-9,
            "Recovery should be 40% of clamped default (400K), got {}",
            flows[1].amount.amount()
        );
    }

    #[test]
    fn test_default_skipped_when_outstanding_zero() {
        // If outstanding is already 0, default should be skipped entirely
        let d = Date::from_calendar_date(2025, Month::March, 1).expect("valid date");
        let event = DefaultEvent {
            default_date: d,
            defaulted_amount: 100_000.0,
            recovery_rate: 0.40,
            recovery_lag: 12,
            recovery_bdc: None,
            recovery_calendar_id: None,
            accrued_on_default: None,
        };

        let mut outstanding: f64 = 0.0;
        let mut flows = Vec::new();
        emit_default_on(d, &[event], &mut outstanding, Currency::USD, &mut flows)
            .expect("should succeed but emit no flows");

        // No flows should be emitted when nothing to default
        assert!(
            flows.is_empty(),
            "Should emit no flows when outstanding is 0"
        );
        assert!(outstanding.abs() < 1e-9, "Outstanding should remain 0");
    }

    #[test]
    fn test_accrued_on_default_emission() {
        // When accrued_on_default is Some(12_500.0), emit_default_on should
        // produce 3 flows: DefaultedNotional + Recovery + AccruedOnDefault.
        let d = Date::from_calendar_date(2025, Month::March, 1).expect("valid date");
        let event = DefaultEvent {
            default_date: d,
            defaulted_amount: 100_000.0,
            recovery_rate: 0.40,
            recovery_lag: 12,
            recovery_bdc: None,
            recovery_calendar_id: None,
            accrued_on_default: Some(12_500.0),
        };

        let mut outstanding = 1_000_000.0;
        let mut flows = Vec::new();
        emit_default_on(d, &[event], &mut outstanding, Currency::USD, &mut flows)
            .expect("should emit default with accrued");

        // Outstanding reduced by defaulted amount
        assert_eq!(outstanding, 900_000.0);

        // Should produce 3 flows: DefaultedNotional + Recovery + AccruedOnDefault
        assert_eq!(flows.len(), 3, "Expected 3 flows, got {}", flows.len());

        // First flow: DefaultedNotional
        assert_eq!(flows[0].kind, CFKind::DefaultedNotional);
        assert_eq!(flows[0].amount.amount(), 100_000.0);
        assert_eq!(flows[0].date, d);

        // Second flow: Recovery
        assert_eq!(flows[1].kind, CFKind::Recovery);
        assert_eq!(flows[1].amount.amount(), 40_000.0);
        let expected_recovery_date = d.add_months(12);
        assert_eq!(flows[1].date, expected_recovery_date);

        // Third flow: AccruedOnDefault
        assert_eq!(flows[2].kind, CFKind::AccruedOnDefault);
        assert!(
            (flows[2].amount.amount() - 12_500.0).abs() < 1e-9,
            "AccruedOnDefault amount should be 12,500, got {}",
            flows[2].amount.amount()
        );
        assert_eq!(flows[2].date, d, "AccruedOnDefault should be on default date");
        assert_eq!(flows[2].accrual_factor, 0.0);
        assert!(flows[2].rate.is_none());
    }

    #[test]
    fn test_accrued_on_default_none_no_extra_flow() {
        // When accrued_on_default is None, only DefaultedNotional + Recovery are emitted.
        let d = Date::from_calendar_date(2025, Month::March, 1).expect("valid date");
        let event = DefaultEvent {
            default_date: d,
            defaulted_amount: 100_000.0,
            recovery_rate: 0.40,
            recovery_lag: 12,
            recovery_bdc: None,
            recovery_calendar_id: None,
            accrued_on_default: None,
        };

        let mut outstanding = 1_000_000.0;
        let mut flows = Vec::new();
        emit_default_on(d, &[event], &mut outstanding, Currency::USD, &mut flows)
            .expect("should emit default without accrued");

        // Should produce only 2 flows: DefaultedNotional + Recovery (no AccruedOnDefault)
        assert_eq!(
            flows.len(),
            2,
            "Expected 2 flows (no AccruedOnDefault), got {}",
            flows.len()
        );
        assert_eq!(flows[0].kind, CFKind::DefaultedNotional);
        assert_eq!(flows[1].kind, CFKind::Recovery);
    }

    #[test]
    fn test_accrued_on_default_zero_no_extra_flow() {
        // When accrued_on_default is Some(0.0), no AccruedOnDefault flow should be emitted
        // because the guard checks accrued_amt > 0.0.
        let d = Date::from_calendar_date(2025, Month::March, 1).expect("valid date");
        let event = DefaultEvent {
            default_date: d,
            defaulted_amount: 100_000.0,
            recovery_rate: 0.40,
            recovery_lag: 12,
            recovery_bdc: None,
            recovery_calendar_id: None,
            accrued_on_default: Some(0.0),
        };

        let mut outstanding = 1_000_000.0;
        let mut flows = Vec::new();
        emit_default_on(d, &[event], &mut outstanding, Currency::USD, &mut flows)
            .expect("should emit default without accrued for zero amount");

        // Should produce only 2 flows: no AccruedOnDefault for zero amount
        assert_eq!(
            flows.len(),
            2,
            "Expected 2 flows (zero accrued should not emit), got {}",
            flows.len()
        );
        assert_eq!(flows[0].kind, CFKind::DefaultedNotional);
        assert_eq!(flows[1].kind, CFKind::Recovery);
    }
}
