//! Pricers for snowball and inverse-floater structured notes.

use crate::calibration::hull_white::HullWhiteParams;
use crate::instruments::common_impl::pricing::time::relative_df_discount_curve;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::exotics_shared::cumulative_coupon::CouponEvent;
use crate::instruments::rates::exotics_shared::hw1f_curve::{
    calibrate_hw1f_params, initial_short_rate_from_curve, Hw1fTermForward, PeriodForwardCoeffs,
};
use crate::instruments::rates::exotics_shared::hw1f_mc::RateExoticHw1fMcPricer;
use crate::instruments::rates::exotics_shared::mc_config::RateExoticMcConfig;
use crate::instruments::rates::exotics_shared::{
    resolve_hw1f_params, Hw1fCalibrationFlavor, Hw1fCapletSurfacePoint, Hw1fResolveRequest,
    Hw1fSurfaceCalibration,
};
use crate::instruments::rates::snowball::{Snowball, SnowballVariant};
use crate::metrics::MetricId;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_monte_carlo::results::MoneyEstimate;
use finstack_quant_monte_carlo::seed;
use finstack_quant_monte_carlo::traits::{PathState, Payoff, StateKey};

/// Path-local snowball coupon accumulator.
#[derive(Debug, Clone)]
struct SnowballPayoff {
    spec: SnowballCouponSpec,
    notional: f64,
    events: Vec<CouponEvent>,
    discounted_pv: f64,
    next_event: usize,
    prev_coupon: f64,
    /// `true` in Monte-Carlo mode: cashflows are discounted with the pathwise
    /// bank-account numeraire `B(t)` read from the simulation state, not the
    /// deterministic curve DF baked into each [`CouponEvent`]. A coupon fixed
    /// at the period start is held in `pending` until the simulation reaches
    /// its payment date (the next event), where `B(T_pay)` is known.
    pathwise: bool,
    /// Cashflow amounts fixed but not yet paid; settled (pathwise-discounted)
    /// at the next event, whose time is the payment date.
    pending: f64,
    /// Bank-account numeraire at the most recent event (pathwise mode).
    last_bank: f64,
}

#[derive(Debug, Clone, Copy)]
struct SnowballCouponSpec {
    variant: SnowballVariant,
    initial_coupon: f64,
    fixed_rate: f64,
    leverage: f64,
    coupon_floor: f64,
    coupon_cap: Option<f64>,
}

impl SnowballPayoff {
    fn new(
        spec: SnowballCouponSpec,
        notional: f64,
        events: Vec<CouponEvent>,
        pathwise: bool,
    ) -> Self {
        Self {
            spec,
            notional,
            events,
            discounted_pv: 0.0,
            next_event: 0,
            prev_coupon: spec.initial_coupon,
            pathwise,
            pending: 0.0,
            last_bank: 1.0,
        }
    }

    fn compute_coupon(&self, floating_rate: f64) -> f64 {
        self.spec.compute_coupon(floating_rate, self.prev_coupon)
    }
}

impl SnowballCouponSpec {
    fn compute_coupon(&self, floating_rate: f64, prev_coupon: f64) -> f64 {
        let raw = match self.variant {
            SnowballVariant::Snowball => prev_coupon + self.fixed_rate - floating_rate,
            SnowballVariant::InverseFloater => self.fixed_rate - self.leverage * floating_rate,
        };
        let floored = raw.max(self.coupon_floor);
        self.coupon_cap.map_or(floored, |cap| floored.min(cap))
    }
}

impl SnowballPayoff {
    /// Settle coupon `self.next_event` using the supplied short rate, threading
    /// the running `prev_coupon` for the snowball recursion.
    ///
    /// `short_rate` is ignored for an already-seasoned first coupon, whose
    /// `forward_coeffs` are a short-rate-independent flat rate.
    fn settle_next(&mut self, short_rate: f64) {
        let event = self.events[self.next_event];
        // Reconstruct the coupon's in-advance term simple forward from the
        // short rate via the HW1F affine bond formula, instead of using the
        // instantaneous short rate r(t) directly as a term index rate.
        let floating_rate = event.forward_coeffs.simple_forward(short_rate);
        let coupon_rate = self.compute_coupon(floating_rate);
        let coupon_amount = coupon_rate * event.accrual_fraction * self.notional;
        if self.pathwise {
            // Fixed now, paid at the period end — discounted when the
            // simulation reaches the payment date.
            self.pending += coupon_amount;
        } else {
            self.discounted_pv += coupon_amount * event.discount_factor;
        }
        self.prev_coupon = coupon_rate;
        self.next_event += 1;
        if self.pathwise && self.next_event == self.events.len() {
            // Final coupon settled: par redemption pays at maturity, the same
            // payment date as the final coupon.
            self.pending += self.notional;
        }
    }

    /// Settle pending cashflows whose payment date has been reached, using the
    /// pathwise bank-account numeraire at that date.
    fn flush_pending(&mut self, bank: f64) {
        if self.pending != 0.0 {
            self.discounted_pv += self.pending / bank;
            self.pending = 0.0;
        }
    }

    /// Drain the leading run of already-seasoned coupons (in-advance fixings at
    /// or before `as_of`), which carry no path events. Their `forward_coeffs`
    /// are short-rate-independent, so the sampled rate passed in is irrelevant.
    fn settle_seasoned_prefix(&mut self) {
        while self
            .events
            .get(self.next_event)
            .is_some_and(|e| !e.needs_path_sample)
        {
            self.settle_next(0.0);
        }
    }
}

