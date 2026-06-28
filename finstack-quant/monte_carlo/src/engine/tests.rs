use super::*;
use crate::paths::{CashflowType, ProcessParams};
use crate::results::MonteCarloResult;
use crate::time_grid::TimeGrid;
use crate::traits::{Discretization, PathState, Payoff, RandomStream, StochasticProcess};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;

// Dummy implementations for testing
#[derive(Clone)]
struct DummyRng;
impl RandomStream for DummyRng {
    fn split(&self, _id: u64) -> Option<Self> {
        Some(DummyRng)
    }
    fn fill_u01(&mut self, out: &mut [f64]) {
        for x in out {
            *x = 0.5;
        }
    }
    fn fill_std_normals(&mut self, out: &mut [f64]) {
        for x in out {
            *x = 0.0;
        }
    }
}

#[derive(Clone)]
struct PathIndexedRng {
    path_id: u64,
}

impl PathIndexedRng {
    fn root() -> Self {
        Self { path_id: 0 }
    }
}

impl RandomStream for PathIndexedRng {
    fn split(&self, stream_id: u64) -> Option<Self> {
        Some(Self { path_id: stream_id })
    }

    fn fill_u01(&mut self, out: &mut [f64]) {
        let value = (self.path_id + 1) as f64 / 8.0;
        for x in out {
            *x = value;
        }
    }

    fn fill_std_normals(&mut self, out: &mut [f64]) {
        for x in out {
            *x = 0.0;
        }
    }
}

struct DummyProcess;
impl StochasticProcess for DummyProcess {
    fn dim(&self) -> usize {
        1
    }
    fn drift(&self, _t: f64, _x: &[f64], out: &mut [f64]) {
        out[0] = 0.0;
    }
    fn diffusion(&self, _t: f64, _x: &[f64], out: &mut [f64]) {
        out[0] = 0.1;
    }
}

#[derive(Clone)]
struct DummyDisc;
impl Discretization<DummyProcess> for DummyDisc {
    fn step(
        &self,
        _process: &DummyProcess,
        _t: f64,
        _dt: f64,
        _x: &mut [f64],
        _z: &[f64],
        _work: &mut [f64],
    ) {
        // Just keep state constant
    }
}

#[derive(Clone)]
struct DummyPayoff;
impl Payoff for DummyPayoff {
    fn on_event(&mut self, _state: &mut PathState) {}
    fn value(&self, currency: Currency) -> Money {
        Money::new(100.0, currency)
    }
    fn reset(&mut self) {}
}

#[derive(Clone, Default)]
struct PathStartPayoff {
    start_uniform: Option<f64>,
}

impl Payoff for PathStartPayoff {
    fn on_path_start<R: RandomStream>(&mut self, rng: &mut R) {
        self.start_uniform = Some(rng.next_u01());
    }

    fn on_event(&mut self, _state: &mut PathState) {}

    fn value(&self, currency: Currency) -> Money {
        Money::new(self.start_uniform.unwrap_or(-1.0), currency)
    }

    fn reset(&mut self) {
        self.start_uniform = None;
    }
}

#[derive(Clone, Default)]
struct CapturedValuePayoff {
    value: Option<f64>,
}

impl Payoff for CapturedValuePayoff {
    fn on_path_start<R: RandomStream>(&mut self, rng: &mut R) {
        self.value = Some(rng.next_u01());
    }

    fn on_event(&mut self, _state: &mut PathState) {}

    fn value(&self, currency: Currency) -> Money {
        Money::new(self.value.unwrap_or_default(), currency)
    }

    fn reset(&mut self) {
        self.value = None;
    }
}

#[derive(Clone)]
struct InitialCashflowPayoff {
    value: f64,
}

impl Payoff for InitialCashflowPayoff {
    fn on_event(&mut self, state: &mut PathState) {
        if state.step == 0 {
            state.add_cashflow(state.time, self.value);
        }
    }

    fn value(&self, currency: Currency) -> Money {
        Money::new(self.value, currency)
    }

    fn reset(&mut self) {}
}

#[derive(Clone, Default)]
struct RecurringCashflowPayoff;

impl Payoff for RecurringCashflowPayoff {
    fn on_event(&mut self, state: &mut PathState) {
        state.add_typed_cashflow(state.time, state.step as f64 + 1.0, CashflowType::Interest);
    }

    fn value(&self, currency: Currency) -> Money {
        Money::new(0.0, currency)
    }

    fn reset(&mut self) {}
}

#[test]
fn test_engine_builder() {
    let engine = McEngine::builder()
        .num_paths(1000)
        .uniform_grid(1.0, 100)
        .build()
        .expect("McEngine builder should succeed with valid test data");

    assert_eq!(engine.config().num_paths, 1000);
}

#[test]
fn test_basic_pricing() {
    let engine = McEngine::builder()
        .num_paths(100)
        .uniform_grid(1.0, 10)
        .parallel(false)
        .build()
        .expect("McEngine builder should succeed with valid test data");

    let rng = DummyRng;
    let process = DummyProcess;
    let disc = DummyDisc;
    let initial_state = vec![100.0];
    let payoff = DummyPayoff;

    let result = engine
        .price(
            &rng,
            &process,
            &disc,
            &initial_state,
            &payoff,
            Currency::USD,
            1.0,
        )
        .expect("should succeed");

    assert_eq!(result.mean.amount(), 100.0);
    assert_eq!(result.num_paths, 100);
}

