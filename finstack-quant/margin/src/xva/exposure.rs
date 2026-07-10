//! Exposure simulation engine for XVA calculations.
//!
//! Computes exposure profiles (EPE, ENE, PFE) for a portfolio of instruments
//! by re-valuing them at future time points.
//!
//! # Methodology
//!
//! This module implements a **deterministic exposure** approach:
//! at each future time point, instruments are re-valued under the current
//! market data (curves rolled forward deterministically). This is a simplified
//! but conservative approach suitable for:
//!
//! - Initial XVA framework validation
//! - Portfolios with linear instruments (bonds, swaps)
//! - Regulatory SA-CCR style calculations
//!
//! For a full production implementation, Monte Carlo simulation of risk factors
//! would replace the deterministic forward roll. The API is designed to be
//! extended without breaking changes.
//!
//! # Exposure Definitions
//!
//! ```text
//! V(t)   = portfolio mark-to-market at time t
//! EPE(t) = E[max(V(t), 0)]     — Expected Positive Exposure
//! ENE(t) = E[max(-V(t), 0)]    — Expected Negative Exposure
//! PFE(t) = quantile(V(t), α)   — Potential Future Exposure at level α
//! ```
//!
//! # References
//!
//! - Gregory XVA Challenge: `docs/REFERENCES.md#gregory-xva-challenge`
//! - BCBS 279 SA-CCR: `docs/REFERENCES.md#bcbs-279-saccr`

use std::sync::Arc;

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, CALENDAR_DAYS_PER_YEAR};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::neumaier_sum;
use finstack_quant_core::money::fx::FxConversionPolicy;
use finstack_quant_core::money::fx::FxQuery;
use finstack_quant_core::money::Money;

use super::traits::Valuable;
#[cfg(test)]
use finstack_quant_monte_carlo::rng::philox::PhiloxRng;
#[cfg(test)]
use finstack_quant_monte_carlo::{
    state_keys, Discretization, PathState, RandomStream, StochasticProcess,
};

use super::netting::{apply_collateral, apply_netting};
use super::types::{ExposureProfile, XvaConfig, XvaNettingSet};
#[cfg(test)]
use super::types::{StochasticExposureConfig, StochasticExposureProfile};

/// Map a year fraction to a whole-day offset using ACT/365F-style scaling.
///
/// Uses **half-up** rounding to the nearest calendar day. This avoids IEEE
/// "ties to even" surprises from [`f64::round`] (e.g. 182.5 days rounding to 182).
#[inline]
fn years_to_days_act_365f(years: f64) -> i64 {
    let raw = years * CALENDAR_DAYS_PER_YEAR;
    if !raw.is_finite() {
        return 0;
    }
    if raw >= 0.0 {
        (raw + 0.5).floor() as i64
    } else {
        (raw - 0.5).ceil() as i64
    }
}

fn resolve_reporting_currency(
    instruments: &[Arc<dyn Valuable>],
    market: &MarketContext,
    as_of: Date,
    netting_set: &XvaNettingSet,
) -> finstack_quant_core::Result<Currency> {
    if let Some(currency) = netting_set.reporting_currency {
        return Ok(currency);
    }

    let mut observed: Option<Currency> = None;
    for inst in instruments {
        let currency = inst.value(market, as_of)?.currency();
        match observed {
            None => observed = Some(currency),
            Some(existing) if existing == currency => {}
            Some(_) => {
                return Err(finstack_quant_core::Error::Validation(
                    "XVA exposure requires an explicit reporting currency for mixed-currency portfolios"
                        .to_string(),
                ))
            }
        }
    }

    observed.ok_or_else(|| {
        finstack_quant_core::Error::Validation(
            "XVA exposure requires at least one instrument to infer reporting currency".to_string(),
        )
    })
}

fn convert_to_reporting(
    value: Money,
    reporting_currency: Currency,
    market: &MarketContext,
    on: Date,
) -> finstack_quant_core::Result<f64> {
    if value.currency() == reporting_currency {
        return Ok(value.amount());
    }

    let fx = market.fx().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "XVA exposure requires FX data to convert {} into reporting currency {}",
            value.currency(),
            reporting_currency
        ))
    })?;
    let rate = fx
        .rate(FxQuery::with_policy(
            value.currency(),
            reporting_currency,
            on,
            FxConversionPolicy::CashflowDate,
        ))?
        .rate;
    Ok(value.amount() * rate)
}

