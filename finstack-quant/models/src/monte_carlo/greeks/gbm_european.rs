//! Canonical GBM European finite-difference Greek entry points for host bindings.
//!
//! These free functions own the `TimeGrid` + engine + GBM process + vanilla
//! payoff composition that Python exposes as `finite_diff_delta` /
//! `finite_diff_gamma` (and the CRN variants), so the pipeline — including
//! registry-backed defaults — is defined once in Rust.

use crate::monte_carlo::discretization::exact::ExactGbm;
use crate::monte_carlo::engine::{McEngine, McEngineConfig};
use crate::monte_carlo::estimate::Estimate;
use crate::monte_carlo::greeks::finite_diff::{
    finite_diff_delta, finite_diff_delta_crn, finite_diff_gamma, finite_diff_gamma_crn,
    FiniteDiffInputs,
};
use crate::monte_carlo::payoff::vanilla::{EuropeanCall, EuropeanPut};
use crate::monte_carlo::process::gbm::GbmProcess;
use crate::monte_carlo::registry;
use crate::monte_carlo::rng::philox::PhiloxRng;
use crate::monte_carlo::TimeGrid;
use crate::types::OptionType;
use finstack_quant_core::cashflow::flat_discount_factor;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::Result;
use std::str::FromStr;

/// Two-sided 95% normal quantile used to turn `(value, stderr)` into a CI.
const CI_95_Z: f64 = 1.96;

/// Inputs for the GBM European finite-difference Greek convenience functions.
#[derive(Debug, Clone)]
pub struct GbmEuropeanFdSpec {
    /// Spot level at time `0`.
    pub spot: f64,
    /// Exercise price in the same units as `spot`.
    pub strike: f64,
    /// Continuously compounded risk-free rate (decimal, annualized).
    pub rate: f64,
    /// Continuous dividend yield (decimal, annualized).
    pub div_yield: f64,
    /// Annualized GBM volatility (decimal); must be strictly positive.
    pub vol: f64,
    /// Time to expiry in years; also the uniform-grid horizon.
    pub expiry: f64,
    /// Independent estimators; `None` uses the registry binding default.
    /// Each estimator uses two physical paths when the registry enables antithetics.
    pub num_paths: Option<usize>,
    /// RNG seed; `None` uses the registry binding default.
    pub seed: Option<u64>,
    /// Time steps per path; `None` uses the registry binding default.
    pub num_steps: Option<usize>,
    /// Relative spot bump used as an MC finite-difference shock; `None` uses
    /// the registry binding default (a 1% of spot MC shock, not a local
    /// closed-form step).
    pub bump_size: Option<f64>,
    /// Call or put payoff. Required; there is no default option type.
    pub option_type: OptionType,
    /// Currency stamped on simulated payoffs; `None` uses the registry default.
    pub currency: Option<Currency>,
}

enum FdKind {
    Delta,
    DeltaCrn,
    Gamma,
    GammaCrn,
}

/// Finite-difference delta for a vanilla European option under GBM.
///
/// Both this function and [`finite_diff_delta_crn_gbm`] reuse common random
/// numbers via a splittable RNG. This function reports a **conservative
/// independence-bound** stderr; [`finite_diff_delta_crn_gbm`] reports the
/// tighter paired CRN stderr.
///
/// # Arguments
///
/// * `spec` - Spot, strike, GBM parameters, required `option_type`, and
///   optional registry overrides. `bump_size` is a relative MC shock
///   (registry default `0.01` = 1% of spot).
///
/// # Returns
///
/// An [`Estimate`] whose `mean` is the delta, `stderr` the estimator's
/// standard error, `ci_95` the symmetric normal 95% band, and `num_paths` the
/// independent estimator count used per bumped valuation. `num_simulated_paths`
/// records the physical path count, including antithetic partners.
///
/// # Errors
///
/// Returns an error if registry defaults cannot be loaded, `vol` is not
/// strictly positive, GBM or bump inputs fail validation, or either pricing
/// run fails.
pub fn finite_diff_delta_gbm(spec: GbmEuropeanFdSpec) -> Result<Estimate> {
    run_gbm_fd(spec, FdKind::Delta)
}

/// Finite-difference delta with paired common-random-number stderr.
///
/// Same CRN-priced central difference as [`finite_diff_delta_gbm`]; only the
/// reported stderr estimator differs (paired pathwise differences instead of
/// the independence bound).
///
/// # Arguments
///
/// * `spec` - Spot, strike, GBM parameters, required `option_type`, and
///   optional registry overrides.
///
/// # Errors
///
/// Same failure modes as [`finite_diff_delta_gbm`].
pub fn finite_diff_delta_crn_gbm(spec: GbmEuropeanFdSpec) -> Result<Estimate> {
    run_gbm_fd(spec, FdKind::DeltaCrn)
}