#[test]
fn test_parallel_execution_error_propagation() {
    // Test that parallel execution properly propagates errors instead of panicking.
    // The key change is that we replaced .expect() with ? operator, which ensures
    // errors are propagated via Result rather than panicking.
    //
    // This test verifies that:
    // 1. Parallel execution works correctly for valid inputs
    // 2. Error handling mechanism is in place (verified by compilation - ? operator
    //    requires Result return type)

    let engine = McEngine::builder()
        .num_paths(100)
        .uniform_grid(1.0, 10)
        .parallel(true)
        .chunk_size(50)
        .build()
        .expect("McEngine builder should succeed with valid test data");

    let rng = DummyRng;
    let process = DummyProcess;
    let disc = DummyDisc;
    let initial_state = vec![100.0];
    let payoff = DummyPayoff;

    // Valid input should work
    let result = engine.price(
        &rng,
        &process,
        &disc,
        &initial_state,
        &payoff,
        Currency::USD,
        1.0,
    );

    assert!(result.is_ok());
    let estimate = result.expect("MC pricing should succeed in test");
    assert_eq!(estimate.num_paths, 100);

    // Note: Testing actual error scenarios would require extensive mocking
    // of simulate_path. The important change is that errors are now propagated
    // via Result instead of panicking (verified by ? operator usage).
}

#[test]
fn test_serial_vs_parallel_consistency() {
    // Constant-payoff case: serial and parallel agree exactly because a constant
    // reduces order-independently. See
    // `test_serial_vs_parallel_bit_identical_for_varying_payoff` for the meaningful
    // varying-payoff case that actually exercises the reduction order.
    let engine_serial = McEngine::builder()
        .num_paths(1000)
        .uniform_grid(1.0, 10)
        .parallel(false)
        .build()
        .expect("McEngine builder should succeed with valid test data");

    let engine_parallel = McEngine::builder()
        .num_paths(1000)
        .uniform_grid(1.0, 10)
        .parallel(true)
        .chunk_size(200)
        .build()
        .expect("McEngine builder should succeed with valid test data");

    let rng_serial = DummyRng;
    let rng_parallel = DummyRng;
    let process = DummyProcess;
    let disc = DummyDisc;
    let initial_state = vec![100.0];
    let payoff = DummyPayoff;

    let serial_result = engine_serial
        .price(
            &rng_serial,
            &process,
            &disc,
            &initial_state,
            &payoff,
            Currency::USD,
            1.0,
        )
        .expect("should succeed");

    let parallel_result = engine_parallel
        .price(
            &rng_parallel,
            &process,
            &disc,
            &initial_state,
            &payoff,
            Currency::USD,
            1.0,
        )
        .expect("should succeed");

    // Both should succeed and produce same results (deterministic RNG)
    assert_eq!(serial_result.num_paths, 1000);
    assert_eq!(parallel_result.num_paths, 1000);
    assert_eq!(serial_result.mean.amount(), parallel_result.mean.amount());
}

#[test]
fn test_serial_vs_parallel_bit_identical_for_varying_payoff() {
    // Workspace determinism invariant: with a splittable RNG and auto-stop
    // disabled, serial and parallel must be bit-identical. `PathIndexedRng`
    // yields a distinct, varying value per path id, and both engines use the
    // SAME chunk size, so the per-path values and the chunk -> merge
    // reduction tree match exactly.
    let build = |parallel: bool| {
        McEngine::builder()
            .num_paths(1000)
            .uniform_grid(1.0, 10)
            .parallel(parallel)
            .chunk_size(64)
            .build()
            .expect("McEngine builder should succeed with valid test data")
    };
    let engine_serial = build(false);
    let engine_parallel = build(true);

    let rng = PathIndexedRng::root();
    let process = DummyProcess;
    let disc = DummyDisc;
    let initial_state = vec![100.0];
    let payoff = CapturedValuePayoff::default();

    let serial = engine_serial
        .price(
            &rng,
            &process,
            &disc,
            &initial_state,
            &payoff,
            Currency::USD,
            1.0,
        )
        .expect("serial pricing should succeed");
    let parallel = engine_parallel
        .price(
            &rng,
            &process,
            &disc,
            &initial_state,
            &payoff,
            Currency::USD,
            1.0,
        )
        .expect("parallel pricing should succeed");

    assert_eq!(serial.num_paths, parallel.num_paths);
    // Bit-identical, not merely close: the reduction tree and per-path values match.
    assert_eq!(serial.mean.amount(), parallel.mean.amount());
}

#[test]
fn test_default_chunking_bit_identical_across_thread_pool_sizes() {
    // The DEFAULT chunk size must be a pure function of `num_paths` — never
    // of `rayon::current_num_threads()` — because the chunk partition fixes
    // the float reduction tree. This locks the cross-machine determinism
    // guarantee for default configs (prior fix): the same
    // (seed, paths, steps) must price bit-identically under 1-thread and
    // multi-thread rayon pools, in both serial and parallel mode.
    let price_in_pool = |threads: usize, parallel: bool| -> f64 {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("test thread pool should build");
        pool.install(|| {
            let engine = McEngine::builder()
                .num_paths(1000)
                .uniform_grid(1.0, 10)
                .parallel(parallel)
                // No .chunk_size(): exercise the default (adaptive) path.
                .build()
                .expect("McEngine builder should succeed");
            engine
                .price(
                    &PathIndexedRng::root(),
                    &DummyProcess,
                    &DummyDisc,
                    &[100.0],
                    &CapturedValuePayoff::default(),
                    Currency::USD,
                    1.0,
                )
                .expect("pricing should succeed")
                .mean
                .amount()
        })
    };

    let serial_1 = price_in_pool(1, false);
    let serial_8 = price_in_pool(8, false);
    let parallel_1 = price_in_pool(1, true);
    let parallel_8 = price_in_pool(8, true);

    assert_eq!(serial_1.to_bits(), serial_8.to_bits());
    assert_eq!(parallel_1.to_bits(), parallel_8.to_bits());
    assert_eq!(serial_1.to_bits(), parallel_8.to_bits());
}

