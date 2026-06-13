//! Portfolio margin aggregation.

use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;

use finstack_margin::{
    ClearingHouseImCalculator, ImCalculator, ImMethodology, NettingSetId, SimmCalculator,
    SimmSensitivities, VmCalculator,
};

use crate::margin::netting_set::{NettingSet, NettingSetManager};
use crate::margin::results::{NettingSetMargin, PortfolioMarginResult};
use crate::portfolio::Portfolio;
use crate::position::{Position, PositionUnit};
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
        let Some(marginable) = position.instrument.as_marginable() else {
            return;
        };
        let netting_set_id = marginable.netting_set_id();
        let margin_spec = marginable.margin_spec().cloned();

        if let Some(ns_id) = netting_set_id {
            self.netting_sets
                .add_position(position, Some(ns_id.clone()), margin_spec);
            self.positions.push((position.position_id.clone(), ns_id));
        }
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
        self.netting_sets.reset_sensitivities();

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
                .map(|(pos_id, ns_id)| {
                    if let Some(position) = portfolio.get_position(pos_id.as_str()) {
                        let sens = self.calculate_position_sensitivities(position, market, as_of);
                        (position.position_id.clone(), ns_id.clone(), sens)
                    } else {
                        (
                            pos_id.clone(),
                            ns_id.clone(),
                            Err(Error::validation(format!(
                                "MO-15: tracked margin position '{pos_id}' is missing from portfolio"
                            ))),
                        )
                    }
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
            .saturating_sub(result.total_positions)
            + result.degraded_positions.len();

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
        if !sensitivities.fx_delta.is_empty() {
            return Err(Error::invalid_input(format!(
                "MO-17: cannot rebase SIMM FX delta from {} to {} for position '{}' without an explicit calculation-currency remap policy",
                sensitivities.base_currency, self.base_currency, position_id
            )));
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
            } else {
                degraded_positions.push((
                    pos_id.clone(),
                    format!("MO-15: tracked margin position '{pos_id}' is missing from portfolio"),
                ));
            }
        }

        // VM is the net CSA call amount after threshold/MTA terms when a
        // netting-set CSA is available; otherwise preserve the legacy raw MTM.
        let vm = self.apply_vm_terms(
            netting_set,
            Money::new(total_mtm, self.base_currency),
            as_of,
        )?;

        let (im, im_methodology, simm_breakdown) = if netting_set.is_cleared() {
            let im = self.calculate_clearing_im(
                netting_set,
                portfolio,
                market,
                as_of,
                &mut degraded_positions,
            )?;
            (im, ImMethodology::ClearingHouse, None)
        } else {
            let (im, simm_breakdown) =
                if let Some(ref sensitivities) = netting_set.aggregated_sensitivities {
                    let (im, breakdown) = self.calculate_simm_with_breakdown(sensitivities);
                    (im, Some((sensitivities.clone(), breakdown)))
                } else {
                    (Money::new(0.0, self.base_currency), None)
                };
            (im, ImMethodology::Simm, simm_breakdown)
        };

        let mut result = NettingSetMargin::new(
            netting_set.id.clone(),
            as_of,
            im,
            vm,
            position_count,
            im_methodology,
        );

        if let Some((sensitivities, breakdown)) = simm_breakdown {
            result = result.with_simm_breakdown(sensitivities, breakdown);
        }

        Ok((result, degraded_positions))
    }

    fn calculate_clearing_im(
        &self,
        netting_set: &NettingSet,
        portfolio: &Portfolio,
        market: &MarketContext,
        as_of: Date,
        degraded_positions: &mut Vec<(PositionId, String)>,
    ) -> Result<Money> {
        let calculator = ClearingHouseImCalculator::for_ccp(netting_set.id.counterparty_id());
        let mut total = 0.0;

        for pos_id in &netting_set.positions {
            let Some(position) = portfolio.get_position(pos_id.as_str()) else {
                degraded_positions.push((
                    pos_id.clone(),
                    "M-10: cleared position registration missing from portfolio".to_string(),
                ));
                continue;
            };
            let Some(marginable) = position.instrument.as_marginable() else {
                degraded_positions.push((
                    position.position_id.clone(),
                    "M-10: cleared position is not marginable".to_string(),
                ));
                continue;
            };

            match calculator.calculate(marginable, market, as_of) {
                Ok(im_result) => {
                    let amount = if im_result.amount.currency() == self.base_currency {
                        Ok(im_result.amount.amount())
                    } else {
                        self.convert_to_base(im_result.amount, market, as_of)
                    };
                    match amount {
                        Ok(value) => total += value,
                        Err(err) => {
                            degraded_positions.push((position.position_id.clone(), err.to_string()))
                        }
                    }
                }
                Err(err) => degraded_positions.push((
                    position.position_id.clone(),
                    format!("M-10: clearing IM calculation failed: {err}"),
                )),
            }
        }

        Ok(Money::new(total, self.base_currency))
    }

    fn apply_vm_terms(
        &self,
        netting_set: &NettingSet,
        gross_vm: Money,
        as_of: Date,
    ) -> Result<Money> {
        let Some(spec) = &netting_set.margin_spec else {
            return Ok(gross_vm);
        };

        let current_collateral = Money::new(0.0, self.base_currency);
        let vm_result = VmCalculator::new(spec.csa.clone())
            .calculate(gross_vm, current_collateral, as_of)
            .map_err(|e| Error::validation(format!("M-13: CSA VM calculation failed: {e}")))?;
        Ok(vm_result.net_margin())
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
            if let PositionUnit::Notional(Some(notional_ccy)) = position.unit {
                if notional_ccy != unit_mtm.currency() {
                    return Err(Error::invalid_input(format!(
                        "minor 18: position '{}' notional currency {} does not match VM currency {}",
                        position.position_id,
                        notional_ccy,
                        unit_mtm.currency()
                    )));
                }
            }
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
    use crate::position::{Position, PositionUnit};
    use crate::types::{Entity, DUMMY_ENTITY_ID};
    use crate::Portfolio;
    use finstack_core::types::Attributes;
    use finstack_margin::{
        ClearingHouseImCalculator, CsaSpec, Marginable, OtcMarginSpec, VmParameters,
    };
    use finstack_valuations::instruments::{Instrument, MarketDependencies};
    use finstack_valuations::pricer::InstrumentType;
    use std::any::Any;
    use std::sync::Arc;
    use time::macros::date;

    #[derive(Clone)]
    struct TestMarginableInstrument {
        id: String,
        netting_set_id: NettingSetId,
        attributes: Attributes,
        ir_delta: f64,
        mtm: Money,
        margin_spec: Option<OtcMarginSpec>,
        im_exposure_base: Option<Money>,
    }

    impl TestMarginableInstrument {
        fn new(id: &str, netting_set_id: NettingSetId, ir_delta: f64, mtm: Money) -> Self {
            Self {
                id: id.to_string(),
                netting_set_id,
                attributes: Attributes::default(),
                ir_delta,
                mtm,
                margin_spec: None,
                im_exposure_base: None,
            }
        }

        fn with_margin_spec(mut self, margin_spec: OtcMarginSpec) -> Self {
            self.margin_spec = Some(margin_spec);
            self
        }

        fn with_im_exposure_base(mut self, im_exposure_base: Money) -> Self {
            self.im_exposure_base = Some(im_exposure_base);
            self
        }
    }

    finstack_valuations::impl_empty_cashflow_provider!(
        TestMarginableInstrument,
        finstack_cashflows::builder::CashflowRepresentation::NoResidual
    );

    impl Instrument for TestMarginableInstrument {
        fn id(&self) -> &str {
            &self.id
        }

        fn key(&self) -> InstrumentType {
            InstrumentType::IRS
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }

        fn attributes(&self) -> &Attributes {
            &self.attributes
        }

        fn attributes_mut(&mut self) -> &mut Attributes {
            &mut self.attributes
        }

        fn clone_box(&self) -> Box<dyn Instrument> {
            Box::new(self.clone())
        }

        fn base_value(
            &self,
            _market: &MarketContext,
            _as_of: Date,
        ) -> finstack_core::Result<Money> {
            Ok(self.mtm)
        }

        fn market_dependencies(&self) -> finstack_core::Result<MarketDependencies> {
            Ok(MarketDependencies::new())
        }

        fn as_marginable(&self) -> Option<&dyn Marginable> {
            Some(self)
        }
    }

    impl Marginable for TestMarginableInstrument {
        fn id(&self) -> &str {
            &self.id
        }

        fn margin_spec(&self) -> Option<&OtcMarginSpec> {
            self.margin_spec.as_ref()
        }

        fn netting_set_id(&self) -> Option<NettingSetId> {
            Some(self.netting_set_id.clone())
        }

        fn simm_sensitivities(
            &self,
            _market: &MarketContext,
            _as_of: Date,
        ) -> finstack_core::Result<SimmSensitivities> {
            let mut sensitivities = SimmSensitivities::new(self.mtm.currency());
            sensitivities.add_ir_delta(self.mtm.currency(), "5y", self.ir_delta);
            Ok(sensitivities)
        }

        fn mtm_for_vm(
            &self,
            _market: &MarketContext,
            _as_of: Date,
        ) -> finstack_core::Result<Money> {
            Ok(self.mtm)
        }

        fn im_exposure_base(
            &self,
            _market: &MarketContext,
            _as_of: Date,
        ) -> finstack_core::Result<Option<Money>> {
            Ok(self.im_exposure_base)
        }
    }

    #[test]
    fn test_aggregator_creation() {
        let aggregator = PortfolioMarginAggregator::new(Currency::USD);
        assert_eq!(aggregator.netting_set_count(), 0);
    }

    #[test]
    fn mo17_cross_currency_fx_delta_rebase_fails_fast() {
        let aggregator = PortfolioMarginAggregator::new(Currency::USD);
        let mut sensitivities = SimmSensitivities::new(Currency::EUR);
        sensitivities.fx_delta.insert(Currency::USD, 1_000.0);

        let err = aggregator
            .convert_sensitivities_to_base(
                sensitivities,
                &MarketContext::new(),
                date!(2024 - 01 - 01),
                &PositionId::new("pos-fx"),
            )
            .expect_err("MO-17: FX-delta calc-currency rebase must not silently relabel");
        assert!(err.to_string().contains("MO-17"), "unexpected error: {err}");
    }

    #[test]
    fn test_b2_repeated_calculate_does_not_accumulate_simm_sensitivities() {
        let as_of = date!(2024 - 01 - 01);
        let netting_set_id = NettingSetId::bilateral("BANK", "CSA");
        let instrument = Arc::new(TestMarginableInstrument::new(
            "irs-1",
            netting_set_id,
            1_000_000.0,
            Money::new(0.0, Currency::USD),
        ));
        let position = Position::new(
            "pos-1",
            DUMMY_ENTITY_ID,
            "irs-1",
            instrument,
            1.0,
            PositionUnit::Units,
        )
        .expect("position should build");
        let portfolio = Portfolio::builder("portfolio")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new(DUMMY_ENTITY_ID))
            .position(position)
            .build()
            .expect("portfolio should build");
        let mut aggregator = PortfolioMarginAggregator::from_portfolio(&portfolio);

        let first = aggregator
            .calculate(&portfolio, &MarketContext::new(), as_of)
            .expect("first margin run should succeed");
        let second = aggregator
            .calculate(&portfolio, &MarketContext::new(), as_of)
            .expect("second margin run should succeed");

        assert_eq!(first.netting_set_count(), 1);
        assert_eq!(second.netting_set_count(), 1);
        assert!(
            (first.total_initial_margin.amount() - second.total_initial_margin.amount()).abs()
                < 1e-9,
            "B-2 repeated calculate should be idempotent, first IM {}, second IM {}",
            first.total_initial_margin.amount(),
            second.total_initial_margin.amount()
        );
    }

    #[test]
    fn minor18_calculate_degrades_notional_currency_mismatch_for_vm() {
        let as_of = date!(2024 - 01 - 01);
        let netting_set_id = NettingSetId::bilateral("BANK", "CSA");
        let instrument = Arc::new(TestMarginableInstrument::new(
            "irs-1",
            netting_set_id,
            0.0,
            Money::new(100.0, Currency::USD),
        ));
        let position = Position::new(
            "pos-1",
            DUMMY_ENTITY_ID,
            "irs-1",
            instrument,
            1.0,
            PositionUnit::Notional(Some(Currency::EUR)),
        )
        .expect("position should build");
        let portfolio = Portfolio::builder("portfolio")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new(DUMMY_ENTITY_ID))
            .position(position)
            .build()
            .expect("portfolio should build");
        let mut aggregator = PortfolioMarginAggregator::from_portfolio(&portfolio);

        let result = aggregator
            .calculate(&portfolio, &MarketContext::new(), as_of)
            .expect("minor 18: portfolio-level margin should degrade the bad position");
        assert!(
            result
                .degraded_positions
                .iter()
                .any(|(id, reason)| id.as_str() == "pos-1" && reason.contains("notional currency")),
            "minor 18: expected degraded notional-currency mismatch, got {:?}",
            result.degraded_positions
        );
        assert_eq!(result.total_variation_margin.amount(), 0.0);
    }

    #[test]
    fn m13_calculate_applies_csa_vm_threshold_and_mta() {
        let as_of = date!(2024 - 01 - 01);
        let netting_set_id = NettingSetId::bilateral("BANK", "CSA");
        let mut csa = CsaSpec::usd_regulatory().expect("registry should load");
        csa.vm_params = VmParameters::with_threshold(
            Money::new(1_000_000.0, Currency::USD),
            Money::new(100_000.0, Currency::USD),
        );
        let margin_spec = OtcMarginSpec::bilateral_simm(csa);
        let instrument = Arc::new(
            TestMarginableInstrument::new(
                "irs-1",
                netting_set_id,
                0.0,
                Money::new(1_200_000.0, Currency::USD),
            )
            .with_margin_spec(margin_spec),
        );
        let position = Position::new(
            "pos-1",
            DUMMY_ENTITY_ID,
            "irs-1",
            instrument,
            1.0,
            PositionUnit::Units,
        )
        .expect("position should build");
        let portfolio = Portfolio::builder("portfolio")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new(DUMMY_ENTITY_ID))
            .position(position)
            .build()
            .expect("portfolio should build");
        let mut aggregator = PortfolioMarginAggregator::from_portfolio(&portfolio);

        let result = aggregator
            .calculate(&portfolio, &MarketContext::new(), as_of)
            .expect("M-13: CSA VM terms should calculate");

        assert_eq!(
            result.total_variation_margin.amount(),
            200_000.0,
            "M-13: VM should be raw MTM less threshold when above MTA"
        );
    }

    #[test]
    fn m10_cleared_netting_set_uses_clearing_house_im_calculator() {
        let as_of = date!(2024 - 01 - 01);
        let netting_set_id = NettingSetId::cleared("LCH");
        let exposure_base = Money::new(10_000_000.0, Currency::USD);
        let expected_im = ClearingHouseImCalculator::for_ccp("LCH")
            .calculate_conservative(exposure_base)
            .amount();
        let instrument = Arc::new(
            TestMarginableInstrument::new(
                "irs-1",
                netting_set_id,
                9_999_999.0,
                Money::new(0.0, Currency::USD),
            )
            .with_im_exposure_base(exposure_base),
        );
        let position = Position::new(
            "pos-1",
            DUMMY_ENTITY_ID,
            "irs-1",
            instrument,
            1.0,
            PositionUnit::Units,
        )
        .expect("position should build");
        let portfolio = Portfolio::builder("portfolio")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new(DUMMY_ENTITY_ID))
            .position(position)
            .build()
            .expect("portfolio should build");
        let mut aggregator = PortfolioMarginAggregator::from_portfolio(&portfolio);

        let result = aggregator
            .calculate(&portfolio, &MarketContext::new(), as_of)
            .expect("M-10: cleared IM should calculate from CCP exposure base");
        let netting_set = result
            .by_netting_set
            .get(&NettingSetId::cleared("LCH"))
            .expect("M-10: cleared netting set should be present");

        assert_eq!(netting_set.im_methodology, ImMethodology::ClearingHouse);
        assert!(
            (netting_set.initial_margin.amount() - expected_im).abs() < 1e-9,
            "M-10: cleared IM should use CCP calculator, expected {expected_im}, got {}",
            netting_set.initial_margin.amount()
        );
    }

    #[test]
    fn mo15_stale_tracked_position_is_reported_once() {
        let as_of = date!(2024 - 01 - 01);
        let netting_set_id = NettingSetId::bilateral("BANK", "CSA");
        let instrument = Arc::new(TestMarginableInstrument::new(
            "irs-1",
            netting_set_id,
            1_000_000.0,
            Money::new(0.0, Currency::USD),
        ));
        let position = Position::new(
            "pos-1",
            DUMMY_ENTITY_ID,
            "irs-1",
            instrument,
            1.0,
            PositionUnit::Units,
        )
        .expect("position should build");
        let original_portfolio = Portfolio::builder("portfolio")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new(DUMMY_ENTITY_ID))
            .position(position)
            .build()
            .expect("portfolio should build");
        let empty_portfolio = Portfolio::builder("portfolio")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new(DUMMY_ENTITY_ID))
            .build()
            .expect("empty portfolio should build");
        let mut aggregator = PortfolioMarginAggregator::from_portfolio(&original_portfolio);

        let result = aggregator
            .calculate(&empty_portfolio, &MarketContext::new(), as_of)
            .expect("MO-15: stale registration should degrade, not fail portfolio margin");

        assert_eq!(
            result
                .degraded_positions
                .iter()
                .filter(|(id, _)| id.as_str() == "pos-1")
                .count(),
            1,
            "MO-15/MO-14: stale tracked position should be reported once"
        );
        assert_eq!(result.positions_without_margin, 1);
        assert_eq!(result.truly_non_marginable_count(), 0);
    }
}
