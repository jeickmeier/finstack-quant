use finstack_attribution::{AttributionMethod, PnlAttribution};
use finstack_core::currency::Currency;
use finstack_core::money::Money;
use time::macros::date;

#[test]
fn test_pnl_attribution_new() {
    let total = Money::new(1000.0, Currency::USD);
    let attr = PnlAttribution::new(
        total,
        "BOND-001",
        date!(2025 - 01 - 15),
        date!(2025 - 01 - 16),
        AttributionMethod::Parallel,
    );

    assert_eq!(attr.total_pnl, total);
    assert_eq!(attr.carry.amount(), 0.0);
    assert_eq!(attr.residual, total);
    assert_eq!(attr.meta.residual_pct, 100.0);
}

#[test]
fn test_compute_residual() {
    let total = Money::new(1000.0, Currency::USD);
    let mut attr = PnlAttribution::new(
        total,
        "BOND-001",
        date!(2025 - 01 - 15),
        date!(2025 - 01 - 16),
        AttributionMethod::Parallel,
    );

    attr.carry = Money::new(100.0, Currency::USD);
    attr.rates_curves_pnl = Money::new(500.0, Currency::USD);
    attr.fx_pnl = Money::new(390.0, Currency::USD);

    attr.compute_residual()
        .expect("Residual computation should succeed in test");

    assert_eq!(attr.residual.amount(), 10.0);
    assert!((attr.meta.residual_pct - 1.0).abs() < 1e-10);
}

#[test]
fn test_residual_tolerance() {
    let total = Money::new(10000.0, Currency::USD);
    let mut attr = PnlAttribution::new(
        total,
        "BOND-001",
        date!(2025 - 01 - 15),
        date!(2025 - 01 - 16),
        AttributionMethod::Parallel,
    );

    attr.carry = Money::new(9990.0, Currency::USD);
    attr.compute_residual()
        .expect("Residual computation should succeed in test");

    assert!(attr.residual_within_tolerance(0.1, 100.0));
    assert!(!attr.residual_within_tolerance(0.05, 5.0));
    assert!(attr.residual_within_tolerance(0.01, 100.0));
}

#[test]
fn test_currency_validation() {
    let total = Money::new(1000.0, Currency::USD);
    let mut attr = PnlAttribution::new(
        total,
        "BOND-001",
        date!(2025 - 01 - 15),
        date!(2025 - 01 - 16),
        AttributionMethod::Parallel,
    );

    assert!(attr.validate_currencies().is_ok());

    attr.fx_pnl = Money::new(100.0, Currency::EUR);
    assert!(attr.validate_currencies().is_err());

    let result = attr.compute_residual();
    assert!(result.is_err());
    assert!(!attr.meta.notes.is_empty());
    assert_eq!(attr.residual.amount(), 0.0);
}

#[test]
fn test_zero_total_pnl_with_nonzero_factors() {
    let total = Money::new(0.0, Currency::USD);
    let mut attr = PnlAttribution::new(
        total,
        "BOND-001",
        date!(2025 - 01 - 15),
        date!(2025 - 01 - 16),
        AttributionMethod::Parallel,
    );

    attr.carry = Money::new(100.0, Currency::USD);
    attr.rates_curves_pnl = Money::new(-100.0, Currency::USD);

    attr.compute_residual()
        .expect("Residual computation should succeed with zero total P&L");

    assert_eq!(attr.residual.amount(), 0.0);
    assert!(!attr.meta.residual_pct.is_nan());
    assert!(!attr.meta.residual_pct.is_infinite());
    assert_eq!(attr.meta.residual_pct, 0.0);
    assert!(attr.residual_within_tolerance(0.01, 0.01));
}

#[test]
fn test_zero_total_pnl_with_nonzero_residual() {
    let total = Money::new(0.0, Currency::USD);
    let mut attr = PnlAttribution::new(
        total,
        "BOND-001",
        date!(2025 - 01 - 15),
        date!(2025 - 01 - 16),
        AttributionMethod::Parallel,
    );

    attr.carry = Money::new(100.0, Currency::USD);
    attr.rates_curves_pnl = Money::new(-50.0, Currency::USD);

    attr.compute_residual()
        .expect("Residual computation should succeed");

    assert_eq!(attr.residual.amount(), -50.0);
    assert!(!attr.meta.residual_pct.is_nan());
    assert_eq!(attr.meta.residual_pct, 0.0);
    assert!(!attr.residual_within_tolerance(0.01, 10.0));
    assert!(attr.residual_within_tolerance(0.01, 100.0));
}

#[test]
fn test_pnl_attribution_json_envelope_trait() {
    let total = Money::new(1000.0, Currency::USD);
    let mut attr = PnlAttribution::new(
        total,
        "BOND-001",
        date!(2025 - 01 - 15),
        date!(2025 - 01 - 16),
        AttributionMethod::Parallel,
    );

    attr.carry = Money::new(100.0, Currency::USD);
    attr.rates_curves_pnl = Money::new(500.0, Currency::USD);
    attr.fx_pnl = Money::new(390.0, Currency::USD);
    attr.compute_residual()
        .expect("Residual computation should succeed");

    let json = serde_json::to_string_pretty(&attr).expect("to_json should succeed");
    assert!(json.contains("BOND-001"));
    assert!(json.contains("\"carry\""));

    let parsed = serde_json::from_str::<PnlAttribution>(&json).expect("from_json should succeed");
    assert_eq!(parsed.total_pnl, attr.total_pnl);
    assert_eq!(parsed.carry, attr.carry);
    assert_eq!(parsed.residual.amount(), attr.residual.amount());

    let reader = std::io::Cursor::new(json.as_bytes());
    let parsed_from_reader =
        serde_json::from_reader::<_, PnlAttribution>(reader).expect("from_reader should succeed");
    assert_eq!(parsed_from_reader.total_pnl, attr.total_pnl);
}
