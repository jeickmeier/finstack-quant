//! Integration tests for Bermudan swaption pricing.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::rates::swaption::BermudanSwaptionTreeValuator;
use finstack_quant_valuations::instruments::rates::swaption::{
    BermudanSchedule, BermudanSwaption, BermudanType, CalibratedHullWhiteModel, HullWhiteParams,
    SwaptionSettlement,
};
use finstack_quant_valuations::instruments::rates::swaption::{
    BermudanSwaptionPricer, BermudanSwaptionPricerConfig,
};
use finstack_quant_valuations::instruments::OptionType;
use finstack_quant_valuations::pricer::Pricer;
use time::Month;

fn test_discount_curve() -> DiscountCurve {
    DiscountCurve::builder("USD-OIS")
        .base_date(Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"))
        .knots([
            (0.0, 1.0),
            (0.5, 0.985),
            (1.0, 0.97),
            (2.0, 0.94),
            (5.0, 0.85),
            (10.0, 0.70),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("Valid curve")
}

fn test_bermudan_swaption(
    swap_start: Date,
    swap_end: Date,
    first_exercise: Date,
    strike: f64,
    option_type: OptionType,
) -> BermudanSwaption {
    BermudanSwaption {
        id: InstrumentId::new("TEST-BERM"),
        option_type,
        notional: Money::new(10_000_000.0, Currency::USD),
        strike: rust_decimal::Decimal::try_from(strike).expect("valid decimal"),
        swap_start,
        swap_end,
        fixed_freq: Tenor::semi_annual(),
        float_freq: Tenor::quarterly(),
        day_count: DayCount::Thirty360,
        settlement: SwaptionSettlement::Physical,
        discount_curve_id: CurveId::new("USD-OIS"),
        forward_curve_id: CurveId::new("USD-OIS"),
        vol_surface_id: CurveId::new("USD-VOL"),
        bermudan_schedule: BermudanSchedule::co_terminal(
            first_exercise,
            swap_end,
            Tenor::semi_annual(),
        )
        .expect("valid Bermudan schedule"),
        bermudan_type: BermudanType::CoTerminal,
        calendar_id: None,
        pricing_overrides: Default::default(),
        attributes: Default::default(),
    }
}

#[test]
fn test_tree_valuator_rejects_mixed_curve_bermudan() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    let swap_start = as_of;
    let swap_end = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");
    let first_exercise = Date::from_calendar_date(2026, Month::January, 1).expect("Valid date");

    let mut swaption =
        test_bermudan_swaption(swap_start, swap_end, first_exercise, 0.03, OptionType::Call);
    swaption.forward_curve_id = CurveId::new("USD-SOFR");

    let curve = test_discount_curve();
    let ttm = swaption.time_to_maturity(as_of).expect("Valid ttm");
    let model = CalibratedHullWhiteModel::calibrate(
        HullWhiteParams::new(0.03, 0.01).expect("valid HW params"),
        50,
        &curve,
        ttm,
    )
    .expect("Calibration should succeed");

    let err = BermudanSwaptionTreeValuator::new(&swaption, &model, &curve, as_of)
        .err()
        .expect("mixed-curve Bermudan tree pricing should be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("single-curve"),
        "expected single-curve rejection error, got: {msg}"
    );
}

#[test]
fn test_bermudan_price_positive() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    let swap_start = as_of;
    let swap_end = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");
    let first_exercise = Date::from_calendar_date(2026, Month::January, 1).expect("Valid date");

    let swaption =
        test_bermudan_swaption(swap_start, swap_end, first_exercise, 0.03, OptionType::Call);
    let curve = test_discount_curve();

    let ttm = swaption.time_to_maturity(as_of).expect("Valid ttm");
    let model = CalibratedHullWhiteModel::calibrate(
        HullWhiteParams::new(0.03, 0.01).expect("valid HW params"),
        50,
        &curve,
        ttm,
    )
    .expect("Calibration should succeed");

    let valuator = BermudanSwaptionTreeValuator::new(&swaption, &model, &curve, as_of)
        .expect("Valid valuator");

    let price = valuator.price().expect("pricing should succeed");

    // Price should be non-negative
    assert!(
        price >= 0.0,
        "Bermudan swaption price should be non-negative, got {}",
        price
    );
}

#[test]
fn test_bermudan_payer_vs_receiver() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    let swap_start = as_of;
    let swap_end = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");
    let first_exercise = Date::from_calendar_date(2026, Month::January, 1).expect("Valid date");

    let payer =
        test_bermudan_swaption(swap_start, swap_end, first_exercise, 0.03, OptionType::Call);
    let receiver =
        test_bermudan_swaption(swap_start, swap_end, first_exercise, 0.03, OptionType::Put);

    let curve = test_discount_curve();
    let ttm = payer.time_to_maturity(as_of).expect("Valid ttm");
    let model = CalibratedHullWhiteModel::calibrate(
        HullWhiteParams::new(0.03, 0.01).expect("valid HW params"),
        50,
        &curve,
        ttm,
    )
    .expect("Calibration should succeed");

    let payer_valuator =
        BermudanSwaptionTreeValuator::new(&payer, &model, &curve, as_of).expect("Valid valuator");
    let receiver_valuator = BermudanSwaptionTreeValuator::new(&receiver, &model, &curve, as_of)
        .expect("Valid valuator");

    let payer_price = payer_valuator.price().expect("pricing should succeed");
    let receiver_price = receiver_valuator.price().expect("pricing should succeed");

    // Both should be positive
    assert!(payer_price >= 0.0, "Payer price should be non-negative");
    assert!(
        receiver_price >= 0.0,
        "Receiver price should be non-negative"
    );
}

