//! Range accrual Monte Carlo and Analytical pricers.
//!
//! This module provides two pricing methods:
//!
//! 1. **Static Replication (Default, `ModelKey::StaticReplication`)**: Uses digital
//!    call spread replication to price the range accrual analytically. Captures
//!    volatility skew/smile naturally.
//!
//! 2. **Monte Carlo (`ModelKey::MonteCarloGBM`)**: Path-dependent simulation for
//!    complex cases.
//!
//! Both methods support:
//! - Absolute or relative bounds (via `BoundsType`)
//! - Quanto drift adjustment (requires `quanto_correlation` and `fx_vol_surface_id`)
//! - Historical fixings for mid-life valuations (via `past_fixings_in_range`)

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::range_accrual::monte_carlo::RangeAccrualPayoff;
use crate::instruments::rates::range_accrual::types::RangeAccrual;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext, PricingResult,
};
use crate::results::ValuationResult;
use finstack_core::dates::{Date, DayCountContext};
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::Result;
use finstack_monte_carlo::pricer::path_dependent::{
    PathDependentPricer, PathDependentPricerConfig,
};
use finstack_monte_carlo::process::gbm::{GbmParams, GbmProcess};

/// Resolve the FX spot required for a quanto range-accrual payoff.
///
/// When `quanto.fx_spot_id` is configured, the spot **must** resolve from the
/// market context. Silently substituting `1.0` — as the prior implementation
/// did — masks missing market data and materially mis-prices the quanto
/// adjustment term (which scales multiplicatively with `fx_spot`). Callers
/// that truly want an ATM approximation should set `fx_spot_id = None`
/// explicitly.
fn get_fx_spot(inst: &RangeAccrual, curves: &MarketContext) -> Result<f64> {
    let fx_spot_id = inst.quanto.as_ref().and_then(|q| q.fx_spot_id.as_deref());

    match fx_spot_id {
        None => Ok(1.0),
        Some(id) => {
            let ms = curves.get_price(id).map_err(|e| {
                finstack_core::Error::Validation(format!(
                    "range-accrual quanto fx_spot_id '{id}' not found in market context: {e}. \
                     Provide the FX spot scalar or drop the fx_spot_id to use the ATM \
                     approximation explicitly."
                ))
            })?;
            Ok(match ms {
                finstack_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
                finstack_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
            })
        }
    }
}

/// Range accrual Monte Carlo pricer.
///
/// # Flat-volatility limitation (audit item 9)
///
/// This pricer simulates the underlying with a **single, constant** GBM
/// volatility — `σ = vol_surface.value_clamped(T, S₀)`, the ATM vol at the
/// final maturity. Geometric Brownian motion is a constant-volatility process,
/// so the Monte Carlo path-set cannot represent a volatility **skew** or
/// **term structure**. On a non-flat surface this MC therefore diverges from
/// [`RangeAccrualStaticReplicationPricer`], which samples the surface
/// per-observation and per-strike (via the digital call-spread replication)
/// and so captures the smile and term structure exactly.
///
/// **Use the static-replication pricer (`ModelKey::StaticReplication`, the
/// default) for any surface that is not flat.** This MC path is retained for
/// flat-vol scenarios and as a cross-check; it is selected only explicitly
/// (`ModelKey::MonteCarloGBM`) or by the deprecated `mc_seed_scenario`
/// override. Capturing skew/term-structure here would require a local- or
/// stochastic-volatility process, which is outside this crate.
pub struct RangeAccrualMcPricer {
    config: PathDependentPricerConfig,
}

impl RangeAccrualMcPricer {
    /// Create a new range accrual MC pricer with default config.
    pub fn new() -> Self {
        Self {
            config: PathDependentPricerConfig::default(),
        }
    }

