//! Shared runtime types and solver contracts for market calibration.
//!
use finstack_quant_calibration::validation::{
    validate_butterfly_call_convexity, validate_butterfly_spread, validate_calendar_spread,
    validate_surface, validate_vol_bounds, CurveValidator,
};
use finstack_quant_calibration::ValidationConfig;
use finstack_quant_core::dates::Date;
use finstack_quant_core::math::interp::InterpStyle;
use time::Month;

#[test]
fn test_discount_curve_validation() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let config = ValidationConfig::default();

    // Valid curve - monotonically decreasing DFs
    let valid_curve =
        finstack_quant_core::market_data::term_structures::DiscountCurve::builder("TEST-VALID")
            .base_date(base_date)
            .knots(vec![
                (0.0, 1.0),
                (0.25, 0.9950),
                (0.5, 0.9900),
                (1.0, 0.9800),
                (2.0, 0.9600),
                (5.0, 0.9000),
            ])
            .interp(InterpStyle::Linear)
            .build()
            .expect("should build valid curve");

    assert!(valid_curve.validate(&config).is_ok());

    // Invalid curve - increasing discount factors
    // NOTE: Must use allow_non_monotonic() since monotonicity is now enforced by default
    let invalid_curve =
        finstack_quant_core::market_data::term_structures::DiscountCurve::builder("TEST-INVALID")
            .base_date(base_date)
            .knots(vec![
                (0.0, 1.0),
                (0.25, 0.99), // Positive rates at short end
                (1.0, 0.95),
                (2.0, 0.96), // Increases! Violation.
                (5.0, 0.90),
            ])
            .interp(InterpStyle::Linear)
            .validation(
                finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                    allow_non_monotonic: true,
                    forward_floor: None,
                },
            ) // Allow construction of invalid curve for testing validation
            .build()
            .expect("should build invalid curve for testing");

    // Default config now enforces monotonicity (allow_negative_rates = false)
    assert!(invalid_curve.validate_monotonicity(&config).is_err());
}

#[test]
fn test_hazard_curve_validation() {
    use finstack_quant_core::market_data::term_structures::{HazardCurve, Seniority};

    let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let config = ValidationConfig::default();

    // Valid hazard curve
    let valid_curve = HazardCurve::builder("TEST-HAZARD")
        .base_date(base_date)
        .recovery_rate(0.40)
        .seniority(Seniority::Senior)
        .knots(vec![(1.0, 0.01), (2.0, 0.015), (5.0, 0.02)])
        .build()
        .expect("should build valid hazard curve");

    assert!(valid_curve.validate(&config).is_ok());

    // Check survival probability monotonicity
    assert!(valid_curve.validate_monotonicity(&config).is_ok());
}

#[test]
fn test_forward_curve_validation() {
    use finstack_quant_core::market_data::term_structures::ForwardCurve;

    let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let config = ValidationConfig::default();

    // Valid forward curve
    let valid_curve = ForwardCurve::builder("TEST-FWD", 0.25)
        .base_date(base_date)
        .knots(vec![
            (0.25, 0.045),
            (0.5, 0.046),
            (1.0, 0.047),
            (2.0, 0.048),
        ])
        .build()
        .expect("should build valid forward curve");

    assert!(valid_curve.validate(&config).is_ok());

    // Curve with negative forward rates (should fail if too negative)
    let negative_curve = ForwardCurve::builder("TEST-NEG-FWD", 0.25)
        .base_date(base_date)
        .knots(vec![
            (0.25, -0.08), // -8% forward rate (builder may fail on very negative)
            (0.5, 0.02),
            (1.0, 0.03),
        ])
        .build();

    // The curve builder itself might reject very negative rates,
    // or if it accepts them, our validation should reject them
    match negative_curve {
        Ok(curve) => {
            // If builder accepts it, our validation should reject it
            assert!(curve.validate_bounds(&config).is_err());
        }
        Err(_) => {
            // Builder rejected it, which is also a valid outcome
        }
    }
}

#[test]
fn test_base_correlation_validation() {
    use finstack_quant_core::market_data::term_structures::BaseCorrelationCurve;

    let config = ValidationConfig::default();
    // Valid base correlation curve - monotonically increasing
    let valid_curve = BaseCorrelationCurve::builder("TEST-CORR")
        .knots(vec![
            (3.0, 0.20),
            (7.0, 0.35),
            (10.0, 0.45),
            (15.0, 0.60),
            (30.0, 0.80),
        ])
        .build()
        .expect("should build valid base correlation curve");

    assert!(valid_curve.validate(&config).is_ok());

    // Structurally valid decreasing quotes violate the calibration shape policy.
    let invalid_curve = BaseCorrelationCurve::builder("TEST-INVALID-CORR")
        .knots(vec![(3.0, 0.40), (7.0, 0.30), (10.0, 0.50)])
        .build()
        .expect("should build invalid curve for testing");

    assert!(invalid_curve.validate_no_arbitrage(&config).is_err());
}

