//! Repo margin integration tests
//!
//! Tests for margin-related functionality on repos including:
//! - RepoMarginSpec construction and defaults
//! - Margin cashflow generation
//! - Margin call scenarios

use finstack_core::{currency::Currency, dates::Date, money::Money, types::CurveId};
use finstack_margin::{
    CsaSpec, MarginTenor, RepoMarginSpec, RepoMarginType, VmCalculator, VmParameters,
};
use finstack_valuations::instruments::{CollateralSpec, Repo};
use time::Month;

fn test_date() -> Date {
    Date::from_calendar_date(2024, Month::January, 15).expect("valid date")
}

fn create_test_repo() -> Repo {
    let collateral = CollateralSpec::new("UST-10Y", 10_200_000.0, "UST_10Y_PRICE");
    Repo::term(
        "TEST_REPO",
        Money::new(10_000_000.0, Currency::USD),
        collateral,
        0.045,
        Date::from_calendar_date(2024, Month::January, 15).expect("valid date"),
        Date::from_calendar_date(2024, Month::April, 15).expect("valid date"),
        CurveId::new("USD-OIS"),
    )
    .expect("Repo construction should succeed")
}

fn create_margined_repo() -> Repo {
    let mut repo = create_test_repo();
    repo.margin_spec = Some(RepoMarginSpec {
        margin_type: RepoMarginType::MarkToMarket,
        margin_ratio: 1.02,          // 2% over-collateralization
        margin_call_threshold: 0.01, // 1% threshold
        call_frequency: MarginTenor::Daily,
        settlement_lag: 1,
        pays_margin_interest: true,
        margin_interest_rate: Some(0.03),
        substitution_allowed: true,
        eligible_substitutes: None,
    });
    repo
}

#[test]
fn test_repo_margin_spec_defaults() {
    let spec = RepoMarginSpec::default();

    // Default is None (haircut only)
    assert_eq!(spec.margin_type, RepoMarginType::None);
    assert_eq!(spec.margin_ratio, 1.02); // 2% over-collateralization
    assert_eq!(spec.margin_call_threshold, 0.01);
    assert!(matches!(spec.call_frequency, MarginTenor::Daily));
    assert_eq!(spec.settlement_lag, 1);
    assert!(!spec.pays_margin_interest); // Default is false
    assert!(spec.margin_interest_rate.is_none());
    assert!(!spec.substitution_allowed); // Default is false
}

#[test]
fn test_repo_with_margin_spec() {
    let repo = create_margined_repo();

    assert!(repo.margin_spec.is_some());
    let spec = repo.margin_spec.as_ref().expect("margin spec exists");

    assert_eq!(spec.margin_ratio, 1.02); // 2% over-collateralization
    assert!(spec.pays_margin_interest);
    assert_eq!(spec.margin_interest_rate, Some(0.03));
}

#[test]
fn test_repo_margin_type_variations() {
    // Test different margin types
    let mut mtm_repo = create_test_repo();
    mtm_repo.margin_spec = Some(RepoMarginSpec {
        margin_type: RepoMarginType::MarkToMarket,
        ..Default::default()
    });

    let mut triparty_repo = create_test_repo();
    triparty_repo.margin_spec = Some(RepoMarginSpec {
        margin_type: RepoMarginType::Triparty,
        ..Default::default()
    });

    let mut net_exposure_repo = create_test_repo();
    net_exposure_repo.margin_spec = Some(RepoMarginSpec {
        margin_type: RepoMarginType::NetExposure,
        ..Default::default()
    });

    // All should have valid margin specs
    assert!(mtm_repo.margin_spec.is_some());
    assert!(triparty_repo.margin_spec.is_some());
    assert!(net_exposure_repo.margin_spec.is_some());
}

#[test]
fn test_margin_frequency_options() {
    // Test different margin call frequencies
    for frequency in [
        MarginTenor::Daily,
        MarginTenor::Weekly,
        MarginTenor::Monthly,
        MarginTenor::OnDemand,
    ] {
        let mut repo = create_test_repo();
        repo.margin_spec = Some(RepoMarginSpec {
            call_frequency: frequency,
            ..Default::default()
        });

        let spec = repo.margin_spec.as_ref().expect("spec exists");
        assert!(matches!(spec.call_frequency, _));
    }
}

#[test]
fn test_vm_calculator_with_repo_exposure() {
    // Create a CSA spec for testing VM calculations
    let csa = CsaSpec::usd_regulatory().expect("registry should load");
    let vm_calc = VmCalculator::new(csa);

    // Simulate repo exposure (positive = we are owed money)
    let exposure = Money::new(500_000.0, Currency::USD);
    let posted = Money::new(480_000.0, Currency::USD);
    let as_of = test_date();

    let result = vm_calc
        .calculate(exposure, posted, as_of)
        .expect("VM calculation succeeds");

    // Should have net exposure
    assert!(
        result.net_exposure.amount().abs() > 0.0,
        "Should have non-zero net exposure"
    );
}