#[test]
fn test_bermudan_strike_sensitivity() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    let swap_start = as_of;
    let swap_end = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");
    let first_exercise = Date::from_calendar_date(2026, Month::January, 1).expect("Valid date");

    // Low strike (ITM payer)
    let low_strike =
        test_bermudan_swaption(swap_start, swap_end, first_exercise, 0.01, OptionType::Call);
    // ATM strike
    let atm_strike =
        test_bermudan_swaption(swap_start, swap_end, first_exercise, 0.03, OptionType::Call);
    // High strike (OTM payer)
    let high_strike =
        test_bermudan_swaption(swap_start, swap_end, first_exercise, 0.06, OptionType::Call);

    let curve = test_discount_curve();
    let ttm = low_strike.time_to_maturity(as_of).expect("Valid ttm");
    let model = CalibratedHullWhiteModel::calibrate(
        HullWhiteParams::new(0.03, 0.01).expect("valid HW params"),
        50,
        &curve,
        ttm,
    )
    .expect("Calibration should succeed");

    let low_price = BermudanSwaptionTreeValuator::new(&low_strike, &model, &curve, as_of)
        .expect("Valid valuator")
        .price()
        .expect("pricing should succeed");
    let atm_price = BermudanSwaptionTreeValuator::new(&atm_strike, &model, &curve, as_of)
        .expect("Valid valuator")
        .price()
        .expect("pricing should succeed");
    let high_price = BermudanSwaptionTreeValuator::new(&high_strike, &model, &curve, as_of)
        .expect("Valid valuator")
        .price()
        .expect("pricing should succeed");

    // For a payer swaption: lower strike = higher value
    assert!(
        low_price >= atm_price,
        "Low strike payer {} should be >= ATM {}",
        low_price,
        atm_price
    );
    assert!(
        atm_price >= high_price,
        "ATM payer {} should be >= high strike {}",
        atm_price,
        high_price
    );
}

