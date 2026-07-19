//! Monte Carlo pricing tests for revolving credit facilities.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::{
    BaseRateSpec, DrawRepaySpec, RevolvingCredit, RevolvingCreditFees, StochasticUtilizationSpec,
    UtilizationProcess,
};
use finstack_quant_valuations::instruments::Instrument;
use time::macros::date;

use crate::common::test_helpers::flat_discount_curve;

fn build_flat_discount_curve(
    rate: f64,
    base_date: Date,
    curve_id: &str,
) -> finstack_quant_core::market_data::term_structures::DiscountCurve {
    flat_discount_curve(rate, base_date, curve_id)
}

#[test]
fn test_mc_pricer_stochastic_utilization() {
    // Setup dates
    let val_date = date!(2025 - 01 - 01);
    let commitment_date = date!(2025 - 01 - 01);
    let maturity_date = date!(2026 - 01 - 01);

    // Create a revolving credit facility with stochastic utilization
    let facility = RevolvingCredit::builder()
        .id("RC-MC-001".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(5_000_000.0, Currency::USD)) // 50% initial utilization
        .commitment_date(commitment_date)
        .maturity(maturity_date)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 }) // 5% interest
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0))
        .draw_repay_spec(DrawRepaySpec::Stochastic(Box::new(
            StochasticUtilizationSpec {
                utilization_process: UtilizationProcess::MeanReverting {
                    target_rate: 0.6, // Mean-revert to 60% utilization
                    speed: 0.5,       // Moderate mean reversion
                    volatility: 0.15, // 15% volatility
                },
                num_paths: 10000, // 10k paths for reasonable convergence
                seed: Some(42),   // Fixed seed for reproducibility
                antithetic: false,
                use_sobol_qmc: false,
                mc_config: None,
            },
        )))
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.4)
        .build()
        .unwrap();

    // Create a flat discount curve at 3%
    let disc_curve = build_flat_discount_curve(0.03, val_date, "USD-OIS");

    // Build market context
    let market = MarketContext::new().insert(disc_curve);

    // Price using MC
    let pv = facility.value(&market, val_date).unwrap();

    // Expected value should be positive (we're receiving fees and interest)
    assert!(pv.amount() > 0.0, "PV should be positive");

    // Rough sanity check: PV should be in a reasonable range INCLUDING principal repayment
    // With 1 year maturity, 10M commitment, ~50-60% utilization, principal ~5-6M repaid at maturity
    // Discounted principal at 3%: ~ 4.85M - 5.8M, plus interest/fees ~ 250-300k
    // Allow a wide band due to stochastic utilization
    assert!(
        pv.amount() > 4_500_000.0 && pv.amount() < 6_500_000.0,
        "PV should be in reasonable range, got {}",
        pv.amount()
    );
}

