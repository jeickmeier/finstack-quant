//! Hull-White 1F Monte Carlo pricer for TARNs.

use crate::calibration::hull_white::HullWhiteParams;
use crate::instruments::common_impl::pricing::time::relative_df_discount_curve;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::exotics_shared::cumulative_coupon::{
    CouponEvent, CumulativeCouponTracker,
};
use crate::instruments::rates::exotics_shared::hw1f_curve::{
    calibrate_hw1f_params, initial_short_rate_from_curve, Hw1fTermForward, PeriodForwardCoeffs,
};
use crate::instruments::rates::exotics_shared::hw1f_mc::RateExoticHw1fMcPricer;
use crate::instruments::rates::exotics_shared::mc_config::RateExoticMcConfig;
use crate::instruments::rates::tarn::Tarn;
use crate::metrics::MetricId;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext, PricingResult,
};
use crate::results::ValuationResult;
use finstack_core::dates::{Date, DayCountContext};
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::Result;
use finstack_monte_carlo::results::MoneyEstimate;
use finstack_monte_carlo::seed;
use finstack_monte_carlo::traits::{PathState, Payoff, StateKey};
use std::sync::Arc;

/// Path-local TARN payoff accumulator.
///
/// The immutable coupon-event schedule is shared across all simulated paths
/// via `Arc`, so per-path payoff clones only bump the reference count instead
/// of deep-copying the event vector.
#[derive(Debug, Clone)]
struct TarnPayoff {
    fixed_rate: f64,
    coupon_floor: f64,
    notional: f64,
    events: Arc<[CouponEvent]>,
    tracker: CumulativeCouponTracker,
    discounted_pv: f64,
    next_event: usize,
    redeemed: bool,
}

impl TarnPayoff {
    fn new(
        fixed_rate: f64,
        coupon_floor: f64,
        target_coupon: f64,
        notional: f64,
        events: Arc<[CouponEvent]>,
    ) -> Self {
        Self {
            fixed_rate,
            coupon_floor,
            notional,
            events,
            tracker: CumulativeCouponTracker::with_target(target_coupon),
            discounted_pv: 0.0,
            next_event: 0,
            redeemed: false,
        }
    }

    fn add_redemption(&mut self, event: &CouponEvent) {
        if !self.redeemed {
            self.discounted_pv += self.notional * event.discount_factor;
            self.redeemed = true;
        }
    }

    /// Settle coupon `self.next_event` using the supplied short rate, advancing
    /// the cumulative-coupon tracker and redeeming on knockout.
    ///
    /// `short_rate` is ignored for an already-seasoned first coupon, whose
    /// `forward_coeffs` are a short-rate-independent flat rate.
    fn settle_next(&mut self, short_rate: f64) {
        let event = self.events[self.next_event];
        // Reconstruct the coupon's in-advance term simple forward from the
        // short rate via the HW1F affine bond formula, instead of using the
        // instantaneous short rate r(t) directly as a term index rate.
        let floating_rate = event.forward_coeffs.simple_forward(short_rate);
        let coupon_rate = (self.fixed_rate - floating_rate).max(self.coupon_floor);
        let period_coupon = coupon_rate * event.accrual_fraction;
        let actual_coupon = self.tracker.add_coupon(period_coupon);

        self.discounted_pv += actual_coupon * self.notional * event.discount_factor;
        if self.tracker.is_knocked_out() {
            self.add_redemption(&event);
        }
        self.next_event += 1;
    }

    /// Drain the leading run of already-seasoned coupons (in-advance fixings at
    /// or before `as_of`), which carry no path events. Their `forward_coeffs`
    /// are short-rate-independent, so the sampled rate passed in is irrelevant.
    fn settle_seasoned_prefix(&mut self) {
        while self.next_event < self.events.len()
            && !self.events[self.next_event].needs_path_sample
            && !self.redeemed
        {
            self.settle_next(0.0);
        }
    }

