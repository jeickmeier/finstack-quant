//! Hull-White 1F Monte Carlo pricer for TARNs.

use crate::calibration::hull_white::HullWhiteParams;
use crate::instruments::common_impl::pricing::time::relative_df_discount_curve;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::exotics_shared::cumulative_coupon::CumulativeCouponTracker;
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

#[derive(Debug, Clone, Copy)]
struct TarnCouponEvent {
    accrual_fraction: f64,
    discount_factor: f64,
    /// HW1F bond-reconstruction coefficients for the period's term forward.
    ///
    /// The coupon's floating index is the τ-tenor simple forward (not the raw
    /// instantaneous short rate); this turns the simulated `r(t)` at the event
    /// date into that term rate via the affine HW1F bond formula.
    forward_coeffs: PeriodForwardCoeffs,
}

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
    events: Arc<[TarnCouponEvent]>,
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
        events: Arc<[TarnCouponEvent]>,
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

    fn add_redemption(&mut self, event: &TarnCouponEvent) {
        if !self.redeemed {
            self.discounted_pv += self.notional * event.discount_factor;
            self.redeemed = true;
        }
    }
}

impl Payoff for TarnPayoff {
    fn on_event(&mut self, state: &mut PathState) {
        if self.next_event >= self.events.len() || self.redeemed {
            return;
        }

        let event = self.events[self.next_event];
        // Reconstruct the period's term simple forward from the simulated
        // short rate via the HW1F affine bond formula, instead of using the
        // instantaneous short rate r(t) directly as a term index rate.
        let short_rate = state.get_key(StateKey::ShortRate).unwrap_or(0.0);
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
        // simulated short rate at each event into the period term forward.
        let term_forward = Hw1fTermForward::new(hw_params, discount_curve.as_ref(), as_of)?;

        let mut events = Vec::new();
        let mut event_times = Vec::new();
        let mut has_future_period = false;

        for period in inst.coupon_dates.windows(2) {
            let start = period[0];
            let end = period[1];
            if end <= as_of {
                continue;
            }
            has_future_period = true;

            let event_time =
                inst.day_count
                    .year_fraction(as_of, end, DayCountContext::default())?;
            if event_time <= 0.0 {
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
            events.push(TarnCouponEvent {
                accrual_fraction,
                discount_factor,
                forward_coeffs: term_forward.period_coeffs(event_time, accrual_fraction),
            });
            event_times.push(event_time);
        }

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

        if !has_future_period {
            return Err(finstack_core::Error::Validation(format!(
                "TARN {} has no future coupon period after {as_of}",
                inst.id.as_str()
            )));
        }

        // Initial short rate = discount-curve instantaneous forward f(0,0).
        // HW1F reprices the discount curve only when r(0) = f(0,0); seeding it
        // from a separate forward-curve projection would offset the simulated
        // short rate from the curve and break the M6 repricing property.
        let r0 = initial_short_rate_from_curve(discount_curve.as_ref(), as_of)?;
        if !r0.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "TARN {} initial short rate is not finite",
                inst.id.as_str()
            )));
        }

        // Bootstrap a time-dependent θ(t) from the discount curve so the
        // simulated short rate reprices the initial curve (HW1F, not Vasicek).
        let horizon = event_times.last().copied().unwrap_or(0.0);
        let process_params =
            calibrate_hw1f_params(hw_params, discount_curve.as_ref(), as_of, horizon)?;
        let config = self.effective_config(inst);
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

    fn market(as_of: Date, discount_rate: f64, forward_rate: f64) -> MarketContext {
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
        let forward = ForwardCurve::builder("USD-SOFR-6M", 0.5)
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, forward_rate), (1.5, forward_rate)])
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

    /// Expected deterministic-path PV.
    ///
    /// With a near-zero σ the simulated HW1F short rate collapses to the
    /// curve's instantaneous forward, so the M7 term-forward reconstruction
    /// (built on the discount curve) returns the curve's own τ-tenor simple
    /// forward observed at each coupon's event date `end`. This mirror
    /// reconstructs that period forward exactly the way the pricer does.
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
            let accrual = dc.year_fraction(start, end, ctx).expect("accrual");
            // Term forward over [end, end+accrual] reconstructed from the
            // discount curve — the deterministic-σ limit of the HW1F M7 path.
            let t_end = dc.year_fraction(as_of, end, ctx).expect("t_end");
            let p_end = disc.df(t_end);
            let p_end_tau = disc.df(t_end + accrual);
            let floating_rate = (p_end / p_end_tau - 1.0) / accrual;

            let df = relative_df_discount_curve(disc.as_ref(), as_of, end).expect("df");
            let coupon = (tarn.fixed_rate - floating_rate).max(tarn.coupon_floor) * accrual;
            let actual = tracker.add_coupon(coupon);
            pv += actual * tarn.notional.amount() * df;
            if tracker.is_knocked_out() {
                pv += tarn.notional.amount() * df;
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
        // exercises payoff mechanics (coupon cap + knock-out) only.
        let coeffs = PeriodForwardCoeffs::from_flat_rate(0.01, 1.0);
        let events = vec![
            TarnCouponEvent {
                accrual_fraction: 1.0,
                discount_factor: 1.0,
                forward_coeffs: coeffs,
            },
            TarnCouponEvent {
                accrual_fraction: 1.0,
                discount_factor: 1.0,
                forward_coeffs: coeffs,
            },
        ];
        let mut payoff = TarnPayoff::new(0.06, 0.0, 0.10, 1_000_000.0, Arc::from(events));

        let mut state = PathState::new(0, 1.0);
        state.set_key(StateKey::ShortRate, 0.01);
        payoff.on_event(&mut state);
        payoff.on_event(&mut state);

        assert!((payoff.value(Currency::USD).amount() - 1_100_000.0).abs() < 1e-8);
    }

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
            (estimate.mean.amount() - expected).abs() < 1.0,
            "mc={}, expected={}",
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
}
