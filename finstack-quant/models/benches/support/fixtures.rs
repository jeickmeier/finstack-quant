//! Shared fixtures for Monte Carlo Criterion targets.
//!
//! Built once outside `b.iter` so process / grid / payoff construction is not
//! folded into the measured path cost. Inputs are deterministic: seeded Philox
//! or closed-form designs, never a clock.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(dead_code)]

use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::BarrierType;
use finstack_quant_models::monte_carlo::discretization::lmm_predictor_corrector::LmmPredictorCorrector;
use finstack_quant_models::monte_carlo::discretization::{
    EulerMaruyama, ExactGbm, ExactHullWhite1F, ExactMultiGbmCorrelated, ExactSchwartzSmith, QeCir,
    QeHeston,
};
use finstack_quant_models::monte_carlo::engine::{McEngine, McEngineConfig, PathCaptureConfig};
use finstack_quant_models::monte_carlo::payoff::asian::{
    default_fixing_steps, AsianCall, AveragingMethod,
};
use finstack_quant_models::monte_carlo::payoff::barrier::{
    BarrierMonitoring, BarrierOptionPayoff, OptionKind,
};
use finstack_quant_models::monte_carlo::payoff::lookback::{Lookback, LookbackDirection};
use finstack_quant_models::monte_carlo::payoff::vanilla::EuropeanCall;
use finstack_quant_models::monte_carlo::pricer::path_dependent::{
    PathDependentPricer, PathDependentPricerConfig,
};
use finstack_quant_models::monte_carlo::process::cir::CirProcess;
use finstack_quant_models::monte_carlo::process::gbm::{GbmParams, GbmProcess, MultiGbmProcess};
use finstack_quant_models::monte_carlo::process::heston::HestonProcess;
use finstack_quant_models::monte_carlo::process::lmm::{LmmParams, LmmProcess};
use finstack_quant_models::monte_carlo::process::ou::{HullWhite1FParams, HullWhite1FProcess};
use finstack_quant_models::monte_carlo::process::schwartz_smith::{
    SchwartzSmithParams, SchwartzSmithProcess,
};
use finstack_quant_models::monte_carlo::rng::philox::PhiloxRng;
use finstack_quant_models::monte_carlo::traits::{PathState, Payoff};
use finstack_quant_models::monte_carlo::TimeGrid;

pub const SPOT: f64 = 100.0;
pub const STRIKE: f64 = 100.0;
pub const RATE: f64 = 0.05;
pub const DIV: f64 = 0.02;
pub const VOL: f64 = 0.20;
pub const SEED: u64 = 42;
pub const SS_RHO: f64 = -0.3;

pub fn gbm() -> GbmProcess {
    GbmProcess::with_params(RATE, DIV, VOL).expect("valid GBM")
}

pub fn heston() -> HestonProcess {
    HestonProcess::with_params(0.03, 0.0, 2.0, 0.04, 0.3, -0.7, 0.04).expect("valid Heston")
}

pub fn hw1f() -> HullWhite1FProcess {
    HullWhite1FProcess::new(
        HullWhite1FParams::new(0.1, 0.01, 0.03).expect("valid Hull-White parameters"),
    )
}

pub fn cir() -> CirProcess {
    CirProcess::with_params(2.0, 0.04, 0.3).expect("valid CIR")
}

pub fn schwartz_smith() -> SchwartzSmithProcess {
    let params = SchwartzSmithParams::new(1.5, 0.3, 0.02, 0.15, SS_RHO).expect("valid SS");
    SchwartzSmithProcess::new(params, 0.0, SPOT.ln())
}

pub fn european_call(num_steps: usize) -> EuropeanCall {
    EuropeanCall::new(STRIKE, 1.0, num_steps)
}

pub fn asian_call(num_steps: usize) -> AsianCall {
    AsianCall::new(
        STRIKE,
        1.0,
        AveragingMethod::Arithmetic,
        default_fixing_steps(num_steps),
    )
    .expect("benchmark fixing schedule must be nonempty")
}

pub fn lookback_call(num_steps: usize) -> Lookback {
    Lookback::new(LookbackDirection::Call, STRIKE, 1.0, num_steps)
}

pub fn barrier_up_out(num_steps: usize) -> BarrierOptionPayoff {
    let grid = TimeGrid::uniform(1.0, num_steps).expect("valid grid");
    BarrierOptionPayoff::new(
        STRIKE,
        120.0,
        BarrierType::UpAndOut,
        OptionKind::Call,
        None,
        1.0,
        num_steps,
        VOL,
        &grid,
        BarrierMonitoring::Continuous { start_step: 0 },
    )
}

pub fn serial_engine(num_paths: usize, num_steps: usize) -> McEngine {
    McEngine::new(
        McEngineConfig::uniform(num_paths, 1.0, num_steps)
            .expect("valid engine")
            .parallel(false)
            .antithetic(false),
    )
}

