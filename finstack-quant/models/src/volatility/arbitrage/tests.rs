use super::engine::{expand_forward_prices, local_vol_density_violations};
use super::*;
use finstack_quant_core::market_data::surfaces::{VolQuoteType, VolSurface, VolSurfaceAxis};

/// Flat vol surface: constant 20% vol everywhere. Must pass all checks.
fn flat_surface() -> VolSurface {
    VolSurface::builder("FLAT-20")
        .expiries(&[0.25, 0.5, 1.0, 2.0])
        .secondary_values(&[80.0, 90.0, 100.0, 110.0, 120.0])
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(finstack_quant_core::market_data::surfaces::VolInterpolationMode::Vol)
        .row(&[0.20, 0.20, 0.20, 0.20, 0.20])
        .row(&[0.20, 0.20, 0.20, 0.20, 0.20])
        .row(&[0.20, 0.20, 0.20, 0.20, 0.20])
        .row(&[0.20, 0.20, 0.20, 0.20, 0.20])
        .build()
        .expect("flat surface should build")
}

/// Clean smile surface: strongly convex in strike (total variance is
/// quadratic), term structure with total variance strictly increasing.
/// Should pass butterfly and calendar spread checks.
fn clean_smile_surface() -> VolSurface {
    VolSurface::builder("CLEAN-SMILE")
        .expiries(&[0.5, 1.0, 2.0])
        .secondary_values(&[80.0, 90.0, 100.0, 110.0, 120.0])
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(finstack_quant_core::market_data::surfaces::VolInterpolationMode::Vol)
        .row(&[0.35, 0.27, 0.20, 0.27, 0.35])
        .row(&[0.33, 0.26, 0.20, 0.26, 0.33])
        .row(&[0.31, 0.25, 0.20, 0.25, 0.31])
        .build()
        .expect("clean smile should build")
}

/// Surface with butterfly violation: non-convex strike profile at T=1.0.
/// The middle strike has vol lower than what convexity allows.
fn butterfly_violation_surface() -> VolSurface {
    VolSurface::builder("BUTTERFLY-BAD")
        .expiries(&[0.5, 1.0])
        .secondary_values(&[80.0, 90.0, 100.0, 110.0, 120.0])
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(finstack_quant_core::market_data::surfaces::VolInterpolationMode::Vol)
        .row(&[0.30, 0.25, 0.20, 0.25, 0.30]) // T=0.5 clean
        .row(&[0.22, 0.25, 0.30, 0.25, 0.22]) // T=1.0 concave in total var
        .build()
        .expect("butterfly violation surface should build")
}

/// Surface with calendar spread violation: T=2 has lower total variance
/// than T=1 at certain strikes.
fn calendar_spread_violation_surface() -> VolSurface {
    // T=1.0: vol = 0.25 -> total var = 0.0625
    // T=2.0: vol = 0.15 -> total var = 0.045 < 0.0625 => violation
    VolSurface::builder("CALENDAR-BAD")
        .expiries(&[0.5, 1.0, 2.0])
        .secondary_values(&[80.0, 90.0, 100.0, 110.0, 120.0])
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(finstack_quant_core::market_data::surfaces::VolInterpolationMode::Vol)
        .row(&[0.30, 0.25, 0.20, 0.25, 0.30])
        .row(&[0.30, 0.25, 0.25, 0.25, 0.30]) // T=1.0 higher vol
        .row(&[0.20, 0.15, 0.15, 0.15, 0.20]) // T=2.0 much lower vol
        .build()
        .expect("calendar spread violation surface should build")
}

#[test]
fn expand_forward_prices_broadcasts_single_value() {
    assert_eq!(
        expand_forward_prices(3, vec![100.0]).expect("broadcast forwards"),
        vec![100.0, 100.0, 100.0]
    );
    assert!(expand_forward_prices(3, vec![100.0, 101.0]).is_err());
}