#[cfg(test)]
fn interpolate_quantile(samples: &mut [f64], quantile: f64) -> f64 {
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if samples.len() == 1 {
        return samples[0];
    }

    let scaled = quantile * (samples.len() - 1) as f64;
    let lo = scaled.floor() as usize;
    let hi = scaled.ceil() as usize;
    if lo == hi {
        samples[lo]
    } else {
        let w = scaled - lo as f64;
        samples[lo] * (1.0 - w) + samples[hi] * w
    }
}

/// Compute the exposure profile for a portfolio of instruments.
///
/// For each time point in the configuration's time grid, this function:
/// 1. Rolls the market data forward to `as_of + t` (deterministic roll)
/// 2. Re-values each instrument at the future date
/// 3. Applies close-out netting across the netting set
/// 4. Applies CSA collateral terms (if present)
/// 5. Records EPE and ENE values
///
/// # Arguments
///
/// * `instruments` - Portfolio of instruments in this netting set
/// * `market` - Current market data context
/// * `as_of` - Valuation date (T+0)
/// * `config` - XVA configuration (time grid, recovery, etc.)
/// * `netting_set` - Netting set specification with optional CSA
///
/// # Returns
///
/// An [`ExposureProfile`] containing MtM, EPE, and ENE at each time point.
///
/// # Errors
///
/// Returns an error if:
/// - Configuration validation fails
/// - More than 50% of time grid points fail (market roll or valuation)
///
/// # Warnings
///
/// Time points where market data cannot be rolled forward are recorded
/// as zero exposure with a log warning. Instruments that fail to value
/// at a given horizon are treated as zero value (matured/settled).
///
/// # Limitations
///
/// - Uses deterministic (single-scenario) exposure; no Monte Carlo
/// - PFE equals EPE in this simplified model
/// - Does not model margin period of risk (MPOR) explicitly
/// - Curve roll uses constant-curves assumption (no carry/theta)
///
/// # References
///
/// - Gregory XVA Challenge: `docs/REFERENCES.md#gregory-xva-challenge`
/// - BCBS 279 SA-CCR: `docs/REFERENCES.md#bcbs-279-saccr`
#[tracing::instrument(skip(instruments, market), fields(grid_points = config.time_grid.len()))]
pub fn compute_exposure_profile(
    instruments: &[Arc<dyn Valuable>],
    market: &MarketContext,
    as_of: Date,
    config: &XvaConfig,
    netting_set: &XvaNettingSet,
) -> finstack_quant_core::Result<ExposureProfile> {
    config.validate()?;
    let reporting_currency = resolve_reporting_currency(instruments, market, as_of, netting_set)?;

    let n = config.time_grid.len();
    let mut times = Vec::with_capacity(n);
    let mut mtm_values = Vec::with_capacity(n);
    let mut epe = Vec::with_capacity(n);
    let mut ene = Vec::with_capacity(n);

    for &t in &config.time_grid {
        // Convert years to days using ACT/365F convention
        let days = years_to_days_act_365f(t);
        let future_date = as_of + time::Duration::days(days);

        // Roll market data forward (constant-curves assumption).
        let rolled_market = market.roll_forward(days).map_err(|error| {
            finstack_quant_core::Error::Validation(format!(
                "XVA market roll failed at horizon {t} years: {error}"
            ))
        })?;

        // Value each instrument at the future date
        let mut values = Vec::with_capacity(instruments.len());
        for inst in instruments {
            let value = inst
                .value(&rolled_market, future_date)
                .and_then(|value| {
                    convert_to_reporting(value, reporting_currency, &rolled_market, future_date)
                })
                .map_err(|error| {
                    finstack_quant_core::Error::Validation(format!(
                        "XVA valuation failed for instrument '{}' at horizon {t} years: {error}",
                        inst.id()
                    ))
                })?;
            values.push(value);
        }

        // Apply close-out netting: net portfolio value
        let net_value: f64 = neumaier_sum(values.iter().copied());
        let net_positive_exposure = apply_netting(&values);
        let net_negative_exposure = (-net_value).max(0.0);

        let (positive_exposure, negative_exposure) = if let Some(ref csa) = netting_set.csa {
            (
                apply_collateral(net_positive_exposure, csa),
                // `independent_amount` is documented as collateral posted by
                // the counterparty. It reduces EPE, but cannot also represent
                // independent collateral posted by the bank for ENE/DVA.
                super::netting::apply_variation_margin(net_negative_exposure, csa),
            )
        } else {
            (net_positive_exposure, net_negative_exposure)
        };

        times.push(t);
        mtm_values.push(net_value);
        epe.push(positive_exposure);
        ene.push(negative_exposure);
    }

    Ok(ExposureProfile {
        times,
        mtm_values,
        epe,
        ene,
        diagnostics: None,
    })
}