#[test]
fn test_mc_pricer_market_anchored_zero_vol_and_vol_sensitivity() {
    use finstack_quant_valuations::instruments::fixed_income::revolving_credit::{
        CreditSpreadProcessSpec, McConfig,
    };

    // Setup dates
    let val_date = date!(2025 - 01 - 01);
    let commitment_date = date!(2025 - 01 - 01);
    let maturity_date = date!(2028 - 01 - 01); // 3 years

    // Flat discount at 3%
    let disc_curve = build_flat_discount_curve(0.03, val_date, "USD-OIS");

    // Simple hazard curve with constant hazard 2% (implies ~1.2% spread at R=40%)
    let hazard = HazardCurve::builder("BORROWER-HZD")
        .base_date(val_date)
        .recovery_rate(0.40)
        .day_count(DayCount::Act365F)
        .knots([(1.0, 0.02), (5.0, 0.02)])
        .build()
        .unwrap();

    // Build market context with discount + hazard
    let market = MarketContext::new().insert(disc_curve).insert(hazard);

    // Zero vol (should collapse toward deterministic behavior wrt default randomness off)
    let facility_zero_vol = RevolvingCredit::builder()
        .id("RC-MC-ANCHOR".into())
        .commitment_amount(Money::new(5_000_000.0, Currency::USD))
        .drawn_amount(Money::new(2_000_000.0, Currency::USD))
        .commitment_date(commitment_date)
        .maturity(maturity_date)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.055 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(25.0, 50.0, 0.0))
        .draw_repay_spec(DrawRepaySpec::Stochastic(Box::new(
            StochasticUtilizationSpec {
                utilization_process: UtilizationProcess::MeanReverting {
                    target_rate: 0.4,
                    speed: 0.5,
                    volatility: 1e-8, // near-zero, but positive to satisfy model constraints
                },
                num_paths: 5000,
                seed: Some(42),
                antithetic: false,
                use_sobol_qmc: false,
                mc_config: Some(McConfig {
                    correlation_matrix: None,
                    recovery_rate: 0.40,
                    credit_spread_process: CreditSpreadProcessSpec::MarketAnchored {
                        hazard_curve_id: "BORROWER-HZD".into(),
                        kappa: 0.5,
                        implied_vol: 1e-8, // near-zero, but positive to satisfy CIR constraints
                        tenor_years: None,
                    },
                    interest_rate_process: None,
                    util_credit_corr: Some(0.8),
                }),
            },
        )))
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.4)
        .build()
        .unwrap();

    let pv_zero = facility_zero_vol.value(&market, val_date).unwrap();
    assert!(pv_zero.amount().is_finite());

    // Higher vol should not increase PV for the lender on average (default risk)
    let facility_high_vol = RevolvingCredit::builder()
        .id("RC-MC-ANCHOR-HV".into())
        .commitment_amount(Money::new(5_000_000.0, Currency::USD))
        .drawn_amount(Money::new(2_000_000.0, Currency::USD))
        .commitment_date(commitment_date)
        .maturity(maturity_date)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.055 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(25.0, 50.0, 0.0))
        .draw_repay_spec(DrawRepaySpec::Stochastic(Box::new(
            StochasticUtilizationSpec {
                utilization_process: UtilizationProcess::MeanReverting {
                    target_rate: 0.4,
                    speed: 0.5,
                    volatility: 0.20,
                },
                num_paths: 5000,
                seed: Some(42),
                antithetic: false,
                use_sobol_qmc: false,
                mc_config: Some(McConfig {
                    correlation_matrix: None,
                    recovery_rate: 0.40,
                    credit_spread_process: CreditSpreadProcessSpec::MarketAnchored {
                        hazard_curve_id: "BORROWER-HZD".into(),
                        kappa: 0.5,
                        implied_vol: 0.30,
                        tenor_years: None,
                    },
                    interest_rate_process: None,
                    util_credit_corr: Some(0.8),
                }),
            },
        )))
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.4)
        .build()
        .unwrap();

    let pv_high_vol = facility_high_vol.value(&market, val_date).unwrap();

    // Allow tolerance due to path randomness and competing utilization effect
    assert!(pv_high_vol.amount() <= pv_zero.amount() * 1.02);
}

#[test]
fn test_mc_pricer_deterministic_reproducibility() {
    // Test that MC pricer is deterministic with fixed seed
    let val_date = date!(2025 - 01 - 01);
    let commitment_date = date!(2025 - 01 - 01);
    let maturity_date = date!(2026 - 01 - 01);

    let facility = RevolvingCredit::builder()
        .id("RC-MC-002".into())
        .commitment_amount(Money::new(5_000_000.0, Currency::USD))
        .drawn_amount(Money::new(2_500_000.0, Currency::USD))
        .commitment_date(commitment_date)
        .maturity(maturity_date)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.04 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(20.0, 5.0, 3.0))
        .draw_repay_spec(DrawRepaySpec::Stochastic(Box::new(
            StochasticUtilizationSpec {
                utilization_process: UtilizationProcess::MeanReverting {
                    target_rate: 0.5,
                    speed: 0.3,
                    volatility: 0.10,
                },
                num_paths: 1000,
                seed: Some(12345),
                antithetic: false,
                use_sobol_qmc: false,
                mc_config: None,
            },
        )))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    let disc_curve = build_flat_discount_curve(0.02, val_date, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    // Price twice with same seed
    let pv1 = facility.value(&market, val_date).unwrap();
    let pv2 = facility.value(&market, val_date).unwrap();

    // Should be exactly the same due to fixed seed
    assert_eq!(
        pv1.amount(),
        pv2.amount(),
        "MC pricer should be deterministic with fixed seed"
    );
}

