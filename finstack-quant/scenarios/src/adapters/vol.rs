//! Volatility surface shock adapter.
//!
//! Surface shocks are **sticky-strike** multiplicative moves on the stored
//! `(expiry, secondary_value)` grid. They do not re-mark along a sticky-delta
//! convention.
//!
//! # Arbitrage Detection
//!
//! The [`check_arbitrage`] function screens vol surface grids:
//! - **Calendar spread**: for `Strike`-axis `BlackLognormal` surfaces, total
//!   variance is non-decreasing in expiry at **fixed strike**. This is a
//!   heuristic screen, not a no-arbitrage certificate (the exact condition is
//!   fixed moneyness).
//! - **Non-negative vol**: all volatilities must be finite and non-negative (`>= 0.0`).

use crate::engine::ExecutionContext;
use crate::engine::ScenarioEffect;
use crate::error::{Error, Result};
use crate::warning::Warning;
use finstack_quant_core::dates::{BusinessDayConvention, Tenor, calendar_by_id_strict};
use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_quant_core::market_data::surfaces::{VolQuoteType, VolSurface, VolSurfaceAxis};
use finstack_quant_core::types::CurveId;

/// Threshold for warning about large negative vol shocks that may cause arbitrage.
/// A -50% shock could push post-shock vols down near zero or create calendar inversion.
const LARGE_NEGATIVE_VOL_SHOCK_PCT: f64 = -50.0;

/// Arbitrage violation types detected in volatility surfaces.
#[derive(Debug, Clone)]
enum ArbitrageViolation {
    /// Calendar spread arbitrage: total variance decreases with expiry at given strike
    CalendarSpread {
        /// Strike level where violation was detected
        strike: f64,
        /// Expiry time in years where violation was detected
        expiry: f64,
        /// Total variance at previous expiry
        prev_variance: f64,
        /// Total variance at current expiry (lower than prev, indicating arbitrage)
        curr_variance: f64,
    },
    /// Negative or non-finite volatility detected
    NegativeVol {
        /// Expiry time in years where violation was detected
        expiry: f64,
        /// Secondary-axis coordinate where violation was detected
        secondary: f64,
        /// The negative or non-finite volatility value
        vol: f64,
    },
}

impl std::fmt::Display for ArbitrageViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArbitrageViolation::CalendarSpread {
                strike,
                expiry,
                prev_variance,
                curr_variance,
            } => write!(
                f,
                "Calendar spread arbitrage at strike={strike:.2}, expiry={expiry:.4}Y: \
                 total variance decreased from {prev_variance:.6} to {curr_variance:.6}"
            ),
            ArbitrageViolation::NegativeVol {
                expiry,
                secondary,
                vol,
            } => write!(
                f,
                "Negative or non-finite vol at expiry={expiry:.4}Y, secondary={secondary:.2}: vol={vol:.6}"
            ),
        }
    }
}