/// Finite-difference gamma for a vanilla European option under GBM.
///
/// Both this function and [`finite_diff_gamma_crn_gbm`] reuse common random
/// numbers. This function reports a conservative independence-bound stderr.
///
/// # Arguments
///
/// * `spec` - Spot, strike, GBM parameters, required `option_type`, and
///   optional registry overrides.
///
/// # Errors
///
/// Same failure modes as [`finite_diff_delta_gbm`].
pub fn finite_diff_gamma_gbm(spec: GbmEuropeanFdSpec) -> Result<Estimate> {
    run_gbm_fd(spec, FdKind::Gamma)
}

/// Finite-difference gamma with paired common-random-number stderr.
///
/// Same CRN-priced second difference as [`finite_diff_gamma_gbm`]; only the
/// reported stderr estimator differs.
///
/// # Arguments
///
/// * `spec` - Spot, strike, GBM parameters, required `option_type`, and
///   optional registry overrides.
///
/// # Errors
///
/// Same failure modes as [`finite_diff_delta_gbm`].
pub fn finite_diff_gamma_crn_gbm(spec: GbmEuropeanFdSpec) -> Result<Estimate> {
    run_gbm_fd(spec, FdKind::GammaCrn)
}

fn parse_registry_currency(code: &str) -> Result<Currency> {
    Currency::from_str(code).map_err(|err| {
        finstack_quant_core::Error::Validation(format!(
            "invalid registry default currency '{code}': {err}"
        ))
    })
}

