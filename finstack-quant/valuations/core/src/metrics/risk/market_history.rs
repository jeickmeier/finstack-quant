//! Market history storage for Historical VaR scenarios.
//!
//! Stores historical shift scenarios (`MarketScenario`, `RiskFactorShift`) and
//! `MarketHistory` series as wire-compatible data inputs. Scenario application
//! and tabular export live in `finstack_quant_valuations::metrics::risk`.

use crate::metrics::risk::RiskFactorType;
use finstack_quant_core::dates::Date;
use finstack_quant_core::types::CurveId;
use std::collections::BTreeMap;

/// Historical shift for a single risk factor on a single date.
///
/// Represents the change in a market variable from its base value.
/// For example, a +15bp shift in 5Y USD rates.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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

/// Treatment of a hazard source whose calibration discount curve moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UnshiftedHazardPolicy {
    /// Hold the source's original intensity while the rate scenario moves.
    FrozenIntensity,
    /// Refit to unchanged original credit observations on the shocked discount curve.
    RefitOriginalQuotes,
}

/// Collection of all risk factor shifts for a single historical date.
///
/// Represents a complete market scenario that can be applied to revalue
/// a portfolio. Each scenario contains shifts for all relevant risk factors.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MarketScenario {
    /// Historical date this scenario represents
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// All risk factor shifts on this date (relative to base date)
    pub shifts: Vec<RiskFactorShift>,
    /// Required treatments for unshifted calibrated hazards whose discount dependencies move.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub unshifted_hazard_policies: BTreeMap<CurveId, UnshiftedHazardPolicy>,
}

impl MarketScenario {
    /// Create a new market scenario.
    pub fn new(date: Date, shifts: Vec<RiskFactorShift>) -> Self {
        Self {
            date,
            shifts,
            unshifted_hazard_policies: BTreeMap::new(),
        }
    }

    /// Re-date this scenario so its shifts apply as of `date`.
    ///
    /// Historical VaR replays a past day's shifts on today's market. FX spot
    /// shifts are quoted as of `self.date`, so they must be applied at the
    /// valuation date, not the historical date the shifts were observed on.
    ///
    /// # Arguments
    ///
    /// * `date` - Valuation date the shifts are applied on; replaces the
    ///   historical observation date. The shifts themselves are unchanged.
    #[must_use]
    pub fn at_date(mut self, date: Date) -> Self {
        self.date = date;
        self
    }
}

/// Collection of historical market scenarios for VaR calculation.
///
/// Stores a time series of market scenarios representing historical market
/// shifts over a lookback window (e.g., last 500 days).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MarketHistory {
    /// Base date (current market state reference point)
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
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
    use time::macros::date;

    #[test]
    fn test_market_scenario_creation() {
        let scenario_date = date!(2024 - 01 - 02);
        let shifts = vec![
            RiskFactorShift {
                factor: RiskFactorType::DiscountRate {
                    curve_id: CurveId::from("USD-OIS"),
                    tenor_years: 5.0,
                    grid_years: crate::metrics::STANDARD_BUCKETS_YEARS.to_vec(),
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
}