    /// Settle every coupon deterministically (no Monte-Carlo path), for a
    /// schedule whose coupons are all already seasoned.
    fn settle_all_seasoned(&mut self) {
        self.settle_seasoned_prefix();
        debug_assert_eq!(
            self.next_event,
            self.events.len(),
            "settle_all_seasoned called with a non-seasoned coupon present",
        );
    }
}

impl Payoff for TarnPayoff {
    fn on_event(&mut self, state: &mut PathState) {
        // The simulation fires one event per *forward-starting* coupon, at that
        // coupon's period start (the in-advance fixing date). A leading
        // already-seasoned coupon carries no path event, so drain any such
        // coupons that precede this path-fixed one before settling it.
        self.settle_seasoned_prefix();
        if self.next_event >= self.events.len() || self.redeemed {
            return;
        }
        let short_rate = state.get_key(StateKey::ShortRate).unwrap_or(0.0);
        self.settle_next(short_rate);
    }

    fn value(&self, currency: finstack_core::currency::Currency) -> Money {
        let mut pv = self.discounted_pv;
        if !self.redeemed {
            if let Some(final_event) = self.events.last() {
                pv += self.notional * final_event.discount_factor;
            }
        }
        Money::new(pv, currency)
    }

    fn reset(&mut self) {
        self.tracker.reset();
        self.discounted_pv = 0.0;
        self.next_event = 0;
        self.redeemed = false;
    }
}

/// TARN pricer using short-rate paths from the shared HW1F Monte Carlo harness.
#[derive(Debug, Clone)]
pub struct TarnPricer {
    hw_params: HullWhiteParams,
    config: RateExoticMcConfig,
}

impl TarnPricer {
    /// Create a TARN pricer with default HW1F parameters and MC settings.
    pub fn new() -> Self {
        Self {
            hw_params: HullWhiteParams::default(),
            config: RateExoticMcConfig::default(),
        }
    }

    /// Create a TARN pricer with explicit HW1F parameters.
    pub fn with_hw_params(hw_params: HullWhiteParams) -> Self {
        Self {
            hw_params,
            config: RateExoticMcConfig::default(),
        }
    }

    /// Create a TARN pricer with explicit MC configuration.
    pub fn with_config(mut self, config: RateExoticMcConfig) -> Self {
        self.config = config;
        self
    }

    fn effective_hw_params(&self, inst: &Tarn) -> Result<HullWhiteParams> {
        let kappa = inst
            .pricing_overrides
            .model_config
            .mean_reversion
            .unwrap_or(self.hw_params.kappa);
        let sigma = inst
            .pricing_overrides
            .market_quotes
            .implied_volatility
            .unwrap_or(self.hw_params.sigma);
        HullWhiteParams::new(kappa, sigma)
    }

    fn effective_config(&self, inst: &Tarn) -> RateExoticMcConfig {
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
        inst: &Tarn,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<MoneyEstimate> {
        inst.validate()?;

        let discount_curve = market.get_discount(inst.discount_curve_id.as_ref())?;
        // The HW1F MC path is single-curve: both the period term forwards (M7)
        // and the simulated short rate are reconstructed from `discount_curve`.
        // The declared floating index is still required to exist in the market
        // as an instrument-contract precondition, but is not otherwise read.
        let _forward_curve = market.get_forward(inst.floating_index_id.as_ref())?;

        let hw_params = self.effective_hw_params(inst)?;
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
            return Err(finstack_core::Error::Validation(format!(
                "TARN {} initial short rate is not finite",
                inst.id.as_str()
            )));
        }

        let mut events = Vec::new();
        let mut event_times = Vec::new();

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
                return Err(finstack_core::Error::Validation(format!(
                    "TARN {} has invalid accrual fraction {accrual_fraction} for {start} to {end}",
                    inst.id.as_str()
                )));
            }

            let discount_factor = relative_df_discount_curve(discount_curve.as_ref(), as_of, end)?;

