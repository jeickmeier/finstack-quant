//! Hull-White 1F Monte Carlo pricer for callable range accruals.

use crate::instruments::common_impl::pricing::time::relative_df_discount_curve;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::exotics::callable_range_accrual::CallableRangeAccrual;
use crate::instruments::rates::hw1f::{
    basis_for_degree, initial_short_rate_from_curve, prepare_hw1f_params, resolve_hw1f_params,
    ExerciseBoundaryPayoff, Hw1fParamFamily, Hw1fTermForward, PeriodForwardCoeffs,
    RateExoticHw1fLsmcPricer, RateExoticHw1fMcPricer, RateExoticMcConfig,
};
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_models::monte_carlo::results::MoneyEstimate;
use finstack_quant_models::monte_carlo::traits::{PathState, Payoff, StateKey};
use finstack_quant_models::rates::clock::model_time;
use finstack_quant_models::rates::hull_white::HullWhiteCalibrationParams;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy)]
struct CallableRangeAccrualEvent {
    is_observation: bool,
    /// HW1F bond-reconstruction coefficients for the reference rate tested
    /// against the accrual range.
    ///
    /// A range accrual checks a *term* reference rate (its tenor tracks the
    /// observation frequency), not the instantaneous short rate. These
    /// coefficients turn the simulated `r(t)` at the observation date into
    /// that term rate via the HW1F affine bond formula. Call-only events
    /// (`is_observation == false`) carry an unused passthrough.
    forward_coeffs: PeriodForwardCoeffs,
    /// Index of the call date this event falls on, if any.
    exercise_slot: Option<usize>,
    /// HW1F bond from this event to the final payment date. On a call date it
    /// prices the gain from receiving the accrued coupon at the call rather
    /// than at the final payment.
    to_final_payment: PeriodForwardCoeffs,
}

impl Default for CallableRangeAccrualEvent {
    fn default() -> Self {
        Self {
            is_observation: false,
            // Replaced with real reconstructions in `build_schedule`; harmless
            // passthroughs for events that do not need them.
            forward_coeffs: PeriodForwardCoeffs::from_flat_rate(0.0, 0.0),
            exercise_slot: None,
            to_final_payment: PeriodForwardCoeffs::from_flat_rate(0.0, 0.0),
        }
    }
}

#[derive(Debug, Clone)]
struct CallableRangeAccrualSchedule {
    events: Vec<CallableRangeAccrualEvent>,
    event_times: Vec<f64>,
    exercise_times: Vec<f64>,
    call_prices: Vec<f64>,
    /// Index (into `events`) of the event coinciding with the final payment
    /// date, where the pathwise bank-account numeraire `B(T_pay)` is read.
    final_payment_event_idx: Option<usize>,
    final_payment_discount_factor: f64,
    future_observations: usize,
}

#[derive(Debug, Clone)]
struct CallableRangeAccrualPayoff {
    lower_bound: f64,
    upper_bound: f64,
    coupon_rate: f64,
    notional: f64,
    events: Vec<CallableRangeAccrualEvent>,
    call_prices: Vec<f64>,
    final_payment_event_idx: Option<usize>,
    final_payment_discount_factor: f64,
    past_in_range: usize,
    total_past_observations: usize,
    future_observations: usize,
    days_in_range: usize,
    observations_seen: usize,
    next_event: usize,
    /// Simulated in-range observation count as of each call date (indexed by
    /// exercise slot), recorded as the path passes it.
    in_range_at_exercise: Vec<usize>,
    /// Path discount factor from each call date to the final payment date.
    bond_to_final_at_exercise: Vec<f64>,
    /// Pathwise bank-account numeraire observed at the final payment event;
    /// 0.0 until recorded.
    bank_at_final_payment: f64,
    /// LSMC regression basis degree from `RateExoticMcConfig::basis_degree`
    /// (2 → `standard_basis`, 3+ → `extended_basis`).
    basis_degree: usize,
}

impl CallableRangeAccrualPayoff {
    #[allow(clippy::too_many_arguments)]
    fn new(
        lower_bound: f64,
        upper_bound: f64,
        coupon_rate: f64,
        notional: f64,
        events: Vec<CallableRangeAccrualEvent>,
        call_prices: Vec<f64>,
        final_payment_event_idx: Option<usize>,
        final_payment_discount_factor: f64,
        past_in_range: usize,
        total_past_observations: usize,
        future_observations: usize,
        basis_degree: usize,
    ) -> Self {
        let call_prices_len = call_prices.len();
        Self {
            lower_bound,
            upper_bound,
            coupon_rate,
            notional,
            events,
            call_prices,
            final_payment_event_idx,
            final_payment_discount_factor,
            past_in_range,
            total_past_observations,
            future_observations,
            days_in_range: 0,
            observations_seen: 0,
            next_event: 0,
            in_range_at_exercise: vec![0; call_prices_len],
            bond_to_final_at_exercise: vec![1.0; call_prices_len],
            bank_at_final_payment: 0.0,
            basis_degree,
        }
    }

