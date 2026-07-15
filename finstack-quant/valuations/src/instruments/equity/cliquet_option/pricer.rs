//! Cliquet option Monte Carlo pricer.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::equity::cliquet_option::monte_carlo::CliquetCallPayoff;
use crate::instruments::equity::cliquet_option::types::{CliquetOption, CliquetPayoffType};
use crate::instruments::equity::piecewise_gbm::PiecewiseExactGbm;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_monte_carlo::engine::{McEngine, McEngineConfig};
use finstack_quant_monte_carlo::pricer::path_dependent::PathDependentPricerConfig;
use finstack_quant_monte_carlo::rng::philox::PhiloxRng;
use finstack_quant_monte_carlo::time_grid::TimeGrid;

/// Cliquet option Monte Carlo pricer.
pub struct CliquetOptionMcPricer {
    config: PathDependentPricerConfig,
}

impl CliquetOptionMcPricer {
    /// Create a new cliquet option MC pricer with default config.
    pub fn new() -> Self {
        Self {
            config: PathDependentPricerConfig::default(),
        }
    }

    /// Price a cliquet option using Monte Carlo.
    fn price_internal(
        &self,
        inst: &CliquetOption,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<finstack_quant_core::money::Money> {
        inst.validate()?;
        if as_of > inst.expiry {
            return Ok(Money::new(0.0, inst.notional.currency()));
        }
        let spot_scalar = curves.get_price(&inst.spot_id)?;
        let initial_spot = match spot_scalar {
            finstack_quant_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
            finstack_quant_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
        };

        // Get curves
        let disc_curve = curves.get_discount(inst.discount_curve_id.as_str())?;

        // Deterministic evaluation of past reset periods (seasoned trade).
        //
        // Reset dates on or before as_of are fully observed: their period
        // returns are locked in and must come from recorded fixings, never
        // from simulated spot (the old `t > 0` filter silently repriced a
        // mid-life cliquet as a shorter new contract).
        let n_past = inst.reset_dates.iter().take_while(|&&d| d <= as_of).count();
        let has_strictly_past = inst.reset_dates.iter().take(n_past).any(|&d| d < as_of);
        if has_strictly_past && inst.initial_level.is_none() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CliquetOption '{}' has reset dates before as_of {} but no initial_level; \
                 the strike-set level is required to compute locked-in period returns",
                inst.id, as_of
            )));
        }

        // Walk the observed anchor chain: strike-set level, then each past
        // reset fixing. A reset date equal to as_of may fall back to the
        // current spot (today's level is observable). When no initial_level
        // is given (fresh trade), a reset at as_of is a strike-set event,
        // not a period observation (W-36).
        let mut anchor: Option<f64> = inst.initial_level;
        let mut locked_sum = 0.0;
        let mut locked_growth = 1.0;
        for &d in &inst.reset_dates[..n_past] {
            let level = match inst.fixing_on(d) {
                Some(v) => v,
                None if d == as_of => initial_spot,
                None => {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "CliquetOption '{}': reset date {} is before as_of {} but has no \
                         entry in past_fixings; provide the observed fixing to price this \
                         seasoned trade",
                        inst.id, d, as_of
                    )));
                }
            };
            if let Some(prev) = anchor {
                let period_return = (level / prev - 1.0)
                    .max(inst.local_floor)
                    .min(inst.local_cap);
                locked_sum += period_return;
                locked_growth *= 1.0 + period_return;
            }
            anchor = Some(level);
        }
        // Anchor for the first simulated period: last observed reset level,
        // else the strike-set level, else (fresh trade) the current spot.
        let sim_anchor = anchor.or(inst.initial_level).unwrap_or(initial_spot);

        let future_resets: Vec<Date> = inst.reset_dates[n_past..].to_vec();

        // All periods observed: the payoff is fully determined; discount the
        // known cashflow from the contract expiry.
        if future_resets.is_empty() {
            if as_of > inst.expiry {
                return Ok(Money::new(0.0, inst.notional.currency()));
            }
            let total_return = match inst.payoff_type {
                CliquetPayoffType::Additive => locked_sum,
                CliquetPayoffType::Multiplicative => locked_growth - 1.0,
            };
            let clamped = total_return
                .max(inst.global_floor)
                .min(inst.global_cap)
                .max(0.0);
            let df = if as_of == inst.expiry {
                1.0
            } else {
                disc_curve.df_between_dates(as_of, inst.expiry)?
            };
            return Ok(Money::new(
                clamped * inst.notional.amount() * df,
                inst.notional.currency(),
            ));
        }

        // Safe: future_resets is non-empty (checked above).
        let final_date = *future_resets.last().unwrap_or(&inst.expiry);
        let t = inst
            .day_count
            .year_fraction(as_of, final_date, DayCountContext::default())?;
        if t <= 0.0 {
            return Ok(Money::new(0.0, inst.notional.currency()));
        }

        // Dividend yield from scalar id if provided
        //
        // When a dividend yield ID is explicitly provided, we require the lookup to succeed
        // and return a unitless scalar. Silent fallback to 0.0 would mask market data
        // configuration errors.
        let div_yield = if let Some(div_id) = &inst.div_yield_id {
            let ms = curves.get_price(div_id.as_str()).map_err(|e| {
                finstack_quant_core::Error::Validation(format!(
                    "Failed to fetch dividend yield '{}': {}",
                    div_id, e
                ))
            })?;
            match ms {
                finstack_quant_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
                finstack_quant_core::market_data::scalars::MarketScalar::Price(m) => {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "Dividend yield '{}' should be a unitless scalar, got Price({})",
                        div_id,
                        m.currency()
                    )));
                }
            }
        } else {
            0.0
        };

        // Period boundaries for the forward-vol/rate bootstrap: the remaining
        // reset dates plus the final maturity so the process covers the whole
        // horizon.
        let mut check_points: Vec<f64> = future_resets
            .iter()
            .map(|d| {
                inst.day_count
                    .year_fraction(as_of, *d, DayCountContext::default())
            })
            .collect::<finstack_quant_core::Result<Vec<_>>>()?
            .into_iter()
            .filter(|&t| t > 0.0)
            .collect();
        check_points.sort_by(|a, b| a.total_cmp(b));
        check_points.dedup();
        if let Some(&last) = check_points.last() {
            if last < t - 1e-6 {
                check_points.push(t);
            }
        } else {
            check_points.push(t);
        }

        let process = crate::instruments::equity::piecewise_gbm::bootstrap_forward_gbm(
            disc_curve.as_ref(),
            curves,
            &inst.instrument_pricing_overrides.market_quotes,
            inst.vol_surface_id.as_str(),
            as_of,
            initial_spot,
            div_yield,
            &check_points,
            &format!("CliquetOption {}", inst.id),
        )?;

        let steps_per_year = self.config.steps_per_year;
        let num_steps = ((t * steps_per_year).round() as usize).max(self.config.min_steps);

        // Payoff reset times: the remaining (future) reset dates only.
        //
        // Past reset dates (including a strike-set reset at the contract
        // start, W-36) were already consumed by the deterministic seasoned
        // evaluation above; the payoff anchors its first simulated period to
        // `sim_anchor`. The `t > 0` filter is kept as a guard so the payoff
        // schedule stays consistent with `check_points` and the grid.
        let reset_times: Vec<f64> = future_resets
            .iter()
            .map(|&date| {
                inst.day_count
                    .year_fraction(as_of, date, DayCountContext::default())
            })
            .collect::<finstack_quant_core::Result<Vec<_>>>()?
            .into_iter()
            .filter(|&t| t > 0.0)
            .collect();

        // Derive deterministic seed from instrument ID and scenario

        use finstack_quant_monte_carlo::seed;

        let seed = if let Some(ref scenario) = inst.metric_pricing_overrides.mc_seed_scenario {
            seed::derive_seed(&inst.id, scenario)
        } else {
            seed::derive_seed(&inst.id, "base")
        };

        // Build time grid that includes reset dates to ensure exact period boundaries.
        // Without this, the MC simulation may not visit exact reset dates, leading to
        // interpolation error in the piecewise process and slightly wrong period returns.
        let mut grid_times = Vec::with_capacity(num_steps + reset_times.len() + 1);
        grid_times.push(0.0);

        // Add uniform steps
        let dt_grid = t / num_steps as f64;
        for i in 1..=num_steps {
            grid_times.push(i as f64 * dt_grid);
        }

        // Add reset times (ensure we visit exact dates)
        for &reset_t in &reset_times {
            if reset_t > 1e-10 && reset_t <= t {
                grid_times.push(reset_t);
            }
        }

        // Sort and dedup (prefer reset times when merging nearby points)
        grid_times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        grid_times.dedup_by(|a, b| (*a - *b).abs() < 1e-10);

        let time_grid = TimeGrid::from_times(grid_times)?;

        // Build payoff (consumes reset_times via move). The first simulated
        // period anchors to the last observed level (`sim_anchor`), and the
        // locked-in past returns seed the global cap/floor aggregation.
        let payoff = CliquetCallPayoff::new(
            reset_times,
            inst.local_cap,
            inst.local_floor,
            inst.global_cap,
            inst.global_floor,
            inst.notional.amount(),
            inst.notional.currency(),
            sim_anchor,
            inst.payoff_type,
        )?
        .with_prior_locked_returns(locked_sum, locked_growth);

        let merged_cfg = crate::instruments::common_impl::helpers::merged_path_config(
            &self.config,
            &inst.instrument_pricing_overrides,
        )?;
        let engine_config = McEngineConfig {
            num_paths: merged_cfg.num_paths,
            time_grid,
            target_ci_half_width: None,
            use_parallel: merged_cfg.use_parallel,
            chunk_size: Some(merged_cfg.chunk_size),
            path_capture: merged_cfg.path_capture.clone(),
            antithetic: merged_cfg.antithetic,
        };
        let engine = McEngine::new(engine_config);

        let rng = PhiloxRng::new(seed);
        let disc = PiecewiseExactGbm::new();
        let initial_state = vec![initial_spot];

        // Use the contract expiry rather than indexing reset_dates so this
        // remains panic-free if reset_dates is somehow empty here.
        let maturity_date = inst.reset_dates.last().copied().unwrap_or(inst.expiry);
        let discount_factor = disc_curve.df_between_dates(as_of, maturity_date)?;

        let result = engine.price(
            &rng,
            &process,
            &disc,
            &initial_state,
            &payoff,
            inst.notional.currency(),
            discount_factor,
        )?;

        Ok(result.mean)
    }
}

