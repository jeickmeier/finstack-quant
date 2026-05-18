//! Cliquet option Monte Carlo pricer.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::equity::cliquet_option::monte_carlo::{
    CliquetCallPayoff, CliquetPayoffType as McPayoffType,
};
use crate::instruments::equity::cliquet_option::types::{CliquetOption, CliquetPayoffType};
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext, PricingResult,
};
use crate::results::ValuationResult;
use finstack_core::dates::{Date, DayCountContext};
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::Result;
use finstack_monte_carlo::engine::{McEngine, McEngineConfig};
use finstack_monte_carlo::paths::ProcessParams;
use finstack_monte_carlo::pricer::path_dependent::PathDependentPricerConfig;
use finstack_monte_carlo::process::metadata::ProcessMetadata;
use finstack_monte_carlo::rng::philox::PhiloxRng;
use finstack_monte_carlo::time_grid::TimeGrid;
use finstack_monte_carlo::traits::Discretization;
use finstack_monte_carlo::traits::StochasticProcess;

/// Piecewise constant GBM process for cliquet options.
/// Handles term structure of volatility and rates between reset dates.
#[derive(Debug, Clone)]
struct PiecewiseGbmProcess {
    /// End times of intervals (sorted)
    times: Vec<f64>,
    /// Risk-free rates per interval
    rs: Vec<f64>,
    /// Dividend yields per interval
    qs: Vec<f64>,
    /// Volatilities per interval
    sigmas: Vec<f64>,
}

impl StochasticProcess for PiecewiseGbmProcess {
    fn dim(&self) -> usize {
        1
    }

    fn num_factors(&self) -> usize {
        1
    }

    fn drift(&self, t: f64, x: &[f64], out: &mut [f64]) {
        let idx = self.times.partition_point(|&time| time < t);
        let idx = idx.min(self.times.len() - 1);
        // μ(S) = (r - q) S
        out[0] = (self.rs[idx] - self.qs[idx]) * x[0];
    }

    fn diffusion(&self, t: f64, x: &[f64], out: &mut [f64]) {
        let idx = self.times.partition_point(|&time| time < t);
        let idx = idx.min(self.times.len() - 1);
        // σ(S) = σ S
        out[0] = self.sigmas[idx] * x[0];
    }
}

impl ProcessMetadata for PiecewiseGbmProcess {
    fn metadata(&self) -> ProcessParams {
        let mut params = ProcessParams::new("PiecewiseGBM");
        // Just report first interval params as representative for metadata
        if !self.rs.is_empty() {
            params.add_param("r_initial", self.rs[0]);
            params.add_param("q_initial", self.qs[0]);
            params.add_param("sigma_initial", self.sigmas[0]);
        }
        params.with_factors(vec!["spot".to_string()])
    }
}

/// Exact discretization for Piecewise GBM.
#[derive(Debug, Clone, Default)]
struct PiecewiseExactGbm;

impl PiecewiseExactGbm {
    fn new() -> Self {
        Self
    }
}

impl Discretization<PiecewiseGbmProcess> for PiecewiseExactGbm {
    fn step(
        &self,
        process: &PiecewiseGbmProcess,
        t: f64,
        dt: f64,
        x: &mut [f64],
        z: &[f64],
        _work: &mut [f64],
    ) {
        let idx = process.times.partition_point(|&time| time < t);
        let idx = idx.min(process.times.len() - 1);

        let r = process.rs[idx];
        let q = process.qs[idx];
        let sigma = process.sigmas[idx];

        // S(t+dt) = S(t) * exp( (r - q - 0.5*sigma^2)*dt + sigma*sqrt(dt)*Z )
        let drift = (r - q - 0.5 * sigma * sigma) * dt;
        let diffusion = sigma * dt.sqrt() * z[0];
        x[0] *= (drift + diffusion).exp();
    }