    /// Price a range accrual using Monte Carlo.
    fn price_internal(
        &self,
        inst: &RangeAccrual,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<finstack_core::money::Money> {
        inst.validate()?;
        let spot_scalar = curves.get_price(&inst.spot_id)?;
        let initial_spot = match spot_scalar {
            finstack_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
            finstack_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
        };

        // Compute effective bounds based on BoundsType
        let effective_lower = inst.effective_lower_bound(initial_spot);
        let effective_upper = inst.effective_upper_bound(initial_spot);

        let final_date = inst
            .payment_date
            .unwrap_or(inst.observation_dates.last().copied().unwrap_or(as_of));
        let t = inst
            .day_count
            .year_fraction(as_of, final_date, DayCountContext::default())?;

        // Count future observations only
        let future_obs_count = inst
            .observation_dates
            .iter()
            .filter(|&&date| {
                inst.day_count
                    .year_fraction(as_of, date, DayCountContext::default())
                    .unwrap_or(0.0)
                    > 0.0
            })
            .count();

        // If no future observations, return value based on past fixings only
        if future_obs_count == 0 || t <= 0.0 {
            return compute_past_only_value(inst);
        }

        let disc_curve = curves.get_discount(inst.discount_curve_id.as_str())?;
        let r = disc_curve.zero(t);
        let discount_factor = disc_curve.df_between_dates(as_of, final_date)?;

        let mut q = if let Some(div_id) = &inst.div_yield_id {
            match curves.get_price(div_id.as_str()) {
                Ok(ms) => match ms {
                    finstack_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
                    finstack_core::market_data::scalars::MarketScalar::Price(_) => 0.0,
                },
                Err(_) => 0.0,
            }
        } else {
            0.0
        };

        let vol_surface = curves.get_surface(inst.vol_surface_id.as_str())?;
        // FLAT-VOL APPROXIMATION (audit item 9 — see the struct-level doc):
        // GBM is a constant-volatility process, so the whole simulation uses a
        // single ATM vol. This deliberately does NOT capture volatility skew or
        // term structure; on a non-flat surface this MC diverges from the
        // static-replication pricer (which samples per-observation and
        // per-strike). Use `ModelKey::StaticReplication` for non-flat surfaces.
        let sigma = vol_surface.value_clamped(t, initial_spot);

        // Quanto Adjustment using FX spot for vol lookup
        if let Some(quanto) = &inst.quanto {
            let fx_vol_surface = curves.get_surface(quanto.fx_vol_surface_id.as_str())?;
            let fx_spot = get_fx_spot(inst, curves)?;
            let sigma_fx = fx_vol_surface.value_clamped(t, fx_spot);

            // Drift adjustment: q_param = q_real + rho * sigma_S * sigma_FX
            q += quanto.correlation * sigma * sigma_fx;
        }

        let gbm_params = GbmParams::new(r, q, sigma)?;
        let process = GbmProcess::new(gbm_params);

        let steps_per_year = self.config.steps_per_year;
        let num_steps = ((t * steps_per_year).round() as usize).max(self.config.min_steps);

        // Map only future observation dates to times (filter out past observations)
        let observation_times: Vec<f64> = inst
            .observation_dates
            .iter()
            .filter_map(|&date| {
                let t_obs = inst
                    .day_count
                    .year_fraction(as_of, date, DayCountContext::default())
                    .unwrap_or(0.0);
                if t_obs > 0.0 {
                    Some(t_obs)
                } else {
                    None
                }
            })
            .collect();

        // Create payoff with effective bounds and historical fixing info
        let payoff = RangeAccrualPayoff::new_with_history(
            observation_times,
            effective_lower,
            effective_upper,
            inst.coupon_rate,
            inst.notional.amount(),
            inst.notional.currency(),
            inst.past_fixings_in_range.unwrap_or(0),
            inst.total_past_observations.unwrap_or(0),
        )?;

        // Derive deterministic seed from instrument ID and scenario
        use finstack_monte_carlo::seed;

        let seed = if let Some(ref scenario) = inst.pricing_overrides.metrics.mc_seed_scenario {
            seed::derive_seed(&inst.id, scenario)
        } else {
            seed::derive_seed(&inst.id, "base")
        };

        let mut config = self.config.clone();
        config.seed = seed;
        let pricer = PathDependentPricer::new(config);
        let result = pricer.price(
            &process,
            initial_spot,
            t,
            num_steps,
            &payoff,
            inst.notional.currency(),
            discount_factor,
        )?;

        Ok(result.mean)
    }
}

/// Compute value when only past fixings exist (no future observations).
fn compute_past_only_value(inst: &RangeAccrual) -> Result<Money> {
    match (inst.past_fixings_in_range, inst.total_past_observations) {
        (Some(in_range), Some(total)) if total > 0 => {
            let accrual_fraction = in_range as f64 / total as f64;
            let fv = inst.notional.amount() * inst.coupon_rate * accrual_fraction;
            // No discounting needed - payment date is in the past or at as_of
            Ok(Money::new(fv, inst.notional.currency()))
        }
        _ => Ok(Money::new(0.0, inst.notional.currency())),
    }
}

impl Default for RangeAccrualMcPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for RangeAccrualMcPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::RangeAccrual, ModelKey::MonteCarloGBM)
    }

    fn price_dyn(
        &self,
        instrument: &dyn crate::instruments::common_impl::traits::Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> PricingResult<ValuationResult> {
        let range_accrual = instrument
            .as_any()
            .downcast_ref::<RangeAccrual>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::RangeAccrual, instrument.key())
            })?;

        let pv = self
            .price_internal(range_accrual, market, as_of)
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        Ok(ValuationResult::stamped(range_accrual.id(), as_of, pv))
    }
}

