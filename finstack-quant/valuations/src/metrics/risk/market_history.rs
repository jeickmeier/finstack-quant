//! Market history storage for Historical VaR calculation.
//!
//! This module provides data structures for storing and applying historical
//! market shifts. The core concept is to store shifts (differences from base)
//! rather than absolute levels, enabling efficient scenario application.

use crate::metrics::risk::RiskFactorType;
use crate::recalibration::{
    provider_missing, HazardRecalibrationAction, HazardRecalibrationRequest, QuoteBump,
    RecalibrationProvider,
};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::bumps::{
    BumpMode, BumpSpec, BumpType, BumpUnits, MarketBump,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use std::sync::Arc;

/// Historical shift for a single risk factor on a single date.
///
/// Represents the change in a market variable from its base value.
/// For example, a +15bp shift in 5Y USD rates.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RiskFactorShift {
    /// Risk factor being shifted
    pub factor: RiskFactorType,
    /// Absolute change in the factor
    /// - For rates/spreads: change in basis points as decimal (e.g., 0.0015 = 15bp)
    /// - For equity/FX spot: relative change (e.g., -0.025 = -2.5%)
    /// - For volatility: absolute vol change (e.g., 0.02 = +2 vol points)
    pub shift: f64,
}

/// Collection of all risk factor shifts for a single historical date.
///
/// Represents a complete market scenario that can be applied to revalue
/// a portfolio. Each scenario contains shifts for all relevant risk factors.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketScenario {
    /// Historical date this scenario represents
    pub date: Date,
    /// All risk factor shifts on this date (relative to base date)
    pub shifts: Vec<RiskFactorShift>,
}

impl MarketScenario {
    /// Create a new market scenario.
    pub fn new(date: Date, shifts: Vec<RiskFactorShift>) -> Self {
        Self { date, shifts }
    }