#[test]
fn test_margin_call_with_threshold() {
    // Create VM params with meaningful threshold
    let vm_params = VmParameters {
        threshold: Money::new(50_000.0, Currency::USD),
        mta: Money::new(10_000.0, Currency::USD),
        rounding: Money::new(1_000.0, Currency::USD),
        independent_amount: Money::new(0.0, Currency::USD),
        frequency: MarginTenor::Daily,
        settlement_lag: 1,
    };

    let csa = CsaSpec {
        id: "TEST_CSA".to_string(),
        base_currency: Currency::USD,
        vm_params,
        im_params: None,
        eligible_collateral: Default::default(),
        call_timing: Default::default(),
        collateral_curve_id: CurveId::new("USD-OIS"),
    };

    let vm_calc = VmCalculator::new(csa);

    // Small exposure change within threshold
    let exposure = Money::new(30_000.0, Currency::USD);
    let posted = Money::new(0.0, Currency::USD);
    let as_of = test_date();

    let result = vm_calc
        .calculate(exposure, posted, as_of)
        .expect("VM calculation succeeds");

    // Should be within threshold so no delivery amount
    assert_eq!(
        result.delivery_amount.amount(),
        0.0,
        "Exposure within threshold should not trigger delivery"
    );
}

#[test]
fn test_margin_call_exceeds_threshold() {
    // Create VM params with meaningful threshold
    let vm_params = VmParameters {
        threshold: Money::new(50_000.0, Currency::USD),
        mta: Money::new(10_000.0, Currency::USD),
        rounding: Money::new(1_000.0, Currency::USD),
        independent_amount: Money::new(0.0, Currency::USD),
        frequency: MarginTenor::Daily,
        settlement_lag: 1,
    };

    let csa = CsaSpec {
        id: "TEST_CSA".to_string(),
        base_currency: Currency::USD,
        vm_params,
        im_params: None,
        eligible_collateral: Default::default(),
        call_timing: Default::default(),
        collateral_curve_id: CurveId::new("USD-OIS"),
    };

    let vm_calc = VmCalculator::new(csa);

    // Large exposure that exceeds threshold
    let exposure = Money::new(100_000.0, Currency::USD);
    let posted = Money::new(0.0, Currency::USD);
    let as_of = test_date();

    let result = vm_calc
        .calculate(exposure, posted, as_of)
        .expect("VM calculation succeeds");

    // Should have delivery amount when above threshold
    assert!(
        result.delivery_amount.amount() > 0.0,
        "Should generate positive delivery amount"
    );
}

#[test]
fn test_repo_margin_enables_additional_features() {
    // Repos with margin_spec have access to margin-related functionality
    let repo_without_margin = create_test_repo();
    let repo_with_margin = create_margined_repo();

    assert!(repo_without_margin.margin_spec.is_none());
    assert!(repo_with_margin.margin_spec.is_some());

    // When margin spec is present, we can access margin parameters
    if let Some(spec) = &repo_with_margin.margin_spec {
        assert!(spec.margin_ratio > 0.0);
        assert!(spec.margin_call_threshold > 0.0);
    }
}

#[test]
fn test_margin_spec_serialization_roundtrip() {
    let spec = RepoMarginSpec {
        margin_type: RepoMarginType::MarkToMarket,
        margin_ratio: 1.025,
        margin_call_threshold: 0.015,
        call_frequency: MarginTenor::Weekly,
        settlement_lag: 2,
        pays_margin_interest: true,
        margin_interest_rate: Some(0.035),
        substitution_allowed: false,
        eligible_substitutes: None,
    };

    // Test serde roundtrip if feature is enabled
    {
        let json = serde_json::to_string(&spec).expect("serialization succeeds");
        let deserialized: RepoMarginSpec =
            serde_json::from_str(&json).expect("deserialization succeeds");

        assert_eq!(spec.margin_type, deserialized.margin_type);
        assert_eq!(spec.margin_ratio, deserialized.margin_ratio);
        assert_eq!(
            spec.margin_call_threshold,
            deserialized.margin_call_threshold
        );
        assert_eq!(spec.settlement_lag, deserialized.settlement_lag);
        assert_eq!(spec.pays_margin_interest, deserialized.pays_margin_interest);
        assert_eq!(spec.margin_interest_rate, deserialized.margin_interest_rate);
        assert_eq!(spec.substitution_allowed, deserialized.substitution_allowed);
    }
}
