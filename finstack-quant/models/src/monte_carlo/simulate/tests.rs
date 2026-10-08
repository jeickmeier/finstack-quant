//! Tests for [`simulate_paths`](super::simulate_paths): the GBM bit pin, the
//! process x scheme matrix, analytic moments per process, and input errors.

use super::*;
use crate::monte_carlo::engine::{McEngineConfig, PathCaptureConfig};
use crate::monte_carlo::payoff::vanilla::EuropeanCall;
use crate::volatility::local_vol::LocalVolSurface;
use finstack_quant_core::currency::Currency;

const SCHEMES: [SchemeSpec; 4] = [
    SchemeSpec::Default,
    SchemeSpec::Euler,
    SchemeSpec::LogEuler,
    SchemeSpec::Milstein,
];

/// Paths per moment check. Tolerances are stated in standard errors of the
/// sample mean at this size.
const N: usize = 20_000;

pub(super) fn spec(
    process: ProcessSpec,
    initial_state: &[f64],
    num_steps: usize,
) -> PathSimulationSpec {
    PathSimulationSpec {
        process,
        scheme: SchemeSpec::Default,
        initial_state: initial_state.to_vec(),
        time_grid: TimeGridSpec::Uniform {
            expiry: 1.0,
            num_steps,
        },
        num_paths: N,
        seed: 20_261_004,
        antithetic: false,
        fbm: None,
    }
}

fn gbm() -> ProcessSpec {
    ProcessSpec::Gbm(GbmParams::new(0.04, 0.01, 0.25).unwrap())
}

fn gbm_with_dividends() -> ProcessSpec {
    ProcessSpec::GbmWithDividends {
        params: GbmParams::new(0.04, 0.0, 0.25).unwrap(),
        dividends: vec![
            (0.6, Dividend::Cash(1.5)),
            (0.25, Dividend::Proportional(0.02)),
        ],
    }
}

fn multi_gbm() -> ProcessSpec {
    ProcessSpec::MultiGbm {
        assets: vec![
            GbmParams::new(0.04, 0.01, 0.25).unwrap(),
            GbmParams::new(0.04, 0.03, 0.15).unwrap(),
        ],
        correlation: Some(vec![1.0, 0.6, 0.6, 1.0]),
    }
}

fn brownian() -> ProcessSpec {
    ProcessSpec::Brownian(BrownianParams::new(0.3, 0.5).unwrap())
}

fn multi_brownian() -> ProcessSpec {
    ProcessSpec::MultiBrownian {
        mus: vec![0.3, -0.1],
        sigmas: vec![0.5, 0.2],
        correlation: Some(vec![1.0, -0.4, -0.4, 1.0]),
    }
}

fn multi_ou() -> ProcessSpec {
    ProcessSpec::MultiOu(
        MultiOuParams::new(
            vec![2.0, 0.5],
            vec![1.0, -1.0],
            vec![0.3, 0.4],
            Some(vec![1.0, 0.5, 0.5, 1.0]),
        )
        .unwrap(),
    )
}

fn hull_white() -> ProcessSpec {
    ProcessSpec::HullWhite1F(HullWhite1FParams::new(0.8, 0.01, 0.05).unwrap())
}

fn cir() -> ProcessSpec {
    ProcessSpec::Cir(CirParams::new(0.5, 0.04, 0.1).unwrap())
}

fn cir_plus_plus() -> ProcessSpec {
    ProcessSpec::CirPlusPlus {
        params: CirParams::new(0.5, 0.04, 0.1).unwrap(),
        shift_curve: vec![0.01, 0.02],
        shift_times: vec![0.0, 0.5],
    }
}

fn heston() -> ProcessSpec {
    ProcessSpec::Heston(HestonPricingParams::new(0.04, 0.01, 2.0, 0.05, 0.3, -0.7, 0.03).unwrap())
}

fn schwartz_smith() -> ProcessSpec {
    ProcessSpec::SchwartzSmith(
        SchwartzSmithParams::new(1.5, 0.3, 0.02, 0.15, 0.3)
            .unwrap()
            .with_lambda_x(0.05)
            .unwrap(),
    )
}

/// Every process with a valid initial state, in `ProcessSpec` order.
fn local_vol() -> ProcessSpec {
    let surface = LocalVolSurface::new(
        vec![0.25, 1.0],
        vec![80.0, 100.0, 120.0],
        vec![0.30, 0.22, 0.18, 0.26, 0.20, 0.17],
    )
    .unwrap();
    ProcessSpec::LocalVol(LocalVolParams::new(0.04, 0.01, surface).unwrap())
}

fn processes() -> Vec<(ProcessSpec, Vec<f64>)> {
    vec![
        (gbm(), vec![100.0]),
        (gbm_with_dividends(), vec![100.0]),
        (multi_gbm(), vec![100.0, 50.0]),
        (brownian(), vec![1.0]),
        (multi_brownian(), vec![1.0, -2.0]),
        (multi_ou(), vec![2.0, 0.0]),
        (hull_white(), vec![0.02]),
        (cir(), vec![0.03]),
        (cir_plus_plus(), vec![0.04]),
        (heston(), vec![100.0, 0.03]),
        (schwartz_smith(), vec![0.2, 4.0]),
        (local_vol(), vec![100.0]),
    ]
}