#[test]
fn check_surface_grid_runs_standard_suite() {
    let strikes = vec![80.0, 90.0, 100.0, 110.0, 120.0];
    let expiries = vec![0.25, 0.5, 1.0, 2.0];
    let vols = vec![
        vec![0.20, 0.20, 0.20, 0.20, 0.20],
        vec![0.20, 0.20, 0.20, 0.20, 0.20],
        vec![0.20, 0.20, 0.20, 0.20, 0.20],
        vec![0.20, 0.20, 0.20, 0.20, 0.20],
    ];

    let report = check_surface_grid(
        &expiries,
        &strikes,
        &vols,
        vec![100.0, 101.0, 102.0, 103.0],
        1e-6,
    )
    .expect("grid arbitrage check");

    assert!(report.passed);
    assert_eq!(report.vol_surface_id, "surface-grid");
}

#[test]
fn check_surface_uses_forward_prices_for_local_vol_check() {
    let surface = calendar_spread_violation_surface();
    let forwards = vec![100.0, 101.0, 102.0];
    let direct_local_vol_violations = local_vol_density_violations(&surface, &forwards, 1e-10);
    assert!(!direct_local_vol_violations.is_empty());

    let config = ArbitrageCheckConfig {
        check_butterfly: false,
        check_calendar_spread: false,
        check_local_vol_density: true,
        forward_prices: Some(forwards),
        ..Default::default()
    };
    let report = check_surface(&surface, &config).expect("arbitrage check should succeed");

    assert_eq!(report.violations.len(), direct_local_vol_violations.len());
    assert!(!report.passed);
}

#[test]
fn check_surface_grid_preserves_per_expiry_local_vol_behavior() {
    let strikes = vec![80.0, 90.0, 100.0, 110.0, 120.0];
    let expiries = vec![0.5, 1.0, 2.0];
    let vols = vec![
        vec![0.30, 0.25, 0.20, 0.25, 0.30],
        vec![0.30, 0.25, 0.25, 0.25, 0.30],
        vec![0.20, 0.15, 0.15, 0.15, 0.20],
    ];

    let report = check_surface_grid(&expiries, &strikes, &vols, vec![100.0, 101.0, 102.0], 1e-10)
        .expect("grid arbitrage check");

    let local_vol_count = report
        .violations
        .iter()
        .filter(|v| v.violation_type == ArbitrageType::LocalVolDensity)
        .count();
    assert!(local_vol_count > 0);
    assert_eq!(
        report
            .counts_by_type
            .get(&ArbitrageType::LocalVolDensity)
            .copied()
            .unwrap_or(0),
        local_vol_count
    );
}

#[test]
fn flat_surface_passes_all_checks() {
    let surface = flat_surface();
    let config = ArbitrageCheckConfig {
        forward_prices: Some(vec![100.0]),
        ..Default::default()
    };
    let report = check_surface(&surface, &config).expect("arbitrage check should succeed");

    assert!(
        report.passed,
        "Flat surface should pass all checks. Violations: {:?}",
        report
            .violations
            .iter()
            .map(|v| &v.description)
            .collect::<Vec<_>>()
    );
    assert!(
        report.violations.is_empty()
            || report
                .violations
                .iter()
                .all(|v| v.severity == ArbitrageSeverity::Negligible),
        "Flat surface should have no violations above Negligible"
    );
}

#[test]
fn clean_smile_passes_butterfly_and_calendar() {
    let surface = clean_smile_surface();
    let config = ArbitrageCheckConfig {
        forward_prices: Some(vec![100.0]),
        min_severity: ArbitrageSeverity::Minor,
        ..Default::default()
    };
    let report = check_surface(&surface, &config).expect("arbitrage check should succeed");

    // Filter to non-density checks (butterfly and calendar only)
    let butterfly_violations: Vec<_> = report
        .violations
        .iter()
        .filter(|v| v.violation_type == ArbitrageType::Butterfly)
        .collect();
    let calendar_violations: Vec<_> = report
        .violations
        .iter()
        .filter(|v| v.violation_type == ArbitrageType::CalendarSpread)
        .collect();

    assert!(
        butterfly_violations.is_empty(),
        "Clean smile should pass butterfly check. Violations: {:?}",
        butterfly_violations
            .iter()
            .map(|v| &v.description)
            .collect::<Vec<_>>()
    );
    assert!(
        calendar_violations.is_empty(),
        "Clean smile should pass calendar spread check. Violations: {:?}",
        calendar_violations
            .iter()
            .map(|v| &v.description)
            .collect::<Vec<_>>()
    );
}