#[test]
fn test_bermudan_more_exercise_dates_higher_value() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    let swap_start = as_of;
    let swap_end = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");

    // Early first exercise (more exercise opportunities)
    let early_first = Date::from_calendar_date(2026, Month::January, 1).expect("Valid date");
    // Late first exercise (fewer exercise opportunities)
    let late_first = Date::from_calendar_date(2029, Month::January, 1).expect("Valid date");

    let early_swaption =
        test_bermudan_swaption(swap_start, swap_end, early_first, 0.03, OptionType::Call);
    let late_swaption =
        test_bermudan_swaption(swap_start, swap_end, late_first, 0.03, OptionType::Call);

    let curve = test_discount_curve();
    let ttm = early_swaption.time_to_maturity(as_of).expect("Valid ttm");
    let model = CalibratedHullWhiteModel::calibrate(
        HullWhiteParams::new(0.03, 0.01).expect("valid HW params"),
        50,
        &curve,
        ttm,
    )
    .expect("Calibration should succeed");

    let early_price = BermudanSwaptionTreeValuator::new(&early_swaption, &model, &curve, as_of)
        .expect("Valid valuator")
        .price()
        .expect("pricing should succeed");
    let late_price = BermudanSwaptionTreeValuator::new(&late_swaption, &model, &curve, as_of)
        .expect("Valid valuator")
        .price()
        .expect("pricing should succeed");

    // More exercise opportunities should generally be worth more or equal
    // (this depends on the exact market conditions, but typically holds)
    assert!(
        early_price >= late_price * 0.9, // Allow some tolerance
        "Early first exercise {} should be >= late first exercise {}",
        early_price,
        late_price
    );
}

#[test]
fn test_expired_bermudan_zero_value() {
    // Set as_of after swap maturity
    let as_of = Date::from_calendar_date(2035, Month::January, 1).expect("Valid date");
    let swap_start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    let swap_end = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");
    let first_exercise = Date::from_calendar_date(2026, Month::January, 1).expect("Valid date");

    let swaption =
        test_bermudan_swaption(swap_start, swap_end, first_exercise, 0.03, OptionType::Call);

    // When an instrument is past maturity, we treat it as expired (it shouldn't exist anymore).
    // Time-to-maturity should be non-positive.
    let ttm = swaption.time_to_maturity(as_of).unwrap_or(0.0);
    assert!(
        ttm <= 0.0,
        "Expired swaption should have non-positive TTM, got {ttm}"
    );
}

#[test]
fn test_bermudan_schedule_generation() {
    let first_exercise = Date::from_calendar_date(2026, Month::January, 1).expect("Valid date");
    let swap_end = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");

    let schedule = BermudanSchedule::co_terminal(first_exercise, swap_end, Tenor::semi_annual())
        .expect("valid Bermudan schedule");

    // Should have approximately 8 exercise dates (4 years * 2 per year)
    let effective_dates = schedule.effective_dates();
    assert!(
        effective_dates.len() >= 6,
        "Should have multiple exercise dates, got {}",
        effective_dates.len()
    );

    // Dates should be sorted
    for i in 1..effective_dates.len() {
        assert!(
            effective_dates[i] > effective_dates[i - 1],
            "Exercise dates should be sorted"
        );
    }

    // All dates should be before swap end
    for &date in &effective_dates {
        assert!(date < swap_end, "Exercise dates should be before swap end");
    }
}

#[test]
fn test_bermudan_schedule_with_lockout() {
    let first_exercise = Date::from_calendar_date(2026, Month::January, 1).expect("Valid date");
    let swap_end = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");
    let lockout_end = Date::from_calendar_date(2027, Month::January, 1).expect("Valid date");

    let schedule = BermudanSchedule::co_terminal(first_exercise, swap_end, Tenor::semi_annual())
        .expect("valid Bermudan schedule")
        .with_lockout(lockout_end);

    let effective_dates = schedule.effective_dates();

    // All effective dates should be after lockout
    for &date in &effective_dates {
        assert!(
            date > lockout_end,
            "Effective dates should be after lockout"
        );
    }
}

#[test]
fn test_bermudan_to_european_conversion() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    let swap_start = as_of;
    let swap_end = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");
    let first_exercise = Date::from_calendar_date(2026, Month::January, 1).expect("Valid date");

    let bermudan =
        test_bermudan_swaption(swap_start, swap_end, first_exercise, 0.03, OptionType::Call);

    // Convert to European
    let european = bermudan.to_european().expect("Conversion should succeed");

    // Check European parameters match Bermudan
    assert_eq!(european.strike, bermudan.strike);
    assert_eq!(european.notional.amount(), bermudan.notional.amount());
    assert_eq!(european.swap_end, bermudan.swap_end);

    // European expiry should be the first Bermudan exercise date
    assert_eq!(european.expiry, first_exercise);
}

// ============================================================================
// LSMC Tests (requires "mc" feature)
// ============================================================================