#[test]
fn test_non_monotone_positive_rate_curve_rejected() {
    use finstack_quant_core::market_data::term_structures::DiscountCurve;

    let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

    let non_monotone_curve = DiscountCurve::builder("TEST-NON-MONOTONE")
        .base_date(base_date)
        .knots(vec![
            (0.0, 1.0),
            (0.25, 0.99), // Positive rates (DF < 1)
            (0.5, 0.98),
            (1.0, 0.95),
            (2.0, 0.96), // DF(2Y) > DF(1Y) - violation!
            (5.0, 0.90),
        ])
        .interp(InterpStyle::Linear)
        .validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        )
        .build()
        .expect("should build non-monotone curve for testing");

    let short_rate = non_monotone_curve.zero(0.25);
    assert!(
        short_rate > 0.0,
        "Expected positive short-end rate, got {}",
        short_rate
    );

    let default_config = ValidationConfig::default();
    let result = non_monotone_curve.validate_monotonicity(&default_config);
    assert!(result.is_err());
    let err_msg = result.expect_err("Expected validation error").to_string();
    assert!(err_msg.contains("not monotonically decreasing"));
}

#[test]
fn test_negative_rate_environment_opt_in() {
    use finstack_quant_core::market_data::term_structures::DiscountCurve;

    let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

    let negative_rate_curve = DiscountCurve::builder("TEST-NEGATIVE-RATES")
        .base_date(base_date)
        .knots(vec![
            (0.0, 1.0),
            (0.25, 1.005), // DF > 1.0 implies negative rates
            (0.5, 1.008),
            (1.0, 1.010),
            (2.0, 1.005),
            (5.0, 0.99),
        ])
        .interp(InterpStyle::Linear)
        .validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        )
        .build()
        .expect("should build negative rate curve for testing");

    let short_rate = negative_rate_curve.zero(0.25);
    assert!(
        short_rate < 0.0,
        "Expected negative short-end rate, got {}",
        short_rate
    );

    let default_config = ValidationConfig::default();
    let strict_result = negative_rate_curve.validate_monotonicity(&default_config);
    assert!(strict_result.is_err());

    let permissive_config = ValidationConfig {
        allow_negative_rates: true,
        ..Default::default()
    };
    let permissive_result = negative_rate_curve.validate_monotonicity(&permissive_config);
    assert!(
        permissive_result.is_ok(),
        "expected ok: {:?}",
        permissive_result
    );
}

#[test]
fn negative_rate_opt_in_preserves_the_configured_forward_floor() {
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ValidationMode};

    let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let config = ValidationConfig {
        allow_negative_rates: true,
        ..ValidationConfig::default()
    };
    let grids: &[&[f64]] = &[&[0.0, 0.1], &[0.0, 0.25], &[0.0, 0.25, 0.5, 1.0]];
    for &times in grids {
        for (rate, accepted) in [(-0.03_f64, false), (-0.005, true), (0.03, true)] {
            let curve = DiscountCurve::builder("SIGNED-FORWARD-FLOOR")
                .base_date(base_date)
                .knots(times.iter().map(|&time| (time, (-rate * time).exp())))
                .interp(InterpStyle::LogLinear)
                .validation(ValidationMode::Raw {
                    allow_non_monotonic: true,
                    forward_floor: None,
                })
                .build()
                .expect("structurally valid discount curve");

            // All fixtures pass the remaining curve checks. The -3% curve must
            // fail against the -1% forward floor, including before the first
            // standard sampling tenor and at a three-month-only horizon.
            curve
                .validate_monotonicity(&config)
                .expect("negative rates opted in");
            curve
                .validate_bounds(&config)
                .expect("zero-rate and DF bounds");
            let result = curve.validate(&config);
            assert_eq!(
                result.is_ok(),
                accepted,
                "times={times:?}, rate={rate}: {result:?}"
            );
            if let Err(error) = result {
                assert!(error.to_string().contains("Negative forward rate"));
            }
        }
    }
}