            // TARN coupons fix in advance: the floating rate is set at the
            // period start and applies over `[start, end]`.
            let forward_coeffs = if start > as_of {
                // Forward-starting coupon: the fixing is sampled from the
                // simulated short rate at the period start `t_fix`; the index
                // is the `[start, end]`-tenor simple forward. `t_fix` is the
                // event time the simulation fires `on_event` at.
                let fixing_time =
                    inst.day_count
                        .year_fraction(as_of, start, DayCountContext::default())?;
                if !fixing_time.is_finite() || fixing_time <= 0.0 {
                    return Err(finstack_core::Error::Validation(format!(
                        "TARN {} has invalid fixing time {fixing_time} for period start {start}",
                        inst.id.as_str()
                    )));
                }
                event_times.push(fixing_time);
                events.push(CouponEvent {
                    accrual_fraction,
                    discount_factor,
                    forward_coeffs: term_forward
                        .period_coeffs(fixing_time, inst.floating_tenor.to_years_simple()),
                    needs_path_sample: true,
                });
                continue;
            } else {
                // Already-seasoned first coupon: its in-advance fixing date is
                // at or before `as_of`, so the rate is the deterministic
                // `r(0) = f(0,0)` reconstruction. Mirroring the discounting
                // pricer, the rate is projected over the *remaining* window
                // `[as_of, end]` while the coupon still accrues over the full
                // `[start, end]`. A flat (short-rate-independent) coeff set
                // bakes that rate in; the event consumes no path sample.
                let seasoned_rate = term_forward
                    .period_coeffs(0.0, inst.floating_tenor.to_years_simple())
                    .simple_forward(r0);
                PeriodForwardCoeffs::from_flat_rate(seasoned_rate, accrual_fraction)
            };

            events.push(CouponEvent {
                accrual_fraction,
                discount_factor,
                forward_coeffs,
                needs_path_sample: false,
            });
        }

        if events.is_empty() {
            return Err(finstack_core::Error::Validation(format!(
                "TARN {} has no future coupon period after {as_of}",
                inst.id.as_str()
            )));
        }

        let config = self.effective_config(inst);

        // No forward-starting coupon: every fixing is the deterministic
        // `r(0) = f(0,0)` reconstruction, so the price has no Monte-Carlo
        // component. Settle the (fully seasoned) schedule directly.
        if event_times.is_empty() {
            return Ok(deterministic_estimate(
                inst,
                &events,
                config.effective_path_count(),
            ));
        }

        // Bootstrap a time-dependent θ(t) from the discount curve so the
        // simulated short rate reprices the initial curve (HW1F, not Vasicek).
        // The grid covers up to the last fixing — no coupon fixes after it and
        // redemption discounting is deterministic.
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

        let payoff = TarnPayoff::new(
            inst.fixed_rate,
            inst.coupon_floor,
            inst.target_coupon,
            inst.notional.amount(),
            Arc::from(events),
        );
        mc.price(|| payoff.clone())
    }

    fn price_internal(&self, inst: &Tarn, market: &MarketContext, as_of: Date) -> Result<Money> {
        Ok(self.price_estimate(inst, market, as_of)?.mean)
    }
}

impl Default for TarnPricer {
    fn default() -> Self {
        Self::new()
    }
}

/// PV of a fully-seasoned TARN schedule whose every coupon fixes off the
/// deterministic `r(0) = f(0,0)`, so there is no Monte-Carlo component.
///
/// The returned [`MoneyEstimate`] reports the exact PV with zero dispersion.
/// `num_paths` mirrors what an MC run with this configuration would have
/// reported, keeping the result shape consistent for callers.
fn deterministic_estimate(inst: &Tarn, events: &[CouponEvent], num_paths: usize) -> MoneyEstimate {
    let mut payoff = TarnPayoff::new(
        inst.fixed_rate,
        inst.coupon_floor,
        inst.target_coupon,
        inst.notional.amount(),
        Arc::from(events.to_vec()),
    );
    payoff.settle_all_seasoned();
    let pv = payoff.value(inst.notional.currency());
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

impl Pricer for TarnPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::Tarn, ModelKey::MonteCarloHullWhite1F)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> PricingResult<ValuationResult> {
        let tarn = instrument
            .as_any()
            .downcast_ref::<Tarn>()
            .ok_or_else(|| PricingError::type_mismatch(InstrumentType::Tarn, instrument.key()))?;