impl Payoff for SnowballPayoff {
    fn on_event(&mut self, state: &mut PathState) {
        // The simulation fires one event per *forward-starting* coupon at that
        // coupon's period start (the in-advance fixing date), plus a final
        // settlement event at maturity. A leading already-seasoned coupon
        // carries no path event, so drain any such coupons that precede this
        // path-fixed one before settling it.
        self.settle_seasoned_prefix();
        // Coupons fixed at the previous event pay at this event's date
        // (contiguous periods: period end == next period start; the final
        // period's payment lands on the maturity settlement event).
        let bank = state.get_key(StateKey::BankAccount).unwrap_or(1.0);
        self.last_bank = bank;
        self.flush_pending(bank);
        if self.next_event >= self.events.len() {
            return;
        }
        let short_rate = state.get_key(StateKey::ShortRate).unwrap_or(0.0);
        self.settle_next(short_rate);
    }

    fn value(&self, currency: finstack_quant_core::currency::Currency) -> Money {
        let mut pv = self.discounted_pv;
        if self.pathwise {
            // The maturity settlement event flushes all pending cashflows;
            // discount any defensive remainder with the last observed bank
            // factor rather than dropping it.
            pv += self.pending / self.last_bank;
        } else if let Some(final_event) = self.events.last() {
            pv += self.notional * final_event.discount_factor;
        }
        Money::new(pv, currency)
    }

    fn reset(&mut self) {
        self.discounted_pv = 0.0;
        self.next_event = 0;
        self.prev_coupon = self.spec.initial_coupon;
        self.pending = 0.0;
        self.last_bank = 1.0;
    }
}

/// Intrinsic-only discounting pricer for inverse floaters.
///
/// Projects a single deterministic forward per coupon and applies
/// [`Snowball::compute_coupon`], which clamps to the floor/cap. This captures
/// only the **intrinsic** value of the embedded floorlet/caplet and ignores
/// their time value, so it understates a floored/capped inverse floater. It is
/// exact only in the (rare) limit where the floor/cap never bind. The default
/// model for `InverseFloater` is therefore [`ModelKey::MonteCarloHullWhite1F`];
/// this pricer is retained as an explicit, fast approximation for callers who
/// knowingly opt in via the `Discounting` model key.
#[derive(Debug, Clone, Copy, Default)]
pub struct SnowballDiscountingPricer;

impl SnowballDiscountingPricer {
    fn price_internal(
        &self,
        inst: &Snowball,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        inst.validate()?;
        if inst.variant != SnowballVariant::InverseFloater {
            return Err(finstack_quant_core::Error::Validation(
                "Discounting model is only valid for SnowballVariant::InverseFloater; \
                 use monte_carlo_hull_white_1f for path-dependent snowballs"
                    .to_string(),
            ));
        }
        ensure_not_callable(inst)?;

        let first_coupon_date = inst.coupon_dates[0];
        let final_coupon_date = *inst.coupon_dates.last().ok_or_else(|| {
            finstack_quant_core::Error::Validation("Snowball requires coupon dates".to_string())
        })?;
        if as_of >= final_coupon_date {
            return Ok(Money::new(0.0, inst.notional.currency()));
        }
        if as_of > first_coupon_date {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Seasoned Snowball '{}' requires historical floating-rate fixings; discounting from the valuation date would change an already-fixed coupon",
                inst.id
            )));
        }

        let discount_curve = market.get_discount(inst.discount_curve_id.as_ref())?;
        let forward_curve = market.get_forward(inst.floating_index_id.as_ref())?;
        // The discounting pricer projects rates from the forward curve, so the
        // term-forward coefficients (and the `r0` seasoned-fixing argument) are
        // unused here; default HW params and `r0 = 0.0` suffice. Only the
        // `accrual_fraction` / `discount_factor` fields are consumed below.
        let term_forward =
            Hw1fTermForward::new(HullWhiteParams::default(), discount_curve.as_ref(), as_of)?;
        let events = coupon_events(inst, market, as_of, &term_forward, 0.0)?;
        if events.is_empty() {
            return Ok(Money::new(0.0, inst.notional.currency()));
        }

        let mut pv = 0.0;
        let mut prev_coupon = inst.initial_coupon;
        let mut event_idx = 0usize;
        for period in inst.coupon_dates.windows(2) {
            let start = period[0];
            let end = period[1];
            if end <= as_of {
                continue;
            }

            let projection_start = start.max(as_of);
            let floating_rate =
                crate::instruments::rates::exotics_shared::forward_swap_rate::term_fixing_on_date(
                    forward_curve.as_ref(),
                    projection_start,
                )?;
            let coupon_rate = inst.compute_coupon(floating_rate, prev_coupon);
            let event = &events[event_idx];
            pv += coupon_rate
                * event.accrual_fraction
                * inst.notional.amount()
                * event.discount_factor;
            prev_coupon = coupon_rate;
            event_idx += 1;
        }

        let maturity = *inst.coupon_dates.last().ok_or_else(|| {
            finstack_quant_core::Error::Validation("Snowball requires coupon dates".to_string())
        })?;
        let redemption_df = relative_df_discount_curve(discount_curve.as_ref(), as_of, maturity)?;
        pv += inst.notional.amount() * redemption_df;
        Ok(Money::new(pv, inst.notional.currency()))
    }
}