/// Whether `scheme` is documented as available for `process`.
fn supported(process: &ProcessSpec, scheme: SchemeSpec) -> bool {
    match scheme {
        SchemeSpec::Default => true,
        SchemeSpec::Euler => !matches!(process, ProcessSpec::GbmWithDividends { .. }),
        SchemeSpec::LogEuler => matches!(
            process,
            ProcessSpec::Gbm(_) | ProcessSpec::MultiGbm { .. } | ProcessSpec::LocalVol(_)
        ),
        SchemeSpec::Milstein => {
            matches!(process, ProcessSpec::Gbm(_) | ProcessSpec::MultiGbm { .. })
        }
    }
}

/// Small spec for every supported process x scheme pair.
fn supported_specs() -> Vec<PathSimulationSpec> {
    let mut specs = Vec::new();
    for (process, x0) in processes() {
        for scheme in SCHEMES {
            if supported(&process, scheme) {
                let mut s = spec(process.clone(), &x0, 6);
                s.scheme = scheme;
                s.num_paths = 9;
                specs.push(s);
            }
        }
    }
    specs
}

/// Values of `factor` at the last time on every stored path.
pub(super) fn terminal(summary: &PathSummary, factor: usize) -> Vec<f64> {
    let path_len = summary.times.len() * summary.dim;
    summary
        .values
        .chunks(path_len)
        .map(|path| path[path_len - summary.dim + factor])
        .collect()
}

pub(super) fn mean_var(xs: &[f64]) -> (f64, f64) {
    let n = xs.len() as f64;
    let mean = xs.iter().sum::<f64>() / n;
    let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
    (mean, var)
}

fn correlation(xs: &[f64], ys: &[f64]) -> f64 {
    let ((mx, vx), (my, vy)) = (mean_var(xs), mean_var(ys));
    let cov = xs
        .iter()
        .zip(ys)
        .map(|(x, y)| (x - mx) * (y - my))
        .sum::<f64>()
        / (xs.len() as f64 - 1.0);
    cov / (vx * vy).sqrt()
}

/// Assert the sample mean is within four standard errors of `expected`.
pub(super) fn assert_mean(xs: &[f64], expected: f64, what: &str) {
    let (mean, var) = mean_var(xs);
    let stderr = (var / xs.len() as f64).sqrt();
    assert!(
        (mean - expected).abs() <= 4.0 * stderr,
        "{what}: mean {mean} vs {expected}, stderr {stderr}"
    );
}

/// Assert the sample variance is within four standard errors of `expected`
/// (normal-sample standard error `σ² √(2/n)`).
fn assert_var(xs: &[f64], expected: f64, what: &str) {
    let (_, var) = mean_var(xs);
    let stderr = expected * (2.0 / xs.len() as f64).sqrt();
    assert!(
        (var - expected).abs() <= 4.0 * stderr,
        "{what}: variance {var} vs {expected}, stderr {stderr}"
    );
}

/// Assert the sample correlation is within four standard errors
/// `(1 - ρ²)/√n` of `expected`.
fn assert_correlation(xs: &[f64], ys: &[f64], expected: f64, what: &str) {
    let rho = correlation(xs, ys);
    let stderr = (1.0 - expected * expected) / (xs.len() as f64).sqrt();
    assert!(
        (rho - expected).abs() <= 4.0 * stderr,
        "{what}: correlation {rho} vs {expected}, stderr {stderr}"
    );
}

pub(super) fn error_message(spec: &PathSimulationSpec) -> String {
    simulate_paths(spec)
        .expect_err("spec should be rejected")
        .to_string()
}

// ---------------------------------------------------------------------------
// GBM bit pin
// ---------------------------------------------------------------------------