    fn work_size(&self, _process: &PiecewiseGbmProcess) -> usize {
        0
    }
}

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
    ) -> Result<finstack_core::money::Money> {
        let spot_scalar = curves.get_price(&inst.spot_id)?;
        let initial_spot = match spot_scalar {
            finstack_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
            finstack_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
        };

        let final_date = inst.reset_dates.last().copied().unwrap_or(as_of);
        let t = inst
            .day_count
            .year_fraction(as_of, final_date, DayCountContext::default())?;
        if t <= 0.0 {
            return Ok(Money::new(0.0, inst.notional.currency()));
        }

        // Get curves
        let disc_curve = curves.get_discount(inst.discount_curve_id.as_str())?;

        // Dividend yield from scalar id if provided
        //
        // When a dividend yield ID is explicitly provided, we require the lookup to succeed
        // and return a unitless scalar. Silent fallback to 0.0 would mask market data
        // configuration errors.
        let div_yield = if let Some(div_id) = &inst.div_yield_id {
            let ms = curves.get_price(div_id.as_str()).map_err(|e| {
                finstack_core::Error::Validation(format!(
                    "Failed to fetch dividend yield '{}': {}",
                    div_id, e
                ))
            })?;
            match ms {
                finstack_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
                finstack_core::market_data::scalars::MarketScalar::Price(m) => {
                    return Err(finstack_core::Error::Validation(format!(
                        "Dividend yield '{}' should be a unitless scalar, got Price({})",
                        div_id,
                        m.currency()
                    )));
                }
            }
        } else {
            0.0
        };

        // Build piecewise parameters
        // Interval boundaries: reset dates
        let mut times = Vec::new();
        let mut rs = Vec::new();
        let mut qs = Vec::new();
        let mut sigmas = Vec::new();

        let mut prev_t = 0.0;
        let mut prev_var = 0.0;

        // Combine reset dates into a sorted list of times including maturity
        let mut check_points: Vec<f64> = inst
            .reset_dates
            .iter()
            .map(|d| {
                inst.day_count
                    .year_fraction(as_of, *d, DayCountContext::default())
            })
            .collect::<finstack_core::Result<Vec<_>>>()?
            .into_iter()
            .filter(|&t| t > 0.0)
            .collect();
        check_points.sort_by(|a, b| a.total_cmp(b));
        check_points.dedup();

        // Ensure we cover up to t if not in reset dates
        if let Some(&last) = check_points.last() {
            if last < t - 1e-6 {
                check_points.push(t);
            }
        } else {
            check_points.push(t);
        }

        // Hoist loop-invariant year fractions out of the per-period loop:
        // disc_curve.df(t) takes a curve-time relative to base_date, so we
        // need t_curve = year_fraction(base_date, as_of) + period_offset.
        let t_base_to_as_of = disc_curve.day_count().year_fraction(
            disc_curve.base_date(),
            as_of,
            DayCountContext::default(),
        )?;
        let df_base_to_as_of = disc_curve.df(t_base_to_as_of);

        for &curr_t in &check_points {
            if curr_t <= prev_t {
                continue;
            }

            let df_prev = disc_curve.df(t_base_to_as_of + prev_t);
            let df_curr = disc_curve.df(t_base_to_as_of + curr_t);
            let dt = curr_t - prev_t;

            // Forward rate over [prev_t, curr_t] using df_prev / df_curr.
            // Reject degenerate curve data instead of silently using r=0.
            if df_curr <= 0.0 || !df_curr.is_finite() {
                return Err(finstack_core::Error::Validation(format!(
                    "CliquetOption: discount factor at t={} is non-positive ({}); \
                     curve '{}' is degenerate or extrapolated past its valid range",
                    curr_t, df_curr, inst.discount_curve_id
                )));
            }
            if dt <= 1e-6 {
                return Err(finstack_core::Error::Validation(format!(
                    "CliquetOption: degenerate time step dt={} between periods at \
                     t_prev={} and t_curr={}; check that reset_dates are distinct",
                    dt, prev_t, curr_t
                )));
            }
            let fwd_r = (df_prev / df_curr).ln() / dt;

            // ATM forward price for vol surface lookup:
            //   F(0, curr_t) = S_0 * exp(-q*curr_t) / DF(as_of, curr_t)
            //   DF(as_of, curr_t) = df_curr / df_base_to_as_of
            let forward_price =
                initial_spot * (-div_yield * curr_t).exp() / df_curr * df_base_to_as_of;

            let vol_curr = crate::instruments::common_impl::vol_resolution::resolve_sigma_at(
                &inst.pricing_overrides.market_quotes,
                curves,
                inst.vol_surface_id.as_str(),
                curr_t,
                forward_price,
            )?;
            let var_curr = vol_curr * vol_curr * curr_t;

            // Forward variance over [prev_t, curr_t] from the bootstrap:
            //   fwd_var = σ²(curr_t)·curr_t − σ²(prev_t)·prev_t.
            //
            // A well-formed (calendar-arbitrage-free) total-variance surface is
            // non-decreasing in maturity, so `fwd_var >= 0`. A non-monotone
            // surface yields `fwd_var < 0`, which is an impossible (negative)
            // period variance.
            //
            // The previous code silently substituted the *terminal* vol
            // `vol_curr` for that period — an arbitrary value unrelated to the
            // (degenerate) forward variance, mis-setting the period vol. Handle
            // the non-monotone case explicitly instead: floor the forward
            // variance at zero (the no-arbitrage minimum — that period simply
            // contributes ~no volatility) and emit a diagnostic so the bad
            // surface is visible in production logs.
            let fwd_var = var_curr - prev_var;
            let fwd_sigma = if fwd_var >= 0.0 {
                (fwd_var / dt).sqrt()
            } else {
                tracing::warn!(
                    instrument_id = %inst.id,
                    surface_id = %inst.vol_surface_id,
                    t_prev = prev_t,
                    t_curr = curr_t,
                    total_var_prev = prev_var,
                    total_var_curr = var_curr,
                    forward_variance = fwd_var,
                    "CliquetOption forward-vol bootstrap: total-variance surface is \
                     non-monotone over [t_prev, t_curr] (calendar-spread arbitrage); \
                     flooring the negative forward variance to zero for this period"
                );
                0.0
            };

            times.push(curr_t);
            rs.push(fwd_r);
            qs.push(div_yield);
            sigmas.push(fwd_sigma);

            prev_t = curr_t;
            prev_var = var_curr;
        }

        let process = PiecewiseGbmProcess {
            times,
            rs,
            qs,
            sigmas,
        };

        let steps_per_year = self.config.steps_per_year;
        let num_steps = ((t * steps_per_year).round() as usize).max(self.config.min_steps);

        // Payoff reset times.
        //
        // A reset date at the contract start (t == 0) is a strike-set event,
        // not a period observation: the payoff already anchors period 1 to
        // `initial_spot`. The MC engine fires `on_event` at t=0 before any
        // step, so passing a t=0 reset to the payoff records `initial_spot`
        // as `reset_spots[0]`, producing a guaranteed-zero period-1 return and
        // shifting every later period by one. Filter `t > 0` here so the
        // payoff schedule stays consistent with `check_points` and the grid.
        let reset_times: Vec<f64> = inst
            .reset_dates
            .iter()
            .map(|&date| {
                inst.day_count
                    .year_fraction(as_of, date, DayCountContext::default())
            })
            .collect::<finstack_core::Result<Vec<_>>>()?
            .into_iter()
            .filter(|&t| t > 0.0)
            .collect();

        let payoff_type = match inst.payoff_type {
            CliquetPayoffType::Additive => McPayoffType::Additive,
            CliquetPayoffType::Multiplicative => McPayoffType::Multiplicative,
        };

        // Derive deterministic seed from instrument ID and scenario

        use finstack_monte_carlo::seed;

        let seed = if let Some(ref scenario) = inst.pricing_overrides.metrics.mc_seed_scenario {
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

        // Build payoff (consumes reset_times via move)
        let payoff = CliquetCallPayoff::new(
            reset_times,
            inst.local_cap,
            inst.local_floor,
            inst.global_cap,
            inst.global_floor,
            inst.notional.amount(),
            inst.notional.currency(),
            initial_spot,
            payoff_type,
        )?;

        let merged_cfg = crate::instruments::common_impl::helpers::merged_path_config(
            &self.config,
            &inst.pricing_overrides,
        )?;
        let engine_config = McEngineConfig {
            num_paths: merged_cfg.num_paths,
            time_grid,
            target_ci_half_width: None,
            use_parallel: merged_cfg.use_parallel,
            chunk_size: merged_cfg.chunk_size,
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
    ) -> PricingResult<ValuationResult> {
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
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::instruments::equity::cliquet_option::types::{CliquetOption, CliquetPayoffType};
    use crate::instruments::{Attributes, PricingOverrides};
    use finstack_core::currency::Currency;
    use finstack_core::market_data::scalars::MarketScalar;
    use finstack_core::market_data::surfaces::VolSurface;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::money::Money;
    use finstack_core::types::{CurveId, InstrumentId};
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
            .day_count(finstack_core::dates::DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(Some(CurveId::new("SPX-DIV")))
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("cliquet option")
    }

    #[test]
    fn expired_cliquet_returns_zero_for_price_and_unitless_spot() {
        let as_of = date(2025, 1, 1);
        let mut option = CliquetOption::example().expect("example");
        option.reset_dates.clear();
        option.expiry = as_of;

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
        let err =
            compute_pv(&option, &market(as_of).clone(), as_of).expect("base case should succeed");
        assert!(err.amount().is_finite());

        let missing_div_market = MarketContext::new()
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
            .day_count(finstack_core::dates::DayCount::Act365F)
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
            .day_count(finstack_core::dates::DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(Some(CurveId::new("SPX-DIV")))
            .pricing_overrides(PricingOverrides::default())
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
            .day_count(finstack_core::dates::DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(Some(CurveId::new("SPX-DIV")))
            .pricing_overrides(PricingOverrides::default())
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
            .day_count(finstack_core::dates::DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(Some(CurveId::new("SPX-DIV")))
            .pricing_overrides(PricingOverrides::default())
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
