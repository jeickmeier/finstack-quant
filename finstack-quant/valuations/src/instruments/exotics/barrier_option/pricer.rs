//! Barrier option pricers (Monte Carlo and analytical).

// Common imports for all pricers
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::common_impl::two_clock::TwoClockParams;
use crate::instruments::exotics::barrier_option::types::BarrierOption;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

// DayCountContext is now threaded via `TwoClockParams`; the test-only
// import is retained here because analytical tests still build without
// `mc` and need the reference.
#[cfg(test)]
#[allow(unused_imports)]
use finstack_quant_core::dates::DayCountContext;

// MC-specific imports
use finstack_quant_monte_carlo::payoff::barrier::BarrierOptionPayoff;
use finstack_quant_monte_carlo::payoff::barrier::{BarrierType as McBarrierType, OptionKind};
use finstack_quant_monte_carlo::pricer::path_dependent::{
    PathDependentPricer, PathDependentPricerConfig,
};
use finstack_quant_monte_carlo::process::gbm::{GbmParams, GbmProcess};

/// Whether the instrument's rebate should be paid at the hit time.
///
/// When true, the MC payoff is configured via
/// [`BarrierOptionPayoff::with_rebate_at_hit`] so the rebate compounds
/// forward from the hit time τ at the flat rate and the engine's maturity
/// discount factor nets to `DF(τ)` — exact at-hit discounting, matching the
/// analytical [`crate::models::closed_form::barrier::barrier_rebate`] with
/// [`RebateTiming::AtHit`](crate::models::closed_form::barrier::RebateTiming::AtHit).
pub(crate) fn wants_at_hit_rebate(inst: &BarrierOption) -> bool {
    use crate::models::closed_form::barrier::RebateTiming;
    inst.rebate.is_some() && inst.rebate_timing == RebateTiming::AtHit
}

/// Barrier option Monte Carlo pricer.
pub struct BarrierOptionMcPricer {
    config: PathDependentPricerConfig,
}

impl BarrierOptionMcPricer {
    /// Create a new barrier option MC pricer with default config.
    pub fn new() -> Self {
        Self {
            config: PathDependentPricerConfig::default(),
        }
    }

    fn convert_option_kind(option_type: crate::instruments::OptionType) -> OptionKind {
        match option_type {
            crate::instruments::OptionType::Call => OptionKind::Call,
            crate::instruments::OptionType::Put => OptionKind::Put,
        }
    }

    /// Price a barrier option using Monte Carlo.
    ///
    /// # Day Count Convention Handling
    ///
    /// Uses separate day count bases for different purposes:
    /// - **Discounting**: Uses the discount curve's own day count for DF and zero rate calculations
    /// - **Volatility lookup**: Uses the instrument's day count (assumed to match vol surface calibration)
    /// - **Monte Carlo time grid**: Uses the vol surface time basis for proper barrier monitoring
    fn price_internal(
        &self,
        inst: &BarrierOption,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        // Get discount curve
        let disc_curve = curves.get_discount(inst.discount_curve_id.as_str())?;

        // Two-clock plumbing: t_vol drives the vol surface / MC time
        // grid; t_disc + df drive the drift rate and final discounting.
        // Keeping these separate makes the pricer bump-and-reval-
        // consistent with the curve's own day-count convention when it
        // differs from the vol surface basis.
        let clocks = TwoClockParams::from_curve_and_instrument(
            &disc_curve,
            inst.day_count,
            as_of,
            inst.expiry,
        )?;
        let t_vol = clocks.t_vol;

        if t_vol <= 0.0 {
            return price_expired_barrier(inst, curves);
        }

        let discount_factor = clocks.df;
        // Drift rate on the discount curve's clock.
        let r = clocks.r_disc();

        // Get spot
        let spot_scalar = curves.get_price(&inst.spot_id)?;
        let spot = match spot_scalar {
            finstack_quant_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
            finstack_quant_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
        };

        // Get dividend yield
        let q = crate::instruments::common_impl::helpers::resolve_optional_dividend_yield(
            curves,
            inst.div_yield_id.as_ref(),
        )?;

        // Get volatility (override → surface, using vol surface time basis)
        let sigma = crate::instruments::common_impl::vol_resolution::resolve_sigma_at(
            &inst.pricing_overrides.market_quotes,
            curves,
            inst.vol_surface_id.as_str(),
            t_vol,
            inst.strike,
        )?;

        // Create GBM process
        let gbm_params = GbmParams::new(r, q, sigma)?;
        let process = GbmProcess::new(gbm_params);

        // Create time grid with minimum-capped steps (using vol surface time basis for proper
        // barrier monitoring - this ensures time steps align with volatility assumptions)
        let steps_per_year = self.config.steps_per_year;
        let num_steps = ((t_vol * steps_per_year).round() as usize).max(self.config.min_steps);
        let time_grid = finstack_quant_monte_carlo::time_grid::TimeGrid::uniform(t_vol, num_steps)?;
        // `maturity_step` must equal `time_grid.num_steps()` (= num_steps): the engine
        // calls `on_event` with `state.step = num_steps` on the last iteration, so the
        // terminal-spot capture guard `state.step == maturity_step` must fire there.
        let maturity_step = num_steps;

        // Create payoff (using vol surface time for barrier adjustment calculations)
        let mc_barrier_type: McBarrierType = inst.barrier_type.into();
        let mut payoff = BarrierOptionPayoff::new(
            inst.strike,
            inst.barrier.amount(),
            mc_barrier_type,
            Self::convert_option_kind(inst.option_type),
            inst.rebate.map(|m| m.amount()),
            inst.notional.amount(),
            maturity_step,
            sigma,
            &time_grid,
            inst.use_gobet_miri,
        );
        if wants_at_hit_rebate(inst) {
            payoff = payoff.with_rebate_at_hit(r);
        }

        // Derive deterministic seed from instrument ID and scenario

        use finstack_quant_monte_carlo::seed;

        let seed = if let Some(ref scenario) = inst.pricing_overrides.metrics.mc_seed_scenario {
            seed::derive_seed(&inst.id, scenario)
        } else {
            seed::derive_seed(&inst.id, "base")
        };

        // Create config with derived seed
        let mut config = self.config.clone();
        config.seed = seed;

        // Price using path-dependent pricer (using vol surface time basis for simulation)
        let pricer = PathDependentPricer::new(config);
        let result = pricer.price(
            &process,
            spot,
            t_vol,
            num_steps,
            &payoff,
            inst.notional.currency(),
            discount_factor,
        )?;

        Ok(result.mean)
    }

