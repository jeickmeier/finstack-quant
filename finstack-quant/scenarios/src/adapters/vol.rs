//! Volatility surface shock adapter.
//!
//! Surface shocks are **sticky-strike** multiplicative moves on the stored
//! `(expiry, strike)` grid. They do not re-mark along a sticky-delta
//! convention.
//!
//! # Arbitrage Detection
//!
//! The [`check_arbitrage`] function screens vol surface grids:
//! - **Calendar spread**: total variance non-decreasing in expiry at
//!   **fixed strike**. This is a heuristic screen, not a no-arbitrage
//!   certificate (the exact condition is fixed moneyness).
//! - **Positive vol**: all volatilities must be positive

use crate::adapters::traits::ScenarioEffect;
use crate::engine::ExecutionContext;
use crate::error::{Error, Result};
use crate::warning::Warning;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount, Tenor};
use finstack_quant_core::market_data::bumps::{
    BumpMode, BumpSpec, BumpType, BumpUnits, Bumpable, MarketBump,
};
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::types::CurveId;

/// Threshold for warning about large negative vol shocks that may cause arbitrage.
/// A -50% shock could produce non-positive vols for low-vol points.
const LARGE_NEGATIVE_VOL_SHOCK_PCT: f64 = -50.0;

/// Arbitrage violation types detected in volatility surfaces.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ArbitrageViolation {
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
    /// Negative or zero volatility detected
    NonPositiveVol {
        /// Expiry time in years where violation was detected
        expiry: f64,
        /// Strike level where violation was detected
        strike: f64,
        /// The non-positive volatility value
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
            ArbitrageViolation::NonPositiveVol {
                expiry,
                strike,
                vol,
            } => write!(
                f,
                "Non-positive vol at expiry={expiry:.4}Y, strike={strike:.2}: vol={vol:.6}"
            ),
        }
    }
}

/// Check a vol surface grid for arbitrage violations.
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
pub fn check_arbitrage(
    expiries: &[f64],
    strikes: &[f64],
    vols: &[Vec<f64>],
) -> Vec<ArbitrageViolation> {
    let mut violations = Vec::new();

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

    for (strike_idx, &strike) in strikes.iter().enumerate() {
        let mut prev_var = 0.0;

        for (exp_idx, &expiry) in expiries.iter().enumerate() {
            if exp_idx >= vols.len() || strike_idx >= vols[exp_idx].len() {
                continue;
            }

            let vol = vols[exp_idx][strike_idx];

            if vol <= 0.0 {
                violations.push(ArbitrageViolation::NonPositiveVol {
                    expiry,
                    strike,
                    vol,
                });
                continue;
            }

            let total_var = vol * vol * expiry;
            if total_var < prev_var - 1e-8 {
                violations.push(ArbitrageViolation::CalendarSpread {
                    strike,
                    expiry,
                    prev_variance: prev_var,
                    curr_variance: total_var,
                });
            }
            prev_var = total_var;
        }
    }

    violations
}

fn surface_grid(surface: &VolSurface) -> Result<Vec<Vec<f64>>> {
    surface
        .expiries()
        .iter()
        .map(|&expiry| {
            surface
                .strikes()
                .iter()
                .map(|&strike| {
                    Ok(finstack_quant_models::volatility::get_surface_vol(
                        surface, expiry, strike,
                    )?)
                })
                .collect()
        })
        .collect()
}

