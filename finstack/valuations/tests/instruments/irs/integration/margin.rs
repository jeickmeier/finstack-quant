//! IRS margin integration tests
//!
//! Tests for margin-related functionality on interest rate swaps including:
//! - OtcMarginSpec integration
//! - DV01-based SIMM calculations
//! - CSA bilateral vs cleared margin requirements

use finstack_core::HashMap;
use finstack_core::{currency::Currency, dates::Date, money::Money, types::InstrumentId};
use finstack_margin::{
    ClearingStatus, CsaSpec, ImMethodology, ImParameters, MarginTenor, OtcMarginSpec,
    ScheduleImCalculator, SimmCalculator, VmCalculator, VmParameters,
};
use finstack_valuations::instruments::{InterestRateSwap, PayReceive};
use time::Month;

fn test_date() -> Date {
    Date::from_calendar_date(2024, Month::June, 15).expect("valid date")
}

fn create_test_swap() -> InterestRateSwap {
    let start = Date::from_calendar_date(2024, Month::June, 15).expect("valid date");
    let end = Date::from_calendar_date(2029, Month::June, 15).expect("valid date");

    crate::finstack_test_utils::usd_irs_swap(
        InstrumentId::new("TEST_IRS"),
        Money::new(100_000_000.0, Currency::USD), // 100MM notional
        0.035,                                    // 3.5% fixed rate
        start,
        end,
        PayReceive::Pay,
    )
    .expect("swap creation succeeds")
}

fn create_bilateral_margin_spec() -> OtcMarginSpec {
    let vm_params = VmParameters {
        threshold: Money::new(500_000.0, Currency::USD),
        mta: Money::new(100_000.0, Currency::USD),
        rounding: Money::new(10_000.0, Currency::USD),
        independent_amount: Money::new(0.0, Currency::USD),
        frequency: MarginTenor::Daily,
        settlement_lag: 1,
    };

    let im_params = ImParameters {
        methodology: ImMethodology::Simm,
        mpor_days: 10, // Standard bilateral MPOR
        threshold: Money::new(50_000_000.0, Currency::USD),
        mta: Money::new(500_000.0, Currency::USD),
        segregated: true,
    };

    let csa = CsaSpec {
        id: "TEST_CSA".to_string(),
        base_currency: Currency::USD,
        vm_params,
        im_params: Some(im_params),
        eligible_collateral: Default::default(),
        call_timing: Default::default(),
        collateral_curve_id: "USD-OIS".into(),
    };

    OtcMarginSpec {
        csa,
        clearing_status: ClearingStatus::Bilateral,
        im_methodology: ImMethodology::Simm,
        vm_frequency: MarginTenor::Daily,
        settlement_lag: 1,
    }
}

fn create_cleared_margin_spec() -> OtcMarginSpec {
    let vm_params = VmParameters {
        threshold: Money::new(0.0, Currency::USD), // CCPs have zero threshold
        mta: Money::new(0.0, Currency::USD),
        rounding: Money::new(1.0, Currency::USD),
        independent_amount: Money::new(0.0, Currency::USD),
        frequency: MarginTenor::Daily,
        settlement_lag: 0, // Same-day settlement at CCPs
    };

    let im_params = ImParameters {
        methodology: ImMethodology::ClearingHouse,
        mpor_days: 5, // Typically shorter for cleared
        threshold: Money::new(0.0, Currency::USD),
        mta: Money::new(0.0, Currency::USD),
        segregated: true,
    };

    let csa = CsaSpec {
        id: "LCH_CSA".to_string(),
        base_currency: Currency::USD,
        vm_params,
        im_params: Some(im_params),
        eligible_collateral: Default::default(),
        call_timing: Default::default(),
        collateral_curve_id: "USD-OIS".into(),
    };

    OtcMarginSpec {
        csa,
        clearing_status: ClearingStatus::Cleared {
            ccp: "LCH".to_string(),
        },
        im_methodology: ImMethodology::ClearingHouse,
        vm_frequency: MarginTenor::Daily,
        settlement_lag: 0,
    }
}

#[test]
fn test_irs_with_bilateral_margin() {
    let mut swap = create_test_swap();
    swap.margin_spec = Some(create_bilateral_margin_spec());

    assert!(swap.margin_spec.is_some());
    let spec = swap.margin_spec.as_ref().expect("margin spec exists");

    assert!(matches!(spec.clearing_status, ClearingStatus::Bilateral));
    assert!(matches!(spec.im_methodology, ImMethodology::Simm));
    assert_eq!(spec.csa.vm_params.threshold.amount(), 500_000.0);
}

#[test]
fn test_irs_with_cleared_margin() {
    let mut swap = create_test_swap();
    swap.margin_spec = Some(create_cleared_margin_spec());

    let spec = swap.margin_spec.as_ref().expect("margin spec exists");

    assert!(matches!(
        spec.clearing_status,
        ClearingStatus::Cleared { .. }
    ));
    if let ClearingStatus::Cleared { ccp } = &spec.clearing_status {
        assert_eq!(ccp, "LCH");
    }
    // CCPs have zero threshold
    assert_eq!(spec.csa.vm_params.threshold.amount(), 0.0);
}