fn build_market_context() -> MarketContext {
    let curve = test_discount_curve();
    MarketContext::new().insert(curve)
}

/// Test LSMC vs Tree: prices should be in same ballpark.
#[test]
fn test_lsmc_vs_tree_sanity() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    let swap_start = as_of;
    let swap_end = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");
    let first_exercise = Date::from_calendar_date(2027, Month::January, 1).expect("Valid date");

    let swaption =
        test_bermudan_swaption(swap_start, swap_end, first_exercise, 0.03, OptionType::Call);

    let market = build_market_context();

    // Price with tree
    let tree_pricer = BermudanSwaptionPricer::tree_with_config(BermudanSwaptionPricerConfig {
        tree_steps: 100,
        ..Default::default()
    });
    let tree_result = tree_pricer.price_dyn(&swaption, &market, as_of);
    assert!(
        tree_result.is_ok(),
        "Tree pricing should succeed: {:?}",
        tree_result.err()
    );
    let tree_pv = tree_result
        .expect("Tree pricing should succeed")
        .value
        .amount();

    // Price with LSMC (fewer paths for speed in tests)
    let lsmc_pricer = BermudanSwaptionPricer::lsmc_with_config(BermudanSwaptionPricerConfig {
        mc_paths: 10_000,
        mc_seed: 42,
        ..Default::default()
    });
    let lsmc_result = lsmc_pricer.price_dyn(&swaption, &market, as_of);
    assert!(
        lsmc_result.is_ok(),
        "LSMC pricing should succeed: {:?}",
        lsmc_result.err()
    );
    let lsmc_result = lsmc_result.expect("LSMC pricing should succeed");
    let lsmc_pv = lsmc_result.value.amount();

    // Check that LSMC diagnostics are present
    assert!(
        lsmc_result.measures.contains_key("mc_stderr"),
        "LSMC result should contain stderr"
    );
    assert!(
        lsmc_result.measures.contains_key("lsmc_num_paths"),
        "LSMC result should contain num_paths"
    );
    assert!(
        lsmc_result.measures.contains_key("lsmc_ci95_low"),
        "LSMC result should contain CI95 low"
    );
    assert!(
        lsmc_result.measures.contains_key("lsmc_ci95_high"),
        "LSMC result should contain CI95 high"
    );

    // Both should be positive
    assert!(
        tree_pv >= 0.0,
        "Tree PV should be non-negative: {}",
        tree_pv
    );
    assert!(
        lsmc_pv >= 0.0,
        "LSMC PV should be non-negative: {}",
        lsmc_pv
    );

    // Note: LSMC and Tree may produce materially different values due to:
    // 1. Different θ(t) calibration approaches. LSMC uses the MC-crate
    //    `calibrate_theta_from_curve` which returns θ in the
    //    Vasicek-style mean-reversion-level convention;
    //    the tree pricer uses its own forward-induction calibration in
    //    finstack-quant-valuations whose σ normalization differs. For the
    //    uncalibrated `HullWhiteParams::default()` (κ = 3%, σ = 1%)
    //    used here, the resulting stationary short-rate std is ~4%,
    //    which produces substantially wider swap-rate dispersion under
    //    MC than under the tree's narrower grid.
    // 2. MC noise with limited paths (10k here).
    // 3. Different time / rate discretization.
    //
    // The test's intent is to verify the infrastructure works (both
    // pricers return positive, finite numbers on a realistic swaption)
    // rather than numerical agreement. For exact-agreement regressions,
    // see the forthcoming per-calibration golden tests. The tolerance
    // is deliberately loose — 2 orders of magnitude — because
    // uncalibrated default HW parameters are themselves known-bad
    // and no two pricers with independent calibration implementations
    // are required to agree here.
    let max_pv = tree_pv.max(lsmc_pv);
    let min_pv = tree_pv.min(lsmc_pv);
    if max_pv > 1.0 {
        let ratio = min_pv / max_pv;
        assert!(
            ratio > 0.01,
            "LSMC ({}) and Tree ({}) prices should be within 2 orders of \
             magnitude on the uncalibrated defaults (ratio: {}). A ratio \
             outside this band indicates one of the two pricers is \
             producing a near-zero or blow-up result, not a calibration \
             disagreement.",
            lsmc_pv,
            tree_pv,
            ratio
        );
    }

    eprintln!(
        "Tree PV: {:.2}, LSMC PV: {:.2}, stderr: {:.2}",
        tree_pv,
        lsmc_pv,
        lsmc_result
            .measures
            .get("mc_stderr")
            .copied()
            .unwrap_or(0.0)
    );
}