#[test]
fn discount_forward_floor_checks_knots_between_standard_sampling_tenors() {
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ValidationMode};

    let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let curve = DiscountCurve::builder("INTERIOR-FORWARD-FLOOR")
        .base_date(base_date)
        .knots([
            (0.0, 1.0),
            (0.5, (-0.015_f64).exp()),
            (0.75, (-0.0075_f64).exp()),
            (1.0, (-0.03_f64).exp()),
        ])
        .interp(InterpStyle::LogLinear)
        .validation(ValidationMode::Raw {
            allow_non_monotonic: true,
            forward_floor: None,
        })
        .build()
        .expect("structurally valid discount curve");
    let config = ValidationConfig {
        allow_negative_rates: true,
        ..ValidationConfig::default()
    };

    // The old 0.5-to-1.0 sample sees a +3% average forward, hiding the
    // -3% forward from the 0.5 knot to the actual 0.75 knot.
    let error = curve
        .validate(&config)
        .expect_err("interior forward below floor");
    assert!(error.to_string().contains("Negative forward rate"));
}

#[test]
fn test_butterfly_arbitrage_detected_and_fails() {
    use finstack_quant_core::market_data::surfaces::VolSurface;

    let expiries = vec![0.25, 0.5, 1.0];
    let strikes = vec![90.0, 100.0, 110.0];
    let vol_grid = vec![
        // T=0.25
        0.20, 0.18, 0.20, // T=0.5 - extreme butterfly violation
        0.20, 0.50, 0.20, // T=1.0
        0.22, 0.20, 0.22,
    ];

    let surface = VolSurface::from_grid("TEST-BUTTERFLY-ARB", &expiries, &strikes, &vol_grid)
        .expect("should build vol surface for testing");

    let strict_config = ValidationConfig::default();
    let result = validate_butterfly_spread(&surface, &strict_config);
    assert!(result.is_err());

    let err_msg = result.expect_err("Expected validation error").to_string();
    assert!(err_msg.contains("Butterfly") || err_msg.contains("butterfly"));

    let lenient_config = ValidationConfig {
        lenient_arbitrage: true,
        ..Default::default()
    };
    let lenient_result = validate_butterfly_spread(&surface, &lenient_config);
    assert!(lenient_result.is_ok());
}

#[test]
fn test_call_convexity_rejects_vertical_spread_above_strike_width() {
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_models::closed_form::black_call;

    let strikes = [90.0, 100.0, 110.0];
    let vols = [0.80, 0.01, 0.01];
    let calls: [f64; 3] =
        std::array::from_fn(|index| black_call(100.0, strikes[index], vols[index], 1.0));
    assert!(calls[0] - calls[1] > strikes[1] - strikes[0]);
    assert!(calls[1] < (calls[0] + calls[2]) / 2.0);

    // Both a two-strike grid and a convex three-strike grid must enforce the
    // upper bound on the payoff of the low-strike call spread.
    for point_count in [2, 3] {
        let surface = VolSurface::from_grid(
            "TEST-VERTICAL-SPREAD-WIDTH",
            &[1.0],
            &strikes[..point_count],
            &vols[..point_count],
        )
        .expect("surface should build");
        let error =
            validate_butterfly_call_convexity(&surface, &ValidationConfig::default(), &[100.0])
                .expect_err("call spread must not cost more than its maximum payoff");
        assert!(error.to_string().contains("Vertical spread"));
        assert!(error.to_string().contains("strike width"));

        let lenient_config = ValidationConfig {
            lenient_arbitrage: true,
            ..Default::default()
        };
        assert!(validate_butterfly_call_convexity(&surface, &lenient_config, &[100.0]).is_ok());
    }
}

#[test]
fn test_call_convexity_checks_two_strike_monotonicity() {
    use finstack_quant_core::market_data::surfaces::VolSurface;

    let surface = VolSurface::from_grid(
        "TEST-NEGATIVE-VERTICAL-SPREAD",
        &[1.0],
        &[90.0, 100.0],
        &[0.01, 0.80],
    )
    .expect("surface should build");
    let error = validate_butterfly_call_convexity(&surface, &ValidationConfig::default(), &[100.0])
        .expect_err("higher-strike call must not cost more than lower-strike call");
    assert!(error.to_string().contains("Vertical spread"));
}

#[test]
fn test_call_convexity_accepts_valid_two_strike_spreads() {
    use finstack_quant_core::market_data::surfaces::VolSurface;

    // Zero volatility saturates the strike-width bound for two in-the-money
    // calls; a flat positive-volatility smile lies strictly within the bound.
    for vols in [[0.0, 0.0], [0.20, 0.20]] {
        let surface =
            VolSurface::from_grid("TEST-VALID-VERTICAL-SPREAD", &[1.0], &[80.0, 90.0], &vols)
                .expect("surface should build");
        assert!(validate_butterfly_call_convexity(
            &surface,
            &ValidationConfig::default(),
            &[100.0]
        )
        .is_ok());
    }
}