#[test]
fn test_vm_calculation_for_irs() {
    let _swap = create_test_swap();
    let margin_spec = create_bilateral_margin_spec();

    let vm_calc = VmCalculator::new(margin_spec.csa.clone());

    // Simulate MTM exposure
    let exposure = Money::new(2_000_000.0, Currency::USD);
    let posted = Money::new(1_500_000.0, Currency::USD);
    let as_of = test_date();

    let result = vm_calc
        .calculate(exposure, posted, as_of)
        .expect("VM calculation succeeds");

    // Should require additional margin (exposure > posted + threshold)
    // Net exposure should reflect the difference
    assert!(
        result.net_exposure.amount().abs() > 0.0,
        "Should have non-zero net exposure"
    );
}

#[test]
fn test_simm_calculator_exists() {
    // Verify SIMM calculator can be created
    let simm = SimmCalculator::default();

    // Test IR delta calculation with empty sensitivities (edge case)
    let empty_dv01: HashMap<String, f64> = HashMap::default();
    let ir_margin = simm.calculate_ir_delta(&empty_dv01);

    // Empty sensitivities should produce zero margin
    assert_eq!(
        ir_margin, 0.0,
        "Empty sensitivities should produce zero margin"
    );
}

#[test]
fn test_schedule_calculator_exists() {
    // Verify schedule IM calculator can be created
    let schedule_calc = ScheduleImCalculator::bcbs_standard()
        .expect("bcbs_standard should load from embedded registry");

    // Just verify it was created successfully
    // Full integration test would require setting up market context
    assert!(schedule_calc.default_maturity_years > 0.0);
}

#[test]
fn test_bilateral_vs_cleared_im_difference() {
    let _swap = create_test_swap();
    let bilateral_spec = create_bilateral_margin_spec();
    let cleared_spec = create_cleared_margin_spec();

    // Bilateral MPOR is typically 10 days
    assert_eq!(
        bilateral_spec.csa.im_params.as_ref().map(|p| p.mpor_days),
        Some(10)
    );

    // Cleared MPOR is typically 5 days
    assert_eq!(
        cleared_spec.csa.im_params.as_ref().map(|p| p.mpor_days),
        Some(5)
    );

    // This difference would result in lower IM for cleared trades
    // (shorter MPOR = smaller potential loss = lower margin)
}

#[test]
fn test_vm_threshold_behavior() {
    let margin_spec = create_bilateral_margin_spec();
    let vm_calc = VmCalculator::new(margin_spec.csa);
    let as_of = test_date();

    // Exposure below threshold (500k)
    let small_exposure = Money::new(200_000.0, Currency::USD);
    let no_posted = Money::new(0.0, Currency::USD);

    let result = vm_calc
        .calculate(small_exposure, no_posted, as_of)
        .expect("VM calculation succeeds");

    // Net exposure within threshold means delivery_amount should be zero
    assert_eq!(
        result.delivery_amount.amount(),
        0.0,
        "Small exposure within threshold should not require delivery"
    );

    // Exposure above threshold
    let large_exposure = Money::new(1_000_000.0, Currency::USD);

    let result = vm_calc
        .calculate(large_exposure, no_posted, as_of)
        .expect("VM calculation succeeds");

    // Should have delivery amount
    assert!(
        result.delivery_amount.amount() > 0.0,
        "Large exposure should require delivery"
    );
}

#[test]
fn test_margin_call_series_generation() {
    let margin_spec = create_bilateral_margin_spec();
    let vm_calc = VmCalculator::new(margin_spec.csa);

    // Simulate daily MTM exposures over a week
    let exposures: Vec<(Date, Money)> = vec![
        (
            Date::from_calendar_date(2024, Month::June, 10).expect("valid"),
            Money::new(100_000.0, Currency::USD),
        ),
        (
            Date::from_calendar_date(2024, Month::June, 11).expect("valid"),
            Money::new(300_000.0, Currency::USD),
        ),
        (
            Date::from_calendar_date(2024, Month::June, 12).expect("valid"),
            Money::new(800_000.0, Currency::USD),
        ),
        (
            Date::from_calendar_date(2024, Month::June, 13).expect("valid"),
            Money::new(1_200_000.0, Currency::USD),
        ),
        (
            Date::from_calendar_date(2024, Month::June, 14).expect("valid"),
            Money::new(600_000.0, Currency::USD),
        ),
    ];

    let initial_collateral = Money::new(0.0, Currency::USD);

    let margin_calls = vm_calc
        .generate_margin_calls(&exposures, initial_collateral)
        .expect("margin call generation succeeds");

    // Should generate calls when exposure exceeds threshold
    // With threshold of 500k, we should see calls on days 3-5
    assert!(!margin_calls.is_empty(), "Should generate margin calls");
}