#[test]
fn butterfly_violation_detected() {
    let surface = butterfly_violation_surface();
    let config = ArbitrageCheckConfig {
        check_butterfly: true,
        check_calendar_spread: false,
        check_local_vol_density: false,
        forward_prices: Some(vec![100.0]),
        min_severity: ArbitrageSeverity::Minor,
        ..Default::default()
    };
    let report = check_surface(&surface, &config).expect("arbitrage check should succeed");

    let butterfly_violations: Vec<_> = report
        .violations
        .iter()
        .filter(|v| v.violation_type == ArbitrageType::Butterfly)
        .collect();

    assert!(
        !butterfly_violations.is_empty(),
        "Should detect butterfly violation on concave smile"
    );

    // Violation should be at T=1.0 (the concave slice)
    assert!(
        butterfly_violations
            .iter()
            .any(|v| (v.location.expiry - 1.0).abs() < 0.01),
        "Butterfly violation should be at T=1.0"
    );
}

#[test]
fn calendar_spread_violation_detected() {
    let surface = calendar_spread_violation_surface();
    let config = ArbitrageCheckConfig {
        check_butterfly: false,
        check_calendar_spread: true,
        check_local_vol_density: false,
        forward_prices: Some(vec![100.0]),
        min_severity: ArbitrageSeverity::Negligible,
        ..Default::default()
    };
    let report = check_surface(&surface, &config).expect("arbitrage check should succeed");

    let calendar_violations: Vec<_> = report
        .violations
        .iter()
        .filter(|v| v.violation_type == ArbitrageType::CalendarSpread)
        .collect();

    assert!(
        !calendar_violations.is_empty(),
        "Should detect calendar spread violation"
    );

    // Violation should reference T=2.0
    assert!(
        calendar_violations
            .iter()
            .any(|v| (v.location.expiry - 2.0).abs() < 0.01),
        "Calendar violation should be at T=2.0"
    );

    // Should have adjacent_expiry pointing back to T=1.0
    assert!(
        calendar_violations.iter().any(|v| v
            .location
            .adjacent_expiry
            .is_some_and(|ae| (ae - 1.0).abs() < 0.01)),
        "Calendar violation should reference T=1.0 as adjacent expiry"
    );
}

#[test]
fn single_expiry_surface_no_calendar_violations() {
    let surface = VolSurface::builder("SINGLE-EXP")
        .expiries(&[1.0])
        .secondary_values(&[80.0, 90.0, 100.0, 110.0, 120.0])
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(finstack_quant_core::market_data::surfaces::VolInterpolationMode::Vol)
        .row(&[0.30, 0.25, 0.20, 0.25, 0.30])
        .build()
        .expect("single expiry surface should build");

    let config = ArbitrageCheckConfig {
        check_calendar_spread: true,
        check_butterfly: false,
        check_local_vol_density: false,
        forward_prices: Some(vec![100.0]),
        ..Default::default()
    };
    let report = check_surface(&surface, &config).expect("arbitrage check should succeed");

    assert!(
        report.violations.is_empty(),
        "Single expiry surface should have no calendar spread violations"
    );
}

#[test]
fn two_strikes_no_butterfly_violations() {
    let surface = VolSurface::builder("TWO-STRIKES")
        .expiries(&[1.0, 2.0])
        .secondary_values(&[90.0, 110.0])
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(finstack_quant_core::market_data::surfaces::VolInterpolationMode::Vol)
        .row(&[0.25, 0.25])
        .row(&[0.24, 0.24])
        .build()
        .expect("two strikes surface should build");

    let config = ArbitrageCheckConfig {
        check_butterfly: true,
        check_calendar_spread: false,
        check_local_vol_density: false,
        forward_prices: Some(vec![100.0]),
        ..Default::default()
    };
    let report = check_surface(&surface, &config).expect("arbitrage check should succeed");

    let butterfly_violations: Vec<_> = report
        .violations
        .iter()
        .filter(|v| v.violation_type == ArbitrageType::Butterfly)
        .collect();
    assert!(
        butterfly_violations.is_empty(),
        "Two-strike surface should skip butterfly check (needs 3+ strikes)"
    );
}

