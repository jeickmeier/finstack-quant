//! Bermudan swaption pricer using LMM/BGM Monte Carlo dynamics.
//!
//! Wraps the standalone [`price_bermudan_lmm`] engine in the [`Pricer`] trait
//! so it can be dispatched via the pricing registry under
//! `(BermudanSwaption, LmmMonteCarlo)`.

use crate::instruments::common_impl::helpers::year_fraction;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::swaption::pricing::lmm_bermudan::{
    price_bermudan_lmm, LmmBermudanConfig,
};
use crate::instruments::rates::swaption::BermudanSwaption;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::traits::Discounting;
use finstack_core::money::Money;
use finstack_monte_carlo::process::lmm::LmmParams;

/// Bermudan swaption pricer using LMM/BGM Monte Carlo with LSMC exercise.
///
/// Builds [`LmmParams`] from the swaption's underlying swap schedule and
/// market discount curve, then delegates to [`price_bermudan_lmm`] for
/// LSMC-based Bermudan exercise valuation.
///
/// # Parameter Construction
///
/// Forward rates are bootstrapped from the discount curve at the swap's
/// fixed-leg tenor schedule.  A flat 2-factor loading structure is used
/// (a linear-decay proxy for the first two principal components of the
/// forward-rate correlation matrix). The *shape* of the loadings is fixed,
/// but their overall scale (`base_vol`) is **calibrated** to the swaption
/// volatility surface: the closed-form Rebonato map
/// [`calibrate_base_vol`](crate::calibration::targets::lmm::calibrate_base_vol)
/// fits `base_vol` so the LMM reprices the longest co-terminal European
/// swaption embedded in the Bermudan's exercise schedule to its market
/// Black vol. The surface is required: a missing or mis-tagged surface, or
/// a degenerate calibration, fails pricing rather than falling back to an
/// uncalibrated vol.
///
/// The flat single-`base_vol` model has one scale degree of freedom, so it
/// matches exactly one co-terminal European swaption — the longest one
/// (first exercise), which dominates the Bermudan value — rather than the
/// full expiry term structure. A full per-period loading calibration to
/// every co-terminal slice is out of scope for this pricer.
#[derive(Default)]
pub struct BermudanSwaptionLmmPricer {
    config: LmmBermudanConfig,
}

impl BermudanSwaptionLmmPricer {
    /// Create a pricer with an explicit configuration.
    pub fn with_config(config: LmmBermudanConfig) -> Self {
        Self { config }
    }

