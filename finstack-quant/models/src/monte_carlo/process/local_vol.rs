//! Dupire local-volatility process for equity and FX simulation.
//!
//! # Stochastic Differential Equation
//!
//! Under the risk-neutral measure:
//!
//! ```text
//! dS_t = (r − q) S_t dt + σ_loc(t, S_t) S_t dW_t
//! ```
//!
//! where `σ_loc` is a [`LocalVolSurface`] read at calendar time `t` and spot
//! level `S_t`. With the surface extracted from an implied volatility surface
//! by [`LocalVolSurface::from_implied_vol`], the process reprices every
//! European option on that surface (Dupire 1994), provided the forwards given
//! to the extractor are `S_0·exp((r − q)·T)` for this process's `r` and `q`.
//!
//! # Discretization
//!
//! There is no exact transition. The canonical scheme is
//! [`LogEuler`](crate::monte_carlo::discretization::euler::LogEuler), which
//! freezes the volatility at the start of each step and keeps the spot
//! positive:
//!
//! ```text
//! S_{t+Δt} = S_t · exp[(r − q − ½σ²)Δt + σ√Δt · Z],   σ = σ_loc(t, S_t)
//! ```
//!
//! Its weak error is first order in `Δt`.
//! [`EulerMaruyama`](crate::monte_carlo::discretization::euler::EulerMaruyama)
//! on the spot is also valid but can cross zero. The diffusion is not
//! proportional to the spot, so the Milstein scheme of this crate, which
//! assumes `∂σ/∂S = σ/S`, does not apply.
//!
//! # References
//!
//! - Dupire, B. (1994). "Pricing with a Smile." *Risk*, 7(1), 18-20.
//!   `docs/REFERENCES.md#dupire-1994`
//! - Gatheral, J. (2006). *The Volatility Surface: A Practitioner's Guide*.
//!   Wiley. Chapter 1. `docs/REFERENCES.md#gatheral-volatility-surface`
//! - Glasserman, P. (2003). *Monte Carlo Methods in Financial Engineering*.
//!   Springer. Section 6.1 (Euler scheme).
//!   `docs/REFERENCES.md#glasserman-2004-monte-carlo`

use super::super::paths::ProcessParams;
use super::super::traits::StochasticProcess;
use super::metadata::ProcessMetadata;
use crate::volatility::local_vol::LocalVolSurface;
use std::sync::Arc;

/// Local-volatility process parameters.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LocalVolParams {
    /// Risk-free rate, continuously compounded (annualized decimal).
    pub r: f64,
    /// Dividend yield or foreign rate, continuously compounded (annualized
    /// decimal).
    pub q: f64,
    /// Local volatility `σ_loc(t, S)`: expiry axis in years from the
    /// simulation start, strike axis in spot price units. Flat outside its
    /// grid.
    pub surface: LocalVolSurface,
}

impl LocalVolParams {
    /// Create local-volatility parameters with validation.
    ///
    /// # Arguments
    ///
    /// * `r` - Continuously compounded risk-free rate as an annualized
    ///   decimal.
    /// * `q` - Continuously compounded dividend yield (or foreign rate) as an
    ///   annualized decimal.
    /// * `surface` - Local volatility `σ_loc(t, S)` with time in years from
    ///   the simulation start and the spot level in price units; flat outside
    ///   its grid. To reprice an implied surface it must be extracted with
    ///   forwards `S_0·exp((r − q)·T)`.
    ///
    /// # Errors
    ///
    /// Returns a validation error if `r` or `q` is non-finite.
    pub fn new(r: f64, q: f64, surface: LocalVolSurface) -> finstack_quant_core::Result<Self> {
        for (name, value) in [("r (risk-free rate)", r), ("q (dividend yield)", q)] {
            if !value.is_finite() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "local-volatility {name} must be finite, got {value}"
                )));
            }
        }
        Ok(Self { r, q, surface })
    }
}

/// Single-factor local-volatility process.
///
/// State: `[spot]`. Factor: one Brownian motion.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_models::monte_carlo::process::local_vol::{LocalVolParams, LocalVolProcess};
/// use finstack_quant_models::monte_carlo::traits::StochasticProcess;
/// use finstack_quant_models::volatility::local_vol::LocalVolSurface;
///
/// // 30% volatility below 100, 20% above, constant in time.
/// let surface = LocalVolSurface::new(vec![1.0], vec![99.0, 101.0], vec![0.30, 0.20])?;
/// let process = LocalVolProcess::new(LocalVolParams::new(0.03, 0.01, surface)?);
///
/// let (mut drift, mut diffusion) = ([0.0], [0.0]);
/// process.drift(0.5, &[90.0], &mut drift);
/// process.diffusion(0.5, &[90.0], &mut diffusion);
/// assert!((drift[0] - 0.02 * 90.0).abs() < 1e-12);
/// assert!((diffusion[0] - 0.30 * 90.0).abs() < 1e-12);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
#[derive(Debug, Clone)]
pub struct LocalVolProcess {
    r: f64,
    q: f64,
    surface: Arc<LocalVolSurface>,
}