#[test]
fn test_call_convexity_still_rejects_butterfly_with_valid_vertical_spreads() {
    use finstack_quant_core::market_data::surfaces::VolSurface;

    let surface = VolSurface::from_grid(
        "TEST-CALL-BUTTERFLY",
        &[1.0],
        &[90.0, 100.0, 110.0],
        &[0.20, 0.25, 0.20],
    )
    .expect("surface should build");
    let error = validate_butterfly_call_convexity(&surface, &ValidationConfig::default(), &[100.0])
        .expect_err("a middle call above its neighbors' chord violates convexity");
    let message = error.to_string();
    assert!(message.contains("Butterfly"));
    assert!(!message.contains("Vertical spread"));
}

#[test]
fn test_calendar_arbitrage_detected_and_fails() {
    use finstack_quant_core::market_data::surfaces::VolSurface;

    let expiries = vec![0.25, 0.5, 1.0];
    let strikes = vec![95.0, 100.0, 105.0];
    let vol_grid = vec![
        // T=0.25
        0.35, 0.40, 0.35, // T=0.5 - lower vol causes calendar arbitrage
        0.18, 0.20, 0.18, // T=1.0
        0.20, 0.22, 0.20,
    ];

    let surface = VolSurface::from_grid("TEST-CALENDAR-ARB", &expiries, &strikes, &vol_grid)
        .expect("should build vol surface for testing");

    let strict_config = ValidationConfig::default();
    let result = validate_calendar_spread(&surface, &strict_config);
    assert!(result.is_err());

    let err_msg = result.expect_err("Expected validation error").to_string();
    assert!(err_msg.contains("Calendar") || err_msg.contains("calendar"));

    let lenient_config = ValidationConfig {
        lenient_arbitrage: true,
        ..Default::default()
    };
    let lenient_result = validate_calendar_spread(&surface, &lenient_config);
    assert!(lenient_result.is_ok());
}

#[test]
fn test_valid_surface_passes_arbitrage_checks() {
    use finstack_quant_core::market_data::surfaces::VolSurface;

    let expiries = vec![0.25, 0.5, 1.0];
    let strikes = vec![90.0, 100.0, 110.0];
    let vol_grid = vec![
        // T=0.25: mild smile that satisfies total-variance convexity
        0.215, 0.21, 0.215, // T=0.5
        0.225, 0.22, 0.225, // T=1.0
        0.245, 0.24, 0.245,
    ];

    let surface = VolSurface::from_grid("TEST-VALID-SURFACE", &expiries, &strikes, &vol_grid)
        .expect("should build valid vol surface");

    let config = ValidationConfig::default();
    assert!(validate_calendar_spread(&surface, &config).is_ok());
    assert!(validate_butterfly_spread(&surface, &config).is_ok());
    assert!(validate_vol_bounds(&surface, &config).is_ok());
    assert!(validate_surface(&surface, &config).is_ok());
}

#[test]
fn test_discount_curve_bounds_rejects_excessive_df() {
    use finstack_quant_core::market_data::term_structures::DiscountCurve;

    let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let config = ValidationConfig::default();

    let curve = DiscountCurve::builder("TEST-DF-BOUNDS")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (0.25, 1.10), (1.0, 0.95)])
        .interp(InterpStyle::Linear)
        .validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        )
        .build()
        .expect("curve should build");

    let err = curve
        .validate_bounds(&config)
        .expect_err("should reject DF > 1.0");
    assert!(err.to_string().contains("exceeds"));
}

#[test]
fn test_forward_curve_bounds_rejects_excessive_rate() {
    use finstack_quant_core::market_data::term_structures::ForwardCurve;

    let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let config = ValidationConfig::default();

    let curve = ForwardCurve::builder("TEST-FWD-HIGH", 0.25)
        .base_date(base_date)
        .knots(vec![(0.25, 0.02), (1.0, 0.75), (2.0, 0.03)])
        .build()
        .expect("forward curve");

    let err = curve
        .validate_bounds(&config)
        .expect_err("should reject extreme forward rate");
    assert!(err.to_string().contains("too high"));
}

#[test]
fn test_inflation_curve_hyperinflation_rejected() {
    use finstack_quant_core::market_data::term_structures::InflationCurve;

    let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let config = ValidationConfig::default();

    let curve = InflationCurve::builder("TEST-INFL-HYPER")
        .base_date(base_date)
        .base_cpi(100.0)
        .knots(vec![(1.0, 200.0), (2.0, 300.0)])
        .build()
        .expect("inflation curve");

    let err = curve
        .validate_monotonicity(&config)
        .expect_err("hyperinflation should be rejected");
    assert!(err.to_string().contains("Hyperinflation"));
}
