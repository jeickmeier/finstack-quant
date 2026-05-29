//! Portfolio margin aggregation.

use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;

use finstack_margin::{ImMethodology, NettingSetId, SimmCalculator, SimmSensitivities};

use crate::margin::netting_set::{NettingSet, NettingSetManager};
use crate::margin::results::{NettingSetMargin, PortfolioMarginResult};
use crate::portfolio::Portfolio;
use crate::position::Position;
use crate::types::PositionId;
use crate::{Error, Result};

// ============================================================================
// Portfolio Margin Aggregator
// ============================================================================

/// Aggregates margin requirements across a portfolio.
///
/// Organizes positions into netting sets and calculates aggregate
/// margin requirements with proper netting of sensitivities.
///
/// # References
///
/// - `docs/REFERENCES.md#isda-simm`
#[derive(Debug)]
pub struct PortfolioMarginAggregator {
    /// Netting set manager
    netting_sets: NettingSetManager,
    /// Position references for calculation
    positions: Vec<(PositionId, NettingSetId)>,
    /// Base currency for aggregation
    base_currency: Currency,
    /// Cached SIMM calculator for efficiency
    simm_calculator: SimmCalculator,
}

impl PortfolioMarginAggregator {
    /// Create a new aggregator with a base currency.
    ///
    /// # Arguments
    ///
    /// * `base_currency` - Reporting currency for aggregated initial and
    ///   variation margin.
    ///
    /// # Returns
    ///
    /// Empty aggregator with no positions or netting sets loaded.
    #[must_use]
    pub fn new(base_currency: Currency) -> Self {
        Self {
            netting_sets: NettingSetManager::new(),
            positions: Vec::new(),
            base_currency,
            simm_calculator: SimmCalculator::default(),
        }
    }

    /// Create an aggregator from a portfolio.
    ///
    /// Automatically organizes positions into netting sets based on their
    /// margin specifications.
    ///
    /// # Arguments
    ///
    /// * `portfolio` - Portfolio whose positions should seed the aggregator.
    ///
    /// # Returns
    ///
    /// Aggregator pre-populated with positions that expose margin metadata.
    #[must_use]
    pub fn from_portfolio(portfolio: &Portfolio) -> Self {
        let mut aggregator = Self::new(portfolio.base_ccy);

        // Iterate through all positions
        for position in &portfolio.positions {
            aggregator.add_position(position);
        }

        aggregator
    }

    /// Add a position to the aggregator.
    ///
    /// The position will be assigned to its appropriate netting set
    /// based on its margin specification.
    ///
    /// # Arguments
    ///
    /// * `position` - Position to inspect and register.
    pub fn add_position(&mut self, position: &Position) {
        // Try to get netting set ID from the instrument
        let netting_set_id = self.get_netting_set_for_position(position);

        if let Some(ns_id) = netting_set_id {
            self.netting_sets
                .add_position(position, Some(ns_id.clone()));
            self.positions.push((position.position_id.clone(), ns_id));
        }
    }

    /// Get the netting set ID for a position based on its instrument.
    fn get_netting_set_for_position(&self, position: &Position) -> Option<NettingSetId> {
        position
            .instrument
            .as_marginable()
            .and_then(|m| m.netting_set_id())
    }

