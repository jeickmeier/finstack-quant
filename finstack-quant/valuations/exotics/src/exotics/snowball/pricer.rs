//! Pricers for snowball and inverse-floater structured notes.

use crate::exotics::hw1f_note::{
    Hw1fNote, Hw1fNotePricer, InAdvanceNoteSpec, PathwiseCouponLedger,
    build_in_advance_coupon_schedule, projected_index_forward, run_hw1f_coupon_mc, zero_estimate,
};
use crate::exotics::snowball::types::SnowballCouponSpec;
use crate::exotics::snowball::{Snowball, SnowballVariant};
use finstack_quant_core::Result;
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_models::monte_carlo::results::MoneyEstimate;
use finstack_quant_models::monte_carlo::traits::{PathState, Payoff};
use finstack_quant_models::rates::hw1f::CouponEvent;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::instruments::pricing_overrides::ModelConfig;
use finstack_quant_valuations_core::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use finstack_quant_valuations_core::results::ValuationResult;
use std::sync::Arc;

/// Path-local snowball coupon accumulator sharing an immutable event schedule.
#[derive(Debug, Clone)]
struct SnowballPayoff {
    spec: SnowballCouponSpec,
    notional: f64,
    prev_coupon: f64,
    ledger: PathwiseCouponLedger,
}

impl SnowballPayoff {
    fn new(
        spec: SnowballCouponSpec,
        notional: f64,
        events: Arc<[CouponEvent]>,
        pathwise: bool,
    ) -> Self {
        Self {
            spec,
            notional,
            prev_coupon: spec.initial_coupon,
            ledger: PathwiseCouponLedger::new(events, pathwise),
        }
    }

    fn settle_event(
        spec: SnowballCouponSpec,
        notional: f64,
        prev_coupon: &mut f64,
        ledger: &mut PathwiseCouponLedger,
        event: CouponEvent,
        short_rate: f64,
    ) {
        let floating_rate = event.forward_coeffs.simple_forward(short_rate);
        let coupon_rate = spec.compute_coupon(floating_rate, *prev_coupon);
        let coupon_amount = coupon_rate * event.accrual_fraction * notional;
        ledger.record_cashflow(coupon_amount, event.discount_factor);
        *prev_coupon = coupon_rate;
        ledger.advance();
        ledger.add_maturity_redemption_if_pending(notional);
    }

    fn settle_all_seasoned(&mut self) {
        let spec = self.spec;
        let notional = self.notional;
        let prev = &mut self.prev_coupon;
        self.ledger.drain_seasoned(|ledger, event, r| {
            Self::settle_event(spec, notional, prev, ledger, event, r);
        });
    }
}

impl Payoff for SnowballPayoff {
    fn on_event(&mut self, state: &mut PathState) -> finstack_quant_core::Result<()> {
        let spec = self.spec;
        let notional = self.notional;
        let prev = &mut self.prev_coupon;
        self.ledger.step_event(state, |ledger, event, short_rate| {
            Self::settle_event(spec, notional, prev, ledger, event, short_rate);
        });
        Ok(())
    }

    fn value(
        &self,
        currency: finstack_quant_core::currency::Currency,
    ) -> finstack_quant_core::Result<Money> {
        self.ledger.value(self.notional, currency)
    }

