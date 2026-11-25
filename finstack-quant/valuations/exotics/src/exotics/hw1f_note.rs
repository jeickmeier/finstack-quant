//! Shared Hull-White 1F Monte Carlo pricer for the structured rate notes.
//!
//! TARNs, snowballs and callable range accruals price off the same HW1F
//! harness: HW1F parameters resolved from the instrument's `model_config`
//! (or an explicit override), a base [`Hw1fMcConfig`] with per-instrument
//! path/seed overrides, and an MC estimate published with its diagnostics.
//! [`Hw1fNotePricer`] owns that shell once; each note supplies its payoff
//! simulation through [`Hw1fNote`].

use std::marker::PhantomData;
use std::sync::Arc;

use finstack_quant_core::Result;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, DayCountContext, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_models::monte_carlo::results::MoneyEstimate;
use finstack_quant_models::monte_carlo::traits::{PathState, Payoff, StateKey};
use finstack_quant_models::rates::clock::model_time;
use finstack_quant_models::rates::hull_white::HullWhiteCalibrationParams;
use finstack_quant_models::rates::hw1f::{
    CouponEvent, Hw1fMcConfig, Hw1fMcPricer, Hw1fTermForward, PeriodForwardCoeffs,
    initial_short_rate_from_curve, prepare_hw1f_params,
};
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::instruments::pricing::hw1f_params::{
    Hw1fParamFamily, resolve_hw1f_params,
};
use finstack_quant_valuations_core::instruments::pricing_overrides::ModelConfig;
use finstack_quant_valuations_core::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext, expect_inst,
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

    #[tracing::instrument(
        name = "hw1f_note.mc.price_dyn",
        level = "debug",
        skip(self, instrument, market),
        fields(
            note = N::LABEL,
            inst_id = %instrument.id(),
            as_of = %as_of,
        ),
        err,
    )]
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
        finstack_quant_valuations_core::instruments::pricing::monte_carlo::attach_mc_diagnostics(
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

/// Project the contractual `index_tenor` forward rate starting at `proj_start`.
pub(crate) fn projected_index_forward(
    forward_curve: &ForwardCurve,
    day_count: DayCount,
    proj_start: Date,
    index_tenor: Tenor,
) -> Result<f64> {
    let index_end = index_tenor.add_to_date(proj_start, None, BusinessDayConvention::Unadjusted)?;
    let index_yf = day_count.year_fraction(proj_start, index_end, DayCountContext::default())?;
    forward_curve.rate_between_dates(proj_start, index_end, index_yf)
}

/// Immutable coupon-note metadata needed to build an in-advance HW1F schedule.
pub(crate) struct InAdvanceNoteSpec<'a> {
    pub(crate) label: &'static str,
    pub(crate) id: &'a InstrumentId,
    pub(crate) start_date: Date,
    pub(crate) payment_dates: &'a [Date],
    pub(crate) index_tenor: Tenor,
    pub(crate) forward_curve_id: &'a CurveId,
    pub(crate) discount_curve_id: &'a CurveId,
    pub(crate) day_count: DayCount,
    pub(crate) currency: Currency,
}

/// Prepared in-advance HW1F schedule and simulation inputs for live notes.
pub(crate) struct PreparedInAdvanceSchedule {
    pub(crate) events: Arc<[CouponEvent]>,
    pub(crate) event_times: Vec<f64>,
    pub(crate) hw_params: HullWhiteCalibrationParams,
    pub(crate) r0: f64,
    pub(crate) final_coupon_date: Date,
    pub(crate) config: Hw1fMcConfig,
}