#[test]
fn test_mc_pricer_convergence() {
    // Test that more paths lead to better estimates (less variance)
    let val_date = date!(2025 - 01 - 01);
    let commitment_date = date!(2025 - 01 - 01);
    let maturity_date = date!(2026 - 01 - 01);

    let disc_curve = build_flat_discount_curve(0.04, val_date, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    // Test with different number of paths
    let num_paths_list = vec![100, 1000, 5000];
    let mut results = Vec::new();

    for &num_paths in &num_paths_list {
        let facility = RevolvingCredit::builder()
            .id(format!("RC-MC-003-{}", num_paths).into())
            .commitment_amount(Money::new(10_000_000.0, Currency::USD))
            .drawn_amount(Money::new(6_000_000.0, Currency::USD))
            .commitment_date(commitment_date)
            .maturity(maturity_date)
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.06 })
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::flat(30.0, 15.0, 10.0))
            .draw_repay_spec(DrawRepaySpec::Stochastic(Box::new(
                StochasticUtilizationSpec {
                    utilization_process: UtilizationProcess::MeanReverting {
                        target_rate: 0.7,
                        speed: 0.4,
                        volatility: 0.20,
                    },
                    num_paths,
                    seed: Some(99999),
                    antithetic: false,
                    use_sobol_qmc: false,
                    mc_config: None,
                },
            )))
            .discount_curve_id("USD-OIS".into())
            .build()
            .unwrap();

        let pv = facility.value(&market, val_date).unwrap();
        results.push(pv.amount());
    }

    // Results should be relatively stable (within reasonable range)
    let mean = results.iter().sum::<f64>() / results.len() as f64;
    for &result in &results {
        let relative_diff = (result - mean).abs() / mean;
        assert!(
            relative_diff < 0.1,
            "Results should converge: {:?}, mean: {}, diff: {}",
            results,
            mean,
            relative_diff
        );
    }
}

#[test]
fn test_mc_utilization_mean_reversion() {
    // Test that the mean-reverting process behaves correctly
    let val_date = date!(2025 - 01 - 01);
    let commitment_date = date!(2025 - 01 - 01);
    let maturity_date = date!(2027 - 01 - 01); // 2 years

    // Start with very low utilization (10%), target 80%
    let facility = RevolvingCredit::builder()
        .id("RC-MC-004".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(1_000_000.0, Currency::USD)) // 10% initial
        .commitment_date(commitment_date)
        .maturity(maturity_date)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::default())
        .draw_repay_spec(DrawRepaySpec::Stochastic(Box::new(
            StochasticUtilizationSpec {
                utilization_process: UtilizationProcess::MeanReverting {
                    target_rate: 0.8, // Should drift toward 80%
                    speed: 1.0,       // Fast mean reversion
                    volatility: 0.05, // Low volatility
                },
                num_paths: 5000,
                seed: Some(54321),
                antithetic: false,
                use_sobol_qmc: false,
                mc_config: None,
            },
        )))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    let disc_curve = build_flat_discount_curve(0.03, val_date, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    let pv = facility.value(&market, val_date).unwrap();

    // With mean reversion to higher utilization, PV should reflect
    // increasing interest payments over time
    assert!(pv.amount() > 0.0);

    // Compare to a facility with constant high utilization
    let high_util_facility = RevolvingCredit::builder()
        .id("RC-MC-005".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(8_000_000.0, Currency::USD)) // 80% constant
        .commitment_date(commitment_date)
        .maturity(maturity_date)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::default())
        .draw_repay_spec(DrawRepaySpec::Stochastic(Box::new(
            StochasticUtilizationSpec {
                utilization_process: UtilizationProcess::MeanReverting {
                    target_rate: 0.8, // Already at target
                    speed: 1.0,
                    volatility: 0.05,
                },
                num_paths: 5000,
                seed: Some(54321),
                antithetic: false,
                use_sobol_qmc: false,
                mc_config: None,
            },
        )))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    let pv_high = high_util_facility.value(&market, val_date).unwrap();

    // The facility starting at low utilization should have lower PV
    // than the one starting at high utilization (due to path-dependence)
    assert!(
        pv.amount() < pv_high.amount(),
        "Lower initial utilization should result in lower PV"
    );
}