/// Check a vol surface grid for negative/non-finite vols and, when applicable,
/// fixed-strike calendar-spread arbitrage violations.
///
/// # Fixed-strike limitation
///
/// The calendar-spread test requires total variance `σ²(K, T)·T` to be
/// non-decreasing in expiry **per fixed strike** `K`. The theoretically exact
/// condition holds at fixed *moneyness* `K/F(T)` (see Gatheral, *The
/// Volatility Surface*, 2006, §4): on surfaces with pronounced forward drift
/// the fixed-strike check can flag a spurious violation, or miss a genuine
/// one at constant moneyness. Treat calendar-spread results as a heuristic
/// screen, not a proof of arbitrage.
fn check_arbitrage(
    expiries: &[f64],
    secondary_values: &[f64],
    vols: &[Vec<f64>],
    check_calendar_spread: bool,
) -> Vec<ArbitrageViolation> {
    let mut violations = Vec::new();

    if check_calendar_spread {
        for win in expiries.windows(2) {
            let (prev, next) = (win[0], win[1]);
            if !(prev.is_finite() && next.is_finite() && next > prev) {
                violations.push(ArbitrageViolation::CalendarSpread {
                    strike: f64::NAN,
                    expiry: next,
                    prev_variance: f64::NAN,
                    curr_variance: f64::NAN,
                });
                return violations;
            }
        }
    }

    for (sec_idx, &secondary) in secondary_values.iter().enumerate() {
        let mut prev_var = 0.0;

        for (exp_idx, &expiry) in expiries.iter().enumerate() {
            if exp_idx >= vols.len() || sec_idx >= vols[exp_idx].len() {
                continue;
            }

            let vol = vols[exp_idx][sec_idx];

            if !vol.is_finite() || vol < 0.0 {
                violations.push(ArbitrageViolation::NegativeVol {
                    expiry,
                    secondary,
                    vol,
                });
                continue;
            }

            if check_calendar_spread {
                let total_var = vol * vol * expiry;
                if total_var < prev_var - 1e-8 {
                    violations.push(ArbitrageViolation::CalendarSpread {
                        strike: secondary,
                        expiry,
                        prev_variance: prev_var,
                        curr_variance: total_var,
                    });
                }
                prev_var = total_var;
            }
        }
    }

    violations
}

fn surface_grid(surface: &VolSurface) -> Vec<Vec<f64>> {
    let n_sec = surface.secondary_values().len();
    if n_sec == 0 {
        return Vec::new();
    }
    surface
        .vols()
        .chunks_exact(n_sec)
        .map(<[f64]>::to_vec)
        .collect()
}

/// Validate a post-shock surface preview: for `Strike` + `BlackLognormal`
/// surfaces, calendar-spread decreases become warnings; any negative or
/// non-finite vol is a hard error. Zero volatilities (`vol == 0.0`) are
/// valid stored surface quotes under `CORE-MARKET-14`.
fn arbitrage_warnings_for_surface(
    vol_surface_id: &CurveId,
    surface: &VolSurface,
) -> Result<Vec<Warning>> {
    let vols = surface_grid(surface);
    let check_calendar_spread = surface.secondary_axis() == VolSurfaceAxis::Strike
        && surface.quote_type() == VolQuoteType::BlackLognormal;
    let mut warnings = Vec::new();
    for violation in check_arbitrage(
        surface.expiries(),
        surface.secondary_values(),
        &vols,
        check_calendar_spread,
    ) {
        match violation {
            ArbitrageViolation::NegativeVol {
                expiry,
                secondary,
                vol,
            } => {
                return Err(Error::Validation(format!(
                    "Vol surface '{vol_surface_id}' shock would produce negative or non-finite vol \
                     ({vol:.6}) at expiry={expiry:.4}Y, secondary={secondary:.2}; volatility \
                     must remain non-negative. Reduce the shock magnitude."
                )));
            }
            calendar @ ArbitrageViolation::CalendarSpread { .. } => {
                warnings.push(Warning::VolSurfaceArbitrage {
                    vol_surface_id: vol_surface_id.as_str().to_string(),
                    detail: calendar.to_string(),
                });
            }
        }
    }
    Ok(warnings)
}

