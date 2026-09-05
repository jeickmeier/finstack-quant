//! Portfolio margin aggregation.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::HashMap;

use finstack_quant_margin::{
    ClearingHouseImCalculator, ImCalculator, ImMethodology, NettingSetId, SimmCalculator,
    SimmSensitivities, VmCalculator,
};

use crate::margin::netting_set::NettingSet;
use crate::margin::results::{NettingSetMargin, PortfolioMarginResult};
use crate::portfolio::Portfolio;
use crate::position::{Position, PositionUnit};
use crate::types::PositionId;
use crate::{Error, Result};

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
    /// Netting sets indexed by their ID
    netting_sets: HashMap<NettingSetId, NettingSet>,
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
    pub(crate) fn new(base_currency: Currency) -> Self {
        Self {
            netting_sets: HashMap::default(),
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
        let mut aggregator = Self::new(portfolio.base_currency);

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
    pub(crate) fn add_position(&mut self, position: &Position) {
        let Some(marginable) = position.instrument.as_marginable() else {
            return;
        };
        let netting_set_id = marginable.netting_set_id();
        let margin_spec = marginable.margin_spec().cloned();

        if let Some(ns_id) = netting_set_id {
            let netting_set = self
                .netting_sets
                .entry(ns_id.clone())
                .or_insert_with(|| NettingSet::new(ns_id.clone()));
            if let Some(spec) = margin_spec {
                match &netting_set.margin_spec {
                    None => netting_set.margin_spec = Some(spec),
                    Some(existing) if existing != &spec => {
                        tracing::warn!(
                            netting_set_id = ?netting_set.id,
                            "Conflicting margin specs registered for netting set; keeping first spec"
                        );
                    }
                    Some(_) => {}
                }
            }
            netting_set.positions.push(position.position_id.clone());
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
        for netting_set in self.netting_sets.values_mut() {
            netting_set.reset_sensitivities();
        }

        let map_position = |(pos_id, ns_id): &(PositionId, NettingSetId)| {
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
        };

        #[cfg(not(target_arch = "wasm32"))]
        let position_sensitivities: Vec<(
            PositionId,
            NettingSetId,
            Result<SimmSensitivities>,
        )> = {
            use rayon::prelude::*;
            self.positions.par_iter().map(map_position).collect()
        };

        #[cfg(target_arch = "wasm32")]
        let position_sensitivities: Vec<(
            PositionId,
            NettingSetId,
            Result<SimmSensitivities>,
        )> = self.positions.iter().map(map_position).collect();

        for (position_id, ns_id, sens_result) in position_sensitivities {
            match sens_result {
                Ok(sensitivities) => {
                    if let Some(netting_set) = self.netting_sets.get_mut(&ns_id) {
                        netting_set.merge_sensitivities(&sensitivities);
                    }
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

        for netting_set in self.netting_sets.values() {
            let (ns_margin, degraded_positions) =
                self.calculate_netting_set_margin(netting_set, portfolio, market, as_of)?;
            for (position_id, message) in degraded_positions {
                result.add_degraded_position(position_id, message);
            }
            result.add_netting_set(ns_margin)?;
        }

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
            // B-6: instrument sensitivities are per unit (same contract as
            // `mtm_for_vm`), so scale them to the HELD sensitivity by the signed
            // position factor before the netting-set merge. The sign matters:
            // ISDA SIMM nets signed trade-level sensitivities within a netting
            // set, so a short position must offset an equal long.
            let sens = sens.scaled(position.scale_factor());
            // FX-collapse to the aggregator base currency before the netting-set
            // merge sums raw amounts. Without this, sensitivities produced in a
            // position's own base currency are added across currencies, breaking
            // currency safety and the SIMM calculation-currency convention.
            self.convert_sensitivities_to_base(sens, market, as_of, &position.position_id)
        } else {
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
            .convert_to_base(
                Money::from((1_i64, sensitivities.base_currency)),
                market,
                as_of,
            )
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
        let mut total_mtm = 0.0;
        let mut position_count = 0;
        let mut degraded_positions = Vec::new();

        for pos_id in &netting_set.positions {
            if let Some(position) = portfolio.get_position(pos_id.as_str()) {
                match self.get_position_mtm(position, market, as_of) {
                    Ok(mtm) => {
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

        // VM is the net CSA call amount after threshold/MTA terms. A netting
        // set with no `margin_spec` has no CSA to apply, so the gross MTM is
        // reported unchanged — that is a degradation, not a result, and it is
        // recorded rather than passed off silently. Repo netting sets reach
        // this path: they carry a `RepoMarginSpec` that the aggregator does
        // not yet consume, so their haircut terms are not reflected in VM.
        if netting_set.margin_spec.is_none() && !netting_set.is_cleared() {
            for pos_id in &netting_set.positions {
                degraded_positions.push((
                    pos_id.clone(),
                    format!(
                        "MO-16: netting set '{}' has no margin_spec; variation margin is the \
                         gross MTM with no threshold, MTA or haircut applied",
                        netting_set.id
                    ),
                ));
            }
        }
        let vm = self.apply_vm_terms(
            netting_set,
            Money::new(total_mtm, self.base_currency)?,
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
                    let (im, breakdown) = self.calculate_simm_with_breakdown(sensitivities)?;
                    (im, Some((sensitivities.clone(), breakdown)))
                } else {
                    (Money::from((0_i64, self.base_currency)), None)
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
        )?;

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
                    // B-6: the calculator prices the marginable's deal
                    // exposure (instrument already carries deal notional,
                    // matching `mtm_for_vm`), so scale the IM contribution
                    // by the signed lot multiplier. The sign is preserved:
                    // identical cleared contracts are fungible at the CCP,
                    // so an offsetting short cancels its long's IM
                    // contribution.
                    let scaled_im = position.scale_value(im_result.amount)?;
                    let amount = if scaled_im.currency() == self.base_currency {
                        Ok(scaled_im.amount())
                    } else {
                        self.convert_to_base(scaled_im, market, as_of)
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

        Ok(Money::new(total, self.base_currency)?)
    }

    fn apply_vm_terms(
        &self,
        netting_set: &NettingSet,
        gross_vm: Money,
        as_of: Date,
    ) -> Result<Money> {
        // No CSA on this netting set: nothing to net against. The caller
        // records the degradation so the unadjusted figure is not mistaken
        // for a CSA-netted call amount.
        let Some(spec) = &netting_set.margin_spec else {
            return Ok(gross_vm);
        };

        let current_collateral = Money::from((0_i64, self.base_currency));
        let vm_result = VmCalculator::new(spec.csa.clone())
            .calculate(gross_vm, current_collateral, as_of)
            .map_err(|e| Error::validation(format!("M-13: CSA VM calculation failed: {e}")))?;
        Ok(vm_result.net_margin())
    }

    /// Get MTM for a position in its native currency, scaled by position quantity.
    ///
    /// `mtm_for_vm` returns the instrument's deal MTM; this method applies
    /// [`Position::scale_value`] so VM reflects the lot multiplier.
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
            if let PositionUnit::Notional(Some(notional_currency)) = position.unit {
                if notional_currency != unit_mtm.currency() {
                    return Err(Error::invalid_input(format!(
                        "minor 18: position '{}' notional currency {} does not match VM currency {}",
                        position.position_id,
                        notional_currency,
                        unit_mtm.currency()
                    )));
                }
            }
            Ok(position.scale_value(unit_mtm)?)
        } else {
            Ok(Money::from((0_i64, self.base_currency)))
        }
    }

    /// Spot-convert `amount` to the aggregator base currency as `f64`.
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
    ) -> finstack_quant_core::Result<(Money, finstack_quant_core::HashMap<String, Money>)> {
        Ok({
            let (total_im, breakdown) = self
                .simm_calculator
                .calculate_from_sensitivities_parts(sensitivities, self.base_currency)?;
            (Money::new(total_im, self.base_currency)?, breakdown)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::{Position, PositionUnit};
    use crate::types::{Entity, DUMMY_ENTITY_ID};
    use crate::Portfolio;
    use finstack_quant_core::types::Attributes;
    use finstack_quant_margin::{
        ClearingHouseImCalculator, CsaSpec, Marginable, OtcMarginSpec, VmParameters,
    };
    use finstack_quant_valuations::instruments::{Instrument, MarketDependencies};
    use finstack_quant_valuations::pricer::InstrumentType;
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

    finstack_quant_valuations::impl_empty_cashflow_provider!(
        TestMarginableInstrument,
        finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
    );

    impl Instrument for TestMarginableInstrument {
        fn id(&self) -> &str {
            &self.id
        }

        fn key(&self) -> InstrumentType {
            InstrumentType::Irs
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
        ) -> finstack_quant_core::Result<Money> {
            Ok(self.mtm)
        }

        fn market_dependencies(&self) -> finstack_quant_core::Result<MarketDependencies> {
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
        ) -> finstack_quant_core::Result<SimmSensitivities> {
            let mut sensitivities = SimmSensitivities::new(self.mtm.currency());
            sensitivities.add_ir_delta(self.mtm.currency(), "5y", self.ir_delta);
            Ok(sensitivities)
        }

        fn mtm_for_vm(
            &self,
            _market: &MarketContext,
            _as_of: Date,
        ) -> finstack_quant_core::Result<Money> {
            Ok(self.mtm)
        }

        fn im_exposure_base(
            &self,
            _market: &MarketContext,
            _as_of: Date,
        ) -> finstack_quant_core::Result<Option<Money>> {
            Ok(self.im_exposure_base)
        }
    }

    #[test]
    fn test_aggregator_creation() {
        let aggregator = PortfolioMarginAggregator::new(Currency::USD);
        assert!(aggregator.netting_sets.is_empty());
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
            Money::from((0_i64, Currency::USD)),
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
            .base_currency(Currency::USD)
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

        assert_eq!(first.by_netting_set.len(), 1);
        assert_eq!(second.by_netting_set.len(), 1);
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
            Money::from((100_i64, Currency::USD)),
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
            .base_currency(Currency::USD)
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
            Money::from((1_000_000_i64, Currency::USD)),
            Money::from((100_000_i64, Currency::USD)),
        );
        let margin_spec = OtcMarginSpec::bilateral_simm(csa);
        let instrument = Arc::new(
            TestMarginableInstrument::new(
                "irs-1",
                netting_set_id,
                0.0,
                Money::from((1_200_000_i64, Currency::USD)),
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
            .base_currency(Currency::USD)
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
        let exposure_base = Money::from((10_000_000_i64, Currency::USD));
        let expected_im = ClearingHouseImCalculator::for_ccp("LCH")
            .calculate_conservative(exposure_base)
            .amount();
        let instrument = Arc::new(
            TestMarginableInstrument::new(
                "irs-1",
                netting_set_id,
                9_999_999.0,
                Money::from((0_i64, Currency::USD)),
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
            .base_currency(Currency::USD)
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
            Money::from((0_i64, Currency::USD)),
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
            .base_currency(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new(DUMMY_ENTITY_ID))
            .position(position)
            .build()
            .expect("portfolio should build");
        let empty_portfolio = Portfolio::builder("portfolio")
            .base_currency(Currency::USD)
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
        assert_eq!(
            result.positions_without_margin,
            result.degraded_positions.len()
        );
    }
}