#[test]
fn report_severity_filtering() {
    let surface = calendar_spread_violation_surface();
    let config_all = ArbitrageCheckConfig {
        min_severity: ArbitrageSeverity::Negligible,
        forward_prices: Some(vec![100.0]),
        ..Default::default()
    };
    let report = check_surface(&surface, &config_all).expect("arbitrage check should succeed");

    // Should have some violations
    assert!(
        !report.violations.is_empty(),
        "Should have violations on dirty surface"
    );
}

#[test]
fn report_has_correct_counts() {
    let surface = calendar_spread_violation_surface();
    let config = ArbitrageCheckConfig {
        check_butterfly: false,
        check_calendar_spread: true,
        check_local_vol_density: false,
        forward_prices: Some(vec![100.0]),
        ..Default::default()
    };
    let report = check_surface(&surface, &config).expect("arbitrage check should succeed");

    let type_count = report.counts_by_type.get(&ArbitrageType::CalendarSpread);
    let total_calendar: usize = report
        .violations
        .iter()
        .filter(|v| v.violation_type == ArbitrageType::CalendarSpread)
        .count();

    assert_eq!(
        type_count.copied().unwrap_or(0),
        total_calendar,
        "counts_by_type should match actual violation count"
    );
}

#[test]
fn config_rejects_enabled_checks_without_forwards() {
    let surface = flat_surface();
    let config = ArbitrageCheckConfig {
        check_butterfly: true,
        check_calendar_spread: true,
        check_local_vol_density: true,
        ..Default::default()
    };
    assert!(check_surface(&surface, &config).is_err());
}

#[test]
fn report_serialization_round_trip() {
    let surface = calendar_spread_violation_surface();
    let config = ArbitrageCheckConfig {
        forward_prices: Some(vec![100.0]),
        ..Default::default()
    };
    let report = check_surface(&surface, &config).expect("arbitrage check should succeed");

    // Serialize to JSON
    let json = serde_json::to_string(&report).expect("report should serialize");

    // Deserialize back
    let deserialized: ArbitrageReport =
        serde_json::from_str(&json).expect("report should deserialize");

    assert_eq!(report.vol_surface_id, deserialized.vol_surface_id);
    assert_eq!(report.passed, deserialized.passed);
    assert_eq!(report.violations.len(), deserialized.violations.len());
}

#[test]
fn config_serialization_round_trip() {
    let config = ArbitrageCheckConfig {
        check_butterfly: true,
        check_calendar_spread: false,
        check_local_vol_density: true,
        forward_prices: Some(vec![100.0, 101.0]),
        tolerance: 1e-8,
        min_severity: ArbitrageSeverity::Minor,
    };

    let json = serde_json::to_string(&config).expect("config should serialize");
    let deserialized: ArbitrageCheckConfig =
        serde_json::from_str(&json).expect("config should deserialize");

    assert_eq!(config.check_butterfly, deserialized.check_butterfly);
    assert_eq!(
        config.check_calendar_spread,
        deserialized.check_calendar_spread
    );
    assert_eq!(config.forward_prices, deserialized.forward_prices);
    assert!((config.tolerance - deserialized.tolerance).abs() < 1e-14);
    assert_eq!(config.min_severity, deserialized.min_severity);

    // schema-rejection-test: the retired scalar key is not an alias.
    let mut retired = serde_json::to_value(&config).expect("serialize config fixture");
    retired["forward"] = serde_json::json!(100.0);
    assert!(serde_json::from_value::<ArbitrageCheckConfig>(retired).is_err());
}
