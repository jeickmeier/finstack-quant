//! WASM bindings for the Monte Carlo engine in `finstack-quant-models`.
//!
//! Mirrors `finstack-quant-py/src/bindings/models/monte_carlo/`: Heston and
//! GBM convenience pricers, path simulation for the built-in processes (Markov
//! and path-dependent) and finite-difference Greeks. Estimates cross the boundary as the canonical
//! Rust `MoneyEstimate` / `Estimate` serde objects, and simulated paths as the
//! `PathSummary` object. Closed-form Black-Scholes references live in
//! `models.bsPrice`; `simulatePaths` selects a process and discretization by
//! spec, while the process, discretization and RNG types remain Rust-only.

use crate::utils::input::{
    from_js_json, js_bool, js_f64, js_opt_bool, js_opt_currency, js_opt_string, js_opt_u64,
    js_opt_uint, js_u64, opt_from_js_json,
};

use crate::utils::{to_js_err, to_js_value};
use finstack_quant_models::closed_form::HestonPricingParams;
use finstack_quant_models::monte_carlo::convenience;
use finstack_quant_models::monte_carlo::greeks::gbm_european::{self, GbmEuropeanFdSpec};
use finstack_quant_models::monte_carlo::pricer::european::EuropeanPricer;
use finstack_quant_models::monte_carlo::pricer::lsmc::LsmcPricer;
use finstack_quant_models::monte_carlo::pricer::path_dependent::PathDependentPricer;
use finstack_quant_models::monte_carlo::results::MoneyEstimate;
use finstack_quant_models::monte_carlo::simulate::PathSimulationSpec;
use finstack_quant_models::types::{OptionMarketParams, OptionType};
use wasm_bindgen::prelude::*;

/// Price a European option under Heston stochastic volatility by Monte Carlo.
///
/// Twin of the Rust `price_heston` and the Fourier `models.hestonPrice`: same
/// market and model inputs, with Monte Carlo settings in place of the Fourier
/// settings. Paths use the Andersen (2008) QE discretization.
///
/// # Errors
///
/// Throws a `validation` error if `params` does not match
/// `HestonPricingParams` (non-finite `r`/`q`, non-positive or non-finite
/// `kappa`, `theta`, `sigma_v` or `v0`, `rho` outside `[-1, 1]`) or
/// `settings` does not match `HestonMcSettings` (unknown currency, a
/// fractional or out-of-range count), if the embedded defaults cannot be
/// loaded, the expiry, step count, path count or computed discount factor
/// fails validation, a simulated discounted payoff is non-finite, or the
/// result cannot be serialized.
/// @returns The Rust `MoneyEstimate` serde object: `mean` and `ci_95` as
/// `{amount, currency}` money, plus `stderr`, `num_paths`,
/// `num_simulated_paths` and the optional statistics (`null` when not captured).
/// @param spot - Current spot price or exchange rate in the same units as the strike.
/// @param strike - Option strike price in the same price units as the underlying.
/// @param expiry - Time to option expiry in years on the model's annual time basis.
/// @param params - `HestonPricingParams` object or JSON text, every key required: `r` (continuously compounded risk-free rate, decimal), `q` (continuous dividend yield, decimal), `v0` (initial instantaneous variance), `kappa` (variance mean-reversion speed per year), `theta` (long-run variance), `sigma_v` (annualized volatility of variance) and `rho` (asset/variance shock correlation in `[-1, 1]`).
/// @param isCall - Whether to value a call (`true`) or put (`false`).
/// @param settings - Optional `HestonMcSettings` object or JSON text; every key may be omitted and an omitted key uses the Rust registry European-pricer default: `num_paths` (independent path estimators in `[2, 10000000]`, each antithetic pair counts once; default 100 000), `seed` (Philox root seed, number or BigInt), `num_steps` (positive time steps per path) and `currency` (ISO-4217 code stamped on the estimate).
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const estimate = models.monteCarlo.priceHeston(
///   100, 100, 1.0,
///   { r: 0.03, q: 0.0, v0: 0.04, kappa: 2.0, theta: 0.04, sigma_v: 0.3, rho: -0.7 },
///   true,
///   { num_paths: 2000, seed: 42, num_steps: 16 },
/// );
/// console.log(estimate.mean.amount, estimate.stderr);
/// ```
#[wasm_bindgen(js_name = priceHeston, unchecked_return_type = "MoneyEstimate")]
pub fn price_heston(
    #[wasm_bindgen(unchecked_param_type = "number")] spot: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] expiry: JsValue,
    #[wasm_bindgen(unchecked_param_type = "generated.models.HestonPricingParams | string")]
    params: JsValue,
    #[wasm_bindgen(js_name = "isCall", unchecked_param_type = "boolean")] is_call: JsValue,
    #[wasm_bindgen(
        unchecked_optional_param_type = "generated.models.HestonMcSettings | string | null"
    )]
    settings: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    use finstack_quant_models::monte_carlo::pricer::heston as canonical;

    let spot = js_f64(&spot, "spot")?;
    let strike = js_f64(&strike, "strike")?;
    let expiry = js_f64(&expiry, "expiry")?;
    let params: HestonPricingParams = from_js_json(&params, "params")?;
    let option_type = OptionType::from(js_bool(&is_call, "isCall")?);
    // The canonical entry point owns the registry defaults for path count,
    // seed, step count, currency, and (wasm-gated) parallelism; the binding
    // only marshals explicitly supplied values.
    let settings: canonical::HestonMcSettings =
        opt_from_js_json(settings.as_ref(), "settings")?.unwrap_or_default();
    let est = canonical::price_heston(spot, strike, expiry, &params, option_type, &settings)
        .map_err(to_js_err)?;
    crate::utils::to_js_value(&est)
}