impl Default for CliquetOptionMcPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for CliquetOptionMcPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::CliquetOption, ModelKey::MonteCarloGBM)
    }

    #[tracing::instrument(
        name = "cliquet_option.mc.price_dyn",
        level = "debug",
        skip(self, instrument, market),
        fields(inst_id = %instrument.id(), as_of = %as_of),
        err,
    )]
    fn price_dyn(
        &self,
        instrument: &dyn crate::instruments::common_impl::traits::Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let cliquet = instrument
            .as_any()
            .downcast_ref::<CliquetOption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::CliquetOption, instrument.key())
            })?;

        let pv = self.price_internal(cliquet, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(
                e.to_string(),
                PricingErrorContext::from_instrument(cliquet).model(ModelKey::MonteCarloGBM),
            )
        })?;

        Ok(ValuationResult::stamped(cliquet.id(), as_of, pv))
    }
}

/// Present value using Monte Carlo.
pub(crate) fn compute_pv(
    inst: &CliquetOption,
    curves: &MarketContext,
    as_of: Date,
) -> Result<Money> {
    let pricer = CliquetOptionMcPricer::new();
    pricer.price_internal(inst, curves, as_of)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::equity::cliquet_option::types::{CliquetOption, CliquetPayoffType};
    use crate::instruments::Attributes;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use time::Month;

    fn date(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(year, Month::try_from(month).expect("valid month"), day)
            .expect("valid date")
    }

    fn market(as_of: Date) -> MarketContext {
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
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
            .insert_price("SPX-DIV", MarketScalar::Unitless(0.01))
    }

    fn live_option() -> CliquetOption {
        CliquetOption::builder()
            .id(InstrumentId::new("CLIQ-TEST"))
            .underlying_ticker("SPX".to_string())
            .reset_dates(vec![date(2024, 6, 30), date(2024, 12, 31)])
            .expiry(date(2024, 12, 31))
            .local_cap(0.05)
            .local_floor(0.0)
            .global_cap(0.20)
            .global_floor(0.0)
            .notional(Money::new(100_000.0, Currency::USD))
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(Some(CurveId::new("SPX-DIV")))
            .attributes(Attributes::new())
            .build()
            .expect("cliquet option")
    }

    #[test]
    fn expired_cliquet_returns_zero_for_price_and_unitless_spot() {
        let as_of = date(2025, 1, 1);
        let option = CliquetOption::example().expect("example");

        let unitless_market = market(as_of);
        let price_market = unitless_market.clone().insert_price(
            "SPX-SPOT",
            MarketScalar::Price(Money::new(100.0, Currency::USD)),
        );

        let pv_unitless = compute_pv(&option, &unitless_market, as_of).expect("unitless pv");
        let pv_price = compute_pv(&option, &price_market, as_of).expect("price pv");

        assert_eq!(pv_unitless.amount(), 0.0);
        assert_eq!(pv_price.amount(), 0.0);
    }

    #[test]
    fn cliquet_rejects_missing_dividend_yield_when_id_is_configured() {
        let as_of = date(2024, 1, 1);
        let option = live_option();
        let err = compute_pv(&option, &market(as_of), as_of).expect("base case should succeed");
        assert!(err.amount().is_finite());

        let missing_div_market = MarketContext::new()
            .insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(as_of)
                    .day_count(finstack_quant_core::dates::DayCount::Act365F)
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

        let err =
            compute_pv(&option, &missing_div_market, as_of).expect_err("missing div should error");
        assert!(err.to_string().contains("Failed to fetch dividend yield"));
    }

    #[test]
    fn cliquet_rejects_price_scalar_dividend_yield_and_keeps_multiplicative_seeded() {
        let as_of = date(2024, 1, 1);
        let mut option = live_option();
        option.payoff_type = CliquetPayoffType::Multiplicative;

        let bad_market = market(as_of).insert_price(
            "SPX-DIV",
            MarketScalar::Price(Money::new(1.0, Currency::USD)),
        );
        let err = compute_pv(&option, &bad_market, as_of).expect_err("price div should error");
        assert!(err.to_string().contains("unitless scalar"));

        let good_market = market(as_of);
        let pv1 = compute_pv(&option, &good_market, as_of).expect("pv1");
        let pv2 = compute_pv(&option, &good_market, as_of).expect("pv2");
        assert_eq!(pv1.amount(), pv2.amount());
    }

    /// W-37: a non-monotone total-variance surface (calendar-spread arbitrage)
    /// must be handled explicitly by the forward-vol bootstrap. The forward
    /// variance over a period whose total variance *decreases* is negative and
    /// impossible; the bootstrap must floor it to zero (not silently substitute
    /// the terminal vol) and still produce a finite, deterministic price.
    #[test]
    fn cliquet_non_monotone_total_variance_surface_is_handled() {
        let as_of = date(2024, 1, 1);

        // Steeply *inverted* vol term structure: short-dated vol far exceeds
        // long-dated vol, so total variance σ²·t is NON-monotone in maturity.
        //   t = 0.5: σ = 0.60 => total var = 0.36 * 0.5 = 0.18
        //   t = 1.0: σ = 0.20 => total var = 0.04 * 1.0 = 0.04  (< 0.18!)
        let tv_short = 0.60_f64 * 0.60 * 0.5;
        let tv_long = 0.20_f64 * 0.20 * 1.0;
        assert!(
            tv_long < tv_short,
            "test surface must be non-monotone in total variance: \
             short={tv_short} long={tv_long}"
        );

        let inverted_surface = VolSurface::builder("SPX-VOL")
            .expiries(&[0.25, 0.5, 1.0, 2.0])
            .strikes(&[80.0, 100.0, 120.0, 140.0])
            .row(&[0.80, 0.80, 0.80, 0.80])
            .row(&[0.60, 0.60, 0.60, 0.60])
            .row(&[0.20, 0.20, 0.20, 0.20])
            .row(&[0.15, 0.15, 0.15, 0.15])
            .build()
            .expect("inverted surface");
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, 0.97), (2.0, 0.94)])
            .build()
            .expect("curve");
        let market = MarketContext::new()
            .insert(curve)
            .insert_surface(inverted_surface)
            .insert_price("SPX-SPOT", MarketScalar::Unitless(100.0))
            .insert_price("SPX-DIV", MarketScalar::Unitless(0.01));

        // Reset dates at ~0.5y and ~1.0y exercise the non-monotone period.
        let option = CliquetOption::builder()
            .id(InstrumentId::new("CLIQ-NONMONO"))
            .underlying_ticker("SPX".to_string())
            .reset_dates(vec![date(2024, 7, 1), date(2024, 12, 31)])
            .expiry(date(2024, 12, 31))
            .local_cap(0.05)
            .local_floor(0.0)
            .global_cap(0.20)
            .global_floor(0.0)
            .notional(Money::new(100_000.0, Currency::USD))
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(Some(CurveId::new("SPX-DIV")))
            .attributes(Attributes::new())
            .build()
            .expect("non-monotone cliquet");

        // The bootstrap must not panic and must return a finite, non-negative
        // price (the long cliquet call payoff is floored at zero).
        let pv1 = compute_pv(&option, &market, as_of)
            .expect("non-monotone surface must price, not panic");
        assert!(
            pv1.amount().is_finite() && pv1.amount() >= 0.0,
            "non-monotone surface must yield a finite non-negative price; got {}",
            pv1.amount()
        );

        // Determinism (seeded RNG) holds through the non-monotone branch.
        let pv2 = compute_pv(&option, &market, as_of).expect("repeat price");
        assert_eq!(pv1.amount(), pv2.amount());
    }

    /// Seasoned cliquet: reset dates strictly before as_of without fixings
    /// (or without the strike-set level) must error, never silently reprice
    /// as a shorter new contract.
    #[test]
    fn seasoned_cliquet_requires_initial_level_and_fixings() {
        let as_of = date(2024, 9, 1); // first reset (2024-06-30) is past
        let option = live_option();
        let mkt = market(as_of);

        let err = compute_pv(&option, &mkt, as_of).expect_err("missing initial_level");
        assert!(
            err.to_string().contains("initial_level"),
            "expected initial_level error, got: {err}"
        );

        let mut with_level = live_option();
        with_level.initial_level = Some(100.0);
        let err = compute_pv(&with_level, &mkt, as_of).expect_err("missing fixing");
        assert!(
            err.to_string().contains("past_fixings"),
            "expected past_fixings error, got: {err}"
        );
    }

    /// A seasoned cliquet must carry its locked-in past period returns: with a
    /// positive locked-in return it is worth strictly more than the identical
    /// contract whose past period return was zero.
    #[test]
    fn seasoned_cliquet_carries_locked_in_returns() {
        let as_of = date(2024, 9, 1);
        let mkt = market(as_of);

        // Past period 100 -> 104 (capped at 5%): locked-in +4%.
        let mut up = live_option();
        up.initial_level = Some(100.0);
        up.past_fixings = vec![(date(2024, 6, 30), 104.0)];

        // Past period flat: locked-in 0%.
        let mut flat = live_option();
        flat.initial_level = Some(100.0);
        flat.past_fixings = vec![(date(2024, 6, 30), 100.0)];

        let pv_up = compute_pv(&up, &mkt, as_of).expect("pv up");
        let pv_flat = compute_pv(&flat, &mkt, as_of).expect("pv flat");
        assert!(
            pv_up.amount() > pv_flat.amount(),
            "locked-in +4% must be worth more than locked-in 0%: up={} flat={}",
            pv_up.amount(),
            pv_flat.amount()
        );
        // The locked-in return is worth roughly 4% of notional discounted; it
        // must not have been silently discarded (old behavior: identical PVs).
        assert!(
            pv_up.amount() - pv_flat.amount() > 0.01 * 100_000.0,
            "locked-in return contribution too small: diff={}",
            pv_up.amount() - pv_flat.amount()
        );
    }

    /// A cliquet whose reset dates are all observed is a deterministic
    /// cashflow: clamp the locked-in returns and discount from expiry.
    #[test]
    fn fully_observed_cliquet_is_deterministic() {
        let as_of = date(2025, 1, 10);
        let mkt = market(as_of);

        let mut option = live_option();
        option.expiry = date(2025, 1, 31); // settlement after last reset
        option.initial_level = Some(100.0);
        // Periods: 100->103 (+3%), 103->105.06 (~+2%); both inside the 5% cap.
        option.past_fixings = vec![(date(2024, 6, 30), 103.0), (date(2024, 12, 31), 105.06)];

        let pv = compute_pv(&option, &mkt, as_of).expect("deterministic pv");

        let r1 = 0.03_f64.clamp(0.0, 0.05);
        let r2 = (105.06_f64 / 103.0 - 1.0).clamp(0.0, 0.05);
        let total = (r1 + r2).clamp(0.0, 0.20);
        let disc = mkt.get_discount("USD-OIS").expect("curve");
        let df = disc.df_between_dates(as_of, option.expiry).expect("df");
        let expected = total * 100_000.0 * df;
        assert!(
            (pv.amount() - expected).abs() < 1e-9,
            "fully observed cliquet must be deterministic: pv={} expected={expected}",
            pv.amount()
        );
    }

    /// W-36: a reset date at the contract start (`t == 0`) is a strike-set
    /// event, not a period observation. The payoff already anchors period 1 to
    /// `initial_spot`, so a t=0 reset must be economically a no-op: the cliquet
    /// must price identically to the same contract without that redundant reset.
    ///
    /// Before the fix, `reset_times` was built from ALL `reset_dates` with no
    /// `t > 0` filter, so the t=0 reset recorded `initial_spot` as
    /// `reset_spots[0]`. With a *positive* `local_floor` that phantom period
    /// contributes a guaranteed `local_floor` of fake return (`0.max(floor)`),
    /// inflating every path's payoff and making the two prices diverge.
    #[test]
    fn cliquet_t0_reset_is_a_strike_set_not_a_period() {
        // as_of coincides with the first reset date.
        let as_of = date(2024, 1, 1);

        // Positive local floor: a guaranteed-zero phantom period would pay
        // `local_floor`, so the bug is observable as a price difference.
        let local_floor = 0.01;

        // Contract WITH a redundant strike-set reset at the contract start.
        let with_t0 = CliquetOption::builder()
            .id(InstrumentId::new("CLIQ-T0"))
            .underlying_ticker("SPX".to_string())
            .reset_dates(vec![as_of, date(2024, 6, 30), date(2024, 12, 31)])
            .expiry(date(2024, 12, 31))
            .local_cap(0.05)
            .local_floor(local_floor)
            .global_cap(0.50)
            .global_floor(0.0)
            .notional(Money::new(100_000.0, Currency::USD))
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(Some(CurveId::new("SPX-DIV")))
            .attributes(Attributes::new())
            .build()
            .expect("with-t0 cliquet");

        // Same contract WITHOUT the redundant t=0 reset.
        let without_t0 = CliquetOption::builder()
            .id(InstrumentId::new("CLIQ-T0"))
            .underlying_ticker("SPX".to_string())
            .reset_dates(vec![date(2024, 6, 30), date(2024, 12, 31)])
            .expiry(date(2024, 12, 31))
            .local_cap(0.05)
            .local_floor(local_floor)
            .global_cap(0.50)
            .global_floor(0.0)
            .notional(Money::new(100_000.0, Currency::USD))
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(Some(CurveId::new("SPX-DIV")))
            .attributes(Attributes::new())
            .build()
            .expect("without-t0 cliquet");

        let mkt = market(as_of);
        let pv_with = compute_pv(&with_t0, &mkt, as_of).expect("pv with t0 reset");
        let pv_without = compute_pv(&without_t0, &mkt, as_of).expect("pv without t0 reset");

        assert_eq!(
            pv_with.amount(),
            pv_without.amount(),
            "a t=0 strike-set reset must not change the cliquet price; \
             with={} without={}",
            pv_with.amount(),
            pv_without.amount()
        );
    }
}