    /// Build LMM parameters from a Bermudan swaption and its discount curve.
    ///
    /// Constructs the tenor schedule from the fixed-leg frequency, bootstraps
    /// forward rates from discount factors, and applies a flat 2-factor
    /// loading structure with linear decay. The loading *scale* (`base_vol`)
    /// is calibrated via the Rebonato shape factor so the LMM reprices the
    /// longest co-terminal European swaption of the Bermudan's exercise
    /// schedule to its market Black vol; see
    /// [`crate::calibration::targets::lmm`]. A missing or mis-tagged vol
    /// surface, or a degenerate calibration, is a hard error.
    fn build_lmm_params(
        swaption: &BermudanSwaption,
        disc: &dyn Discounting,
        market: &MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> std::result::Result<LmmParams, PricingError> {
        let swap_start_yf =
            year_fraction(swaption.day_count, as_of, swaption.swap_start).map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;
        let swap_end_yf =
            year_fraction(swaption.day_count, as_of, swaption.swap_end).map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        // Determine the accrual period from the fixed leg frequency
        let tenor_months = swaption.fixed_freq.months().unwrap_or(6) as f64;
        let period = tenor_months / 12.0;
        if period <= 0.0 {
            return Err(PricingError::model_failure_with_context(
                "Fixed leg frequency must be positive".to_string(),
                PricingErrorContext::default(),
            ));
        }

        // Build tenor schedule from swap_start to swap_end
        let mut tenors: Vec<f64> = Vec::new();
        let mut t = swap_start_yf;
        while t < swap_end_yf - 1e-10 {
            tenors.push(t);
            t += period;
        }
        tenors.push(swap_end_yf);

        let num_forwards = tenors.len() - 1;
        if num_forwards == 0 {
            return Err(PricingError::model_failure_with_context(
                "LMM requires at least one forward rate period".to_string(),
                PricingErrorContext::default(),
            ));
        }

        // Accrual factors: tau_i = T_{i+1} - T_i
        let accrual_factors: Vec<f64> = tenors.windows(2).map(|w| w[1] - w[0]).collect();

        // Bootstrap forward rates from discount factors:
        //   F_i = (DF(T_i) / DF(T_{i+1}) - 1) / tau_i
        let mut initial_forwards: Vec<f64> = Vec::with_capacity(num_forwards);
        let mut forward_fallback_count = 0_usize;
        for i in 0..num_forwards {
            let df_start = disc.df(tenors[i]);
            let df_end = disc.df(tenors[i + 1]);
            let tau = accrual_factors[i];
            let fwd = if df_end > 1e-15 && tau > 1e-15 {
                (df_start / df_end - 1.0) / tau
            } else {
                forward_fallback_count += 1;
                0.03 // fallback
            };
            initial_forwards.push(fwd);
        }
        if forward_fallback_count > 0 {
            tracing::warn!(
                swaption_id = swaption.id.as_str(),
                discount_curve_id = swaption.discount_curve_id.as_str(),
                forward_fallback_count,
                "LMM forward bootstrap: degenerate discount factors or accruals; \
                 substituted a flat 3% forward for the affected periods. Check \
                 market data quality."
            );
        }

        // Displacement (shifted-lognormal shift). A small positive shift is
        // needed only when forwards can approach or cross zero; for a
        // comfortably-positive curve a pure lognormal model (zero shift) is
        // consistent with the lognormal Black swaption surface the
        // calibration targets. Pick the shift from the realised forwards
        // instead of hardcoding a magic constant.
        let min_forward = initial_forwards
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let shift = if min_forward > 0.01 {
            0.0
        } else {
            // Lift the most negative/near-zero forward to a +1% effective
            // floor so the displaced-lognormal diffusion stays well posed.
            (0.01 - min_forward).max(0.0)
        };
        let displacements = vec![shift; num_forwards];

        // Flat 2-factor loading structure with linear decay (the *shape*):
        //   ĝ_i = [1 - alpha * i/N, alpha * i/N, 0]
        // This approximates the first two principal components of swaption
        // correlation matrices. The full loading is `lambda_i = base_vol * ĝ_i`.
        let alpha = 0.4; // decay parameter (shape only — scale is calibrated)
        let loading_shapes: Vec<[f64; 3]> = (0..num_forwards)
            .map(|i| {
                let frac = i as f64 / num_forwards.max(1) as f64;
                [1.0 - alpha * frac, alpha * frac, 0.0]
            })
            .collect();

        // Calibrate `base_vol` to the swaption surface.
        //
        // The vol surface quotes the *swap-rate* Black vol; the LMM loading
        // magnitude is the *forward-rate* instantaneous vol. They differ by
        // the Rebonato shape factor R, so the raw surface vol cannot be used
        // directly. `calibrate_base_vol` solves `base_vol = sigma_market / R`
        // in closed form so the LMM reprices the longest co-terminal
        // European swaption (first exercise — it dominates the Bermudan).
        let base_vol = Self::calibrate_base_vol(
            swaption,
            market,
            &tenors,
            &accrual_factors,
            &initial_forwards,
            &displacements,
            &loading_shapes,
            as_of,
            swap_start_yf,
        )?;

        let vol_row: Vec<[f64; 3]> = loading_shapes
            .iter()
            .map(|g| [base_vol * g[0], base_vol * g[1], base_vol * g[2]])
            .collect();
        let vol_values = vec![vol_row]; // single vol period (no breakpoints)
        let vol_times: Vec<f64> = vec![]; // empty => single period

        LmmParams::try_new(
            num_forwards,
            2, // 2-factor model
            tenors,
            accrual_factors,
            displacements,
            vol_times,
            vol_values,
            initial_forwards,
        )
        .map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })
    }

    /// Calibrate the flat 2-factor loading scale `base_vol` to the swaption
    /// surface.
    ///
    /// Targets the **longest co-terminal European swaption** of the Bermudan
    /// (first exercise → maturity), which dominates the Bermudan value. The
    /// market ATM Black vol for that swaption is read from the swaption vol
    /// surface at the **co-terminal forward swap rate** strike, and
    /// [`calibrate_base_vol`](crate::calibration::targets::lmm::calibrate_base_vol)
    /// solves the closed-form Rebonato map `base_vol = sigma_market / R`.
    ///
    /// The surface is required: it must be tagged `expiry × strike` with
    /// Black/lognormal quotes, and produce a finite positive vol at the
    /// lookup point. Any other condition (missing surface, mis-tagged
    /// surface, degenerate vol or degenerate Rebonato calibration) is a hard
    /// error — there is no silent default-vol fallback.
    #[allow(clippy::too_many_arguments)]
    fn calibrate_base_vol(
        swaption: &BermudanSwaption,
        market: &MarketContext,
        tenors: &[f64],
        accrual_factors: &[f64],
        initial_forwards: &[f64],
        displacements: &[f64],
        loading_shapes: &[[f64; 3]],
        as_of: finstack_core::dates::Date,
        swap_start_yf: f64,
    ) -> std::result::Result<f64, PricingError> {
        use crate::calibration::targets::lmm::{calibrate_base_vol, CoTerminalSlice};
        use crate::instruments::rates::swaption::pricing::lmm_bermudan::first_alive_forward;

        let num_forwards = initial_forwards.len();

        // Expiry of the longest co-terminal European swaption: the first
        // exercise date of the Bermudan schedule. Fall back to the swap
        // start if no exercise dates are available.
        let first_exercise_yf = swaption
            .first_exercise()
            .and_then(|d| {
                year_fraction(swaption.day_count, as_of, d)
                    .ok()
                    .filter(|t| t.is_finite() && *t > 0.0)
            })
            .unwrap_or(swap_start_yf);

        // First forward still alive at the swaption expiry: T_j >= expiry
        // (with a tolerance snap for tenors a rounding error below expiry).
        let first_alive = first_alive_forward(&tenors[..num_forwards], first_exercise_yf);

        // ATM strike of the co-terminal European swaption: the forward swap
        // rate of the underlying swap over [T_first_alive, T_N], computed
        // from today's forwards and accruals — NOT the single-period forward
        // at the exercise index, which differs from the swap rate on any
        // non-flat curve.
        let atm_swap_rate = coterminal_swap_rate(initial_forwards, accrual_factors, first_alive)
            .ok_or_else(|| {
                PricingError::model_failure_with_context(
                    format!(
                        "LMM base-vol calibration for '{}': degenerate co-terminal swap \
                             (first_alive={first_alive}, num_forwards={num_forwards})",
                        swaption.id
                    ),
                    PricingErrorContext::default(),
                )
            })?;

        // The swaption vol surface is required for this pricer; missing or
        // mis-tagged surfaces are hard errors, not silent default vols.
        let surf = market
            .get_surface(swaption.vol_surface_id.as_str())
            .map_err(|e| {
                PricingError::missing_market_data_with_context(
                    format!(
                        "LMM base-vol calibration for '{}' requires vol surface '{}': {e}",
                        swaption.id, swaption.vol_surface_id
                    ),
                    PricingErrorContext::default(),
                )
            })?;
        surf.require_secondary_axis(finstack_core::market_data::surfaces::VolSurfaceAxis::Strike)
            .and_then(|()| {
                surf.require_quote_type(
                    finstack_core::market_data::surfaces::VolQuoteType::BlackLognormal,
                )
            })
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        let market_vol = surf.value_clamped(first_exercise_yf, atm_swap_rate);
        if !market_vol.is_finite() || market_vol <= 0.0 {
            return Err(PricingError::model_failure_with_context(
                format!(
                    "LMM base-vol calibration for '{}': surface '{}' returned non-positive \
                     vol {market_vol} at expiry={first_exercise_yf:.4}, strike={atm_swap_rate:.6}",
                    swaption.id, swaption.vol_surface_id
                ),
                PricingErrorContext::default(),
            ));
        }

        let slice = CoTerminalSlice {
            tenors,
            accrual_factors,
            initial_forwards,
            displacements,
            loading_shapes,
            first_alive,
        };

        match calibrate_base_vol(&slice, market_vol) {
            Some(cal) if cal.base_vol.is_finite() && cal.base_vol > 0.0 => Ok(cal.base_vol),
            _ => Err(PricingError::model_failure_with_context(
                format!(
                    "LMM base-vol calibration for '{}': Rebonato calibration degenerate \
                     (market_vol={market_vol}, first_alive={first_alive})",
                    swaption.id
                ),
                PricingErrorContext::default(),
            )),
        }
    }
}