/// Validate a post-shock surface preview: calendar-spread violations become
/// warnings, but any non-positive vol is a hard error. This mirrors the
/// VolIndex positivity guard — a zeroed or negative vol point produces
/// `d1 = ±inf`/NaN in every Black-Scholes consumer downstream, which is too
/// severe to surface as a warning-and-apply.
fn arbitrage_warnings_for_surface(
    vol_surface_id: &CurveId,
    surface: &VolSurface,
) -> Result<Vec<Warning>> {
    let vols = surface_grid(surface)?;
    let mut warnings = Vec::new();
    for violation in check_arbitrage(surface.expiries(), surface.strikes(), &vols) {
        match violation {
            ArbitrageViolation::NonPositiveVol {
                expiry,
                strike,
                vol,
            } => {
                return Err(crate::error::Error::Validation(format!(
                    "Vol surface '{vol_surface_id}' shock would produce non-positive vol \
                     ({vol:.6}) at expiry={expiry:.4}Y, strike={strike:.2}; volatility \
                     must stay positive. Reduce the shock magnitude."
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

/// Generate effects for a parallel vol-surface percent shock.
pub(crate) fn vol_parallel_effects(
    vol_surface_id: &CurveId,
    pct: f64,
    ctx: &ExecutionContext,
) -> Result<Vec<ScenarioEffect>> {
    let mut effects = Vec::new();
    let surface = ctx.market.get_surface(vol_surface_id.as_str())?;

    if pct <= LARGE_NEGATIVE_VOL_SHOCK_PCT {
        effects.push(ScenarioEffect::Warning(
            Warning::VolSurfaceLargeNegativeShock {
                vol_surface_id: vol_surface_id.as_str().to_string(),
                pct,
                bucket: false,
            },
        ));
    }

    let parallel_spec = BumpSpec {
        mode: BumpMode::Multiplicative,
        units: BumpUnits::Factor,
        value: 1.0 + (pct / 100.0),
        bump_type: BumpType::Parallel,
    };

    let preview = surface.as_ref().apply_bump(parallel_spec)?;
    for w in arbitrage_warnings_for_surface(vol_surface_id, &preview)? {
        effects.push(ScenarioEffect::Warning(w));
    }

    effects.push(ScenarioEffect::SurfaceBump {
        id: vol_surface_id.clone(),
        spec: parallel_spec,
    });

    Ok(effects)
}

/// Maximum slack when snapping a tenor-derived year fraction to a surface
/// grid expiry: the larger of ~7 calendar days or 2% of the grid value covers
/// day-count and holiday-adjustment drift without bridging distinct expiries.
const GRID_EXPIRY_SNAP_TOLERANCE_YEARS: f64 = 0.02;

/// Snap a tenor-derived year fraction to the nearest surface grid expiry.
///
/// Returns the exact grid value when one lies within
/// [`GRID_EXPIRY_SNAP_TOLERANCE_YEARS`] (absolute, or the same fraction
/// relative for long tenors); errors when no grid expiry is close enough so a
/// mistyped tenor cannot silently bump nothing.
fn snap_to_grid_expiry(
    years: f64,
    grid: &[f64],
    tenor: &str,
) -> std::result::Result<f64, finstack_quant_core::Error> {
    let nearest = grid
        .iter()
        .copied()
        .min_by(|a, b| (a - years).abs().total_cmp(&(b - years).abs()));
    if let Some(g) = nearest {
        let tol = GRID_EXPIRY_SNAP_TOLERANCE_YEARS.max(GRID_EXPIRY_SNAP_TOLERANCE_YEARS * g);
        if (g - years).abs() <= tol {
            return Ok(g);
        }
    }
    Err(finstack_quant_core::Error::Validation(format!(
        "vol bucket expiry tenor '{tenor}' (= {years:.6}y) matches no surface grid expiry within snap tolerance"
    )))
}

/// Generate effects for a bucketed vol-surface percent shock.
pub(crate) fn vol_bucket_effects(
    vol_surface_id: &CurveId,
    tenors: Option<&[String]>,
    strikes: Option<&[f64]>,
    pct: f64,
    ctx: &ExecutionContext,
) -> Result<Vec<ScenarioEffect>> {
    let mut warnings = Vec::new();
    let surface = ctx.market.get_surface(vol_surface_id.as_str())?;

    if pct <= LARGE_NEGATIVE_VOL_SHOCK_PCT {
        warnings.push(Warning::VolSurfaceLargeNegativeShock {
            vol_surface_id: vol_surface_id.as_str().to_string(),
            pct,
            bucket: true,
        });
    }

    let exp_years = if let Some(t) = tenors {
        let parsed: std::result::Result<Vec<f64>, _> = t
            .iter()
            .map(|s| {
                Tenor::parse(s)
                    .map_err(|e| Error::InvalidTenor(e.to_string()))?
                    .to_years_with_context(
                        ctx.as_of,
                        ctx.calendar,
                        BusinessDayConvention::Unadjusted,
                        DayCount::Act365F,
                    )
                    .map_err(|e| Error::Internal(e.to_string()))
            })
            .collect();
        // Tenor-derived year fractions carry day-count/calendar slack (e.g.
        // "6M" under Act/365F is ~0.4959, not 0.5), while the core bucket bump
        // matches grid expiries exactly. Snap each parsed value to the nearest
        // surface expiry so the emitted bump carries exact grid values; fail
        // loudly when no grid expiry is close enough rather than silently
        // bumping nothing.
        let snapped: std::result::Result<Vec<f64>, finstack_quant_core::Error> = parsed?
            .into_iter()
            .zip(t.iter())
            .map(|(years, tenor)| snap_to_grid_expiry(years, surface.expiries(), tenor))
            .collect();
        Some(snapped?)
    } else {
        None
    };

    let preview = surface
        .apply_bucket_bump(exp_years.as_deref(), strikes, pct)
        .ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::DimensionMismatch)
        })?;
    warnings.extend(arbitrage_warnings_for_surface(vol_surface_id, &preview)?);

    let bump = MarketBump::VolBucketPct {
        vol_surface_id: vol_surface_id.clone(),
        expiries: exp_years,
        strikes: strikes.map(<[f64]>::to_vec),
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
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use time::macros::date;

    #[test]
    fn test_arbitrage_detection_calendar_spread() {
        let expiries = vec![0.25, 0.5, 1.0];
        let strikes = vec![100.0];
        let vols = vec![vec![0.3], vec![0.2], vec![0.15]];

        let violations = check_arbitrage(&expiries, &strikes, &vols);
        assert!(!violations.is_empty());
        assert!(matches!(
            &violations[0],
            ArbitrageViolation::CalendarSpread { .. }
        ));
    }

    #[test]
    fn test_arbitrage_detection_non_positive() {
        let expiries = vec![0.5];
        let strikes = vec![100.0, 110.0];
        let vols = vec![vec![0.2, -0.1]];

        let violations = check_arbitrage(&expiries, &strikes, &vols);
        assert!(!violations.is_empty());
        assert!(matches!(
            &violations[0],
            ArbitrageViolation::NonPositiveVol { .. }
        ));
    }

    #[test]
    fn test_arbitrage_rejects_unsorted_expiries() {
        let expiries = vec![1.0, 0.5];
        let strikes = vec![100.0];
        let vols = vec![vec![0.20], vec![0.25]];

        let violations = check_arbitrage(&expiries, &strikes, &vols);
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

        let violations = check_arbitrage(&expiries, &strikes, &vols);
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

        let violations = check_arbitrage(&expiries, &strikes, &vols);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_vol_surface_parallel_pct_integration() -> crate::error::Result<()> {
        use crate::engine::ScenarioEngine;
        use crate::spec::{OperationSpec, ScenarioSpec};

        let surface = VolSurface::builder("VOL")
            .expiries(&[0.5, 1.0])
            .strikes(&[100.0])
            .row(&[0.20])
            .row(&[0.22])
            .build()?;
        let mut market = MarketContext::new().insert_surface(surface);
        let mut model = finstack_quant_statements::FinancialModelSpec::new("test", vec![]);

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
            hazard_bump_mode: Default::default(),
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

    /// A parallel shock that drives every vol to zero (or below) must be
    /// rejected outright, mirroring the VolIndex positivity guard: a zeroed
    /// surface produces `d1 = ±inf`/NaN in any Black-Scholes consumer, and a
    /// warning is too quiet for that failure mode.
    #[test]
    fn parallel_shock_to_non_positive_vol_is_rejected() -> crate::error::Result<()> {
        let surface = VolSurface::builder("VOL")
            .expiries(&[0.5, 1.0])
            .strikes(&[100.0])
            .row(&[0.20])
            .row(&[0.22])
            .build()?;
        let mut market = MarketContext::new().insert_surface(surface);
        let mut model = finstack_quant_statements::FinancialModelSpec::new("test", vec![]);
        let ctx = ExecutionContext {
            market: &mut market,
            model: Some(&mut model),
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of: date!(2025 - 01 - 01),
        };

        let vol_surface_id = CurveId::from("VOL");
        let err = vol_parallel_effects(&vol_surface_id, -100.0, &ctx)
            .expect_err("a -100% vol shock must be rejected, not warned");
        assert!(
            err.to_string().contains("non-positive"),
            "error should name the non-positive vol condition: {err}"
        );
        Ok(())
    }

    /// A bucket shock that pushes a low-vol point non-positive must also be
    /// rejected; other buckets staying positive does not make the surface
    /// usable.
    #[test]
    fn bucket_shock_to_non_positive_vol_is_rejected() -> crate::error::Result<()> {
        let surface = VolSurface::builder("VOL")
            .expiries(&[0.5, 1.0])
            .strikes(&[100.0])
            .row(&[0.20])
            .row(&[0.22])
            .build()?;
        let mut market = MarketContext::new().insert_surface(surface);
        let mut model = finstack_quant_statements::FinancialModelSpec::new("test", vec![]);
        let ctx = ExecutionContext {
            market: &mut market,
            model: Some(&mut model),
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of: date!(2025 - 01 - 01),
        };

        let vol_surface_id = CurveId::from("VOL");
        let err = vol_bucket_effects(
            &vol_surface_id,
            Some(&["6M".to_string()]),
            None,
            -100.0,
            &ctx,
        )
        .expect_err("a bucket shock zeroing a vol point must be rejected");
        assert!(
            err.to_string().contains("non-positive"),
            "error should name the non-positive vol condition: {err}"
        );
        Ok(())
    }

    #[test]
    fn test_bucket_shock_warns_on_post_bump_arbitrage() -> crate::error::Result<()> {
        let surface = VolSurface::builder("VOL")
            .expiries(&[0.25, 0.5])
            .strikes(&[100.0])
            .row(&[0.30])
            .row(&[0.22])
            .build()?;
        let mut market = MarketContext::new().insert_surface(surface);
        let mut model = finstack_quant_statements::FinancialModelSpec::new("test", vec![]);
        let ctx = ExecutionContext {
            market: &mut market,
            model: Some(&mut model),
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of: date!(2025 - 01 - 01),
        };

        let vol_surface_id = CurveId::from("VOL");
        let effects = vol_bucket_effects(
            &vol_surface_id,
            Some(&["6M".to_string()]),
            None,
            -30.0,
            &ctx,
        )?;

        assert!(effects.iter().any(|effect| matches!(
            effect,
            ScenarioEffect::Warning(Warning::VolSurfaceArbitrage { .. })
        )));
        Ok(())
    }
}