        let estimate = self.price_estimate(tarn, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(
                e.to_string(),
                PricingErrorContext::from_instrument(instrument)
                    .model(ModelKey::MonteCarloHullWhite1F)
                    .curve_ids([
                        tarn.discount_curve_id.as_str().to_string(),
                        tarn.floating_index_id.as_str().to_string(),
                    ]),
            )
        })?;

        let mut result = ValuationResult::stamped(tarn.id.as_str(), as_of, estimate.mean);
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
    ) -> PricingResult<f64> {
        let tarn = instrument
            .as_any()
            .downcast_ref::<Tarn>()
            .ok_or_else(|| PricingError::type_mismatch(InstrumentType::Tarn, instrument.key()))?;
        self.price_internal(tarn, market, as_of)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::PricingOverrides;
    use finstack_core::currency::Currency;
    use finstack_core::dates::{Date, DayCount, Tenor};
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
    use finstack_core::types::{CurveId, InstrumentId};
    use time::Month;

    fn date(year: i32, month: Month, day: u8) -> Date {
        Date::from_calendar_date(year, month, day).expect("valid date")
    }

    fn test_tarn(target_coupon: f64) -> Tarn {
        Tarn {
            id: InstrumentId::new("TARN-TEST"),
            fixed_rate: 0.06,
            coupon_floor: 0.0,
            target_coupon,
            notional: Money::new(1_000_000.0, Currency::USD),
            coupon_dates: vec![
                date(2025, Month::January, 1),
                date(2025, Month::July, 1),
                date(2026, Month::January, 1),
                date(2026, Month::July, 1),
            ],
            floating_tenor: Tenor::semi_annual(),
            floating_index_id: CurveId::new("USD-SOFR-6M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            day_count: DayCount::Act365F,
            pricing_overrides: PricingOverrides::default(),
            attributes: Default::default(),
        }
    }

    /// Test market with an **upward-sloping** discount curve.
    ///
    /// The slope is deliberate: on a flat curve the simple forward over a
    /// coupon's in-advance window `[start, end]` equals the forward over the
    /// in-arrears window `[end, end+τ]`, so a flat curve cannot tell a correct
    /// in-advance fixing from the (incorrect) in-arrears one. Here the zero rate
    /// climbs from `discount_rate` at the short end to `discount_rate + 3%`,
    /// making the two windows give measurably different forwards. Knots run out
    /// to 3y so even the in-arrears `[end, end+τ]` reconstruction stays on-curve.
    fn market(as_of: Date, discount_rate: f64, forward_rate: f64) -> MarketContext {
        let knots: Vec<(f64, f64)> = (0..=12)
            .map(|i| {
                let t = i as f64 * 0.25;
                let zero = discount_rate + 0.03 * (1.0 - (-0.6 * t).exp());
                (t, (-zero * t).exp())
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

    fn deterministic_pricer(paths: usize) -> TarnPricer {
        TarnPricer::with_hw_params(HullWhiteParams::new(0.05, 1e-12).expect("hw params"))
            .with_config(RateExoticMcConfig {
                num_paths: paths,
                antithetic: false,
                min_steps_between_events: 1,
                ..Default::default()
            })
    }

    /// Independent in-advance ground-truth PV for the deterministic-σ limit.
    ///
    /// This is **not** a copy of the pricer's accumulation — it is derived from
    /// the in-advance contract directly. A TARN coupon over `[start, end]` fixes
    /// its floating rate *at the period start* and the rate applies over
    /// `[start, end]`. At σ → 0 the simulated short rate is exactly the curve's
    /// instantaneous forward, so the realised floating rate is the **discount
    /// curve's own model-free simple forward** over the fixing window:
    ///
    /// ```text
    /// L_i = (P(0,start_i) / P(0,end_i) − 1) / accrual_i.
    /// ```
    ///
    /// For an already-seasoned first coupon (`start ≤ as_of`) the fixing window
    /// is the remaining `[as_of, end]` — mirroring how the snowball discounting
    /// pricer clamps the projection start to `as_of` — while accrual stays the
    /// full `[start, end]`. Coupons are run through an independent cumulative
    /// tracker for the knockout, and each cashflow is discounted with the curve
    /// DF. Computing `L_i` from `P_start / P_end` (rather than the pricer's
    /// `P_end / P_{end+τ}`) is what pins the *in-advance* convention: if the
    /// pricer reverts to sampling at `end`, this mirror no longer matches.
    fn expected_deterministic_pv(tarn: &Tarn, market: &MarketContext, as_of: Date) -> f64 {
        let disc = market
            .get_discount(tarn.discount_curve_id.as_ref())
            .expect("discount");
        let dc = tarn.day_count;
        let ctx = DayCountContext::default();
        let mut tracker = CumulativeCouponTracker::with_target(tarn.target_coupon);
        let mut pv = 0.0;
        let mut redeemed = false;

        for period in tarn.coupon_dates.windows(2) {
            let start = period[0];
            let end = period[1];
            if end <= as_of {
                continue;
            }
            let accrual = dc.year_fraction(start, end, ctx).expect("accrual");
            // In-advance fixing: simple forward over the fixing window
            // [max(start, as_of), end] implied by the discount curve.
            let fixing_start = start.max(as_of);
            let p_start =
                relative_df_discount_curve(disc.as_ref(), as_of, fixing_start).expect("p_start");
            let p_end = relative_df_discount_curve(disc.as_ref(), as_of, end).expect("p_end");
            let floating_rate = (p_start / p_end - 1.0) / accrual;

            let coupon = (tarn.fixed_rate - floating_rate).max(tarn.coupon_floor) * accrual;
            let actual = tracker.add_coupon(coupon);
            pv += actual * tarn.notional.amount() * p_end;
            if tracker.is_knocked_out() {
                pv += tarn.notional.amount() * p_end;
                redeemed = true;
                break;
            }
        }

        if !redeemed {
            let maturity = *tarn.coupon_dates.last().expect("maturity");
            let df = relative_df_discount_curve(disc.as_ref(), as_of, maturity).expect("df");
            pv += tarn.notional.amount() * df;
        }
        pv
    }

    #[test]
    fn payoff_caps_final_coupon_and_redeems() {
        // Fixed 1% floating index via degenerate (B=0) reconstruction coeffs:
        // exercises payoff mechanics (coupon cap + knock-out) only. Both
        // coupons are path-fixed so each consumes one `on_event`.
        let coeffs = PeriodForwardCoeffs::from_flat_rate(0.01, 1.0);
        let events = vec![
            CouponEvent {
                accrual_fraction: 1.0,
                discount_factor: 1.0,
                forward_coeffs: coeffs,
                needs_path_sample: true,
            },
            CouponEvent {
                accrual_fraction: 1.0,
                discount_factor: 1.0,
                forward_coeffs: coeffs,
                needs_path_sample: true,
            },
        ];
        let mut payoff = TarnPayoff::new(0.06, 0.0, 0.10, 1_000_000.0, Arc::from(events));

        let mut state = PathState::new(0, 1.0);
        state.set_key(StateKey::ShortRate, 0.01);
        payoff.on_event(&mut state);
        payoff.on_event(&mut state);

        assert!((payoff.value(Currency::USD).amount() - 1_100_000.0).abs() < 1e-8);
    }

    /// Deterministic-σ PV must match the **independent in-advance** ground
    /// truth, pinning the in-advance fixing convention.
    ///
    /// `expected_deterministic_pv` derives each coupon's floating rate from the
    /// discount curve's simple forward over the *in-advance* window
    /// `[start, end]`. The HW1F MC, at σ = 1e-12, samples the short rate at each
    /// coupon's period start and reconstructs the same `[start, end]` forward.
    ///
    /// # Tolerance
    ///
    /// On the *sloped* test curve the residual is the θ(t)-bootstrap
    /// discretization error: the monthly-grid piecewise-constant θ reprices
    /// bonds to O(spacing²), so the σ → 0 short rate equals the curve's
    /// instantaneous forward only to a few bp — empirically ≲ $700 of notional
    /// here. The 2,000 bound clears that residual yet is ~5× below the ≈ $11k
    /// PV shift an in-arrears `[end, end+τ]` fixing would produce, so the test
    /// genuinely fails if the pricer reverts to sampling at the period end.
    #[test]
    fn deterministic_path_matches_discounted_coupon_formula() {
        let as_of = date(2025, Month::January, 1);
        let market = market(as_of, 0.02, 0.03);
        let tarn = test_tarn(1.0);
        let expected = expected_deterministic_pv(&tarn, &market, as_of);

        let estimate = deterministic_pricer(32)
            .price_estimate(&tarn, &market, as_of)
            .expect("price");

        assert!(
            (estimate.mean.amount() - expected).abs() < 2_000.0,
            "mc={}, expected={} (in-advance fixing); a >$2k gap indicates the \
             pricer fixed in-arrears",
            estimate.mean.amount(),
            expected
        );
    }

    #[test]
    fn zero_target_redeems_on_first_coupon_date() {
        let as_of = date(2025, Month::January, 1);
        let market = market(as_of, 0.02, 0.03);
        let tarn = test_tarn(0.0);
        let expected = expected_deterministic_pv(&tarn, &market, as_of);

        let estimate = deterministic_pricer(16)
            .price_estimate(&tarn, &market, as_of)
            .expect("price");

        assert!((estimate.mean.amount() - expected).abs() < 1.0);
        let first_coupon_df = market
            .get_discount("USD-OIS")
            .expect("discount")
            .df_between_dates(as_of, tarn.coupon_dates[1])
            .expect("df");
        assert!((expected - tarn.notional.amount() * first_coupon_df).abs() < 1e-8);
    }

    #[test]
    fn higher_path_count_reduces_standard_error() {
        let as_of = date(2025, Month::January, 1);
        let market = market(as_of, 0.02, 0.03);
        let tarn = test_tarn(1.0);

        let low = TarnPricer::with_hw_params(HullWhiteParams::new(0.05, 0.015).expect("hw"))
            .with_config(RateExoticMcConfig {
                num_paths: 200,
                antithetic: true,
                min_steps_between_events: 1,
                seed: 7,
                ..Default::default()
            })
            .price_estimate(&tarn, &market, as_of)
            .expect("low path price");
        let high = TarnPricer::with_hw_params(HullWhiteParams::new(0.05, 0.015).expect("hw"))
            .with_config(RateExoticMcConfig {
                num_paths: 2_000,
                antithetic: true,
                min_steps_between_events: 1,
                seed: 7,
                ..Default::default()
            })
            .price_estimate(&tarn, &market, as_of)
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
        let tarn = test_tarn(1.0);
        let result = deterministic_pricer(16)
            .price_dyn(&tarn, &market, as_of)
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
        let tarn_3m = Tarn {
            floating_tenor: Tenor::quarterly(),
            ..test_tarn(1.0)
        };
        // Same instrument but 6M floating index.
        let tarn_6m = test_tarn(1.0); // already floating_tenor = semi_annual()

        let pv_3m = deterministic_pricer(32)
            .price_estimate(&tarn_3m, &market, as_of)
            .expect("3M price")
            .mean
            .amount();
        let pv_6m = deterministic_pricer(32)
            .price_estimate(&tarn_6m, &market, as_of)
            .expect("6M price")
            .mean
            .amount();

        // On a sloped curve the 3M and 6M simple forwards differ materially.
        // A PV shift of at least $100 is expected; if the two are equal (within
        // $1) the pricer is ignoring floating_tenor.
        assert!(
            (pv_3m - pv_6m).abs() > 100.0,
            "pv_3m={pv_3m:.2} pv_6m={pv_6m:.2}: |Δ|={:.2} ≤ $100 — \
             the pricer is ignoring floating_tenor",
            (pv_3m - pv_6m).abs()
        );
    }
}