    /// Range coupon earned on `simulated_in_range` path observations plus the
    /// historical ones: the full-life coupon times the share of all
    /// observations that fell in range.
    fn accrued_coupon(&self, simulated_in_range: usize) -> f64 {
        let total_observations = self.total_past_observations + self.future_observations;
        if total_observations == 0 {
            return 0.0;
        }
        let in_range = self.past_in_range + simulated_in_range;
        self.coupon_rate * self.notional * in_range as f64 / total_observations as f64
    }

    /// Time-0 path discount factor of a cashflow on the final payment date.
    fn final_payment_discount(&self) -> f64 {
        // Pathwise bank-account numeraire observed at the final payment
        // event; the deterministic curve DF only when the simulation never
        // reached that event.
        if self.bank_at_final_payment > 0.0 {
            1.0 / self.bank_at_final_payment
        } else {
            self.final_payment_discount_factor
        }
    }

    fn is_in_range(&self, rate: f64) -> bool {
        rate >= self.lower_bound && rate <= self.upper_bound
    }

    /// Pathwise value of the callable range-accrual **note** when it is *not*
    /// called: the range-accrual coupon (a single payment based on the full-life
    /// fraction of observations in range) plus redemption of principal, both at
    /// the final payment date.
    ///
    /// Principal is included so this continuation value is on the same basis
    /// as the call amount returned by [`Self::intrinsic_at`].
    fn note_value(&self) -> f64 {
        (self.accrued_coupon(self.days_in_range) + self.notional) * self.final_payment_discount()
    }
}

impl Payoff for CallableRangeAccrualPayoff {
    fn on_event(&mut self, state: &mut PathState) -> finstack_quant_core::Result<()> {
        if self.next_event >= self.events.len() {
            return Ok(());
        }

        if self.final_payment_event_idx == Some(self.next_event) {
            self.bank_at_final_payment = state.get_key(StateKey::BankAccount).unwrap_or(1.0);
        }

        let event = self.events[self.next_event];
        if event.is_observation {
            // The accrual range is tested against the *term* reference rate
            // reconstructed from the simulated short rate via the HW1F affine
            // bond formula, not the instantaneous short rate r(t) directly.
            let short_rate = state.get_key(StateKey::ShortRate).unwrap_or(0.0);
            let reference_rate = event.forward_coeffs.simple_forward(short_rate);
            if self.is_in_range(reference_rate) {
                self.days_in_range += 1;
            }
            self.observations_seen += 1;
        }
        if let Some(slot) = event.exercise_slot {
            let short_rate = state.get_key(StateKey::ShortRate).unwrap_or(0.0);
            self.in_range_at_exercise[slot] = self.days_in_range;
            self.bond_to_final_at_exercise[slot] =
                event.to_final_payment.discount_factor(short_rate);
        }
        self.next_event += 1;
        Ok(())
    }

    fn value(
        &self,
        currency: finstack_quant_core::currency::Currency,
    ) -> finstack_quant_core::Result<Money> {
        Money::new(self.note_value(), currency)
    }

    fn reset(&mut self) {
        self.days_in_range = 0;
        self.observations_seen = 0;
        self.next_event = 0;
        self.in_range_at_exercise.fill(0);
        self.bond_to_final_at_exercise.fill(1.0);
        self.bank_at_final_payment = 0.0;
    }
}

impl ExerciseBoundaryPayoff for CallableRangeAccrualPayoff {
    fn intrinsic_at(
        &self,
        exercise_idx: usize,
        _short_rate: f64,
        currency: finstack_quant_core::currency::Currency,
    ) -> finstack_quant_core::Result<Money> {
        // Undiscounted at-exercise call amount; the LSMC harness discounts it
        // to time 0 with the pathwise bank-account numeraire B(t_exercise).
        //
        // A call redeems the note at the call price and pays the range coupon
        // accrued to the call date. The harness already keeps that accrued
        // coupon as a pre-exercise cashflow valued on the final payment date
        // (see `value_after`), so the call amount adds only what is gained by
        // receiving it at the call date instead: `accrued · (1 − P(t, T))`.
        // The issuer therefore calls exactly when
        // `call price + accrued < accrued · P(t, T) + remaining note value`.
        let call_price = self.call_prices.get(exercise_idx).copied().unwrap_or(0.0);
        let accrued = self.accrued_coupon(self.days_in_range);
        let bond_to_final = self
            .bond_to_final_at_exercise
            .get(exercise_idx)
            .copied()
            .unwrap_or(1.0);
        Money::new(
            call_price * self.notional + accrued * (1.0 - bond_to_final),
            currency,
        )
    }

    /// Note value excluding the coupon accrued up to the call date: that
    /// coupon is owed whether or not the issuer calls, so it is neither
    /// regressed as continuation value nor replaced on exercise.
    fn value_after(
        &self,
        exercise_idx: usize,
        currency: finstack_quant_core::currency::Currency,
    ) -> finstack_quant_core::Result<Money> {
        let in_range_at_call = self
            .in_range_at_exercise
            .get(exercise_idx)
            .copied()
            .unwrap_or(0);
        Money::new(
            self.note_value()
                - self.accrued_coupon(in_range_at_call) * self.final_payment_discount(),
            currency,
        )
    }

