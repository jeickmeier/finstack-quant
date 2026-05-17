//! Pricers for snowball and inverse-floater structured notes.

use crate::calibration::hull_white::HullWhiteParams;
use crate::instruments::common_impl::pricing::time::{
    rate_period_on_dates, relative_df_discount_curve,
};
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::exotics_shared::hw1f_curve::{
    calibrate_hw1f_params, initial_short_rate_from_curve, Hw1fTermForward, PeriodForwardCoeffs,
};
use crate::instruments::rates::exotics_shared::hw1f_mc::RateExoticHw1fMcPricer;
use crate::instruments::rates::exotics_shared::mc_config::RateExoticMcConfig;
use crate::instruments::rates::snowball::{Snowball, SnowballVariant};
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

#[derive(Debug, Clone, Copy)]
struct SnowballCouponEvent {
    accrual_fraction: f64,
    discount_factor: f64,
    /// HW1F bond-reconstruction coefficients for the period's term forward.
    ///
    /// The coupon's floating index is the τ-tenor simple forward (not the raw
    /// instantaneous short rate); this turns the simulated `r(t)` at the event
    /// date into that term rate via the affine HW1F bond formula.
    forward_coeffs: PeriodForwardCoeffs,
}

/// Path-local snowball coupon accumulator.
#[derive(Debug, Clone)]
struct SnowballPayoff {
    spec: SnowballCouponSpec,
    notional: f64,
    events: Vec<SnowballCouponEvent>,
    discounted_pv: f64,
    next_event: usize,
    prev_coupon: f64,
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
    fn new(spec: SnowballCouponSpec, notional: f64, events: Vec<SnowballCouponEvent>) -> Self {
        Self {
            spec,
            notional,
            events,
            discounted_pv: 0.0,
            next_event: 0,
            prev_coupon: spec.initial_coupon,
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

impl Payoff for SnowballPayoff {
    fn on_event(&mut self, state: &mut PathState) {
        if self.next_event >= self.events.len() {
            return;
        }

        let event = self.events[self.next_event];
        // Reconstruct the period's term simple forward from the simulated
        // short rate via the HW1F affine bond formula, instead of using the
        // instantaneous short rate r(t) directly as a term index rate.
        let short_rate = state.get_key(StateKey::ShortRate).unwrap_or(0.0);
        let floating_rate = event.forward_coeffs.simple_forward(short_rate);
        let coupon_rate = self.compute_coupon(floating_rate);
        self.discounted_pv +=
            coupon_rate * event.accrual_fraction * self.notional * event.discount_factor;
        self.prev_coupon = coupon_rate;
        self.next_event += 1;
    }

    fn value(&self, currency: finstack_core::currency::Currency) -> Money {
        let mut pv = self.discounted_pv;
        if let Some(final_event) = self.events.last() {
            pv += self.notional * final_event.discount_factor;
        }
        Money::new(pv, currency)
    }

    fn reset(&mut self) {
        self.discounted_pv = 0.0;
        self.next_event = 0;
        self.prev_coupon = self.spec.initial_coupon;
    }
}

/// Discounting pricer for path-independent inverse floaters.
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
            return Err(finstack_core::Error::Validation(
                "Discounting model is only valid for SnowballVariant::InverseFloater; \
                 use monte_carlo_hull_white_1f for path-dependent snowballs"
                    .to_string(),
            ));
        }
        ensure_not_callable(inst)?;

        let discount_curve = market.get_discount(inst.discount_curve_id.as_ref())?;
        let forward_curve = market.get_forward(inst.floating_index_id.as_ref())?;
        // The discounting pricer projects rates from the forward curve, so the
        // term-forward coefficients are unused here; default HW params suffice.
        let term_forward =
            Hw1fTermForward::new(HullWhiteParams::default(), discount_curve.as_ref(), as_of)?;
        let events = coupon_events(inst, market, as_of, &term_forward)?;
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
                rate_period_on_dates(forward_curve.as_ref(), projection_start, end)?;
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
            finstack_core::Error::Validation("Snowball requires coupon dates".to_string())
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
    ) -> PricingResult<ValuationResult> {
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
    ) -> PricingResult<f64> {
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

    fn effective_hw_params(&self, inst: &Snowball) -> Result<HullWhiteParams> {
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
        let events = coupon_events(inst, market, as_of, &term_forward)?;
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

        // A future coupon period must exist for the simulation to have any
        // events; `event_times` below would otherwise be empty.
        first_future_period(inst, as_of)?;
        // Initial short rate = discount-curve instantaneous forward f(0,0).
        // HW1F reprices the discount curve only when r(0) = f(0,0); seeding it
        // from a separate forward-curve projection would offset the simulated
        // short rate from the curve and break the M6 repricing property.
        let r0 = initial_short_rate_from_curve(discount_curve.as_ref(), as_of)?;
        if !r0.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Snowball {} initial short rate is not finite",
                inst.id.as_str()
            )));
        }

        let event_times = event_times(inst, as_of)?;
        // Bootstrap a time-dependent θ(t) from the discount curve so the
        // simulated short rate reprices the initial curve (HW1F, not Vasicek).
        let horizon = event_times.last().copied().unwrap_or(0.0);
        let process_params =
            calibrate_hw1f_params(hw_params, discount_curve.as_ref(), as_of, horizon)?;
        let mc = RateExoticHw1fMcPricer {
            process_params,
            r0,
            event_times,
            config: self.effective_config(inst),
            currency: inst.notional.currency(),
        };

        let payoff = SnowballPayoff::new(
            SnowballCouponSpec {
                variant: inst.variant,
                initial_coupon: inst.initial_coupon,
                fixed_rate: inst.fixed_rate,
                leverage: inst.leverage,
                coupon_floor: inst.coupon_floor,
                coupon_cap: inst.coupon_cap,
            },
            inst.notional.amount(),
            events,
        );
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