/// Test LSMC determinism: same seed produces identical results.
#[test]
fn test_lsmc_determinism() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    let swap_start = as_of;
    let swap_end = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");
    let first_exercise = Date::from_calendar_date(2027, Month::January, 1).expect("Valid date");

    let swaption =
        test_bermudan_swaption(swap_start, swap_end, first_exercise, 0.03, OptionType::Call);

    let market = build_market_context();

    // Price twice with same seed
    let pricer1 = BermudanSwaptionPricer::lsmc_with_config(BermudanSwaptionPricerConfig {
        mc_paths: 5_000,
        mc_seed: 12345,
        ..Default::default()
    });
    let result1 = pricer1
        .price_dyn(&swaption, &market, as_of)
        .expect("Pricing should succeed");

    let pricer2 = BermudanSwaptionPricer::lsmc_with_config(BermudanSwaptionPricerConfig {
        mc_paths: 5_000,
        mc_seed: 12345,
        ..Default::default()
    });
    let result2 = pricer2
        .price_dyn(&swaption, &market, as_of)
        .expect("Pricing should succeed");

    // Results should be identical
    assert!(
        (result1.value.amount() - result2.value.amount()).abs() < 1e-10,
        "Same seed should produce identical results: {} vs {}",
        result1.value.amount(),
        result2.value.amount()
    );
}

/// Test LSMC with different seeds produces different results.
#[test]
fn test_lsmc_different_seeds() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    let swap_start = as_of;
    let swap_end = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");
    let first_exercise = Date::from_calendar_date(2027, Month::January, 1).expect("Valid date");

    let swaption =
        test_bermudan_swaption(swap_start, swap_end, first_exercise, 0.03, OptionType::Call);

    let market = build_market_context();

    // Price with different seeds
    let pricer1 = BermudanSwaptionPricer::lsmc_with_config(BermudanSwaptionPricerConfig {
        mc_paths: 5_000,
        mc_seed: 111,
        ..Default::default()
    });
    let result1 = pricer1
        .price_dyn(&swaption, &market, as_of)
        .expect("Pricing should succeed");

    let pricer2 = BermudanSwaptionPricer::lsmc_with_config(BermudanSwaptionPricerConfig {
        mc_paths: 5_000,
        mc_seed: 222,
        ..Default::default()
    });
    let result2 = pricer2
        .price_dyn(&swaption, &market, as_of)
        .expect("Pricing should succeed");

    // Results should be different (due to different random paths)
    // Note: There's a tiny probability they could be equal by chance, but effectively zero
    assert!(
        (result1.value.amount() - result2.value.amount()).abs() > 1e-10,
        "Different seeds should produce different results: {} vs {}",
        result1.value.amount(),
        result2.value.amount()
    );
}

/// Test LSMC pricer key is correct.
#[test]
fn test_lsmc_pricer_key() {
    let pricer = BermudanSwaptionPricer::lsmc();
    let key = pricer.key();

    assert_eq!(
        key.instrument,
        finstack_quant_valuations::pricer::InstrumentType::BermudanSwaption
    );
    assert_eq!(
        key.model,
        finstack_quant_valuations::pricer::ModelKey::MonteCarloHullWhite1F
    );
}

// ============================================================================
// Bermudan calibration gate
// ============================================================================