    fn continuation_basis(&self, _exercise_idx: usize, t_years: f64, short_rate: f64) -> Vec<f64> {
        basis_for_degree(self.basis_degree, t_years, short_rate)
    }
}

/// Callable range accrual pricer using shared HW1F MC/LSMC infrastructure.
#[derive(Debug, Clone)]
pub struct CallableRangeAccrualPricer {
    hw_params: Option<HullWhiteCalibrationParams>,
    config: RateExoticMcConfig,
}

impl CallableRangeAccrualPricer {
    /// Create a pricer requiring explicit or pre-calibrated HW1F parameters.
    pub fn new() -> Self {
        Self {
            hw_params: None,
            config: RateExoticMcConfig::default(),
        }
    }

    /// Create a callable range accrual pricer with explicit HW1F parameters.
    pub fn with_hw_params(hw_params: HullWhiteCalibrationParams) -> Self {
        Self {
            hw_params: Some(hw_params),
            config: RateExoticMcConfig::default(),
        }
    }

    /// Set the Monte Carlo configuration (paths, seed, antithetic flag, time
    /// steps) used by this pricer; instrument overrides still apply on top.
    ///
    /// # Arguments
    ///
    /// * `config` - Base Hull-White Monte Carlo configuration.
    pub fn with_config(mut self, config: RateExoticMcConfig) -> Self {
        self.config = config;
        self
    }

    fn effective_hw_params(
        &self,
        inst: &CallableRangeAccrual,
        market: &MarketContext,
    ) -> Result<HullWhiteCalibrationParams> {
        resolve_hw1f_params(
            Hw1fParamFamily::CapFloor,
            inst.range_accrual.discount_curve_id.as_str(),
            &inst.instrument_pricing_overrides.model_config,
            self.hw_params,
            &format!("CallableRangeAccrual {}", inst.id),
            market,
        )
    }

    fn effective_config(&self, inst: &CallableRangeAccrual) -> RateExoticMcConfig {
        self.config.with_instrument_overrides(
            &inst.id,
            inst.instrument_pricing_overrides.model_config.mc_paths,
            inst.instrument_pricing_overrides
                .model_config
                .mc_seed_scenario
                .as_deref(),
        )
    }

    fn price_estimate(
        &self,
        inst: &CallableRangeAccrual,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<MoneyEstimate> {
        inst.validate()?;

        // A callable range accrual carries the same historical-observation
        // state as its underlying range accrual.  Do not silently treat
        // already-fixed observations as out-of-range when a seasoned trade is
        // valued without that state.
        let range = &inst.range_accrual;
        let historical_count = range
            .observation_dates
            .iter()
            .filter(|&&date| date <= as_of)
            .count();
        if historical_count > 0 {
            let supplied_total = range.total_past_observations.ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "CallableRangeAccrual '{}' requires total_past_observations for {historical_count} historical observations",
                    inst.id
                ))
            })?;
            let supplied_in_range = range.past_observations_in_range.ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "CallableRangeAccrual '{}' requires past_observations_in_range for {historical_count} historical observations",
                    inst.id
                ))
            })?;
            if supplied_total != historical_count || supplied_in_range > supplied_total {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "CallableRangeAccrual '{}' historical observation state is inconsistent: supplied total={supplied_total}, in_range={supplied_in_range}, expected total={historical_count}",
                    inst.id
                )));
            }
        }

        let discount_curve = market.get_discount(inst.range_accrual.discount_curve_id.as_ref())?;
        let hw_params = self.effective_hw_params(inst, market)?;
        // HW1F bond-reconstruction built from the discount curve; turns the
        // simulated short rate at each observation into the term reference rate.
        let term_forward = Hw1fTermForward::new(hw_params, discount_curve.as_ref(), as_of)?;
        let schedule = build_schedule(inst, market, as_of, &term_forward)?;
        if schedule.event_times.is_empty() {
            return Ok(zero_estimate(inst.range_accrual.notional.currency()));
        }

        // The observed spot reference rate (e.g. today's SOFR fixing) scales
        // `RelativeToInitialSpot` accrual bounds — a *contractual* spot, distinct
        // from the HW1F simulation's initial short rate `r0` below.
        let initial_rate = initial_short_rate(inst, market)?;
        let lower_bound = inst.range_accrual.effective_lower_bound(initial_rate);
        let upper_bound = inst.range_accrual.effective_upper_bound(initial_rate);
        // Initial short rate = discount-curve instantaneous forward f(0,0).
        // HW1F reprices the discount curve only when r(0) = f(0,0); seeding it
        // from the spot fixing would offset the simulated short rate from the
        // curve and break the M6 repricing property.
        let r0 = initial_short_rate_from_curve(discount_curve.as_ref(), as_of)?;
        let config = self.effective_config(inst);
        let payoff = CallableRangeAccrualPayoff::new(
            lower_bound,
            upper_bound,
            inst.range_accrual.coupon_rate * inst.range_accrual.accrual_year_fraction()?,
            inst.range_accrual.notional.amount(),
            schedule.events.clone(),
            schedule.call_prices.clone(),
            schedule.final_payment_event_idx,
            schedule.final_payment_discount_factor,
            inst.range_accrual.past_observations_in_range.unwrap_or(0),
            inst.range_accrual.total_past_observations.unwrap_or(0),
            schedule.future_observations,
            config.basis_degree,
        );
        // Bootstrap a time-dependent θ(t) from the discount curve so the
        // simulated short rate reprices the initial curve (HW1F, not Vasicek).
        let horizon = schedule.event_times.last().copied().unwrap_or(0.0);
        let process_params =
            prepare_hw1f_params(hw_params, discount_curve.as_ref(), as_of, horizon)?;
        if schedule.exercise_times.is_empty() {
            let mc = RateExoticHw1fMcPricer {
                process_params,
                r0,
                event_times: schedule.event_times,
                config,
                currency: inst.range_accrual.notional.currency(),
            };
            return mc.price(|| payoff.clone());
        }

        let lsmc = RateExoticHw1fLsmcPricer {
            process_params,
            r0,
            event_times: schedule.event_times,
            exercise_times: schedule.exercise_times,
            config,
            currency: inst.range_accrual.notional.currency(),
        };
        lsmc.price(|| payoff.clone())
    }

    pub(crate) fn price_internal(
        &self,
        inst: &CallableRangeAccrual,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        Ok(self.price_estimate(inst, market, as_of)?.mean)
    }
}