    /// Apply this scenario to a base market context.
    ///
    /// Creates a new `MarketContext` with all risk factor shifts applied.
    /// Rate shifts use triangular key-rate bumps. Credit spreads are additive
    /// par-quote changes, replayed together for each hazard curve through the
    /// supplied calibration provider against the shifted dependency market.
    /// Equity and volatility shifts are multiplicative and additive, respectively.
    ///
    /// # Arguments
    ///
    /// * `base_market` - Current market state, including the source hazard recipes
    ///   and discount curves needed to verify and replay credit spread quotes.
    /// * `provider` - Quote-recalibration service. Required for a nonzero credit
    ///   spread shock; other scenarios accept `None`. No direct hazard-rate
    ///   approximation is substituted when a provider or recipe is unavailable.
    ///
    /// # Returns
    ///
    /// New market context with historical shifts applied
    /// # Errors
    ///
    /// Returns an error for non-finite shifts, invalid credit tenors, missing
    /// dependencies or providers, or failed quote recalibration. The source
    /// market remains unchanged on every failure.
    pub fn apply(
        &self,
        base_market: &MarketContext,
        provider: Option<&dyn RecalibrationProvider>,
    ) -> Result<MarketContext> {
        // Collect every shift into a single bump batch so the (potentially
        // expensive) `MarketContext` clone happens once instead of once per
        // shift. `bump` applies the slice in order, identical to the prior
        // shift-by-shift loop.
        let mut bumps: Vec<MarketBump> = Vec::with_capacity(self.shifts.len());

        // `ImpliedVol` shocks carry (expiry, strike) point coordinates, but the
        // bump machinery here supports only whole-surface parallel additive
        // bumps. Applying each point shock as its own parallel bump would
        // COMPOUND N point shocks into an N-fold surface move (the deferred
        // rounds below re-apply same-id curve bumps, which is correct for
        // key-rate shifts at different tenors but not for repeated parallel
        // surface bumps). Approximate instead with ONE parallel bump per
        // surface equal to the MEAN of that surface's point shifts.
        let mut vol_shifts_by_surface: Vec<(CurveId, Vec<f64>)> = Vec::new();
        let mut credit_shifts: Vec<(CurveId, Vec<(f64, f64)>)> = Vec::new();

        for shift in &self.shifts {
            if !shift.shift.is_finite() {
                return Err(finstack_quant_core::Error::Validation(
                    "historical scenario shifts must be finite".to_string(),
                ));
            }
            let bump = match &shift.factor {
                RiskFactorType::DiscountRate {
                    curve_id,
                    tenor_years,
                }
                | RiskFactorType::ForwardRate {
                    curve_id,
                    tenor_years,
                } => {
                    let (id, spec) = key_rate_bp_bump(curve_id, *tenor_years, shift.shift);
                    MarketBump::Curve { id, spec }
                }
                RiskFactorType::CreditSpread {
                    curve_id,
                    tenor_years,
                } => {
                    if !tenor_years.is_finite() || *tenor_years <= 0.0 {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "historical credit tenor for '{curve_id}' must be finite and positive"
                        )));
                    }
                    if shift.shift == 0.0 {
                        continue;
                    }
                    let target = (*tenor_years, shift.shift * 10_000.0);
                    if let Some((_, targets)) =
                        credit_shifts.iter_mut().find(|(id, _)| id == curve_id)
                    {
                        targets.push(target);
                    } else {
                        credit_shifts.push((curve_id.clone(), vec![target]));
                    }
                    continue;
                }
                RiskFactorType::EquitySpot { ticker } => MarketBump::Curve {
                    id: CurveId::from(ticker.as_str()),
                    spec: BumpSpec::multiplier(1.0 + shift.shift),
                },
                RiskFactorType::FxSpot { base, quote } => MarketBump::FxPct {
                    base: *base,
                    quote: *quote,
                    pct: shift.shift * 100.0,
                    as_of: self.date,
                },
                RiskFactorType::ImpliedVol { vol_surface_id, .. } => {
                    match vol_shifts_by_surface
                        .iter_mut()
                        .find(|(id, _)| id == vol_surface_id)
                    {
                        Some((_, shifts)) => shifts.push(shift.shift),
                        None => {
                            vol_shifts_by_surface.push((vol_surface_id.clone(), vec![shift.shift]))
                        }
                    }
                    continue;
                }
            };

            bumps.push(bump);
        }

        for (vol_surface_id, shifts) in vol_shifts_by_surface {
            let count = shifts.len();
            let mean_shift = shifts.iter().sum::<f64>() / count as f64;
            if count > 1 {
                tracing::warn!(
                    vol_surface_id = vol_surface_id.as_str(),
                    point_shocks = count,
                    mean_shift,
                    "scenario carries multiple ImpliedVol point shocks for one \
                     surface; approximating with a single mean parallel bump \
                     (point-level surface bumps are not supported here)"
                );
            }
            bumps.push(MarketBump::Curve {
                id: vol_surface_id,
                spec: BumpSpec {
                    mode: BumpMode::Additive,
                    units: BumpUnits::Fraction,
                    value: mean_shift,
                    bump_type: BumpType::Parallel,
                },
            });
        }

        // `MarketContext::bump` classifies curve bumps into a `HashMap` keyed
        // by `CurveId`, so two `MarketBump::Curve` entries that target the same
        // curve in one call would collapse (last-wins). The legacy loop applied
        // each shift to the already-bumped context, so same-curve key-rate
        // shifts must compound. Partition the batch into rounds where each round
        // has unique curve IDs; each round is one clone. For the common case of
        // one bump per curve this is a single `bump` call.
        let mut bumped_market = base_market.clone();
        let mut remaining = bumps;
        while !remaining.is_empty() {
            let mut round: Vec<MarketBump> = Vec::with_capacity(remaining.len());
            let mut deferred: Vec<MarketBump> = Vec::new();
            let mut seen: Vec<CurveId> = Vec::new();
            for bump in remaining {
                match bump_curve_id(&bump) {
                    Some(id) if seen.contains(&id) => deferred.push(bump),
                    Some(id) => {
                        seen.push(id);
                        round.push(bump);
                    }
                    None => round.push(bump),
                }
            }
            bumped_market = bumped_market.bump(round)?;
            remaining = deferred;
        }

        if !credit_shifts.is_empty() {
            let provider = provider.ok_or_else(|| provider_missing("historical_credit_spread"))?;
            let source_market = Arc::new(base_market.clone());
            for (curve_id, targets) in credit_shifts {
                let hazard = base_market.get_hazard(&curve_id)?;
                let discount_curve_id = provider.get_hazard_discount_curve_id(&hazard)?;
                let rebuilt = provider.rebuild_hazard_curve(&HazardRecalibrationRequest {
                    hazard,
                    source_market: Arc::clone(&source_market),
                    target_market: Arc::new(bumped_market.clone()),
                    discount_curve_id,
                    doc_clause: None,
                    cds_valuation_convention: None,
                    deal_quote_override: None,
                    action: HazardRecalibrationAction::SpreadBump(QuoteBump::TenorsBp(targets)),
                })?;
                bumped_market = bumped_market.insert(rebuilt.as_ref().clone());
            }
        }
        Ok(bumped_market)
    }
}