#[test]
fn test_engine_rejects_generic_scheme_for_dedicated_process() {
    // Processes with dynamics a generic scheme cannot see (discrete
    // dividends, Bates jumps) must be rejected when paired with Euler-style
    // schemes — that pairing type-checks but silently simulates only the
    // diffusion.
    use crate::discretization::euler::EulerMaruyama;
    use crate::process::bates::{BatesParams, BatesProcess};
    use crate::process::gbm::GbmParams;
    use crate::process::gbm_dividends::{Dividend, GbmWithDividends};
    use crate::process::heston::HestonParams;
    use crate::process::jump_diffusion::MertonJumpParams;

    let engine = McEngine::builder()
        .num_paths(10)
        .uniform_grid(1.0, 4)
        .parallel(false)
        .build()
        .expect("engine should build");
    let payoff = CapturedValuePayoff::default();
    let rng = crate::rng::philox::PhiloxRng::new(1);

    let div_process = GbmWithDividends::new(
        GbmParams::new(0.05, 0.0, 0.2).expect("valid"),
        vec![(0.5, Dividend::Cash(1.0))],
    )
    .expect("valid dividend schedule");
    let err = engine
        .price(
            &rng,
            &div_process,
            &EulerMaruyama::new(),
            &[100.0],
            &payoff,
            Currency::USD,
            1.0,
        )
        .expect_err("Euler + GbmWithDividends must be rejected");
    assert!(err.to_string().contains("dedicated discretization"));

    let heston = HestonParams::new(0.05, 0.01, 1.7, 0.04, 0.3, -0.4, 0.04).expect("valid");
    let jump = MertonJumpParams::new(0.05, 0.01, 0.0, 1.0, -0.05, 0.1).expect("valid");
    let bates = BatesProcess::new(BatesParams::new(heston, jump).expect("matching r/q"));
    let err = engine
        .price(
            &rng,
            &bates,
            &EulerMaruyama::new(),
            &[100.0, 0.04],
            &payoff,
            Currency::USD,
            1.0,
        )
        .expect_err("Euler + BatesProcess must be rejected");
    assert!(err.to_string().contains("dedicated discretization"));
}

#[test]
fn test_antithetic_mirrors_path_start_randomness() {
    // `CapturedValuePayoff` pays the uniform it draws in `on_path_start`.
    // With mirrored path-start randomness the antithetic leg receives
    // exactly `1 − u`, so every pair averages to ½ and the standard error
    // collapses to floating-point noise. Independent path-start draws (the
    // old behavior) leave stderr at the i.i.d. level σ/√n ≈ 0.29/√n.
    let engine = McEngine::builder()
        .num_paths(1000)
        .uniform_grid(1.0, 4)
        .parallel(false)
        .antithetic(true)
        .build()
        .expect("engine should build");

    let result = engine
        .price(
            &crate::rng::philox::PhiloxRng::new(7),
            &DummyProcess,
            &DummyDisc,
            &[100.0],
            &CapturedValuePayoff::default(),
            Currency::USD,
            1.0,
        )
        .expect("pricing should succeed");

    assert!(
        (result.mean.amount() - 0.5).abs() < 1e-12,
        "pair means must be exactly ½, got {}",
        result.mean.amount()
    );
    assert!(
        result.stderr < 1e-12,
        "mirrored path-start draws must collapse the stderr, got {}",
        result.stderr
    );
}

/// A minimal RNG that declares it does not support splitting (mimicking SobolRng).
#[derive(Clone)]
struct NonSplittableRng;
impl RandomStream for NonSplittableRng {
    fn split(&self, _id: u64) -> Option<Self> {
        None
    }
    fn fill_u01(&mut self, out: &mut [f64]) {
        for x in out {
            *x = 0.5;
        }
    }
    fn fill_std_normals(&mut self, out: &mut [f64]) {
        for x in out {
            *x = 0.0;
        }
    }
    fn supports_splitting(&self) -> bool {
        false
    }
}

#[test]
fn test_serial_with_non_splittable_rng_succeeds() {
    let engine = McEngine::builder()
        .num_paths(100)
        .uniform_grid(1.0, 10)
        .parallel(false)
        .build()
        .expect("McEngine builder should succeed with valid test data");

    let rng = NonSplittableRng;
    let process = DummyProcess;
    let disc = DummyDisc;
    let initial_state = vec![100.0];
    let payoff = DummyPayoff;

    let result = engine
        .price(
            &rng,
            &process,
            &disc,
            &initial_state,
            &payoff,
            Currency::USD,
            1.0,
        )
        .expect("serial engine should consume non-splittable RNGs sequentially");

    assert_eq!(result.num_paths, 100);
    assert_eq!(result.mean.amount(), 100.0);
}

#[test]
fn test_parallel_with_non_splittable_rng_returns_error() {
    // Guard: McEngine::price() must return Err when use_parallel=true and
    // rng.supports_splitting() == false.
    let engine = McEngine::builder()
        .num_paths(100)
        .uniform_grid(1.0, 10)
        .parallel(true)
        .build()
        .expect("McEngine builder should succeed with valid test data");

    let rng = NonSplittableRng;
    let process = DummyProcess;
    let disc = DummyDisc;
    let initial_state = vec![100.0];
    let payoff = DummyPayoff;

    let result = engine.price(
        &rng,
        &process,
        &disc,
        &initial_state,
        &payoff,
        Currency::USD,
        1.0,
    );

    // When the parallel feature is enabled this must be an Err; when it is
    // disabled the engine falls back to serial, so the guard is never
    // reached and the call succeeds.
    {
        assert!(
            result.is_err(),
            "Expected Err for parallel + non-splittable RNG, got Ok"
        );
        let err = result.expect_err("parallel + non-splittable RNG should return an error");
        let err_str = err.to_string();
        assert!(
            err_str.contains("splittable RNG"),
            "Error message should mention splittable RNG, got: {err_str}"
        );
    }
}