/// Range accrual static replication pricer.
pub struct RangeAccrualStaticReplicationPricer;

impl Pricer for RangeAccrualStaticReplicationPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::RangeAccrual, ModelKey::StaticReplication)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> PricingResult<ValuationResult> {
        let range_accrual = instrument
            .as_any()
            .downcast_ref::<RangeAccrual>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::RangeAccrual, instrument.key())
            })?;

        let pv = npv_analytic(range_accrual, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        Ok(ValuationResult::stamped(range_accrual.id(), as_of, pv))
    }
}

/// Present value using Monte Carlo.
pub(crate) fn compute_pv(
    inst: &RangeAccrual,
    curves: &MarketContext,
    as_of: Date,
) -> Result<Money> {
    if inst.pricing_overrides.metrics.mc_seed_scenario.is_some() {
        tracing::warn!(
            instrument_id = %inst.id,
            "range_accrual mc_seed_scenario override forcing MonteCarloGBM is deprecated; \
             price with ModelKey::MonteCarloGBM instead"
        );
        let pricer = RangeAccrualMcPricer::new();
        pricer.price_internal(inst, curves, as_of)
    } else {
        npv_analytic(inst, curves, as_of)
    }
}

/// Present value using Static Replication (Analytic).
///
/// Replicates the range accrual as a sum of digital options (binary call spreads).
/// Captures volatility skew/smile and term structure naturally from the surface.
///
/// This method:
/// - Uses effective bounds based on `BoundsType` (absolute or relative to initial spot)
/// - Applies quanto drift adjustment using FX spot for vol lookup when available
/// - Includes historical fixings in the accrual calculation for mid-life valuations
pub fn npv_analytic(inst: &RangeAccrual, curves: &MarketContext, as_of: Date) -> Result<Money> {
    use crate::instruments::common_impl::models::volatility::black::d1_d2_black76;
    use finstack_core::math::special_functions::norm_cdf;

    inst.validate()?;
    let spot_scalar = curves.get_price(&inst.spot_id)?;
    let initial_spot = match spot_scalar {
        finstack_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
        finstack_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
    };

    // Compute effective bounds based on BoundsType
    let effective_lower = inst.effective_lower_bound(initial_spot);
    let effective_upper = inst.effective_upper_bound(initial_spot);

    let final_date = inst
        .payment_date
        .unwrap_or(inst.observation_dates.last().copied().unwrap_or(as_of));

    let disc_curve = curves.get_discount(inst.discount_curve_id.as_str())?;
    let discount_factor = disc_curve.df_between_dates(as_of, final_date)?;

    let q_yield = if let Some(div_id) = &inst.div_yield_id {
        match curves.get_price(div_id.as_str()) {
            Ok(ms) => match ms {
                finstack_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
                finstack_core::market_data::scalars::MarketScalar::Price(_) => 0.0,
            },
            Err(_) => 0.0,
        }
    } else {
        0.0
    };

    let vol_surface = curves.get_surface(inst.vol_surface_id.as_str())?;

    // Get FX spot for quanto vol lookup (uses actual spot if available, else 1.0)
    let fx_spot = get_fx_spot(inst, curves)?;

    // Count observations and track past/future split
    let n_total_obs = inst.observation_dates.len();
    if n_total_obs == 0 {
        return Ok(Money::new(0.0, inst.notional.currency()));
    }

    // Count future observations
    let mut future_obs_count = 0usize;
    let mut total_expected_in_range = 0.0;

    for &date in &inst.observation_dates {
        let t_obs = inst
            .day_count
            .year_fraction(as_of, date, DayCountContext::default())?;

        if t_obs <= 0.0 {
            // Past observation - skip (handled via past_fixings_in_range)
            continue;
        }

        future_obs_count += 1;
        let r_obs = disc_curve.zero(t_obs);

        // Quanto drift adjustment specific to this horizon
        let mut drift_adj = 0.0;
        if let Some(quanto) = &inst.quanto {
            let fx_vol_surface = curves.get_surface(quanto.fx_vol_surface_id.as_str())?;
            // Vol of Asset (S) for drift adj: use ATM at current spot
            let sig_s = vol_surface.value_clamped(t_obs, initial_spot);
            // Vol of FX for drift adj: use ATM at FX spot
            let sig_fx = fx_vol_surface.value_clamped(t_obs, fx_spot);
            drift_adj = quanto.correlation * sig_s * sig_fx;
        }

        // Forward Price F = S * exp((r - q - drift_adj) * t)
        let forward = initial_spot * ((r_obs - q_yield - drift_adj) * t_obs).exp();

        // Digital Call Probability P(S_t > K) via finite-width call spread.
        //
        // Using a 25bp spread replaces the analytically thin N(d₂) digital with
        // a hedgeable call spread that correctly captures the volatility skew
        // contribution to the digital price.  The formula is:
        //
        //   P(S > K) ≈ [Call(K - h/2) - Call(K + h/2)] / h
        //
        // where h = DIGITAL_SPREAD_WIDTH = 0.0025 (25 basis points) and
        // Call(k) is the undiscounted Black-76 call price at strike k.
        //
        // For a flat smile this recovers N(d₂) exactly as h → 0.  With a
        // downward skew (higher vol at lower strikes), P(S > K) is larger
        // than the flat-smile N(d₂), matching market digital prices.
        //
        // Lower node is clamped to DIGITAL_SPREAD_FLOOR to ensure K - h/2 > 0.
        const DIGITAL_SPREAD_WIDTH: f64 = 0.0025; // 25 bp
        const DIGITAL_SPREAD_FLOOR: f64 = 1e-6; // prevent negative strikes

        // Undiscounted Black-76 call price: F·N(d1) - K·N(d2)
        let black_call = |k: f64| -> f64 {
            let vol = vol_surface.value_clamped(t_obs, k);
            let std_dev = vol * t_obs.sqrt();
            if std_dev < 1e-6 {
                return (forward - k).max(0.0);
            }
            let (d1, d2) = d1_d2_black76(forward, k, vol, t_obs);
            forward * norm_cdf(d1) - k * norm_cdf(d2)
        };

        // Digital call probability using finite-width call spread.
        // The spread half-width is clipped so K - h/2 ≥ DIGITAL_SPREAD_FLOOR.
        let calc_prob_above = |strike: f64| -> finstack_core::Result<f64> {
            let half_h = DIGITAL_SPREAD_WIDTH / 2.0;
            let k_lo = (strike - half_h).max(DIGITAL_SPREAD_FLOOR);
            let k_hi = strike + half_h;
            // Effective spread width (may be narrower near zero)
            let spread = k_hi - k_lo;
            if spread < 1e-12 {
                // Degenerate: fall back to a binary step on the forward
                return Ok(if forward > strike { 1.0 } else { 0.0 });
            }
            let prob = (black_call(k_lo) - black_call(k_hi)) / spread;
            Ok(prob.clamp(0.0, 1.0))
        };

        let p_lower = calc_prob_above(effective_lower)?;
        let p_upper = calc_prob_above(effective_upper)?;

        // Prob in range [L, U] = P(S > L) - P(S > U)
        let p_in_range = (p_lower - p_upper).clamp(0.0, 1.0);
        total_expected_in_range += p_in_range;
    }

    // Include historical fixings in the total
    // Total observations = past observations + future observations
    let past_in_range = inst.past_fixings_in_range.unwrap_or(0) as f64;
    let total_past_obs = inst.total_past_observations.unwrap_or(0);

    // Total observations across full life of instrument
    let total_obs_count = total_past_obs + future_obs_count;
    if total_obs_count == 0 {
        return Ok(Money::new(0.0, inst.notional.currency()));
    }

    // Expected total days in range = known past + expected future
    let expected_total_in_range = past_in_range + total_expected_in_range;

    // Accrual fraction = expected days in range / total days
    let expected_fraction = expected_total_in_range / (total_obs_count as f64);

    // Future value and present value
    let fv = inst.notional.amount() * inst.coupon_rate * expected_fraction;
    let pv = fv * discount_factor;

    Ok(Money::new(pv, inst.notional.currency()))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::instruments::rates::range_accrual::types::RangeAccrual;
    use finstack_core::currency::Currency;
    use finstack_core::market_data::scalars::MarketScalar;
    use finstack_core::market_data::surfaces::VolSurface;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::money::Money;
    use time::Month;

    fn date(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(year, Month::try_from(month).expect("valid month"), day)
            .expect("valid date")
    }

    fn market(as_of: Date) -> MarketContext {
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(finstack_core::dates::DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, 0.97), (2.0, 0.94)])
            .build()
            .expect("curve");
        let surface = VolSurface::builder("SPX-VOL")
            .expiries(&[0.25, 0.5, 1.0, 2.0])
            .strikes(&[80.0, 100.0, 120.0, 140.0])
            .row(&[0.20, 0.20, 0.20, 0.20])
            .row(&[0.20, 0.20, 0.20, 0.20])
            .row(&[0.20, 0.20, 0.20, 0.20])
            .row(&[0.20, 0.20, 0.20, 0.20])
            .build()
            .expect("surface");

        MarketContext::new()
            .insert(curve)
            .insert_surface(surface)
            .insert_price("SPX-SPOT", MarketScalar::Unitless(100.0))
            .insert_price("SPX-DIV", MarketScalar::Unitless(0.02))
    }

    #[test]
    fn analytic_range_accrual_returns_zero_when_all_observations_are_past_and_no_history_is_supplied(
    ) {
        let as_of = date(2024, 4, 30);
        let mut inst = RangeAccrual::example();
        inst.observation_dates = vec![as_of];
        inst.payment_date = Some(as_of);
        inst.past_fixings_in_range = None;
        inst.total_past_observations = None;

        let pv = npv_analytic(&inst, &market(as_of), as_of).expect("pv");
        assert_eq!(pv.amount(), 0.0);
    }

    #[test]
    fn analytic_range_accrual_payment_date_changes_discounting_only() {
        let as_of = date(2024, 1, 1);
        let base = RangeAccrual::example();
        let mut delayed = base.clone();
        delayed.payment_date = Some(date(2025, 12, 31));

        let curves = market(as_of);
        let base_pv = npv_analytic(&base, &curves, as_of).expect("base pv");
        let delayed_pv = npv_analytic(&delayed, &curves, as_of).expect("delayed pv");

        assert!(base_pv.amount() > 0.0);
        assert!(delayed_pv.amount() > 0.0);
        assert!(delayed_pv.amount() < base_pv.amount());
    }

    #[test]
    fn analytic_range_accrual_treats_missing_or_price_dividend_scalar_as_zero_yield() {
        let as_of = date(2024, 1, 1);
        let inst = RangeAccrual::example();
        let base_market = market(as_of);
        let no_div_market = MarketContext::new()
            .insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(as_of)
                    .day_count(finstack_core::dates::DayCount::Act365F)
                    .knots([(0.0, 1.0), (1.0, 0.97), (2.0, 0.94)])
                    .build()
                    .expect("curve"),
            )
            .insert_surface(
                VolSurface::builder("SPX-VOL")
                    .expiries(&[0.25, 0.5, 1.0, 2.0])
                    .strikes(&[80.0, 100.0, 120.0, 140.0])
                    .row(&[0.20, 0.20, 0.20, 0.20])
                    .row(&[0.20, 0.20, 0.20, 0.20])
                    .row(&[0.20, 0.20, 0.20, 0.20])
                    .row(&[0.20, 0.20, 0.20, 0.20])
                    .build()
                    .expect("surface"),
            )
            .insert_price("SPX-SPOT", MarketScalar::Unitless(100.0));
        let price_div_market = market(as_of).insert_price(
            "SPX-DIV",
            MarketScalar::Price(Money::new(2.0, Currency::USD)),
        );

        let pv_missing = npv_analytic(&inst, &no_div_market, as_of).expect("missing div pv");
        let pv_price = npv_analytic(&inst, &price_div_market, as_of).expect("price div pv");
        let mut no_div_inst = inst.clone();
        no_div_inst.div_yield_id = None;
        let pv_explicit_none =
            npv_analytic(&no_div_inst, &base_market, as_of).expect("none div pv");

        assert!((pv_missing.amount() - pv_explicit_none.amount()).abs() < 1e-12);
        assert!((pv_price.amount() - pv_explicit_none.amount()).abs() < 1e-12);
    }
}