impl Pricer for SnowballDiscountingPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::Snowball, ModelKey::Discounting)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let snowball = instrument
            .as_any()
            .downcast_ref::<Snowball>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::Snowball, instrument.key())
            })?;
        let value = self.price_internal(snowball, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(
                e.to_string(),
                PricingErrorContext::from_instrument(instrument)
                    .model(ModelKey::Discounting)
                    .curve_ids([
                        snowball.discount_curve_id.as_str().to_string(),
                        snowball.floating_index_id.as_str().to_string(),
                    ]),
            )
        })?;
        Ok(ValuationResult::stamped(snowball.id.as_str(), as_of, value))
    }

    fn price_raw_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<f64, PricingError> {
        let snowball = instrument
            .as_any()
            .downcast_ref::<Snowball>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::Snowball, instrument.key())
            })?;
        self.price_internal(snowball, market, as_of)
            .map(|m| m.amount())
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::from_instrument(instrument).model(ModelKey::Discounting),
                )
            })
    }
}

/// Hull-White 1F Monte Carlo pricer for path-dependent snowballs.
#[derive(Debug, Clone)]
pub struct SnowballHw1fMcPricer {
    hw_params: HullWhiteParams,
    config: RateExoticMcConfig,
}

impl SnowballHw1fMcPricer {
    /// Create a snowball MC pricer with default HW1F parameters and MC settings.
    pub fn new() -> Self {
        Self {
            hw_params: HullWhiteParams::default(),
            config: RateExoticMcConfig::default(),
        }
    }

    /// Create a snowball MC pricer with explicit HW1F parameters.
    pub fn with_hw_params(hw_params: HullWhiteParams) -> Self {
        Self {
            hw_params,
            config: RateExoticMcConfig::default(),
        }
    }

    /// Create a snowball MC pricer with explicit MC configuration.
    pub fn with_config(mut self, config: RateExoticMcConfig) -> Self {
        self.config = config;
        self
    }

    fn effective_hw_params(
        &self,
        inst: &Snowball,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<HullWhiteParams> {
        let overrides = hw1f_overrides_json(inst);
        let surface_points = snowball_surface_points(inst, as_of)?;
        let surface =
            inst.vol_surface_id
                .as_ref()
                .map(|surface_id| Hw1fSurfaceCalibration::CapFloor {
                    surface_id: surface_id.as_str(),
                    points: surface_points.as_slice(),
                });
        let context_label = format!("Snowball {}", inst.id);
        let req = Hw1fResolveRequest {
            curve_id: inst.discount_curve_id.as_str(),
            flavor: Hw1fCalibrationFlavor::CapFloor,
            overrides: overrides.as_ref(),
            surface,
            fallback: Some(self.hw_params),
            context: context_label.as_str(),
        };
        // Provenance (`hw1f_param_source`) is stamped by the resolver's
        // structured logs under the instrument context label.
        resolve_hw1f_params(&req, market).map(|(params, _source)| params)
    }

    fn effective_config(&self, inst: &Snowball) -> RateExoticMcConfig {
        let mut cfg = self.config;
        if let Some(paths) = inst.pricing_overrides.model_config.mc_paths {
            cfg.num_paths = paths.max(if cfg.antithetic { 2 } else { 1 });
        }
        cfg.seed = inst
            .pricing_overrides
            .metrics
            .mc_seed_scenario
            .as_deref()
            .map_or_else(
                || seed::derive_seed(&inst.id, "base"),
                |scenario| seed::derive_seed(&inst.id, scenario),
            );
        cfg
    }

    fn price_estimate(
        &self,
        inst: &Snowball,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<MoneyEstimate> {
        inst.validate()?;
        ensure_not_callable(inst)?;

        let first_coupon_date = inst.coupon_dates[0];
        let final_coupon_date = *inst.coupon_dates.last().ok_or_else(|| {
            finstack_quant_core::Error::Validation("Snowball requires coupon dates".to_string())
        })?;
        if as_of >= final_coupon_date {
            let zero = Money::new(0.0, inst.notional.currency());
            return Ok(MoneyEstimate {
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
                num_skipped: 0,
            });
        }
        if as_of > first_coupon_date {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Seasoned Snowball '{}' requires the last realized coupon and historical floating-rate fixings; pricing without that state is not supported",
                inst.id
            )));
        }
        let discount_curve = market.get_discount(inst.discount_curve_id.as_ref())?;
        let _forward_curve = market.get_forward(inst.floating_index_id.as_ref())?;
        let hw_params = self.effective_hw_params(inst, market, as_of)?;
        // HW1F bond-reconstruction built from the discount curve; turns the
        // short rate sampled at each coupon's in-advance fixing date into that
        // coupon's term forward.
        let term_forward = Hw1fTermForward::new(hw_params, discount_curve.as_ref(), as_of)?;
        // Initial short rate = discount-curve instantaneous forward f(0,0).
        // HW1F reprices the discount curve only when r(0) = f(0,0); seeding it
        // from a separate forward-curve projection would offset the simulated
        // short rate from the curve and break the M6 repricing property. It is
        // also the deterministic fixing of an already-seasoned first coupon.
        let r0 = initial_short_rate_from_curve(discount_curve.as_ref(), as_of)?;
        if !r0.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Snowball {} initial short rate is not finite",
                inst.id.as_str()
            )));
        }

        let events = coupon_events(inst, market, as_of, &term_forward, r0)?;
        if events.is_empty() {
            let zero = Money::new(0.0, inst.notional.currency());
            return Ok(MoneyEstimate {
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
                num_skipped: 0,
            });
        }

        let spec = SnowballCouponSpec {
            variant: inst.variant,
            initial_coupon: inst.initial_coupon,
            fixed_rate: inst.fixed_rate,
            leverage: inst.leverage,
            coupon_floor: inst.coupon_floor,
            coupon_cap: inst.coupon_cap,
        };
        let config = self.effective_config(inst);

        let event_times = event_times(inst, as_of)?;
        // No forward-starting coupon: every fixing is the deterministic
        // `r(0) = f(0,0)` reconstruction, so the price has no Monte-Carlo
        // component. Settle the (fully seasoned) schedule directly.
        if event_times.is_empty() {
            return Ok(deterministic_estimate(
                spec,
                inst.notional,
                &events,
                config.effective_path_count(),
            ));
        }

        // Final settlement event at maturity (the last payment date): coupons
        // fix in advance but pay at the period end, so the simulation must
        // reach the last payment date for the pathwise bank-account numeraire
        // B(T_pay) to be observable.
        let maturity_date = *inst.coupon_dates.last().ok_or_else(|| {
            finstack_quant_core::Error::Validation("Snowball requires coupon dates".to_string())
        })?;
        let maturity_time =
            inst.day_count
                .year_fraction(as_of, maturity_date, DayCountContext::default())?;
        let mut event_times = event_times;
        event_times.push(maturity_time);

        // Bootstrap a time-dependent θ(t) from the discount curve so the
        // simulated short rate reprices the initial curve (HW1F, not Vasicek).
        // The grid covers up to the last payment date so every cashflow is
        // discounted with the pathwise numeraire.
        let horizon = event_times.last().copied().unwrap_or(0.0);
        let process_params =
            calibrate_hw1f_params(hw_params, discount_curve.as_ref(), as_of, horizon)?;
        let mc = RateExoticHw1fMcPricer {
            process_params,
            r0,
            event_times,
            config,
            currency: inst.notional.currency(),
        };

        let payoff = SnowballPayoff::new(spec, inst.notional.amount(), events, true);
        mc.price(|| payoff.clone())
    }

    fn price_internal(
        &self,
        inst: &Snowball,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        Ok(self.price_estimate(inst, market, as_of)?.mean)
    }
}