/// Build an in-advance coupon event schedule and HW1F parameters for a rate note.
///
/// Returns `Ok(None)` when `as_of >= final_coupon_date`.
pub(crate) fn build_in_advance_coupon_schedule<N: Hw1fNote>(
    note: &N,
    pricer: &Hw1fNotePricer<N>,
    spec: &InAdvanceNoteSpec<'_>,
    market: &MarketContext,
    as_of: Date,
    seasoned_err_msg: &str,
) -> Result<Option<PreparedInAdvanceSchedule>> {
    let first_coupon_date = spec.start_date;
    let final_coupon_date = *spec.payment_dates.last().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!("{} requires payment dates", spec.label))
    })?;
    if as_of >= final_coupon_date {
        return Ok(None);
    }
    if as_of > first_coupon_date {
        return Err(finstack_quant_core::Error::Validation(
            seasoned_err_msg.to_string(),
        ));
    }

    let discount_curve = market.get_discount(spec.discount_curve_id.as_ref())?;
    let forward_curve = market.get_forward(spec.forward_curve_id.as_ref())?;
    finstack_quant_valuations_core::instruments::pricing::term_fixings::validate_term_curve_tenor(
        forward_curve.as_ref(),
        spec.index_tenor,
        spec.id.as_str(),
    )?;

    let hw_params = pricer.effective_hw_params(note, market)?;
    let term_forward = Hw1fTermForward::new(hw_params, discount_curve.as_ref(), as_of)?;
    let r0 = initial_short_rate_from_curve(discount_curve.as_ref(), as_of)?;
    if !r0.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{} {} initial short rate is not finite",
            spec.label,
            spec.id.as_str()
        )));
    }

    let boundaries = period_boundaries(spec.start_date, spec.payment_dates);
    let mut events = Vec::new();
    let mut event_times = Vec::new();
    let tenor_years = spec.index_tenor.to_years_approx();

    for period in boundaries.windows(2) {
        let start = period[0];
        let end = period[1];
        if end <= as_of {
            continue;
        }
        let accrual_fraction =
            spec.day_count
                .year_fraction(start, end, DayCountContext::default())?;
        if !accrual_fraction.is_finite() || accrual_fraction <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{} {} has invalid accrual fraction {accrual_fraction} for {start} to {end}",
                spec.label,
                spec.id.as_str()
            )));
        }
        let discount_factor = discount_curve.as_ref().df_between_dates(as_of, end)?;

        let proj_start = start.max(as_of);
        let projected_index_rate = projected_index_forward(
            forward_curve.as_ref(),
            spec.day_count,
            proj_start,
            spec.index_tenor,
        )?;

        if start > as_of {
            let fixing_time = model_time(as_of, start);
            if !fixing_time.is_finite() || fixing_time <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{} {} has invalid fixing time {fixing_time} for period start {start}",
                    spec.label,
                    spec.id.as_str()
                )));
            }
            event_times.push(fixing_time);
            let coeffs = term_forward.period_coeffs(fixing_time, tenor_years)?;
            let discount_forward = term_forward.curve_forward(fixing_time, tenor_years)?;
            let basis = projected_index_rate - discount_forward;
            events.push(CouponEvent {
                accrual_fraction,
                discount_factor,
                forward_coeffs: coeffs.with_additive_spread(basis),
                needs_path_sample: true,
            });
        } else {
            events.push(CouponEvent {
                accrual_fraction,
                discount_factor,
                forward_coeffs: PeriodForwardCoeffs::from_flat_rate(
                    projected_index_rate,
                    accrual_fraction,
                ),
                needs_path_sample: false,
            });
        }
    }

    if events.is_empty() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{} {} has no future coupon period after {as_of}",
            spec.label,
            spec.id.as_str()
        )));
    }

    Ok(Some(PreparedInAdvanceSchedule {
        events: Arc::from(events),
        event_times,
        hw_params,
        r0,
        final_coupon_date,
        config: pricer.effective_config(note),
    }))
}

