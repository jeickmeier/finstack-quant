//! Tests for the surrounding crate component and its documented behavior.
//!
// Tests for Monte Carlo path capture functionality (integration tests).
use finstack_quant_monte_carlo::process::gbm::GbmProcess;
use finstack_quant_monte_carlo::process::metadata::ProcessMetadata;
use finstack_quant_monte_carlo::time_grid::TimeGrid;
use finstack_quant_monte_carlo::engine::{
    McEngineConfig, PathCaptureConfig, PathCaptureMode,
};
use finstack_quant_monte_carlo::payoff::vanilla::EuropeanCall;
use finstack_quant_monte_carlo::prelude::{
    PathDependentPricer, PathDependentPricerConfig,
};
use finstack_quant_core::currency::Currency;

#[test]
fn test_path_capture_all() {
    let time_grid = TimeGrid::uniform(1.0, 10).unwrap();
    let config = McEngineConfig {
        num_paths: 100,
        time_grid,
        target_ci_half_width: None,
        use_parallel: false,
        chunk_size: 1000,
        path_capture: PathCaptureConfig::all(),
        antithetic: false,
    };

    assert!(config.path_capture.enabled);
    assert_eq!(config.path_capture.capture_mode, PathCaptureMode::All);
}

#[test]
fn test_path_capture_sample() {
    let time_grid = TimeGrid::uniform(1.0, 10).unwrap();
    let config = McEngineConfig {
        num_paths: 1000,
        time_grid,
        target_ci_half_width: None,
        use_parallel: false,
        chunk_size: 1000,
        path_capture: PathCaptureConfig::sample(50, 123),
        antithetic: false,
    };

    assert!(config.path_capture.enabled);
    assert_eq!(
        config.path_capture.capture_mode,
        PathCaptureMode::Sample {
            count: 50,
            seed: 123
        }
    );
}

#[test]
fn test_path_capture_should_capture() {
    let config = PathCaptureConfig::sample(10, 42);

    // With 100 total paths and 10 sample count, roughly 10% should be captured
    let mut captured = 0;
    for path_id in 0..100 {
        if config.should_capture(path_id, 100) {
            captured += 1;
        }
    }

    // Should capture approximately 10 paths (with some variance due to hashing)
    assert!((5..=15).contains(&captured));
}

#[test]
fn test_process_metadata_gbm() {
    let gbm = GbmProcess::with_params(0.05, 0.02, 0.2).unwrap();
    let metadata = gbm.metadata();

    assert_eq!(metadata.process_type, "GBM");
    assert_eq!(metadata.parameters.get("r"), Some(&0.05));
    assert_eq!(metadata.parameters.get("q"), Some(&0.02));
    assert_eq!(metadata.parameters.get("sigma"), Some(&0.2));
    assert_eq!(metadata.factor_names, vec!["spot".to_string()]);
    assert!(metadata.correlation.is_none());
}

#[test]
fn test_path_dependent_pricer_with_paths() {
    let config = PathDependentPricerConfig::new(1000)
        .with_seed(42)
        .capture_sample_paths(10, 123);

    let pricer = PathDependentPricer::new(config);

    let gbm = GbmProcess::with_params(0.05, 0.02, 0.2).unwrap();
    let call = EuropeanCall::new(100.0, 1.0, 50);

    // Price with path capture
    let result = pricer
        .price_with_paths(&gbm, 100.0, 1.0, 50, &call, Currency::USD, 0.95)
        .unwrap();

    // Should have the estimate
    assert!(result.estimate.mean.amount() > 0.0);

    // Should have captured paths
    assert!(result.has_paths());
    let paths = result.paths().unwrap();

    // Should have captured approximately 10 paths
    assert!((5..=15).contains(&paths.num_captured()));
    assert_eq!(paths.num_paths_total, 1000);
}