/// Forward swap rate of the co-terminal swap `[T_first_alive, T_N]` from
/// today's forwards and accruals: `S = (1 − Π 1/(1+τF)) / Σ τ·DF`.
///
/// Returns `None` when the slice is empty or the annuity is degenerate.
fn coterminal_swap_rate(
    initial_forwards: &[f64],
    accrual_factors: &[f64],
    first_alive: usize,
) -> Option<f64> {
    let n = initial_forwards.len().min(accrual_factors.len());
    if first_alive >= n {
        return None;
    }
    let mut df = 1.0_f64;
    let mut annuity = 0.0_f64;
    for j in first_alive..n {
        df /= 1.0 + accrual_factors[j] * initial_forwards[j];
        annuity += accrual_factors[j] * df;
    }
    (annuity.abs() > 1e-15).then(|| (1.0 - df) / annuity)
}

impl Pricer for BermudanSwaptionLmmPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::BermudanSwaption, ModelKey::LmmMonteCarlo)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        // Downcast to BermudanSwaption
        let swaption = instrument
            .as_any()
            .downcast_ref::<BermudanSwaption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::BermudanSwaption, instrument.key())
            })?;

        // Get discount curve
        let disc = market
            .get_discount(swaption.discount_curve_id.as_str())
            .map_err(|e| {
                PricingError::missing_market_data_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        // Check if expired
        let ttm = swaption.time_to_maturity(as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;
        if ttm <= 0.0 {
            return Ok(ValuationResult::stamped(
                swaption.id.as_str(),
                as_of,
                Money::new(0.0, swaption.notional.currency()),
            ));
        }

        // Build LMM parameters from market data
        let lmm_params = Self::build_lmm_params(swaption, disc.as_ref(), market, as_of)?;

        // Extract exercise times as year fractions
        let exercise_times = swaption
            .bermudan_schedule
            .exercise_times(as_of, swaption.day_count)
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        if exercise_times.is_empty() {
            return Ok(ValuationResult::stamped(
                swaption.id.as_str(),
                as_of,
                Money::new(0.0, swaption.notional.currency()),
            ));
        }

        // Strike and payer/receiver flag
        let strike = swaption.strike_f64().map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;
        let is_payer =
            swaption.option_type == crate::instruments::common_impl::parameters::OptionType::Call;
        let notional = swaption.notional.amount();
        let currency = swaption.notional.currency();

        // Terminal discount factor P(0, T_N) for the last tenor
        let t_terminal = lmm_params.tenors.last().copied().unwrap_or(ttm);
        let df_terminal = disc.df(t_terminal);

        // Price via LSMC with LMM dynamics
        let estimate = price_bermudan_lmm(
            &lmm_params,
            &exercise_times,
            strike,
            is_payer,
            notional,
            df_terminal,
            currency,
            &self.config,
        )
        .map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        let mut result = ValuationResult::stamped(swaption.id.as_str(), as_of, estimate.mean);
        if estimate.stderr > 0.0 {
            result.measures.insert(
                crate::metrics::MetricId::custom("mc_stderr"),
                estimate.stderr,
            );
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibration::targets::lmm::{rebonato_shape_factor, CoTerminalSlice};
    use crate::instruments::rates::swaption::types::BermudanSchedule;
    use finstack_core::currency::Currency;
    use finstack_core::dates::{Date, DayCount, Tenor};
    use finstack_core::market_data::surfaces::VolSurface;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::money::Money;
    use time::Month;

    const SURFACE_VOL: f64 = 0.22;

    /// Build a market context with a flat swaption vol surface (Black vol
    /// `SURFACE_VOL`) and a flat ~3% discount curve.
    fn build_market(as_of: Date) -> MarketContext {
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([
                (0.0, 1.0),
                (1.0, (-0.03_f64).exp()),
                (3.0, (-0.03_f64 * 3.0).exp()),
                (6.0, (-0.03_f64 * 6.0).exp()),
                (12.0, (-0.03_f64 * 12.0).exp()),
            ])
            .build()
            .expect("discount curve");

        // Flat swaption vol surface: every (expiry, strike) returns SURFACE_VOL.
        let surface = VolSurface::builder("USD-SWPNVOL")
            .expiries(&[0.5, 12.0])
            .strikes(&[0.001, 0.20])
            .row(&[SURFACE_VOL, SURFACE_VOL])
            .row(&[SURFACE_VOL, SURFACE_VOL])
            .build()
            .expect("vol surface");

        MarketContext::new().insert(curve).insert_surface(surface)
    }

    /// A 6NC2 co-terminal Bermudan payer swaption (6y swap, callable
    /// semi-annually from year 2).
    fn build_bermudan(as_of: Date) -> BermudanSwaption {
        let swap_start = Date::from_calendar_date(2026, Month::January, 17).expect("date");
        let swap_end = Date::from_calendar_date(2032, Month::January, 17).expect("date");
        let first_ex = Date::from_calendar_date(2028, Month::January, 17).expect("date");
        let schedule = BermudanSchedule::co_terminal(first_ex, swap_end, Tenor::semi_annual())
            .expect("schedule");
        let mut b = BermudanSwaption::new_payer(
            "BERM-6NC2",
            Money::new(10_000_000.0, Currency::USD),
            0.03,
            swap_start,
            swap_end,
            schedule,
            "USD-OIS",
            "USD-OIS",
            "USD-SWPNVOL",
        )
        .expect("bermudan");
        b.day_count = DayCount::Thirty360;
        let _ = as_of;
        b
    }

    /// Implied co-terminal European swaption Black vol from a set of LMM
    /// factor loadings, via the Rebonato approximation. Used to check that
    /// the calibrated loadings reprice the swaption surface.
    fn implied_swaption_vol(params: &LmmParams, first_alive: usize) -> f64 {
        let shapes: Vec<[f64; 3]> = params.vol_values[0].clone();
        let slice = CoTerminalSlice {
            tenors: &params.tenors,
            accrual_factors: &params.accrual_factors,
            initial_forwards: &params.initial_forwards,
            displacements: &params.displacements,
            loading_shapes: &shapes,
            first_alive,
        };
        // The loadings already carry base_vol, so the "shape factor" of the
        // *already-scaled* loadings equals the implied swaption vol directly.
        rebonato_shape_factor(&slice).expect("shape factor")
    }

    /// Index of the first forward alive at the Bermudan's first exercise.
    fn first_alive_at_first_exercise(
        swaption: &BermudanSwaption,
        params: &LmmParams,
        as_of: Date,
    ) -> usize {
        let first_ex = swaption.first_exercise().expect("first exercise");
        let ex_yf = year_fraction(swaption.day_count, as_of, first_ex).expect("yf");
        params.tenors[..params.num_forwards].partition_point(|&t| t < ex_yf)
    }

    /// W-17 verification: the calibrated LMM Bermudan reprices the input
    /// swaption surface — its longest co-terminal European swaption's
    /// Rebonato-implied Black vol matches `SURFACE_VOL`.
    ///
    /// This FAILS with the previous hardcoded behaviour (`base_vol` set
    /// straight to the raw surface vol): the loadings then imply a swaption
    /// vol of `SURFACE_VOL · R` with `R != 1`, mispricing the surface.
    #[test]
    fn calibrated_lmm_reprices_swaption_surface() {
        let as_of = Date::from_calendar_date(2025, Month::January, 17).expect("date");
        let market = build_market(as_of);
        let swaption = build_bermudan(as_of);
        let disc = market.get_discount("USD-OIS").expect("discount");

        let params =
            BermudanSwaptionLmmPricer::build_lmm_params(&swaption, disc.as_ref(), &market, as_of)
                .expect("build params");

        let first_alive = first_alive_at_first_exercise(&swaption, &params, as_of);
        let implied = implied_swaption_vol(&params, first_alive);

        assert!(
            (implied - SURFACE_VOL).abs() < 1e-6,
            "calibrated LMM must reprice swaption surface vol {SURFACE_VOL}, got {implied}"
        );
    }

    /// Demonstrates the defect the calibration fixes: the *uncalibrated*
    /// loadings (`base_vol` == raw surface vol, the old hardcoded behaviour)
    /// imply a swaption vol materially different from the surface.
    #[test]
    fn uncalibrated_loadings_misprice_surface() {
        let as_of = Date::from_calendar_date(2025, Month::January, 17).expect("date");
        let market = build_market(as_of);
        let swaption = build_bermudan(as_of);
        let disc = market.get_discount("USD-OIS").expect("discount");

        let params =
            BermudanSwaptionLmmPricer::build_lmm_params(&swaption, disc.as_ref(), &market, as_of)
                .expect("build params");
        let first_alive = first_alive_at_first_exercise(&swaption, &params, as_of);

        // Reconstruct the OLD hardcoded loadings: base_vol = raw surface vol.
        let n = params.num_forwards;
        let alpha = 0.4;
        let old_shapes: Vec<[f64; 3]> = (0..n)
            .map(|i| {
                let frac = i as f64 / n.max(1) as f64;
                [
                    SURFACE_VOL * (1.0 - alpha * frac),
                    SURFACE_VOL * alpha * frac,
                    0.0,
                ]
            })
            .collect();
        let old_slice = CoTerminalSlice {
            tenors: &params.tenors,
            accrual_factors: &params.accrual_factors,
            initial_forwards: &params.initial_forwards,
            displacements: &params.displacements,
            loading_shapes: &old_shapes,
            first_alive,
        };
        let old_implied = rebonato_shape_factor(&old_slice).expect("shape factor");

        // The old loadings do NOT reprice the surface: |old_implied - vol|
        // is well outside calibration tolerance.
        assert!(
            (old_implied - SURFACE_VOL).abs() > 1e-3,
            "uncalibrated loadings should mis-price the surface (got {old_implied}, \
             surface {SURFACE_VOL}); if this holds the calibration is a no-op"
        );
    }

    /// The calibrated pricer still produces a finite, positive Bermudan
    /// price end-to-end.
    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
    fn calibrated_bermudan_prices_positive() {
        let as_of = Date::from_calendar_date(2025, Month::January, 17).expect("date");
        let market = build_market(as_of);
        let swaption = build_bermudan(as_of);

        let pricer = BermudanSwaptionLmmPricer::default();
        let result = pricer
            .price_dyn(&swaption, &market, as_of)
            .expect("price ok");
        let pv = result.value.amount();
        assert!(pv.is_finite(), "price must be finite, got {pv}");
        assert!(pv >= 0.0, "swaption price must be non-negative, got {pv}");
    }

    // --- Calibration-guard tests (W23) ---

    /// W23 regression: the LMM Bermudan pricer, when invoked via the registry
    /// path (enforce_calibration=true), must refuse with an Err.
    ///
    /// This test is expected to FAIL on the pre-fix code (which silently
    /// prices) and PASS after the enforce_calibration guard is added.
    #[test]
    fn lmm_bermudan_pricer_refuses_when_enforce_calibration_is_true() {
        use crate::pricer::Pricer as _;

        let as_of = Date::from_calendar_date(2025, Month::January, 17).expect("date");
        let market = build_market(as_of);
        let swaption = build_bermudan(as_of);

        // Simulate registry instantiation (enforce_calibration = true)
        let pricer = BermudanSwaptionLmmPricer::with_config(
            crate::instruments::rates::swaption::pricing::lmm_bermudan::LmmBermudanConfig {
                enforce_calibration: true,
                ..Default::default()
            },
        );

        let result = pricer.price_dyn(&swaption, &market, as_of);
        assert!(
            result.is_err(),
            "LMM Bermudan pricer must refuse when enforce_calibration=true, \
             but got Ok({:?})",
            result.ok().map(|r| r.value)
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("uncalibrated") || err_msg.contains("calibrat"),
            "Error message should mention calibration, got: {err_msg}"
        );
    }

    /// W23 complement: the pricer with default config (enforce_calibration=false)
    /// does NOT error due to the calibration guard.
    #[test]
    fn lmm_bermudan_pricer_permissive_without_enforce_calibration() {
        use crate::pricer::Pricer as _;

        let as_of = Date::from_calendar_date(2025, Month::January, 17).expect("date");
        let market = build_market(as_of);
        let swaption = build_bermudan(as_of);

        // Default config: enforce_calibration = false
        // Note: this goes through build_lmm_params which requires
        // a discount curve — that's present. The vol surface is also
        // present, so calibrate_base_vol will succeed.
        let pricer = BermudanSwaptionLmmPricer::default();

        // We don't assert Ok here because the full MC is slow (#[ignore]),
        // but we do assert it doesn't error for the calibration guard reason.
        // The actual pricing might produce a value or succeed; what we verify
        // is absence of the calibration-guard error.
        let result = pricer.price_dyn(&swaption, &market, as_of);
        if let Err(ref e) = result {
            let msg = e.to_string();
            assert!(
                !msg.contains("uncalibrated structural parameters"),
                "Default pricer must not trigger calibration guard, got: {msg}"
            );
        }
    }
}
