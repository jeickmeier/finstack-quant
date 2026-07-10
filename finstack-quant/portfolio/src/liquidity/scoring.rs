//! Portfolio-level liquidity scoring.
//!
//! Scores each position by its liquidity characteristics (days to liquidate,
//! tier, cost) and aggregates into a portfolio-level report.

use crate::portfolio::Portfolio;
use crate::types::PositionId;
use crate::valuation::PortfolioValuation;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::types::{
    classify_tier, days_to_liquidate, LiquidityConfig, LiquidityProfile, LiquidityTier,
    TierAllocation,
};

/// Minimum portfolio size at which per-position liquidity scoring is run in
/// parallel. Below this threshold the work per position (a few lookups and
/// divisions) is too small to amortize Rayon's thread-pool dispatch overhead,
/// so a serial iterator is used instead.
const PARALLEL_SCORING_THRESHOLD: usize = 512;

/// Liquidity score for a single position.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PositionLiquidityScore {
    /// Position identifier.
    pub position_id: PositionId,

    /// Instrument identifier.
    pub instrument_id: String,

    /// Absolute position value in portfolio base currency.
    pub position_value: f64,

    /// Days required to fully liquidate at the configured participation rate.
    ///
    /// ```text
    /// days_to_liquidate = |position_quantity| / (participation_rate * ADV)
    /// ```
    pub days_to_liquidate: f64,

    /// Liquidity tier classification.
    pub tier: LiquidityTier,

    /// Position value as a percentage of ADV (in notional terms).
    ///
    /// ```text
    /// pct_adv = |position_quantity| / ADV * 100
    /// ```
    pub pct_of_adv: f64,

    /// Estimated one-way liquidation cost in basis points.
    pub liquidation_cost_bps: f64,
}

/// Complete portfolio liquidity analysis.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PortfolioLiquidityReport {
    /// Per-position liquidity scores, sorted by days_to_liquidate descending
    /// (most illiquid first).
    pub position_scores: Vec<PositionLiquidityScore>,

    /// Portfolio NAV used for percentage calculations.
    pub portfolio_nav: f64,

    /// Percentage of NAV in each liquidity tier.
    pub tier_allocation: TierAllocation,

    /// Weighted-average days to liquidate the entire portfolio.
    pub weighted_avg_days_to_liquidate: f64,

    /// Largest position as a percentage of its instrument's ADV.
    ///
    /// High values indicate concentration risk in a single name.
    pub max_pct_of_adv: f64,

    /// Position with the highest concentration risk.
    pub most_concentrated_position: Option<PositionId>,

    /// Percentage of NAV that can be liquidated within N days.
    ///
    /// Keyed by number of days: {1: 45.2, 5: 78.0, 20: 95.0, 60: 100.0}.
    pub liquidation_schedule: IndexMap<u32, f64>,

    /// Positions without liquidity profiles (excluded from scoring).
    pub unscored_positions: Vec<PositionId>,
}

enum PositionLiquidityOutcome {
    Scored(PositionLiquidityScore),
    Unscored(PositionId),
}