fn run_gbm_fd(spec: GbmEuropeanFdSpec, kind: FdKind) -> Result<Estimate> {
    let defaults = &registry::embedded_defaults()?.convenience.greeks;
    let num_paths = spec.num_paths.unwrap_or(defaults.num_paths);
    let seed = spec.seed.unwrap_or(defaults.seed);
    let num_steps = spec.num_steps.unwrap_or(defaults.num_steps);
    let bump_size = spec.bump_size.unwrap_or(defaults.bump_size);
    let currency = match spec.currency {
        Some(currency) => currency,
        None => {
            parse_registry_currency(&registry::embedded_defaults()?.convenience.default_currency)?
        }
    };

    #[cfg(target_arch = "wasm32")]
    let use_parallel = false;
    #[cfg(not(target_arch = "wasm32"))]
    let use_parallel = defaults.use_parallel;

    crate::monte_carlo::require_positive_vol(spec.vol)?;
    let time_grid = TimeGrid::uniform(spec.expiry, num_steps)?;
    let engine = McEngine::new(
        McEngineConfig::new(num_paths, time_grid)
            .parallel(use_parallel)
            .chunk_size(defaults.chunk_size)
            .antithetic(defaults.antithetic),
    );
    let rng = PhiloxRng::new(seed);
    let gbm = GbmProcess::with_params(spec.rate, spec.div_yield, spec.vol)?;
    let disc = ExactGbm::new();
    let discount_factor = flat_discount_factor(spec.rate, spec.expiry)?;

    let (value, stderr) = match spec.option_type {
        OptionType::Call => {
            let payoff = EuropeanCall::new(spec.strike, 1.0, num_steps);
            let inputs = FiniteDiffInputs {
                engine: &engine,
                rng: &rng,
                process: &gbm,
                disc: &disc,
                payoff: &payoff,
                currency,
                discount_factor,
            };
            match kind {
                FdKind::Delta => finite_diff_delta(&inputs, spec.spot, bump_size)?,
                FdKind::DeltaCrn => finite_diff_delta_crn(&inputs, spec.spot, bump_size)?,
                FdKind::Gamma => finite_diff_gamma(&inputs, spec.spot, bump_size)?,
                FdKind::GammaCrn => finite_diff_gamma_crn(&inputs, spec.spot, bump_size)?,
            }
        }
        OptionType::Put => {
            let payoff = EuropeanPut::new(spec.strike, 1.0, num_steps);
            let inputs = FiniteDiffInputs {
                engine: &engine,
                rng: &rng,
                process: &gbm,
                disc: &disc,
                payoff: &payoff,
                currency,
                discount_factor,
            };
            match kind {
                FdKind::Delta => finite_diff_delta(&inputs, spec.spot, bump_size)?,
                FdKind::DeltaCrn => finite_diff_delta_crn(&inputs, spec.spot, bump_size)?,
                FdKind::Gamma => finite_diff_gamma(&inputs, spec.spot, bump_size)?,
                FdKind::GammaCrn => finite_diff_gamma_crn(&inputs, spec.spot, bump_size)?,
            }
        }
    };

    let half_width = CI_95_Z * stderr;
    Ok(Estimate::new(
        value,
        stderr,
        (value - half_width, value + half_width),
        num_paths,
    )
    .with_num_simulated_paths(if defaults.antithetic {
        2 * num_paths
    } else {
        num_paths
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atm_spec() -> GbmEuropeanFdSpec {
        GbmEuropeanFdSpec {
            spot: 100.0,
            strike: 100.0,
            rate: 0.05,
            div_yield: 0.0,
            vol: 0.2,
            expiry: 1.0,
            num_paths: Some(2_000),
            seed: Some(42),
            num_steps: Some(12),
            bump_size: None,
            option_type: OptionType::Call,
            currency: None,
        }
    }

    #[test]
    fn finite_diff_delta_gbm_atm_call_is_between_zero_and_one() {
        let est = finite_diff_delta_gbm(atm_spec()).expect("delta should succeed");
        assert!(est.mean > 0.0 && est.mean < 1.0, "delta={}", est.mean);
        assert!(est.stderr.is_finite() && est.stderr >= 0.0);
        assert_eq!(est.num_paths, 2_000);
        assert!(est.ci_95.0 <= est.mean && est.mean <= est.ci_95.1);
    }

    fn bs_spot_delta_gamma(
        spot: f64,
        strike: f64,
        rate: f64,
        div_yield: f64,
        vol: f64,
        expiry: f64,
        is_call: bool,
    ) -> (f64, f64) {
        let forward = spot * ((rate - div_yield) * expiry).exp();
        let df_q = (-div_yield * expiry).exp();
        let forward_delta = if is_call {
            crate::closed_form::black_delta_call(forward, strike, vol, expiry)
        } else {
            crate::closed_form::black_delta_put(forward, strike, vol, expiry)
        };
        let forward_gamma = crate::closed_form::black_gamma(forward, strike, vol, expiry);
        (
            df_q * forward_delta,
            df_q * forward_gamma * (forward / spot),
        )
    }

    fn crn_spec(spot: f64, strike: f64, option_type: OptionType) -> GbmEuropeanFdSpec {
        GbmEuropeanFdSpec {
            spot,
            strike,
            rate: 0.05,
            div_yield: 0.0,
            vol: 0.2,
            expiry: 1.0,
            num_paths: Some(8_000),
            seed: Some(7),
            num_steps: Some(1),
            bump_size: Some(0.01),
            option_type,
            currency: None,
        }
    }

    #[test]
    fn finite_diff_delta_crn_gbm_matches_black_scholes_atm_and_25d() {
        let (atm_spot, atm_strike) = (100.0, 100.0);
        let est = finite_diff_delta_crn_gbm(crn_spec(atm_spot, atm_strike, OptionType::Call))
            .expect("atm delta");
        let (bs_delta, _) = bs_spot_delta_gamma(atm_spot, atm_strike, 0.05, 0.0, 0.2, 1.0, true);
        let tol = (4.0 * est.stderr).max(0.03);
        assert!(
            (est.mean - bs_delta).abs() < tol,
            "ATM call delta {} vs BS {bs_delta} (stderr={}, tol={tol})",
            est.mean,
            est.stderr
        );

        let otm_strike = 120.0;
        let est_25 = finite_diff_delta_crn_gbm(crn_spec(atm_spot, otm_strike, OptionType::Call))
            .expect("otm delta");
        let (bs_delta_25, _) = bs_spot_delta_gamma(atm_spot, otm_strike, 0.05, 0.0, 0.2, 1.0, true);
        let tol_25 = (4.0 * est_25.stderr).max(0.03);
        assert!(
            (est_25.mean - bs_delta_25).abs() < tol_25,
            "OTM call delta {} vs BS {bs_delta_25} (stderr={}, tol={tol_25})",
            est_25.mean,
            est_25.stderr
        );
    }

    #[test]
    fn finite_diff_gamma_crn_gbm_matches_black_scholes_atm() {
        let spec = crn_spec(100.0, 100.0, OptionType::Call);
        let est = finite_diff_gamma_crn_gbm(spec).expect("atm gamma");
        let (_, bs_gamma) = bs_spot_delta_gamma(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, true);
        let tol = (4.0 * est.stderr).max(0.01);
        assert!(
            (est.mean - bs_gamma).abs() < tol,
            "ATM call gamma {} vs BS {bs_gamma} (stderr={}, tol={tol})",
            est.mean,
            est.stderr
        );
    }

    #[test]
    fn finite_diff_put_delta_is_negative() {
        let est =
            finite_diff_delta_crn_gbm(crn_spec(100.0, 100.0, OptionType::Put)).expect("put delta");
        assert!(est.mean < 0.0 && est.mean > -1.0, "put delta={}", est.mean);
    }

    #[test]
    fn finite_diff_delta_gbm_rejects_non_positive_vol() {
        let mut spec = atm_spec();
        spec.vol = 0.0;
        let err = finite_diff_delta_gbm(spec).expect_err("zero vol");
        assert!(
            err.to_string().contains("strictly positive volatility"),
            "unexpected error: {err}"
        );
    }
}
