//! Property-based tests for revolving credit facilities.
//!
//! These tests use proptest to verify invariants hold across a wide range of inputs.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::{
    BaseRateSpec, DrawRepayEvent, DrawRepaySpec, RevolvingCredit, RevolvingCreditFees,
    StochasticUtilizationSpec, UtilizationProcess,
};
use proptest::prelude::*;
use time::Month;

/// Helper function to generate deterministic cashflows using the new engine
fn _generate_deterministic_cashflows_replaced(
    facility: &RevolvingCredit,
    as_of: Date,
) -> finstack_quant_core::Result<finstack_quant_cashflows::builder::CashFlowSchedule> {
    use finstack_quant_valuations::instruments::fixed_income::revolving_credit::cashflow_engine::CashflowEngine;
    let engine = CashflowEngine::new(facility, None, as_of, None)?;
    let path_schedule = engine.generate_deterministic()?;
    Ok(path_schedule.schedule)
}

/// Strategy for generating valid dates for revolving credit facilities
#[allow(dead_code)]
fn date_strategy() -> impl Strategy<Value = Date> {
    (2020i32..=2030, 1u8..=12, 1u8..=28).prop_map(|(year, month, day)| {
        Date::from_calendar_date(
            year,
            match month {
                1 => Month::January,
                2 => Month::February,
                3 => Month::March,
                4 => Month::April,
                5 => Month::May,
                6 => Month::June,
                7 => Month::July,
                8 => Month::August,
                9 => Month::September,
                10 => Month::October,
                11 => Month::November,
                _ => Month::December,
            },
            day,
        )
        .unwrap()
    })
}

/// Strategy for generating valid utilization process parameters
#[allow(dead_code)]
fn utilization_process_strategy() -> impl Strategy<Value = UtilizationProcess> {
    (0.1f64..0.9, 0.1f64..5.0, 0.01f64..0.5).prop_map(|(target, speed, vol)| {
        UtilizationProcess::MeanReverting {
            target_rate: target,
            speed,
            volatility: vol,
        }
    })
}