pub fn antithetic_engine(num_paths: usize, num_steps: usize) -> McEngine {
    McEngine::new(
        McEngineConfig::uniform(num_paths, 1.0, num_steps)
            .expect("valid engine")
            .parallel(false)
            .antithetic(true),
    )
}

pub fn capture_engine(num_paths: usize, num_steps: usize, sample: usize) -> McEngine {
    McEngine::new(
        McEngineConfig::uniform(num_paths, 1.0, num_steps)
            .expect("valid engine")
            .parallel(false)
            .antithetic(false)
            .path_capture(PathCaptureConfig::sample(sample, SEED)),
    )
}

pub fn path_dependent_pricer(num_paths: usize) -> PathDependentPricer {
    PathDependentPricer::new(
        PathDependentPricerConfig::new(num_paths)
            .with_seed(SEED)
            .with_parallel(false),
    )
}

pub fn sobol_pricer(num_paths: usize) -> PathDependentPricer {
    PathDependentPricer::new(
        PathDependentPricerConfig::new(num_paths)
            .with_seed(SEED)
            .with_parallel(false)
            .with_sobol(true),
    )
}

pub fn philox() -> PhiloxRng {
    PhiloxRng::new(SEED)
}

pub fn discount() -> f64 {
    (-RATE).exp()
}

/// Equicorrelated multi-asset GBM with exact correlated log steps.
pub fn multi_gbm(num_assets: usize) -> (MultiGbmProcess, ExactMultiGbmCorrelated, Vec<f64>) {
    let params = (0..num_assets)
        .map(|i| {
            let vol = VOL + 0.01 * i as f64;
            GbmParams::new(RATE, DIV, vol).expect("valid GBM params")
        })
        .collect();
    let mut corr = vec![0.0; num_assets * num_assets];
    for i in 0..num_assets {
        for j in 0..num_assets {
            corr[i * num_assets + j] = if i == j { 1.0 } else { 0.30 };
        }
    }
    let process = MultiGbmProcess::new(params, Some(corr.clone())).expect("valid multi-GBM");
    let disc = ExactMultiGbmCorrelated::new(&corr, num_assets).expect("valid correlation");
    let spots = vec![SPOT; num_assets];
    (process, disc, spots)
}

/// Annual LMM book: `n` forwards, 2 factors, flat 3% with a 50 bp displacement.
pub fn lmm_process(num_forwards: usize) -> LmmProcess {
    let tenors: Vec<f64> = (0..=num_forwards).map(|i| i as f64).collect();
    let accruals = vec![1.0; num_forwards];
    let displacements = vec![0.005; num_forwards];
    let forwards = vec![0.03; num_forwards];
    let loadings: Vec<[f64; 3]> = (0..num_forwards)
        .map(|i| {
            let w = 0.15 - 0.002 * i as f64;
            [w, 0.05, 0.0]
        })
        .collect();
    let params = LmmParams {
        num_forwards,
        num_factors: 2,
        tenors,
        accrual_factors: accruals,
        displacements,
        vol_times: vec![],
        vol_values: vec![loadings],
        initial_forwards: forwards,
    }
    .validate()
    .expect("valid LMM");
    LmmProcess::new(params)
}

/// Caplet-style payoff on the first LMM forward (`spot_0`).
#[derive(Clone)]
pub struct IndexedSpotCall {
    strike: f64,
    maturity_step: usize,
    last: f64,
}

impl IndexedSpotCall {
    pub fn new(strike: f64, maturity_step: usize) -> Self {
        Self {
            strike,
            maturity_step,
            last: 0.0,
        }
    }
}

impl Payoff for IndexedSpotCall {
    fn on_event(&mut self, state: &mut PathState) -> finstack_quant_core::Result<()> {
        if state.step == self.maturity_step {
            self.last = state.get("spot_0").unwrap_or(0.0);
        }
        Ok(())
    }

    fn value(&self, currency: Currency) -> finstack_quant_core::Result<Money> {
        Ok(Money::new((self.last - self.strike).max(0.0), currency).expect("valid money fixture"))
    }

    fn reset(&mut self) {
        self.last = 0.0;
    }

    fn max_event_step(&self) -> Option<usize> {
        Some(self.maturity_step)
    }
}

pub fn exact_gbm() -> ExactGbm {
    ExactGbm::new()
}

pub fn qe_heston() -> QeHeston {
    QeHeston::new()
}

pub fn qe_cir() -> QeCir {
    QeCir::new()
}

pub fn exact_hw1f() -> ExactHullWhite1F {
    ExactHullWhite1F::new()
}

pub fn euler() -> EulerMaruyama {
    EulerMaruyama::new()
}

pub fn exact_ss(rho: f64) -> ExactSchwartzSmith {
    ExactSchwartzSmith::new(rho).expect("valid SS scheme")
}

pub fn lmm_scheme() -> LmmPredictorCorrector {
    LmmPredictorCorrector::new()
}