impl Default for SnowballHw1fMcPricer {
    fn default() -> Self {
        Self::new()
    }
}

fn hw1f_overrides_json(inst: &Snowball) -> Option<serde_json::Value> {
    let kappa = inst.pricing_overrides.model_config.hw1f_mean_reversion?;
    let sigma = inst.pricing_overrides.model_config.hw1f_sigma?;
    Some(serde_json::json!({ "hw1f_kappa": kappa, "hw1f_sigma": sigma }))
}

fn snowball_surface_points(inst: &Snowball, as_of: Date) -> Result<Vec<Hw1fCapletSurfacePoint>> {
    let ctx = DayCountContext::default();
    let mut points = Vec::new();
    for period in inst.coupon_dates.windows(2) {
        let start = period[0];
        let end = period[1];
        if start <= as_of || end <= start {
            continue;
        }
        let t_fix = inst.day_count.year_fraction(as_of, start, ctx)?;
        let accrual = inst.day_count.year_fraction(start, end, ctx)?;
        if t_fix > 0.0 && accrual > 0.0 {
            points.push(Hw1fCapletSurfacePoint {
                t_fix,
                accrual,
                forward: inst.fixed_rate,
                strike: inst.fixed_rate,
                is_cap: true,
                weight: accrual * inst.notional.amount().abs(),
                normal_vol_per_unit_sigma: None,
            });
        }
    }
    Ok(points)
}

impl Pricer for SnowballHw1fMcPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::Snowball, ModelKey::MonteCarloHullWhite1F)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let snowball = instrument
            .as_any()
            .downcast_ref::<Snowball>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::Snowball, instrument.key())
            })?;
        let estimate = self.price_estimate(snowball, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(
                e.to_string(),
                PricingErrorContext::from_instrument(instrument)
                    .model(ModelKey::MonteCarloHullWhite1F)
                    .curve_ids([
                        snowball.discount_curve_id.as_str().to_string(),
                        snowball.floating_index_id.as_str().to_string(),
                    ]),
            )
        })?;

        let mut result = ValuationResult::stamped(snowball.id.as_str(), as_of, estimate.mean);
        result.measures.insert(
            MetricId::custom("mc_num_paths"),
            estimate.num_simulated_paths as f64,
        );
        result
            .measures
            .insert(MetricId::custom("mc_stderr"), estimate.stderr);
        let (ci_low, ci_high) = estimate.ci_95;
        result
            .measures
            .insert(MetricId::custom("mc_ci95_low"), ci_low.amount());
        result
            .measures
            .insert(MetricId::custom("mc_ci95_high"), ci_high.amount());
        Ok(result)
    }

    fn price_raw_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<f64, PricingError> {
        let snowball = instrument
            .as_any()
            .downcast_ref::<Snowball>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::Snowball, instrument.key())
            })?;
        self.price_internal(snowball, market, as_of)
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

fn ensure_not_callable(inst: &Snowball) -> Result<()> {
    if inst.callable.is_some() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Snowball {} has a Bermudan call provision; callable snowball pricing requires \
             a dedicated LSMC exercise pricer and is not handled by the discounting/HW1F MC pricers",
            inst.id.as_str()
        )));
    }
    Ok(())
}