/// FNV-1a over the IEEE-754 bit patterns of every time, then every value.
fn bit_hash(summary: &PathSummary) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for value in summary.times.iter().chain(&summary.values) {
        for byte in value.to_bits().to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

/// The hashes and terminal spots were recorded from the GBM-only path
/// simulator that `simulate_paths` replaced.
#[test]
fn gbm_default_scheme_is_pinned_bit_for_bit() {
    let pinned = |r, q, sigma, spot, expiry, num_steps, num_paths, seed| {
        simulate_paths(&PathSimulationSpec {
            process: ProcessSpec::Gbm(GbmParams::new(r, q, sigma).unwrap()),
            scheme: SchemeSpec::Default,
            initial_state: vec![spot],
            time_grid: TimeGridSpec::Uniform { expiry, num_steps },
            num_paths,
            seed,
            antithetic: false,
            fbm: None,
        })
        .unwrap()
    };

    let a = pinned(0.04, 0.01, 0.25, 100.0, 1.3, 17, 32, 19);
    assert_eq!(bit_hash(&a), 0x7e9a_6884_54fb_d37c);
    assert_eq!(a.values[31 * 18 + 17].to_bits(), 0x4060_5556_4027_5291);

    let b = pinned(-0.01, 0.03, 0.6, 42.5, 0.37, 1, 5, 7_919);
    assert_eq!(bit_hash(&b), 0x2b4a_52ec_fdb1_426b);
    assert_eq!(b.values[4 * 2 + 1].to_bits(), 0x4040_f04d_39a6_c0fb);
}

// ---------------------------------------------------------------------------
// Every supported pair: shape, determinism, execution-mode independence
// ---------------------------------------------------------------------------

#[test]
fn every_pair_is_either_supported_or_a_named_error() {
    for (process, x0) in processes() {
        for scheme in SCHEMES {
            let mut s = spec(process.clone(), &x0, 4);
            s.scheme = scheme;
            s.num_paths = 3;
            let result = simulate_paths(&s);
            if supported(&process, scheme) {
                assert!(result.is_ok(), "{} + {scheme:?}", process.tag());
            } else {
                let message = result.expect_err("unsupported pair").to_string();
                assert!(
                    message.contains(process.tag()) && message.contains(scheme_tag(scheme)),
                    "{message}"
                );
            }
        }
    }
}

#[test]
fn every_supported_pair_has_the_documented_shape() {
    for s in supported_specs() {
        let out = simulate_paths(&s).unwrap();
        let dim = s.initial_state.len();
        assert_eq!(out.num_paths, 9);
        assert_eq!(out.num_simulated_paths, 9);
        assert_eq!(out.dim, dim);
        assert_eq!(out.factor_names.len(), dim);
        assert_eq!(out.times.len(), 7);
        assert_eq!(out.times[0], 0.0);
        assert_eq!(out.times[6], 1.0);
        assert_eq!(out.values.len(), 9 * 7 * dim);
        for path in out.values.chunks(7 * dim) {
            assert_eq!(&path[..dim], s.initial_state.as_slice());
        }
        assert!(out.values.iter().all(|value| value.is_finite()));
    }
}

#[test]
fn factor_names_follow_the_state_layout() {
    let names = |process: ProcessSpec, x0: &[f64]| {
        let mut s = spec(process, x0, 2);
        s.num_paths = 1;
        simulate_paths(&s).unwrap().factor_names
    };
    assert_eq!(names(gbm(), &[100.0]), ["spot"]);
    assert_eq!(names(gbm_with_dividends(), &[100.0]), ["spot"]);
    assert_eq!(names(multi_gbm(), &[100.0, 50.0]), ["spot_0", "spot_1"]);
    assert_eq!(names(brownian(), &[1.0]), ["x"]);
    assert_eq!(names(multi_brownian(), &[1.0, -2.0]), ["x_0", "x_1"]);
    assert_eq!(names(multi_ou(), &[2.0, 0.0]), ["x_0", "x_1"]);
    assert_eq!(names(hull_white(), &[0.02]), ["short_rate"]);
    assert_eq!(names(cir(), &[0.03]), ["short_rate"]);
    assert_eq!(names(cir_plus_plus(), &[0.04]), ["short_rate"]);
    assert_eq!(names(heston(), &[100.0, 0.03]), ["spot", "variance"]);
    assert_eq!(names(schwartz_smith(), &[0.2, 4.0]), ["x", "y"]);
}

#[test]
fn same_spec_reproduces_and_a_new_seed_differs() {
    for s in supported_specs() {
        let first = simulate_paths(&s).unwrap();
        assert_eq!(first, simulate_paths(&s).unwrap());

        let mut reseeded = s.clone();
        reseeded.seed += 1;
        let other = simulate_paths(&reseeded).unwrap();
        assert_eq!(first.times, other.times);
        assert_ne!(first.values, other.values, "{}", s.process.tag());
    }
}

#[test]
fn serial_and_parallel_runs_are_identical() {
    for mut s in supported_specs() {
        for antithetic in [false, true] {
            s.antithetic = antithetic;
            assert_eq!(
                dispatch(&s, false).unwrap(),
                dispatch(&s, true).unwrap(),
                "{} + {:?}",
                s.process.tag(),
                s.scheme
            );
        }
    }
}

#[test]
fn stream_values_do_not_depend_on_the_number_of_streams() {
    let mut few = spec(multi_gbm(), &[100.0, 50.0], 5);
    few.num_paths = 4;
    let mut many = few.clone();
    many.num_paths = 11;
    let (few, many) = (
        simulate_paths(&few).unwrap(),
        simulate_paths(&many).unwrap(),
    );
    assert_eq!(few.values, many.values[..few.values.len()]);
}

#[test]
fn explicit_times_grid_is_honoured() {
    let mut s = spec(hull_white(), &[0.02], 0);
    s.num_paths = 3;
    s.time_grid = TimeGridSpec::Times {
        times: vec![0.0, 0.1, 0.5, 2.0],
    };
    let out = simulate_paths(&s).unwrap();
    assert_eq!(out.values.len(), 3 * 4);
    for (time, expected) in out.times.iter().zip([0.0, 0.1, 0.5, 2.0]) {
        assert!((time - expected).abs() < 1e-15);
    }
}

// ---------------------------------------------------------------------------
// Agreement with the generic engine's captured paths
// ---------------------------------------------------------------------------

/// States captured by `McEngine` for the same process, scheme, seed and grid.
pub(super) fn engine_states<P, D>(process: &P, scheme: &D, x0: &[f64], num_paths: usize) -> Vec<f64>
where
    P: StochasticProcess + ProcessMetadata,
    D: Discretization<P> + Clone,
{
    let engine = McEngine::new(
        McEngineConfig::uniform(num_paths, 1.0, 6)
            .unwrap()
            .antithetic(false)
            .parallel(false)
            .path_capture(PathCaptureConfig::all()),
    );
    let dataset = engine
        .price_with_capture(
            &PhiloxRng::new(20_261_004),
            process,
            scheme,
            x0,
            &EuropeanCall::new(0.0, 1.0, 6),
            Currency::USD,
            1.0,
            process.metadata(),
        )
        .unwrap()
        .paths
        .unwrap();
    dataset
        .paths
        .iter()
        .flat_map(|path| path.points.iter().flat_map(|point| point.state.to_vec()))
        .collect()
}

#[test]
fn single_factor_correlated_and_internally_correlated_arms_match_the_engine() {
    let mut s = spec(gbm(), &[100.0], 6);
    s.num_paths = 8;
    let process = GbmProcess::with_params(0.04, 0.01, 0.25).unwrap();
    let out = simulate_paths(&s).unwrap();
    assert_eq!(out.values, engine_states(&process, &ExactGbm, &[100.0], 8));
    assert_eq!(out.num_simulated_paths, 8);

    let mut s = spec(multi_gbm(), &[100.0, 50.0], 6);
    s.num_paths = 8;
    let process = MultiGbmProcess::new(
        vec![
            GbmParams::new(0.04, 0.01, 0.25).unwrap(),
            GbmParams::new(0.04, 0.03, 0.15).unwrap(),
        ],
        Some(vec![1.0, 0.6, 0.6, 1.0]),
    )
    .unwrap();
    assert_eq!(
        simulate_paths(&s).unwrap().values,
        engine_states(&process, &ExactMultiGbm, &[100.0, 50.0], 8)
    );

    let mut s = spec(heston(), &[100.0, 0.03], 6);
    s.num_paths = 8;
    let process = HestonProcess::with_params(0.04, 0.01, 2.0, 0.05, 0.3, -0.7, 0.03).unwrap();
    assert_eq!(
        simulate_paths(&s).unwrap().values,
        engine_states(&process, &QeHeston::new(), &[100.0, 0.03], 8)
    );
}

// ---------------------------------------------------------------------------
// Antithetic sampling
// ---------------------------------------------------------------------------

#[test]
fn antithetic_partner_mirrors_the_stream_draws() {
    // Driftless Brownian motion from zero: every increment is `σ √Δt z`, so
    // the partner path is the exact negation of the stream's path.
    let mut s = spec(
        ProcessSpec::Brownian(BrownianParams::new(0.0, 0.5).unwrap()),
        &[0.0],
        5,
    );
    s.num_paths = 7;
    let plain = simulate_paths(&s).unwrap();
    s.antithetic = true;
    let paired = simulate_paths(&s).unwrap();

    assert_eq!(paired.num_paths, 7);
    assert_eq!(paired.num_simulated_paths, 14);
    assert_eq!(paired.values.len(), 14 * 6);
    for (stream, pair) in paired.values.chunks(12).enumerate() {
        let (primary, mirrored) = pair.split_at(6);
        assert_eq!(primary, &plain.values[stream * 6..(stream + 1) * 6]);
        for (p, m) in primary.iter().zip(mirrored) {
            assert_eq!(*m, -*p);
        }
    }
}

// ---------------------------------------------------------------------------
// Analytic moments (all within four standard errors at N = 20 000)
// ---------------------------------------------------------------------------

#[test]
fn gbm_exact_log_moments() {
    let out = simulate_paths(&spec(gbm(), &[100.0], 8)).unwrap();
    let logs: Vec<f64> = terminal(&out, 0).iter().map(|s| s.ln()).collect();
    assert_mean(
        &logs,
        100.0_f64.ln() + 0.04 - 0.01 - 0.5 * 0.25 * 0.25,
        "gbm log mean",
    );
    assert_var(&logs, 0.25 * 0.25, "gbm log variance");
}

#[test]
fn gbm_approximate_schemes_recover_the_forward() {
    // Euler and Milstein compound the drift as (1 + (r-q)Δt)^N; log-Euler is
    // exact for constant coefficients.
    let steps = 64_i32;
    let compounded = 100.0 * (1.0 + 0.03 / f64::from(steps)).powi(steps);
    for (scheme, forward) in [
        (SchemeSpec::Euler, compounded),
        (SchemeSpec::Milstein, compounded),
        (SchemeSpec::LogEuler, 100.0 * 0.03_f64.exp()),
    ] {
        let mut s = spec(gbm(), &[100.0], 64);
        s.scheme = scheme;
        let out = simulate_paths(&s).unwrap();
        assert_mean(&terminal(&out, 0), forward, &format!("gbm {scheme:?}"));
    }
}

#[test]
fn gbm_with_dividends_forward_drops_by_each_dividend() {
    // 2% of spot at t = 0.25 (a grid knot), then 1.5 in cash at t = 0.6
    // (inside a step), both carried forward at r = 4%.
    let out = simulate_paths(&spec(gbm_with_dividends(), &[100.0], 4)).unwrap();
    let after_first = 100.0 * (0.04_f64 * 0.25).exp() * 0.98;
    let after_second = after_first * (0.04_f64 * 0.35).exp() - 1.5;
    assert_mean(
        &terminal(&out, 0),
        after_second * (0.04_f64 * 0.4).exp(),
        "gbm with dividends forward",
    );
}

#[test]
fn multi_gbm_log_means_and_correlation() {
    for scheme in SCHEMES {
        let mut s = spec(multi_gbm(), &[100.0, 50.0], 32);
        s.scheme = scheme;
        let out = simulate_paths(&s).unwrap();
        let a: Vec<f64> = terminal(&out, 0).iter().map(|s| s.ln()).collect();
        let b: Vec<f64> = terminal(&out, 1).iter().map(|s| s.ln()).collect();
        assert_correlation(&a, &b, 0.6, &format!("multi gbm {scheme:?}"));
        if scheme == SchemeSpec::Default || scheme == SchemeSpec::LogEuler {
            assert_mean(&a, 100.0_f64.ln() + 0.03 - 0.5 * 0.0625, "asset 0 log mean");
            assert_mean(&b, 50.0_f64.ln() + 0.01 - 0.5 * 0.0225, "asset 1 log mean");
        }
    }
}

#[test]
fn brownian_moments() {
    let out = simulate_paths(&spec(brownian(), &[1.0], 8)).unwrap();
    let xs = terminal(&out, 0);
    assert_mean(&xs, 1.3, "brownian mean");
    assert_var(&xs, 0.25, "brownian variance");
}

#[test]
fn multi_brownian_moments_and_correlation() {
    let out = simulate_paths(&spec(multi_brownian(), &[1.0, -2.0], 8)).unwrap();
    let (a, b) = (terminal(&out, 0), terminal(&out, 1));
    assert_mean(&a, 1.3, "component 0 mean");
    assert_mean(&b, -2.1, "component 1 mean");
    assert_var(&a, 0.25, "component 0 variance");
    assert_var(&b, 0.04, "component 1 variance");
    assert_correlation(&a, &b, -0.4, "multi brownian");
}

#[test]
fn multi_ou_euler_mean() {
    // The Euler mean recursion is exact: m_N = θ + (x0 - θ)(1 - κΔt)^N.
    let steps = 252_i32;
    let out = simulate_paths(&spec(multi_ou(), &[2.0, 0.0], 252)).unwrap();
    let decay = |kappa: f64| (1.0 - kappa / f64::from(steps)).powi(steps);
    assert_mean(&terminal(&out, 0), 1.0 + decay(2.0), "ou component 0");
    assert_mean(&terminal(&out, 1), -1.0 + decay(0.5), "ou component 1");
    // Close to the continuous-time mean θ + (x0 - θ) e^{-κT} as well.
    assert!((decay(2.0) - (-2.0_f64).exp()).abs() < 2e-3);
}

#[test]
fn hull_white_moments() {
    let exact = simulate_paths(&spec(hull_white(), &[0.02], 4)).unwrap();
    let rates = terminal(&exact, 0);
    assert_mean(
        &rates,
        0.05 + (0.02 - 0.05) * (-0.8_f64).exp(),
        "hull-white exact mean",
    );
    assert_var(
        &rates,
        0.01 * 0.01 * (1.0 - (-1.6_f64).exp()) / 1.6,
        "hull-white exact variance",
    );

    let mut s = spec(hull_white(), &[0.02], 64);
    s.scheme = SchemeSpec::Euler;
    let euler = simulate_paths(&s).unwrap();
    assert_mean(
        &terminal(&euler, 0),
        0.05 + (0.02 - 0.05) * (1.0_f64 - 0.8 / 64.0).powi(64),
        "hull-white euler mean",
    );
}

#[test]
fn cir_means() {
    let qe = simulate_paths(&spec(cir(), &[0.03], 12)).unwrap();
    assert!(qe.values.iter().all(|rate| *rate >= 0.0));
    assert_mean(
        &terminal(&qe, 0),
        0.04 + (0.03 - 0.04) * (-0.5_f64).exp(),
        "cir qe mean",
    );

    // Feller holds comfortably (2κθ = 0.04 > σ² = 0.01), so truncation is
    // inactive and the Euler mean recursion is exact.
    let mut s = spec(cir(), &[0.03], 64);
    s.scheme = SchemeSpec::Euler;
    let euler = simulate_paths(&s).unwrap();
    assert_mean(
        &terminal(&euler, 0),
        0.04 + (0.03 - 0.04) * (1.0_f64 - 0.5 / 64.0).powi(64),
        "cir euler mean",
    );
}

#[test]
fn cir_plus_plus_is_the_cir_factor_plus_the_shift() {
    for scheme in [SchemeSpec::Default, SchemeSpec::Euler] {
        let mut shifted = spec(cir_plus_plus(), &[0.04], 12);
        shifted.scheme = scheme;
        shifted.num_paths = 50;
        let mut factor = spec(cir(), &[0.04 - 0.01], 12);
        factor.scheme = scheme;
        factor.num_paths = 50;
        let (shifted, factor) = (
            simulate_paths(&shifted).unwrap(),
            simulate_paths(&factor).unwrap(),
        );
        for (r, x) in shifted.values.chunks(13).zip(factor.values.chunks(13)) {
            assert_eq!(r[0], 0.04);
            for step in 1..13 {
                let shift = if shifted.times[step] >= 0.5 {
                    0.02
                } else {
                    0.01
                };
                assert_eq!(r[step], x[step] + shift);
            }
        }
    }

    let out = simulate_paths(&spec(cir_plus_plus(), &[0.04], 12)).unwrap();
    assert_mean(
        &terminal(&out, 0),
        0.02 + 0.04 + (0.03 - 0.04) * (-0.5_f64).exp(),
        "cir++ mean",
    );
}

#[test]
fn heston_qe_forward_and_variance_mean() {
    let out = simulate_paths(&spec(heston(), &[100.0, 0.03], 50)).unwrap();
    assert_mean(&terminal(&out, 0), 100.0 * 0.03_f64.exp(), "heston forward");
    assert_mean(
        &terminal(&out, 1),
        0.05 + (0.03 - 0.05) * (-2.0_f64).exp(),
        "heston variance mean",
    );
    assert!(terminal(&out, 1).iter().all(|v| *v >= 0.0));
}

#[test]
fn heston_euler_recovers_the_forward() {
    // The Euler spot recursion multiplies by (1 + (r-q)Δt + √v √Δt z), whose
    // conditional mean is (1 + (r-q)Δt) whatever the variance path does.
    let mut s = spec(heston(), &[100.0, 0.03], 100);
    s.scheme = SchemeSpec::Euler;
    let out = simulate_paths(&s).unwrap();
    assert_mean(
        &terminal(&out, 0),
        100.0 * (1.0_f64 + 0.03 / 100.0).powi(100),
        "heston euler forward",
    );
}

#[test]
fn schwartz_smith_log_spot_mean_and_futures() {
    let (kappa, lambda, mu_y) = (1.5_f64, 0.05, 0.02);
    let (x0, y0) = (0.2, 4.0);

    let exact = simulate_paths(&spec(schwartz_smith(), &[x0, y0], 4)).unwrap();
    let logs: Vec<f64> = terminal(&exact, 0)
        .iter()
        .zip(terminal(&exact, 1))
        .map(|(x, y)| x + y)
        .collect();
    let decay = (-kappa).exp();
    assert_mean(
        &logs,
        decay * x0 + y0 + mu_y - (1.0 - decay) * lambda / kappa,
        "schwartz-smith log spot",
    );
    let ProcessSpec::SchwartzSmith(params) = schwartz_smith() else {
        unreachable!()
    };
    let spots: Vec<f64> = logs.iter().map(|log| log.exp()).collect();
    assert_mean(
        &spots,
        SchwartzSmithProcess::new(params, x0, y0).futures_price(1.0),
        "schwartz-smith futures",
    );

    // Euler: x_N = a^N x0 - (λ/κ)(1 - a^N) with a = 1 - κΔt; y is exact.
    let mut s = spec(schwartz_smith(), &[x0, y0], 100);
    s.scheme = SchemeSpec::Euler;
    let euler = simulate_paths(&s).unwrap();
    let a_n = (1.0 - kappa / 100.0).powi(100);
    assert_mean(
        &terminal(&euler, 0),
        a_n * x0 - (lambda / kappa) * (1.0 - a_n),
        "schwartz-smith euler x",
    );
    assert_mean(&terminal(&euler, 1), y0 + mu_y, "schwartz-smith euler y");

    // First-step increments of the exact scheme (Δt = 0.25, four values per
    // time): corr(ΔX, ΔY) = ρ (1 - e^{-κΔt})/κ / sqrt((1 - e^{-2κΔt})/(2κ) Δt).
    let dt = 0.25_f64;
    let increments = |factor: usize| -> Vec<f64> {
        exact
            .values
            .chunks(10)
            .map(|path| path[2 + factor] - path[factor])
            .collect()
    };
    let var_x = (1.0 - (-2.0 * kappa * dt).exp()) / (2.0 * kappa);
    let cov = 0.3 * (1.0 - (-kappa * dt).exp()) / kappa;
    assert_correlation(
        &increments(0),
        &increments(1),
        cov / (var_x * dt).sqrt(),
        "schwartz-smith increment correlation",
    );
}

// ---------------------------------------------------------------------------
// Input errors
// ---------------------------------------------------------------------------

#[test]
fn initial_state_must_match_the_process_dimension() {
    for (process, x0) in processes() {
        let tag = process.tag();
        let mut short = spec(process.clone(), &x0[1..], 2);
        short.num_paths = 2;
        assert!(error_message(&short).contains("initial_state"), "{tag}");

        let mut long = spec(process.clone(), &[x0.as_slice(), &[1.0]].concat(), 2);
        long.num_paths = 2;
        assert!(error_message(&long).contains("initial_state"), "{tag}");

        let mut nan = spec(process, &x0, 2);
        nan.initial_state[0] = f64::NAN;
        assert!(simulate_paths(&nan).is_err(), "{tag}");
    }
}

#[test]
fn initial_state_must_lie_in_the_process_domain() {
    let rejects = |process: ProcessSpec, x0: &[f64]| {
        let mut s = spec(process, x0, 2);
        s.num_paths = 2;
        for scheme in [SchemeSpec::Default, SchemeSpec::Euler] {
            s.scheme = scheme;
            assert!(simulate_paths(&s).is_err(), "{:?}", s.initial_state);
        }
    };
    rejects(gbm(), &[0.0]);
    rejects(gbm(), &[-1.0]);
    rejects(multi_gbm(), &[100.0, 0.0]);
    rejects(cir(), &[-0.01]);
    rejects(heston(), &[0.0, 0.03]);
    // The starting variance must be the v0 parameter.
    rejects(heston(), &[100.0, 0.04]);
    // Short rate below the initial shift leaves a negative CIR factor.
    rejects(cir_plus_plus(), &[0.005]);

    let mut s = spec(gbm_with_dividends(), &[-1.0], 2);
    s.num_paths = 2;
    assert!(simulate_paths(&s).is_err());
}

#[test]
fn workload_is_bounded_before_allocation() {
    let run = |num_steps: usize, num_paths: usize, antithetic: bool| {
        let mut s = spec(multi_gbm(), &[100.0, 50.0], num_steps);
        s.num_paths = num_paths;
        s.antithetic = antithetic;
        simulate_paths(&s)
    };
    assert!(run(1, 0, false).is_err());
    assert!(run(1, MAX_CAPTURED_PATHS + 1, false).is_err());
    assert!(run(usize::MAX, 1, false).is_err());
    assert!(run(MAX_STORED_VALUES, 1, false).is_err());
    // 100 000 streams x 2 factors x 200 times is 40M values; the antithetic
    // partners double that past the 64M budget.
    assert!(error_message(&{
        let mut s = spec(multi_gbm(), &[100.0, 50.0], 199);
        s.num_paths = MAX_CAPTURED_PATHS;
        s.antithetic = true;
        s
    })
    .contains("64000000"));
    assert!(run(3, 5, true).is_ok());
}

#[test]
fn invalid_time_grids_are_errors() {
    let with_grid = |time_grid| {
        let mut s = spec(gbm(), &[100.0], 2);
        s.num_paths = 2;
        s.time_grid = time_grid;
        simulate_paths(&s)
    };
    let uniform = |expiry, num_steps| TimeGridSpec::Uniform { expiry, num_steps };
    let times = |times: &[f64]| TimeGridSpec::Times {
        times: times.to_vec(),
    };
    assert!(with_grid(uniform(1.0, 0)).is_err());
    assert!(with_grid(uniform(0.0, 4)).is_err());
    assert!(with_grid(uniform(f64::NAN, 4)).is_err());
    assert!(with_grid(times(&[])).is_err());
    assert!(with_grid(times(&[0.0])).is_err());
    assert!(with_grid(times(&[0.5, 1.0])).is_err());
    assert!(with_grid(times(&[0.0, 1.0, 0.5])).is_err());
    assert!(with_grid(times(&[0.0, 0.5, 1.0])).is_ok());
}

/// Parameters are public fields, so a spec built in memory or read from JSON
/// can hold values the constructors reject; `simulate_paths` re-validates.
#[test]
fn out_of_range_parameters_are_errors() {
    let rejects = |process: ProcessSpec, x0: &[f64]| {
        let tag = process.tag();
        let mut s = spec(process, x0, 2);
        s.num_paths = 2;
        assert!(simulate_paths(&s).is_err(), "{tag}");
    };
    let bad_gbm = GbmParams {
        r: 0.0,
        q: 0.0,
        sigma: -0.2,
    };
    let bad_cir = CirParams {
        kappa: -1.0,
        theta: 0.04,
        sigma: 0.1,
    };
    rejects(ProcessSpec::Gbm(bad_gbm.clone()), &[100.0]);
    rejects(
        ProcessSpec::GbmWithDividends {
            params: bad_gbm.clone(),
            dividends: vec![],
        },
        &[100.0],
    );
    for dividend in [
        (0.0, Dividend::Cash(1.0)),
        (0.5, Dividend::Cash(-1.0)),
        (0.5, Dividend::Proportional(f64::NAN)),
    ] {
        rejects(
            ProcessSpec::GbmWithDividends {
                params: GbmParams::new(0.04, 0.0, 0.25).unwrap(),
                dividends: vec![dividend],
            },
            &[100.0],
        );
    }
    rejects(
        ProcessSpec::GbmWithDividends {
            params: GbmParams::new(0.04, 0.0, 0.25).unwrap(),
            dividends: vec![(0.5, Dividend::Cash(1.0)), (0.5, Dividend::Cash(2.0))],
        },
        &[100.0],
    );
    rejects(
        ProcessSpec::MultiGbm {
            assets: vec![bad_gbm.clone(), bad_gbm],
            correlation: None,
        },
        &[100.0, 50.0],
    );
    rejects(
        ProcessSpec::MultiGbm {
            assets: vec![
                GbmParams::new(0.0, 0.0, 0.2).unwrap(),
                GbmParams::new(0.0, 0.0, 0.2).unwrap(),
            ],
            correlation: Some(vec![1.0, 1.5, 1.5, 1.0]),
        },
        &[100.0, 50.0],
    );
    rejects(
        ProcessSpec::Brownian(BrownianParams {
            mu: f64::INFINITY,
            sigma: 0.1,
        }),
        &[0.0],
    );
    rejects(
        ProcessSpec::MultiBrownian {
            mus: vec![0.0, 0.0],
            sigmas: vec![0.1],
            correlation: None,
        },
        &[0.0, 0.0],
    );
    rejects(
        ProcessSpec::MultiOu(MultiOuParams {
            kappas: vec![1.0, 1.0],
            thetas: vec![0.0],
            sigmas: vec![0.1, 0.1],
            correlation: None,
        }),
        &[0.0, 0.0],
    );
    rejects(ProcessSpec::Cir(bad_cir.clone()), &[0.03]);
    rejects(
        ProcessSpec::CirPlusPlus {
            params: bad_cir,
            shift_curve: vec![0.01],
            shift_times: vec![0.0],
        },
        &[0.04],
    );
    rejects(
        ProcessSpec::CirPlusPlus {
            params: CirParams::new(0.5, 0.04, 0.1).unwrap(),
            shift_curve: vec![0.01, 0.02],
            shift_times: vec![0.0],
        },
        &[0.04],
    );
    rejects(
        ProcessSpec::SchwartzSmith(SchwartzSmithParams {
            kappa: 1.5,
            sigma_x: 0.3,
            mu_y: 0.02,
            sigma_y: 0.15,
            rho_xy: 1.5,
            lambda_x: 0.0,
        }),
        &[0.2, 4.0],
    );
}

// ---------------------------------------------------------------------------
// Wire format
// ---------------------------------------------------------------------------

#[test]
fn specs_round_trip_through_json_without_changing_the_paths() {
    for mut s in supported_specs() {
        s.antithetic = true;
        let json = serde_json::to_string(&s).unwrap();
        let restored: PathSimulationSpec = serde_json::from_str(&json).unwrap();
        let out = simulate_paths(&s).unwrap();
        assert_eq!(out, simulate_paths(&restored).unwrap(), "{json}");

        let summary_json = serde_json::to_string(&out).unwrap();
        assert_eq!(
            out,
            serde_json::from_str::<PathSummary>(&summary_json).unwrap()
        );
    }
}

#[test]
fn process_and_scheme_tags_are_stable() {
    let tags: Vec<String> = processes()
        .iter()
        .map(|(process, _)| {
            let value = serde_json::to_value(process).unwrap();
            assert_eq!(value["type"], process.tag());
            process.tag().to_string()
        })
        .collect();
    assert_eq!(
        tags,
        [
            "gbm",
            "gbm_with_dividends",
            "multi_gbm",
            "brownian",
            "multi_brownian",
            "multi_ou",
            "hull_white_1f",
            "cir",
            "cir_plus_plus",
            "heston",
            "schwartz_smith",
            "local_vol",
        ]
    );
    for scheme in SCHEMES {
        assert_eq!(serde_json::to_value(scheme).unwrap(), scheme_tag(scheme));
    }
}

#[test]
fn json_spec_uses_defaults_and_rejects_unknown_fields() {
    let json = |process: &str, extra: &str| {
        format!(
            r#"{{"process": {process}, "initial_state": [100.0],
                "time_grid": {{"type": "uniform", "expiry": 1.0, "num_steps": 4}},
                "num_paths": 3, "seed": 5{extra}}}"#
        )
    };
    let gbm = r#"{"type": "gbm", "r": 0.03, "q": 0.0, "sigma": 0.2}"#;
    let parsed: PathSimulationSpec = serde_json::from_str(&json(gbm, "")).unwrap();
    assert_eq!(parsed.scheme, SchemeSpec::Default);
    assert!(!parsed.antithetic);
    assert_eq!(simulate_paths(&parsed).unwrap().values.len(), 15);

    let parse = |text: String| serde_json::from_str::<PathSimulationSpec>(&text);
    // Unknown key on the spec, on a parameter struct, and on a struct variant.
    assert!(parse(json(gbm, r#", "paths": 3"#)).is_err());
    assert!(parse(json(
        r#"{"type": "gbm", "r": 0.03, "q": 0.0, "sigma": 0.2, "vol": 0.2}"#,
        ""
    ))
    .is_err());
    assert!(parse(json(
        r#"{"type": "multi_brownian", "mus": [0.0], "sigmas": [0.1], "rho": 0.0}"#,
        ""
    ))
    .is_err());
    assert!(parse(json(r#"{"type": "local_vol"}"#, "")).is_err());
    assert!(parse(json(gbm, r#", "scheme": "exact""#)).is_err());

    let dividends = r#"{"type": "gbm_with_dividends",
        "params": {"r": 0.03, "q": 0.0, "sigma": 0.2},
        "dividends": [[0.5, {"cash": 1.0}], [0.75, {"proportional": 0.01}]]}"#;
    let parsed: PathSimulationSpec = serde_json::from_str(&json(dividends, "")).unwrap();
    assert!(simulate_paths(&parsed).is_ok());
}