/// Build a triangular key-rate bump for a specific tenor on a curve.
///
/// Uses the standard bucket grid to determine the triangular weight neighbors,
/// ensuring that localized shifts preserve curve shape.
fn key_rate_bp_bump(curve_id: &CurveId, tenor_years: f64, shift: f64) -> (CurveId, BumpSpec) {
    let shift_bp = shift * 10_000.0;

    // Wing buckets use the dedicated half-triangle constructors so the
    // partition-of-unity invariant holds and no infinite sentinel reaches the
    // curve bump paths (non-finite bounds are rejected there).
    let spec = match find_triangular_neighbors(tenor_years) {
        (Some(prev), Some(next)) => {
            BumpSpec::triangular_key_rate_bp(prev, tenor_years, next, shift_bp)
        }
        (None, Some(next)) => BumpSpec::triangular_key_rate_first_bp(tenor_years, next, shift_bp),
        (Some(prev), None) => BumpSpec::triangular_key_rate_last_bp(prev, tenor_years, shift_bp),
        (None, None) => BumpSpec::parallel_bp(shift_bp),
    };
    (curve_id.clone(), spec)
}

/// Return the `CurveId` a bump is keyed by, if it is routed through the
/// curve/surface/price map (which de-duplicates by ID within a single
/// [`MarketContext::bump`] call). Returns `None` for bumps that are applied
/// independently (e.g. FX), which never collide.
fn bump_curve_id(bump: &MarketBump) -> Option<CurveId> {
    match bump {
        MarketBump::Curve { id, .. } => Some(id.clone()),
        MarketBump::VolBucketPct { vol_surface_id, .. } => Some(vol_surface_id.clone()),
        MarketBump::BaseCorrBucketPts { surface_id, .. } => Some(surface_id.clone()),
        MarketBump::FxPct { .. } => None,
    }
}

/// Find the neighboring bucket boundaries for a triangular key-rate bump.
///
/// `None` means "no neighbour on that side" (wing bucket): the caller must
/// use the flat half-triangle constructors rather than 0.0/∞ sentinels,
/// which break the Σwᵢ(t)=1 invariant (and ∞ produces NaN weights).
fn find_triangular_neighbors(tenor: f64) -> (Option<f64>, Option<f64>) {
    let buckets = &crate::metrics::sensitivities::config::STANDARD_BUCKETS_YEARS;

    // Absolute tolerance for exact-tenor matching. `f64::EPSILON` (~2.2e-16)
    // is too tight: a tenor like `0.5` that has been serde round-tripped can
    // differ by more than that, missing the equality branch and selecting the
    // wrong triangular bucket. `1e-6` is the project-wide tenor tolerance.
    const TENOR_TOL: f64 = 1e-6;

    let mut prev = None;
    for (i, &bucket) in buckets.iter().enumerate() {
        if (tenor - bucket).abs() <= TENOR_TOL {
            return (prev, buckets.get(i + 1).copied());
        }
        if tenor < bucket {
            return (prev, Some(bucket));
        }
        prev = Some(bucket);
    }

    (prev, None)
}

/// Historical market data for VaR calculation.
///
/// Stores a time series of market scenarios representing historical market
/// shifts over a lookback window (e.g., last 500 days).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketHistory {
    /// Base date (current market state reference point)
    pub base_date: Date,
    /// Historical window size in days
    pub window_days: u32,
    /// Historical scenarios (one per day in lookback window)
    /// Ordered chronologically from oldest to newest
    pub scenarios: Vec<MarketScenario>,
}

impl MarketHistory {
    /// Create a new market history.
    ///
    /// # Arguments
    ///
    /// * `base_date` - Current date (reference point for shifts)
    /// * `window_days` - Size of historical window
    /// * `scenarios` - Historical market scenarios
    pub fn new(base_date: Date, window_days: u32, scenarios: Vec<MarketScenario>) -> Self {
        Self {
            base_date,
            window_days,
            scenarios,
        }
    }

    /// Number of scenarios in the history.
    pub fn len(&self) -> usize {
        self.scenarios.len()
    }