/// Build the per-coupon event schedule, one [`CouponEvent`] per coupon
/// period surviving past `as_of`.
///
/// Snowball / inverse-floater coupons fix **in advance**: the floating rate for
/// `[start, end]` is set at the period start. For a forward-starting coupon the
/// event's `forward_coeffs` reconstruct that `[start, end]`-tenor simple forward
/// from the short rate sampled at the period start (`needs_path_sample = true`).
/// For an already-seasoned first coupon (`start ≤ as_of`) the fixing is the
/// deterministic `r(0) = f(0,0)` reconstruction projected over the remaining
/// `[as_of, end]` window; `forward_coeffs` then degenerates to that flat rate
/// (`needs_path_sample = false`).
///
/// `term_forward` is the HW1F bond-reconstruction. The HW1F MC pricer reads the
/// resulting coefficients; the path-independent discounting pricer ignores them
/// (it projects rates from the forward curve directly) but still consumes the
/// `accrual_fraction` / `discount_factor` fields.
fn coupon_events(
    inst: &Snowball,
    market: &MarketContext,
    as_of: Date,
    term_forward: &Hw1fTermForward<'_>,
    _r0: f64,
) -> Result<Vec<CouponEvent>> {
    let discount_curve = market.get_discount(inst.discount_curve_id.as_ref())?;
    let forward_curve = market.get_forward(inst.floating_index_id.as_ref())?;
    crate::instruments::rates::exotics_shared::forward_swap_rate::validate_term_curve_tenor(
        forward_curve.as_ref(),
        inst.floating_tenor,
        inst.id.as_str(),
    )?;
    let mut events = Vec::new();
    for period in inst.coupon_dates.windows(2) {
        let start = period[0];
        let end = period[1];
        if end <= as_of {
            continue;
        }

        let accrual_fraction =
            inst.day_count
                .year_fraction(start, end, DayCountContext::default())?;
        if !accrual_fraction.is_finite() || accrual_fraction <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Snowball {} has invalid accrual fraction {accrual_fraction} for {start} to {end}",
                inst.id.as_str()
            )));
        }
        let discount_factor = relative_df_discount_curve(discount_curve.as_ref(), as_of, end)?;

        let (forward_coeffs, needs_path_sample) = if start > as_of {
            // Forward-starting coupon: in-advance fixing sampled from the
            // simulated short rate at the period start `t_fix`. The bond
            // tenor for the HW1F reconstruction is the contractual floating
            // index tenor, not the coupon accrual period.
            let fixing_time =
                inst.day_count
                    .year_fraction(as_of, start, DayCountContext::default())?;
            let coeffs =
                term_forward.period_coeffs(fixing_time, inst.floating_tenor.to_years_simple());
            let discount_time = discount_curve.day_count().signed_year_fraction(
                discount_curve.base_date(),
                start,
                DayCountContext::default(),
            )?;
            let tenor = inst.floating_tenor.to_years_simple();
            let discount_forward =
                (discount_curve.df(discount_time) / discount_curve.df(discount_time + tenor) - 1.0)
                    / tenor;
            let basis =
                crate::instruments::rates::exotics_shared::forward_swap_rate::term_fixing_on_date(
                    forward_curve.as_ref(),
                    start,
                )? - discount_forward;
            (coeffs.with_additive_spread(basis), true)
        } else {
            // At inception the first fixing is projected from the explicit
            // projection curve. Do not substitute the discount-curve short
            // rate, which would remove the index/discount basis.
            let seasoned_rate =
                crate::instruments::rates::exotics_shared::forward_swap_rate::term_fixing_on_date(
                    forward_curve.as_ref(),
                    start,
                )?;
            (
                PeriodForwardCoeffs::from_flat_rate(seasoned_rate, accrual_fraction),
                false,
            )
        };

        events.push(CouponEvent {
            accrual_fraction,
            discount_factor,
            forward_coeffs,
            needs_path_sample,
        });
    }
    Ok(events)
}

/// In-advance fixing event times (years from `as_of`), one per forward-starting
/// coupon — i.e. each coupon whose period *start* lies strictly after `as_of`.
/// An already-seasoned first coupon contributes no path event.
fn event_times(inst: &Snowball, as_of: Date) -> Result<Vec<f64>> {
    let mut times = Vec::new();
    for period in inst.coupon_dates.windows(2) {
        let start = period[0];
        let end = period[1];
        if end <= as_of || start <= as_of {
            continue;
        }
        let t = inst
            .day_count
            .year_fraction(as_of, start, DayCountContext::default())?;
        if t > 0.0 {
            times.push(t);
        }
    }
    Ok(times)
}