/// Whether Heston parameters satisfy the Feller condition `2 * kappa * theta >= volOfVol^2`.
/// @param kappa - Mean-reversion speed of the variance, per year.
/// @param theta - Long-run variance level (annualized, as a decimal).
/// @param volOfVol - Volatility of variance (annualized, as a decimal).
/// @returns `true` when the variance process stays strictly positive.
///
/// # Errors
///
/// Throws a `TypeError` if an argument is not a number.
#[wasm_bindgen(js_name = hestonSatisfiesFeller)]
pub fn heston_satisfies_feller(
    #[wasm_bindgen(unchecked_param_type = "number")] kappa: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] theta: JsValue,
    #[wasm_bindgen(js_name = "volOfVol", unchecked_param_type = "number")] vol_of_vol: JsValue,
) -> Result<bool, JsValue> {
    Ok(
        finstack_quant_models::monte_carlo::process::heston::heston_satisfies_feller(
            js_f64(&kappa, "kappa")?,
            js_f64(&theta, "theta")?,
            js_f64(&vol_of_vol, "volOfVol")?,
        ),
    )
}

/// Simulate paths of any built-in process on a shared time grid.
///
/// Twin of the Rust and Python `simulate_paths`: the spec selects the process,
/// the discretization scheme, the time grid and the random streams, and Rust
/// validates every field.
/// @param spec - `PathSimulationSpec` object or JSON: `process` (tagged by `type`, with annualized decimal rates and volatilities), optional `scheme` (`"default"` uses the process's canonical scheme), `initial_state` in the process's state layout, `time_grid` in years, `num_paths` in [1, 100000], `seed`, optional `antithetic` (default `false`), and optional `fbm`, the fractional-noise generator of the `rough_bergomi` and `cheyette_rough` processes (`{ type: "volterra" }` when omitted).
/// @returns The `PathSummary` object: `num_paths`, `num_simulated_paths`, `dim`, `times`, `factor_names`, and `values` in row-major `[path][time][factor]` order, so factor `f` on path `p` at `times[s]` is `values[(p * times.length + s) * dim + f]`.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const paths = models.monteCarlo.simulatePaths({
///   process: { type: "gbm", r: 0.05, q: 0.0, sigma: 0.2 },
///   initial_state: [100],
///   time_grid: { type: "uniform", expiry: 1.0, num_steps: 2 },
///   num_paths: 3,
///   seed: 7,
/// });
/// console.log(paths.times); // [0, 0.5, 1]
/// console.log(paths.factor_names); // ["spot"]
/// console.log(paths.values.length); // 9 = 3 paths x 3 times x 1 factor
/// ```
///
/// # Errors
///
/// Throws a `TypeError` if `spec` is neither an object nor JSON text, and a
/// `validation` error if it does not match `PathSimulationSpec`, a process
/// parameter is out of range, the scheme is not available for the process,
/// `fbm` is set for a process that does not consume fractional noise,
/// `initial_state` has the wrong length or lies outside the process's domain,
/// the time grid is invalid, `num_paths` is outside [1, 100000], the output
/// would exceed 64 million stored values, or a simulated state is non-finite.
#[wasm_bindgen(js_name = simulatePaths, unchecked_return_type = "generated.models.PathSummary")]
pub fn simulate_paths(
    #[wasm_bindgen(unchecked_param_type = "generated.models.PathSimulationSpec | string")]
    spec: JsValue,
) -> Result<JsValue, JsValue> {
    let spec: PathSimulationSpec = from_js_json(&spec, "spec")?;
    let summary =
        finstack_quant_models::monte_carlo::simulate::simulate_paths(&spec).map_err(to_js_err)?;
    to_js_value(&summary)
}

/// Relative standard error of a Monte Carlo money estimate, `stderr / |mean|`.
///
/// Twin of the Rust and Python `MoneyEstimate.relative_stderr`.
/// @param estimate - `MoneyEstimate` object or JSON, as returned by the pricing functions of this namespace.
/// @returns The relative standard error; `Infinity` when the mean is numerically zero.
///
/// # Errors
///
/// Throws a `validation` error if `estimate` is malformed.
#[wasm_bindgen(js_name = relativeStderr)]
pub fn relative_stderr(
    #[wasm_bindgen(unchecked_param_type = "MoneyEstimate | string")] estimate: JsValue,
) -> Result<f64, JsValue> {
    let estimate: MoneyEstimate = from_js_json(&estimate, "estimate")?;
    Ok(estimate.relative_stderr())
}