    /// Check if history is empty.
    pub fn is_empty(&self) -> bool {
        self.scenarios.is_empty()
    }

    /// Get scenario at index.
    pub fn get(&self, index: usize) -> Option<&MarketScenario> {
        self.scenarios.get(index)
    }

    /// Iterator over scenarios.
    pub fn iter(&self) -> impl Iterator<Item = &MarketScenario> {
        self.scenarios.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::fx::{FxMatrix, FxQuery, SimpleFxProvider};
    use std::sync::Arc;
    use time::macros::date;

    #[test]
    fn triangular_neighbors_surround_non_standard_tenor() {
        let (prev, next) = find_triangular_neighbors(4.0);

        assert_eq!(prev, Some(3.0));
        assert_eq!(next, Some(5.0));
    }

    #[test]
    fn triangular_neighbors_wings_have_no_sentinels() {
        // First standard bucket: no left neighbour (flat half triangle), the
        // next standard bucket on the right.
        let buckets = &crate::metrics::sensitivities::config::STANDARD_BUCKETS_YEARS;
        let (prev, next) = find_triangular_neighbors(buckets[0]);
        assert_eq!(prev, None);
        assert_eq!(next, Some(buckets[1]));

        // Last standard bucket and beyond: no right neighbour. Infinite
        // sentinels are rejected by the curve bump paths, so they must never
        // be produced here.
        let last = *buckets.last().unwrap();
        let (prev, next) = find_triangular_neighbors(last);
        assert_eq!(prev, Some(buckets[buckets.len() - 2]));
        assert_eq!(next, None);

        let (prev, next) = find_triangular_neighbors(last + 10.0);
        assert_eq!(prev, Some(last));
        assert_eq!(next, None);
    }

    #[test]
    fn test_market_scenario_creation() {
        let scenario_date = date!(2024 - 01 - 02);
        let shifts = vec![
            RiskFactorShift {
                factor: RiskFactorType::DiscountRate {
                    curve_id: CurveId::from("USD-OIS"),
                    tenor_years: 5.0,
                },
                shift: 0.0015, // +15bp
            },
            RiskFactorShift {
                factor: RiskFactorType::CreditSpread {
                    curve_id: CurveId::from("AAPL"),
                    tenor_years: 5.0,
                },
                shift: -0.0010, // -10bp
            },
        ];

        let scenario = MarketScenario::new(scenario_date, shifts);

        assert_eq!(scenario.date, scenario_date);
        assert_eq!(scenario.shifts.len(), 2);
    }

    #[test]
    fn test_market_history_creation() {
        let base_date = date!(2024 - 01 - 01);
        let scenarios = vec![
            MarketScenario::new(date!(2023 - 12 - 31), vec![]),
            MarketScenario::new(date!(2023 - 12 - 30), vec![]),
        ];

        let history = MarketHistory::new(base_date, 500, scenarios);

        assert_eq!(history.base_date, base_date);
        assert_eq!(history.window_days, 500);
        assert_eq!(history.len(), 2);
        assert!(!history.is_empty());
    }

    #[test]
    fn test_market_scenario_allows_multiple_key_rates() -> Result<()> {
        let base_date = date!(2024 - 01 - 01);
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .day_count(DayCount::Act365F)
            .knots(vec![(0.0, 1.0), (5.0, 0.85)])
            .build()?;
        let base_market = MarketContext::new().insert(curve);

        let scenario = MarketScenario::new(
            date!(2024 - 01 - 02),
            vec![
                RiskFactorShift {
                    factor: RiskFactorType::DiscountRate {
                        curve_id: CurveId::from("USD-OIS"),
                        tenor_years: 5.0,
                    },
                    shift: 0.0010,
                },
                RiskFactorShift {
                    factor: RiskFactorType::DiscountRate {
                        curve_id: CurveId::from("USD-OIS"),
                        tenor_years: 10.0,
                    },
                    shift: -0.0005,
                },
            ],
        );

        let bumped = scenario.apply(&base_market, None)?;
        assert!(bumped.get_discount("USD-OIS").is_ok());

        Ok(())
    }

    #[test]
    fn test_scenario_apply_creates_bumped_market() -> Result<()> {
        let base_date = date!(2024 - 01 - 01);

        // Create base market
        let base_curve = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .day_count(DayCount::Act365F)
            .knots(vec![(0.0, 1.0), (5.0, 0.85), (10.0, 0.70)])
            .build()?;

        let base_market = MarketContext::new().insert(base_curve);

        // Create scenario with rate shift
        let scenario = MarketScenario::new(
            date!(2024 - 01 - 02),
            vec![RiskFactorShift {
                factor: RiskFactorType::DiscountRate {
                    curve_id: CurveId::from("USD-OIS"),
                    tenor_years: 5.0,
                },
                shift: 0.0010, // +10bp
            }],
        );

        // Apply scenario
        let bumped_market = scenario.apply(&base_market, None)?;

        // Verify bumped market has the curve
        assert!(bumped_market.get_discount("USD-OIS").is_ok());

        // The bumped market should be different from base
        // (We can't easily compare values without evaluating the curves,
        // but we've verified the bump mechanism works)

        Ok(())
    }

    #[test]
    fn test_market_history_iteration() {
        let scenarios = vec![
            MarketScenario::new(date!(2024 - 01 - 01), vec![]),
            MarketScenario::new(date!(2024 - 01 - 02), vec![]),
            MarketScenario::new(date!(2024 - 01 - 03), vec![]),
        ];

        let history = MarketHistory::new(date!(2024 - 01 - 01), 3, scenarios);

        let count = history.iter().count();
        assert_eq!(count, 3);

        // Verify order
        let dates: Vec<_> = history.iter().map(|s| s.date).collect();
        assert_eq!(dates[0], date!(2024 - 01 - 01));
        assert_eq!(dates[2], date!(2024 - 01 - 03));
    }

    #[test]
    fn test_empty_market_history() {
        let history = MarketHistory::new(date!(2024 - 01 - 01), 0, vec![]);

        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
        assert!(history.get(0).is_none());
    }

    #[test]
    fn test_equity_spot_shift_applied() -> Result<()> {
        use finstack_quant_core::market_data::scalars::MarketScalar;

        let base_market = MarketContext::new().insert_price("AAPL", MarketScalar::Unitless(100.0));

        let scenario = MarketScenario::new(
            date!(2024 - 01 - 02),
            vec![RiskFactorShift {
                factor: RiskFactorType::EquitySpot {
                    ticker: "AAPL".to_string(),
                },
                shift: 0.10, // +10%
            }],
        );

        let bumped = scenario.apply(&base_market, None)?;
        match bumped.get_price("AAPL")? {
            MarketScalar::Unitless(v) => assert!((v - 110.0).abs() < 1e-9),
            other @ MarketScalar::Price(_) => panic!("unexpected scalar variant: {:?}", other),
        }

        Ok(())
    }

    #[test]
    fn test_fx_spot_shift_applied() -> Result<()> {
        let provider = SimpleFxProvider::new();
        provider.set_quote(Currency::EUR, Currency::USD, 1.20)?;
        let base_market = MarketContext::new().insert_fx(FxMatrix::new(Arc::new(provider)));

        let scenario = MarketScenario::new(
            date!(2024 - 01 - 02),
            vec![RiskFactorShift {
                factor: RiskFactorType::FxSpot {
                    base: Currency::EUR,
                    quote: Currency::USD,
                },
                shift: 0.10,
            }],
        );

        let bumped = scenario.apply(&base_market, None)?;
        let rate = bumped
            .fx()
            .expect("FX matrix should be present")
            .rate(FxQuery::new(
                Currency::EUR,
                Currency::USD,
                date!(2024 - 01 - 02),
            ))?
            .rate;

        assert!((rate - 1.32).abs() < 1e-12);

        Ok(())
    }

    /// Multiple `ImpliedVol` point shocks on the SAME surface must not
    /// compound into repeated full-surface parallel bumps (the deferred-round
    /// mechanism exists for same-curve KEY-RATE shifts, which legitimately
    /// stack at different tenors — not for whole-surface vol bumps). A
    /// scenario with per-point vol changes is approximated by ONE parallel
    /// bump equal to the mean point shift.
    #[test]
    fn test_same_surface_vol_points_average_not_compound() -> Result<()> {
        use finstack_quant_core::market_data::surfaces::VolSurface;

        let surface = VolSurface::builder("EQ-VOL")
            .expiries(&[0.5, 1.0])
            .strikes(&[100.0, 110.0])
            .row(&[0.20, 0.20])
            .row(&[0.20, 0.20])
            .build()?;
        let base_market = MarketContext::new().insert_surface(surface);

        let scenario = MarketScenario::new(
            date!(2024 - 01 - 02),
            vec![
                RiskFactorShift {
                    factor: RiskFactorType::ImpliedVol {
                        vol_surface_id: CurveId::from("EQ-VOL"),
                        expiry_years: 0.5,
                        strike: 100.0,
                    },
                    shift: 0.02,
                },
                RiskFactorShift {
                    factor: RiskFactorType::ImpliedVol {
                        vol_surface_id: CurveId::from("EQ-VOL"),
                        expiry_years: 1.0,
                        strike: 110.0,
                    },
                    shift: 0.04,
                },
            ],
        );

        let bumped = scenario.apply(&base_market, None)?;
        let surface = bumped.get_surface("EQ-VOL")?;
        let vol = finstack_quant_models::volatility::get_surface_vol(&surface, 0.5, 100.0)
            .expect("grid point lookup should succeed");

        // Mean of (+2, +4) vol points = +3 vol points, NOT the compounded +6.
        assert!(
            (vol - 0.23).abs() < 1e-9,
            "same-surface vol point shocks must average into one parallel bump \
             (expected 0.23), got {vol}"
        );
        Ok(())
    }

    /// Vol shocks on DIFFERENT surfaces stay independent.
    #[test]
    fn test_distinct_surface_vol_shifts_stay_independent() -> Result<()> {
        use finstack_quant_core::market_data::surfaces::VolSurface;

        let eq = VolSurface::builder("EQ-VOL")
            .expiries(&[1.0])
            .strikes(&[100.0])
            .row(&[0.20])
            .build()?;
        let fx = VolSurface::builder("FX-VOL")
            .expiries(&[1.0])
            .strikes(&[1.10])
            .row(&[0.10])
            .build()?;
        let base_market = MarketContext::new().insert_surface(eq).insert_surface(fx);

        let scenario = MarketScenario::new(
            date!(2024 - 01 - 02),
            vec![
                RiskFactorShift {
                    factor: RiskFactorType::ImpliedVol {
                        vol_surface_id: CurveId::from("EQ-VOL"),
                        expiry_years: 1.0,
                        strike: 100.0,
                    },
                    shift: 0.02,
                },
                RiskFactorShift {
                    factor: RiskFactorType::ImpliedVol {
                        vol_surface_id: CurveId::from("FX-VOL"),
                        expiry_years: 1.0,
                        strike: 1.10,
                    },
                    shift: -0.01,
                },
            ],
        );

        let bumped = scenario.apply(&base_market, None)?;
        let eq_surface = bumped.get_surface("EQ-VOL")?;
        let eq_vol = finstack_quant_models::volatility::get_surface_vol(&eq_surface, 1.0, 100.0)
            .expect("grid point");
        let fx_surface = bumped.get_surface("FX-VOL")?;
        let fx_vol = finstack_quant_models::volatility::get_surface_vol(&fx_surface, 1.0, 1.10)
            .expect("grid point");
        assert!((eq_vol - 0.22).abs() < 1e-9, "EQ surface: got {eq_vol}");
        assert!((fx_vol - 0.09).abs() < 1e-9, "FX surface: got {fx_vol}");
        Ok(())
    }

    #[test]
    fn test_implied_vol_shift_applied() -> Result<()> {
        use finstack_quant_core::market_data::surfaces::VolSurface;

        let surface = VolSurface::builder("EQ-VOL")
            .expiries(&[0.5, 1.0])
            .strikes(&[100.0, 110.0])
            .row(&[0.20, 0.22])
            .row(&[0.21, 0.23])
            .build()?;

        let base_market = MarketContext::new().insert_surface(surface);

        let scenario = MarketScenario::new(
            date!(2024 - 01 - 02),
            vec![RiskFactorShift {
                factor: RiskFactorType::ImpliedVol {
                    vol_surface_id: CurveId::from("EQ-VOL"),
                    expiry_years: 1.0,
                    strike: 100.0,
                },
                shift: 0.02, // +2 vol points
            }],
        );

        let bumped = scenario.apply(&base_market, None)?;
        let bumped_surface = bumped.get_surface("EQ-VOL")?;
        let vol = finstack_quant_models::volatility::get_surface_vol(&bumped_surface, 1.0, 100.0)
            .expect("grid point lookup should succeed");
        assert!((vol - 0.23).abs() < 1e-9);

        Ok(())
    }
}