impl Pricer for SnowballHw1fMcPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::Snowball, ModelKey::MonteCarloHullWhite1F)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> PricingResult<ValuationResult> {
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
    ) -> PricingResult<f64> {
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
        return Err(finstack_core::Error::Validation(format!(
            "Snowball {} has a Bermudan call provision; callable snowball pricing requires \
             a dedicated LSMC exercise pricer and is not handled by the discounting/HW1F MC pricers",
            inst.id.as_str()
        )));
    }
    Ok(())
}

/// Build the per-coupon event schedule.
///
/// `term_forward` is the HW1F bond-reconstruction used to populate each event's
/// term-forward coefficients. The HW1F MC pricer reads those coefficients; the
/// path-independent discounting pricer ignores them (it projects rates from the
/// forward curve directly) but still supplies a reconstruction for one code
/// path.
fn coupon_events(
    inst: &Snowball,
    market: &MarketContext,
    as_of: Date,
    term_forward: &Hw1fTermForward<'_>,
) -> Result<Vec<SnowballCouponEvent>> {
    let discount_curve = market.get_discount(inst.discount_curve_id.as_ref())?;
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
            return Err(finstack_core::Error::Validation(format!(
                "Snowball {} has invalid accrual fraction {accrual_fraction} for {start} to {end}",
                inst.id.as_str()
            )));
        }
        let event_time = inst
            .day_count
            .year_fraction(as_of, end, DayCountContext::default())?;
        let discount_factor = relative_df_discount_curve(discount_curve.as_ref(), as_of, end)?;
        events.push(SnowballCouponEvent {
            accrual_fraction,
            discount_factor,
            forward_coeffs: term_forward.period_coeffs(event_time, accrual_fraction),
        });
    }
    Ok(events)
}

fn event_times(inst: &Snowball, as_of: Date) -> Result<Vec<f64>> {
    let mut times = Vec::new();
    for &date in inst.coupon_dates.iter().skip(1) {
        if date <= as_of {
            continue;
        }
        let t = inst
            .day_count
            .year_fraction(as_of, date, DayCountContext::default())?;
        if t > 0.0 {
            times.push(t);
        }
    }
    Ok(times)
}

fn first_future_period(inst: &Snowball, as_of: Date) -> Result<(Date, Date)> {
    inst.coupon_dates
        .windows(2)
        .find_map(|period| {
            let start = period[0];
            let end = period[1];
            (end > as_of).then_some((start.max(as_of), end))
        })
        .ok_or_else(|| {
            finstack_core::Error::Validation(format!(
                "Snowball {} has no future coupon period after {as_of}",
                inst.id.as_str()
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::rates::exotics_shared::bermudan_call::BermudanCallProvision;
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

    fn deterministic_mc_pricer(paths: usize) -> SnowballHw1fMcPricer {
        SnowballHw1fMcPricer::with_hw_params(HullWhiteParams::new(0.05, 1e-12).expect("hw params"))
            .with_config(RateExoticMcConfig {
                num_paths: paths,
                antithetic: false,
                min_steps_between_events: 1,
                ..Default::default()
            })
    }

    /// Expected deterministic-path PV.
    ///
    /// `floating_rate`:
    /// - `Some(r)` — every period uses the flat rate `r` (the discounting
    ///   pricer, which projects from the forward curve).
    /// - `None` — each period uses the discount curve's τ-tenor simple forward
    ///   observed at the coupon's event date `end`. This is the deterministic-σ
    ///   limit of the HW1F MC path with the M6/M7 fixes (the simulated short
    ///   rate collapses to the curve forward, and M7 reconstructs the term
    ///   rate from the discount curve).
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
            let accrual = dc.year_fraction(start, end, ctx).expect("accrual");
            let rate = match floating_rate {
                Some(r) => r,
                None => {
                    // Term forward over [end, end+accrual] from the curve.
                    let t_end = dc.year_fraction(as_of, end, ctx).expect("t_end");
                    let p_end = disc.df(t_end);
                    let p_end_tau = disc.df(t_end + accrual);
                    (p_end / p_end_tau - 1.0) / accrual
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

    #[test]
    fn deterministic_mc_snowball_matches_discounted_coupon_formula() {
        let as_of = date(2025, Month::January, 1);
        let market = market(as_of, 0.02, 0.03);
        let inst = test_snowball();
        // None: the deterministic-σ HW1F path reconstructs the floating index
        // from the discount curve (M7), not from the forward curve.
        let expected = expected_deterministic_pv(&inst, &market, as_of, None);

        let estimate = deterministic_mc_pricer(32)
            .price_estimate(&inst, &market, as_of)
            .expect("price");

        assert!(
            (estimate.mean.amount() - expected).abs() < 1.0,
            "mc={}, expected={expected}",
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
}