/// Score portfolio liquidity across all positions.
///
/// # Arguments
///
/// * `portfolio` - Portfolio with positions to score.
/// * `valuation` - Most recent portfolio valuation (for position values).
/// * `profiles` - Map from instrument_id to liquidity profile.
/// * `config` - Liquidity scoring configuration.
///
/// # Returns
///
/// A complete [`PortfolioLiquidityReport`].
///
/// # Parallelism
///
/// For portfolios with at least 512 positions (the internal
/// `PARALLEL_SCORING_THRESHOLD`),
/// per-position scoring runs via Rayon's parallel iterator. For smaller
/// portfolios the work per position is too small to amortize the thread-pool
/// overhead, so a serial iterator is used. Results are sorted deterministically
/// after collection regardless of code path.
pub fn score_portfolio_liquidity(
    portfolio: &Portfolio,
    valuation: &PortfolioValuation,
    profiles: &HashMap<String, LiquidityProfile>,
    config: &LiquidityConfig,
) -> PortfolioLiquidityReport {
    let portfolio_nav = valuation.total_base_ccy.amount();
    let nav_abs = portfolio_nav.abs();

    let mut position_scores = Vec::new();
    let mut unscored_positions = Vec::new();

    // Score each position
    let score_fn = |pos: &crate::position::Position| -> PositionLiquidityOutcome {
        let Some(profile) = profiles.get(&pos.instrument_id) else {
            return PositionLiquidityOutcome::Unscored(pos.position_id.clone());
        };

        let Some(position_value) = valuation.get_position_value(pos.position_id.as_str()) else {
            return PositionLiquidityOutcome::Unscored(pos.position_id.clone());
        };
        let pv = position_value.value_base.amount().abs();

        // ADV is expressed in instrument units. Use the canonical unit-aware
        // position multiplier rather than reconstructing units from a
        // base-currency PV and a potentially native-currency market price.
        let position_units = pos.scale_factor().abs();

        let dtl = days_to_liquidate(
            position_units,
            profile.avg_daily_volume,
            config.participation_rate,
        );

        let tier = classify_tier(dtl, &config.tier_thresholds);

        let pct_adv = if profile.avg_daily_volume > 0.0 {
            position_units / profile.avg_daily_volume * 100.0
        } else {
            f64::INFINITY
        };

        // Liquidation cost: half-spread as basis points
        let liquidation_cost_bps = profile.relative_spread() * 0.5 * 10_000.0;

        PositionLiquidityOutcome::Scored(PositionLiquidityScore {
            position_id: pos.position_id.clone(),
            instrument_id: pos.instrument_id.clone(),
            position_value: pv,
            days_to_liquidate: dtl,
            tier,
            pct_of_adv: pct_adv,
            liquidation_cost_bps,
        })
    };

    let positions = portfolio.positions();
    let results: Vec<_> = if positions.len() >= PARALLEL_SCORING_THRESHOLD {
        use rayon::prelude::*;
        positions.par_iter().map(score_fn).collect()
    } else {
        positions.iter().map(score_fn).collect()
    };

    for result in results {
        match result {
            PositionLiquidityOutcome::Scored(score) => position_scores.push(score),
            PositionLiquidityOutcome::Unscored(position_id) => {
                unscored_positions.push(position_id);
            }
        }
    }

    // Sort by days_to_liquidate descending (most illiquid first)
    position_scores.sort_by(|a, b| {
        b.days_to_liquidate
            .partial_cmp(&a.days_to_liquidate)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Compute tier allocation
    let mut tier_allocation = TierAllocation::default();
    for score in &position_scores {
        if nav_abs > 0.0 {
            let pct = score.position_value / nav_abs * 100.0;
            match score.tier {
                LiquidityTier::Tier1 => tier_allocation.tier1_pct += pct,
                LiquidityTier::Tier2 => tier_allocation.tier2_pct += pct,
                LiquidityTier::Tier3 => tier_allocation.tier3_pct += pct,
                LiquidityTier::Tier4 => tier_allocation.tier4_pct += pct,
                LiquidityTier::Tier5 => tier_allocation.tier5_pct += pct,
            }
        }
    }

    // Weighted-average days to liquidate
    let total_scored_value: f64 = position_scores.iter().map(|s| s.position_value).sum();
    let weighted_avg_days_to_liquidate = if total_scored_value > 0.0 {
        if position_scores
            .iter()
            .any(|s| s.position_value > 0.0 && s.days_to_liquidate.is_infinite())
        {
            f64::INFINITY
        } else {
            position_scores
                .iter()
                .filter(|s| s.days_to_liquidate.is_finite())
                .map(|s| s.days_to_liquidate * s.position_value)
                .sum::<f64>()
                / total_scored_value
        }
    } else {
        0.0
    };

    // Maximum concentration
    let (max_pct_of_adv, most_concentrated_position) = position_scores
        .iter()
        .filter(|s| !s.pct_of_adv.is_nan())
        .max_by(|a, b| a.pct_of_adv.total_cmp(&b.pct_of_adv))
        .map(|s| (s.pct_of_adv, Some(s.position_id.clone())))
        .unwrap_or((0.0, None));

    // Liquidation schedule: % of NAV that can be liquidated within each
    // configured tier boundary. Schedule keys are derived from
    // `config.tier_thresholds` (ceil to whole trading days) so the schedule
    // stays aligned with the tier bucketing used in `tier_allocation`.
    let mut liquidation_schedule = IndexMap::new();
    let mut last_key: Option<u32> = None;
    for &threshold in &config.tier_thresholds {
        if !threshold.is_finite() || threshold <= 0.0 {
            continue;
        }
        let days = threshold.ceil() as u32;
        if last_key == Some(days) {
            continue;
        }
        last_key = Some(days);
        let liquidatable_value: f64 = position_scores
            .iter()
            .filter(|s| s.days_to_liquidate <= threshold)
            .map(|s| s.position_value)
            .sum();
        let pct = if nav_abs > 0.0 {
            liquidatable_value / nav_abs * 100.0
        } else {
            0.0
        };
        liquidation_schedule.insert(days, pct);
    }

    PortfolioLiquidityReport {
        position_scores,
        portfolio_nav,
        tier_allocation,
        weighted_avg_days_to_liquidate,
        max_pct_of_adv,
        most_concentrated_position,
        liquidation_schedule,
        unscored_positions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::{Position, PositionUnit};
    use crate::types::Entity;
    use crate::valuation::{PortfolioValuation, PositionValue};
    use crate::Portfolio;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::fx::FxConversionPolicy;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::rates::deposit::Deposit;
    use indexmap::IndexMap;
    use std::sync::Arc;
    use time::macros::date;

    #[test]
    fn position_score_serde_round_trip() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let score = PositionLiquidityScore {
            position_id: PositionId::new("POS1"),
            instrument_id: "AAPL".to_string(),
            position_value: 1_000_000.0,
            days_to_liquidate: 2.5,
            tier: LiquidityTier::Tier2,
            pct_of_adv: 5.0,
            liquidation_cost_bps: 3.5,
        };
        let json = serde_json::to_string(&score)?;
        let score2: PositionLiquidityScore = serde_json::from_str(&json)?;
        assert_eq!(score, score2);
        Ok(())
    }

    #[test]
    fn tier_allocation_sums_correctly() {
        let alloc = TierAllocation {
            tier1_pct: 40.0,
            tier2_pct: 30.0,
            tier3_pct: 15.0,
            tier4_pct: 10.0,
            tier5_pct: 5.0,
        };
        let sum =
            alloc.tier1_pct + alloc.tier2_pct + alloc.tier3_pct + alloc.tier4_pct + alloc.tier5_pct;
        assert!((sum - 100.0).abs() < 1e-10);
    }

    fn test_position(position_id: &str, instrument_id: &str) -> Position {
        let as_of = date!(2024 - 01 - 01);
        let deposit = Deposit::builder()
            .id(instrument_id.to_string().into())
            .notional(Money::new(1.0, Currency::USD))
            .start_date(as_of)
            .maturity(date!(2024 - 02 - 01))
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .discount_curve_id("USD".into())
            .build()
            .expect("test deposit should build");
        Position::new(
            position_id,
            "E",
            instrument_id,
            Arc::new(deposit),
            1.0,
            PositionUnit::Units,
        )
        .expect("test position should build")
    }

    fn synthetic_position_value(position_id: &str, amount: f64) -> PositionValue {
        PositionValue {
            position_id: PositionId::new(position_id),
            entity_id: "E".into(),
            value_native: Money::new(amount, Currency::USD),
            value_base: Money::new(amount, Currency::USD),
            metric_scale: 1.0,
            risk_metrics_complete: true,
            risk_error: None,
            valuation_result: None,
        }
    }

    #[test]
    fn mo13_infinite_dtl_drives_weighted_average_and_concentration() {
        let as_of = date!(2024 - 01 - 01);
        let liquid = test_position("LIQUID", "LIQ");
        let stuck = test_position("STUCK", "STUCK");
        let portfolio = Portfolio::builder("P")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new("E"))
            .position(liquid)
            .position(stuck)
            .build()
            .expect("portfolio should build");
        let valuation = PortfolioValuation {
            as_of,
            position_values: [
                (
                    PositionId::new("LIQUID"),
                    synthetic_position_value("LIQUID", 100.0),
                ),
                (
                    PositionId::new("STUCK"),
                    synthetic_position_value("STUCK", 100.0),
                ),
            ]
            .into_iter()
            .collect(),
            total_base_ccy: Money::new(200.0, Currency::USD),
            by_entity: IndexMap::new(),
            degraded_positions: Vec::new(),
            fx_collapse_policy: FxConversionPolicy::CashflowDate,
        };
        let profiles = [
            (
                "LIQ".to_string(),
                LiquidityProfile::new("LIQ", 1.0, 1.0, 1.0, 100.0, 10.0, 0.0)
                    .expect("profile should build"),
            ),
            (
                "STUCK".to_string(),
                LiquidityProfile::new("STUCK", 1.0, 1.0, 1.0, 0.0, 0.0, 0.0)
                    .expect("profile should build"),
            ),
        ]
        .into_iter()
        .collect();
        let config = LiquidityConfig {
            participation_rate: 1.0,
            ..LiquidityConfig::default()
        };

        let report = score_portfolio_liquidity(&portfolio, &valuation, &profiles, &config);

        assert_eq!(report.weighted_avg_days_to_liquidate, f64::INFINITY);
        assert_eq!(report.max_pct_of_adv, f64::INFINITY);
        assert_eq!(
            report.most_concentrated_position,
            Some(PositionId::new("STUCK"))
        );
    }

    #[test]
    fn scoring_uses_position_units_and_rejects_missing_valuation() {
        let as_of = date!(2024 - 01 - 01);
        let mut valued = test_position("VALUED", "VALUED_INST");
        valued.quantity = 1_000.0;
        let missing = test_position("MISSING", "MISSING_INST");
        let portfolio = Portfolio::builder("P")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new("E"))
            .position(valued)
            .position(missing)
            .build()
            .expect("portfolio should build");
        let valuation = PortfolioValuation {
            as_of,
            position_values: [(
                PositionId::new("VALUED"),
                synthetic_position_value("VALUED", 120_000.0),
            )]
            .into_iter()
            .collect(),
            total_base_ccy: Money::new(120_000.0, Currency::USD),
            by_entity: IndexMap::new(),
            degraded_positions: Vec::new(),
            fx_collapse_policy: FxConversionPolicy::CashflowDate,
        };
        let profiles = [
            (
                "VALUED_INST".to_string(),
                LiquidityProfile::new("VALUED_INST", 100.0, 99.0, 101.0, 10_000.0, 10.0, 0.0)
                    .expect("profile should build"),
            ),
            (
                "MISSING_INST".to_string(),
                LiquidityProfile::new("MISSING_INST", 100.0, 99.0, 101.0, 10_000.0, 10.0, 0.0)
                    .expect("profile should build"),
            ),
        ]
        .into_iter()
        .collect();
        let config = LiquidityConfig {
            participation_rate: 1.0,
            ..LiquidityConfig::default()
        };

        let report = score_portfolio_liquidity(&portfolio, &valuation, &profiles, &config);
        assert_eq!(report.position_scores.len(), 1);
        assert!((report.position_scores[0].days_to_liquidate - 0.1).abs() < 1.0e-12);
        assert_eq!(report.unscored_positions, vec![PositionId::new("MISSING")]);
    }
}