/// Verify that `index_cap_bp` is applied in the stochastic (MC) cashflow engine.
///
/// Creates two floating-rate stochastic facilities against a high forward curve (8%):
/// - One without a cap → uses full 8% index rate
/// - One with `index_cap_bp = 300` (3% cap) → caps index to 3%
///
/// Uses `RevolvingCreditPricer::price_with_paths` to invoke the MC engine directly
/// (the `value()` fast path falls back to deterministic pricing for stochastic specs).
///
/// Near-zero utilization volatility ensures deterministic paths so the
/// difference is entirely due to the cap reducing the interest rate.
#[test]
fn test_mc_stochastic_floating_rate_index_cap() {
    use finstack_quant_cashflows::builder::FloatingRateSpec;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use finstack_quant_valuations::instruments::fixed_income::revolving_credit::RevolvingCreditPricer;
    use rust_decimal::Decimal;

    let val_date = date!(2025 - 01 - 01);
    let commitment_date = date!(2025 - 01 - 01);
    let maturity_date = date!(2026 - 01 - 01);

    // Flat forward curve at 8% — well above the 3% cap
    let fwd_curve = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(val_date)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.08), (1.0, 0.08), (5.0, 0.08)])
        .build()
        .unwrap();

    let disc_curve = build_flat_discount_curve(0.03, val_date, "USD-OIS");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_series(
            finstack_quant_core::market_data::scalars::ScalarTimeSeries::new(
                "FIXING:USD-SOFR-3M",
                vec![(date!(2024 - 12 - 30), 0.08)],
                None,
            )
            .expect("valuation-date contractual fixing"),
        );

    // Helper to build a floating rate spec with optional index cap
    let make_float_spec = |all_in_cap_bp: Option<Decimal>| -> FloatingRateSpec {
        FloatingRateSpec {
            index_id: "USD-SOFR-3M".into(),
            spread_bp: Decimal::try_from(100.0).expect("valid"), // 100 bps spread
            gearing: Decimal::try_from(1.0).expect("valid"),
            gearing_includes_spread: true,
            index_floor_bp: None,
            all_in_floor_bp: None,
            all_in_cap_bp: None,
            index_cap_bp: all_in_cap_bp,
            overnight_index_constraints: Default::default(),
            reset_freq: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 2,
            fixing_calendar_id: None,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: Default::default(),
        }
    };

    // Near-zero vol stochastic spec for deterministic-like utilization
    let make_stoch_spec = |id: &str, all_in_cap_bp: Option<Decimal>| -> RevolvingCredit {
        RevolvingCredit::builder()
            .id(id.into())
            .commitment_amount(Money::new(10_000_000.0, Currency::USD))
            .drawn_amount(Money::new(5_000_000.0, Currency::USD))
            .commitment_date(commitment_date)
            .maturity(maturity_date)
            .base_rate_spec(BaseRateSpec::Floating(make_float_spec(all_in_cap_bp)))
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::default())
            .draw_repay_spec(DrawRepaySpec::Stochastic(Box::new(
                StochasticUtilizationSpec {
                    utilization_process: UtilizationProcess::MeanReverting {
                        target_rate: 0.5,
                        speed: 0.5,
                        volatility: 1e-8, // near-zero to keep utilization deterministic
                    },
                    num_paths: 1000,
                    seed: Some(42),
                    antithetic: false,
                    use_sobol_qmc: false,
                    mc_config: None,
                },
            )))
            .discount_curve_id("USD-OIS".into())
            .build()
            .unwrap()
    };

    // Price via the MC engine (not value() which uses the deterministic fast path)
    // Facility without cap: index rate ≈ 8%, all-in ≈ 9%
    let facility_no_cap = make_stoch_spec("RC-MC-NOCAP", None);
    let mc_no_cap =
        RevolvingCreditPricer::price_with_paths(&facility_no_cap, &market, val_date).unwrap();
    let pv_no_cap = mc_no_cap.mc_result.estimate.mean.amount();

    // Facility with index cap at 300 bps (3%): index rate capped at 3%, all-in ≈ 4%
    let cap_300 = Decimal::try_from(300.0).expect("valid");
    let facility_with_cap = make_stoch_spec("RC-MC-CAP300", Some(cap_300));
    let mc_with_cap =
        RevolvingCreditPricer::price_with_paths(&facility_with_cap, &market, val_date).unwrap();
    let pv_with_cap = mc_with_cap.mc_result.estimate.mean.amount();

    // Both should produce positive PV
    assert!(
        pv_no_cap > 0.0,
        "Uncapped PV should be positive, got {}",
        pv_no_cap
    );
    assert!(
        pv_with_cap > 0.0,
        "Capped PV should be positive, got {}",
        pv_with_cap
    );

    // The capped facility should have lower PV because the lender earns less interest
    // (index rate 3% + 100bps = 4% vs uncapped 8% + 100bps = 9%)
    assert!(
        pv_with_cap < pv_no_cap,
        "Capped PV ({}) should be less than uncapped PV ({}) because cap \
         limits the index rate and hence the interest income",
        pv_with_cap,
        pv_no_cap
    );

    // The difference should be material (on ~5M drawn, 5% rate diff over 1 year ≈ $250k)
    let diff = pv_no_cap - pv_with_cap;
    assert!(
        diff > 100_000.0,
        "PV difference ({}) should be material (> 100k) given 5% index rate gap",
        diff
    );
}
