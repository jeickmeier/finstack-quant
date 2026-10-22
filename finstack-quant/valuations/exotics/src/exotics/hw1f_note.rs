//! Shared Hull-White 1F Monte Carlo pricer for the structured rate notes.
//!
//! TARNs, snowballs and callable range accruals price off the same HW1F
//! harness: HW1F parameters resolved from the instrument's `model_config`
//! (or an explicit override), a base [`Hw1fMcConfig`] with per-instrument
//! path/seed overrides, and an MC estimate published with its diagnostics.
//! [`Hw1fNotePricer`] owns that shell once; each note supplies its payoff
//! simulation through [`Hw1fNote`].

use std::marker::PhantomData;

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_core::Result;
use finstack_quant_models::monte_carlo::results::MoneyEstimate;
use finstack_quant_models::rates::hull_white::HullWhiteCalibrationParams;
use finstack_quant_models::rates::hw1f::Hw1fMcConfig;
use finstack_quant_valuations_core::instruments::pricing::hw1f_params::{
    resolve_hw1f_params, Hw1fParamFamily,
};
use finstack_quant_valuations_core::instruments::pricing_overrides::ModelConfig;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::pricer::{
    expect_inst, InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use finstack_quant_valuations_core::results::ValuationResult;

/// A structured rate note priced by [`Hw1fNotePricer`].
pub trait Hw1fNote: Instrument + Sized + 'static {
    /// Instrument type the pricer is registered under.
    const INSTRUMENT_TYPE: InstrumentType;

    /// Note name used in HW1F parameter-resolution errors (`"TARN"`).
    const LABEL: &'static str;

    /// Instrument identifier, used for per-instrument MC seed overrides.
    fn note_id(&self) -> &InstrumentId;

    /// Discount curve whose calibrated HW1F parameters the note uses.
    fn hw1f_curve_id(&self) -> &str;

    /// Instrument-owned model settings (`hw1f_*`, `mc_paths`, seed scenario).
    fn model_config(&self) -> &ModelConfig;

    /// Curve ids attached to a pricing failure's error context.
    fn error_curve_ids(&self) -> Vec<String>;

    /// Simulate the note and return its Monte Carlo PV estimate.
    ///
    /// # Arguments
    ///
    /// * `pricer` - Pricer supplying the effective HW1F parameters and MC
    ///   configuration.
    /// * `market` - Market data holding the note's curves.
    /// * `as_of` - Valuation date.
    ///
    /// # Errors
    ///
    /// Returns an error when the note is invalid, a curve is missing, or the
    /// HW1F parameters cannot be resolved.
    fn hw1f_estimate(
        &self,
        pricer: &Hw1fNotePricer<Self>,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<MoneyEstimate>;
}

/// Hull-White 1F Monte Carlo pricer for a structured rate note `N`.
///
/// Registered under `(N::INSTRUMENT_TYPE, ModelKey::MonteCarloHullWhite1F)`.
/// The default pricer takes HW1F parameters from the instrument's
/// `model_config` or the market's calibrated parameters; instrument MC
/// overrides apply on top of the base configuration.
#[derive(Debug, Clone)]
pub struct Hw1fNotePricer<N> {
    hw_params: Option<HullWhiteCalibrationParams>,
    config: Hw1fMcConfig,
    _note: PhantomData<fn() -> N>,
}

impl<N> Default for Hw1fNotePricer<N> {
    fn default() -> Self {
        Self {
            hw_params: None,
            config: Hw1fMcConfig::default(),
            _note: PhantomData,
        }
    }
}

impl<N: Hw1fNote> Hw1fNotePricer<N> {
    /// Create a pricer with explicit HW1F parameters.
    ///
    /// # Arguments
    ///
    /// * `hw_params` - Mean reversion and volatility used when the instrument's
    ///   `model_config` does not set them.
    pub fn with_hw_params(hw_params: HullWhiteCalibrationParams) -> Self {
        Self {
            hw_params: Some(hw_params),
            ..Self::default()
        }
    }

    /// Set the Monte Carlo configuration (paths, seed, antithetic flag, time
    /// steps) used by this pricer; instrument overrides still apply on top.
    ///
    /// # Arguments
    ///
    /// * `config` - Base Hull-White Monte Carlo configuration.
    pub fn with_config(mut self, config: Hw1fMcConfig) -> Self {
        self.config = config;
        self
    }

    /// HW1F parameters for `inst`: its `model_config`, else this pricer's
    /// explicit parameters, else the market's calibrated cap/floor set.
    pub(crate) fn effective_hw_params(
        &self,
        inst: &N,
        market: &MarketContext,
    ) -> Result<HullWhiteCalibrationParams> {
        resolve_hw1f_params(
            Hw1fParamFamily::CapFloor,
            inst.hw1f_curve_id(),
            inst.model_config(),
            self.hw_params,
            &format!("{} {}", N::LABEL, inst.note_id()),
            market,
        )
    }

    /// Base MC configuration with the instrument's path and seed overrides.
    pub(crate) fn effective_config(&self, inst: &N) -> Hw1fMcConfig {
        let model_config = inst.model_config();
        self.config.with_instrument_overrides(
            inst.note_id(),
            model_config.mc_paths,
            model_config.mc_seed_scenario.as_deref(),
        )
    }

    /// Monte Carlo PV estimate of `inst`.
    pub(crate) fn price_estimate(
        &self,
        inst: &N,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<MoneyEstimate> {
        inst.hw1f_estimate(self, market, as_of)
    }

    /// Mean Monte Carlo PV of `inst`.
    pub(crate) fn price_internal(
        &self,
        inst: &N,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        Ok(self.price_estimate(inst, market, as_of)?.mean)
    }
}

impl<N: Hw1fNote> Pricer for Hw1fNotePricer<N> {
    fn key(&self) -> PricerKey {
        PricerKey::new(N::INSTRUMENT_TYPE, ModelKey::MonteCarloHullWhite1F)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let note = expect_inst::<N>(instrument, N::INSTRUMENT_TYPE)?;
        let estimate = self.price_estimate(note, market, as_of).map_err(|e| {
            PricingError::from_core(
                e,
                PricingErrorContext::from_instrument(instrument)
                    .model(ModelKey::MonteCarloHullWhite1F)
                    .curve_ids(note.error_curve_ids()),
            )
        })?;

        let mut result = ValuationResult::stamped(note.id(), as_of, estimate.mean);
        finstack_quant_valuations_core::instruments::common_impl::helpers::attach_mc_diagnostics(
            &mut result,
            &estimate,
        );
        Ok(result)
    }

    fn price_raw_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<f64, PricingError> {
        let note = expect_inst::<N>(instrument, N::INSTRUMENT_TYPE)?;
        self.price_internal(note, market, as_of)
            .map(|m| m.amount())
            .map_err(|e| {
                PricingError::from_core(
                    e,
                    PricingErrorContext::from_instrument(instrument)
                        .model(ModelKey::MonteCarloHullWhite1F),
                )
            })
    }
}

/// Zero-dispersion estimate of a value known exactly (no Monte Carlo
/// component), reporting the path counts the configured run would have used.
pub(crate) fn exact_estimate(
    pv: Money,
    num_paths: usize,
    num_simulated_paths: usize,
) -> MoneyEstimate {
    MoneyEstimate {
        mean: pv,
        stderr: 0.0,
        ci_95: (pv, pv),
        num_paths,
        num_simulated_paths,
        std_dev: Some(0.0),
        median: None,
        percentile_25: None,
        percentile_75: None,
        min: Some(pv.amount()),
        max: Some(pv.amount()),
    }
}

/// Exact zero estimate of a note with no remaining cashflows.
pub(crate) fn zero_estimate(currency: finstack_quant_core::currency::Currency) -> MoneyEstimate {
    exact_estimate(Money::from((0_i64, currency)), 0, 0)
}

/// Coupon period boundaries: `start_date` followed by every payment date.
///
/// Period `i` accrues over `[boundaries[i], boundaries[i + 1]]`, fixes in
/// advance at `boundaries[i]` and pays at `boundaries[i + 1]`.
pub(crate) fn period_boundaries(start_date: Date, payment_dates: &[Date]) -> Vec<Date> {
    std::iter::once(start_date)
        .chain(payment_dates.iter().copied())
        .collect()
}