proptest! {
    /// Property: Mean-reverting utilization parameters should always produce valid process
    #[test]
    fn prop_utilization_process_valid_params(
        target_rate in 0.0f64..1.0,
        speed in 0.01f64..10.0,
        volatility in 0.0f64..1.0,
    ) {
        let process = UtilizationProcess::MeanReverting {
            target_rate,
            speed,
            volatility,
        };

        // Process should be constructible
        match process {
            UtilizationProcess::MeanReverting { target_rate: t, speed: s, volatility: v } => {
                prop_assert!((0.0..=1.0).contains(&t), "Target rate should be in [0, 1]");
                prop_assert!(s > 0.0, "Speed should be positive");
                prop_assert!(v >= 0.0, "Volatility should be non-negative");
            }
        }
    }

    /// Property: Facility utilization rate should always be in [0, 1]
    #[test]
    fn prop_utilization_rate_bounded(
        commitment in 1_000_000.0f64..100_000_000.0,
        utilization_pct in 0.0f64..1.0,
    ) {
        let drawn = commitment * utilization_pct;

        let facility = RevolvingCredit::builder()
            .id("TEST".into())
            .commitment_amount(Money::new(commitment, Currency::USD))
            .drawn_amount(Money::new(drawn, Currency::USD))
            .commitment_date(Date::from_calendar_date(2025, Month::January, 1).unwrap())
            .maturity(Date::from_calendar_date(2026, Month::January, 1).unwrap())
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::default())
            .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
            .discount_curve_id("USD-OIS".into())
            .build()
            .unwrap();

        let util_rate = facility.utilization_rate();

        prop_assert!((0.0..=1.0).contains(&util_rate),
            "Utilization rate {} should be in [0, 1]", util_rate);
        // Allow for floating point precision errors
        prop_assert!((util_rate - utilization_pct).abs() < 1e-6,
            "Utilization rate {} should approximately match target {}", util_rate, utilization_pct);
    }

    /// Property: Undrawn amount should equal commitment - drawn
    #[test]
    fn prop_undrawn_amount_arithmetic(
        commitment in 1_000_000.0f64..100_000_000.0,
        utilization_pct in 0.0f64..1.0,
    ) {
        let drawn = commitment * utilization_pct;

        let facility = RevolvingCredit::builder()
            .id("TEST".into())
            .commitment_amount(Money::new(commitment, Currency::USD))
            .drawn_amount(Money::new(drawn, Currency::USD))
            .commitment_date(Date::from_calendar_date(2025, Month::January, 1).unwrap())
            .maturity(Date::from_calendar_date(2026, Month::January, 1).unwrap())
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::default())
            .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
            .discount_curve_id("USD-OIS".into())
            .build()
            .unwrap();

        let undrawn = facility.undrawn_amount().unwrap();
        let expected_undrawn = commitment - drawn;

        // Allow for floating point precision errors
        prop_assert!((undrawn.amount() - expected_undrawn).abs() < 0.01,
            "Undrawn {} should approximately equal commitment - drawn {}",
            undrawn.amount(), expected_undrawn);
        prop_assert!(undrawn.amount() >= -1e-6,
            "Undrawn amount should be non-negative (allowing for rounding)");
    }

    /// Property: Draw events should increase drawn balance, repay events should decrease it
    #[test]
    fn prop_draw_repay_balance_consistency(
        initial_drawn in 1_000_000.0f64..5_000_000.0,
        draw_amount in 100_000.0f64..1_000_000.0,
    ) {
        let commitment = 10_000_000.0;
        let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
        let end = Date::from_calendar_date(2026, Month::January, 1).unwrap();
        let draw_date = Date::from_calendar_date(2025, Month::March, 1).unwrap();

        // Create facility with a draw event
        let facility = RevolvingCredit::builder()
            .id("TEST".into())
            .commitment_amount(Money::new(commitment, Currency::USD))
            .drawn_amount(Money::new(initial_drawn, Currency::USD))
            .commitment_date(start)
            .maturity(end)
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::default())
            .draw_repay_spec(DrawRepaySpec::Deterministic(vec![
                DrawRepayEvent {
                    date: draw_date,
                    amount: Money::new(draw_amount, Currency::USD),
                    is_draw: true,
                },
            ]))
            .discount_curve_id("USD-OIS".into())
            .build()
            .unwrap();

        // Calculate balance after draw
        use finstack_quant_valuations::instruments::fixed_income::revolving_credit::cashflow_engine::calculate_drawn_balance_at_date;
        let balance_after = calculate_drawn_balance_at_date(&facility, draw_date).unwrap();

        let expected_balance = initial_drawn + draw_amount;

        // Allow for floating point precision errors
        prop_assert!((balance_after.amount() - expected_balance).abs() < 0.01,
            "Balance {} should approximately equal initial + draw {}",
            balance_after.amount(), expected_balance);
        prop_assert!(balance_after.amount() <= commitment + 1e-6,
            "Balance should not exceed commitment");
    }

    /// Property: Cashflows should be ordered by date
    #[test]
    fn prop_cashflows_date_ordering(
        initial_drawn in 1_000_000.0f64..5_000_000.0,
    ) {
        let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
        let end = Date::from_calendar_date(2026, Month::January, 1).unwrap();

        let facility = RevolvingCredit::builder()
            .id("TEST".into())
            .commitment_amount(Money::new(10_000_000.0, Currency::USD))
            .drawn_amount(Money::new(initial_drawn, Currency::USD))
            .commitment_date(start)
            .maturity(end)
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::default())
            .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
            .discount_curve_id("USD-OIS".into())
            .build()
            .unwrap();

        let schedule = _generate_deterministic_cashflows_replaced(
            &facility,
            start,
        ).unwrap();

        // Check that cashflows are ordered by date
        for i in 1..schedule.flows.len() {
            prop_assert!(schedule.flows[i].date >= schedule.flows[i-1].date,
                "Cashflows should be ordered by date");
        }
    }

    /// Property: Fee rates should produce non-negative fee amounts
    #[test]
    fn prop_fees_non_negative(
        commitment_fee_bp in 0.0f64..100.0,
        usage_fee_bp in 0.0f64..100.0,
        facility_fee_bp in 0.0f64..100.0,
    ) {
        let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
        let end = Date::from_calendar_date(2026, Month::January, 1).unwrap();

        let facility = RevolvingCredit::builder()
            .id("TEST".into())
            .commitment_amount(Money::new(10_000_000.0, Currency::USD))
            .drawn_amount(Money::new(5_000_000.0, Currency::USD))
            .commitment_date(start)
            .maturity(end)
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::flat(commitment_fee_bp, usage_fee_bp, facility_fee_bp))
            .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
            .discount_curve_id("USD-OIS".into())
            .build()
            .unwrap();

        let schedule = _generate_deterministic_cashflows_replaced(
            &facility,
            start,
        ).unwrap();

        // All fee cashflows should be non-negative (from lender perspective, fees are paid by borrower)
        for cf in &schedule.flows {
            if cf.kind == finstack_quant_core::cashflow::CFKind::Fee {
                prop_assert!(cf.amount.amount() >= 0.0,
                    "Fee cashflows should be non-negative");
            }
        }
    }

    /// Property: Stochastic utilization spec should have valid parameters
    #[test]
    fn prop_stochastic_spec_valid(
        target_rate in 0.1f64..0.9,
        speed in 0.1f64..5.0,
        volatility in 0.01f64..0.5,
        num_paths in 100usize..10000,
    ) {
        let process = UtilizationProcess::MeanReverting {
            target_rate,
            speed,
            volatility,
        };

        let spec = StochasticUtilizationSpec {
            utilization_process: process,
            num_paths,
            seed: Some(42),
            antithetic: false,
            use_sobol_qmc: false,

            mc_config: None,
        };

        // Spec should be valid
        prop_assert!(spec.num_paths > 0, "Number of paths should be positive");
        prop_assert!(spec.seed.is_some(), "Seed should be set for reproducibility");

        match spec.utilization_process {
            UtilizationProcess::MeanReverting { target_rate: t, .. } => {
                prop_assert!((0.0..=1.0).contains(&t), "Target rate should be in [0, 1]");
            }
        }
    }
}

