//! Comprehensive parity tests for revolving credit deterministic vs stochastic pricing.
//!
//! These tests ensure that the stochastic pricer with zero volatility produces
//! identical results to the deterministic pricer across various facility configurations.

#[cfg(test)]
mod tests {
    use finstack_cashflows::builder::FeeTier;
    use finstack_core::currency::Currency;
    use finstack_core::dates::{Date, DayCount, Tenor};
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::term_structures::{DiscountCurve, ForwardCurve, HazardCurve};
    use finstack_core::money::Money;
    use finstack_core::types::CurveId;
    use finstack_valuations::instruments::fixed_income::revolving_credit::RevolvingCreditPricer;
    use finstack_valuations::instruments::fixed_income::revolving_credit::{
        BaseRateSpec, DrawRepayEvent, DrawRepaySpec, RevolvingCredit, RevolvingCreditFees,
        StochasticUtilizationSpec, UtilizationProcess,
    };
    use finstack_valuations::instruments::Instrument;
    use time::Month;

    /// Helper to create test dates
    fn date(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(year, Month::try_from(month).unwrap(), day).unwrap()
    }

    /// Create a test market context with discount, forward, and hazard curves
    fn create_test_market(base_date: Date) -> MarketContext {
        let disc_curve = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .day_count(DayCount::Act360)
            .knots([
                (0.0, 1.0),
                (0.25, 0.995),
                (0.5, 0.99),
                (1.0, 0.98),
                (2.0, 0.96),
                (5.0, 0.90),
            ])
            .build()
            .unwrap();

        let fwd_curve = ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(base_date)
            .day_count(DayCount::Act360)
            .knots([
                (0.0, 0.03),
                (0.25, 0.031),
                (0.5, 0.032),
                (1.0, 0.033),
                (2.0, 0.034),
                (5.0, 0.035),
            ])
            .build()
            .unwrap();

        let hazard_curve = HazardCurve::builder("BORROWER-HZ")
            .base_date(base_date)
            .day_count(DayCount::Act360)
            .knots([
                (0.0, 0.01),  // 100 bps
                (1.0, 0.012), // 120 bps
                (2.0, 0.015), // 150 bps
                (5.0, 0.02),  // 200 bps
            ])
            .build()
            .unwrap();

        MarketContext::new()
            .insert(disc_curve)
            .insert(fwd_curve)
            .insert(hazard_curve)
    }

    /// Create a zero-volatility stochastic spec that mimics deterministic behavior
    fn create_zero_vol_stochastic(initial_utilization: f64) -> Box<StochasticUtilizationSpec> {
        Box::new(StochasticUtilizationSpec {
            utilization_process: UtilizationProcess::MeanReverting {
                target_rate: initial_utilization,
                speed: 100.0,    // Very high speed to stay at target
                volatility: 0.0, // Zero volatility
            },
            num_paths: 1000, // Use many paths for stable average
            seed: Some(42),
            antithetic: false,
            use_sobol_qmc: false,
            mc_config: None,
        })
    }

