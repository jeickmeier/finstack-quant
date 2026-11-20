//! Volatility surface shock adapter.
//!
//! Surface shocks are **sticky-strike** multiplicative moves on the stored
//! `(expiry, secondary_value)` grid. They do not re-mark along a sticky-delta
//! convention.
//!
//! Arbitrage diagnostics can be run separately on the shocked artifact.

use crate::engine::ExecutionContext;
use crate::engine::ScenarioEffect;
use crate::error::{Error, Result};
use crate::warning::Warning;
use finstack_quant_core::dates::{BusinessDayConvention, Tenor, calendar_by_id_strict};
use finstack_quant_core::market_data::bumps::BumpSpec;
use finstack_quant_core::market_data::surfaces::VolSurfaceAxis;
use finstack_quant_core::types::CurveId;

/// Generate effects for a vol-surface percent shock.
///
/// `tenors` and `strikes` restrict the shock to those grid buckets; `None`
/// for both shocks every point, which is the parallel shock.
pub(crate) fn vol_effects(
    vol_surface_id: &CurveId,
    tenors: Option<&[String]>,
    strikes: Option<&[f64]>,
    pct: f64,
    ctx: &ExecutionContext,
) -> Result<Vec<ScenarioEffect>> {
    if !pct.is_finite() {
        return Err(Error::Validation(format!(
            "Volatility shock percentage for '{}' must be finite (got {pct})",
            vol_surface_id.as_str()
        )));
    }
    let factor = (100.0 + pct) / 100.0;
    if !factor.is_finite() || factor < 0.0 {
        return Err(Error::Validation(format!(
            "Volatility shock percentage {pct:.1}% for '{}' implies invalid scaling factor \
             {factor}; minimum permitted volatility shock is -100%",
            vol_surface_id.as_str()
        )));
    }

    let surface = ctx.market.get_surface(vol_surface_id.as_str()).map_err(|err| {
        let source_kind = if ctx.market.get_vol_cube(vol_surface_id.as_str()).is_ok() {
            "VolCube"
        } else if ctx.market.get_fx_delta_vol_surface(vol_surface_id.as_str()).is_ok() {
            "FxDeltaVolSurface"
        } else {
            return err;
        };
        finstack_quant_core::Error::Validation(format!(
            "Cannot apply scenario volatility shock to '{vol_surface_id}': registered as {source_kind}, which does not support VolSurface grid bumps"
        ))
    })?;

    if strikes.is_some() {
        surface.require_secondary_axis(VolSurfaceAxis::Strike)?;
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

    let updated =
        surface.apply_bucket_bump(exp_years.as_deref(), strikes, BumpSpec::multiplier(factor))?;
    let empty = exp_years.as_deref().is_some_and(<[f64]>::is_empty)
        || strikes.is_some_and(<[f64]>::is_empty);
    let mut effects = vec![ScenarioEffect::UpdateVolSurface {
        surface: std::sync::Arc::new(updated),
        expiries_years: exp_years,
    }];

    if empty {
        effects.push(ScenarioEffect::Warning(
            Warning::VolSurfaceBucketEmptySelection {
                vol_surface_id: vol_surface_id.as_str().to_string(),
            },
        ));
    }

    Ok(effects)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ExecutionContext;
    use finstack_quant_core::dates::{Date, DayCount, DayCountContextState, VolatilityClock};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::surfaces::{
        VolInterpolationMode, VolQuoteType, VolSurface,
    };
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
    fn surface_shock_selects_exact_source_and_preserves_positive_boundary_factor()
    -> crate::error::Result<()> {
        use crate::engine::ScenarioEngine;
        use crate::spec::{OperationSpec, ScenarioSpec};
        use finstack_quant_core::market_data::surfaces::{SabrParameterData, VolCube};

        let as_of = date!(2025 - 01 - 01);
        let surface = strike_black_surface("VOL", &[0.5], &[100.0], &[&[0.20]], None);
        let sabr = SabrParameterData::new(0.2, 0.5, 0.3, -0.2, None)?;
        let cube = VolCube::from_grid("VOL", &[1.0], &[5.0], &[sabr], &[0.03])?;
        let mut market = MarketContext::new()
            .insert_surface(surface)
            .insert_vol_cube(cube);
        let mut ctx = ExecutionContext {
            market: &mut market,
            model: None,
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of,
        };
        let pct = (-100.0_f64).next_up();
        let scenario = ScenarioSpec {
            id: "stress_sigma".into(),
            operations: vec![OperationSpec::VolSurfaceParallelPct {
                vol_surface_id: "VOL".into(),
                pct,
            }],
            ..Default::default()
        };
        ScenarioEngine::new().apply(&scenario, &mut ctx)?;
        let expected = 0.20 * ((100.0 + pct) / 100.0);
        assert!(expected > 0.0);
        assert_eq!(market.get_surface("VOL")?.vols(), &[expected]);
        assert_eq!(market.get_vol_cube("VOL")?.forwards(), &[0.03]);
        Ok(())
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
    /// cleanly, while pct < -100% is rejected outright.
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
            &ctx_bucket,
        )?;
        assert!(matches!(
            bucket_effects.as_slice(),
            [ScenarioEffect::UpdateVolSurface { surface, .. }] if surface.vols() == [0.20, 0.0]
        ));

        let err = vol_effects(&vol_surface_id, None, None, -100.1, &ctx_bucket)
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
        let err = vol_effects(&id, Some(&["6M".to_string()]), None, 10.0, &ctx)
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
        let err = vol_effects(&id, Some(&["6M".to_string()]), None, 10.0, &ctx)
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
        vol_effects(&id, Some(&["6M".to_string()]), None, 10.0, &ctx)
            .expect_err("inexact tenor mapping must fail closed without snapping");
    }

    #[test]
    fn empty_bucket_selector_emits_warning_and_tenor_axis_rejects_strike_filters()
    -> crate::error::Result<()> {
        let as_of = date!(2025 - 01 - 01);
        let id = CurveId::from("VOL");

        // An explicit empty strike selector emits a no-effect diagnostic.
        let normal_strike = VolSurface::builder("VOL")
            .expiries(&[0.25, 0.5])
            .secondary_values(&[100.0])
            .secondary_axis(VolSurfaceAxis::Strike)
            .quote_type(VolQuoteType::Normal)
            .interpolation_mode(VolInterpolationMode::Vol)
            .row(&[0.0080])
            .row(&[0.0040])
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
        let effects = vol_effects(&id, None, Some(&[]), 5.0, &ctx)?;
        assert!(effects.iter().any(|e| matches!(
            e,
            ScenarioEffect::Warning(Warning::VolSurfaceBucketEmptySelection { .. })
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
        vol_effects(&id, None, Some(&[1.0]), 5.0, &ctx)
            .expect_err("strike filter on Tenor secondary_axis must fail");
        Ok(())
    }
}