#[cfg(test)]
mod deterministic_tests {
    use super::*;

    #[test]
    fn test_zero_commitment_invalid() {
        // Zero commitment should still be constructible but have zero utilization
        let facility = RevolvingCredit::builder()
            .id("ZERO".into())
            .commitment_amount(Money::new(0.0, Currency::USD))
            .drawn_amount(Money::new(0.0, Currency::USD))
            .commitment_date(Date::from_calendar_date(2025, Month::January, 1).unwrap())
            .maturity(Date::from_calendar_date(2026, Month::January, 1).unwrap())
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::default())
            .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
            .discount_curve_id("USD-OIS".into())
            .build()
            .unwrap();

        assert_eq!(facility.utilization_rate(), 0.0);
    }

    #[test]
    fn test_full_utilization() {
        let commitment = 10_000_000.0;
        let facility = RevolvingCredit::builder()
            .id("FULL".into())
            .commitment_amount(Money::new(commitment, Currency::USD))
            .drawn_amount(Money::new(commitment, Currency::USD))
            .commitment_date(Date::from_calendar_date(2025, Month::January, 1).unwrap())
            .maturity(Date::from_calendar_date(2026, Month::January, 1).unwrap())
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::default())
            .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
            .discount_curve_id("USD-OIS".into())
            .build()
            .unwrap();

        assert!((facility.utilization_rate() - 1.0).abs() < 1e-10);
        assert_eq!(facility.undrawn_amount().unwrap().amount(), 0.0);
    }
}
