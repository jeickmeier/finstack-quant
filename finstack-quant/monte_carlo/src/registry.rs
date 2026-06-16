//! Embedded Monte Carlo defaults registry.
//!
//! Runtime defaults are versioned JSON data so path counts, seeds, parallel
//! defaults, and Python convenience defaults can be reviewed and changed
//! without hunting through constructor bodies.

use std::sync::OnceLock;

use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::{Error, Result};
use serde::Deserialize;

/// Config extension key for overriding Monte Carlo defaults.
pub const MONTE_CARLO_DEFAULTS_EXTENSION_KEY: &str = "monte_carlo.defaults.v1";

const PRICER_DEFAULTS: &str = include_str!("../data/defaults/pricer_defaults.v1.json");

static EMBEDDED_DEFAULTS: OnceLock<Result<MonteCarloDefaults>> = OnceLock::new();

/// Resolved Monte Carlo defaults.
#[derive(Debug, Clone)]
pub struct MonteCarloDefaults {
    /// Rust API defaults.
    pub rust: RustDefaults,
    /// Python binding convenience defaults.
    pub python_bindings: PythonBindingDefaults,
}

/// Defaults used by Rust Monte Carlo APIs.
#[derive(Debug, Clone)]
pub struct RustDefaults {
    /// Generic engine defaults.
    pub engine: EngineDefaults,
    /// Engine-builder defaults.
    pub engine_builder: EngineBuilderDefaults,
    /// European pricer defaults.
    pub european_pricer: PricerRuntimeDefaults,
    /// Path-dependent pricer defaults.
    pub path_dependent_pricer: PathDependentPricerDefaults,
    /// LSMC configuration defaults.
    pub lsmc: LsmcRuntimeDefaults,
    /// Shared rate-exotic Monte Carlo defaults.
    pub rate_exotics: RateExoticDefaults,
    /// Swaption LSMC defaults.
    pub swaption_lsmc: SwaptionLsmcDefaults,
    /// LMM Bermudan swaption defaults.
    pub lmm_bermudan: LmmBermudanDefaults,
    /// Cheyette rough-vol Bermudan swaption defaults.
    pub cheyette_rough: CheyetteRoughDefaults,
    /// Merton PIK-bond Monte Carlo defaults.
    pub merton_pik_bond: MertonPikBondDefaults,
}

/// Defaults used by Python Monte Carlo bindings.
#[derive(Debug, Clone)]
pub struct PythonBindingDefaults {
    /// Default currency code for Python convenience functions.
    pub default_currency: String,
    /// Python engine constructor defaults.
    pub engine: PythonEngineDefaults,
    /// Python European pricer defaults.
    pub european_pricer: PythonPricerDefaults,
    /// Python path-dependent pricer defaults.
    pub path_dependent_pricer: PythonPricerDefaults,
    /// Python LSMC pricer defaults.
    pub lsmc: PythonLsmcDefaults,
    /// Python Greek estimator defaults.
    pub greeks: PythonGreekDefaults,
}

/// Common path-count, seed, and parallel-execution defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PricerRuntimeDefaults {
    /// Number of Monte Carlo paths.
    pub num_paths: usize,
    /// Root RNG seed.
    pub seed: u64,
    /// Whether parallel execution is requested by default.
    pub use_parallel: bool,
}

/// Generic engine runtime defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineDefaults {
    /// Whether parallel execution is requested by default.
    pub use_parallel: bool,
    /// Parallel chunk size.
    pub chunk_size: usize,
    /// Whether antithetic variance reduction is enabled by default.
    pub antithetic: bool,
}

/// Engine-builder runtime defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineBuilderDefaults {
    /// Number of Monte Carlo paths.
    pub num_paths: usize,
    /// Whether parallel execution is requested by default.
    pub use_parallel: bool,
    /// Parallel chunk size.
    pub chunk_size: usize,
    /// Whether antithetic variance reduction is enabled by default.
    pub antithetic: bool,
}