impl Default for CallableRangeAccrualPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for CallableRangeAccrualPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(
            InstrumentType::CallableRangeAccrual,
            ModelKey::MonteCarloHullWhite1F,
        )
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let callable = crate::pricer::expect_inst::<CallableRangeAccrual>(
            instrument,
            InstrumentType::CallableRangeAccrual,
        )?;
        let estimate = self.price_estimate(callable, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(
                e.to_string(),
                PricingErrorContext::from_instrument(instrument)
                    .model(ModelKey::MonteCarloHullWhite1F)
                    .curve_ids([callable
                        .range_accrual
                        .discount_curve_id
                        .as_str()
                        .to_string()]),
            )
        })?;

        let mut result = ValuationResult::stamped(callable.id.as_str(), as_of, estimate.mean);
        crate::instruments::common_impl::helpers::attach_mc_diagnostics(&mut result, &estimate);
        Ok(result)
    }

    fn price_raw_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<f64, PricingError> {
        let callable = crate::pricer::expect_inst::<CallableRangeAccrual>(
            instrument,
            InstrumentType::CallableRangeAccrual,
        )?;
        self.price_internal(callable, market, as_of)
            .map(|m| m.amount())
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::from_instrument(instrument)
                        .model(ModelKey::MonteCarloHullWhite1F),
                )
            })
    }
}

fn build_schedule(
    inst: &CallableRangeAccrual,
    market: &MarketContext,
    as_of: Date,
    term_forward: &Hw1fTermForward<'_>,
) -> Result<CallableRangeAccrualSchedule> {
    let range = &inst.range_accrual;
    let discount_curve = market.get_discount(range.discount_curve_id.as_ref())?;
    let final_payment_date = range
        .payment_date
        .or_else(|| range.observation_dates.last().copied())
        .ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "CallableRangeAccrual {} requires at least one observation date",
                inst.id.as_str()
            ))
        })?;
    let final_payment_discount_factor =
        relative_df_discount_curve(discount_curve.as_ref(), as_of, final_payment_date)?;

    let index_tenor = range
        .index_tenor
        .ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "CallableRangeAccrual requires index_tenor".to_string(),
            )
        })?
        .to_years();
    if !index_tenor.is_finite() || index_tenor <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "CallableRangeAccrual '{}' index_tenor must be positive",
            inst.id
        )));
    }

    let mut event_dates: BTreeMap<Date, CallableRangeAccrualEvent> = BTreeMap::new();
    for &date in &range.observation_dates {
        if date > as_of {
            event_dates.entry(date).or_default().is_observation = true;
        }
    }

    let eligible_call_dates = inst.call_provision.eligible_call_dates();
    if let Some(late) = eligible_call_dates
        .iter()
        .find(|date| **date > final_payment_date)
    {
        return Err(finstack_quant_core::Error::Validation(format!(
            "CallableRangeAccrual '{}' call date {late} is after the final payment date              {final_payment_date}; the note has already redeemed",
            inst.id
        )));
    }
    for &date in &eligible_call_dates {
        if date > as_of {
            event_dates.entry(date).or_default();
        }
    }

    // The final payment date must be a simulation event so the payoff can
    // observe the pathwise bank-account numeraire B(T_pay) there.
    if final_payment_date > as_of {
        event_dates.entry(final_payment_date).or_default();
    }

    let final_payment_time = model_time(as_of, final_payment_date);
    let mut events = Vec::with_capacity(event_dates.len());
    let mut event_times = Vec::with_capacity(event_dates.len());
    let mut final_payment_event_idx = None;
    let mut next_exercise_slot = 0_usize;
    for (date, mut event) in event_dates {
        let t = model_time(as_of, date);
        if t > 0.0 {
            if event.is_observation {
                // Term reference rate over [t, t + index_tenor].
                event.forward_coeffs = term_forward.period_coeffs(t, index_tenor);
            }
            if eligible_call_dates.contains(&date) {
                event.exercise_slot = Some(next_exercise_slot);
                event.to_final_payment = term_forward.period_coeffs(t, final_payment_time - t);
                next_exercise_slot += 1;
            }
            if date == final_payment_date {
                final_payment_event_idx = Some(events.len());
            }
            events.push(event);
            event_times.push(t);
        }
    }

    let mut exercise_times = Vec::new();
    let mut call_prices = Vec::new();
    for date in eligible_call_dates.into_iter().filter(|d| *d > as_of) {
        let t = model_time(as_of, date);
        if t <= 0.0 {
            continue;
        }
        exercise_times.push(t);
        // Percent of par on the wire, fraction of notional in the payoff.
        call_prices.push(inst.call_provision.price_pct_of_par / 100.0);
    }

    let future_observations = events.iter().filter(|event| event.is_observation).count();
    Ok(CallableRangeAccrualSchedule {
        events,
        event_times,
        exercise_times,
        call_prices,
        final_payment_event_idx,
        final_payment_discount_factor,
        future_observations,
    })
}

