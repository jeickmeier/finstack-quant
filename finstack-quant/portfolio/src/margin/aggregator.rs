//! Portfolio margin aggregation.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::HashMap;

use finstack_quant_margin::{
    ClearingHouseImCalculator, ImCalculator, ImMethodology, NettingSetId, ScheduleAssetClass,
    ScheduleImCalculator, SimmCalculator, SimmSensitivities, VmCalculator,
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
    ///
    /// # Errors
    ///
    /// Returns a validation error if positions in one netting set carry
    /// conflicting margin specifications.
    pub fn from_portfolio(portfolio: &Portfolio) -> Result<Self> {
        let mut aggregator = Self::new(portfolio.base_currency);

        for position in &portfolio.positions {
            aggregator.add_position(position)?;
        }

        let mut csa_owners = HashMap::default();
        for ns in aggregator.netting_sets.values() {
            if let Some(spec) = &ns.margin_spec {
                if let Some((owner, existing)) =
                    csa_owners.insert(spec.csa.id.clone(), (ns.id.counterparty_id(), &spec.csa))
                {
                    if owner != ns.id.counterparty_id() || existing != &spec.csa {
                        return Err(Error::validation(format!(
                            "CSA '{}' has conflicting terms or different counterparties",
                            spec.csa.id
                        )));
                    }
                }
            }
        }
        Ok(aggregator)
    }

    /// Add a position to the aggregator.
    ///
    /// The position will be assigned to its appropriate netting set
    /// based on its margin specification.
    ///
    /// # Arguments
    ///
    /// * `position` - Position to inspect and register.
    pub(crate) fn add_position(&mut self, position: &Position) -> Result<()> {
        let Some(marginable) = position.instrument.as_marginable() else {
            return Ok(());
        };
        let netting_set_id = marginable.netting_set_id();
        let margin_spec = marginable.margin_spec().cloned();
        if let Some(spec) = &margin_spec {
            spec.validate()?;
        }

        if let Some(ns_id) = netting_set_id {
            let netting_set = self
                .netting_sets
                .entry(ns_id.clone())
                .or_insert_with(|| NettingSet::new(ns_id.clone()));
            if let Some(spec) = margin_spec {
                if let Some(existing) = &netting_set.margin_spec {
                    if existing.csa != spec.csa
                        || existing.clearing_status != spec.clearing_status
                        || existing.im_methodology != spec.im_methodology
                        || existing.vm_frequency != spec.vm_frequency
                        || existing.settlement_lag != spec.settlement_lag
                    {
                        return Err(Error::validation(format!(
                            "Conflicting margin specifications for netting set '{}' at position '{}' (registered positions: {:?})",
                            netting_set.id, position.position_id, netting_set.positions
                        )));
                    }
                } else {
                    netting_set.margin_spec = Some(spec);
                }
            }
            netting_set.positions.push(position.position_id.clone());
            self.positions.push((position.position_id.clone(), ns_id));
        }
        Ok(())
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
    /// * `current_im_collateral` - One-way IM balances keyed by CSA ID in that
    ///   CSA's currency. An absent entry means zero; unknown IDs are rejected.
    ///   Excludes VM and the opposite party's segregated IM account.
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
        current_im_collateral: &HashMap<String, Money>,
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

        self.apply_csa_im_terms(&mut result, current_im_collateral, market, as_of)?;
        Ok(result)
    }

    fn apply_csa_im_terms(
        &self,
        result: &mut PortfolioMarginResult,
        current: &HashMap<String, Money>,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<()> {
        let mut gross_by_csa = HashMap::default();
        for ns in self.netting_sets.values() {
            let Some(spec) = &ns.margin_spec else {
                continue;
            };
            if spec.csa.im_params.is_none() {
                continue;
            }
            let gross = result
                .by_netting_set
                .get(&ns.id)
                .ok_or_else(|| Error::validation("Missing netting-set IM result"))?
                .initial_margin;
            let amount =
                crate::fx::convert_to_base(gross, as_of, market, spec.csa.base_currency)?.amount();
            let entry = gross_by_csa
                .entry(spec.csa.id.clone())
                .or_insert((&spec.csa, 0.0));
            entry.1 += amount;
        }
        for id in current.keys() {
            if !gross_by_csa.contains_key(id) {
                return Err(Error::validation(format!(
                    "Unknown IM collateral CSA '{id}'"
                )));
            }
        }
        for (id, (csa, gross)) in gross_by_csa {
            let params = csa
                .im_params
                .as_ref()
                .ok_or_else(|| Error::validation("Missing IM terms"))?;
            let currency = csa.base_currency;
            let account = params.apply_collateral_terms(
                Money::new(gross, currency)?,
                current
                    .get(&id)
                    .copied()
                    .unwrap_or(Money::from((0_i64, currency))),
            )?;
            let required = self.convert_to_base(account.required_collateral, market, as_of)?;
            let transfer = self.convert_to_base(account.transfer, market, as_of)?;
            result.total_required_im_collateral = Money::new(
                result.total_required_im_collateral.amount() + required,
                self.base_currency,
            )?;
            result.total_im_transfer = Money::new(
                result.total_im_transfer.amount() + transfer,
                self.base_currency,
            )?;
            if account.segregated {
                result.total_segregated_im = Money::new(
                    result.total_segregated_im.amount() + required,
                    self.base_currency,
                )?;
            }
            result.by_csa.insert(id, account);
        }
        Ok(())
    }

    /// Calculate sensitivities for a single position.
    fn calculate_position_sensitivities(
        &self,
        position: &Position,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<SimmSensitivities> {
        if let Some(marginable) = position.instrument.as_marginable() {
            if let Some(spec) = marginable.margin_spec() {
                if spec.im_methodology != ImMethodology::Simm || spec.csa.im_params.is_none() {
                    return Ok(SimmSensitivities::new(self.base_currency));
                }
            }
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
        // set with no `margin_spec` has no CSA to apply, so negative gross MTM is
        // reported as desk outflow — that is a degradation, not a result, and it is
        // recorded rather than passed off silently. Repo netting sets reach
        // this path: they carry a `RepoMarginSpec` that the aggregator does
        // not yet consume, so their haircut terms are not reflected in VM.
        if netting_set.margin_spec.is_none() && !netting_set.is_cleared() {
            for pos_id in &netting_set.positions {
                degraded_positions.push((
                    pos_id.clone(),
                    format!(
                        "MO-16: netting set '{}' has no margin_spec; variation margin is the \
                         negative gross MTM with no threshold, MTA or haircut applied",
                        netting_set.id
                    ),
                ));
            }
        }
        let vm = self.apply_vm_terms(
            netting_set,
            Money::new(total_mtm, self.base_currency)?,
            as_of,
            market,
        )?;

        let method = netting_set
            .margin_spec
            .as_ref()
            .map(|s| s.im_methodology)
            .unwrap_or(if netting_set.is_cleared() {
                ImMethodology::ClearingHouse
            } else {
                ImMethodology::Simm
            });
        let im_params = netting_set
            .margin_spec
            .as_ref()
            .and_then(|s| s.csa.im_params.as_ref());
        let no_im = netting_set.margin_spec.is_some() && im_params.is_none();
        let (im, simm_breakdown) = if no_im {
            (Money::from((0_i64, self.base_currency)), None)
        } else {
            match method {
                ImMethodology::Simm => {
                    if let Some(ref sensitivities) = netting_set.aggregated_sensitivities {
                        let days = im_params.map(|p| p.mpor_days).unwrap_or(self.simm_calculator.mpor_days());
                        let calculator = self.simm_calculator.clone().with_mpor(days);
                        let (total, breakdown) = calculator.calculate_from_sensitivities_parts(sensitivities, self.base_currency)?;
                        (Money::new(total, self.base_currency)?, Some((sensitivities.clone(), breakdown)))
                    } else { (Money::from((0_i64, self.base_currency)), None) }
                }
                ImMethodology::Schedule => (self.calculate_schedule_im(netting_set, portfolio, market, as_of)?, None),
                ImMethodology::ClearingHouse => (self.calculate_clearing_im(netting_set, portfolio, market, as_of, &mut degraded_positions)?, None),
                other => return Err(Error::validation(format!(
                    "Portfolio OTC margin does not support configured IM methodology {other:?}; supply its dedicated collateral/model engine"
                ))),
            }
        };
        let im_methodology = method;

        let mut result = NettingSetMargin::new(
            netting_set.id.clone(),
            as_of,
            im,
            vm,
            position_count,
            im_methodology,
        )?;

        result.csa_id = netting_set.margin_spec.as_ref().map(|s| s.csa.id.clone());
        result.is_approximate = netting_set.is_cleared() || simm_breakdown.is_some();

        if let Some((sensitivities, breakdown)) = simm_breakdown {
            result = result.with_simm_breakdown(sensitivities, breakdown);
        }

        Ok((result, degraded_positions))
    }

    fn calculate_schedule_im(
        &self,
        netting_set: &NettingSet,
        portfolio: &Portfolio,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        use finstack_quant_valuations::pricer::InstrumentType;
        let calculator = ScheduleImCalculator::bcbs_standard()?;
        let mut inputs = Vec::new();
        for id in &netting_set.positions {
            let position = portfolio
                .get_position(id.as_str())
                .ok_or_else(|| Error::validation(format!("Missing schedule position '{id}'")))?;
            let instrument = &position.instrument;
            let marginable = instrument
                .as_marginable()
                .ok_or_else(|| Error::validation("Schedule position is not marginable"))?;
            let exposure = marginable.im_exposure_base(market, as_of)?.ok_or_else(|| {
                Error::validation(format!(
                    "Schedule position '{id}' requires regulatory notional"
                ))
            })?;
            let expiry = instrument.expiry().ok_or_else(|| {
                Error::validation(format!(
                    "Schedule position '{id}' requires contractual expiry"
                ))
            })?;
            if expiry <= as_of {
                continue;
            }
            let years = (expiry - as_of).whole_days() as f64 / 365.0;
            let class = match instrument.key() {
                InstrumentType::Irs | InstrumentType::FiIndexTotalReturnSwap => {
                    ScheduleAssetClass::InterestRate
                }
                InstrumentType::Cds | InstrumentType::CdsIndex => ScheduleAssetClass::Credit,
                InstrumentType::EquityTotalReturnSwap => ScheduleAssetClass::Equity,
                other => {
                    return Err(Error::validation(format!(
                        "No approved schedule classification for {other:?}"
                    )))
                }
            };
            let notional = exposure.checked_mul_f64(position.scale_factor().abs())?;
            let notional = Money::new(
                self.convert_to_base(notional, market, as_of)?,
                self.base_currency,
            )?;
            let mtm = self.get_position_mtm(position, market, as_of)?;
            let mtm = Money::new(
                self.convert_to_base(mtm, market, as_of)?,
                self.base_currency,
            )?;
            inputs.push((mtm, notional, class, years));
        }
        let amount = calculator
            .calculate_netting_set_with_ngr(&inputs, as_of)?
            .map(|r| r.amount.amount())
            .unwrap_or(0.0);
        let days = netting_set
            .margin_spec
            .as_ref()
            .and_then(|s| s.csa.im_params.as_ref())
            .map(|p| p.mpor_days)
            .unwrap_or(calculator.mpor_days);
        Ok(Money::new(
            amount * (f64::from(days) / f64::from(calculator.mpor_days)).sqrt(),
            self.base_currency,
        )?)
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
                    // This conservative proxy grants no portfolio offsets. A short
                    // requires collateral just as a long does; signed sensitivity
                    // netting belongs to the SIMM path, not standalone IM amounts.
                    let days = netting_set
                        .margin_spec
                        .as_ref()
                        .and_then(|s| s.csa.im_params.as_ref())
                        .map(|p| p.mpor_days)
                        .unwrap_or(im_result.mpor_days);
                    if im_result.mpor_days == 0 {
                        return Err(Error::validation(
                            "Clearing IM source MPOR must be positive",
                        ));
                    }
                    let scaled_im = im_result.amount.checked_mul_f64(
                        position.scale_factor().abs()
                            * (f64::from(days) / f64::from(im_result.mpor_days)).sqrt(),
                    )?;
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
        market: &MarketContext,
    ) -> Result<Money> {
        // No CSA on this netting set: nothing to net against. The caller
        // records the degradation so the unadjusted figure is not mistaken
        // for a CSA-netted call amount.
        let Some(spec) = &netting_set.margin_spec else {
            return Ok(Money::new(-gross_vm.amount(), self.base_currency)?);
        };

        let gross_vm = crate::fx::convert_to_base(gross_vm, as_of, market, spec.csa.base_currency)?;
        let current_collateral = Money::from((0_i64, spec.csa.base_currency));
        let vm_result = VmCalculator::new(spec.csa.clone())
            .calculate(gross_vm, current_collateral, as_of)
            .map_err(|e| Error::validation(format!("M-13: CSA VM calculation failed: {e}")))?;
        crate::fx::convert_to_base(vm_result.net_margin(), as_of, market, self.base_currency)
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

        fn expiry(&self) -> Option<Date> {
            Some(date!(2025 - 01 - 01))
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
            sensitivities.add_ir_delta(self.mtm.currency(), "5Y", self.ir_delta);
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
    fn production_csa_threshold_is_applied_once_with_mpor_and_separate_custody() {
        let as_of = date!(2024 - 01 - 01);
        for (mta, current, segregated, expected_transfer) in [
            (1500.0, 13000.0, true, 0.0),
            (1000.0, 13000.0, true, 1000.0),
            (1000.0, 16000.0, false, -2000.0),
        ] {
            let mut spec = OtcMarginSpec::usd_bilateral().unwrap();
            spec.csa.id = "shared-csa".into();
            let params = spec.csa.im_params.as_mut().unwrap();
            params.mpor_days = 40;
            params.threshold = Money::new(10000.0, Currency::USD).unwrap();
            params.mta = Money::new(mta, Currency::USD).unwrap();
            params.segregated = segregated;
            let mut builder = Portfolio::builder("two-netting-sets")
                .base_currency(Currency::USD)
                .as_of(as_of)
                .entity(Entity::new(DUMMY_ENTITY_ID));
            for n in ["a", "b"] {
                let instrument = Arc::new(
                    TestMarginableInstrument::new(
                        n,
                        NettingSetId::bilateral("BANK", n),
                        100.0,
                        Money::from((0_i64, Currency::USD)),
                    )
                    .with_margin_spec(spec.clone()),
                );
                builder = builder.position(
                    Position::new(n, DUMMY_ENTITY_ID, n, instrument, 1.0, PositionUnit::Units)
                        .unwrap(),
                );
            }
            let portfolio = builder.build().unwrap();
            let current = HashMap::from_iter([(
                "shared-csa".to_owned(),
                Money::new(current, Currency::USD).unwrap(),
            )]);
            let result = PortfolioMarginAggregator::from_portfolio(&portfolio)
                .unwrap()
                .calculate(&portfolio, &MarketContext::new(), as_of, &current)
                .unwrap();
            // USD 5Y RW=60, no concentration, sqrt(40/10)=2 on each NS.
            assert_eq!(result.total_initial_margin.amount(), 24000.0);
            assert_eq!(result.by_csa.len(), 1);
            assert_eq!(result.total_required_im_collateral.amount(), 14000.0);
            assert_eq!(result.total_im_transfer.amount(), expected_transfer);
            assert_eq!(
                result.total_segregated_im.amount(),
                if segregated { 14000.0 } else { 0.0 }
            );
            let json = serde_json::to_string(&result).unwrap();
            let restored: PortfolioMarginResult = serde_json::from_str(&json).unwrap();
            assert_eq!(restored.total_im_transfer, result.total_im_transfer);
            assert_eq!(restored.by_csa, result.by_csa);
        }
    }

    #[test]
    fn production_schedule_dispatches_and_csa_methodology_conflicts_fail() {
        let as_of = date!(2024 - 01 - 01);
        let mut spec = OtcMarginSpec::bilateral_schedule(
            finstack_quant_margin::CsaSpec::usd_regulatory().unwrap(),
        );
        spec.csa.im_params.as_mut().unwrap().methodology = ImMethodology::Schedule;
        let instrument = Arc::new(
            TestMarginableInstrument::new(
                "schedule",
                NettingSetId::bilateral("BANK", "CSA"),
                1e12,
                Money::from((100_i64, Currency::USD)),
            )
            .with_margin_spec(spec.clone())
            .with_im_exposure_base(Money::from((1_000_000_i64, Currency::USD))),
        );
        let portfolio = Portfolio::builder("schedule")
            .base_currency(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new(DUMMY_ENTITY_ID))
            .position(
                Position::new(
                    "p",
                    DUMMY_ENTITY_ID,
                    "schedule",
                    instrument,
                    1.0,
                    PositionUnit::Units,
                )
                .unwrap(),
            )
            .build()
            .unwrap();
        let result = PortfolioMarginAggregator::from_portfolio(&portfolio)
            .unwrap()
            .calculate(
                &portfolio,
                &MarketContext::new(),
                as_of,
                &finstack_quant_core::HashMap::default(),
            )
            .unwrap();
        assert_eq!(
            result
                .by_netting_set
                .values()
                .next()
                .unwrap()
                .im_methodology,
            ImMethodology::Schedule
        );
        // One-year IRS grid is 1%, NGR = 1. Huge irrelevant SIMM input has no effect.
        assert_eq!(result.total_initial_margin.amount(), 10_000.0);
        spec.im_methodology = ImMethodology::Simm;
        let instrument = Arc::new(
            TestMarginableInstrument::new(
                "conflict",
                NettingSetId::bilateral("BANK", "CSA"),
                0.0,
                Money::from((0_i64, Currency::USD)),
            )
            .with_margin_spec(spec),
        );
        let portfolio = Portfolio::builder("conflict")
            .base_currency(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new(DUMMY_ENTITY_ID))
            .position(
                Position::new(
                    "p",
                    DUMMY_ENTITY_ID,
                    "conflict",
                    instrument,
                    1.0,
                    PositionUnit::Units,
                )
                .unwrap(),
            )
            .build()
            .unwrap();
        assert!(PortfolioMarginAggregator::from_portfolio(&portfolio).is_err());
    }

    #[test]
    fn conflicting_csa_terms_are_rejected_in_either_order() {
        let mut first = OtcMarginSpec::usd_bilateral().expect("CSA");
        first.settlement_lag = 1;
        let mut second = first.clone();
        second.settlement_lag = 2;
        for specs in [[first.clone(), second.clone()], [second, first]] {
            let mut builder = Portfolio::builder("CONFLICT")
                .base_currency(Currency::USD)
                .as_of(date!(2024 - 01 - 01))
                .entity(Entity::new(DUMMY_ENTITY_ID));
            for (i, spec) in specs.into_iter().enumerate() {
                let id = format!("position-{i}");
                let instrument = Arc::new(
                    TestMarginableInstrument::new(
                        &id,
                        NettingSetId::bilateral("BANK", "CSA"),
                        0.0,
                        Money::from((0_i64, Currency::USD)),
                    )
                    .with_margin_spec(spec),
                );
                builder = builder.position(
                    Position::new(
                        id.as_str(),
                        DUMMY_ENTITY_ID,
                        &id,
                        instrument,
                        1.0,
                        PositionUnit::Units,
                    )
                    .unwrap(),
                );
            }
            let error = PortfolioMarginAggregator::from_portfolio(&builder.build().unwrap())
                .expect_err("conflicting CSA");
            assert!(error
                .to_string()
                .contains("Conflicting margin specifications"));
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
        let mut aggregator =
            PortfolioMarginAggregator::from_portfolio(&portfolio).expect("consistent margin terms");

        let first = aggregator
            .calculate(
                &portfolio,
                &MarketContext::new(),
                as_of,
                &finstack_quant_core::HashMap::default(),
            )
            .expect("first margin run should succeed");
        let second = aggregator
            .calculate(
                &portfolio,
                &MarketContext::new(),
                as_of,
                &finstack_quant_core::HashMap::default(),
            )
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
        let mut aggregator =
            PortfolioMarginAggregator::from_portfolio(&portfolio).expect("consistent margin terms");

        let result = aggregator
            .calculate(
                &portfolio,
                &MarketContext::new(),
                as_of,
                &finstack_quant_core::HashMap::default(),
            )
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
        let mut aggregator =
            PortfolioMarginAggregator::from_portfolio(&portfolio).expect("consistent margin terms");

        let result = aggregator
            .calculate(
                &portfolio,
                &MarketContext::new(),
                as_of,
                &finstack_quant_core::HashMap::default(),
            )
            .expect("M-13: CSA VM terms should calculate");

        assert_eq!(
            result.total_variation_margin.amount(),
            -200_000.0,
            "M-13: positive exposure above threshold is a desk collection"
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
        let mut aggregator =
            PortfolioMarginAggregator::from_portfolio(&portfolio).expect("consistent margin terms");

        let result = aggregator
            .calculate(
                &portfolio,
                &MarketContext::new(),
                as_of,
                &finstack_quant_core::HashMap::default(),
            )
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
        let mut aggregator = PortfolioMarginAggregator::from_portfolio(&original_portfolio)
            .expect("consistent margin terms");

        let result = aggregator
            .calculate(
                &empty_portfolio,
                &MarketContext::new(),
                as_of,
                &HashMap::default(),
            )
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