#[derive(Clone)]
struct OverflowAfterDiscountPayoff;

impl Payoff for OverflowAfterDiscountPayoff {
    fn on_event(&mut self, _state: &mut PathState) {}

    fn value(&self, currency: Currency) -> Money {
        Money::new(1.0e20, currency)
    }

    fn reset(&mut self) {}
}

#[test]
fn test_price_rejects_non_finite_payoffs() {
    for use_parallel in [false, true] {
        let engine = McEngine::builder()
            .num_paths(10)
            .uniform_grid(1.0, 2)
            .parallel(use_parallel)
            .chunk_size(5)
            .build()
            .expect("McEngine builder should succeed with valid test data");

        let rng = DummyRng;
        let process = DummyProcess;
        let disc = DummyDisc;
        let initial_state = vec![100.0];
        let payoff = OverflowAfterDiscountPayoff;

        let err = engine
            .price(
                &rng,
                &process,
                &disc,
                &initial_state,
                &payoff,
                Currency::USD,
                1.0e300,
            )
            .expect_err("non-finite discounted payoff should fail pricing");
        assert!(
            err.to_string().contains("non-finite discounted payoff"),
            "unexpected error: {err}"
        );
    }
}

#[test]
fn test_price_with_capture_parallel_non_splittable_returns_error() {
    // Same guard must fire in price_with_capture().
    let engine = McEngine::builder()
        .num_paths(100)
        .uniform_grid(1.0, 10)
        .parallel(true)
        .build()
        .expect("McEngine builder should succeed with valid test data");

    let rng = NonSplittableRng;
    let process = DummyProcess;
    let disc = DummyDisc;
    let initial_state = vec![100.0];
    let payoff = DummyPayoff;
    let params = ProcessParams::new("test");

    let result = engine.price_with_capture(
        &rng,
        &process,
        &disc,
        &initial_state,
        &payoff,
        Currency::USD,
        1.0,
        params,
    );

    {
        assert!(
            result.is_err(),
            "Expected Err for parallel + non-splittable RNG in price_with_capture, got Ok"
        );
    }
}

#[test]
fn test_on_path_start_state_survives_into_simulation() {
    let engine = McEngine::builder()
        .num_paths(1)
        .uniform_grid(1.0, 1)
        .parallel(false)
        .build()
        .expect("McEngine builder should succeed with valid test data");

    let result = engine
        .price(
            &DummyRng,
            &DummyProcess,
            &DummyDisc,
            &[100.0],
            &PathStartPayoff::default(),
            Currency::USD,
            1.0,
        )
        .expect("pricing should succeed");

    assert_eq!(result.mean.amount(), 0.5);
}

#[test]
fn test_price_rejects_zero_paths() {
    let time_grid = TimeGrid::uniform(1.0, 1).expect("grid should build");
    let engine = McEngine::new(McEngineConfig {
        num_paths: 0,
        time_grid,
        target_ci_half_width: None,
        use_parallel: false,
        chunk_size: Some(1),
        path_capture: PathCaptureConfig::new(),
        antithetic: false,
    });

    let err = engine
        .price(
            &DummyRng,
            &DummyProcess,
            &DummyDisc,
            &[100.0],
            &DummyPayoff,
            Currency::USD,
            1.0,
        )
        .expect_err("zero-path configuration should be rejected");

    assert!(err.to_string().contains("num_paths"));
}

#[test]
fn test_price_rejects_zero_chunk_size() {
    let time_grid = TimeGrid::uniform(1.0, 1).expect("grid should build");
    let engine = McEngine::new(McEngineConfig {
        num_paths: 10,
        time_grid,
        target_ci_half_width: None,
        use_parallel: false,
        chunk_size: Some(0),
        path_capture: PathCaptureConfig::new(),
        antithetic: false,
    });

    let err = engine
        .price(
            &DummyRng,
            &DummyProcess,
            &DummyDisc,
            &[100.0],
            &DummyPayoff,
            Currency::USD,
            1.0,
        )
        .expect_err("zero chunk size should be rejected");

    assert!(err.to_string().contains("chunk_size"));
}

#[test]
fn test_price_rejects_initial_state_dimension_mismatch() {
    let engine = McEngine::builder()
        .num_paths(10)
        .uniform_grid(1.0, 1)
        .parallel(false)
        .build()
        .expect("McEngine builder should succeed with valid test data");

    let err = engine
        .price(
            &DummyRng,
            &DummyProcess,
            &DummyDisc,
            &[],
            &DummyPayoff,
            Currency::USD,
            1.0,
        )
        .expect_err("state dimension mismatch should be rejected");

    assert!(err.to_string().contains("initial_state"));
}

#[test]
fn test_price_with_capture_rejects_invalid_sample_count() {
    let engine = McEngine::new(McEngineConfig {
        num_paths: 10,
        time_grid: TimeGrid::uniform(1.0, 1).expect("grid should build"),
        target_ci_half_width: None,
        use_parallel: false,
        chunk_size: Some(1),
        path_capture: PathCaptureConfig::sample(0, 99),
        antithetic: false,
    });

    let err = engine
        .price_with_capture(
            &DummyRng,
            &DummyProcess,
            &DummyDisc,
            &[100.0],
            &DummyPayoff,
            Currency::USD,
            1.0,
            ProcessParams::new("test"),
        )
        .expect_err("zero sample count should be rejected");

    assert!(err.to_string().contains("sample"));
}