/// Launch HW1F Monte Carlo or deterministic exact settlement for a prepared note schedule.
pub(crate) fn run_hw1f_coupon_mc<P: Payoff + Clone + Send + Sync + 'static>(
    spec: &InAdvanceNoteSpec<'_>,
    market: &MarketContext,
    as_of: Date,
    prepared: PreparedInAdvanceSchedule,
    make_payoff: impl FnOnce(Arc<[CouponEvent]>, bool) -> P,
    settle_deterministic: impl FnOnce(&mut P),
) -> Result<MoneyEstimate> {
    let config = prepared.config;
    if prepared.event_times.is_empty() {
        let mut payoff = make_payoff(Arc::clone(&prepared.events), false);
        settle_deterministic(&mut payoff);
        let pv = payoff.value(spec.currency)?;
        return Ok(exact_estimate(
            pv,
            config.num_paths,
            config.simulated_path_count(),
        ));
    }

    let discount_curve = market.get_discount(spec.discount_curve_id.as_ref())?;
    let mut event_times = prepared.event_times;
    event_times.push(model_time(as_of, prepared.final_coupon_date));

    let horizon = event_times.last().copied().unwrap_or(0.0);
    let process_params =
        prepare_hw1f_params(prepared.hw_params, discount_curve.as_ref(), as_of, horizon)?;
    let mc = Hw1fMcPricer {
        process_params,
        r0: prepared.r0,
        event_times,
        config,
        currency: spec.currency,
    };

    let payoff = make_payoff(prepared.events, true);
    mc.price(|| payoff.clone())
}

/// Shared pending/flush/bank-account state machine for in-advance coupon payoffs.
#[derive(Debug, Clone)]
pub(crate) struct PathwiseCouponLedger {
    events: Arc<[CouponEvent]>,
    discounted_pv: f64,
    next_event: usize,
    pathwise: bool,
    pending: f64,
    last_bank: f64,
    redeemed: bool,
}

impl PathwiseCouponLedger {
    /// Construct a new ledger over shared coupon events.
    pub(crate) fn new(events: Arc<[CouponEvent]>, pathwise: bool) -> Self {
        Self {
            events,
            discounted_pv: 0.0,
            next_event: 0,
            pathwise,
            pending: 0.0,
            last_bank: 1.0,
            redeemed: false,
        }
    }

    pub(crate) fn next_event(&self) -> Option<CouponEvent> {
        self.events.get(self.next_event).copied()
    }

    pub(crate) fn is_exhausted(&self) -> bool {
        self.next_event >= self.events.len()
    }

    pub(crate) fn is_redeemed(&self) -> bool {
        self.redeemed
    }

    pub(crate) fn mark_redeemed(&mut self) {
        self.redeemed = true;
    }

    pub(crate) fn record_cashflow(&mut self, amount: f64, discount_factor: f64) {
        if self.pathwise {
            self.pending += amount;
        } else {
            self.discounted_pv += amount * discount_factor;
        }
    }

    pub(crate) fn advance(&mut self) {
        self.next_event += 1;
    }

    pub(crate) fn add_maturity_redemption_if_pending(&mut self, notional: f64) {
        if self.pathwise && self.is_exhausted() {
            self.pending += notional;
        }
    }

    pub(crate) fn drain_seasoned(&mut self, mut settle: impl FnMut(&mut Self, CouponEvent, f64)) {
        while !self.redeemed
            && !self.is_exhausted()
            && let Some(event) = self.next_event()
            && !event.needs_path_sample
        {
            settle(self, event, 0.0);
        }
    }

    pub(crate) fn step_event(
        &mut self,
        state: &mut PathState,
        mut settle: impl FnMut(&mut Self, CouponEvent, f64),
    ) {
        self.drain_seasoned(&mut settle);
        let bank = state.get_key(StateKey::BankAccount).unwrap_or(1.0);
        self.last_bank = bank;
        if self.pending != 0.0 {
            self.discounted_pv += self.pending / bank;
            self.pending = 0.0;
        }
        if self.redeemed || self.is_exhausted() {
            return;
        }
        if let Some(event) = self.next_event() {
            let short_rate = state.get_key(StateKey::ShortRate).unwrap_or(0.0);
            settle(self, event, short_rate);
        }
    }

    pub(crate) fn value(&self, notional: f64, currency: Currency) -> Result<Money> {
        let mut pv = self.discounted_pv;
        if self.pathwise {
            pv += self.pending / self.last_bank;
        } else if !self.redeemed
            && let Some(final_event) = self.events.last()
        {
            pv += notional * final_event.discount_factor;
        }
        Money::from_model_value(pv, currency)
    }

    pub(crate) fn reset(&mut self) {
        self.discounted_pv = 0.0;
        self.next_event = 0;
        self.pending = 0.0;
        self.last_bank = 1.0;
        self.redeemed = false;
    }
}