/// Path-dependent pricer runtime defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathDependentPricerDefaults {
    /// Number of Monte Carlo paths.
    pub num_paths: usize,
    /// Root RNG seed.
    pub seed: u64,
    /// Whether parallel execution is requested by default.
    pub use_parallel: bool,
    /// Parallel chunk size.
    pub chunk_size: usize,
    /// Steps per year for automatic time-grid construction.
    pub steps_per_year: f64,
    /// Minimum number of time-grid steps.
    pub min_steps: usize,
    /// Whether Sobol QMC is enabled by default.
    pub use_sobol: bool,
    /// Whether antithetic variance reduction is enabled by default.
    pub antithetic: bool,
    /// Whether Brownian-bridge ordering is enabled by default.
    pub use_brownian_bridge: bool,
}

/// LSMC runtime defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LsmcRuntimeDefaults {
    /// Root RNG seed.
    pub seed: u64,
    /// Whether parallel execution is requested by default.
    pub use_parallel: bool,
}

/// Shared rate-exotic Monte Carlo defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RateExoticDefaults {
    /// Number of Monte Carlo paths.
    pub num_paths: usize,
    /// Root RNG seed.
    pub seed: u64,
    /// Whether antithetic variance reduction is enabled by default.
    pub antithetic: bool,
    /// Minimum number of simulation sub-steps between events.
    pub min_steps_between_events: usize,
    /// Polynomial basis degree for LSMC regression.
    pub basis_degree: usize,
}

/// Swaption LSMC defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SwaptionLsmcDefaults {
    /// Number of Monte Carlo paths.
    pub num_paths: usize,
    /// Root RNG seed.
    pub seed: u64,
    /// Polynomial basis degree for LSMC regression.
    pub basis_degree: usize,
    /// Whether antithetic variance reduction is enabled by default.
    pub antithetic: bool,
}

/// LMM Bermudan swaption defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LmmBermudanDefaults {
    /// Number of Monte Carlo paths.
    pub num_paths: usize,
    /// Root RNG seed.
    pub seed: u64,
    /// Polynomial basis degree for LSMC regression.
    pub basis_degree: usize,
    /// Whether antithetic variance reduction is enabled by default.
    pub antithetic: bool,
    /// Minimum simulation steps between exercise dates.
    pub min_steps_between_exercises: usize,
}

/// Cheyette rough-vol Bermudan swaption defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheyetteRoughDefaults {
    /// Number of Monte Carlo paths.
    pub num_paths: usize,
    /// Number of simulation time steps.
    pub num_steps: usize,
    /// Polynomial basis degree for LSMC regression.
    pub basis_degree: usize,
}

/// Merton PIK-bond Monte Carlo defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MertonPikBondDefaults {
    /// Number of Monte Carlo paths.
    pub num_paths: usize,
    /// Root RNG seed.
    pub seed: u64,
    /// Whether antithetic variance reduction is enabled by default.
    pub antithetic: bool,
    /// Simulation time steps per year.
    pub time_steps_per_year: usize,
}

/// Python pricer defaults with default time-grid step count.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PythonPricerDefaults {
    /// Number of Monte Carlo paths.
    pub num_paths: usize,
    /// Root RNG seed.
    pub seed: u64,
    /// Whether parallel execution is requested by default.
    pub use_parallel: bool,
    /// Default number of time steps for GBM convenience pricing methods.
    pub num_steps: usize,
}

/// Python engine constructor defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PythonEngineDefaults {
    /// Root RNG seed.
    pub seed: u64,
    /// Whether parallel execution is requested by default.
    pub use_parallel: bool,
    /// Whether antithetic variance reduction is enabled by default.
    pub antithetic: bool,
}

/// Python LSMC pricer defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PythonLsmcDefaults {
    /// Number of Monte Carlo paths.
    pub num_paths: usize,
    /// Root RNG seed.
    pub seed: u64,
    /// Whether parallel execution is requested by default.
    pub use_parallel: bool,
    /// Default regression basis name.
    pub basis: String,
    /// Default regression basis degree.
    pub basis_degree: usize,
    /// Default number of time steps for American option methods.
    pub num_steps: usize,
}