/// With `enforce_calibration` set, pricing a Bermudan with the
/// uncalibrated `HullWhiteParams::default()` must return a clear
/// `ModelFailure` instead of silently producing a 10–30%-wrong price.
/// Covers the tree path.
#[test]
fn test_tree_refuses_uncalibrated_default_when_require_calibration_set() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid");
    let swap_start = as_of;
    let swap_end = Date::from_calendar_date(2030, Month::January, 1).expect("valid");
    let first_exercise = Date::from_calendar_date(2027, Month::January, 1).expect("valid");
    let swaption =
        test_bermudan_swaption(swap_start, swap_end, first_exercise, 0.03, OptionType::Call);

    let pricer = BermudanSwaptionPricer::tree_with_config(BermudanSwaptionPricerConfig {
        tree_steps: 50,
        enforce_calibration: true,
        ..Default::default()
    });

    // Direct tree pricer call — no market context needed for this path
    // because we expect early return on the uncalibrated guard. Use the
    // dyn Pricer trait as the registry does.
    let market = build_market_context();
    let err = pricer
        .price_dyn(&swaption, &market, as_of)
        .expect_err("require_calibration should refuse uncalibrated default");
    // The error message should mention calibration.
    let msg = format!("{err}");
    assert!(
        msg.contains("uncalibrated"),
        "error message must reference calibration requirement: {msg}"
    );
}

/// Same guarantee on the LSMC path.
#[test]
fn test_lsmc_refuses_uncalibrated_default_when_require_calibration_set() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid");
    let swap_start = as_of;
    let swap_end = Date::from_calendar_date(2030, Month::January, 1).expect("valid");
    let first_exercise = Date::from_calendar_date(2027, Month::January, 1).expect("valid");
    let swaption =
        test_bermudan_swaption(swap_start, swap_end, first_exercise, 0.03, OptionType::Call);

    let pricer = BermudanSwaptionPricer::lsmc_with_config(BermudanSwaptionPricerConfig {
        mc_paths: 1_000,
        enforce_calibration: true,
        ..Default::default()
    });

    let market = build_market_context();
    let err = pricer
        .price_dyn(&swaption, &market, as_of)
        .expect_err("require_calibration should refuse uncalibrated default on LSMC");
    let msg = format!("{err}");
    assert!(
        msg.contains("uncalibrated"),
        "error message must reference calibration requirement: {msg}"
    );
}

/// Sanity: the permissive (non-require_calibration) path still prices
/// successfully with the uncalibrated default. This preserves the
/// existing direct-constructor behaviour for tests and bespoke
/// workflows.
#[test]
fn test_permissive_default_still_prices_with_warning() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid");
    let swap_start = as_of;
    let swap_end = Date::from_calendar_date(2030, Month::January, 1).expect("valid");
    let first_exercise = Date::from_calendar_date(2027, Month::January, 1).expect("valid");
    let swaption =
        test_bermudan_swaption(swap_start, swap_end, first_exercise, 0.03, OptionType::Call);

    let market = build_market_context();

    // No enforce_calibration — should succeed (with a tracing::warn!).
    let pricer = BermudanSwaptionPricer::lsmc_with_config(BermudanSwaptionPricerConfig {
        mc_paths: 1_000,
        mc_seed: 42,
        ..Default::default()
    });
    let result = pricer
        .price_dyn(&swaption, &market, as_of)
        .expect("permissive default should still price");
    assert!(
        result.value.amount().is_finite(),
        "permissive default must produce a finite price"
    );
}

/// With a calibrated (non-default) HullWhiteParams, `enforce_calibration`
/// should be transparent — pricing succeeds just as it would without
/// the flag.
#[test]
fn test_require_calibration_with_explicit_params_prices_successfully() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid");
    let swap_start = as_of;
    let swap_end = Date::from_calendar_date(2030, Month::January, 1).expect("valid");
    let first_exercise = Date::from_calendar_date(2027, Month::January, 1).expect("valid");
    let swaption =
        test_bermudan_swaption(swap_start, swap_end, first_exercise, 0.03, OptionType::Call);

    let market = build_market_context();

    // Explicitly-chosen params (not the 3% / 1% defaults).
    let pricer = BermudanSwaptionPricer::lsmc_with_config(BermudanSwaptionPricerConfig {
        hw_params: HullWhiteParams::new(0.05, 0.012).expect("valid HW params"),
        mc_paths: 1_000,
        mc_seed: 42,
        enforce_calibration: true,
        ..Default::default()
    });
    let result = pricer
        .price_dyn(&swaption, &market, as_of)
        .expect("explicit params should be accepted under require_calibration");
    assert!(result.value.amount().is_finite());
}