impl LocalVolProcess {
    /// Create a local-volatility process.
    ///
    /// # Arguments
    ///
    /// * `params` - Rates and the local volatility surface; see
    ///   [`LocalVolParams::new`] for units and conventions.
    pub fn new(params: LocalVolParams) -> Self {
        Self {
            r: params.r,
            q: params.q,
            surface: Arc::new(params.surface),
        }
    }

    /// Risk-neutral drift rate `r − q`.
    pub fn drift_rate(&self) -> f64 {
        self.r - self.q
    }

    /// The local volatility surface `σ_loc(t, S)`.
    pub fn surface(&self) -> &LocalVolSurface {
        &self.surface
    }
}

impl StochasticProcess for LocalVolProcess {
    fn dim(&self) -> usize {
        1
    }

    fn num_factors(&self) -> usize {
        1
    }

    fn drift(&self, _t: f64, x: &[f64], out: &mut [f64]) {
        // μ(S) = (r - q) S
        out[0] = self.drift_rate() * x[0];
    }

    fn diffusion(&self, t: f64, x: &[f64], out: &mut [f64]) {
        // σ(t, S) = σ_loc(t, S) S
        out[0] = self.surface.value(t, x[0]) * x[0];
    }
}

impl ProcessMetadata for LocalVolProcess {
    fn metadata(&self) -> ProcessParams {
        let mut params = ProcessParams::new("LocalVol");
        params.add_param("r", self.r);
        params.add_param("q", self.q);
        params.with_factors(vec!["spot".to_string()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monte_carlo::simulate::{
        simulate_paths, PathSimulationSpec, PathSummary, ProcessSpec, SchemeSpec, TimeGridSpec,
    };
    use crate::monte_carlo::OnlineStats;
    use crate::volatility::implied_vol_black;
    use crate::volatility::local_vol::test_support::{forwards, linspace, SKEWED};

    const SPOT: f64 = 100.0;
    const RATE: f64 = 0.03;
    const DIV_YIELD: f64 = 0.01;

    fn simulate(
        surface: LocalVolSurface,
        scheme: SchemeSpec,
        expiry: f64,
        num_steps: usize,
        num_paths: usize,
        seed: u64,
    ) -> PathSummary {
        simulate_paths(&PathSimulationSpec {
            process: ProcessSpec::LocalVol(
                LocalVolParams::new(RATE, DIV_YIELD, surface).expect("valid parameters"),
            ),
            scheme,
            initial_state: vec![SPOT],
            time_grid: TimeGridSpec::Uniform { expiry, num_steps },
            num_paths,
            seed,
            antithetic: true,
            fbm: None,
        })
        .expect("local-volatility simulation")
    }

    /// Mean and standard error of an undiscounted option payoff at grid index
    /// `step`, with each antithetic pair counted as one sample.
    fn forward_price(paths: &PathSummary, step: usize, strike: f64, is_call: bool) -> (f64, f64) {
        let stride = paths.times.len();
        let payoff = |path: usize| {
            let spot = paths.values[path * stride + step];
            if is_call {
                (spot - strike).max(0.0)
            } else {
                (strike - spot).max(0.0)
            }
        };
        let mut stats = OnlineStats::new();
        for stream in 0..paths.num_paths {
            stats.update(0.5 * (payoff(2 * stream) + payoff(2 * stream + 1)));
        }
        (stats.mean(), stats.stderr())
    }

    #[test]
    fn params_reject_non_finite_rates_and_unknown_fields() {
        let surface = || LocalVolSurface::new(vec![1.0], vec![100.0], vec![0.2]).expect("grid");
        assert!(LocalVolParams::new(f64::NAN, 0.0, surface()).is_err());
        assert!(LocalVolParams::new(0.0, f64::INFINITY, surface()).is_err());
        let params = LocalVolParams::new(0.03, 0.01, surface()).expect("valid");
        let json = serde_json::to_string(&params).expect("serialize");
        let restored: LocalVolParams = serde_json::from_str(&json).expect("round trip");
        assert_eq!(restored.surface, params.surface);
        assert!(serde_json::from_str::<LocalVolParams>(&json.replace("\"q\"", "\"qq\"")).is_err());
        let metadata = LocalVolProcess::new(params).metadata();
        assert_eq!(metadata.factor_names, ["spot"]);
    }

    /// A flat local volatility is geometric Brownian motion, for which the
    /// log-Euler step is the exact transition: the Monte Carlo European price
    /// is unbiased and must sit within sampling error of Black-Scholes.
    #[test]
    fn flat_surface_reprices_black_scholes() {
        let sigma = 0.2;
        let flat = LocalVolSurface::new(vec![0.5, 1.0], vec![50.0, 150.0], vec![sigma; 4])
            .expect("flat grid");
        let expiry = 1.0;
        let paths = simulate(flat, SchemeSpec::Default, expiry, 20, 50_000, 11);
        assert_eq!(paths.factor_names, ["spot"]);
        assert!(paths.values.iter().all(|spot| *spot > 0.0));

        let forward = SPOT * ((RATE - DIV_YIELD) * expiry).exp();
        for strike in [80.0, 100.0, 120.0] {
            let (price, stderr) = forward_price(&paths, 20, strike, true);
            let exact = crate::closed_form::black_call(forward, strike, sigma, expiry);
            assert!(
                (price - exact).abs() < 4.0 * stderr,
                "K={strike}: MC {price} +/- {stderr}, Black {exact}"
            );
        }
    }

    /// The defining Dupire property: simulate the local volatility extracted
    /// from an arbitrage-free skewed implied surface and recover that
    /// surface's implied volatilities from Monte Carlo European prices.
    ///
    /// Implied grid: 80 expiries from 0.02 to 1.6 years by 361 strikes from
    /// 40 to 220. Simulation: 100 log-Euler steps per year, 100,000 antithetic
    /// pairs. Out-of-the-money options keep the sampling error low.
    ///
    /// Measured at this seed, over strikes 80 to 120 and expiries 0.5, 1 and
    /// 1.5 years: errors between -12.5 and +3.9 basis points of implied
    /// volatility against a sampling error of 4 to 7 basis points, the
    /// largest at the 80 strike of the shortest expiry. The 25 basis point
    /// tolerance is about four sampling errors.
    #[test]
    fn skewed_surface_reprices_its_implied_vols() {
        let expiries = linspace(0.02, 1.6, 80);
        let strikes = linspace(40.0, 220.0, 361);
        let carry = RATE - DIV_YIELD;
        let implied = SKEWED.surface(&expiries, &strikes, &forwards(SPOT, carry, &expiries));
        let local = LocalVolSurface::from_implied_vol(&implied, &forwards(SPOT, carry, &expiries))
            .expect("arbitrage-free surface extracts");
        let paths = simulate(local, SchemeSpec::Default, 1.5, 150, 100_000, 2024);

        let mut worst_bp: f64 = 0.0;
        for (step, expiry) in [(50, 0.5), (100, 1.0), (150, 1.5)] {
            let forward = SPOT * (carry * expiry).exp();
            for strike in [80.0, 90.0, 100.0, 110.0, 120.0] {
                let is_call = strike >= forward;
                let (price, stderr) = forward_price(&paths, step, strike, is_call);
                let vol = implied_vol_black(price, forward, strike, expiry, is_call)
                    .expect("Monte Carlo price inverts");
                let target = SKEWED.implied_vol((strike / forward).ln(), expiry);
                // Sampling error of the implied vol: price error over vega.
                let bumped = implied_vol_black(price + stderr, forward, strike, expiry, is_call)
                    .expect("bumped price inverts");
                let error_bp = (vol - target) * 1e4;
                let stderr_bp = (bumped - vol) * 1e4;
                println!(
                    "T={expiry} K={strike}: MC {vol:.5} target {target:.5} \
                     error {error_bp:+.1}bp (stderr {stderr_bp:.1}bp)"
                );
                worst_bp = worst_bp.max(error_bp.abs());
                assert!(
                    error_bp.abs() < 25.0,
                    "T={expiry} K={strike}: MC implied vol {vol} vs surface {target} \
                     ({error_bp:+.1}bp, stderr {stderr_bp:.1}bp)"
                );
            }
        }
        println!("worst repricing error {worst_bp:.1}bp");
    }

    #[test]
    fn euler_scheme_is_accepted_and_others_are_rejected() {
        let surface =
            || LocalVolSurface::new(vec![0.5, 1.0], vec![50.0, 150.0], vec![0.2; 4]).expect("grid");
        let log_euler = simulate(surface(), SchemeSpec::LogEuler, 1.0, 8, 4, 3);
        let default = simulate(surface(), SchemeSpec::Default, 1.0, 8, 4, 3);
        assert_eq!(log_euler, default);
        let euler = simulate(surface(), SchemeSpec::Euler, 1.0, 8, 4, 3);
        assert_ne!(euler.values, default.values);

        let spec = |scheme: SchemeSpec, spot: f64| PathSimulationSpec {
            process: ProcessSpec::LocalVol(
                LocalVolParams::new(RATE, DIV_YIELD, surface()).expect("valid"),
            ),
            scheme,
            initial_state: vec![spot],
            time_grid: TimeGridSpec::Uniform {
                expiry: 1.0,
                num_steps: 8,
            },
            num_paths: 4,
            seed: 3,
            antithetic: false,
            fbm: None,
        };
        let error = simulate_paths(&spec(SchemeSpec::Milstein, SPOT))
            .expect_err("Milstein assumes proportional diffusion")
            .to_string();
        assert!(
            error.contains("scheme 'milstein' is not available for process 'local_vol'"),
            "{error}"
        );
        assert!(simulate_paths(&spec(SchemeSpec::Default, 0.0)).is_err());
        let mut bad_rate = spec(SchemeSpec::Default, SPOT);
        if let ProcessSpec::LocalVol(params) = &mut bad_rate.process {
            params.r = f64::NAN;
        }
        assert!(simulate_paths(&bad_rate).is_err());
    }
}