/// Python finite-difference Greek estimator defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PythonGreekDefaults {
    /// Number of Monte Carlo paths.
    pub num_paths: usize,
    /// Root RNG seed.
    pub seed: u64,
    /// Default number of time steps.
    pub num_steps: usize,
    /// Relative bump size.
    pub bump_size: f64,
    /// Default option type label.
    pub option_type: String,
    /// Whether parallel execution is requested by default.
    pub use_parallel: bool,
    /// Parallel chunk size.
    pub chunk_size: usize,
    /// Whether antithetic variance reduction is enabled by default.
    pub antithetic: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DefaultsFile {
    schema: Option<String>,
    version: Option<u32>,
    rust: RustDefaultsFile,
    python_bindings: PythonBindingDefaultsFile,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RustDefaultsFile {
    engine: EngineDefaults,
    engine_builder: EngineBuilderDefaults,
    european_pricer: PricerRuntimeDefaults,
    path_dependent_pricer: PathDependentPricerDefaults,
    lsmc: LsmcRuntimeDefaults,
    rate_exotics: RateExoticDefaults,
    swaption_lsmc: SwaptionLsmcDefaults,
    lmm_bermudan: LmmBermudanDefaults,
    cheyette_rough: CheyetteRoughDefaults,
    merton_pik_bond: MertonPikBondDefaults,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PythonBindingDefaultsFile {
    default_currency: String,
    engine: PythonEngineDefaults,
    european_pricer: PythonPricerDefaults,
    path_dependent_pricer: PythonPricerDefaults,
    lsmc: PythonLsmcDefaults,
    greeks: PythonGreekDefaults,
}

/// Return the embedded Monte Carlo defaults.
pub fn embedded_defaults() -> Result<&'static MonteCarloDefaults> {
    match EMBEDDED_DEFAULTS.get_or_init(parse_embedded_defaults) {
        Ok(defaults) => Ok(defaults),
        Err(err) => Err(err.clone()),
    }
}

/// Panic-on-failure access for `Default` implementations backed by embedded data.
#[must_use]
#[allow(clippy::expect_used)]
pub fn embedded_defaults_or_panic() -> &'static MonteCarloDefaults {
    embedded_defaults().expect("embedded Monte Carlo defaults are compile-time assets")
}

/// Loads Monte Carlo defaults from configuration or falls back to embedded defaults.
pub fn defaults_from_config(config: &FinstackConfig) -> Result<MonteCarloDefaults> {
    if let Some(value) = config.extensions.get(MONTE_CARLO_DEFAULTS_EXTENSION_KEY) {
        let file: DefaultsFile = serde_json::from_value(value.clone()).map_err(|err| {
            Error::Validation(format!(
                "failed to parse Monte Carlo defaults extension: {err}"
            ))
        })?;
        defaults_from_file(file)
    } else {
        Ok(embedded_defaults()?.clone())
    }
}

fn parse_embedded_defaults() -> Result<MonteCarloDefaults> {
    let file: DefaultsFile = serde_json::from_str(PRICER_DEFAULTS).map_err(|err| {
        Error::Validation(format!(
            "failed to parse embedded Monte Carlo defaults: {err}"
        ))
    })?;
    defaults_from_file(file)
}

fn defaults_from_file(file: DefaultsFile) -> Result<MonteCarloDefaults> {
    validate_file(&file)?;
    Ok(MonteCarloDefaults {
        rust: RustDefaults {
            engine: file.rust.engine,
            engine_builder: file.rust.engine_builder,
            european_pricer: file.rust.european_pricer,
            path_dependent_pricer: file.rust.path_dependent_pricer,
            lsmc: file.rust.lsmc,
            rate_exotics: file.rust.rate_exotics,
            swaption_lsmc: file.rust.swaption_lsmc,
            lmm_bermudan: file.rust.lmm_bermudan,
            cheyette_rough: file.rust.cheyette_rough,
            merton_pik_bond: file.rust.merton_pik_bond,
        },
        python_bindings: PythonBindingDefaults {
            default_currency: file.python_bindings.default_currency,
            engine: file.python_bindings.engine,
            european_pricer: file.python_bindings.european_pricer,
            path_dependent_pricer: file.python_bindings.path_dependent_pricer,
            lsmc: file.python_bindings.lsmc,
            greeks: file.python_bindings.greeks,
        },
    })
}