    /// Price with LRM Greeks (delta, vega) convenience for barrier options.
    ///
    /// Returns `(pv, Option<(delta, vega)>)` where the Greeks are from the
    /// Likelihood Ratio Method (LRM). Greeks are `None` if the option is expired.
    ///
    /// # Day Count Convention Handling
    ///
    /// Uses separate day count bases for different purposes:
    /// - **Discounting**: Uses the discount curve's own day count for DF and zero rate calculations
    /// - **Volatility lookup and MC simulation**: Uses the instrument's day count (assumed to match vol surface calibration)
    #[allow(dead_code)] // May be used by external bindings or tests
    pub(crate) fn price_with_lrm_greeks_internal(
        &self,
        inst: &BarrierOption,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<(finstack_quant_core::money::Money, Option<(f64, f64)>)> {
        // Get discount curve first to access its day count
        let disc_curve = curves.get_discount(inst.discount_curve_id.as_str())?;

        // Two-clock plumbing — see `price_internal` for the rationale.
        let clocks = TwoClockParams::from_curve_and_instrument(
            &disc_curve,
            inst.day_count,
            as_of,
            inst.expiry,
        )?;
        let t_vol = clocks.t_vol;
        if t_vol <= 0.0 {
            let pv = price_expired_barrier(inst, curves)?;
            return Ok((pv, None));
        }

        let discount_factor = clocks.df;
        let r = clocks.r_disc();

        // Spot and dividend yield
        let spot_scalar = curves.get_price(&inst.spot_id)?;
        let spot = match spot_scalar {
            finstack_quant_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
            finstack_quant_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
        };
        let q = crate::instruments::common_impl::helpers::resolve_optional_dividend_yield(
            curves,
            inst.div_yield_id.as_ref(),
        )?;

        // Volatility (override → surface, using vol surface time basis)
        let sigma = crate::instruments::common_impl::vol_resolution::resolve_sigma_at(
            &inst.pricing_overrides.market_quotes,
            curves,
            inst.vol_surface_id.as_str(),
            t_vol,
            inst.strike,
        )?;
        let gbm_params = GbmParams::new(r, q, sigma)?;
        let process = GbmProcess::new(gbm_params);

        // Steps and payoff (using vol surface time basis)
        let steps_per_year = self.config.steps_per_year;
        let num_steps = ((t_vol * steps_per_year).round() as usize).max(self.config.min_steps);
        let time_grid = finstack_quant_monte_carlo::time_grid::TimeGrid::uniform(t_vol, num_steps)?;
        // See `price_internal` for the maturity_step = num_steps rationale.
        let maturity_step = num_steps;
        let mc_barrier_type: McBarrierType = inst.barrier_type.into();
        let mut payoff = BarrierOptionPayoff::new(
            inst.strike,
            inst.barrier.amount(),
            mc_barrier_type,
            Self::convert_option_kind(inst.option_type),
            inst.rebate.map(|m| m.amount()),
            inst.notional.amount(),
            maturity_step,
            sigma,
            &time_grid,
            inst.use_gobet_miri,
        );
        if wants_at_hit_rebate(inst) {
            payoff = payoff.with_rebate_at_hit(r);
        }

        // Seed

        use finstack_quant_monte_carlo::seed;
        let seed = if let Some(ref scenario) = inst.pricing_overrides.metrics.mc_seed_scenario {
            seed::derive_seed(&inst.id, scenario)
        } else {
            seed::derive_seed(&inst.id, "base")
        };
        let mut cfg = self.config.clone();
        cfg.seed = seed;

        let pricer = PathDependentPricer::new(cfg);
        let (est, greeks) = pricer.price_with_lrm_greeks(
            &process,
            spot,
            t_vol,
            num_steps,
            &payoff,
            inst.notional.currency(),
            discount_factor,
            r,
            q,
            sigma,
        )?;

        Ok((est.mean, greeks))
    }
}

impl Default for BarrierOptionMcPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for BarrierOptionMcPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::BarrierOption, ModelKey::MonteCarloGBM)
    }

    fn price_dyn(
        &self,
        instrument: &dyn crate::instruments::common_impl::traits::Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let barrier = instrument
            .as_any()
            .downcast_ref::<BarrierOption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::BarrierOption, instrument.key())
            })?;

        let pv = self.price_internal(barrier, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        Ok(ValuationResult::stamped(barrier.id(), as_of, pv))
    }
}