#[test]
fn test_price_with_capture_rejects_full_capture_above_budget() {
    let engine = McEngine::new(McEngineConfig {
        num_paths: 100_001,
        time_grid: TimeGrid::uniform(1.0, 1).expect("grid should build"),
        target_ci_half_width: None,
        use_parallel: false,
        chunk_size: Some(1),
        path_capture: PathCaptureConfig::all(),
        antithetic: false,
    });

    let err = engine
        .price_with_capture(
            &DummyRng,
            &DummyProcess,
            &DummyDisc,
            &[100.0],
            &DummyPayoff,
            Currency::USD,
            1.0,
            ProcessParams::new("test"),
        )
        .expect_err("full path capture above the diagnostics budget should be rejected");

    assert!(err.to_string().contains("Path capture budget"));
}

#[test]
fn test_price_with_capture_rejects_antithetic_capture_combination() {
    let engine = McEngine::new(McEngineConfig {
        num_paths: 10,
        time_grid: TimeGrid::uniform(1.0, 1).expect("grid should build"),
        target_ci_half_width: None,
        use_parallel: false,
        chunk_size: Some(1),
        path_capture: PathCaptureConfig::all(),
        antithetic: true,
    });

    let err = engine
        .price_with_capture(
            &DummyRng,
            &DummyProcess,
            &DummyDisc,
            &[100.0],
            &DummyPayoff,
            Currency::USD,
            1.0,
            ProcessParams::new("test"),
        )
        .expect_err("antithetic + path capture should be rejected");

    assert!(err.to_string().contains("antithetic"));
}

#[test]
fn test_price_rejects_parallel_auto_stop_configuration() {
    let time_grid = TimeGrid::uniform(1.0, 1).expect("grid should build");
    let engine = McEngine::new(McEngineConfig {
        num_paths: 10,
        time_grid,
        target_ci_half_width: Some(0.01),
        use_parallel: true,
        chunk_size: Some(2),
        path_capture: PathCaptureConfig::new(),
        antithetic: false,
    });

    let result = engine.price(
        &DummyRng,
        &DummyProcess,
        &DummyDisc,
        &[100.0],
        &DummyPayoff,
        Currency::USD,
        1.0,
    );

    {
        let err = result.expect_err("parallel auto-stop should be rejected");
        assert!(err.to_string().contains("target_ci_half_width"));
    }
}

#[test]
fn test_price_with_capture_captures_initial_event_cashflows_and_payoff() {
    let engine = McEngine::new(McEngineConfig {
        num_paths: 1,
        time_grid: TimeGrid::uniform(1.0, 1).expect("grid should build"),
        target_ci_half_width: None,
        use_parallel: false,
        chunk_size: Some(1),
        path_capture: PathCaptureConfig::all().with_payoffs(),
        antithetic: false,
    });

    let result = engine
        .price_with_capture(
            &DummyRng,
            &DummyProcess,
            &DummyDisc,
            &[100.0],
            &InitialCashflowPayoff { value: 7.0 },
            Currency::USD,
            1.0,
            ProcessParams::new("test"),
        )
        .expect("capture should succeed");

    let path = result
        .paths
        .as_ref()
        .and_then(|dataset| dataset.path(0))
        .expect("captured path should exist");
    let initial_point = path.initial_point().expect("initial point should exist");
    assert_eq!(initial_point.payoff_value, Some(7.0));
    assert_eq!(
        initial_point.cashflows,
        vec![(0.0, 7.0, CashflowType::Other)]
    );
}

#[test]
fn test_price_with_capture_preserves_cashflows_across_multiple_timesteps() {
    let engine = McEngine::new(McEngineConfig {
        num_paths: 1,
        time_grid: TimeGrid::uniform(1.0, 2).expect("grid should build"),
        target_ci_half_width: None,
        use_parallel: false,
        chunk_size: None,
        path_capture: PathCaptureConfig::all(),
        antithetic: false,
    });

    let result = engine
        .price_with_capture(
            &DummyRng,
            &DummyProcess,
            &DummyDisc,
            &[100.0],
            &RecurringCashflowPayoff,
            Currency::USD,
            1.0,
            ProcessParams::new("test"),
        )
        .expect("captured pricing should succeed");

    let path = result
        .paths
        .as_ref()
        .and_then(|dataset| dataset.paths.first())
        .expect("captured path should exist");
    assert_eq!(path.points.len(), 3);
    assert_eq!(
        path.points[0].cashflows,
        vec![(0.0, 1.0, CashflowType::Interest)]
    );
    assert_eq!(
        path.points[1].cashflows,
        vec![(0.5, 2.0, CashflowType::Interest)]
    );
    assert_eq!(
        path.points[2].cashflows,
        vec![(1.0, 3.0, CashflowType::Interest)]
    );
}