/// Monte Carlo finite-difference delta of a GBM European option under common random numbers.
///
/// The up and down valuations share each path's random draws; the estimate
/// is the mean of the per-path central differences and `stderr` is their
/// paired (common-random-number) standard error.
/// @param spec - `GbmEuropeanFdSpec` object or JSON text: `spot` (spot level at time 0), `strike` (same units as `spot`), `rate` (continuously compounded risk-free rate, decimal), `div_yield` (continuous dividend yield, decimal), `vol` (annualized GBM volatility, decimal; positive), `expiry` (years) and `option_type` (`"call"` or `"put"`) are required; `num_paths` (paths per evaluation), `seed` (RNG seed, number or BigInt), `num_steps` (time-grid steps), `bump_size` (relative spot shock, 0.01 is 1% of spot) and `currency` (ISO-4217 code of the simulated payoffs) may be omitted or `null`, which uses the registry default.
/// @returns The `Estimate` object for delta (`mean`, paired `stderr`, `ci_lower`, `ci_upper`, `num_paths`).
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const delta = models.monteCarlo.finiteDiffDelta({
///   spot: 100, strike: 100, rate: 0.05, div_yield: 0.0, vol: 0.2, expiry: 1.0,
///   option_type: "call", num_paths: 2000, seed: 42, num_steps: 16,
/// });
/// console.log(delta.mean, delta.stderr);
/// ```
///
/// # Errors
///
/// Throws a `validation` error if `spec` does not match `GbmEuropeanFdSpec`,
/// an input is non-finite or out of range or the currency code is unknown.
#[wasm_bindgen(js_name = finiteDiffDelta, unchecked_return_type = "generated.models.Estimate")]
pub fn finite_diff_delta(
    #[wasm_bindgen(unchecked_param_type = "generated.models.GbmEuropeanFdSpec | string")]
    spec: JsValue,
) -> Result<JsValue, JsValue> {
    let spec: GbmEuropeanFdSpec = from_js_json(&spec, "spec")?;
    to_js_value(&gbm_european::finite_diff_delta(spec).map_err(to_js_err)?)
}

/// Monte Carlo finite-difference gamma of a GBM European option under common random numbers.
///
/// The three stencil valuations share each path's random draws; the estimate
/// is the mean of the per-path second differences and `stderr` is their
/// paired (common-random-number) standard error.
/// @param spec - `GbmEuropeanFdSpec` object or JSON text: `spot` (spot level at time 0), `strike` (same units as `spot`), `rate` (continuously compounded risk-free rate, decimal), `div_yield` (continuous dividend yield, decimal), `vol` (annualized GBM volatility, decimal; positive), `expiry` (years) and `option_type` (`"call"` or `"put"`) are required; `num_paths` (paths per evaluation), `seed` (RNG seed, number or BigInt), `num_steps` (time-grid steps), `bump_size` (relative spot shock, 0.01 is 1% of spot) and `currency` (ISO-4217 code of the simulated payoffs) may be omitted or `null`, which uses the registry default.
/// @returns The `Estimate` object for gamma (`mean`, paired `stderr`, `ci_lower`, `ci_upper`, `num_paths`).
///
/// # Errors
///
/// Throws a `validation` error if `spec` does not match `GbmEuropeanFdSpec`,
/// an input is non-finite or out of range or the currency code is unknown.
#[wasm_bindgen(js_name = finiteDiffGamma, unchecked_return_type = "generated.models.Estimate")]
pub fn finite_diff_gamma(
    #[wasm_bindgen(unchecked_param_type = "generated.models.GbmEuropeanFdSpec | string")]
    spec: JsValue,
) -> Result<JsValue, JsValue> {
    let spec: GbmEuropeanFdSpec = from_js_json(&spec, "spec")?;
    to_js_value(&gbm_european::finite_diff_gamma(spec).map_err(to_js_err)?)
}

/// Monte Carlo pricer for European options under geometric Brownian motion.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const pricer = new models.monteCarlo.EuropeanPricer(10000, 42);
/// const call = pricer.priceGbm({
///   spot: 100, strike: 100, rate: 0.05, dividend_yield: 0.0,
///   volatility: 0.2, time_to_expiry: 1.0, option_type: "call",
/// });
/// console.log(call.mean.amount, call.stderr);
/// pricer.free();
/// ```
#[wasm_bindgen(js_name = EuropeanPricer)]
pub struct JsEuropeanPricer {
    inner: EuropeanPricer,
}

#[wasm_bindgen(js_class = EuropeanPricer)]
impl JsEuropeanPricer {
    /// Pricer whose omitted settings come from the Rust Monte Carlo registry.
    /// @param numPaths - Optional number of paths; a positive safe integer. Omitted uses the registry default.
    /// @param seed - Optional seed of the path-indexed random streams, as a safe integer or `bigint`. Omitted uses the registry default.
    /// @param useParallel - Optional; run paths on the thread pool. WebAssembly always runs sequentially with identical results. Omitted uses the registry default.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `numPaths` is zero.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(js_name = "numPaths", unchecked_optional_param_type = "number")]
        num_paths: Option<JsValue>,
        #[wasm_bindgen(unchecked_optional_param_type = "number | bigint")] seed: Option<JsValue>,
        #[wasm_bindgen(js_name = "useParallel", unchecked_optional_param_type = "boolean")]
        use_parallel: Option<JsValue>,
    ) -> Result<JsEuropeanPricer, JsValue> {
        convenience::european_pricer(
            js_opt_uint(num_paths.as_ref(), "numPaths")?,
            js_opt_u64(seed.as_ref(), "seed")?,
            js_opt_bool(use_parallel.as_ref(), "useParallel")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Number of Monte Carlo paths.
    #[wasm_bindgen(getter, js_name = numPaths)]
    pub fn num_paths(&self) -> usize {
        self.inner.num_paths()
    }

    /// Seed of the path-indexed random streams.
    #[wasm_bindgen(getter)]
    pub fn seed(&self) -> u64 {
        self.inner.seed()
    }

    /// Whether paths run on the thread pool on native targets.
    #[wasm_bindgen(getter, js_name = useParallel)]
    pub fn use_parallel(&self) -> bool {
        self.inner.use_parallel()
    }

    /// Price a European option under GBM.
    /// @param market - `OptionMarketParams` object or JSON text, every key required: `spot` (spot level at time 0), `strike` (same units as `spot`), `rate` (continuously compounded risk-free rate, decimal), `dividend_yield` (continuous dividend yield, decimal), `volatility` (annualized GBM volatility, decimal), `time_to_expiry` (years) and `option_type` (`"call"` or `"put"`).
    /// @param numSteps - Optional time-grid steps; omitted uses the registry default.
    /// @param currency - Optional ISO-4217 code stamped on the estimate; omitted uses the registry default.
    /// @returns The `MoneyEstimate` object (`mean`, `stderr`, `ci_lower`, `ci_upper`, `num_paths`).
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if an input is non-finite or out of range or
    /// the currency code is unknown.
    #[wasm_bindgen(js_name = priceGbm, unchecked_return_type = "MoneyEstimate")]
    pub fn price_gbm(
        &self,
        #[wasm_bindgen(unchecked_param_type = "generated.models.OptionMarketParams | string")]
        market: JsValue,
        #[wasm_bindgen(js_name = "numSteps", unchecked_optional_param_type = "number")]
        num_steps: Option<JsValue>,
        #[wasm_bindgen(unchecked_optional_param_type = "string")] currency: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let market: OptionMarketParams = from_js_json(&market, "market")?;
        let num_steps = js_opt_uint(num_steps.as_ref(), "numSteps")?;
        let currency = js_opt_currency(currency.as_ref(), "currency")?;
        let estimate = self
            .inner
            .price_gbm(&market, num_steps, currency)
            .map_err(to_js_err)?;
        to_js_value(&estimate)
    }
}

/// Monte Carlo pricer for arithmetic-average Asian options under GBM.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const pricer = new models.monteCarlo.PathDependentPricer(10000, 42);
/// const call = pricer.priceAsian(
///   { spot: 100, strike: 100, rate: 0.05, dividend_yield: 0.0, volatility: 0.2, time_to_expiry: 1.0, option_type: "call" },
///   12,
/// );
/// console.log(call.mean.amount, pricer.antithetic);
/// pricer.free();
/// ```
#[wasm_bindgen(js_name = PathDependentPricer)]
pub struct JsPathDependentPricer {
    inner: PathDependentPricer,
}

#[wasm_bindgen(js_class = PathDependentPricer)]
impl JsPathDependentPricer {
    /// Pricer whose omitted settings come from the Rust Monte Carlo registry.
    /// @param numPaths - Optional number of paths; a positive safe integer. Omitted uses the registry default.
    /// @param seed - Optional seed of the random streams, as a safe integer or `bigint`. Omitted uses the registry default.
    /// @param useParallel - Optional; run paths on the thread pool. WebAssembly always runs sequentially with identical results. Omitted uses the registry default.
    /// @param antithetic - Optional; use antithetic variates. Omitted keeps the Rust configuration default.
    /// @param useSobol - Optional; use Sobol quasi-random numbers. Omitted keeps the Rust configuration default.
    /// @param useBrownianBridge - Optional; build paths by Brownian bridge. Omitted keeps the Rust configuration default.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the configuration is invalid, for example
    /// a zero path count or Sobol combined with antithetic variates.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(js_name = "numPaths", unchecked_optional_param_type = "number")]
        num_paths: Option<JsValue>,
        #[wasm_bindgen(unchecked_optional_param_type = "number | bigint")] seed: Option<JsValue>,
        #[wasm_bindgen(js_name = "useParallel", unchecked_optional_param_type = "boolean")]
        use_parallel: Option<JsValue>,
        #[wasm_bindgen(unchecked_optional_param_type = "boolean")] antithetic: Option<JsValue>,
        #[wasm_bindgen(js_name = "useSobol", unchecked_optional_param_type = "boolean")] use_sobol: Option<JsValue>,
        #[wasm_bindgen(
            js_name = "useBrownianBridge",
            unchecked_optional_param_type = "boolean"
        )]
        use_brownian_bridge: Option<JsValue>,
    ) -> Result<JsPathDependentPricer, JsValue> {
        convenience::path_dependent_pricer(
            js_opt_uint(num_paths.as_ref(), "numPaths")?,
            js_opt_u64(seed.as_ref(), "seed")?,
            js_opt_bool(use_parallel.as_ref(), "useParallel")?,
            js_opt_bool(antithetic.as_ref(), "antithetic")?,
            js_opt_bool(use_sobol.as_ref(), "useSobol")?,
            js_opt_bool(use_brownian_bridge.as_ref(), "useBrownianBridge")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Number of Monte Carlo paths.
    #[wasm_bindgen(getter, js_name = numPaths)]
    pub fn num_paths(&self) -> usize {
        self.inner.config().num_paths
    }

    /// Seed of the random streams.
    #[wasm_bindgen(getter)]
    pub fn seed(&self) -> u64 {
        self.inner.config().seed
    }

    /// Whether paths run on the thread pool on native targets.
    #[wasm_bindgen(getter, js_name = useParallel)]
    pub fn use_parallel(&self) -> bool {
        self.inner.config().use_parallel
    }

    /// Whether antithetic variates are used.
    #[wasm_bindgen(getter)]
    pub fn antithetic(&self) -> bool {
        self.inner.config().antithetic
    }

    /// Whether Sobol quasi-random numbers are used.
    #[wasm_bindgen(getter, js_name = useSobol)]
    pub fn use_sobol(&self) -> bool {
        self.inner.config().use_sobol
    }

    /// Whether paths are built by Brownian bridge.
    #[wasm_bindgen(getter, js_name = useBrownianBridge)]
    pub fn use_brownian_bridge(&self) -> bool {
        self.inner.config().use_brownian_bridge
    }

    /// Price an arithmetic-average Asian option under GBM.
    ///
    /// # Arguments
    ///
    /// * `market` - `OptionMarketParams` object or JSON text, every key required: `spot` (spot level at time 0), `strike` (same units as `spot`), `rate` (continuously compounded risk-free rate, decimal), `dividend_yield` (continuous dividend yield, decimal), `volatility` (annualized GBM volatility, decimal), `time_to_expiry` (years) and `option_type` (`"call"` or `"put"`).
    /// * `num_steps` - Optional number of averaging steps; omitted uses the registry default.
    /// * `currency` - Optional ISO-4217 code stamped on the estimate; omitted uses the registry default.
    ///
    /// # Returns
    ///
    /// The `MoneyEstimate` object (`mean`, `stderr`, `ci_lower`, `ci_upper`, `num_paths`).
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if an input is non-finite or out of range or the currency code is unknown.
    /// @param market - `OptionMarketParams` object or JSON text, every key required: `spot` (spot level at time 0), `strike` (same units as `spot`), `rate` (continuously compounded risk-free rate, decimal), `dividend_yield` (continuous dividend yield, decimal), `volatility` (annualized GBM volatility, decimal), `time_to_expiry` (years) and `option_type` (`"call"` or `"put"`).
    /// @param numSteps - Optional number of averaging steps; omitted uses the registry default.
    /// @param currency - Optional ISO-4217 code stamped on the estimate; omitted uses the registry default.
    /// @returns The `MoneyEstimate` object (`mean`, `stderr`, `ci_lower`, `ci_upper`, `num_paths`).
    #[wasm_bindgen(js_name = priceAsian, unchecked_return_type = "MoneyEstimate")]
    pub fn price_asian(
        &self,
        #[wasm_bindgen(unchecked_param_type = "generated.models.OptionMarketParams | string")]
        market: JsValue,
        #[wasm_bindgen(js_name = "numSteps", unchecked_optional_param_type = "number")]
        num_steps: Option<JsValue>,
        #[wasm_bindgen(unchecked_optional_param_type = "string")] currency: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let market: OptionMarketParams = from_js_json(&market, "market")?;
        let num_steps = js_opt_uint(num_steps.as_ref(), "numSteps")?;
        let currency = js_opt_currency(currency.as_ref(), "currency")?;
        let estimate = self
            .inner
            .price_asian(&market, num_steps, currency)
            .map_err(to_js_err)?;
        to_js_value(&estimate)
    }

    /// Price an arithmetic-average Asian option under GBM with
    /// likelihood-ratio delta and vega estimated from the same paths.
    ///
    /// Every path is kept in memory, so `numPaths x (numSteps + 1)` may not
    /// exceed 4,000,000 and the default step count is 32, not the 252 steps
    /// of `priceAsian`.
    /// @param market - `OptionMarketParams` object or JSON text, every key required: `spot` (spot level at time 0), `strike` (same units as `spot`), `rate` (continuously compounded risk-free rate, decimal), `dividend_yield` (continuous dividend yield, decimal), `volatility` (annualized GBM volatility, decimal), `time_to_expiry` (years) and `option_type` (`"call"` or `"put"`).
    /// @param numSteps - Optional number of time-grid steps, each an averaging date; omitted uses the registry default `convenience.greeks.lrm_num_steps` (32).
    /// @param currency - Optional ISO-4217 code stamped on the price estimate; omitted uses the registry default.
    /// @returns The `LrmGreeks` object: `price` (a `MoneyEstimate`), `delta` per unit of spot and `vega` per volatility point (`0.01`), each an `Estimate` with `mean`, `stderr` and `ci_95`.
    ///
    /// @example
    /// ```typescript
    /// import init, { models } from "finstack-quant-wasm";
    /// await init();
    /// const pricer = new models.monteCarlo.PathDependentPricer(20000, 7);
    /// const greeks = pricer.priceWithLrmGreeks(
    ///   { spot: 100, strike: 100, rate: 0.04, dividend_yield: 0.01, volatility: 0.25, time_to_expiry: 1.0, option_type: "call" },
    ///   12,
    /// );
    /// console.log(greeks.price.mean.amount, greeks.price.mean.currency);
    /// console.log(greeks.delta.mean > 0 && greeks.delta.mean < 1); // true
    /// console.log(greeks.vega.stderr > 0); // true
    /// ```
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if an argument has the wrong JavaScript type, and
    /// a `validation` error if `spot`, `vol` or `expiry` is not finite and
    /// strictly positive, `numSteps` is zero, the currency code is unknown,
    /// the pricer uses Sobol or antithetic sampling, `numPaths` exceeds
    /// 100,000, or `numPaths x (numSteps + 1)` exceeds 4,000,000.
    #[wasm_bindgen(js_name = priceWithLrmGreeks, unchecked_return_type = "generated.models.LrmGreeks")]
    pub fn price_with_lrm_greeks(
        &self,
        #[wasm_bindgen(unchecked_param_type = "generated.models.OptionMarketParams | string")]
        market: JsValue,
        #[wasm_bindgen(js_name = "numSteps", unchecked_optional_param_type = "number")]
        num_steps: Option<JsValue>,
        #[wasm_bindgen(unchecked_optional_param_type = "string")] currency: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let market: OptionMarketParams = from_js_json(&market, "market")?;
        let num_steps = js_opt_uint(num_steps.as_ref(), "numSteps")?;
        let currency = js_opt_currency(currency.as_ref(), "currency")?;
        let greeks = self
            .inner
            .price_with_lrm_greeks(&market, num_steps, currency)
            .map_err(to_js_err)?;
        to_js_value(&greeks)
    }
}

/// Longstaff-Schwartz Monte Carlo pricer for American options under GBM.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const pricer = new models.monteCarlo.LsmcPricer(10000, 42, false, 50);
/// const put = pricer.priceAmerican(
///   { spot: 100, strike: 100, rate: 0.05, dividend_yield: 0.0, volatility: 0.2, time_to_expiry: 1.0, option_type: "put" },
/// );
/// console.log(put.mean.amount, pricer.basis, pricer.basisDegree);
/// pricer.free();
/// ```
#[wasm_bindgen(js_name = LsmcPricer)]
pub struct JsLsmcPricer {
    inner: LsmcPricer,
}

#[wasm_bindgen(js_class = LsmcPricer)]
impl JsLsmcPricer {
    /// Pricer whose omitted settings come from the Rust Monte Carlo registry.
    /// @param numPaths - Optional number of paths; a positive safe integer. Omitted uses the registry default.
    /// @param seed - Optional seed of the random streams, as a safe integer or `bigint`. Omitted uses the registry default.
    /// @param useParallel - Optional; run paths on the thread pool. WebAssembly always runs sequentially with identical results. Omitted uses the registry default.
    /// @param numSteps - Optional number of exercise dates between 0 and expiry. Omitted uses the registry default.
    /// @param basis - Optional regression basis name, `"laguerre"` or `"polynomial"`. Omitted uses the registry default.
    /// @param basisDegree - Optional highest polynomial degree of the regression basis. Omitted uses the registry default.
    /// @param antithetic - Optional; use antithetic variates. Omitted uses the registry default.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error for an unknown basis name or a zero path or
    /// step count.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(js_name = "numPaths", unchecked_optional_param_type = "number")]
        num_paths: Option<JsValue>,
        #[wasm_bindgen(unchecked_optional_param_type = "number | bigint")] seed: Option<JsValue>,
        #[wasm_bindgen(js_name = "useParallel", unchecked_optional_param_type = "boolean")]
        use_parallel: Option<JsValue>,
        #[wasm_bindgen(js_name = "numSteps", unchecked_optional_param_type = "number")]
        num_steps: Option<JsValue>,
        #[wasm_bindgen(unchecked_optional_param_type = "string")] basis: Option<JsValue>,
        #[wasm_bindgen(js_name = "basisDegree", unchecked_optional_param_type = "number")]
        basis_degree: Option<JsValue>,
        #[wasm_bindgen(unchecked_optional_param_type = "boolean")] antithetic: Option<JsValue>,
    ) -> Result<JsLsmcPricer, JsValue> {
        let basis = js_opt_string(basis.as_ref(), "basis")?;
        LsmcPricer::new(
            js_opt_uint(num_paths.as_ref(), "numPaths")?,
            js_opt_u64(seed.as_ref(), "seed")?,
            js_opt_bool(use_parallel.as_ref(), "useParallel")?,
            js_opt_uint(num_steps.as_ref(), "numSteps")?,
            basis.as_deref(),
            js_opt_uint(basis_degree.as_ref(), "basisDegree")?,
            js_opt_bool(antithetic.as_ref(), "antithetic")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Number of Monte Carlo paths.
    #[wasm_bindgen(getter, js_name = numPaths)]
    pub fn num_paths(&self) -> usize {
        self.inner.pricer().config().num_paths
    }

    /// Seed of the random streams.
    #[wasm_bindgen(getter)]
    pub fn seed(&self) -> u64 {
        self.inner.pricer().config().seed
    }

    /// Whether paths run on the thread pool on native targets.
    #[wasm_bindgen(getter, js_name = useParallel)]
    pub fn use_parallel(&self) -> bool {
        self.inner.pricer().config().use_parallel
    }

    /// Whether antithetic variates are used.
    #[wasm_bindgen(getter)]
    pub fn antithetic(&self) -> bool {
        self.inner.pricer().config().antithetic
    }

    /// Default regression basis name.
    #[wasm_bindgen(getter)]
    pub fn basis(&self) -> String {
        self.inner.basis().as_str().to_owned()
    }

    /// Default highest polynomial degree of the regression basis.
    #[wasm_bindgen(getter, js_name = basisDegree)]
    pub fn basis_degree(&self) -> usize {
        self.inner.basis_degree()
    }

    /// Price an American option by Longstaff-Schwartz regression (in-sample exercise policy).
    ///
    /// # Arguments
    ///
    /// * `market` - `OptionMarketParams` object or JSON text, every key required: `spot` (spot level at time 0), `strike` (same units as `spot`), `rate` (continuously compounded risk-free rate, decimal), `dividend_yield` (continuous dividend yield, decimal), `volatility` (annualized GBM volatility, decimal), `time_to_expiry` (years) and `option_type` (`"call"` or `"put"`).
    /// * `currency` - Optional ISO-4217 code stamped on the estimate; omitted uses the registry default.
    /// * `num_steps` - Optional exercise dates for this call; omitted keeps the pricer's value.
    /// * `basis` - Optional regression basis for this call; omitted keeps the pricer's basis.
    /// * `basis_degree` - Optional basis degree for this call; omitted keeps the pricer's degree.
    ///
    /// # Returns
    ///
    /// The `MoneyEstimate` object (`mean`, `stderr`, `ci_lower`, `ci_upper`, `num_paths`).
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if an input is out of range, the basis name or currency code is unknown, or the regression is ill-conditioned.
    /// @param market - `OptionMarketParams` object or JSON text, every key required: `spot` (spot level at time 0), `strike` (same units as `spot`), `rate` (continuously compounded risk-free rate, decimal), `dividend_yield` (continuous dividend yield, decimal), `volatility` (annualized GBM volatility, decimal), `time_to_expiry` (years) and `option_type` (`"call"` or `"put"`).
    /// @param currency - Optional ISO-4217 code stamped on the estimate; omitted uses the registry default.
    /// @param numSteps - Optional exercise dates for this call; omitted keeps the pricer's value.
    /// @param basis - Optional regression basis for this call; omitted keeps the pricer's basis.
    /// @param basisDegree - Optional basis degree for this call; omitted keeps the pricer's degree.
    /// @returns The `MoneyEstimate` object (`mean`, `stderr`, `ci_lower`, `ci_upper`, `num_paths`).
    #[wasm_bindgen(js_name = priceAmerican, unchecked_return_type = "MoneyEstimate")]
    pub fn price_american(
        &self,
        #[wasm_bindgen(unchecked_param_type = "generated.models.OptionMarketParams | string")]
        market: JsValue,
        #[wasm_bindgen(unchecked_optional_param_type = "string")] currency: Option<JsValue>,
        #[wasm_bindgen(js_name = "numSteps", unchecked_optional_param_type = "number")]
        num_steps: Option<JsValue>,
        #[wasm_bindgen(unchecked_optional_param_type = "string")] basis: Option<JsValue>,
        #[wasm_bindgen(js_name = "basisDegree", unchecked_optional_param_type = "number")]
        basis_degree: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let market: OptionMarketParams = from_js_json(&market, "market")?;
        let currency = js_opt_currency(currency.as_ref(), "currency")?;
        let num_steps = js_opt_uint(num_steps.as_ref(), "numSteps")?;
        let basis = js_opt_string(basis.as_ref(), "basis")?;
        let basis_degree = js_opt_uint(basis_degree.as_ref(), "basisDegree")?;
        let estimate = self
            .inner
            .price_american(&market, num_steps, currency, basis.as_deref(), basis_degree)
            .map_err(to_js_err)?;
        to_js_value(&estimate)
    }

    /// Price an American option with an out-of-sample exercise policy: the
    /// regression is fitted on the pricer's paths and applied to fresh paths
    /// drawn from `pricing_seed`, removing the in-sample high bias.
    ///
    /// # Arguments
    ///
    /// * `market` - `OptionMarketParams` object or JSON text, every key required: `spot` (spot level at time 0), `strike` (same units as `spot`), `rate` (continuously compounded risk-free rate, decimal), `dividend_yield` (continuous dividend yield, decimal), `volatility` (annualized GBM volatility, decimal), `time_to_expiry` (years) and `option_type` (`"call"` or `"put"`).
    /// * `pricing_seed` - Seed of the independent pricing paths, as a safe integer or `bigint`; must differ from the pricer's seed.
    /// * `currency` - Optional ISO-4217 code stamped on the estimate; omitted uses the registry default.
    /// * `num_steps` - Optional exercise dates for this call; omitted keeps the pricer's value.
    /// * `basis` - Optional regression basis for this call; omitted keeps the pricer's basis.
    /// * `basis_degree` - Optional basis degree for this call; omitted keeps the pricer's degree.
    ///
    /// # Returns
    ///
    /// The `MoneyEstimate` object (`mean`, `stderr`, `ci_lower`, `ci_upper`, `num_paths`).
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if an input is out of range, the basis name or currency code is unknown, or `pricingSeed` equals the training seed.
    /// @param market - `OptionMarketParams` object or JSON text, every key required: `spot` (spot level at time 0), `strike` (same units as `spot`), `rate` (continuously compounded risk-free rate, decimal), `dividend_yield` (continuous dividend yield, decimal), `volatility` (annualized GBM volatility, decimal), `time_to_expiry` (years) and `option_type` (`"call"` or `"put"`).
    /// @param pricingSeed - Seed of the independent pricing paths, as a safe integer or `bigint`; must differ from the pricer's seed.
    /// @param currency - Optional ISO-4217 code stamped on the estimate; omitted uses the registry default.
    /// @param numSteps - Optional exercise dates for this call; omitted keeps the pricer's value.
    /// @param basis - Optional regression basis for this call; omitted keeps the pricer's basis.
    /// @param basisDegree - Optional basis degree for this call; omitted keeps the pricer's degree.
    /// @returns The `MoneyEstimate` object (`mean`, `stderr`, `ci_lower`, `ci_upper`, `num_paths`).
    #[wasm_bindgen(js_name = priceAmericanUnbiased, unchecked_return_type = "MoneyEstimate")]
    pub fn price_american_unbiased(
        &self,
        #[wasm_bindgen(unchecked_param_type = "generated.models.OptionMarketParams | string")]
        market: JsValue,
        #[wasm_bindgen(js_name = "pricingSeed", unchecked_param_type = "number | bigint")]
        pricing_seed: JsValue,
        #[wasm_bindgen(unchecked_optional_param_type = "string")] currency: Option<JsValue>,
        #[wasm_bindgen(js_name = "numSteps", unchecked_optional_param_type = "number")]
        num_steps: Option<JsValue>,
        #[wasm_bindgen(unchecked_optional_param_type = "string")] basis: Option<JsValue>,
        #[wasm_bindgen(js_name = "basisDegree", unchecked_optional_param_type = "number")]
        basis_degree: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let market: OptionMarketParams = from_js_json(&market, "market")?;
        let pricing_seed = js_u64(&pricing_seed, "pricingSeed")?;
        let currency = js_opt_currency(currency.as_ref(), "currency")?;
        let num_steps = js_opt_uint(num_steps.as_ref(), "numSteps")?;
        let basis = js_opt_string(basis.as_ref(), "basis")?;
        let basis_degree = js_opt_uint(basis_degree.as_ref(), "basisDegree")?;
        let estimate = self
            .inner
            .price_american_unbiased(
                &market,
                num_steps,
                currency,
                basis.as_deref(),
                basis_degree,
                pricing_seed,
            )
            .map_err(to_js_err)?;
        to_js_value(&estimate)
    }
}