fn initial_short_rate(inst: &CallableRangeAccrual, market: &MarketContext) -> Result<f64> {
    let scalar = market.get_price(inst.range_accrual.spot_id.as_ref())?;
    let rate = match scalar {
        finstack_quant_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
        finstack_quant_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
    };
    if !rate.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "CallableRangeAccrual {} initial rate is not finite",
            inst.id.as_str()
        )));
    }
    Ok(rate)
}

fn zero_estimate(currency: finstack_quant_core::currency::Currency) -> MoneyEstimate {
    let zero = Money::from((0_i64, currency));
    MoneyEstimate {
        mean: zero,
        stderr: 0.0,
        ci_95: (zero, zero),
        num_paths: 0,
        num_simulated_paths: 0,
        std_dev: Some(0.0),
        median: None,
        percentile_25: None,
        percentile_75: None,
        min: Some(0.0),
        max: Some(0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::exotics::range_accrual::{BoundsType, RangeAccrualTerms};
    use crate::instruments::rates::hw1f::bermudan_call::BermudanCallProvision;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use time::Month;

    fn date(year: i32, month: Month, day: u8) -> Date {
        Date::from_calendar_date(year, month, day).expect("valid date")
    }

    fn test_callable(
        call_dates: Vec<Date>,
        lockout_end: Option<Date>,
        coupon_rate: f64,
    ) -> CallableRangeAccrual {
        let observation_dates = vec![
            date(2025, Month::July, 1),
            date(2026, Month::January, 1),
            date(2026, Month::July, 1),
        ];
        CallableRangeAccrual {
            id: InstrumentId::new("CALLABLE-RA-TEST"),
            range_accrual: RangeAccrualTerms::builder()
                .underlying_ticker("SOFR".to_string())
                .observation_dates(observation_dates)
                .lower_bound(0.02)
                .upper_bound(0.04)
                .bounds_type(BoundsType::Absolute)
                .coupon_rate(coupon_rate)
                .notional(Money::from((1_000_000_i64, Currency::USD)))
                .day_count(DayCount::Act365F)
                .discount_curve_id(CurveId::new("USD-OIS"))
                .start_date(date(2025, Month::January, 1))
                .index_id_opt(Some("SOFR".into()))
                .forward_curve_id_opt(Some(CurveId::new("USD-OIS")))
                .index_tenor_opt(Some(
                    finstack_quant_core::dates::Tenor::new(
                        6,
                        finstack_quant_core::dates::TenorUnit::Months,
                    )
                    .expect("valid tenor fixture"),
                ))
                .spot_id("SOFR-RATE".into())
                .vol_surface_id(CurveId::new("SOFR-VOL"))
                .div_yield_id_opt(None)
                .payment_date_opt(None)
                .past_observations_in_range_opt(None)
                .total_past_observations_opt(None)
                .build()
                .expect("range accrual"),
            call_provision: BermudanCallProvision::new(call_dates, 100.0, lockout_end),
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Default::default(),
        }
    }

    fn market(as_of: Date, discount_rate: f64, short_rate: f64) -> MarketContext {
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([
                (0.0, 1.0),
                (0.5, (-discount_rate * 0.5).exp()),
                (1.5, (-discount_rate * 1.5).exp()),
            ])
            .build()
            .expect("discount curve");
        MarketContext::new()
            .insert(discount)
            .insert_price("SOFR-RATE", MarketScalar::Unitless(short_rate))
    }

    fn deterministic_pricer(paths: usize) -> CallableRangeAccrualPricer {
        CallableRangeAccrualPricer::with_hw_params(
            HullWhiteCalibrationParams::new(0.05, 1e-12).expect("hw params"),
        )
        .with_config(RateExoticMcConfig {
            num_paths: paths,
            antithetic: false,
            min_steps_between_events: 1,
            ..Default::default()
        })
    }

    /// `mc_basis_degree` must be a live config surface: degree 3+ switches the
    /// LSMC continuation regression from the degree-2 `standard_basis` to the
    /// degree-3 `extended_basis`, which changes the fitted exercise rule and
    /// therefore the price. Guards against the recurring silently-inert-config
    /// defect class (config parsed and clamped but never read by the pricer).
    #[test]
    fn basis_degree_config_changes_lsmc_price() {
        let as_of = date(2025, Month::January, 1);
        let curves = market(as_of, 0.02, 0.03);
        // One mid-life call date, no lockout, rich coupon so the issuer call
        // is genuinely in play and the regression drives exercise decisions.
        let inst = test_callable(vec![date(2025, Month::July, 1)], None, 0.06);

        let pricer_for_degree = |degree: usize| {
            CallableRangeAccrualPricer::with_hw_params(
                HullWhiteCalibrationParams::new(0.05, 0.01).expect("hw params"),
            )
            .with_config(RateExoticMcConfig {
                num_paths: 256,
                antithetic: false,
                min_steps_between_events: 1,
                basis_degree: degree,
                ..Default::default()
            })
        };

        let pv_deg2 = pricer_for_degree(2)
            .price_estimate(&inst, &curves, as_of)
            .expect("degree 2")
            .mean
            .amount();
        let pv_deg3 = pricer_for_degree(3)
            .price_estimate(&inst, &curves, as_of)
            .expect("degree 3")
            .mean
            .amount();

        assert!(
            (pv_deg2 - pv_deg3).abs() > 1e-9,
            "mc_basis_degree must change the LSMC regression basis and price; \
             got identical PV {pv_deg2} for degrees 2 and 3 (inert config)"
        );
        let rel = (pv_deg2 - pv_deg3).abs() / pv_deg2.abs().max(1.0);
        assert!(
            rel < 0.05,
            "basis change is a regression refinement, not a repricing: deg2={pv_deg2}, deg3={pv_deg3}"
        );
    }

    /// The default estimator count and antithetic convention must reach the
    /// pricer unchanged. Same-seed replay verifies the current numerical engine
    /// without pinning a retired RNG or discretization's historical output.
    #[test]
    fn rate_exotic_defaults_match_explicit_estimator_configuration() {
        let as_of = date(2025, Month::January, 1);
        let curves = market(as_of, 0.02, 0.03);
        let inst = test_callable(vec![date(2025, Month::July, 1)], None, 0.06);
        let hw_params = HullWhiteCalibrationParams::new(0.05, 0.015).expect("hw params");
        let default = CallableRangeAccrualPricer::with_hw_params(hw_params)
            .price_estimate(&inst, &curves, as_of)
            .expect("default price");
        let explicit_config = RateExoticMcConfig {
            num_paths: 10_000,
            seed: 42,
            antithetic: true,
            min_steps_between_events: 4,
            basis_degree: 2,
            oos_lsmc: false,
        };
        let explicit = CallableRangeAccrualPricer::with_hw_params(hw_params)
            .with_config(explicit_config)
            .price_estimate(&inst, &curves, as_of)
            .expect("explicit default price");
        assert_eq!(
            default.mean.amount().to_bits(),
            explicit.mean.amount().to_bits()
        );
        assert_eq!(default.stderr.to_bits(), explicit.stderr.to_bits());
        assert_eq!(default.ci_95, explicit.ci_95);
        assert_eq!(default.num_paths, 10_000);
        assert_eq!(explicit.num_paths, 10_000);
        assert_eq!(default.num_simulated_paths, 20_000);
        assert_eq!(explicit.num_simulated_paths, 20_000);

        // Unlike the replay above, this changes both estimator count and path
        // pairing, proving the explicit configuration is consumed.
        let reduced = CallableRangeAccrualPricer::with_hw_params(hw_params)
            .with_config(RateExoticMcConfig {
                num_paths: 127,
                antithetic: false,
                ..explicit_config
            })
            .price_estimate(&inst, &curves, as_of)
            .expect("reduced unpaired sample");
        assert_eq!(reduced.num_paths, 127);
        assert_eq!(reduced.num_simulated_paths, 127);
        assert_ne!(
            default.mean.amount().to_bits(),
            reduced.mean.amount().to_bits()
        );
    }

    /// A larger issuer redemption amount increases the holder's value under
    /// the same stochastic paths and fitted exercise-policy convention.
    #[test]
    fn higher_call_price_increases_stochastic_note_value() {
        let as_of = date(2025, Month::January, 1);
        let curves = market(as_of, 0.02, 0.03);
        let mut inst = test_callable(vec![date(2025, Month::July, 1)], None, 0.06);
        inst.call_provision.price_pct_of_par = 102.0;
        let pricer = || {
            CallableRangeAccrualPricer::with_hw_params(
                HullWhiteCalibrationParams::new(0.05, 0.015).expect("hw params"),
            )
        };
        let pv = pricer()
            .price_estimate(&inst, &curves, as_of)
            .expect("price")
            .mean
            .amount();
        let at_par = pricer()
            .price_estimate(
                &test_callable(vec![date(2025, Month::July, 1)], None, 0.06),
                &curves,
                as_of,
            )
            .expect("par price")
            .mean
            .amount();
        assert!(
            pv - at_par > 1e-6,
            "a higher issuer call price must increase value: {pv} vs {at_par}"
        );
    }

    #[test]
    fn implied_volatility_is_not_used_as_hw1f_sigma() {
        let as_of = date(2025, Month::January, 1);
        let curves = market(as_of, 0.02, 0.03);
        let no_iv = test_callable(
            vec![date(2025, Month::July, 1)],
            Some(date(2030, Month::January, 1)),
            0.06,
        );
        let mut with_iv = no_iv.clone();
        with_iv
            .instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(0.20);

        let pv_no_iv = deterministic_pricer(8)
            .price_estimate(&no_iv, &curves, as_of)
            .expect("no iv")
            .mean
            .amount();
        let pv_with_iv = deterministic_pricer(8)
            .price_estimate(&with_iv, &curves, as_of)
            .expect("with iv")
            .mean
            .amount();

        assert!(
            (pv_with_iv - pv_no_iv).abs() < 1e-9,
            "implied_volatility must not alter callable range accrual HW1F sigma: no_iv={pv_no_iv}, with_iv={pv_with_iv}"
        );
    }

    #[test]
    fn no_eligible_call_dates_prices_like_noncallable_note() {
        let as_of = date(2025, Month::January, 1);
        let curves = market(as_of, 0.02, 0.03);
        // Lockout of 10 periods makes the single call date ineligible, so the note
        // is never called and prices as a bullet: coupon + principal redemption.
        let inst = test_callable(
            vec![date(2025, Month::July, 1)],
            Some(date(2030, Month::January, 1)),
            0.06,
        );
        let maturity = *inst
            .range_accrual
            .observation_dates
            .last()
            .expect("maturity");
        let df = curves
            .get_discount("USD-OIS")
            .expect("discount")
            .df_between_dates(as_of, maturity)
            .expect("df");
        // All observations are in range (deterministic short rate ≈ 3%). The
        // coupon is quoted annually and scales by the contractual accrual year
        // fraction; principal redeems at par.
        let expected = inst.range_accrual.notional.amount()
            * (inst.range_accrual.coupon_rate
                * inst
                    .range_accrual
                    .accrual_year_fraction()
                    .expect("accrual factor")
                + 1.0)
            * df;

        let estimate = deterministic_pricer(8)
            .price_estimate(&inst, &curves, as_of)
            .expect("price");

        assert!((estimate.mean.amount() - expected).abs() < 1.0);
    }

    #[test]
    fn issuer_call_reduces_value_below_bullet_at_realistic_coupon() {
        let as_of = date(2025, Month::January, 1);
        let curves = market(as_of, 0.02, 0.03);
        let call_date = date(2025, Month::July, 1);
        // Callable: one eligible call date (no lockout) at a realistic 6% coupon.
        let callable = test_callable(vec![call_date], None, 0.06);
        // Bullet: the same note but the call is locked out, so it never fires.
        let bullet = test_callable(vec![call_date], Some(date(2030, Month::January, 1)), 0.06);

        let callable_pv = deterministic_pricer(8)
            .price_estimate(&callable, &curves, as_of)
            .expect("callable")
            .mean
            .amount();
        let bullet_pv = deterministic_pricer(8)
            .price_estimate(&bullet, &curves, as_of)
            .expect("bullet")
            .mean
            .amount();

        // The issuer call strips value from the holder: a realistic-coupon
        // callable note must price strictly below the otherwise-identical bullet.
        // (Before the principal-inclusion fix the call never fired and the two
        // were identical.)
        assert!(
            callable_pv < bullet_pv - 1.0,
            "issuer call must reduce note value: callable={callable_pv}, bullet={bullet_pv}"
        );
    }

    #[test]
    fn forced_call_redeems_percent_of_par_on_the_call_date() {
        let as_of = date(2025, Month::January, 1);
        let discount_rate = 0.02;
        let curves = market(as_of, discount_rate, 0.03);
        let call_date = date(2025, Month::July, 1);
        // A 200% coupon makes continuation more expensive to the issuer than
        // either redemption amount. The validated HW APIs require positive
        // sigma, so the end-to-end MC check below is only near deterministic;
        // it still includes numerical theta/bank-account integration.
        let mut inst = test_callable(vec![call_date], None, 2.0);
        let discount = curves.get_discount("USD-OIS").expect("discount");
        let call_df = discount.df_between_dates(as_of, call_date).expect("df");
        let term_forward = Hw1fTermForward::new(
            HullWhiteCalibrationParams::new(0.05, 1e-12).expect("hw params"),
            discount.as_ref(),
            as_of,
        )
        .expect("term forward");
        let notional = inst.range_accrual.notional.amount();
        // The call pays the coupon accrued to the call date: one of the three
        // observations has been made, and it is in range.
        let accrued = notional
            * inst.range_accrual.coupon_rate
            * inst
                .range_accrual
                .accrual_year_fraction()
                .expect("accrual factor")
            / 3.0;
        let mut deterministic_prices = [0.0; 2];
        for (slot, percent_of_par) in deterministic_prices.iter_mut().zip([100.0, 102.0]) {
            inst.call_provision.price_pct_of_par = percent_of_par;
            let mc_price = deterministic_pricer(4)
                .price_estimate(&inst, &curves, as_of)
                .expect("forced call price")
                .mean
                .amount();
            let expected = notional * percent_of_par / 100.0 * call_df;
            // Keep the original end-to-end PV budget. This rejects redemption
            // at maturity without treating positive-sigma MC as exact. The
            // called note also pays the coupon accrued to the call date.
            let expected_called = expected + accrued * call_df;
            assert!(
                (mc_price - expected_called).abs() < 1.0,
                "called note {mc_price} should be worth the call price plus accrued \
                 at the call date {expected_called}"
            );

            // Isolate redemption units and scheduled payment timing from MC
            // (a fresh payoff has seen no observation, so no coupon is accrued):
            // evaluate the production schedule/payoff on an exact flat-rate
            // bank account, with no random draws or fitted theta involved.
            let schedule =
                build_schedule(&inst, &curves, as_of, &term_forward).expect("callable schedule");
            assert_eq!(schedule.exercise_times.len(), 1);
            let call_time = schedule.exercise_times[0];
            let payoff = CallableRangeAccrualPayoff::new(
                0.02,
                0.04,
                inst.range_accrual.coupon_rate
                    * inst.range_accrual.accrual_year_fraction().expect("accrual"),
                notional,
                schedule.events,
                schedule.call_prices,
                schedule.final_payment_event_idx,
                schedule.final_payment_discount_factor,
                0,
                0,
                schedule.future_observations,
                2,
            );
            let redemption = payoff
                .intrinsic_at(0, discount_rate, Currency::USD)
                .expect("redemption")
                .amount();
            let bank_at_call = (discount_rate * call_time).exp();
            *slot = redemption / bank_at_call;
            assert!((*slot - expected).abs() < 1e-6);
        }
        // A 102% redemption pays exactly 2% extra notional on the eligible
        // call date. This check is deterministic and independent of the RNG.
        let expected_premium = 0.02 * notional * call_df;
        assert!(
            (deterministic_prices[1] - deterministic_prices[0] - expected_premium).abs() < 1e-6,
            "redemption premium {}, expected {expected_premium}",
            deterministic_prices[1] - deterministic_prices[0]
        );
    }

    /// A call on the final payment date changes nothing: the holder receives
    /// par and the whole range coupon either way. A call that forfeited the
    /// coupon would price this note as a zero-coupon bond.
    #[test]
    fn call_on_the_final_payment_date_matches_the_noncallable_note() {
        let as_of = date(2025, Month::January, 1);
        let curves = market(as_of, 0.02, 0.03);
        let final_date = date(2026, Month::July, 1);
        let callable = test_callable(vec![final_date], None, 0.06);
        let bullet = test_callable(vec![final_date], Some(date(2030, Month::January, 1)), 0.06);

        let price = |inst: &CallableRangeAccrual| {
            deterministic_pricer(8)
                .price_estimate(inst, &curves, as_of)
                .expect("price")
                .mean
                .amount()
        };
        let (callable_pv, bullet_pv) = (price(&callable), price(&bullet));
        assert!(
            (callable_pv - bullet_pv).abs() < 1e-6,
            "callable at the final date {callable_pv} should equal the bullet {bullet_pv}"
        );
        let zero_coupon = callable.range_accrual.notional.amount()
            * curves
                .get_discount("USD-OIS")
                .expect("discount")
                .df_between_dates(as_of, final_date)
                .expect("df");
        assert!(
            callable_pv > zero_coupon + 50_000.0,
            "the coupon must not be forfeited: {callable_pv} vs zero-coupon {zero_coupon}"
        );
    }

    #[test]
    fn call_date_after_the_final_payment_is_rejected() {
        let as_of = date(2025, Month::January, 1);
        let curves = market(as_of, 0.02, 0.03);
        let inst = test_callable(vec![date(2027, Month::January, 1)], None, 0.06);

        let error = deterministic_pricer(4)
            .price_estimate(&inst, &curves, as_of)
            .expect_err("a call after redemption is not a valid contract");
        assert!(
            error.to_string().contains("after the final payment date"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn price_dyn_returns_mc_measures() {
        let as_of = date(2025, Month::January, 1);
        let curves = market(as_of, 0.02, 0.03);
        let inst = test_callable(
            vec![date(2025, Month::July, 1)],
            Some(date(2030, Month::January, 1)),
            0.06,
        );
        let result = deterministic_pricer(8)
            .price_dyn(&inst, &curves, as_of)
            .expect("price");

        assert!(result.value.amount() > 0.0);
        assert!(result
            .measures
            .contains_key(&crate::metrics::MetricId::custom("mc_num_paths")));
    }
}