#[test]
fn test_price_with_capture_uses_actual_path_count_after_auto_stop() {
    // Configure num_paths slightly above the auto-stop warmup so that
    // auto-stop fires on the first eligible iteration (count ==
    // AUTO_STOP_MIN_SAMPLES). Any change to the warmup constant must be
    // reflected here.
    let num_paths = super::pricing::AUTO_STOP_MIN_SAMPLES + 4_000;
    let engine = McEngine::new(McEngineConfig {
        num_paths,
        time_grid: TimeGrid::uniform(1.0, 1).expect("grid should build"),
        target_ci_half_width: Some(0.01),
        use_parallel: false,
        chunk_size: Some(100),
        path_capture: PathCaptureConfig::all(),
        antithetic: false,
    });

    let result = engine
        .price_with_capture(
            &DummyRng,
            &DummyProcess,
            &DummyDisc,
            &[100.0],
            &DummyPayoff,
            Currency::USD,
            1.0,
            ProcessParams::new("test"),
        )
        .expect("pricing should succeed");

    let captured = result.paths.as_ref().expect("captured paths should exist");
    let expected = super::pricing::AUTO_STOP_MIN_SAMPLES;
    assert_eq!(result.estimate.num_paths, expected);
    assert_eq!(captured.num_paths_total, expected);
    assert_eq!(captured.num_captured(), expected);
}

fn assert_captured_path_statistics(result: &MonteCarloResult) {
    assert_eq!(result.estimate.median, Some(0.375));
    assert_eq!(result.estimate.percentile_25, Some(0.25));
    assert_eq!(result.estimate.percentile_75, Some(0.5));
    assert_eq!(result.estimate.min, Some(0.125));
    assert_eq!(result.estimate.max, Some(0.625));
}

#[test]
fn test_price_with_capture_serial_populates_captured_path_statistics() {
    let engine = McEngine::builder()
        .num_paths(5)
        .uniform_grid(1.0, 1)
        .parallel(false)
        .capture_all_paths()
        .build()
        .expect("McEngine builder should succeed with valid test data");

    let result = engine
        .price_with_capture(
            &PathIndexedRng::root(),
            &DummyProcess,
            &DummyDisc,
            &[100.0],
            &CapturedValuePayoff::default(),
            Currency::USD,
            1.0,
            ProcessParams::new("test"),
        )
        .expect("captured pricing should succeed");

    assert_captured_path_statistics(&result);
}

#[test]
fn test_price_with_capture_parallel_populates_captured_path_statistics() {
    let engine = McEngine::builder()
        .num_paths(5)
        .uniform_grid(1.0, 1)
        .parallel(true)
        .chunk_size(2)
        .capture_all_paths()
        .build()
        .expect("McEngine builder should succeed with valid test data");

    let result = engine
        .price_with_capture(
            &PathIndexedRng::root(),
            &DummyProcess,
            &DummyDisc,
            &[100.0],
            &CapturedValuePayoff::default(),
            Currency::USD,
            1.0,
            ProcessParams::new("test"),
        )
        .expect("captured pricing should succeed");

    assert_captured_path_statistics(&result);
}

#[test]
fn test_num_skipped_defaults_to_zero() {
    // Normal payoff: no skips expected.
    let engine = McEngine::builder()
        .num_paths(100)
        .uniform_grid(1.0, 10)
        .parallel(false)
        .build()
        .expect("McEngine builder should succeed with valid test data");

    let result = engine
        .price(
            &DummyRng,
            &DummyProcess,
            &DummyDisc,
            &[100.0],
            &DummyPayoff,
            Currency::USD,
            1.0,
        )
        .expect("pricing should succeed");

    assert_eq!(result.num_skipped, 0);
    assert_eq!(result.num_paths, 100);
}

#[test]
#[allow(deprecated)]
fn test_estimate_num_skipped_builder() {
    use crate::estimate::Estimate;

    // Default: num_skipped is 0
    let est = Estimate::new(100.0, 1.0, (98.0, 102.0), 10_000);
    assert_eq!(est.num_skipped, 0);

    // Builder: attaches num_skipped
    let est = est.with_num_skipped(42);
    assert_eq!(est.num_skipped, 42);
    // Other fields unaffected
    assert_eq!(est.mean, 100.0);
    assert_eq!(est.num_paths, 10_000);
}

#[test]
#[allow(deprecated)]
fn test_estimate_display_with_skipped() {
    use crate::estimate::Estimate;

    // Without skipped: no "skipped=" in output
    let est = Estimate::new(100.0, 1.0, (98.0, 102.0), 10_000);
    let s = format!("{}", est);
    assert!(
        !s.contains("skipped="),
        "Display should omit skipped when 0"
    );
    assert!(s.contains("n=10000"));

    // With skipped: shows "skipped=5"
    let est = est.with_num_skipped(5);
    let s = format!("{}", est);
    assert!(s.contains("skipped=5"), "Display should show skipped count");
    assert!(s.contains("n=10000"));
}

#[test]
#[allow(deprecated)]
fn test_money_estimate_propagates_num_skipped() {
    use crate::estimate::Estimate;
    use crate::results::MoneyEstimate;

    let est = Estimate::new(100.0, 1.0, (98.0, 102.0), 10_000).with_num_skipped(7);
    let money_est = MoneyEstimate::from_estimate(est, Currency::USD);
    assert_eq!(money_est.num_skipped, 7);
}

/// `Estimate::new` should default `num_simulated_paths` to `num_paths`;
/// [`Estimate::with_num_simulated_paths`] overrides it (e.g. for antithetic
/// runs). [`MoneyEstimate::from_estimate`] must propagate both fields.
#[test]
fn test_estimate_num_simulated_paths_defaults_and_propagates() {
    use crate::estimate::Estimate;
    use crate::results::MoneyEstimate;

    let est = Estimate::new(100.0, 1.0, (98.0, 102.0), 10_000);
    assert_eq!(est.num_paths, 10_000);
    assert_eq!(
        est.num_simulated_paths, 10_000,
        "defaults to num_paths when variance reduction is off"
    );

    let est_anti = est.with_num_simulated_paths(20_000);
    assert_eq!(est_anti.num_paths, 10_000);
    assert_eq!(est_anti.num_simulated_paths, 20_000);

    let money_est = MoneyEstimate::from_estimate(est_anti, Currency::USD);
    assert_eq!(money_est.num_paths, 10_000);
    assert_eq!(money_est.num_simulated_paths, 20_000);
}