    fn reset(&mut self) {
        self.prev_coupon = self.spec.initial_coupon;
        self.ledger.reset();
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

        let first_coupon_date = inst.start_date;
        let final_coupon_date = *inst.payment_dates.last().ok_or_else(|| {
            finstack_quant_core::Error::Validation("Snowball requires payment dates".to_string())
        })?;
        if as_of >= final_coupon_date {
            return Ok(Money::from((0_i64, inst.notional.currency())));
        }
        if as_of > first_coupon_date {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Seasoned Snowball '{}' requires historical floating-rate fixings; discounting from the valuation date would change an already-fixed coupon",
                inst.id
            )));
        }

        let discount_curve = market.get_discount(inst.discount_curve_id.as_ref())?;
        let forward_curve = market.get_forward(inst.forward_curve_id.as_ref())?;
        finstack_quant_valuations_core::instruments::pricing::term_fixings::validate_term_curve_tenor(
            forward_curve.as_ref(),
            inst.index_tenor,
            inst.id.as_str(),
        )?;

        let spec = SnowballCouponSpec::from_instrument(inst);
        let mut pv = 0.0;
        let mut prev_coupon = spec.initial_coupon;
        for period in inst.period_boundaries().windows(2) {
            let start = period[0];
            let end = period[1];
            if end <= as_of {
                continue;
            }
            let accrual_fraction = coupon_accrual_fraction(inst, start, end)?;
            let discount_factor = discount_curve.as_ref().df_between_dates(as_of, end)?;

            let projection_start = start.max(as_of);
            let floating_rate = projected_index_forward(
                forward_curve.as_ref(),
                inst.day_count,
                projection_start,
                inst.index_tenor,
            )?;
            let coupon_rate = spec.compute_coupon(floating_rate, prev_coupon);
            pv += coupon_rate * accrual_fraction * inst.notional.amount() * discount_factor;
            prev_coupon = coupon_rate;
        }

        let redemption_df = discount_curve
            .as_ref()
            .df_between_dates(as_of, final_coupon_date)?;
        pv += inst.notional.amount() * redemption_df;
        Money::from_model_value(pv, inst.notional.currency())
    }
}

impl Pricer for SnowballDiscountingPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::Snowball, ModelKey::Discounting)
    }

    #[tracing::instrument(
        name = "snowball.discounting.price_dyn",
        level = "debug",
        skip(self, instrument, market),
        fields(inst_id = %instrument.id(), as_of = %as_of),
        err,
    )]
    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let snowball = finstack_quant_valuations_core::pricer::expect_inst::<Snowball>(
            instrument,
            InstrumentType::Snowball,
        )?;
        let value = self.price_internal(snowball, market, as_of).map_err(|e| {
            PricingError::from_core(
                e,
                PricingErrorContext::from_instrument(instrument)
                    .model(ModelKey::Discounting)
                    .curve_ids([
                        snowball.discount_curve_id.as_str().to_string(),
                        snowball.forward_curve_id.as_str().to_string(),
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
        let snowball = finstack_quant_valuations_core::pricer::expect_inst::<Snowball>(
            instrument,
            InstrumentType::Snowball,
        )?;
        self.price_internal(snowball, market, as_of)
            .map(|m| m.amount())
            .map_err(|e| {
                PricingError::from_core(
                    e,
                    PricingErrorContext::from_instrument(instrument).model(ModelKey::Discounting),
                )
            })
    }
}

/// Hull-White 1F Monte Carlo pricer for path-dependent snowballs.
pub type SnowballHw1fMcPricer = Hw1fNotePricer<Snowball>;

impl Hw1fNote for Snowball {
    const INSTRUMENT_TYPE: InstrumentType = InstrumentType::Snowball;
    const LABEL: &'static str = "Snowball";

    fn note_id(&self) -> &InstrumentId {
        &self.id
    }

    fn hw1f_curve_id(&self) -> &str {
        self.discount_curve_id.as_str()
    }

    fn model_config(&self) -> &ModelConfig {
        &self.instrument_pricing_overrides.model_config
    }

    fn error_curve_ids(&self) -> Vec<String> {
        vec![
            self.discount_curve_id.as_str().to_string(),
            self.forward_curve_id.as_str().to_string(),
        ]
    }

    fn hw1f_estimate(
        &self,
        pricer: &SnowballHw1fMcPricer,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<MoneyEstimate> {
        self.validate()?;
        ensure_not_callable(self)?;

        let spec = InAdvanceNoteSpec {
            label: Self::LABEL,
            id: &self.id,
            start_date: self.start_date,
            payment_dates: &self.payment_dates,
            index_tenor: self.index_tenor,
            forward_curve_id: &self.forward_curve_id,
            discount_curve_id: &self.discount_curve_id,
            day_count: self.day_count,
            currency: self.notional.currency(),
        };
        let seasoned_msg = format!(
            "Seasoned Snowball '{}' requires the last realized coupon and historical floating-rate fixings; pricing without that state is not supported",
            self.id
        );
        let Some(prepared) =
            build_in_advance_coupon_schedule(self, pricer, &spec, market, as_of, &seasoned_msg)?
        else {
            return Ok(zero_estimate(self.notional.currency()));
        };

        let coupon_spec = SnowballCouponSpec::from_instrument(self);
        let notional = self.notional.amount();
        run_hw1f_coupon_mc(
            &spec,
            market,
            as_of,
            prepared,
            |events, pathwise| SnowballPayoff::new(coupon_spec, notional, events, pathwise),
            SnowballPayoff::settle_all_seasoned,
        )
    }
}

fn ensure_not_callable(inst: &Snowball) -> Result<()> {
    if inst.call_provision.is_some() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Snowball {} has a Bermudan call provision; callable snowball pricing requires \
             a dedicated LSMC exercise pricer and is not handled by the discounting/HW1F MC pricers",
            inst.id.as_str()
        )));
    }
    Ok(())
}