    /// Calculate margin requirements for the portfolio.
    ///
    /// Returns aggregated margin results by netting set.
    ///
    /// # Arguments
    ///
    /// * `portfolio` - Portfolio used for mark-to-market lookups.
    /// * `market` - Market context required for VM and SIMM sensitivity extraction.
    /// * `as_of` - Valuation date for the margin run.
    ///
    /// # Returns
    ///
    /// Portfolio-level margin report including per-netting-set totals and
    /// degraded positions.
    ///
    /// # Errors
    ///
    /// Propagates portfolio-level calculation failures such as missing FX needed
    /// for base-currency reporting or unexpected aggregation mismatches.
    pub fn calculate(
        &mut self,
        portfolio: &Portfolio,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<PortfolioMarginResult> {
        let mut result = PortfolioMarginResult::new(as_of, self.base_currency);

        // Phase A (parallel): compute SIMM sensitivities for every tracked
        // position. Each `simm_sensitivities` call is a read-only function of
        // the shared `MarketContext` and position state; per-call work
        // (sensitivity extraction across the ISDA SIMM risk classes) dwarfs
        // the Rayon dispatch overhead, so par_iter is a clear win for large
        // margin portfolios. Results are collected positionally so the
        // downstream merge keeps the same deterministic order the serial
        // version produced.
        use rayon::prelude::*;
        let position_sensitivities: Vec<(PositionId, NettingSetId, Result<SimmSensitivities>)> =
            self.positions
                .par_iter()
                .filter_map(|(pos_id, ns_id)| {
                    portfolio.get_position(pos_id.as_str()).map(|position| {
                        let sens = self.calculate_position_sensitivities(position, market, as_of);
                        (position.position_id.clone(), ns_id.clone(), sens)
                    })
                })
                .collect();

        // Phase B (serial): merge into the netting set map. Serial keeps
        // mutation to `self.netting_sets` trivially correct and preserves the
        // tracing warn order callers expect from the prior implementation.
        for (position_id, ns_id, sens_result) in position_sensitivities {
            match sens_result {
                Ok(sensitivities) => {
                    self.netting_sets
                        .merge_sensitivities(&ns_id, &sensitivities);
                }
                Err(err) => {
                    tracing::warn!(
                        position_id = %position_id,
                        error = %err,
                        "Failed to calculate SIMM sensitivities for margin aggregation"
                    );
                    result.add_degraded_position(position_id, err.to_string());
                }
            }
        }

        // Calculate margin for each netting set
        for (_ns_id, netting_set) in self.netting_sets.iter() {
            let (ns_margin, degraded_positions) =
                self.calculate_netting_set_margin(netting_set, portfolio, market, as_of)?;
            for (position_id, message) in degraded_positions {
                result.add_degraded_position(position_id, message);
            }
            // Currency mismatch is impossible here since we create all margins
            // with self.base_currency, but we handle the error for API consistency.
            result.add_netting_set(ns_margin).map_err(|e| {
                Error::validation(format!(
                    "Unexpected currency mismatch in margin aggregation: {}",
                    e
                ))
            })?;
        }

        // Count positions without margin
        result.positions_without_margin = portfolio
            .positions
            .len()
            .saturating_sub(result.total_positions);

        Ok(result)
    }

    /// Calculate sensitivities for a single position.
    fn calculate_position_sensitivities(
        &self,
        position: &Position,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<SimmSensitivities> {
        if let Some(marginable) = position.instrument.as_marginable() {
            let sens = marginable
                .simm_sensitivities(market, as_of)
                .map_err(|e| Error::valuation(position.position_id.clone(), e.to_string()))?;
            // FX-collapse to the aggregator base currency before the netting-set
            // merge sums raw amounts. Without this, sensitivities produced in a
            // position's own base currency are added across currencies, breaking
            // currency safety and the SIMM calculation-currency convention.
            self.convert_sensitivities_to_base(sens, market, as_of, &position.position_id)
        } else {
            // Default: return empty sensitivities
            Ok(SimmSensitivities::new(self.base_currency))
        }
    }

    /// Re-express a position's SIMM sensitivities in the aggregator base
    /// currency via an explicit spot FX conversion.
    fn convert_sensitivities_to_base(
        &self,
        sensitivities: SimmSensitivities,
        market: &MarketContext,
        as_of: Date,
        position_id: &PositionId,
    ) -> Result<SimmSensitivities> {
        if sensitivities.base_currency == self.base_currency {
            return Ok(sensitivities);
        }
        // One unit of the sensitivity currency expressed in base currency is the
        // spot conversion factor applied uniformly to every (amount) entry.
        let fx_rate = self
            .convert_to_base(Money::new(1.0, sensitivities.base_currency), market, as_of)
            .map_err(|e| Error::valuation(position_id.clone(), e.to_string()))?;
        Ok(sensitivities.scaled_to_currency(self.base_currency, fx_rate))
    }

    /// Calculate margin for a netting set.
    fn calculate_netting_set_margin(
        &self,
        netting_set: &NettingSet,
        portfolio: &Portfolio,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<(NettingSetMargin, Vec<(PositionId, String)>)> {
        // Calculate aggregated VM from position MTMs, FX-converting to base currency
        let mut total_mtm = 0.0;
        let mut position_count = 0;
        let mut degraded_positions = Vec::new();

        for pos_id in &netting_set.positions {
            if let Some(position) = portfolio.get_position(pos_id.as_str()) {
                match self.get_position_mtm(position, market, as_of) {
                    Ok(mtm) => {
                        // FX-convert MTM to base currency if necessary
                        let mtm_base = if mtm.currency() == self.base_currency {
                            Ok(mtm.amount())
                        } else {
                            self.convert_to_base(mtm, market, as_of)
                        };
                        match mtm_base {
                            Ok(value) => {
                                total_mtm += value;
                                position_count += 1;
                            }
                            Err(err) => {
                                tracing::warn!(
                                    position_id = %position.position_id,
                                    error = %err,
                                    "Failed to FX-convert VM MTM during margin aggregation"
                                );
                                degraded_positions
                                    .push((position.position_id.clone(), err.to_string()));
                            }
                        }
                    }
                    Err(err) => {
                        tracing::warn!(
                            position_id = %position.position_id,
                            error = %err,
                            "Failed to calculate VM MTM for margin aggregation"
                        );
                        degraded_positions.push((position.position_id.clone(), err.to_string()));
                    }
                }
            }
        }

        // VM is the net MTM (positive = we owe, negative = they owe)
        let vm = Money::new(total_mtm, self.base_currency);

        // Calculate IM + breakdown from aggregated sensitivities in a single pass.
        let (im, simm_breakdown) =
            if let Some(ref sensitivities) = netting_set.aggregated_sensitivities {
                let (im, breakdown) = self.calculate_simm_with_breakdown(sensitivities);
                (im, Some((sensitivities.clone(), breakdown)))
            } else {
                (Money::new(0.0, self.base_currency), None)
            };

        // Determine methodology
        let methodology = if netting_set.is_cleared() {
            ImMethodology::ClearingHouse
        } else {
            ImMethodology::Simm
        };

        let mut result = NettingSetMargin::new(
            netting_set.id.clone(),
            as_of,
            im,
            vm,
            position_count,
            methodology,
        );

        if let Some((sensitivities, breakdown)) = simm_breakdown {
            result = result.with_simm_breakdown(sensitivities, breakdown);
        }

        Ok((result, degraded_positions))
    }

    /// Get MTM for a position in its native currency, scaled by position quantity.
    ///
    /// The underlying `mtm_for_vm` returns unit-notional MTM; this method
    /// applies [`Position::scale_value`] so VM reflects the actual holding.
    fn get_position_mtm(
        &self,
        position: &Position,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        if let Some(marginable) = position.instrument.as_marginable() {
            let unit_mtm = marginable
                .mtm_for_vm(market, as_of)
                .map_err(|e| Error::valuation(position.position_id.clone(), e.to_string()))?;
            Ok(position.scale_value(unit_mtm))
        } else {
            // Default: return zero
            Ok(Money::new(0.0, self.base_currency))
        }
    }

    /// Convert a monetary amount to base currency using the FX matrix.
    ///
    /// Thin wrapper over [`crate::fx::convert_to_base`] that returns the
    /// converted amount as `f64` (margin aggregation works in scalar space).
    fn convert_to_base(&self, amount: Money, market: &MarketContext, as_of: Date) -> Result<f64> {
        crate::fx::convert_to_base(amount, as_of, market, self.base_currency).map(|m| m.amount())
    }

    /// Calculate SIMM total IM and breakdown by risk class in a single pass.
    ///
    /// Returns (total_im, breakdown_by_risk_class). Uses the cached
    /// `SimmCalculator` for efficiency.
    fn calculate_simm_with_breakdown(
        &self,
        sensitivities: &SimmSensitivities,
    ) -> (Money, finstack_core::HashMap<String, Money>) {
        let (total_im, breakdown) = self
            .simm_calculator
            .calculate_from_sensitivities(sensitivities, self.base_currency);
        (Money::new(total_im, self.base_currency), breakdown)
    }

    /// Get the number of netting sets.
    ///
    /// # Returns
    ///
    /// Number of tracked netting sets.
    #[must_use]
    pub fn netting_set_count(&self) -> usize {
        self.netting_sets.count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aggregator_creation() {
        let aggregator = PortfolioMarginAggregator::new(Currency::USD);
        assert_eq!(aggregator.netting_set_count(), 0);
    }
}