fn validate_file(file: &DefaultsFile) -> Result<()> {
    let _schema = &file.schema;
    let _version = file.version;
    validate_runtime("rust.european_pricer", &file.rust.european_pricer)?;
    validate_runtime(
        "rust.path_dependent_pricer",
        &PricerRuntimeDefaults {
            num_paths: file.rust.path_dependent_pricer.num_paths,
            seed: file.rust.path_dependent_pricer.seed,
            use_parallel: file.rust.path_dependent_pricer.use_parallel,
        },
    )?;
    validate_runtime(
        "rust.engine_builder",
        &PricerRuntimeDefaults {
            num_paths: file.rust.engine_builder.num_paths,
            seed: 0,
            use_parallel: file.rust.engine_builder.use_parallel,
        },
    )?;
    validate_engine("rust.engine", &file.rust.engine)?;
    validate_chunk_size("rust.engine.chunk_size", file.rust.engine.chunk_size)?;
    validate_chunk_size(
        "rust.engine_builder.chunk_size",
        file.rust.engine_builder.chunk_size,
    )?;
    validate_chunk_size(
        "rust.path_dependent_pricer.chunk_size",
        file.rust.path_dependent_pricer.chunk_size,
    )?;
    validate_positive_f64(
        "rust.path_dependent_pricer.steps_per_year",
        file.rust.path_dependent_pricer.steps_per_year,
    )?;
    validate_positive_usize(
        "rust.path_dependent_pricer.min_steps",
        file.rust.path_dependent_pricer.min_steps,
    )?;
    validate_python_pricer(
        "python_bindings.european_pricer",
        &file.python_bindings.european_pricer,
    )?;
    validate_nonblank(
        "python_bindings.default_currency",
        &file.python_bindings.default_currency,
    )?;
    validate_python_engine(&file.python_bindings.engine);
    validate_python_pricer(
        "python_bindings.path_dependent_pricer",
        &file.python_bindings.path_dependent_pricer,
    )?;
    validate_rate_exotics("rust.rate_exotics", &file.rust.rate_exotics)?;
    validate_swaption_lsmc("rust.swaption_lsmc", &file.rust.swaption_lsmc)?;
    validate_lmm_bermudan("rust.lmm_bermudan", &file.rust.lmm_bermudan)?;
    validate_cheyette_rough("rust.cheyette_rough", &file.rust.cheyette_rough)?;
    validate_merton_pik_bond("rust.merton_pik_bond", &file.rust.merton_pik_bond)?;
    validate_python_lsmc("python_bindings.lsmc", &file.python_bindings.lsmc)?;
    validate_python_greeks("python_bindings.greeks", &file.python_bindings.greeks)?;
    Ok(())
}

fn validate_runtime(label: &str, defaults: &PricerRuntimeDefaults) -> Result<()> {
    validate_positive_usize(&format!("{label}.num_paths"), defaults.num_paths)?;
    let _seed = defaults.seed;
    let _parallel = defaults.use_parallel;
    Ok(())
}

fn validate_engine(label: &str, defaults: &EngineDefaults) -> Result<()> {
    let _parallel = defaults.use_parallel;
    let _antithetic = defaults.antithetic;
    validate_chunk_size(&format!("{label}.chunk_size"), defaults.chunk_size)
}

fn validate_python_pricer(label: &str, defaults: &PythonPricerDefaults) -> Result<()> {
    validate_positive_usize(&format!("{label}.num_paths"), defaults.num_paths)?;
    validate_positive_usize(&format!("{label}.num_steps"), defaults.num_steps)?;
    let _seed = defaults.seed;
    let _parallel = defaults.use_parallel;
    Ok(())
}

fn validate_python_engine(defaults: &PythonEngineDefaults) {
    let _seed = defaults.seed;
    let _parallel = defaults.use_parallel;
    let _antithetic = defaults.antithetic;
}