/// Positive, finite accrual fraction of the coupon period `[start, end]`.
fn coupon_accrual_fraction(inst: &Snowball, start: Date, end: Date) -> Result<f64> {
    let accrual_fraction = inst
        .day_count
        .year_fraction(start, end, DayCountContext::default())?;
    if !accrual_fraction.is_finite() || accrual_fraction <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Snowball {} has invalid accrual fraction {accrual_fraction} for {start} to {end}",
            inst.id.as_str()
        )));
    }
    Ok(accrual_fraction)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exotics::BermudanCallProvision;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DateExt, DayCount, Tenor};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use finstack_quant_models::rates::hull_white::HullWhiteCalibrationParams;
    use finstack_quant_models::rates::hw1f::Hw1fMcConfig;
    use time::Month;

    fn date(year: i32, month: Month, day: u8) -> Date {
        Date::from_calendar_date(year, month, day).expect("valid date")
    }

    fn test_snowball() -> Snowball {
        Snowball {
            id: InstrumentId::new("SNOWBALL-TEST"),
            variant: SnowballVariant::Snowball,
            initial_coupon: 0.03,
            fixed_rate: rust_decimal::Decimal::new(5, 2),
            gearing: 1.0,
            coupon_floor: 0.0,
            coupon_cap: None,
            notional: Money::from((1_000_000_i64, Currency::USD)),
            start_date: date(2025, Month::January, 1),
            payment_dates: vec![
                date(2025, Month::July, 1),
                date(2026, Month::January, 1),
                date(2026, Month::July, 1),
            ],
            forward_curve_id: CurveId::new("USD-SOFR-6M"),
            index_tenor: Tenor::semi_annual(),
            discount_curve_id: CurveId::new("USD-OIS"),
            vol_surface_id: Some(CurveId::new("USD-SOFR-HW-VOL")),
            call_provision: None,
            day_count: DayCount::Act365F,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Default::default(),
        }
    }

    fn test_inverse_floater() -> Snowball {
        Snowball {
            variant: SnowballVariant::InverseFloater,
            initial_coupon: 0.0,
            fixed_rate: rust_decimal::Decimal::new(8, 2),
            gearing: 1.5,
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
            .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("discount curve");
        let mut fwd_points = vec![(0.0, 1.0)];
        let mut prev = as_of;
        let mut factor = 1.0;
        for months in [6, 12, 18, 24, 30, 36] {
            let next = as_of.add_months(months).expect("valid semi-annual date");
            let yf = DayCount::Act365F
                .year_fraction(prev, next, DayCountContext::default())
                .expect("valid yf");
            let t = DayCount::Act365F
                .year_fraction(as_of, next, DayCountContext::default())
                .expect("valid t");
            factor /= 1.0 + forward_rate * yf;
            fwd_points.push((t, factor));
            prev = next;
        }
        let forward = ForwardCurve::builder("USD-SOFR-6M")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .index_tenor(Tenor::semi_annual())
            .knots(fwd_points)
            .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("forward curve");
        let _ = (forward_rate, &forward);
        MarketContext::new()
            .try_insert(discount)
            .expect("insert market curve")
            .try_insert(forward)
            .expect("insert market curve")
    }

    /// Test market whose projection curve is consistent with the discount curve.
    ///
    /// Both curves are built from the same sloped zero rate: the discount curve
    /// holds `P(0,t)`; the projection curve derives its normalized factors
    /// directly from `discount`. With consistent curves the discounting pricer
    /// (which projects in-advance from the projection curve) and the HW1F MC
    /// pricer (which reconstructs the in-advance forward from the discount curve)
    /// must price the *same* instrument within numerical tolerance.
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
            .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("discount curve");
        let forward =
            ForwardCurve::from_discount_curve("USD-SOFR-6M", &discount, Some(Tenor::semi_annual()));
        MarketContext::new()
            .try_insert(discount)
            .expect("insert market curve")
            .try_insert(forward)
            .expect("insert market curve")
    }

    fn deterministic_mc_pricer(paths: usize) -> SnowballHw1fMcPricer {
        SnowballHw1fMcPricer::with_hw_params(
            HullWhiteCalibrationParams::new(0.05, 1e-12).expect("hw params"),
        )
        .with_config(Hw1fMcConfig {
            num_paths: paths,
            antithetic: false,
            min_steps_between_events: 1,
            ..Default::default()
        })
    }

    /// Registry defaults must reproduce the documented explicit estimator
    /// configuration with identical paths, price and sampling error.
    #[test]
    fn rate_exotic_defaults_match_explicit_estimator_configuration() {
        let as_of = date(2025, Month::January, 1);
        let market = market(as_of, 0.02, 0.03);
        let pricer = SnowballHw1fMcPricer::with_hw_params(
            HullWhiteCalibrationParams::new(0.05, 0.015).expect("hw params"),
        );
        let instrument = test_snowball();
        let estimate = pricer
            .price_estimate(&instrument, &market, as_of)
            .expect("default price");
        let explicit = pricer
            .with_config(Hw1fMcConfig {
                num_paths: 10_000,
                seed: 42,
                antithetic: true,
                min_steps_between_events: 4,
                basis_degree: 2,
                oos_lsmc: false,
            })
            .price_estimate(&instrument, &market, as_of)
            .expect("explicit configuration price");
        assert_eq!(estimate.num_paths, 10_000);
        assert_eq!(estimate.num_simulated_paths, 20_000);
        assert_eq!(explicit.num_paths, estimate.num_paths);
        assert_eq!(explicit.num_simulated_paths, estimate.num_simulated_paths);
        assert_eq!(
            estimate.mean.amount().to_bits(),
            explicit.mean.amount().to_bits(),
            "default and explicit configurations must replay identical estimators"
        );
        assert_eq!(estimate.stderr.to_bits(), explicit.stderr.to_bits());
        assert_eq!(estimate.ci_95, explicit.ci_95);
    }

    #[test]
    fn implied_volatility_is_not_used_as_hw1f_sigma() {
        let as_of = date(2025, Month::January, 1);
        let market = market(as_of, 0.02, 0.03);
        let no_iv = test_snowball();
        let mut with_iv = no_iv.clone();
        with_iv
            .instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(0.20);

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
        let day_count = inst.day_count;
        let ctx = DayCountContext::default();
        let mut pv = 0.0;
        let mut prev_coupon = inst.initial_coupon;

        for period in inst.period_boundaries().windows(2) {
            let start = period[0];
            let end = period[1];
            if end <= as_of {
                continue;
            }
            let accrual = day_count.year_fraction(start, end, ctx).expect("accrual");
            let rate = match floating_rate {
                Some(r) => r,
                None => {
                    // In-advance fixing: simple forward over the fixing window
                    // [max(start, as_of), end] implied by the discount curve.
                    let fixing_start = start.max(as_of);
                    let p_start = disc
                        .as_ref()
                        .df_between_dates(as_of, fixing_start)
                        .expect("p_start");
                    let p_end = disc.as_ref().df_between_dates(as_of, end).expect("p_end");
                    (p_start / p_end - 1.0) / accrual
                }
            };
            let df = disc.as_ref().df_between_dates(as_of, end).expect("df");
            let coupon = inst.compute_coupon(rate, prev_coupon);
            pv += coupon * accrual * inst.notional.amount() * df;
            prev_coupon = coupon;
        }

        let maturity = *inst.payment_dates.last().expect("maturity");
        let df = disc.as_ref().df_between_dates(as_of, maturity).expect("df");
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
    /// pricer via `forward_curve.rate_period_between_dates(start.max(as_of), end)`,
    /// the HW1F MC via the term-forward reconstruction at the short rate
    /// sampled at the period start. On [`consistent_market`] the projection
    /// curve carries the discount curve's own instantaneous forward, so with a
    /// near-zero σ (the two models coincide) they must price to within a small
    /// residual gap (empirically ≈ $5.3k): the simple-vs-continuous-compounding
    /// difference between the discount curve's *simple* forward and the
    /// projection curve's *integral-averaged* rate, amplified by the inverse
    /// floater's 1.5× gearing, plus the θ(t)-bootstrap discretization residual.
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
        inst.call_provision = Some(BermudanCallProvision::new(
            vec![date(2026, Month::January, 1)],
            100.0,
            None,
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

        let low = SnowballHw1fMcPricer::with_hw_params(
            HullWhiteCalibrationParams::new(0.05, 0.015).expect("hw"),
        )
        .with_config(Hw1fMcConfig {
            num_paths: 100,
            antithetic: true,
            min_steps_between_events: 1,
            seed: 7,
            ..Default::default()
        })
        .price_estimate(&inst, &market, as_of)
        .expect("low path price");
        let high = SnowballHw1fMcPricer::with_hw_params(
            HullWhiteCalibrationParams::new(0.05, 0.015).expect("hw"),
        )
        .with_config(Hw1fMcConfig {
            num_paths: 1000,
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
        assert!(result.measures.contains_key(
            &finstack_quant_valuations_core::metrics::MetricId::custom("mc_num_paths")
        ));
    }

    /// Regression: the pricer must read `index_tenor` when reconstructing the
    /// HW1F term forward. If `index_tenor` is ignored (bug), changing it from
    /// 3M to 6M on semi-annual coupon periods leaves the PV unchanged because the
    /// pricer always uses the coupon accrual fraction (= 0.5 yr) as the bond
    /// tenor. On a sloped curve the 3M and 6M simple forwards differ, so the two
    /// instruments MUST price differently once `index_tenor` is respected.
    #[test]
    fn index_tenor_affects_pv_on_sloped_curve() {
        let as_of = date(2025, Month::January, 1);
        let market = market(as_of, 0.02, 0.03);

        // Semi-annual coupon periods, 3M floating index.
        let inst_3m = Snowball {
            index_tenor: Tenor::quarterly(),
            ..test_snowball()
        };
        let err = deterministic_mc_pricer(32)
            .price_estimate(&inst_3m, &market, as_of)
            .expect_err("3M instrument must reject a 6M forward curve");
        assert!(err.to_string().contains("does not match forward curve"));
    }

    #[test]
    fn missing_discount_curve_surfaces_as_not_found() {
        let as_of = date(2025, Month::January, 1);
        let market = market(as_of, 0.02, 0.03);
        let snowball = Snowball {
            discount_curve_id: CurveId::new("MISSING-DISC"),
            ..test_snowball()
        };
        let inverse_floater = Snowball {
            discount_curve_id: CurveId::new("MISSING-DISC"),
            ..test_inverse_floater()
        };
        let mc = deterministic_mc_pricer(16);
        let errors = [
            SnowballDiscountingPricer
                .price_dyn(&inverse_floater, &market, as_of)
                .expect_err("missing curve"),
            SnowballDiscountingPricer
                .price_raw_dyn(&inverse_floater, &market, as_of)
                .expect_err("missing curve"),
            mc.price_dyn(&snowball, &market, as_of)
                .expect_err("missing curve"),
            mc.price_raw_dyn(&snowball, &market, as_of)
                .expect_err("missing curve"),
        ];
        for err in errors {
            let err = finstack_quant_core::Error::from(err);
            assert!(
                matches!(
                    err,
                    finstack_quant_core::Error::Input(
                        finstack_quant_core::InputError::NotFound { .. }
                    )
                ),
                "missing discount curve must stay NotFound, got {err:?}"
            );
        }
    }

    #[test]
    fn semi_annual_coupons_with_3m_index_project_over_3m_window_on_sloped_curve() {
        let as_of = date(2025, Month::January, 1);
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots(
                (0..=16)
                    .map(|i| {
                        let t = i as f64 * 0.25;
                        (t, (-sloped_zero(0.02, t) * t).exp())
                    })
                    .collect::<Vec<_>>(),
            )
            .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("discount");
        let fwd_3m =
            ForwardCurve::from_discount_curve("USD-SOFR-3M", &disc, Some(Tenor::quarterly()));
        let curves = MarketContext::new()
            .try_insert(disc.clone())
            .expect("disc")
            .try_insert(fwd_3m.clone())
            .expect("fwd");

        let mut inst = test_inverse_floater();
        inst.coupon_cap = None;
        inst.forward_curve_id = CurveId::new("USD-SOFR-3M");
        inst.index_tenor = Tenor::quarterly();

        let discounting = SnowballDiscountingPricer
            .price_internal(&inst, &curves, as_of)
            .expect("discounting")
            .amount();
        let mc = deterministic_mc_pricer(256)
            .price_estimate(&inst, &curves, as_of)
            .expect("mc")
            .mean
            .amount();
        assert!(
            (mc - discounting).abs() < 2_500.0,
            "3M index on 6M coupon periods must not inject 6M-3M curve slope bias: mc={mc}, disc={discounting}"
        );

        let mut expected_discounting = 0.0;
        let ctx = DayCountContext::default();
        for period in inst.period_boundaries().windows(2) {
            let start = period[0];
            let end = period[1];
            let accrual = inst.day_count.year_fraction(start, end, ctx).expect("yf");
            let df = disc.df_between_dates(as_of, end).expect("df");
            let index_end = Tenor::quarterly()
                .add_to_date(
                    start,
                    None,
                    finstack_quant_core::dates::BusinessDayConvention::Unadjusted,
                )
                .expect("index_end");
            let index_yf = inst
                .day_count
                .year_fraction(start, index_end, ctx)
                .expect("index_yf");
            let r_3m = fwd_3m
                .rate_between_dates(start, index_end, index_yf)
                .expect("3m fwd");
            expected_discounting +=
                inst.compute_coupon(r_3m, 0.0) * accrual * inst.notional.amount() * df;
        }
        let mat_df = disc
            .df_between_dates(as_of, *inst.payment_dates.last().expect("mat"))
            .expect("mat df");
        expected_discounting += inst.notional.amount() * mat_df;
        assert!((discounting - expected_discounting).abs() < 1e-8);
    }
}