#[test]
fn test_path_dataset_structure() {
    let config = PathDependentPricerConfig::new(100)
        .with_seed(42)
        .capture_all_paths();

    let pricer = PathDependentPricer::new(config);

    let gbm = GbmProcess::with_params(0.05, 0.02, 0.2).unwrap();
    let call = EuropeanCall::new(100.0, 1.0, 10);

    let result = pricer
        .price_with_paths(&gbm, 100.0, 1.0, 10, &call, Currency::USD, 0.95)
        .unwrap();

    let paths = result.paths().unwrap();

    // All paths should be captured
    assert_eq!(paths.num_captured(), 100);
    assert!(paths.is_complete());

    // Check first path structure
    let first_path = paths.path(0).unwrap();
    assert_eq!(first_path.path_id, 0);
    assert_eq!(first_path.num_steps(), 11); // 10 steps + initial point

    // Check that points have spot values
    for point in &first_path.points {
        assert!(point.spot().is_some());
    }

    // Check state var keys
    let keys = paths.state_var_keys();
    assert!(keys.contains(&"spot".to_string()));
}

#[test]
fn test_disabled_path_capture() {
    let config = PathDependentPricerConfig::new(100).with_seed(42);
    // Path capture should be disabled by default

    let pricer = PathDependentPricer::new(config);

    let gbm = GbmProcess::with_params(0.05, 0.02, 0.2).unwrap();
    let call = EuropeanCall::new(100.0, 1.0, 10);

    let result = pricer
        .price_with_paths(&gbm, 100.0, 1.0, 10, &call, Currency::USD, 0.95)
        .unwrap();

    // Should not have captured paths
    assert!(!result.has_paths());
    assert!(result.paths().is_none());

    // But should still have valid estimate
    assert!(result.estimate.mean.amount() > 0.0);
}

#[test]
fn test_path_capture_determinism_philox() {
    let config = PathDependentPricerConfig::new(50)
        .with_seed(7)
        .with_parallel(false)
        .capture_all_paths();

    let pricer = PathDependentPricer::new(config);
    let gbm = GbmProcess::with_params(0.05, 0.02, 0.2).unwrap();
    let call = EuropeanCall::new(100.0, 1.0, 12);

    let first = pricer
        .price_with_paths(&gbm, 100.0, 1.0, 12, &call, Currency::USD, 0.95)
        .unwrap();
    let second = pricer
        .price_with_paths(&gbm, 100.0, 1.0, 12, &call, Currency::USD, 0.95)
        .unwrap();

    let paths_first = first.paths().unwrap();
    let paths_second = second.paths().unwrap();
    assert_eq!(paths_first.num_captured(), paths_second.num_captured());

    let path_a = paths_first.path(0).unwrap();
    let path_b = paths_second.path(0).unwrap();
    assert_eq!(path_a.points.len(), path_b.points.len());
    assert!((path_a.final_value - path_b.final_value).abs() < 1e-12);

    for (pt_a, pt_b) in path_a.points.iter().zip(path_b.points.iter()) {
        let spot_a = pt_a.spot().unwrap_or(0.0);
        let spot_b = pt_b.spot().unwrap_or(0.0);
        assert!((spot_a - spot_b).abs() < 1e-12);
    }
}

#[test]
fn test_path_capture_determinism_sobol() {
    let config = PathDependentPricerConfig::new(50)
        .with_seed(7)
        .with_parallel(false)
        .with_sobol(true)
        .with_brownian_bridge(false)
        .capture_all_paths();

    let pricer = PathDependentPricer::new(config);
    let gbm = GbmProcess::with_params(0.05, 0.02, 0.2).unwrap();
    let call = EuropeanCall::new(100.0, 1.0, 12);

    let first = pricer
        .price_with_paths(&gbm, 100.0, 1.0, 12, &call, Currency::USD, 0.95)
        .unwrap();
    let second = pricer
        .price_with_paths(&gbm, 100.0, 1.0, 12, &call, Currency::USD, 0.95)
        .unwrap();

    let paths_first = first.paths().unwrap();
    let paths_second = second.paths().unwrap();
    assert_eq!(paths_first.num_captured(), paths_second.num_captured());

    let path_a = paths_first.path(0).unwrap();
    let path_b = paths_second.path(0).unwrap();
    assert_eq!(path_a.points.len(), path_b.points.len());
    assert!((path_a.final_value - path_b.final_value).abs() < 1e-12);

    for (pt_a, pt_b) in path_a.points.iter().zip(path_b.points.iter()) {
        let spot_a = pt_a.spot().unwrap_or(0.0);
        let spot_b = pt_b.spot().unwrap_or(0.0);
        assert!((spot_a - spot_b).abs() < 1e-12);
    }
}