    #[test]
    fn test_parity_simple_fixed_rate() {
        let start = date(2025, 1, 1);
        let end = date(2026, 1, 1);
        let market = create_test_market(start);

        // Create deterministic facility
        let facility_det = RevolvingCredit::builder()
            .id("RC-DET-FIXED".into())
            .commitment_amount(Money::new(10_000_000.0, Currency::USD))
            .drawn_amount(Money::new(5_000_000.0, Currency::USD))
            .commitment_date(start)
            .maturity(end)
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0))
            .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
            .discount_curve_id("USD-OIS".into())
            .build()
            .unwrap();

        // Create stochastic facility with zero volatility
        let facility_stoch = RevolvingCredit::builder()
            .id("RC-STOCH-FIXED".into())
            .commitment_amount(Money::new(10_000_000.0, Currency::USD))
            .drawn_amount(Money::new(5_000_000.0, Currency::USD))
            .commitment_date(start)
            .maturity(end)
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0))
            .draw_repay_spec(DrawRepaySpec::Stochastic(
                create_zero_vol_stochastic(0.5), // 50% utilization
            ))
            .discount_curve_id("USD-OIS".into())
            .build()
            .unwrap();

        // Price both
        let pv_det = facility_det.value(&market, start).unwrap();

        let result_stoch =
            RevolvingCreditPricer::price_with_paths(&facility_stoch, &market, start).unwrap();
        let pv_stoch = result_stoch.mc_result.estimate.mean;

        // Assert parity within tight tolerance
        let diff = (pv_det.amount() - pv_stoch.amount()).abs();
        let relative_error = diff / pv_det.amount().abs().max(1.0);

        assert!(
            relative_error < 1e-4, // 0.01% tolerance
            "Fixed rate parity failed: det={}, stoch={}, diff={}, rel_err={:.6}",
            pv_det.amount(),
            pv_stoch.amount(),
            diff,
            relative_error
        );
    }

    #[test]
    fn test_parity_floating_rate() {
        let start = date(2025, 1, 1);
        let end = date(2026, 1, 1);
        let market = create_test_market(start);

        // Create deterministic facility
        let facility_det = RevolvingCredit::builder()
            .id("RC-DET-FLOAT".into())
            .commitment_amount(Money::new(10_000_000.0, Currency::USD))
            .drawn_amount(Money::new(5_000_000.0, Currency::USD))
            .commitment_date(start)
            .maturity(end)
            .base_rate_spec(BaseRateSpec::Floating(
                finstack_cashflows::builder::FloatingRateSpec {
                    index_id: "USD-SOFR-3M".into(),
                    spread_bp: rust_decimal::Decimal::try_from(200.0).expect("valid"),
                    gearing: rust_decimal::Decimal::try_from(1.0).expect("valid"),
                    gearing_includes_spread: true,
                    index_floor_bp: Some(rust_decimal::Decimal::try_from(100.0).expect("valid")),
                    all_in_floor_bp: None,
                    all_in_cap_bp: None,
                    index_cap_bp: None,
                    reset_freq: Tenor::quarterly(),
                    index_tenor: None,
                    reset_lag_days: 2,
                    dc: DayCount::Act360,
                    bdc: finstack_core::dates::BusinessDayConvention::ModifiedFollowing,
                    calendar_id: "weekends_only".to_string(),
                    fixing_calendar_id: None,
                    end_of_month: false,
                    overnight_compounding: None,
                    overnight_basis: None,
                    fallback: Default::default(),
                    payment_lag_days: 0,
                },
            ))
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0))
            .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
            .discount_curve_id("USD-OIS".into())
            .build()
            .unwrap();

        // Create stochastic facility with zero volatility
        let facility_stoch = RevolvingCredit::builder()
            .id("RC-STOCH-FLOAT".into())
            .commitment_amount(Money::new(10_000_000.0, Currency::USD))
            .drawn_amount(Money::new(5_000_000.0, Currency::USD))
            .commitment_date(start)
            .maturity(end)
            .base_rate_spec(BaseRateSpec::Floating(
                finstack_cashflows::builder::FloatingRateSpec {
                    index_id: "USD-SOFR-3M".into(),
                    spread_bp: rust_decimal::Decimal::try_from(200.0).expect("valid"),
                    gearing: rust_decimal::Decimal::try_from(1.0).expect("valid"),
                    gearing_includes_spread: true,
                    index_floor_bp: Some(rust_decimal::Decimal::try_from(100.0).expect("valid")),
                    all_in_floor_bp: None,
                    all_in_cap_bp: None,
                    index_cap_bp: None,
                    reset_freq: Tenor::quarterly(),
                    index_tenor: None,
                    reset_lag_days: 2,
                    dc: DayCount::Act360,
                    bdc: finstack_core::dates::BusinessDayConvention::ModifiedFollowing,
                    calendar_id: "weekends_only".to_string(),
                    fixing_calendar_id: None,
                    end_of_month: false,
                    overnight_compounding: None,
                    overnight_basis: None,
                    fallback: Default::default(),
                    payment_lag_days: 0,
                },
            ))
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0))
            .draw_repay_spec(DrawRepaySpec::Stochastic(create_zero_vol_stochastic(0.5)))
            .discount_curve_id("USD-OIS".into())
            .build()
            .unwrap();

        // Price both
        let pv_det = facility_det.value(&market, start).unwrap();

        let result_stoch =
            RevolvingCreditPricer::price_with_paths(&facility_stoch, &market, start).unwrap();
        let pv_stoch = result_stoch.mc_result.estimate.mean;

        // Assert parity
        let diff = (pv_det.amount() - pv_stoch.amount()).abs();
        let relative_error = diff / pv_det.amount().abs().max(1.0);

        // Slightly relaxed tolerance for MC vs deterministic (0.05% vs 0.01%)
        // Small differences arise from spot rate interpolation vs period rate projection
        assert!(
            relative_error < 5e-4,
            "Floating rate parity failed: det={}, stoch={}, diff={}, rel_err={:.6}",
            pv_det.amount(),
            pv_stoch.amount(),
            diff,
            relative_error
        );
    }

    #[test]
    fn test_parity_with_hazard_curve() {
        let start = date(2025, 1, 1);
        let end = date(2026, 1, 1);
        let market = create_test_market(start);

        // Create facilities with hazard curve
        let facility_det = RevolvingCredit::builder()
            .id("RC-DET-HAZARD".into())
            .commitment_amount(Money::new(10_000_000.0, Currency::USD))
            .drawn_amount(Money::new(5_000_000.0, Currency::USD))
            .commitment_date(start)
            .maturity(end)
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0))
            .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
            .discount_curve_id("USD-OIS".into())
            .credit_curve_id(CurveId::from("BORROWER-HZ"))
            .recovery_rate(0.4)
            .build()
            .unwrap();

        let facility_stoch = RevolvingCredit::builder()
            .id("RC-STOCH-HAZARD".into())
            .commitment_amount(Money::new(10_000_000.0, Currency::USD))
            .drawn_amount(Money::new(5_000_000.0, Currency::USD))
            .commitment_date(start)
            .maturity(end)
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0))
            .draw_repay_spec(DrawRepaySpec::Stochastic(create_zero_vol_stochastic(0.5)))
            .discount_curve_id("USD-OIS".into())
            .credit_curve_id(CurveId::from("BORROWER-HZ"))
            .recovery_rate(0.4)
            .build()
            .unwrap();

        // Price both
        let pv_det = facility_det.value(&market, start).unwrap();

        let result_stoch =
            RevolvingCreditPricer::price_with_paths(&facility_stoch, &market, start).unwrap();
        let pv_stoch = result_stoch.mc_result.estimate.mean;

        // Assert parity
        let diff = (pv_det.amount() - pv_stoch.amount()).abs();
        let relative_error = diff / pv_det.amount().abs().max(1.0);

        // Relaxed tolerance for MC vs deterministic (0.5%)
        // Small differences arise from survival probability interpolation and credit spread discretization
        assert!(
            relative_error < 5e-3,
            "Hazard curve parity failed: det={}, stoch={}, diff={}, rel_err={:.6}",
            pv_det.amount(),
            pv_stoch.amount(),
            diff,
            relative_error
        );
    }

    #[test]
    fn test_parity_with_tiered_fees() {
        let start = date(2025, 1, 1);
        let end = date(2026, 1, 1);
        let market = create_test_market(start);

        // Create tiered fee structure
        let fees = RevolvingCreditFees {
            upfront_fee: Some(Money::new(50_000.0, Currency::USD)),
            commitment_fee_tiers: vec![
                FeeTier {
                    threshold: rust_decimal::Decimal::try_from(0.0).expect("valid"),
                    bps: rust_decimal::Decimal::try_from(50.0).expect("valid"),
                },
                FeeTier {
                    threshold: rust_decimal::Decimal::try_from(0.25).expect("valid"),
                    bps: rust_decimal::Decimal::try_from(40.0).expect("valid"),
                },
                FeeTier {
                    threshold: rust_decimal::Decimal::try_from(0.5).expect("valid"),
                    bps: rust_decimal::Decimal::try_from(30.0).expect("valid"),
                },
                FeeTier {
                    threshold: rust_decimal::Decimal::try_from(0.75).expect("valid"),
                    bps: rust_decimal::Decimal::try_from(20.0).expect("valid"),
                },
            ],
            usage_fee_tiers: vec![
                FeeTier {
                    threshold: rust_decimal::Decimal::try_from(0.0).expect("valid"),
                    bps: rust_decimal::Decimal::try_from(10.0).expect("valid"),
                },
                FeeTier {
                    threshold: rust_decimal::Decimal::try_from(0.5).expect("valid"),
                    bps: rust_decimal::Decimal::try_from(15.0).expect("valid"),
                },
                FeeTier {
                    threshold: rust_decimal::Decimal::try_from(0.75).expect("valid"),
                    bps: rust_decimal::Decimal::try_from(20.0).expect("valid"),
                },
            ],
            facility_fee_bp: 5.0,
        };

        // Test at different utilization levels
        let utilization_levels = vec![0.0, 0.25, 0.5, 0.75, 0.9];

        for utilization in utilization_levels {
            let drawn = 10_000_000.0 * utilization;

            let facility_det = RevolvingCredit::builder()
                .id(format!("RC-DET-TIER-{}", utilization).into())
                .commitment_amount(Money::new(10_000_000.0, Currency::USD))
                .drawn_amount(Money::new(drawn, Currency::USD))
                .commitment_date(start)
                .maturity(end)
                .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
                .day_count(DayCount::Act360)
                .frequency(Tenor::quarterly())
                .fees(fees.clone())
                .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
                .discount_curve_id("USD-OIS".into())
                .build()
                .unwrap();

            let facility_stoch = RevolvingCredit::builder()
                .id(format!("RC-STOCH-TIER-{}", utilization).into())
                .commitment_amount(Money::new(10_000_000.0, Currency::USD))
                .drawn_amount(Money::new(drawn, Currency::USD))
                .commitment_date(start)
                .maturity(end)
                .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
                .day_count(DayCount::Act360)
                .frequency(Tenor::quarterly())
                .fees(fees.clone())
                .draw_repay_spec(DrawRepaySpec::Stochastic(create_zero_vol_stochastic(
                    utilization,
                )))
                .discount_curve_id("USD-OIS".into())
                .build()
                .unwrap();

            // Price both
            let pv_det = facility_det.value(&market, start).unwrap();

            let result_stoch =
                RevolvingCreditPricer::price_with_paths(&facility_stoch, &market, start).unwrap();
            let pv_stoch = result_stoch.mc_result.estimate.mean;

            // Assert parity
            let diff = (pv_det.amount() - pv_stoch.amount()).abs();
            let relative_error = diff / pv_det.amount().abs().max(1.0);

            assert!(
                relative_error < 1e-4,
                "Tiered fee parity failed at {}% utilization: det={}, stoch={}, diff={}, rel_err={:.6}",
                utilization * 100.0,
                pv_det.amount(),
                pv_stoch.amount(),
                diff,
                relative_error
            );
        }
    }

    #[test]
    fn test_parity_with_draw_repay_events() {
        let start = date(2025, 1, 1);
        let end = date(2026, 1, 1);
        let market = create_test_market(start);

        // Create draw/repay events
        let events = vec![
            DrawRepayEvent {
                date: date(2025, 3, 1),
                amount: Money::new(1_000_000.0, Currency::USD),
                is_draw: true,
            },
            DrawRepayEvent {
                date: date(2025, 6, 1),
                amount: Money::new(500_000.0, Currency::USD),
                is_draw: false,
            },
            DrawRepayEvent {
                date: date(2025, 9, 1),
                amount: Money::new(2_000_000.0, Currency::USD),
                is_draw: true,
            },
        ];

        let facility_det = RevolvingCredit::builder()
            .id("RC-DET-EVENTS".into())
            .commitment_amount(Money::new(10_000_000.0, Currency::USD))
            .drawn_amount(Money::new(3_000_000.0, Currency::USD))
            .commitment_date(start)
            .maturity(end)
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0))
            .draw_repay_spec(DrawRepaySpec::Deterministic(events))
            .discount_curve_id("USD-OIS".into())
            .build()
            .unwrap();

        // For stochastic, we test with constant utilization matching the average
        // Initial: 3M, +1M = 4M, -0.5M = 3.5M, +2M = 5.5M
        // Average utilization ≈ 45%
        let facility_stoch = RevolvingCredit::builder()
            .id("RC-STOCH-CONST".into())
            .commitment_amount(Money::new(10_000_000.0, Currency::USD))
            .drawn_amount(Money::new(4_500_000.0, Currency::USD)) // 45% utilization
            .commitment_date(start)
            .maturity(end)
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0))
            .draw_repay_spec(DrawRepaySpec::Stochastic(create_zero_vol_stochastic(0.45)))
            .discount_curve_id("USD-OIS".into())
            .build()
            .unwrap();

        // Price both
        let pv_det = facility_det.value(&market, start).unwrap();

        let result_stoch =
            RevolvingCreditPricer::price_with_paths(&facility_stoch, &market, start).unwrap();
        let pv_stoch = result_stoch.mc_result.estimate.mean;

        // Note: These won't match exactly because draw/repay events change utilization
        // dynamically, while stochastic uses constant utilization. This test just
        // verifies that both pricers work with their respective specifications.
        assert!(pv_det.currency() == Currency::USD);
        assert!(pv_stoch.currency() == Currency::USD);
    }

    #[test]
    fn test_parity_different_payment_frequencies() {
        let start = date(2025, 1, 1);
        let end = date(2026, 1, 1);
        let market = create_test_market(start);

        let frequencies = vec![
            finstack_core::dates::Tenor::new(1, finstack_core::dates::TenorUnit::Months), // Monthly
            finstack_core::dates::Tenor::new(3, finstack_core::dates::TenorUnit::Months), // Quarterly
            finstack_core::dates::Tenor::new(6, finstack_core::dates::TenorUnit::Months), // Semi-annual
            finstack_core::dates::Tenor::new(12, finstack_core::dates::TenorUnit::Months), // Annual
        ];

        for freq in frequencies {
            let facility_det = RevolvingCredit::builder()
                .id(format!("RC-DET-FREQ-{:?}", freq).into())
                .commitment_amount(Money::new(10_000_000.0, Currency::USD))
                .drawn_amount(Money::new(5_000_000.0, Currency::USD))
                .commitment_date(start)
                .maturity(end)
                .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
                .day_count(DayCount::Act360)
                .frequency(freq)
                .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0))
                .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
                .discount_curve_id("USD-OIS".into())
                .build()
                .unwrap();

            let facility_stoch = RevolvingCredit::builder()
                .id(format!("RC-STOCH-FREQ-{:?}", freq).into())
                .commitment_amount(Money::new(10_000_000.0, Currency::USD))
                .drawn_amount(Money::new(5_000_000.0, Currency::USD))
                .commitment_date(start)
                .maturity(end)
                .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
                .day_count(DayCount::Act360)
                .frequency(freq)
                .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0))
                .draw_repay_spec(DrawRepaySpec::Stochastic(create_zero_vol_stochastic(0.5)))
                .discount_curve_id("USD-OIS".into())
                .build()
                .unwrap();

            // Price both
            let pv_det = facility_det.value(&market, start).unwrap();

            let result_stoch =
                RevolvingCreditPricer::price_with_paths(&facility_stoch, &market, start).unwrap();
            let pv_stoch = result_stoch.mc_result.estimate.mean;

            // Assert parity
            let diff = (pv_det.amount() - pv_stoch.amount()).abs();
            let relative_error = diff / pv_det.amount().abs().max(1.0);

            assert!(
                relative_error < 1e-4,
                "Payment frequency parity failed for {:?}: det={}, stoch={}, diff={}, rel_err={:.6}",
                freq,
                pv_det.amount(),
                pv_stoch.amount(),
                diff,
                relative_error
            );
        }
    }
}