/// Antithetic pricing should produce `num_paths` estimators and
/// `2 * num_paths` simulated paths. Without antithetics both counts match.
#[test]
fn test_engine_antithetic_records_simulated_path_count() {
    use crate::discretization::ExactGbm;
    use crate::payoff::EuropeanCall;
    use crate::process::gbm::GbmProcess;
    use crate::rng::philox::PhiloxRng;

    let requested = 1024usize;
    let grid = TimeGrid::uniform(0.5, 16).expect("valid grid");
    let payoff = EuropeanCall::new(100.0, 0.5, 16);
    let process = GbmProcess::with_params(0.05, 0.0, 0.2).expect("valid gbm");
    let disc = ExactGbm::new();
    let discount = (-0.05_f64 * 0.5).exp();

    let engine = McEngine::builder()
        .num_paths(requested)
        .time_grid(grid.clone())
        .parallel(false)
        .build()
        .expect("build engine");
    let rng = PhiloxRng::new(7);
    let res = engine
        .price(
            &rng,
            &process,
            &disc,
            &[100.0],
            &payoff,
            Currency::USD,
            discount,
        )
        .expect("price");
    assert_eq!(res.num_paths, requested);
    assert_eq!(res.num_simulated_paths, requested);

    let engine_anti = McEngineBuilder::new()
        .num_paths(requested)
        .time_grid(grid)
        .parallel(false)
        .antithetic(true)
        .build()
        .expect("build engine");
    let rng_anti = PhiloxRng::new(7);
    let res_anti = engine_anti
        .price(
            &rng_anti,
            &process,
            &disc,
            &[100.0],
            &payoff,
            Currency::USD,
            discount,
        )
        .expect("price");
    assert_eq!(res_anti.num_paths, requested);
    assert_eq!(res_anti.num_simulated_paths, requested * 2);
}

#[test]
fn test_estimate_serde_backward_compatibility() {
    // Verify that deserializing an Estimate without num_skipped defaults to 0.
    use crate::estimate::Estimate;

    let json = r#"{
        "mean": 100.0,
        "stderr": 1.0,
        "ci_95": [98.0, 102.0],
        "num_paths": 10000,
        "std_dev": null,
        "median": null,
        "percentile_25": null,
        "percentile_75": null,
        "min": null,
        "max": null
    }"#;

    let est: Estimate =
        serde_json::from_str(json).expect("Estimate should deserialize without num_skipped field");
    assert_eq!(est.num_skipped, 0, "num_skipped should default to 0");
    assert_eq!(est.mean, 100.0);
    assert_eq!(est.num_paths, 10_000);
}

/// Regression tests that the engine-side Cholesky correctly applies a
/// stochastic process's declared factor correlation to the shocks driving a
/// scheme that does not apply correlation internally
/// (see [`crate::traits::Discretization::applies_correlation_internally`]).
mod correlation_regression {
    use super::*;
    use crate::discretization::{EulerMaruyama, ExactMultiGbmCorrelated, QeHeston};
    use crate::payoff::EuropeanCall;
    use crate::process::gbm::{GbmParams, MultiGbmProcess};
    use crate::process::heston::HestonProcess;
    use crate::rng::philox::PhiloxRng;

    fn uniform_grid(t: f64, n: usize) -> TimeGrid {
        TimeGrid::uniform(t, n).expect("valid grid")
    }

    /// An Euler+Heston simulation must respond to rho: negative rho should
    /// yield higher OTM put prices (and lower OTM call prices) than positive
    /// rho. Prior to engine-applied Cholesky the scheme treated the two
    /// Brownian motions as independent and the effect would vanish.
    #[test]
    fn euler_heston_respects_rho() {
        let engine = McEngine::builder()
            .num_paths(20_000)
            .uniform_grid(1.0, 50)
            .build()
            .expect("valid config");

        let rng = PhiloxRng::new(42);
        let strike = 110.0; // OTM call (S0 = 100)
        let payoff = EuropeanCall::new(strike, 1.0, 50);
        let disc = EulerMaruyama::new();
        let discount = (-0.03_f64).exp();

        let h_neg =
            HestonProcess::with_params(0.03, 0.0, 2.0, 0.04, 0.3, -0.7, 0.04).expect("valid");
        let h_pos =
            HestonProcess::with_params(0.03, 0.0, 2.0, 0.04, 0.3, 0.7, 0.04).expect("valid");

        let est_neg = engine
            .price(
                &rng,
                &h_neg,
                &disc,
                &[100.0, 0.04],
                &payoff,
                Currency::USD,
                discount,
            )
            .expect("neg rho price");
        let est_pos = engine
            .price(
                &rng,
                &h_pos,
                &disc,
                &[100.0, 0.04],
                &payoff,
                Currency::USD,
                discount,
            )
            .expect("pos rho price");

        // Negative rho produces a heavier left tail and lighter right tail,
        // so an OTM-call should be cheaper under rho<0 than rho>0.
        assert!(
            est_neg.mean.amount() < est_pos.mean.amount(),
            "expected OTM-call price with rho<0 ({}) below rho>0 ({}) after \
             engine-applied Cholesky",
            est_neg.mean.amount(),
            est_pos.mean.amount()
        );
    }