/// Present value using Monte Carlo.
pub(crate) fn compute_pv(
    inst: &BarrierOption,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<Money> {
    let pricer = BarrierOptionMcPricer::new();
    pricer.price_internal(inst, curves, as_of)
}

/// Present value with LRM Greeks via Monte Carlo (barrier option).
///
/// Returns `(pv, Option<(delta, vega)>)` where the Greeks are from the
/// Likelihood Ratio Method. Greeks are `None` if the option is expired.
#[allow(dead_code)] // May be used by external bindings or tests
pub fn npv_with_lrm_greeks(
    inst: &BarrierOption,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<(Money, Option<(f64, f64)>)> {
    let pricer = BarrierOptionMcPricer::new();
    pricer.price_with_lrm_greeks_internal(inst, curves, as_of)
}

// ========================= EXPIRED BARRIER HELPER =========================

/// Price an expired barrier option using explicit observed barrier state.
///
/// Terminal spot alone is insufficient to determine whether a barrier was
/// breached intralife and then later reversed, so expired contracts require
/// the caller to provide `observed_barrier_breached`.
/// The intrinsic value is `max(S - K, 0)` for calls and `max(K - S, 0)` for puts,
/// scaled by notional.
fn price_expired_barrier(
    inst: &BarrierOption,
    curves: &MarketContext,
) -> finstack_quant_core::Result<Money> {
    use crate::instruments::exotics::barrier_option::types::BarrierType;

    let spot_scalar = curves.get_price(&inst.spot_id)?;
    let spot = match spot_scalar {
        finstack_quant_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
        finstack_quant_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
    };

    let ccy = inst.notional.currency();
    let notional = inst.notional.amount();
    let is_knock_out = matches!(
        inst.barrier_type,
        BarrierType::UpAndOut | BarrierType::DownAndOut
    );

    let barrier_breached = inst.observed_barrier_breached.ok_or_else(|| {
        finstack_quant_core::Error::Validation(
            "Expired barrier option requires `observed_barrier_breached` to determine realized payoff"
                .to_string(),
        )
    })?;

    let intrinsic = match inst.option_type {
        crate::instruments::OptionType::Call => (spot - inst.strike).max(0.0) * notional,
        crate::instruments::OptionType::Put => (inst.strike - spot).max(0.0) * notional,
    };
    let rebate = inst.rebate.map(|m| m.amount()).unwrap_or(0.0);

    let pv = if is_knock_out {
        if barrier_breached {
            rebate
        } else {
            intrinsic
        }
    } else {
        // Knock-in
        if barrier_breached {
            intrinsic
        } else {
            rebate
        }
    };

    Ok(Money::new(pv, ccy))
}

// ========================= ANALYTICAL PRICER =========================

use crate::models::closed_form::barrier::{
    barrier_call_continuous, barrier_put_continuous, barrier_rebate, BarrierParams,
    BarrierType as AnalyticalBarrierType,
};
/// Broadie-Glasserman-Kou / Gobet-Miri discrete barrier adjustment constant.
///
/// β = -ζ(1/2) / √(2π) ≈ 0.5825971579390106. Re-exported from the canonical
/// definition in `finstack_quant_monte_carlo::barriers::corrections` so the
/// analytical and MC stacks can never drift apart.
const BG_BETA: f64 = finstack_quant_monte_carlo::barriers::corrections::GOBET_MIRI_BETA;

/// Barrier option analytical pricer (continuous monitoring).
///
/// # Monitoring Convention
///
/// **Important**: This pricer uses **continuous monitoring** Reiner-Rubinstein formulas.
/// Real-world barriers are typically monitored discretely (e.g., daily closes).
/// Continuous barrier formulas **systematically underestimate** knock-out option values
/// and overestimate knock-in option values compared to discrete monitoring.
///
/// For discrete monitoring pricing, use the Monte Carlo pricer
/// ([`BarrierOptionMcPricer`]) which applies the Broadie-Glasserman-Kou / Gobet-Miri
/// correction when `use_gobet_miri = true`.
///
/// `BarrierOption::value()` dispatches to this analytical pricer only when
/// `use_gobet_miri = false`. When `use_gobet_miri = true`, `value()` routes
/// to the MC pricer (`npv_mc()`) for discrete-monitoring-corrected prices.
pub(crate) struct BarrierOptionAnalyticalPricer;

impl BarrierOptionAnalyticalPricer {
    /// Create a new analytical barrier option pricer
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Default for BarrierOptionAnalyticalPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for BarrierOptionAnalyticalPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::BarrierOption, ModelKey::BarrierBSContinuous)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let barrier_opt = instrument
            .as_any()
            .downcast_ref::<BarrierOption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::BarrierOption, instrument.key())
            })?;

        if barrier_opt.use_gobet_miri {
            tracing::warn!(
                "Analytical barrier pricer uses continuous monitoring; discrete monitoring flag \
                 is ignored. Use Monte Carlo pricer for discrete barrier monitoring."
            );
        }

        // Use DF-first input collection to keep vol lookup on the instrument clock
        // while preserving discounting on the discount curve clock.
        let bs_inputs = crate::instruments::common_impl::helpers::collect_black_scholes_inputs_df(
            &barrier_opt.spot_id,
            &barrier_opt.discount_curve_id,
            barrier_opt.div_yield_id.as_ref(),
            &barrier_opt.vol_surface_id,
            barrier_opt.strike,
            barrier_opt.expiry,
            barrier_opt.day_count,
            market,
            as_of,
        )
        .map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;
        let spot = bs_inputs.spot;
        let q = bs_inputs.q;
        let sigma = bs_inputs.sigma;
        let t = bs_inputs.t;
        let df = bs_inputs.df;

        if t <= 0.0 {
            let pv = price_expired_barrier(barrier_opt, market).map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;
            return Ok(ValuationResult::stamped(barrier_opt.id(), as_of, pv));
        }

        // Map barrier type
        use crate::instruments::exotics::barrier_option::types::BarrierType;
        let analytical_barrier_type = match barrier_opt.barrier_type {
            BarrierType::UpAndIn => AnalyticalBarrierType::UpIn,
            BarrierType::UpAndOut => AnalyticalBarrierType::UpOut,
            BarrierType::DownAndIn => AnalyticalBarrierType::DownIn,
            BarrierType::DownAndOut => AnalyticalBarrierType::DownOut,
        };

        // Apply Broadie-Glasserman-Kou discrete monitoring correction when
        // monitoring_frequency is set.
        let is_down = matches!(
            barrier_opt.barrier_type,
            BarrierType::DownAndIn | BarrierType::DownAndOut
        );
        let raw_barrier = barrier_opt.barrier.amount();
        let effective_barrier = if let Some(dt) = barrier_opt.monitoring_frequency {
            let shift = BG_BETA * sigma * dt.sqrt();
            let shifted = if is_down {
                raw_barrier * (-shift).exp()
            } else {
                raw_barrier * shift.exp()
            };
            // Guard against the shift crossing spot (W-09). For a large
            // monitoring interval `βσ√Δt` can be large enough to move the
            // effective barrier onto the *other side* of spot, which flips the
            // option's alive/knocked state and mis-prices a near-barrier
            // option (e.g. a live down-and-out collapses to ~0). The
            // Broadie-Glasserman-Kou adjustment is only meaningful while the
            // effective barrier stays on the same side of spot as the
            // contractual barrier, so clamp it just shy of spot.
            //
            // `BARRIER_SPOT_GAP` keeps the effective barrier strictly off spot
            // so the analytical formula does not see a degenerate
            // barrier == spot input.
            const BARRIER_SPOT_GAP: f64 = 1e-8;
            if raw_barrier <= spot {
                // Down-ish barrier (at or below spot): must not rise to spot.
                shifted.min(spot * (1.0 - BARRIER_SPOT_GAP))
            } else {
                // Up-ish barrier (above spot): must not fall to spot.
                shifted.max(spot * (1.0 + BARRIER_SPOT_GAP))
            }
        } else {
            raw_barrier
        };

        let params =
            BarrierParams::with_df(spot, barrier_opt.strike, effective_barrier, t, df, q, sigma)
                .map_err(|e| {
                    PricingError::model_failure_with_context(
                        e.to_string(),
                        PricingErrorContext::default(),
                    )
                })?;
        let price = match barrier_opt.option_type {
            crate::instruments::OptionType::Call => {
                barrier_call_continuous(&params, analytical_barrier_type)
            }
            crate::instruments::OptionType::Put => {
                barrier_put_continuous(&params, analytical_barrier_type)
            }
        };

        let rebate_val = if let Some(rebate) = barrier_opt.rebate {
            barrier_rebate(
                &params,
                rebate.amount(),
                analytical_barrier_type,
                barrier_opt.rebate_timing,
            )
        } else {
            0.0
        };

        let pv = Money::new(
            (price + rebate_val) * barrier_opt.notional.amount(),
            barrier_opt.notional.currency(),
        );
        Ok(ValuationResult::stamped(barrier_opt.id(), as_of, pv))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::exotics::barrier_option::types::{BarrierOption, BarrierType};
    use crate::instruments::{Attributes, OptionType, PricingOverrides};
    use crate::models::closed_form::barrier::{
        barrier_call_continuous, barrier_put_continuous, barrier_rebate_continuous, down_out_call,
        BarrierParams, BarrierType as AnalyticalBarrierType,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{DayCount, DayCountContext};
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::types::InstrumentId;
    use time::Month;

    fn date(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(year, Month::try_from(month).expect("valid month"), day)
            .expect("valid date")
    }

    fn market(as_of: Date, spot: f64, vol: f64, rate: f64, div_yield: f64) -> MarketContext {
        let discount = DiscountCurve::builder("USD_DISC")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (5.0, (-rate * 5.0).exp())])
            .build()
            .expect("discount curve");
        let surface = VolSurface::builder("SPX_VOL")
            .expiries(&[0.25, 0.5, 1.0, 2.0])
            .strikes(&[80.0, 90.0, 100.0, 110.0, 120.0])
            .row(&[vol, vol, vol, vol, vol])
            .row(&[vol, vol, vol, vol, vol])
            .row(&[vol, vol, vol, vol, vol])
            .row(&[vol, vol, vol, vol, vol])
            .build()
            .expect("vol surface");

        MarketContext::new()
            .insert(discount)
            .insert_surface(surface)
            .insert_price("SPX", MarketScalar::Price(Money::new(spot, Currency::USD)))
            .insert_price("SPX_DIV", MarketScalar::Unitless(div_yield))
    }

    fn down_and_out_call(expiry: Date, strike: f64, barrier: f64) -> BarrierOption {
        BarrierOption {
            id: InstrumentId::new("BARRIER-BENCH"),
            underlying_ticker: "SPX".to_string(),
            strike,
            barrier: Money::new(barrier, Currency::USD),
            rebate: None,
            rebate_timing: Default::default(),
            option_type: OptionType::Call,
            barrier_type: BarrierType::DownAndOut,
            expiry,
            observed_barrier_breached: None,
            notional: Money::new(1.0, Currency::USD),
            day_count: DayCount::Act365F,
            use_gobet_miri: false,
            discount_curve_id: "USD_DISC".into(),
            spot_id: "SPX".into(),
            vol_surface_id: "SPX_VOL".into(),
            div_yield_id: Some("SPX_DIV".into()),
            pricing_overrides: PricingOverrides::default(),
            monitoring_frequency: None,
            attributes: Attributes::new(),
        }
    }

    #[test]
    fn analytical_pricer_matches_reiner_rubinstein_down_and_out_call() {
        let as_of = date(2024, 1, 1);
        let expiry = date(2024, 7, 1);
        let spot = 100.0;
        let strike = 100.0;
        let barrier = 80.0;
        let vol = 0.20;
        let rate = 0.05;
        let div_yield = 0.0;

        let option = down_and_out_call(expiry, strike, barrier);
        let market = market(as_of, spot, vol, rate, div_yield);
        let pv = option.value(&market, as_of).expect("barrier pv").amount();

        let t = option
            .day_count
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("year fraction");
        let expected = down_out_call(spot, strike, barrier, t, rate, div_yield, vol);

        assert!((pv - expected).abs() < 1e-12);
    }

    #[test]
    fn analytical_pricer_adds_reiner_rubinstein_rebate_value() {
        let as_of = date(2024, 1, 1);
        let expiry = date(2025, 1, 1);
        let spot = 100.0;
        let strike = 100.0;
        let barrier = 120.0;
        let vol = 0.18;
        let rate = 0.04;
        let div_yield = 0.01;
        let rebate = 2.5;

        let market = market(as_of, spot, vol, rate, div_yield);
        let base = BarrierOption {
            barrier_type: BarrierType::UpAndOut,
            option_type: OptionType::Call,
            barrier: Money::new(barrier, Currency::USD),
            ..down_and_out_call(expiry, strike, barrier)
        };
        let with_rebate = BarrierOption {
            rebate: Some(Money::new(rebate, Currency::USD)),
            ..base.clone()
        };
        let with_rebate_at_expiry = BarrierOption {
            rebate_timing: crate::models::closed_form::barrier::RebateTiming::AtExpiry,
            ..with_rebate.clone()
        };

        let base_pv = base.value(&market, as_of).expect("base pv").amount();
        let rebate_pv = with_rebate
            .value(&market, as_of)
            .expect("rebate pv")
            .amount();
        let rebate_pv_at_expiry = with_rebate_at_expiry
            .value(&market, as_of)
            .expect("rebate pv at expiry")
            .amount();

        let t = with_rebate
            .day_count
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("year fraction");
        let p = BarrierParams::new(spot, barrier, barrier, t, rate, div_yield, vol);

        // Default timing is at-hit (market standard).
        let expected_at_hit = crate::models::closed_form::barrier::barrier_rebate(
            &p,
            rebate,
            AnalyticalBarrierType::UpOut,
            crate::models::closed_form::barrier::RebateTiming::AtHit,
        );
        assert!(((rebate_pv - base_pv) - expected_at_hit).abs() < 1e-12);

        // Explicit AtExpiry reproduces the legacy pay-at-expiry value.
        let expected_at_expiry =
            barrier_rebate_continuous(&p, rebate, AnalyticalBarrierType::UpOut);
        assert!(((rebate_pv_at_expiry - base_pv) - expected_at_expiry).abs() < 1e-12);

        // At-hit must dominate at-expiry under positive rates.
        assert!(rebate_pv >= rebate_pv_at_expiry);
    }

    #[test]
    fn expired_barrier_paths_cover_knock_in_and_knock_out_matrix() {
        let curves = MarketContext::new().insert_price("SPX", MarketScalar::Unitless(120.0));
        let base = down_and_out_call(date(2024, 7, 1), 100.0, 80.0);

        let knocked_out = BarrierOption {
            rebate: Some(Money::new(3.0, Currency::USD)),
            observed_barrier_breached: Some(true),
            ..base.clone()
        };
        let alive_knock_out = BarrierOption {
            observed_barrier_breached: Some(false),
            ..base.clone()
        };
        let knocked_in = BarrierOption {
            barrier_type: BarrierType::UpAndIn,
            observed_barrier_breached: Some(true),
            ..base.clone()
        };
        let no_hit_knock_in = BarrierOption {
            barrier_type: BarrierType::UpAndIn,
            rebate: Some(Money::new(2.5, Currency::USD)),
            observed_barrier_breached: Some(false),
            ..base
        };

        assert_eq!(
            price_expired_barrier(&knocked_out, &curves)
                .expect("ko")
                .amount(),
            3.0
        );
        assert_eq!(
            price_expired_barrier(&alive_knock_out, &curves)
                .expect("alive ko")
                .amount(),
            20.0
        );
        assert_eq!(
            price_expired_barrier(&knocked_in, &curves)
                .expect("ki")
                .amount(),
            20.0
        );
        assert_eq!(
            price_expired_barrier(&no_hit_knock_in, &curves)
                .expect("no hit ki")
                .amount(),
            2.5
        );
    }

    #[test]
    fn analytical_pricer_applies_monitoring_frequency_shift_for_down_barrier() {
        let as_of = date(2024, 1, 1);
        let expiry = date(2024, 7, 1);
        let spot = 100.0;
        let strike = 100.0;
        let barrier = 80.0;
        let vol = 0.20;
        let rate = 0.05;
        let div_yield = 0.0;
        let monitoring_dt = 1.0 / 252.0;

        let option = BarrierOption {
            monitoring_frequency: Some(monitoring_dt),
            ..down_and_out_call(expiry, strike, barrier)
        };
        let market = market(as_of, spot, vol, rate, div_yield);
        let pv = option.value(&market, as_of).expect("barrier pv").amount();

        let t = option
            .day_count
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("year fraction");
        let df = (-rate * t).exp();
        let shifted_barrier = barrier * (-(BG_BETA * vol * monitoring_dt.sqrt())).exp();
        let p = BarrierParams::with_df(spot, strike, shifted_barrier, t, df, div_yield, vol)
            .expect("positive df constructs");
        let expected = barrier_call_continuous(&p, AnalyticalBarrierType::DownOut);

        assert!((pv - expected).abs() < 1e-12);
    }

    /// W-09: the Broadie-Glasserman-Kou discrete-monitoring shift must not move
    /// the effective barrier across spot. A down-and-out call whose contractual
    /// barrier sits *above* spot is already knocked out (price ~0). With a
    /// large monitoring interval the unguarded shift `barrier · exp(-βσ√Δt)`
    /// can drop the effective barrier below spot, which makes the analytical
    /// formula treat the option as alive and return a spurious positive value.
    ///
    /// The guard clamps the effective barrier just above spot so the
    /// already-knocked-out option stays priced at ~0.
    #[test]
    fn w09_bgk_shift_does_not_cross_spot_for_knocked_out_barrier() {
        let as_of = date(2024, 1, 1);
        let expiry = date(2025, 1, 1);
        let spot = 100.0;
        let strike = 100.0;
        // Down-and-out barrier ABOVE spot: the option is already knocked out.
        let barrier = 130.0;
        let vol = 0.60;
        let rate = 0.05;
        let div_yield = 0.0;
        // A very large monitoring interval makes βσ√Δt large enough that the
        // unguarded shift would push the effective barrier below spot.
        let monitoring_dt = 4.0;

        let option = BarrierOption {
            monitoring_frequency: Some(monitoring_dt),
            ..down_and_out_call(expiry, strike, barrier)
        };
        let market = market(as_of, spot, vol, rate, div_yield);
        let pv = option.value(&market, as_of).expect("barrier pv").amount();

        // The unguarded shift would have produced this (effective barrier far
        // below spot → option treated as alive → large positive price).
        let unguarded_barrier = barrier * (-(BG_BETA * vol * monitoring_dt.sqrt())).exp();
        assert!(
            unguarded_barrier < spot,
            "test setup invalid: the unguarded shift must cross spot \
             (unguarded effective barrier {unguarded_barrier} should be < spot {spot})"
        );

        // With the guard the effective barrier stays at/above spot, so a
        // down-and-out whose barrier is above spot remains knocked out (~0).
        assert!(
            pv.abs() < 1e-6,
            "down-and-out call with barrier above spot is already knocked out \
             and must price to ~0, but got {pv}; the BGK shift crossed spot"
        );
    }

    /// Two-clock migration witness: when the discount curve's day-
    /// count differs from the instrument's (vol-surface) day-count,
    /// the MC pricer must use the curve's clock for the drift rate
    /// rather than the vol-surface clock. We exercise this by pricing
    /// the same barrier option against two curves that share a
    /// discount factor at expiry but differ in day-count, and assert
    /// the prices differ measurably. A single-clock `r_eff =
    /// -ln(DF)/t_vol` would collapse the two cases to the same price.

    #[test]
    fn two_clock_migration_drift_respects_curve_day_count() {
        use finstack_quant_monte_carlo::pricer::path_dependent::PathDependentPricerConfig;

        let as_of = date(2024, 1, 1);
        let expiry = date(2025, 1, 1); // 1 calendar year
        let spot = 100.0;
        let strike = 100.0;
        let barrier = 75.0;
        let vol = 0.25;
        let rate_365 = 0.05;
        let div_yield = 0.0;

        // Curve A: Act/365F, such that DF ≈ exp(-0.05 · 1.0) at t=1yr.
        // Curve B: Act/360, with knots anchored in Act/360 years. On
        // the Act/360 clock, 1 calendar year maps to 365/360 years, so
        // the same DF 0.9512 at `5.0` Act/360-years implies a slightly
        // different continuously-compounded rate relative to calendar
        // time.
        let df_at_5y_365 = (-rate_365 * 5.0_f64).exp();
        let market_365 = {
            let disc = DiscountCurve::builder("USD_DISC")
                .base_date(as_of)
                .day_count(DayCount::Act365F)
                .knots([(0.0, 1.0), (5.0, df_at_5y_365)])
                .build()
                .expect("disc curve 365");
            let surface = VolSurface::builder("SPX_VOL")
                .expiries(&[0.25, 0.5, 1.0, 2.0])
                .strikes(&[80.0, 90.0, 100.0, 110.0, 120.0])
                .row(&[vol; 5])
                .row(&[vol; 5])
                .row(&[vol; 5])
                .row(&[vol; 5])
                .build()
                .expect("surface");
            MarketContext::new()
                .insert(disc)
                .insert_surface(surface)
                .insert_price("SPX", MarketScalar::Price(Money::new(spot, Currency::USD)))
                .insert_price("SPX_DIV", MarketScalar::Unitless(div_yield))
        };
        let market_360 = {
            let disc = DiscountCurve::builder("USD_DISC")
                .base_date(as_of)
                .day_count(DayCount::Act360)
                .knots([(0.0, 1.0), (5.0, df_at_5y_365)])
                .build()
                .expect("disc curve 360");
            let surface = VolSurface::builder("SPX_VOL")
                .expiries(&[0.25, 0.5, 1.0, 2.0])
                .strikes(&[80.0, 90.0, 100.0, 110.0, 120.0])
                .row(&[vol; 5])
                .row(&[vol; 5])
                .row(&[vol; 5])
                .row(&[vol; 5])
                .build()
                .expect("surface");
            MarketContext::new()
                .insert(disc)
                .insert_surface(surface)
                .insert_price("SPX", MarketScalar::Price(Money::new(spot, Currency::USD)))
                .insert_price("SPX_DIV", MarketScalar::Unitless(div_yield))
        };

        // Use the MC pricer explicitly so the drift branch is exercised.
        let mc_pricer = BarrierOptionMcPricer {
            config: PathDependentPricerConfig {
                num_paths: 4_000,
                seed: 42,
                steps_per_year: 50.0,
                min_steps: 50,
                ..Default::default()
            },
        };
        let option = down_and_out_call(expiry, strike, barrier);

        let pv_365 = mc_pricer
            .price_internal(&option, &market_365, as_of)
            .expect("price 365")
            .amount();
        let pv_360 = mc_pricer
            .price_internal(&option, &market_360, as_of)
            .expect("price 360")
            .amount();

        // Both prices must be finite and positive (ITM call, finite DF).
        assert!(pv_365.is_finite() && pv_365 > 0.0);
        assert!(pv_360.is_finite() && pv_360 > 0.0);

        // The DF at t=1cy is different between the two curves (same
        // knot placement in year-fraction units maps to different
        // calendar DF interpolants), AND the drift rate also differs
        // because it's now computed on the curve's own clock. A price
        // gap of > 1e-6 is the migration witness — pre-migration the
        // drift would have been identical once the DF was read, so
        // the gap below would be driven only by the DF and would be
        // markedly smaller.
        let gap = (pv_365 - pv_360).abs();
        assert!(
            gap > 1e-6,
            "pre-migration pricing would be nearly identical across curve \
             day-counts when DFs agree at the knots; two-clock plumbing \
             must now yield measurably different prices: pv_365={pv_365} \
             pv_360={pv_360}"
        );
    }

    #[test]
    fn analytical_pricer_matches_put_reference_branch() {
        let as_of = date(2024, 1, 1);
        let expiry = date(2024, 9, 1);
        let spot = 100.0;
        let strike = 95.0;
        let barrier = 120.0;
        let vol = 0.22;
        let rate = 0.04;
        let div_yield = 0.01;

        let option = BarrierOption {
            barrier_type: BarrierType::UpAndOut,
            option_type: OptionType::Put,
            barrier: Money::new(barrier, Currency::USD),
            ..down_and_out_call(expiry, strike, barrier)
        };
        let market = market(as_of, spot, vol, rate, div_yield);
        let pv = option.value(&market, as_of).expect("put pv").amount();

        let t = option
            .day_count
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("year fraction");
        let df = (-rate * t).exp();
        let p = BarrierParams::with_df(spot, strike, barrier, t, df, div_yield, vol)
            .expect("positive df constructs");
        let expected = barrier_put_continuous(&p, AnalyticalBarrierType::UpOut);

        assert!((pv - expected).abs() < 1e-12);
    }

    /// Verify that the MC pricer captures the terminal spot at the correct maturity step.
    ///
    /// A degenerate up-and-out call with barrier >> spot is effectively a vanilla call
    /// (zero knockout probability). Its MC price must therefore equal the Black-Scholes
    /// call price within MC standard error.
    ///
    /// To make the off-by-one detectable, we use `steps_per_year = 2.0` / `min_steps = 2`
    /// so the time grid has exactly 2 steps (dt = 0.5 years each).  With
    /// `maturity_step = num_steps - 1 = 1`, the payoff reads the spot at t = 0.5
    /// instead of t = 1.0 — a 50% underestimate of the horizon.  The resulting MC
    /// price would equal the BS call at T = 0.5 (≈ 6.89) rather than at T = 1.0
    /// (≈ 10.47), a gap of ~3.6 that is far outside 5 MC standard errors (≈ 0.22).
    ///
    /// After the fix (`maturity_step = num_steps = 2`), the terminal spot is correctly
    /// read at step 2 (t = 1.0) and the MC price converges to the 1-year BS call.
    /// Black-Scholes call price (norm_cdf via Horner rational approximation; max err < 7.5e-8).
    fn bs_call_price(spot: f64, strike: f64, t: f64, r: f64, q: f64, sigma: f64) -> f64 {
        fn n(x: f64) -> f64 {
            if x < -8.0 {
                return 0.0;
            }
            if x > 8.0 {
                return 1.0;
            }
            let tt = 1.0 / (1.0 + 0.2316419 * x.abs());
            let poly = tt
                * (0.319_381_53
                    + tt * (-0.356_563_782
                        + tt * (1.781_477_937 + tt * (-1.821_255_978 + tt * 1.330_274_429))));
            let phi = (-0.5 * x * x).exp() / (2.0 * std::f64::consts::PI).sqrt();
            let c = 1.0 - phi * poly;
            if x >= 0.0 {
                c
            } else {
                1.0 - c
            }
        }
        let d1 = ((spot / strike).ln() + (r - q + 0.5 * sigma * sigma) * t) / (sigma * t.sqrt());
        let d2 = d1 - sigma * t.sqrt();
        spot * (-q * t).exp() * n(d1) - strike * (-r * t).exp() * n(d2)
    }

    /// W-43 / W-44 cross-check: the MC barrier pricer with the Brownian
    /// bridge active (`use_gobet_miri = true`) estimates the
    /// *continuously*-monitored barrier price, because the bridge fills in
    /// between-step crossings. It must therefore agree with the analytical
    /// continuous-monitoring pricer (`use_gobet_miri = false`,
    /// `monitoring_frequency = None`) within Monte Carlo error.
    ///
    /// Before W-43 the MC payoff layered the Broadie–Glasserman–Kou barrier
    /// shift on top of the bridge, biasing the price by order `βσ√Δt`; the
    /// parity below would then fail. Before W-44 the shift also pointed the
    /// wrong way, compounding the error.
    #[test]
    fn mc_bridge_barrier_matches_analytical_continuous() {
        use finstack_quant_monte_carlo::pricer::path_dependent::PathDependentPricerConfig;

        let as_of = date(2024, 1, 1);
        let expiry = date(2025, 1, 1); // 1 year
        let spot = 100.0;
        let strike = 100.0;
        let barrier = 80.0;
        let vol = 0.20;
        let rate = 0.05;
        let div_yield = 0.0;

        let market = market(as_of, spot, vol, rate, div_yield);

        // Analytical continuous-monitoring reference.
        let analytical = down_and_out_call(expiry, strike, barrier);
        let analytical_pv = analytical
            .value(&market, as_of)
            .expect("analytical pv")
            .amount();

        // MC with the bridge active. A fine time grid keeps the bridge
        // approximation tight; a large path count keeps MC error small.
        let mc_option = BarrierOption {
            use_gobet_miri: true,
            ..down_and_out_call(expiry, strike, barrier)
        };
        let mc_pricer = BarrierOptionMcPricer {
            config: PathDependentPricerConfig {
                num_paths: 200_000,
                seed: 20240101,
                steps_per_year: 100.0,
                min_steps: 100,
                ..Default::default()
            },
        };
        let mc_pv = mc_pricer
            .price_internal(&mc_option, &market, as_of)
            .expect("mc pv")
            .amount();

        // The bridge MC must converge to the continuous-monitoring price.
        // A double-counted BGK shift of order βσ√Δt with σ=0.2, Δt=0.01
        // would bias the barrier by ~1.2% — far outside this tolerance.
        let tol = 0.15;
        assert!(
            (mc_pv - analytical_pv).abs() < tol,
            "MC barrier with bridge must match analytical continuous price \
             within {tol}: mc={mc_pv:.6}, analytical={analytical_pv:.6}, \
             diff={:.6}",
            (mc_pv - analytical_pv).abs()
        );
    }

    #[test]
    fn barrier_uao_degenerate_matches_bs() {
        use finstack_quant_monte_carlo::pricer::path_dependent::PathDependentPricerConfig;

        let as_of = date(2024, 1, 1);
        let expiry = date(2025, 1, 1); // 1-year
        let spot = 100.0_f64;
        let strike = 100.0_f64;
        let barrier = 10_000.0_f64; // Far above spot: knockout probability ≈ 0
        let vol = 0.20_f64;
        let rate = 0.05_f64;
        let q = 0.0_f64;

        let t = DayCount::Act365F
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("year fraction");

        // Analytic Black-Scholes call at T = 1 year
        let bs_price_t1 = bs_call_price(spot, strike, t, rate, q, vol);

        // 2-step grid: num_steps = max(round(t * 2.0), 2) = 2.
        // Bug: maturity_step = 1 → spot read at t = 0.5 → price ≈ bs_call(t=0.5) ≈ 6.89.
        // Fix: maturity_step = 2 → spot read at t = 1.0 → price ≈ bs_call(t=1.0) ≈ 10.47.
        let mc_pricer = BarrierOptionMcPricer {
            config: PathDependentPricerConfig {
                num_paths: 200_000,
                seed: 20240101,
                steps_per_year: 2.0, // forces num_steps = 2
                min_steps: 2,
                ..Default::default()
            },
        };

        let option = BarrierOption {
            id: InstrumentId::new("BARRIER-UAO-DEGEN-UNIT"),
            underlying_ticker: "SPX".to_string(),
            strike,
            barrier: Money::new(barrier, Currency::USD),
            rebate: None,
            rebate_timing: Default::default(),
            option_type: OptionType::Call,
            barrier_type: BarrierType::UpAndOut,
            expiry,
            observed_barrier_breached: None,
            notional: Money::new(1.0, Currency::USD),
            day_count: DayCount::Act365F,
            use_gobet_miri: false,
            discount_curve_id: "USD_DISC".into(),
            spot_id: "SPX".into(),
            vol_surface_id: "SPX_VOL".into(),
            div_yield_id: Some("SPX_DIV".into()),
            pricing_overrides: PricingOverrides::default(),
            monitoring_frequency: None,
            attributes: Attributes::new(),
        };

        let mkt = market(as_of, spot, vol, rate, q);
        let mc_pv = mc_pricer
            .price_internal(&option, &mkt, as_of)
            .expect("mc price")
            .amount();

        // MC std error ≈ BS/sqrt(N) ≈ 10.47/sqrt(200_000) ≈ 0.023.
        // Off-by-one bias with 2 steps ≈ bs_call(T=1) - bs_call(T=0.5) ≈ 3.58.
        // Tight tolerance: 0.25 (≈ 11 std errors).  Bug fails spectacularly; fix passes.
        let tolerance = 0.25_f64;
        println!("BS call (T=1): {bs_price_t1:.6}");
        println!("MC call:       {mc_pv:.6}");
        println!("Difference:    {:.6}", (mc_pv - bs_price_t1).abs());
        println!("Tolerance:     {tolerance:.6}");
        assert!(
            (mc_pv - bs_price_t1).abs() < tolerance,
            "MC up-and-out call with far barrier must match BS call at T=1 within {tolerance}: \
             mc={mc_pv:.6}, bs={bs_price_t1:.6}, diff={:.6}. \
             Likely cause: maturity_step is set one step too early.",
            (mc_pv - bs_price_t1).abs()
        );
    }
}