/// Compute a stochastic exposure profile using the Monte Carlo primitives.
///
/// This engine simulates factor paths and revalues the portfolio through a
/// pathwise callback at each time bucket. It keeps the current deterministic
/// exposure API intact while providing a reusable route to genuine exposure
/// distributions and quantile-based PFE.
///
/// # Arguments
///
/// * `process` - Stochastic process that evolves the factor state
/// * `discretization` - Time-stepping scheme used to advance `process`
/// * `initial_state` - Initial factor state vector; length must equal `process.dim()`
/// * `xva_config` - Exposure time grid expressed as year fractions
/// * `stochastic_config` - Monte Carlo path count, RNG seed, and PFE quantile
/// * `valuation_fn` - Callback that converts a simulated [`PathState`] into a
///   signed portfolio MtM in reporting-currency units
///
/// # Returns
///
/// A [`StochasticExposureProfile`] containing path-average MtM/EPE/ENE and a
/// quantile-based positive-exposure profile.
///
/// # Errors
///
/// Returns an error if:
/// - `xva_config` or `stochastic_config` fails validation
/// - `initial_state` has the wrong dimension
/// - `valuation_fn` fails for any simulated path/time step
/// - the aggregated profile fails internal validation
///
/// # Example
///
/// ```ignore
/// use finstack_quant_margin::xva::types::{StochasticExposureConfig, XvaConfig};
/// use finstack_quant_margin::xva::types::{StochasticExposureConfig, XvaConfig};
///
/// #
/// # fn example<P, D>(process: &P, discretization: &D) -> finstack_quant_core::Result<()>
/// # where
/// #     P: finstack_quant_monte_carlo::StochasticProcess,
/// #     D: finstack_quant_monte_carlo::Discretization<P>,
/// # {
/// let xva_config = XvaConfig {
///     time_grid: vec![0.25, 0.5, 1.0],
///     ..XvaConfig::default()
/// };
/// let mc_config = StochasticExposureConfig::default();
/// let initial_state = vec![0.0; process.dim()];
///
/// let profile = compute_stochastic_exposure_profile(
///     process,
///     discretization,
///     &initial_state,
///     &xva_config,
///     &mc_config,
///     |_path_state| Ok(0.0),
/// )?;
/// # let _ = profile;
/// # Ok(())
/// # }
/// ```
///
/// # Limitations
///
/// - Collateral and netting must be represented inside `valuation_fn` or in the
///   factor-to-value mapping around it; this helper only simulates pathwise MtM.
/// - Time points are taken directly from `xva_config.time_grid` and are assumed
///   to be year fractions.
///
/// # References
///
/// - Gregory XVA Challenge: `docs/REFERENCES.md#gregory-xva-challenge`
/// - BCBS 279 SA-CCR: `docs/REFERENCES.md#bcbs-279-saccr`
#[cfg(test)]
pub(crate) fn compute_stochastic_exposure_profile<P, D, V>(
    process: &P,
    discretization: &D,
    initial_state: &[f64],
    xva_config: &XvaConfig,
    stochastic_config: &StochasticExposureConfig,
    valuation_fn: V,
) -> finstack_quant_core::Result<StochasticExposureProfile>
where
    P: StochasticProcess,
    D: Discretization<P>,
    V: Fn(&PathState) -> finstack_quant_core::Result<f64>,
{
    xva_config.validate()?;
    stochastic_config.validate()?;

    if initial_state.len() != process.dim() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Stochastic exposure: initial_state length {} must match process dim {}",
            initial_state.len(),
            process.dim()
        )));
    }

    let time_count = xva_config.time_grid.len();
    let num_paths = stochastic_config.num_paths;
    // Single flat *time-major* buffer indexed as
    // `[step_idx * num_paths + path_idx]`. One heap allocation
    // (vs. `time_count` for the old `Vec<Vec<f64>>`) and aggregation
    // walks each row contiguously, matching the access pattern of the
    // post-loop mean/quantile reductions. Simulation writes are
    // strided by `num_paths`, but the per-step valuation closure
    // dominates wall-time and prefetchers handle a stride this large
    // well; quantile + sum reductions are the cache-sensitive phase.
    let total_cells = time_count.checked_mul(num_paths).ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "Stochastic exposure: time_count ({time_count}) * num_paths ({num_paths}) overflows usize"
        ))
    })?;
    let mut pathwise_mtms = vec![0.0f64; total_cells];
    let base_rng = PhiloxRng::new(stochastic_config.seed);

    let mut state_vector = vec![0.0; process.dim()];
    let mut shocks = vec![0.0; process.num_factors()];
    let mut work = vec![0.0; discretization.work_size(process)];

    for path_idx in 0..num_paths {
        let mut rng = base_rng.substream((path_idx + 1) as u64);
        state_vector.copy_from_slice(initial_state);
        let mut prev_t = 0.0;

        for (step_idx, &t) in xva_config.time_grid.iter().enumerate() {
            let dt = t - prev_t;
            rng.fill_std_normals(&mut shocks);
            discretization.step(process, prev_t, dt, &mut state_vector, &shocks, &mut work);

            let mut path_state = PathState::new(step_idx + 1, t);
            path_state.set(state_keys::TIME, t);
            path_state.set(state_keys::STEP, (step_idx + 1) as f64);
            process.populate_path_state(&state_vector, &mut path_state);

            let mtm = valuation_fn(&path_state)?;
            pathwise_mtms[step_idx * num_paths + path_idx] = mtm;
            prev_t = t;
        }
    }

    let mut mtm_values = Vec::with_capacity(time_count);
    let mut epe = Vec::with_capacity(time_count);
    let mut ene = Vec::with_capacity(time_count);
    let mut pfe_profile = Vec::with_capacity(time_count);
    let mut positive_buf = Vec::with_capacity(num_paths);
    let path_count_f = num_paths as f64;

    for step_idx in 0..time_count {
        positive_buf.clear();
        let mut sum_mtm = 0.0;
        let mut sum_pos = 0.0;
        let mut sum_neg = 0.0;
        let row_start = step_idx * num_paths;
        let row = &pathwise_mtms[row_start..row_start + num_paths];
        for &mtm in row {
            sum_mtm += mtm;
            let pos = mtm.max(0.0);
            sum_pos += pos;
            sum_neg += (-mtm).max(0.0);
            positive_buf.push(pos);
        }
        mtm_values.push(sum_mtm / path_count_f);
        epe.push(sum_pos / path_count_f);
        ene.push(sum_neg / path_count_f);
        pfe_profile.push(interpolate_quantile(
            &mut positive_buf,
            stochastic_config.pfe_quantile,
        ));
    }

    let profile = ExposureProfile {
        times: xva_config.time_grid.clone(),
        mtm_values,
        epe,
        ene,
        diagnostics: None,
    };
    profile.validate()?;

    let stochastic_profile = StochasticExposureProfile {
        profile,
        pfe_profile,
        path_count: stochastic_config.num_paths,
        pfe_quantile: stochastic_config.pfe_quantile,
    };
    stochastic_profile.validate()?;
    Ok(stochastic_profile)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xva::cva::compute_cva;
    use crate::xva::types::CsaTerms;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
    use finstack_quant_core::money::Money;
    use std::sync::Arc;
    use time::Month;

    // Note: Full integration tests require constructing instrument and market mocks.
    // These unit tests verify the exposure profile logic with synthetic data.

    #[derive(Clone, Debug)]
    struct StaticInstrument {
        id: String,
        pv: f64,
    }

    impl StaticInstrument {
        fn new(id: &str, pv: f64) -> Self {
            Self {
                id: id.to_string(),
                pv,
            }
        }
    }

    impl Valuable for StaticInstrument {
        fn id(&self) -> &str {
            &self.id
        }

        fn value(
            &self,
            _market: &MarketContext,
            _as_of: Date,
        ) -> finstack_quant_core::Result<Money> {
            Ok(Money::new(self.pv, Currency::USD))
        }
    }

    #[derive(Clone, Debug)]
    struct MultiCurrencyStaticInstrument {
        id: String,
        pv: Money,
    }

    impl MultiCurrencyStaticInstrument {
        fn new(id: &str, amount: f64, currency: Currency) -> Self {
            Self {
                id: id.to_string(),
                pv: Money::new(amount, currency),
            }
        }
    }

    impl Valuable for MultiCurrencyStaticInstrument {
        fn id(&self) -> &str {
            &self.id
        }

        fn value(
            &self,
            _market: &MarketContext,
            _as_of: Date,
        ) -> finstack_quant_core::Result<Money> {
            Ok(self.pv)
        }
    }

    #[test]
    fn exposure_profile_basic_structure() {
        let config = XvaConfig {
            time_grid: vec![0.25, 0.5, 1.0],
            recovery_rate: 0.40,
            own_recovery_rate: None,
            funding: None,
        };
        config.validate().expect("Config should be valid");
        assert_eq!(config.time_grid.len(), 3);
    }

    #[test]
    fn years_to_days_act_365f_half_up_midpoint() {
        // 0.5 × 365 = 182.5 days → nearest whole day is 183 (half-up), not 182 (IEEE tie-to-even).
        assert_eq!(years_to_days_act_365f(0.5), 183);
    }

    #[test]
    fn exposure_profile_net_mtm_stable_summation() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
        let instruments: Vec<Arc<dyn Valuable>> = vec![
            Arc::new(StaticInstrument::new("BIG", 1e16)),
            Arc::new(StaticInstrument::new("ONE", 1.0)),
            Arc::new(StaticInstrument::new("BIGNEG", -1e16)),
        ];
        let market = MarketContext::new();
        let config = XvaConfig {
            time_grid: vec![0.25],
            recovery_rate: 0.40,
            own_recovery_rate: None,
            funding: None,
        };
        let netting_set = XvaNettingSet {
            id: "NS-STABLE-SUM".into(),
            counterparty_id: "CP".into(),
            csa: None,
            reporting_currency: None,
        };

        let profile = compute_exposure_profile(&instruments, &market, as_of, &config, &netting_set)
            .expect("profile should compute");

        assert!(
            (profile.mtm_values[0] - 1.0).abs() < 1e-10,
            "expected net MtM ≈ 1, got {}",
            profile.mtm_values[0]
        );
    }

    #[test]
    fn exposure_profile_supports_valuable_trait_objects() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
        let instruments: Vec<Arc<dyn Valuable>> =
            vec![Arc::new(StaticInstrument::new("USD-PV", 100.0))];
        let market = MarketContext::new();
        let config = XvaConfig {
            time_grid: vec![0.25],
            recovery_rate: 0.40,
            own_recovery_rate: None,
            funding: None,
        };
        let netting_set = XvaNettingSet {
            id: "NS-VALUABLE".into(),
            counterparty_id: "CP".into(),
            csa: None,
            reporting_currency: None,
        };

        let profile = compute_exposure_profile(&instruments, &market, as_of, &config, &netting_set)
            .expect("profile should compute");

        assert_eq!(profile.mtm_values, vec![100.0]);
        assert_eq!(profile.epe, vec![100.0]);
        assert_eq!(profile.ene, vec![0.0]);
    }

    #[test]
    fn exposure_profile_epe_non_negative() {
        // EPE by construction is max(V, 0) which is always >= 0
        let profile = ExposureProfile {
            times: vec![0.25, 0.5, 1.0],
            mtm_values: vec![100.0, -50.0, 25.0],
            epe: vec![100.0, 0.0, 25.0],
            ene: vec![0.0, 50.0, 0.0],
            diagnostics: None,
        };
        for &e in &profile.epe {
            assert!(e >= 0.0, "EPE must be non-negative, got {e}");
        }
    }

    #[test]
    fn exposure_profile_ene_non_negative() {
        let profile = ExposureProfile {
            times: vec![0.25, 0.5],
            mtm_values: vec![100.0, -50.0],
            epe: vec![100.0, 0.0],
            ene: vec![0.0, 50.0],
            diagnostics: None,
        };
        for &e in &profile.ene {
            assert!(e >= 0.0, "ENE must be non-negative, got {e}");
        }
    }

    // ── Integration tests: synthetic profiles through CVA pipeline ──

    /// Helper: build a flat hazard rate curve.
    fn flat_hazard_curve(lambda: f64) -> HazardCurve {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
        HazardCurve::builder("COUNTERPARTY")
            .base_date(base)
            .knots([(0.0, lambda), (30.0, lambda)])
            .build()
            .expect("HazardCurve should build")
    }

    /// Helper: build a flat discount curve.
    fn flat_discount_curve(rate: f64) -> DiscountCurve {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
        let knots: Vec<(f64, f64)> = (0..=60)
            .map(|i| {
                let t = i as f64 * 0.5;
                (t, (-rate * t).exp())
            })
            .collect();
        DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .knots(knots)
            .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
            .build()
            .expect("DiscountCurve should build")
    }

    #[test]
    fn collateral_reduces_cva_vs_uncollateralized() {
        // A CSA with zero threshold should reduce CVA compared to uncollateralized
        let hazard = flat_hazard_curve(0.02);
        let discount = flat_discount_curve(0.03);
        let times: Vec<f64> = (1..=20).map(|i| i as f64 * 0.5).collect();

        // Uncollateralized profile
        let uncollat_profile = ExposureProfile {
            times: times.clone(),
            mtm_values: times.iter().map(|_| 1_000_000.0).collect(),
            epe: times.iter().map(|_| 1_000_000.0).collect(),
            ene: times.iter().map(|_| 0.0).collect(),
            diagnostics: None,
        };

        // Collateralized profile: apply CSA to reduce EPE
        let csa = CsaTerms {
            threshold: 0.0,
            mta: 500.0,
            mpor_days: 10,
            independent_amount: 0.0,
        };
        let collat_epe: Vec<f64> = times
            .iter()
            .map(|_| apply_collateral(1_000_000.0, &csa))
            .collect();
        let collat_profile = ExposureProfile {
            times: times.clone(),
            mtm_values: times.iter().map(|_| 1_000_000.0).collect(),
            epe: collat_epe,
            ene: times.iter().map(|_| 0.0).collect(),
            diagnostics: None,
        };

        let cva_uncollat = compute_cva(&uncollat_profile, &hazard, &discount, 0.40)
            .expect("should work")
            .cva;
        let cva_collat = compute_cva(&collat_profile, &hazard, &discount, 0.40)
            .expect("should work")
            .cva;

        assert!(
            cva_collat < cva_uncollat,
            "Collateralized CVA ({cva_collat:.2}) should be less than uncollateralized ({cva_uncollat:.2})"
        );
    }

    #[test]
    fn netting_reduces_cva_vs_gross() {
        // Netting offsetting trades should produce lower CVA
        let hazard = flat_hazard_curve(0.02);
        let discount = flat_discount_curve(0.03);
        let times: Vec<f64> = (1..=20).map(|i| i as f64 * 0.5).collect();

        // Gross: treat each trade individually (sum of positive exposures)
        let trade_a: f64 = 1_000_000.0;
        let trade_b: f64 = -800_000.0;
        let gross_epe: Vec<f64> = times.iter().map(|_| trade_a.max(0.0)).collect();
        let gross_profile = ExposureProfile {
            times: times.clone(),
            mtm_values: times.iter().map(|_| trade_a).collect(),
            epe: gross_epe,
            ene: times.iter().map(|_| 0.0).collect(),
            diagnostics: None,
        };

        // Netted: use netting to compute net exposure
        let net_epe: Vec<f64> = times
            .iter()
            .map(|_| apply_netting(&[trade_a, trade_b]))
            .collect();
        let net_profile = ExposureProfile {
            times: times.clone(),
            mtm_values: times.iter().map(|_| trade_a + trade_b).collect(),
            epe: net_epe,
            ene: times
                .iter()
                .map(|_| (-(trade_a + trade_b)).max(0.0))
                .collect(),
            diagnostics: None,
        };

        let cva_gross = compute_cva(&gross_profile, &hazard, &discount, 0.40)
            .expect("should work")
            .cva;
        let cva_net = compute_cva(&net_profile, &hazard, &discount, 0.40)
            .expect("should work")
            .cva;

        assert!(
            cva_net < cva_gross,
            "Netted CVA ({cva_net:.2}) should be less than gross CVA ({cva_gross:.2})"
        );
    }

    #[test]
    fn zero_value_portfolio_gives_zero_cva() {
        let hazard = flat_hazard_curve(0.02);
        let discount = flat_discount_curve(0.03);
        let times: Vec<f64> = (1..=10).map(|i| i as f64).collect();

        let profile = ExposureProfile {
            times: times.clone(),
            mtm_values: vec![0.0; times.len()],
            epe: vec![0.0; times.len()],
            ene: vec![0.0; times.len()],
            diagnostics: None,
        };

        let result = compute_cva(&profile, &hazard, &discount, 0.40)
            .expect("CVA should compute for zero portfolio");
        assert!(
            result.cva.abs() < 1e-12,
            "CVA for zero-value portfolio should be zero, got {}",
            result.cva
        );
    }

    #[test]
    fn single_instrument_profile() {
        // Single instrument with declining exposure (e.g., amortizing swap)
        let hazard = flat_hazard_curve(0.02);
        let discount = flat_discount_curve(0.03);
        let times: Vec<f64> = (1..=10).map(|i| i as f64).collect();

        let epe: Vec<f64> = times
            .iter()
            .map(|&t| 1_000_000.0 * (1.0 - t / 10.0))
            .collect();
        let profile = ExposureProfile {
            times: times.clone(),
            mtm_values: epe.clone(),
            epe,
            ene: vec![0.0; times.len()],
            diagnostics: None,
        };

        let result = compute_cva(&profile, &hazard, &discount, 0.40)
            .expect("CVA should compute for declining profile");

        assert!(result.cva > 0.0, "CVA should be positive");

        // Effective EPE profile should be non-decreasing
        for i in 1..result.effective_epe_profile.len() {
            assert!(
                result.effective_epe_profile[i].1 >= result.effective_epe_profile[i - 1].1 - 1e-12,
                "Effective EPE profile must be non-decreasing"
            );
        }

        // Validate the profile
        profile.validate().expect("Profile should be valid");
    }

    #[test]
    fn exposure_profile_validates_after_construction() {
        let times = vec![0.25, 0.5, 1.0, 2.0, 5.0];
        let profile = ExposureProfile {
            times,
            mtm_values: vec![100.0, -50.0, 25.0, 75.0, -10.0],
            epe: vec![100.0, 0.0, 25.0, 75.0, 0.0],
            ene: vec![0.0, 50.0, 0.0, 0.0, 10.0],
            diagnostics: None,
        };
        profile
            .validate()
            .expect("Manually constructed valid profile should pass validation");
    }

    #[test]
    fn collateral_reduces_ene_for_negative_net_mtm() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
        let instruments: Vec<Arc<dyn Valuable>> =
            vec![Arc::new(StaticInstrument::new("NEGATIVE-PV", -1_000_000.0))];
        let market = MarketContext::new();
        let config = XvaConfig {
            time_grid: vec![0.25],
            recovery_rate: 0.40,
            own_recovery_rate: None,
            funding: None,
        };
        let csa = CsaTerms {
            threshold: 0.0,
            mta: 500.0,
            mpor_days: 10,
            independent_amount: 0.0,
        };
        let netting_set = XvaNettingSet {
            id: "CSA-NEG".into(),
            counterparty_id: "CP".into(),
            csa: Some(csa.clone()),
            reporting_currency: None,
        };

        let profile = compute_exposure_profile(&instruments, &market, as_of, &config, &netting_set)
            .expect("profile should compute");

        let expected_ene = apply_collateral(1_000_000.0, &csa);
        assert!(
            (profile.ene[0] - expected_ene).abs() < 1e-12,
            "CSA should reduce negative exposure symmetrically: got {}, expected {}",
            profile.ene[0],
            expected_ene
        );
    }

    #[test]
    fn mixed_currency_profile_requires_explicit_reporting_currency() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
        let instruments: Vec<Arc<dyn Valuable>> = vec![
            Arc::new(MultiCurrencyStaticInstrument::new(
                "USD-PV",
                100.0,
                Currency::USD,
            )),
            Arc::new(MultiCurrencyStaticInstrument::new(
                "EUR-PV",
                100.0,
                Currency::EUR,
            )),
        ];

        let provider = {
            let p = SimpleFxProvider::new();
            p.set_quote(Currency::EUR, Currency::USD, 2.0)
                .expect("valid rate");
            p
        };
        let fx = FxMatrix::new(Arc::new(provider));

        let market = MarketContext::new().insert_fx(fx);
        let config = XvaConfig {
            time_grid: vec![0.25],
            recovery_rate: 0.40,
            own_recovery_rate: None,
            funding: None,
        };
        let netting_set = XvaNettingSet {
            id: "MIXED-CCY".into(),
            counterparty_id: "CP".into(),
            csa: None,
            reporting_currency: None,
        };

        let err = compute_exposure_profile(&instruments, &market, as_of, &config, &netting_set)
            .expect_err(
            "mixed-currency portfolios must not aggregate without an explicit reporting currency",
        );
        assert!(
            err.to_string().contains("reporting currency"),
            "expected reporting currency validation error, got: {err}"
        );
    }

    #[test]
    fn mixed_currency_profile_converts_into_reporting_currency() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
        let instruments: Vec<Arc<dyn Valuable>> = vec![
            Arc::new(MultiCurrencyStaticInstrument::new(
                "USD-PV",
                100.0,
                Currency::USD,
            )),
            Arc::new(MultiCurrencyStaticInstrument::new(
                "EUR-PV",
                100.0,
                Currency::EUR,
            )),
        ];

        let provider = {
            let p = SimpleFxProvider::new();
            p.set_quote(Currency::EUR, Currency::USD, 2.0)
                .expect("valid rate");
            p
        };
        let fx = FxMatrix::new(Arc::new(provider));

        let market = MarketContext::new().insert_fx(fx);
        let config = XvaConfig {
            time_grid: vec![0.25],
            recovery_rate: 0.40,
            own_recovery_rate: None,
            funding: None,
        };
        let netting_set = XvaNettingSet {
            id: "MIXED-CCY".into(),
            counterparty_id: "CP".into(),
            csa: None,
            reporting_currency: Some(Currency::USD),
        };

        let profile = compute_exposure_profile(&instruments, &market, as_of, &config, &netting_set)
            .expect("mixed-currency profile should compute with explicit reporting currency");
        assert!((profile.mtm_values[0] - 300.0).abs() < 1e-12);
        assert!((profile.epe[0] - 300.0).abs() < 1e-12);
    }

    #[test]
    fn stochastic_exposure_profile_uses_quantile_based_pfe() {
        use crate::xva::types::StochasticExposureConfig;
        use finstack_quant_monte_carlo::prelude::{ExactGbm, GbmProcess};

        let process = GbmProcess::with_params(0.0, 0.0, 0.25).expect("valid GBM params");
        let discretization = ExactGbm::new();
        let xva_config = XvaConfig {
            time_grid: vec![0.5, 1.0],
            recovery_rate: 0.40,
            own_recovery_rate: None,
            funding: None,
        };
        let stochastic = StochasticExposureConfig {
            num_paths: 1_024,
            seed: 7,
            pfe_quantile: 0.975,
        };

        let profile = compute_stochastic_exposure_profile(
            &process,
            &discretization,
            &[100.0],
            &xva_config,
            &stochastic,
            |state| Ok(state.spot().unwrap_or(0.0) - 100.0),
        )
        .expect("stochastic profile should compute");

        assert_eq!(profile.profile.times.len(), 2);
        assert_eq!(profile.pfe_profile.len(), 2);
        assert!(
            profile.pfe_profile[0] > profile.profile.epe[0],
            "PFE should exceed EPE for a non-degenerate positive-tail distribution"
        );
    }

    #[test]
    fn stochastic_exposure_profile_collapses_to_deterministic_when_paths_are_identical() {
        use crate::xva::types::StochasticExposureConfig;
        use finstack_quant_monte_carlo::prelude::{ExactGbm, GbmProcess};

        let process = GbmProcess::with_params(0.0, 0.0, 0.0).expect("valid GBM params");
        let discretization = ExactGbm::new();
        let xva_config = XvaConfig {
            time_grid: vec![0.25, 0.5, 1.0],
            recovery_rate: 0.40,
            own_recovery_rate: None,
            funding: None,
        };
        let stochastic = StochasticExposureConfig {
            num_paths: 128,
            seed: 11,
            pfe_quantile: 0.975,
        };

        let profile = compute_stochastic_exposure_profile(
            &process,
            &discretization,
            &[110.0],
            &xva_config,
            &stochastic,
            |state| Ok(state.spot().unwrap_or(0.0) - 100.0),
        )
        .expect("stochastic profile should compute");

        assert!(profile
            .profile
            .epe
            .iter()
            .zip(profile.pfe_profile.iter())
            .all(|(epe, pfe)| (*epe - *pfe).abs() < 1e-12));
        assert!(profile
            .profile
            .mtm_values
            .iter()
            .all(|mtm| (*mtm - 10.0).abs() < 1e-12));
    }
}