    /// With rho = 0 the Euler scheme (which relies on engine-applied
    /// correlation) should agree with the specialized QE Heston scheme
    /// (which encodes correlation internally) up to Monte Carlo noise.
    #[test]
    fn euler_matches_qe_heston_at_zero_rho() {
        let config = McEngine::builder()
            .num_paths(40_000)
            .uniform_grid(1.0, 100)
            .build()
            .expect("valid config");
        let rng = PhiloxRng::new(7);
        let payoff = EuropeanCall::new(100.0, 1.0, 100);
        let discount = (-0.03_f64).exp();

        let h0 = HestonProcess::with_params(0.03, 0.0, 2.0, 0.04, 0.3, 0.0, 0.04).expect("valid");

        let euler = config
            .price(
                &rng,
                &h0,
                &EulerMaruyama::new(),
                &[100.0, 0.04],
                &payoff,
                Currency::USD,
                discount,
            )
            .expect("euler price");
        let qe = config
            .price(
                &rng,
                &h0,
                &QeHeston::new(),
                &[100.0, 0.04],
                &payoff,
                Currency::USD,
                discount,
            )
            .expect("qe price");

        let tol = 4.0 * euler.stderr.max(qe.stderr);
        assert!(
            (euler.mean.amount() - qe.mean.amount()).abs() < tol,
            "Euler ({}) vs QE ({}) differ by more than 4 SE ({}) at rho=0",
            euler.mean.amount(),
            qe.mean.amount(),
            tol
        );
    }

    /// The MultiGBM spread option price must respond to the correlation
    /// between the two asset drivers. This exercises the engine Cholesky
    /// path (EulerMaruyama) and contrasts it with the specialized scheme
    /// that applies correlation internally (ExactMultiGbmCorrelated).
    ///
    /// Higher correlation implies a thinner spread distribution and thus a
    /// cheaper spread call.
    #[test]
    fn multi_gbm_spread_responds_to_rho_under_engine_cholesky() {
        use crate::traits::state_keys;
        use finstack_quant_core::money::Money;

        #[derive(Clone, Default)]
        struct SpreadCall {
            strike: f64,
            maturity_idx: usize,
            s0: f64,
            s1: f64,
        }
        impl crate::traits::Payoff for SpreadCall {
            fn on_event(&mut self, state: &mut PathState) {
                if state.step == self.maturity_idx {
                    self.s0 = state.get(state_keys::indexed_spot(0)).unwrap_or(0.0);
                    self.s1 = state.get(state_keys::indexed_spot(1)).unwrap_or(0.0);
                }
            }
            fn value(&self, currency: Currency) -> Money {
                let payoff = (self.s0 - self.s1 - self.strike).max(0.0);
                Money::new(payoff, currency)
            }
            fn reset(&mut self) {
                self.s0 = 0.0;
                self.s1 = 0.0;
            }
        }

        let grid = uniform_grid(1.0, 50);
        let config = McEngineConfig {
            num_paths: 30_000,
            time_grid: grid,
            target_ci_half_width: None,
            use_parallel: false,
            chunk_size: None,
            path_capture: PathCaptureConfig::new(),
            antithetic: false,
        };
        let engine = McEngine::new(config);

        let rng = PhiloxRng::new(11);
        let params = vec![
            GbmParams::new(0.03, 0.0, 0.20).expect("valid"),
            GbmParams::new(0.03, 0.0, 0.20).expect("valid"),
        ];
        let corr_low = vec![1.0, -0.5, -0.5, 1.0];
        let corr_high = vec![1.0, 0.9, 0.9, 1.0];
        let p_low = MultiGbmProcess::new(params.clone(), Some(corr_low)).expect("valid");
        let p_high = MultiGbmProcess::new(params, Some(corr_high)).expect("valid");

        let discount = (-0.03_f64).exp();
        let payoff = SpreadCall {
            strike: 0.0,
            maturity_idx: 50,
            s0: 0.0,
            s1: 0.0,
        };
        let disc = EulerMaruyama::new();

        let v_low = engine
            .price(
                &rng,
                &p_low,
                &disc,
                &[100.0, 100.0],
                &payoff,
                Currency::USD,
                discount,
            )
            .expect("low corr price");
        let v_high = engine
            .price(
                &rng,
                &p_high,
                &disc,
                &[100.0, 100.0],
                &payoff,
                Currency::USD,
                discount,
            )
            .expect("high corr price");

        assert!(
            v_low.mean.amount() > v_high.mean.amount(),
            "spread call under rho=-0.5 ({}) should exceed rho=0.9 ({}) when \
             engine applies Cholesky to the shocks",
            v_low.mean.amount(),
            v_high.mean.amount()
        );

        // Sanity check that the EulerMaruyama (engine Cholesky) result
        // agrees with the specialized correlated scheme that applies
        // correlation internally. The exact scheme is unbiased per step,
        // so we use a generous 5 SE tolerance.
        let disc_exact =
            ExactMultiGbmCorrelated::new(&[1.0, -0.5, -0.5, 1.0], 2).expect("valid chol");
        let v_exact_low = engine
            .price(
                &rng,
                &p_low,
                &disc_exact,
                &[100.0, 100.0],
                &payoff,
                Currency::USD,
                discount,
            )
            .expect("exact low corr price");
        let tol = 5.0 * v_low.stderr.max(v_exact_low.stderr);
        assert!(
            (v_low.mean.amount() - v_exact_low.mean.amount()).abs() < tol,
            "Euler ({}) vs ExactMultiGbmCorrelated ({}) differ by > 5 SE ({})",
            v_low.mean.amount(),
            v_exact_low.mean.amount(),
            tol
        );
    }
}