/// Generate effects for a vol-surface percent shock.
///
/// `tenors` and `strikes` restrict the shock to those grid buckets; `None`
/// for both shocks every point, which is the parallel shock. `bucket` records
/// which operation asked for it in [`Warning::VolSurfaceLargeNegativeShock`].
pub(crate) fn vol_effects(
    vol_surface_id: &CurveId,
    tenors: Option<&[String]>,
    strikes: Option<&[f64]>,
    pct: f64,
    bucket: bool,
    ctx: &ExecutionContext,
) -> Result<Vec<ScenarioEffect>> {
    if !pct.is_finite() {
        return Err(Error::Validation(format!(
            "Volatility shock percentage for '{}' must be finite (got {pct})",
            vol_surface_id.as_str()
        )));
    }
    let factor = 1.0 + pct / 100.0;
    if !factor.is_finite() || factor < 0.0 {
        return Err(Error::Validation(format!(
            "Volatility shock percentage {pct:.1}% for '{}' implies invalid scaling factor \
             {factor}; minimum permitted volatility shock is -100%",
            vol_surface_id.as_str()
        )));
    }

    let mut warnings = Vec::new();
    if ctx.market.get_vol_cube(vol_surface_id.as_str()).is_ok() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Cannot apply scenario volatility shock to '{}': registered as VolCube, which does not support VolSurface grid bumps",
            vol_surface_id.as_str()
        ))
        .into());
    }
    if ctx
        .market
        .get_fx_delta_vol_surface(vol_surface_id.as_str())
        .is_ok()
    {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Cannot apply scenario volatility shock to '{}': registered as FxDeltaVolSurface, which does not support VolSurface grid bumps",
            vol_surface_id.as_str()
        ))
        .into());
    }
    let surface = ctx.market.get_surface(vol_surface_id.as_str())?;

    if strikes.is_some() {
        surface.require_secondary_axis(VolSurfaceAxis::Strike)?;
    }

    if pct <= LARGE_NEGATIVE_VOL_SHOCK_PCT {
        warnings.push(Warning::VolSurfaceLargeNegativeShock {
            vol_surface_id: vol_surface_id.as_str().to_string(),
            pct,
            bucket,
        });
    }

    let exp_years = if let Some(t) = tenors {
        let clock = surface.expiry_clock().ok_or_else(|| {
            Error::Validation(format!(
                "Cannot apply tenor-bucketed volatility shock to '{}': surface has no \
                 expiry_clock to resolve tenors deterministically",
                vol_surface_id.as_str()
            ))
        })?;
        if clock.get_reference_date() != ctx.as_of {
            return Err(Error::Validation(format!(
                "Cannot apply tenor-bucketed volatility shock to '{}': surface expiry_clock \
                 reference date ({}) disagrees with scenario execution date ({})",
                vol_surface_id.as_str(),
                clock.get_reference_date(),
                ctx.as_of
            )));
        }
        let clock_cal = match clock.get_context().get_calendar_id() {
            Some(cal_id) => Some(calendar_by_id_strict(cal_id)?),
            None => ctx.calendar,
        };
        let mapped: Result<Vec<f64>> = t
            .iter()
            .map(|s| {
                let tenor = Tenor::parse(s).map_err(|e| Error::InvalidTenor(e.to_string()))?;
                let expiry_date = tenor
                    .add_to_date(ctx.as_of, clock_cal, BusinessDayConvention::Unadjusted)
                    .map_err(|e| Error::Internal(e.to_string()))?;
                clock.expiry_time(expiry_date).map_err(Error::from)
            })
            .collect();
        Some(mapped?)
    } else {
        None
    };

    let preview =
        surface.apply_bucket_bump(exp_years.as_deref(), strikes, BumpSpec::multiplier(factor))?;
    warnings.extend(arbitrage_warnings_for_surface(vol_surface_id, &preview)?);

    if exp_years.as_deref().is_some_and(<[f64]>::is_empty) || strikes.is_some_and(<[f64]>::is_empty)
    {
        warnings.push(Warning::VolSurfaceBucketEmptySelection {
            vol_surface_id: vol_surface_id.as_str().to_string(),
        });
    }

    let bump = MarketBump::VolBucketPct {
        vol_surface_id: vol_surface_id.clone(),
        expiries: exp_years,
        secondary_values: strikes.map(<[f64]>::to_vec),
        pct,
    };

    let mut effects = vec![ScenarioEffect::MarketBump(bump)];
    for w in warnings {
        effects.push(ScenarioEffect::Warning(w));
    }

    Ok(effects)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ExecutionContext;
    use finstack_quant_core::dates::{Date, DayCount, DayCountContextState, VolatilityClock};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::surfaces::{VolInterpolationMode, VolSurface};
    use time::macros::date;

    fn test_clock(as_of: Date) -> VolatilityClock {
        VolatilityClock::try_new(as_of, DayCount::Act365F, DayCountContextState::default())
            .expect("valid Act/365F clock")
    }

    fn clock_expiry(clock: &VolatilityClock, tenor: &str) -> f64 {
        let d = Tenor::parse(tenor)
            .expect("valid tenor")
            .add_to_date(
                clock.get_reference_date(),
                None,
                BusinessDayConvention::Unadjusted,
            )
            .expect("valid tenor date");
        clock.expiry_time(d).expect("valid expiry time")
    }

    fn strike_black_surface(
        id: &str,
        expiries: &[f64],
        strikes: &[f64],
        rows: &[&[f64]],
        clock: Option<VolatilityClock>,
    ) -> VolSurface {
        let mut builder = VolSurface::builder(id)
            .expiries(expiries)
            .secondary_values(strikes)
            .secondary_axis(VolSurfaceAxis::Strike)
            .quote_type(VolQuoteType::BlackLognormal)
            .interpolation_mode(VolInterpolationMode::Vol);
        if let Some(c) = clock {
            builder = builder.expiry_clock(c);
        }
        for row in rows {
            builder = builder.row(row);
        }
        builder.build().expect("valid strike Black VolSurface")
    }

    #[test]
    fn test_arbitrage_detection_calendar_spread() {
        let expiries = vec![0.25, 0.5, 1.0];
        let strikes = vec![100.0];
        let vols = vec![vec![0.3], vec![0.2], vec![0.15]];

        let violations = check_arbitrage(&expiries, &strikes, &vols, true);
        assert!(!violations.is_empty());
        assert!(matches!(
            &violations[0],
            ArbitrageViolation::CalendarSpread { .. }
        ));
    }

    #[test]
    fn test_arbitrage_detection_negative_and_zero_vol() {
        let expiries = vec![0.5];
        let strikes = vec![100.0, 110.0];
        let vols = vec![vec![0.0, -0.1]];

        let violations = check_arbitrage(&expiries, &strikes, &vols, true);
        assert_eq!(violations.len(), 1);
        assert!(matches!(
            &violations[0],
            ArbitrageViolation::NegativeVol { vol, .. } if *vol < 0.0
        ));
    }

    #[test]
    fn test_arbitrage_rejects_unsorted_expiries() {
        let expiries = vec![1.0, 0.5];
        let strikes = vec![100.0];
        let vols = vec![vec![0.20], vec![0.25]];

        let violations = check_arbitrage(&expiries, &strikes, &vols, true);
        assert_eq!(violations.len(), 1);
        assert!(matches!(
            &violations[0],
            ArbitrageViolation::CalendarSpread { prev_variance, curr_variance, .. }
                if prev_variance.is_nan() && curr_variance.is_nan()
        ));
    }

    #[test]
    fn test_arbitrage_rejects_duplicate_expiries() {
        let expiries = vec![0.5, 0.5];
        let strikes = vec![100.0];
        let vols = vec![vec![0.20], vec![0.25]];

        let violations = check_arbitrage(&expiries, &strikes, &vols, true);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_no_arbitrage_clean_surface() {
        let expiries = vec![0.25, 0.5, 1.0];
        let strikes = vec![90.0, 100.0, 110.0];
        let vols = vec![
            vec![0.25, 0.20, 0.22],
            vec![0.24, 0.19, 0.21],
            vec![0.22, 0.18, 0.20],
        ];

        let violations = check_arbitrage(&expiries, &strikes, &vols, true);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_vol_surface_parallel_pct_integration() -> crate::error::Result<()> {
        use crate::engine::ScenarioEngine;
        use crate::spec::{OperationSpec, ScenarioSpec};

        let surface = strike_black_surface("VOL", &[0.5, 1.0], &[100.0], &[&[0.20], &[0.22]], None);
        let mut market = MarketContext::new().insert_surface(surface);
        let mut model = finstack_quant_statements::types::FinancialModelSpec::new(
            "test",
            finstack_quant_core::dates::PeriodPlan::try_new(
                finstack_quant_core::dates::PeriodCalendar::Gregorian,
                vec![],
            )
            .expect("valid period plan"),
        );

        let scenario = ScenarioSpec {
            id: "vol_parallel".into(),
            name: None,
            description: None,
            operations: vec![OperationSpec::VolSurfaceParallelPct {
                vol_surface_id: "VOL".into(),
                pct: 10.0,
            }],
            priority: 0,
            resolution_mode: Default::default(),
        };

        let engine = ScenarioEngine::new();
        {
            let mut ctx = ExecutionContext {
                market: &mut market,
                model: Some(&mut model),
                instruments: None,
                rate_bindings: None,
                calendar: None,
                as_of: date!(2025 - 01 - 01),
            };
            let report = engine.apply(&scenario, &mut ctx)?;
            assert_eq!(report.operations_applied, 1);
        }

        let bumped = market.get_surface("VOL")?;
        let v_05 = finstack_quant_models::volatility::get_surface_vol(&bumped, 0.5, 100.0)?;
        let v_10 = finstack_quant_models::volatility::get_surface_vol(&bumped, 1.0, 100.0)?;
        assert!((v_05 - 0.22).abs() < 1e-10);
        assert!((v_10 - 0.242).abs() < 1e-10);
        Ok(())
    }

    /// Shock a 3x3 surface with one operation and return the stored vol grid.
    fn shocked_vols(operation: crate::spec::OperationSpec) -> crate::error::Result<Vec<f64>> {
        use crate::engine::ScenarioEngine;
        use crate::spec::ScenarioSpec;

        let surface = strike_black_surface(
            "VOL",
            &[0.25, 0.5, 1.0],
            &[90.0, 100.0, 110.0],
            &[
                &[0.27, 0.21, 0.23],
                &[0.26, 0.205, 0.225],
                &[0.25, 0.2, 0.22],
            ],
            None,
        );
        let mut market = MarketContext::new().insert_surface(surface);
        let mut ctx = ExecutionContext {
            market: &mut market,
            model: None,
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of: date!(2025 - 01 - 01),
        };
        let scenario = ScenarioSpec {
            id: "vol".into(),
            operations: vec![operation],
            ..Default::default()
        };
        ScenarioEngine::new().apply(&scenario, &mut ctx)?;
        Ok(market.get_surface("VOL")?.vols().to_vec())
    }

    /// A parallel surface shock is the bucket shock over every expiry and
    /// strike: the stored vols are bit-identical, not merely close.
    #[test]
    fn parallel_shock_equals_unfiltered_bucket_shock_bit_for_bit() -> crate::error::Result<()> {
        use crate::spec::OperationSpec;

        for pct in [-100.0, -50.0, -10.0, 0.0, 7.5, 40.0] {
            let parallel = shocked_vols(OperationSpec::VolSurfaceParallelPct {
                vol_surface_id: "VOL".into(),
                pct,
            })?;
            let bucket = shocked_vols(OperationSpec::VolSurfaceBucketPct {
                vol_surface_id: "VOL".into(),
                tenors: None,
                strikes: None,
                pct,
            })?;
            let parallel_bits: Vec<u64> = parallel.iter().map(|vol| vol.to_bits()).collect();
            let bucket_bits: Vec<u64> = bucket.iter().map(|vol| vol.to_bits()).collect();
            assert_eq!(parallel_bits, bucket_bits, "pct = {pct}");
        }
        Ok(())
    }

    /// Under SCENARIOS-009 / CORE-MARKET-14, -100% stress zeroes stored vols
    /// cleanly (with calendar-spread warning if longer expiry drops below earlier
    /// variance), while pct < -100% is rejected outright.
    #[test]
    fn parallel_and_bucket_minus_100_zero_vols_and_below_minus_100_rejected()
    -> crate::error::Result<()> {
        use crate::engine::ScenarioEngine;
        use crate::spec::{OperationSpec, ScenarioSpec};

        let as_of = date!(2025 - 01 - 01);
        let clock = test_clock(as_of);
        let t_3m = clock_expiry(&clock, "3M");
        let t_6m = clock_expiry(&clock, "6M");
        let surface = strike_black_surface(
            "VOL",
            &[t_3m, t_6m],
            &[100.0],
            &[&[0.20], &[0.22]],
            Some(clock),
        );

        let mut market_parallel = MarketContext::new().insert_surface(surface.clone());
        let mut ctx_parallel = ExecutionContext {
            market: &mut market_parallel,
            model: None,
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of,
        };
        let parallel_scenario = ScenarioSpec {
            id: "zero_parallel".into(),
            operations: vec![OperationSpec::VolSurfaceParallelPct {
                vol_surface_id: "VOL".into(),
                pct: -100.0,
            }],
            ..Default::default()
        };
        ScenarioEngine::new().apply(&parallel_scenario, &mut ctx_parallel)?;
        assert_eq!(market_parallel.get_surface("VOL")?.vols(), &[0.0, 0.0]);

        let vol_surface_id = CurveId::from("VOL");
        let mut market_bucket = MarketContext::new().insert_surface(surface);
        let ctx_bucket = ExecutionContext {
            market: &mut market_bucket,
            model: None,
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of,
        };
        let bucket_effects = vol_effects(
            &vol_surface_id,
            Some(&["6M".to_string()]),
            None,
            -100.0,
            true,
            &ctx_bucket,
        )?;
        assert!(bucket_effects.iter().any(|effect| matches!(
            effect,
            ScenarioEffect::Warning(Warning::VolSurfaceArbitrage { .. })
        )));

        let err = vol_effects(&vol_surface_id, None, None, -100.1, false, &ctx_bucket)
            .expect_err("pct < -100% must be rejected");
        assert!(err.to_string().contains("-100%"));
        Ok(())
    }

    #[test]
    fn tenor_shocks_require_matching_clock_and_exact_expiries() {
        let as_of = date!(2025 - 01 - 01);
        let id = CurveId::from("VOL");

        // No clock attached -> error
        let no_clock_surface =
            strike_black_surface("VOL", &[0.25, 0.5], &[100.0], &[&[0.20], &[0.22]], None);
        let mut market = MarketContext::new().insert_surface(no_clock_surface);
        let ctx = ExecutionContext {
            market: &mut market,
            model: None,
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of,
        };
        let err = vol_effects(&id, Some(&["6M".to_string()]), None, 10.0, true, &ctx)
            .expect_err("tenor bucket shock without expiry_clock must fail closed");
        assert!(err.to_string().contains("expiry_clock"));

        // Mismatched reference_date -> error
        let wrong_clock = test_clock(date!(2025 - 02 - 01));
        let wrong_ref_surface = strike_black_surface(
            "VOL",
            &[clock_expiry(&wrong_clock, "6M")],
            &[100.0],
            &[&[0.20]],
            Some(wrong_clock),
        );
        let mut market = MarketContext::new().insert_surface(wrong_ref_surface);
        let ctx = ExecutionContext {
            market: &mut market,
            model: None,
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of,
        };
        let err = vol_effects(&id, Some(&["6M".to_string()]), None, 10.0, true, &ctx)
            .expect_err("mismatched expiry_clock reference_date must fail closed");
        assert!(err.to_string().contains("disagrees"));

        // Matching clock but non-matching grid coordinate -> error (no snapping)
        let right_clock = test_clock(as_of);
        let non_matching = strike_black_surface(
            "VOL",
            &[0.5], // not clock("6M") = 181/365
            &[100.0],
            &[&[0.20]],
            Some(right_clock),
        );
        let mut market = MarketContext::new().insert_surface(non_matching);
        let ctx = ExecutionContext {
            market: &mut market,
            model: None,
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of,
        };
        vol_effects(&id, Some(&["6M".to_string()]), None, 10.0, true, &ctx)
            .expect_err("inexact tenor mapping must fail closed without snapping");
    }

    #[test]
    fn empty_bucket_selector_emits_warning_and_tenor_axis_controls_calendar_check()
    -> crate::error::Result<()> {
        let as_of = date!(2025 - 01 - 01);
        let id = CurveId::from("VOL");

        // Strike + Normal surface skips fixed-strike lognormal calendar-spread check,
        // and empty strikes selector emits VolSurfaceBucketEmptySelection.
        let normal_strike = VolSurface::builder("VOL")
            .expiries(&[0.25, 0.5])
            .secondary_values(&[100.0])
            .secondary_axis(VolSurfaceAxis::Strike)
            .quote_type(VolQuoteType::Normal)
            .interpolation_mode(VolInterpolationMode::Vol)
            .row(&[0.0080])
            .row(&[0.0040]) // decreasing variance in strike-normal space is NOT flagged as BS calendar arb
            .build()?;
        let mut market = MarketContext::new().insert_surface(normal_strike);
        let ctx = ExecutionContext {
            market: &mut market,
            model: None,
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of,
        };
        let effects = vol_effects(&id, None, Some(&[]), 5.0, true, &ctx)?;
        assert!(effects.iter().any(|e| matches!(
            e,
            ScenarioEffect::Warning(Warning::VolSurfaceBucketEmptySelection { .. })
        )));
        assert!(!effects.iter().any(|e| matches!(
            e,
            ScenarioEffect::Warning(Warning::VolSurfaceArbitrage { .. })
        )));

        // Tenor-axis surface rejects strike filters
        let tenor_surface = VolSurface::builder("VOL")
            .expiries(&[0.5, 1.0])
            .secondary_values(&[1.0, 2.0])
            .secondary_axis(VolSurfaceAxis::Tenor)
            .quote_type(VolQuoteType::BlackLognormal)
            .interpolation_mode(VolInterpolationMode::Vol)
            .row(&[0.25, 0.24])
            .row(&[0.20, 0.19])
            .build()?;
        let mut market = MarketContext::new().insert_surface(tenor_surface);
        let ctx = ExecutionContext {
            market: &mut market,
            model: None,
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of,
        };
        vol_effects(&id, None, Some(&[1.0]), 5.0, true, &ctx)
            .expect_err("strike filter on Tenor secondary_axis must fail");
        let effects = vol_effects(&id, None, None, -30.0, false, &ctx)?;
        assert!(!effects.iter().any(|e| matches!(
            e,
            ScenarioEffect::Warning(Warning::VolSurfaceArbitrage { .. })
        )));
        Ok(())
    }

    #[test]
    fn test_bucket_shock_warns_on_post_bump_arbitrage() -> crate::error::Result<()> {
        let as_of = date!(2025 - 01 - 01);
        let clock = test_clock(as_of);
        let t_3m = clock_expiry(&clock, "3M");
        let t_6m = clock_expiry(&clock, "6M");
        let surface = strike_black_surface(
            "VOL",
            &[t_3m, t_6m],
            &[100.0],
            &[&[0.30], &[0.22]],
            Some(clock),
        );
        let mut market = MarketContext::new().insert_surface(surface);
        let mut model = finstack_quant_statements::types::FinancialModelSpec::new(
            "test",
            finstack_quant_core::dates::PeriodPlan::try_new(
                finstack_quant_core::dates::PeriodCalendar::Gregorian,
                vec![],
            )
            .expect("valid period plan"),
        );
        let ctx = ExecutionContext {
            market: &mut market,
            model: Some(&mut model),
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of,
        };

        let vol_surface_id = CurveId::from("VOL");
        let effects = vol_effects(
            &vol_surface_id,
            Some(&["6M".to_string()]),
            None,
            -30.0,
            true,
            &ctx,
        )?;

        assert!(effects.iter().any(|effect| matches!(
            effect,
            ScenarioEffect::Warning(Warning::VolSurfaceArbitrage { .. })
        )));
        Ok(())
    }
}