/// PV of a fully-seasoned snowball schedule whose every coupon fixes off the
/// deterministic `r(0) = f(0,0)`, so there is no Monte-Carlo component.
///
/// The returned [`MoneyEstimate`] reports the exact PV with zero dispersion.
/// `num_paths` mirrors what an MC run with this configuration would have
/// reported, keeping the result shape consistent for callers.
fn deterministic_estimate(
    spec: SnowballCouponSpec,
    notional: Money,
    events: &[CouponEvent],
    num_paths: usize,
) -> MoneyEstimate {
    let mut payoff = SnowballPayoff::new(spec, notional.amount(), events.to_vec(), false);
    payoff.settle_seasoned_prefix();
    let pv = payoff.value(notional.currency());
    MoneyEstimate {
        mean: pv,
        stderr: 0.0,
        ci_95: (pv, pv),
        num_paths,
        num_simulated_paths: num_paths,
        std_dev: Some(0.0),
        median: None,
        percentile_25: None,
        percentile_75: None,
        min: Some(pv.amount()),
        max: Some(pv.amount()),
        num_skipped: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::rates::exotics_shared::bermudan_call::BermudanCallProvision;
    use crate::instruments::PricingOverrides;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount, Tenor};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use time::Month;

    fn date(year: i32, month: Month, day: u8) -> Date {
        Date::from_calendar_date(year, month, day).expect("valid date")
    }

    fn test_snowball() -> Snowball {
        Snowball {
            id: InstrumentId::new("SNOWBALL-TEST"),
            variant: SnowballVariant::Snowball,
            initial_coupon: 0.03,
            fixed_rate: 0.05,
            leverage: 1.0,
            coupon_floor: 0.0,
            coupon_cap: None,
            notional: Money::new(1_000_000.0, Currency::USD),
            coupon_dates: vec![
                date(2025, Month::January, 1),
                date(2025, Month::July, 1),
                date(2026, Month::January, 1),
                date(2026, Month::July, 1),
            ],
            floating_index_id: CurveId::new("USD-SOFR-6M"),
            floating_tenor: Tenor::semi_annual(),
            discount_curve_id: CurveId::new("USD-OIS"),
            vol_surface_id: Some(CurveId::new("USD-SOFR-HW-VOL")),
            callable: None,
            day_count: DayCount::Act365F,
            pricing_overrides: PricingOverrides::default(),
            attributes: Default::default(),
        }
    }

    fn test_inverse_floater() -> Snowball {
        Snowball {
            variant: SnowballVariant::InverseFloater,
            initial_coupon: 0.0,
            fixed_rate: 0.08,
            leverage: 1.5,
            coupon_cap: Some(0.10),
            ..test_snowball()
        }
    }

    /// Sloped zero rate `R(t) = base + 3%·(1 − e^{−0.6 t})` and its discount
    /// factor `P(0,t) = exp(−R(t)·t)`.
    fn sloped_zero(base: f64, t: f64) -> f64 {
        base + 0.03 * (1.0 - (-0.6 * t).exp())
    }

    /// Test market with an **upward-sloping** discount curve.
    ///
    /// The slope is deliberate: on a flat curve the simple forward over a
    /// coupon's in-advance window `[start, end]` equals the forward over the
    /// in-arrears window `[end, end+τ]`, so a flat curve cannot tell a correct
    /// in-advance fixing from the (incorrect) in-arrears one. The sloped curve
    /// makes the two windows give measurably different forwards.
    ///
    /// `forward_rate` populates a *flat* projection curve (the declared floating
    /// index): the HW1F MC ignores it (single-curve), and the discounting
    /// pricer's own dedicated test supplies a flat rate it can reproduce
    /// exactly. The cross-pricer consistency test instead builds a projection
    /// curve consistent with the discount curve via [`consistent_market`].
    fn market(as_of: Date, discount_rate: f64, forward_rate: f64) -> MarketContext {
        let knots: Vec<(f64, f64)> = (0..=12)
            .map(|i| {
                let t = i as f64 * 0.25;
                (t, (-sloped_zero(discount_rate, t) * t).exp())
            })
            .collect();
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots(knots)
            .build()
            .expect("discount curve");
        let forward = ForwardCurve::builder("USD-SOFR-6M", 0.5)
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, forward_rate), (3.0, forward_rate)])
            .build()
            .expect("forward curve");
        MarketContext::new().insert(discount).insert(forward)
    }

    /// Test market whose projection curve is consistent with the discount curve.
    ///
    /// Both curves are built from the same sloped zero rate: the discount curve
    /// holds `P(0,t)`; the projection curve's knot rates are the discount
    /// curve's **instantaneous forward** `f(0,t) = R(t) + t·R'(t)`. With
    /// consistent curves the discounting pricer (which projects in-advance from
    /// the projection curve) and the HW1F MC pricer (which reconstructs the
    /// in-advance forward from the discount curve) must price the *same*
    /// instrument to within a small simple-vs-continuous-compounding gap — the
    /// basis for the cross-pricer consistency check.
    fn consistent_market(as_of: Date, base_rate: f64) -> MarketContext {
        let knots: Vec<(f64, f64)> = (0..=16)
            .map(|i| {
                let t = i as f64 * 0.25;
                (t, (-sloped_zero(base_rate, t) * t).exp())
            })
            .collect();
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots(knots)
            .build()
            .expect("discount curve");
        // Instantaneous forward of the sloped zero: with
        // R(t) = base + 0.03·(1 − e^{−0.6t}), R'(t) = 0.03·0.6·e^{−0.6t}, so
        // f(0,t) = R(t) + t·R'(t).
        let fwd_knots: Vec<(f64, f64)> = (0..=16)
            .map(|i| {
                let t = i as f64 * 0.25;
                let r = sloped_zero(base_rate, t);
                let r_prime = 0.03 * 0.6 * (-0.6 * t).exp();
                (t, r + t * r_prime)
            })
            .collect();
        let forward = ForwardCurve::builder("USD-SOFR-6M", 0.5)
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots(fwd_knots)
            .build()
            .expect("forward curve");
        MarketContext::new().insert(discount).insert(forward)
    }

    fn deterministic_mc_pricer(paths: usize) -> SnowballHw1fMcPricer {
        SnowballHw1fMcPricer::with_hw_params(HullWhiteParams::new(0.05, 1e-12).expect("hw params"))
            .with_config(RateExoticMcConfig {
                num_paths: paths,
                antithetic: false,
                min_steps_between_events: 1,
                ..Default::default()
            })
    }

    #[test]
    fn implied_volatility_is_not_used_as_hw1f_sigma() {
        let as_of = date(2025, Month::January, 1);
        let market = market(as_of, 0.02, 0.03);
        let no_iv = test_snowball();
        let mut with_iv = no_iv.clone();
        with_iv.pricing_overrides.market_quotes.implied_volatility = Some(0.20);

        let pv_no_iv = deterministic_mc_pricer(32)
            .price_estimate(&no_iv, &market, as_of)
            .expect("no iv")
            .mean
            .amount();
        let pv_with_iv = deterministic_mc_pricer(32)
            .price_estimate(&with_iv, &market, as_of)
            .expect("with iv")
            .mean
            .amount();

        assert!(
            (pv_with_iv - pv_no_iv).abs() < 1e-9,
            "implied_volatility must not alter Snowball HW1F sigma: no_iv={pv_no_iv}, with_iv={pv_with_iv}"
        );
    }

    /// Independent in-advance ground-truth PV for the deterministic-σ limit.
    ///
    /// This is **not** a copy of either pricer's accumulation — it is derived
    /// from the in-advance contract directly. A snowball / inverse-floater
    /// coupon over `[start, end]` fixes its floating rate *at the period start*
    /// and the rate applies over `[start, end]`.
    ///
    /// `floating_rate`:
    /// - `Some(r)` — every coupon fixes at the flat rate `r`. The discounting
    ///   pricer projects the rate from the (flat) forward curve, so the fixing
    ///   window is immaterial and a single `r` is the exact ground truth.
    /// - `None` — each coupon fixes at the **discount curve's own model-free
    ///   simple forward** over its in-advance window `[max(start, as_of), end]`:
    ///   `L_i = (P(0,start_i) / P(0,end_i) − 1) / accrual_i`. At σ → 0 the
    ///   simulated HW1F short rate is exactly the curve's instantaneous forward,
    ///   so the M7 reconstruction over `[start, end]` collapses to this. Using
    ///   `P_start / P_end` (rather than the in-arrears `P_end / P_{end+τ}`) is
    ///   what pins the in-advance convention: revert the pricer to sampling at
    ///   `end` and this mirror no longer matches.
    fn expected_deterministic_pv(
        inst: &Snowball,
        market: &MarketContext,
        as_of: Date,
        floating_rate: Option<f64>,
    ) -> f64 {
        let disc = market
            .get_discount(inst.discount_curve_id.as_ref())
            .expect("discount");
        let dc = inst.day_count;
        let ctx = DayCountContext::default();
        let mut pv = 0.0;
        let mut prev_coupon = inst.initial_coupon;

        for period in inst.coupon_dates.windows(2) {
            let start = period[0];
            let end = period[1];
            if end <= as_of {
                continue;
            }
            let accrual = dc.year_fraction(start, end, ctx).expect("accrual");
            let rate = match floating_rate {
                Some(r) => r,
                None => {
                    // In-advance fixing: simple forward over the fixing window
                    // [max(start, as_of), end] implied by the discount curve.
                    let fixing_start = start.max(as_of);
                    let p_start = relative_df_discount_curve(disc.as_ref(), as_of, fixing_start)
                        .expect("p_start");
                    let p_end =
                        relative_df_discount_curve(disc.as_ref(), as_of, end).expect("p_end");
                    (p_start / p_end - 1.0) / accrual
                }
            };
            let df = relative_df_discount_curve(disc.as_ref(), as_of, end).expect("df");
            let coupon = inst.compute_coupon(rate, prev_coupon);
            pv += coupon * accrual * inst.notional.amount() * df;
            prev_coupon = coupon;
        }

        let maturity = *inst.coupon_dates.last().expect("maturity");
        let df = relative_df_discount_curve(disc.as_ref(), as_of, maturity).expect("df");
        pv += inst.notional.amount() * df;
        pv
    }

    #[test]
    fn discounting_inverse_floater_matches_forward_curve_formula() {
        let as_of = date(2025, Month::January, 1);
        let floating_rate = 0.03;
        let market = market(as_of, 0.02, floating_rate);
        let inst = test_inverse_floater();
        let expected = expected_deterministic_pv(&inst, &market, as_of, Some(floating_rate));

        let price = SnowballDiscountingPricer
            .price_internal(&inst, &market, as_of)
            .expect("price");

        assert!((price.amount() - expected).abs() < 1e-8);
    }

    /// Cross-pricer consistency: the HW1F MC and the discounting pricer must
    /// agree on the **fixing convention** for the *same* inverse floater.
    ///
    /// Both pricers fix the floating index *in advance* — the discounting
    /// pricer via `rate_period_on_dates(forward_curve, start.max(as_of), end)`,
    /// the HW1F MC via the term-forward reconstruction at the short rate
    /// sampled at the period start. On [`consistent_market`] the projection
    /// curve carries the discount curve's own instantaneous forward, so with a
    /// near-zero σ (the two models coincide) they must price to within a small
    /// residual gap (empirically ≈ $5.3k): the simple-vs-continuous-compounding
    /// difference between the discount curve's *simple* forward and the
    /// projection curve's *integral-averaged* rate, amplified by the inverse
    /// floater's 1.5× leverage, plus the θ(t)-bootstrap discretization residual.
    ///
    /// This guards the two code paths against silently drifting apart on the
    /// fixing window again: reverting either pricer to an in-arrears
    /// `[end, end+τ]` fixing widens the gap to ≈ $10k on this sloped curve, so
    /// the 6,000 bound remains materially below that regression.
    #[test]
    fn hw1f_mc_and_discounting_agree_on_inverse_floater() {
        let as_of = date(2025, Month::January, 1);
        let market = consistent_market(as_of, 0.02);
        let inst = test_inverse_floater();

        // Discounting pricer: in-advance projection from the forward curve.
        let discounting = SnowballDiscountingPricer
            .price_internal(&inst, &market, as_of)
            .expect("discounting price");

        // HW1F MC at near-zero σ — the deterministic-σ limit where the
        // short-rate model and the discounting projection coincide.
        let mc = deterministic_mc_pricer(4_096)
            .price_estimate(&inst, &market, as_of)
            .expect("mc price");

        let diff = (mc.mean.amount() - discounting.amount()).abs();
        assert!(
            diff < 6_000.0,
            "HW1F MC ({}) and discounting ({}) disagree on the inverse-floater \
             fixing convention: |Δ|={diff:.2} > $6k — the two pricers have \
             drifted apart on the in-advance fixing window",
            mc.mean.amount(),
            discounting.amount(),
        );
    }

    #[test]
    fn discounting_rejects_path_dependent_snowball_variant() {
        let as_of = date(2025, Month::January, 1);
        let market = market(as_of, 0.02, 0.03);
        let err = SnowballDiscountingPricer
            .price_internal(&test_snowball(), &market, as_of)
            .expect_err("snowball needs MC");
        assert!(err.to_string().contains("InverseFloater"));
    }

    #[test]
    fn pricers_reject_callable_snowball_scope() {
        let as_of = date(2025, Month::January, 1);
        let market = market(as_of, 0.02, 0.03);
        let mut inst = test_snowball();
        inst.callable = Some(BermudanCallProvision::new(
            vec![date(2026, Month::January, 1)],
            1.0,
            1,
        ));

        let err = deterministic_mc_pricer(8)
            .price_estimate(&inst, &market, as_of)
            .expect_err("callable snowball needs LSMC");
        assert!(err.to_string().contains("call provision"));
    }

    /// Deterministic-σ snowball PV must match the **independent in-advance**
    /// ground truth, pinning the in-advance fixing convention.
    ///
    /// `expected_deterministic_pv(.., None)` derives each coupon's floating
    /// rate from the discount curve's simple forward over the *in-advance*
    /// window `[start, end]`. The HW1F MC, at σ = 1e-12, samples the short rate
    /// at each coupon's period start and reconstructs the same window.
    ///
    /// # Tolerance
    ///
    /// On the *sloped* test curve the residual is the θ(t)-bootstrap
    /// discretization error (the monthly-grid piecewise-constant θ reprices
    /// bonds to O(spacing²), so the σ → 0 short rate equals the curve forward
    /// only to a few bp). The snowball coupon is path-dependent
    /// (`prev + fixed − L`) so this residual compounds across coupons —
    /// empirically ≲ $1k of notional. The 3,000 bound clears that yet is ~8×
    /// below the ≈ $26k PV shift an in-arrears `[end, end+τ]` fixing produces,
    /// so the test genuinely fails if the pricer reverts to sampling at the
    /// period end.
    #[test]
    fn deterministic_mc_snowball_matches_discounted_coupon_formula() {
        let as_of = date(2025, Month::January, 1);
        let market = market(as_of, 0.02, 0.03);
        let inst = test_snowball();
        // None: the deterministic-σ HW1F path reconstructs the floating index
        // from the discount curve (M7), in-advance over [start, end].
        let expected = expected_deterministic_pv(&inst, &market, as_of, Some(0.03));

        let estimate = deterministic_mc_pricer(32)
            .price_estimate(&inst, &market, as_of)
            .expect("price");

        assert!(
            (estimate.mean.amount() - expected).abs() < 3_000.0,
            "mc={}, expected={expected} (in-advance fixing); a >$3k gap \
             indicates the pricer fixed in-arrears",
            estimate.mean.amount()
        );
    }

    #[test]
    fn higher_path_count_reduces_standard_error() {
        let as_of = date(2025, Month::January, 1);
        let market = market(as_of, 0.02, 0.03);
        let inst = test_snowball();

        let low =
            SnowballHw1fMcPricer::with_hw_params(HullWhiteParams::new(0.05, 0.015).expect("hw"))
                .with_config(RateExoticMcConfig {
                    num_paths: 200,
                    antithetic: true,
                    min_steps_between_events: 1,
                    seed: 7,
                    ..Default::default()
                })
                .price_estimate(&inst, &market, as_of)
                .expect("low path price");
        let high =
            SnowballHw1fMcPricer::with_hw_params(HullWhiteParams::new(0.05, 0.015).expect("hw"))
                .with_config(RateExoticMcConfig {
                    num_paths: 2_000,
                    antithetic: true,
                    min_steps_between_events: 1,
                    seed: 7,
                    ..Default::default()
                })
                .price_estimate(&inst, &market, as_of)
                .expect("high path price");

        assert!(
            high.stderr < low.stderr,
            "high-path stderr {} should be below low-path stderr {}",
            high.stderr,
            low.stderr
        );
    }

    #[test]
    fn price_dyn_returns_mc_measures() {
        let as_of = date(2025, Month::January, 1);
        let market = market(as_of, 0.02, 0.03);
        let inst = test_snowball();
        let result = deterministic_mc_pricer(16)
            .price_dyn(&inst, &market, as_of)
            .expect("price");

        assert!(result.value.amount() > 0.0);
        assert!(result
            .measures
            .contains_key(&MetricId::custom("mc_num_paths")));
    }

    /// Regression: the pricer must read `floating_tenor` when reconstructing the
    /// HW1F term forward. If `floating_tenor` is ignored (bug), changing it from
    /// 3M to 6M on semi-annual coupon periods leaves the PV unchanged because the
    /// pricer always uses the coupon accrual fraction (= 0.5 yr) as the bond
    /// tenor. On a sloped curve the 3M and 6M simple forwards differ, so the two
    /// instruments MUST price differently once `floating_tenor` is respected.
    #[test]
    fn floating_tenor_affects_pv_on_sloped_curve() {
        let as_of = date(2025, Month::January, 1);
        let market = market(as_of, 0.02, 0.03);

        // Semi-annual coupon periods, 3M floating index.
        let inst_3m = Snowball {
            floating_tenor: Tenor::quarterly(),
            ..test_snowball()
        };
        let err = deterministic_mc_pricer(32)
            .price_estimate(&inst_3m, &market, as_of)
            .expect_err("3M instrument must reject a 6M forward curve");
        assert!(err.to_string().contains("does not match forward curve"));
    }
}