fn validate_python_lsmc(label: &str, defaults: &PythonLsmcDefaults) -> Result<()> {
    validate_positive_usize(&format!("{label}.num_paths"), defaults.num_paths)?;
    validate_positive_usize(&format!("{label}.basis_degree"), defaults.basis_degree)?;
    validate_positive_usize(&format!("{label}.num_steps"), defaults.num_steps)?;
    if defaults.basis.trim().is_empty() {
        return Err(Error::Validation(format!(
            "{label}.basis must not be blank"
        )));
    }
    let _seed = defaults.seed;
    let _parallel = defaults.use_parallel;
    Ok(())
}

fn validate_python_greeks(label: &str, defaults: &PythonGreekDefaults) -> Result<()> {
    validate_positive_usize(&format!("{label}.num_paths"), defaults.num_paths)?;
    validate_positive_usize(&format!("{label}.num_steps"), defaults.num_steps)?;
    validate_positive_f64(&format!("{label}.bump_size"), defaults.bump_size)?;
    validate_chunk_size(&format!("{label}.chunk_size"), defaults.chunk_size)?;
    validate_nonblank(&format!("{label}.option_type"), &defaults.option_type)?;
    let _seed = defaults.seed;
    let _parallel = defaults.use_parallel;
    let _antithetic = defaults.antithetic;
    Ok(())
}

fn validate_rate_exotics(label: &str, defaults: &RateExoticDefaults) -> Result<()> {
    validate_positive_usize(&format!("{label}.num_paths"), defaults.num_paths)?;
    validate_positive_usize(
        &format!("{label}.min_steps_between_events"),
        defaults.min_steps_between_events,
    )?;
    validate_positive_usize(&format!("{label}.basis_degree"), defaults.basis_degree)?;
    let _seed = defaults.seed;
    let _antithetic = defaults.antithetic;
    Ok(())
}

fn validate_swaption_lsmc(label: &str, defaults: &SwaptionLsmcDefaults) -> Result<()> {
    validate_positive_usize(&format!("{label}.num_paths"), defaults.num_paths)?;
    validate_positive_usize(&format!("{label}.basis_degree"), defaults.basis_degree)?;
    let _seed = defaults.seed;
    let _antithetic = defaults.antithetic;
    Ok(())
}

fn validate_lmm_bermudan(label: &str, defaults: &LmmBermudanDefaults) -> Result<()> {
    validate_positive_usize(&format!("{label}.num_paths"), defaults.num_paths)?;
    validate_positive_usize(&format!("{label}.basis_degree"), defaults.basis_degree)?;
    validate_positive_usize(
        &format!("{label}.min_steps_between_exercises"),
        defaults.min_steps_between_exercises,
    )?;
    let _seed = defaults.seed;
    let _antithetic = defaults.antithetic;
    Ok(())
}

fn validate_cheyette_rough(label: &str, defaults: &CheyetteRoughDefaults) -> Result<()> {
    validate_positive_usize(&format!("{label}.num_paths"), defaults.num_paths)?;
    validate_positive_usize(&format!("{label}.num_steps"), defaults.num_steps)?;
    validate_positive_usize(&format!("{label}.basis_degree"), defaults.basis_degree)
}

fn validate_merton_pik_bond(label: &str, defaults: &MertonPikBondDefaults) -> Result<()> {
    validate_positive_usize(&format!("{label}.num_paths"), defaults.num_paths)?;
    validate_positive_usize(
        &format!("{label}.time_steps_per_year"),
        defaults.time_steps_per_year,
    )?;
    let _seed = defaults.seed;
    let _antithetic = defaults.antithetic;
    Ok(())
}

fn validate_chunk_size(label: &str, value: usize) -> Result<()> {
    validate_positive_usize(label, value)
}

fn validate_positive_usize(label: &str, value: usize) -> Result<()> {
    if value == 0 {
        return Err(Error::Validation(format!("{label} must be positive")));
    }
    Ok(())
}

fn validate_positive_f64(label: &str, value: f64) -> Result<()> {
    if !value.is_finite() || value <= 0.0 {
        return Err(Error::Validation(format!("{label} must be positive")));
    }
    Ok(())
}

fn validate_nonblank(label: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(Error::Validation(format!("{label} must not be blank")));
    }
    Ok(())
}
