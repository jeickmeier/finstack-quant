//! Portfolio margin aggregation.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::{FxHashMap, FxHashSet};

use finstack_quant_margin::calculators::im::schedule::{ScheduleAssetClass, ScheduleImCalculator};
use finstack_quant_margin::calculators::im::{ClearingHouseImCalculator, SimmCalculator};
use finstack_quant_margin::calculators::{ImCalculator, VmCalculator};
use finstack_quant_margin::types::{ImMethodology, NettingSetId, SimmSensitivities};

use crate::margin::netting_set::NettingSet;
use crate::margin::results::{NettingSetMargin, PortfolioMarginResult};
use crate::position::{Position, PositionUnit};
use crate::types::PositionId;
use crate::Portfolio;
use crate::{Error, Result};

/// Calculate portfolio margin requirements by netting set.
///
/// Groups the portfolio's marginable positions into netting sets (by each
/// instrument's netting-set ID and margin specification), extracts per-position
/// SIMM sensitivities (in parallel on native targets, merged in portfolio
/// order), and computes per-netting-set initial and variation margin plus the
/// one-way CSA IM collateral terms, all reported in the portfolio base
/// currency.
///
/// # Arguments
///
/// * `portfolio` - Portfolio whose marginable positions are grouped and
///   valued; its `base_currency` is the reporting currency.
/// * `market` - Market context required for VM, SIMM sensitivity extraction
///   and FX conversion into the base currency.
/// * `as_of` - Valuation date for the margin run.
/// * `current_im_collateral` - One-way IM balances keyed by CSA ID in that
///   CSA's currency. An absent entry means zero; unknown IDs are rejected.
///   Excludes VM and the opposite party's segregated IM account.
///
/// # Returns
///
/// Portfolio-level margin report including per-netting-set totals, per-CSA
/// collateral accounts and degraded positions.
///
/// # Errors
///
/// Returns a validation error if positions in one netting set carry
/// conflicting margin specifications, a CSA is shared by different
/// counterparties or terms, a collateral CSA ID is unknown, or a netting set
/// uses an unsupported IM methodology; propagates FX and margin-model
/// failures needed for base-currency reporting.
///
/// # References
///
/// - `docs/REFERENCES.md#isda-simm`
pub fn portfolio_margin(
    portfolio: &Portfolio,
    market: &MarketContext,
    as_of: Date,
    current_im_collateral: &FxHashMap<String, Money>,
) -> Result<PortfolioMarginResult> {
    let run = MarginRun {
        market,
        as_of,
        base_currency: portfolio.base_currency,
        simm_calculator: SimmCalculator::new(finstack_quant_margin::SimmVersion::default())?,
    };
    let (netting_sets, members) = build_netting_sets(portfolio)?;
    let mut result = PortfolioMarginResult::new(as_of, run.base_currency);

    let map_position = |(position, ns_id): &(&Position, NettingSetId)| {
        (
            position.position_id.clone(),
            ns_id.clone(),
            run.calculate_position_sensitivities(position),
        )
    };

    #[cfg(not(target_arch = "wasm32"))]
    let position_sensitivities: Vec<(PositionId, NettingSetId, Result<SimmSensitivities>)> = {
        use rayon::prelude::*;
        members.par_iter().map(map_position).collect()
    };

    #[cfg(target_arch = "wasm32")]
    let position_sensitivities: Vec<(PositionId, NettingSetId, Result<SimmSensitivities>)> =
        members.iter().map(map_position).collect();

    let mut aggregated: FxHashMap<NettingSetId, SimmSensitivities> = FxHashMap::default();
    for (position_id, ns_id, sens_result) in position_sensitivities {
        match sens_result {
            Ok(sensitivities) => {
                if let Some(agg) = aggregated.get_mut(&ns_id) {
                    agg.merge_unchecked(&sensitivities);
                } else {
                    aggregated.insert(ns_id, sensitivities);
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

    for netting_set in netting_sets.values() {
        let (ns_margin, degraded_positions) =
            run.calculate_netting_set_margin(netting_set, aggregated.get(&netting_set.id))?;
        for (position_id, message) in degraded_positions {
            result.add_degraded_position(position_id, message);
        }
        result.add_netting_set(ns_margin)?;
    }

    let tracked: FxHashSet<&str> = members
        .iter()
        .map(|(position, _)| position.position_id.as_str())
        .collect();
    let degraded: FxHashSet<&str> = result
        .degraded_positions
        .iter()
        .map(|(id, _)| id.as_str())
        .collect();
    result.positions_without_margin = portfolio
        .positions
        .iter()
        .filter(|position| {
            !tracked.contains(position.position_id.as_str())
                || degraded.contains(position.position_id.as_str())
        })
        .count();

    run.apply_csa_im_terms(&netting_sets, &mut result, current_im_collateral)?;
    Ok(result)
}

/// Netting sets of one run, keyed by their identifier.
type NettingSets<'a> = FxHashMap<NettingSetId, NettingSet<'a>>;

/// `(position, netting set)` membership pairs in portfolio order.
type Members<'a> = Vec<(&'a Position, NettingSetId)>;

/// Group the portfolio's marginable positions into netting sets.
///
/// Returns the netting sets and the `(position, netting set)` membership pairs
/// in portfolio order; the pairs drive the parallel sensitivity fan-out and
/// fix the merge and degradation order.
fn build_netting_sets(portfolio: &Portfolio) -> Result<(NettingSets<'_>, Members<'_>)> {
    let mut netting_sets: NettingSets<'_> = FxHashMap::default();
    let mut members = Vec::new();
    for position in &portfolio.positions {
        let Some(marginable) = position.instrument.as_marginable() else {
            continue;
        };
        let netting_set_id = marginable.netting_set_id();
        let margin_spec = marginable.margin_spec().cloned();
        if let Some(spec) = &margin_spec {
            spec.validate()?;
        }

        if let Some(ns_id) = netting_set_id {
            let netting_set = netting_sets
                .entry(ns_id.clone())
                .or_insert_with(|| NettingSet::new(ns_id.clone()));
            if let Some(spec) = margin_spec {
                if let Some(existing) = &netting_set.margin_spec {
                    // Credit classification belongs to each instrument, not the shared CSA.
                    if existing.csa != spec.csa || existing.clearing_status != spec.clearing_status
                    {
                        let registered: Vec<&str> = netting_set
                            .positions
                            .iter()
                            .map(|p| p.position_id.as_str())
                            .collect();
                        return Err(Error::invalid_input(format!(
                            "Conflicting margin specifications for netting set '{}' at position '{}' (registered positions: {:?})",
                            netting_set.id, position.position_id, registered
                        )));
                    }
                } else {
                    netting_set.margin_spec = Some(spec);
                }
            }
            netting_set.positions.push(position);
            members.push((position, ns_id));
        }
    }

    let mut csa_owners = FxHashMap::default();
    for ns in netting_sets.values() {
        if let Some(spec) = &ns.margin_spec {
            if let Some((owner, existing)) =
                csa_owners.insert(spec.csa.id.clone(), (ns.id.counterparty_id(), &spec.csa))
            {
                if owner != ns.id.counterparty_id() || existing != &spec.csa {
                    return Err(Error::invalid_input(format!(
                        "CSA '{}' has conflicting terms or different counterparties",
                        spec.csa.id
                    )));
                }
            }
        }
    }
    Ok((netting_sets, members))
}

/// Rescale an IM amount from its source margin period of risk to the target
/// one by the square-root-of-time rule: `amount * sqrt(target / source)`.
fn rescale_to_mpor(amount: f64, source_days: u32, target_days: u32) -> Result<f64> {
    if source_days == 0 {
        return Err(Error::invalid_input("IM source MPOR must be positive"));
    }
    Ok(amount * (f64::from(target_days) / f64::from(source_days)).sqrt())
}

/// Inputs shared by every netting set of one margin run.
struct MarginRun<'a> {
    market: &'a MarketContext,
    as_of: Date,
    base_currency: Currency,
    simm_calculator: SimmCalculator,
}

impl MarginRun<'_> {
    fn apply_csa_im_terms(
        &self,
        netting_sets: &NettingSets<'_>,
        result: &mut PortfolioMarginResult,
        current: &FxHashMap<String, Money>,
    ) -> Result<()> {
        let mut gross_by_csa = FxHashMap::default();
        for ns in netting_sets.values() {
            let Some(spec) = &ns.margin_spec else {
                continue;
            };
            if spec.csa.im_params.is_none() {
                continue;
            }
            let gross = result
                .by_netting_set
                .get(&ns.id)
                .ok_or_else(|| Error::invalid_input("Missing netting-set IM result"))?
                .initial_margin;
            let amount =
                crate::fx::convert_to_base(gross, self.as_of, self.market, spec.csa.base_currency)?
                    .amount();
            let entry = gross_by_csa
                .entry(spec.csa.id.clone())
                .or_insert((&spec.csa, 0.0));
            entry.1 += amount;
        }
        for id in current.keys() {
            if !gross_by_csa.contains_key(id) {
                return Err(Error::invalid_input(format!(
                    "Unknown IM collateral CSA '{id}'"
                )));
            }
        }
        for (id, (csa, gross)) in gross_by_csa {
            let params = csa
                .im_params
                .as_ref()
                .ok_or_else(|| Error::invalid_input("Missing IM terms"))?;
            let currency = csa.base_currency;
            let account = params.apply_collateral_terms(
                Money::new(gross, currency)?,
                current
                    .get(&id)
                    .copied()
                    .unwrap_or(Money::from((0_i64, currency))),
            )?;
            let required = self.convert_to_base(account.required_collateral)?;
            let transfer = self.convert_to_base(account.transfer)?;
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
    fn calculate_position_sensitivities(&self, position: &Position) -> Result<SimmSensitivities> {
        if let Some(marginable) = position.instrument.as_marginable() {
            if let Some(spec) = marginable.margin_spec() {
                if spec.get_im_methodology() != Some(ImMethodology::Simm) {
                    return Ok(SimmSensitivities::new(self.base_currency));
                }
            }
            let sens = marginable
                .simm_sensitivities(self.market, self.as_of)
                .map_err(|e| {
                    Error::valuation(position.position_id.clone(), e.kind(), e.to_string())
                })?;
            // B-6: instrument sensitivities are per unit (same contract as
            // `mtm_for_vm`), so scale them to the HELD sensitivity by the signed
            // position factor before the netting-set merge. The sign matters:
            // ISDA SIMM nets signed trade-level sensitivities within a netting
            // set, so a short position must offset an equal long.
            let sens = sens.scaled(position.scale_factor());
            // FX-collapse to the portfolio base currency before the netting-set
            // merge sums raw amounts. Without this, sensitivities produced in a
            // position's own base currency are added across currencies, breaking
            // currency safety and the SIMM calculation-currency convention.
            self.convert_sensitivities_to_base(sens, &position.position_id)
        } else {
            Ok(SimmSensitivities::new(self.base_currency))
        }
    }

    /// Re-express a position's SIMM sensitivities in the portfolio base
    /// currency via an explicit spot FX conversion.
    fn convert_sensitivities_to_base(
        &self,
        sensitivities: SimmSensitivities,
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
            .convert_to_base(Money::from((1_i64, sensitivities.base_currency)))
            .map_err(|e| Error::valuation(position_id.clone(), e.kind(), e.to_string()))?;
        Ok(sensitivities.scaled_to_currency(self.base_currency, fx_rate))
    }

    /// Calculate margin for a netting set from its merged SIMM sensitivities.
    fn calculate_netting_set_margin(
        &self,
        netting_set: &NettingSet<'_>,
        sensitivities: Option<&SimmSensitivities>,
    ) -> Result<(NettingSetMargin, Vec<(PositionId, String)>)> {
        let mut total_mtm = 0.0;
        let mut position_count = 0;
        let mut degraded_positions = Vec::new();
        let is_cleared = netting_set.id.is_cleared();

        for position in &netting_set.positions {
            match self.get_position_mtm(position) {
                Ok(mtm) => {
                    let mtm_base = if mtm.currency() == self.base_currency {
                        Ok(mtm.amount())
                    } else {
                        self.convert_to_base(mtm)
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

        // VM is the net CSA call amount after threshold/MTA terms. A netting
        // set with no `margin_spec` has no CSA to apply, so negative gross MTM is
        // reported as desk outflow — that is a degradation, not a result, and it is
        // recorded rather than passed off silently. Repo netting sets reach
        // this path: they carry a `RepoMarginSpec` that portfolio margin does
        // not yet consume, so their haircut terms are not reflected in VM.
        if netting_set.margin_spec.is_none() && !is_cleared {
            for position in &netting_set.positions {
                degraded_positions.push((
                    position.position_id.clone(),
                    format!(
                        "MO-16: netting set '{}' has no margin_spec; variation margin is the \
                         negative gross MTM with no threshold, MTA or haircut applied",
                        netting_set.id
                    ),
                ));
            }
        }
        let vm = self.apply_vm_terms(netting_set, Money::new(total_mtm, self.base_currency)?)?;

        let method = netting_set
            .margin_spec
            .as_ref()
            .and_then(|s| s.get_im_methodology())
            .unwrap_or(if is_cleared {
                ImMethodology::ClearingHouse
            } else {
                ImMethodology::Simm
            });
        let im_params = netting_set
            .margin_spec
            .as_ref()
            .and_then(|s| s.csa.im_params.as_ref());
        let target_mpor = im_params.map(|p| p.mpor_days);
        let no_im = netting_set.margin_spec.is_some() && im_params.is_none();
        let (im, simm_breakdown) = if no_im {
            (Money::from((0_i64, self.base_currency)), None)
        } else {
            match method {
                ImMethodology::Simm => {
                    if let Some(sensitivities) = sensitivities {
                        let days = target_mpor.unwrap_or(self.simm_calculator.mpor_days());
                        let calculator = self.simm_calculator.clone().with_mpor(days);
                        let (total, breakdown) = calculator.calculate_from_sensitivities_parts(sensitivities, self.base_currency)?;
                        (Money::new(total, self.base_currency)?, Some((sensitivities.clone(), breakdown)))
                    } else { (Money::from((0_i64, self.base_currency)), None) }
                }
                ImMethodology::Schedule => (self.calculate_schedule_im(netting_set, target_mpor)?, None),
                ImMethodology::ClearingHouse => (self.calculate_clearing_im(netting_set, target_mpor, &mut degraded_positions)?, None),
                other => return Err(Error::invalid_input(format!(
                    "Portfolio OTC margin does not support configured IM methodology {other:?}; supply its dedicated collateral/model engine"
                ))),
            }
        };
        let im_methodology = method;

        let mut result = NettingSetMargin::new(
            netting_set.id.clone(),
            self.as_of,
            im,
            vm,
            position_count,
            im_methodology,
        )?;

        result.csa_id = netting_set.margin_spec.as_ref().map(|s| s.csa.id.clone());
        result.is_approximate = is_cleared || simm_breakdown.is_some();

        if let Some((sensitivities, breakdown)) = simm_breakdown {
            result = result.with_simm_breakdown(sensitivities, breakdown);
        }

        Ok((result, degraded_positions))
    }

    /// Schedule IM for a netting set, rescaled from the schedule's MPOR to
    /// `target_mpor` (the CSA's, when it has IM terms).
    fn calculate_schedule_im(
        &self,
        netting_set: &NettingSet<'_>,
        target_mpor: Option<u32>,
    ) -> Result<Money> {
        use finstack_quant_valuations::pricer::InstrumentType;
        let calculator = ScheduleImCalculator::bcbs_standard()?;
        let mut inputs = Vec::new();
        for position in &netting_set.positions {
            let id = &position.position_id;
            let instrument = &position.instrument;
            let marginable = instrument
                .as_marginable()
                .ok_or_else(|| Error::invalid_input("Schedule position is not marginable"))?;
            let exposure = marginable
                .im_exposure_base(self.market, self.as_of)?
                .ok_or_else(|| {
                    Error::invalid_input(format!(
                        "Schedule position '{id}' requires regulatory notional"
                    ))
                })?;
            let expiry = instrument.expiry().ok_or_else(|| {
                Error::invalid_input(format!(
                    "Schedule position '{id}' requires contractual expiry"
                ))
            })?;
            if expiry <= self.as_of {
                continue;
            }
            let years = (expiry - self.as_of).whole_days() as f64 / 365.0;
            let class = match instrument.key() {
                InstrumentType::Irs | InstrumentType::FiIndexTotalReturnSwap => {
                    ScheduleAssetClass::InterestRate
                }
                InstrumentType::Cds | InstrumentType::CdsIndex => ScheduleAssetClass::Credit,
                InstrumentType::EquityTotalReturnSwap => ScheduleAssetClass::Equity,
                other => {
                    return Err(Error::invalid_input(format!(
                        "No approved schedule classification for {other:?}"
                    )))
                }
            };
            let notional = exposure.checked_mul_f64(position.scale_factor().abs())?;
            let notional = Money::new(self.convert_to_base(notional)?, self.base_currency)?;
            let mtm = self.get_position_mtm(position)?;
            let mtm = Money::new(self.convert_to_base(mtm)?, self.base_currency)?;
            inputs.push((mtm, notional, class, years));
        }
        let amount = calculator
            .calculate_netting_set_with_ngr(&inputs, self.as_of)?
            .map(|r| r.amount.amount())
            .unwrap_or(0.0);
        let days = target_mpor.unwrap_or(calculator.mpor_days);
        Ok(Money::new(
            rescale_to_mpor(amount, calculator.mpor_days, days)?,
            self.base_currency,
        )?)
    }

    /// Conservative CCP IM proxy for a cleared netting set, each position
    /// rescaled from the CCP's MPOR to `target_mpor` when one is set.
    fn calculate_clearing_im(
        &self,
        netting_set: &NettingSet<'_>,
        target_mpor: Option<u32>,
        degraded_positions: &mut Vec<(PositionId, String)>,
    ) -> Result<Money> {
        let calculator = ClearingHouseImCalculator::for_ccp(netting_set.id.counterparty_id());
        let mut total = 0.0;

        for position in &netting_set.positions {
            let Some(marginable) = position.instrument.as_marginable() else {
                degraded_positions.push((
                    position.position_id.clone(),
                    "M-10: cleared position is not marginable".to_string(),
                ));
                continue;
            };

            match calculator.calculate(marginable, self.market, self.as_of) {
                Ok(im_result) => {
                    // This conservative proxy grants no portfolio offsets. A short
                    // requires collateral just as a long does; signed sensitivity
                    // netting belongs to the SIMM path, not standalone IM amounts.
                    let days = target_mpor.unwrap_or(im_result.mpor_days);
                    let scale =
                        rescale_to_mpor(position.scale_factor().abs(), im_result.mpor_days, days)?;
                    let scaled_im = im_result.amount.checked_mul_f64(scale)?;
                    let amount = if scaled_im.currency() == self.base_currency {
                        Ok(scaled_im.amount())
                    } else {
                        self.convert_to_base(scaled_im)
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

    fn apply_vm_terms(&self, netting_set: &NettingSet<'_>, gross_vm: Money) -> Result<Money> {
        // No CSA on this netting set: nothing to net against. The caller
        // records the degradation so the unadjusted figure is not mistaken
        // for a CSA-netted call amount.
        let Some(spec) = &netting_set.margin_spec else {
            return Ok(Money::new(-gross_vm.amount(), self.base_currency)?);
        };

        let gross_vm =
            crate::fx::convert_to_base(gross_vm, self.as_of, self.market, spec.csa.base_currency)?;
        let current_collateral = Money::from((0_i64, spec.csa.base_currency));
        let vm_result = VmCalculator::new(spec.csa.clone())
            .calculate(gross_vm, current_collateral, self.as_of)
            .map_err(|e| Error::invalid_input(format!("M-13: CSA VM calculation failed: {e}")))?;
        crate::fx::convert_to_base(
            vm_result.net_margin(),
            self.as_of,
            self.market,
            self.base_currency,
        )
    }

    /// Get MTM for a position in its native currency, scaled by position quantity.
    ///
    /// `mtm_for_vm` returns the instrument's deal MTM; this method applies
    /// [`Position::scale_value`] so VM reflects the lot multiplier.
    fn get_position_mtm(&self, position: &Position) -> Result<Money> {
        if let Some(marginable) = position.instrument.as_marginable() {
            let unit_mtm = marginable
                .mtm_for_vm(self.market, self.as_of)
                .map_err(|e| {
                    Error::valuation(position.position_id.clone(), e.kind(), e.to_string())
                })?;
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

    /// Spot-convert `amount` to the portfolio base currency as `f64`.
    fn convert_to_base(&self, amount: Money) -> Result<f64> {
        crate::fx::convert_to_base(amount, self.as_of, self.market, self.base_currency)
            .map(|m| m.amount())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::{Position, PositionUnit};
    use crate::types::{Entity, DUMMY_ENTITY_ID};
    use crate::Portfolio;
    use finstack_quant_core::types::Attributes;
    use finstack_quant_margin::calculators::im::ClearingHouseImCalculator;
    use finstack_quant_margin::types::{CsaSpec, OtcMarginSpec, VmParameters};
    use finstack_quant_margin::Marginable;
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
            let current = FxHashMap::from_iter([(
                "shared-csa".to_owned(),
                Money::new(current, Currency::USD).unwrap(),
            )]);
            let result =
                portfolio_margin(&portfolio, &MarketContext::new(), as_of, &current).unwrap();
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
            finstack_quant_margin::types::CsaSpec::usd_regulatory().unwrap(),
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
        let result = portfolio_margin(
            &portfolio,
            &MarketContext::new(),
            as_of,
            &finstack_quant_core::FxHashMap::default(),
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
        spec.csa.im_params.as_mut().unwrap().methodology = ImMethodology::ClearingHouse;
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
        assert!(build_netting_sets(&portfolio).is_err());
    }

    #[test]
    fn conflicting_csa_terms_are_rejected_in_either_order() {
        let mut first = OtcMarginSpec::usd_bilateral().expect("CSA");
        first.csa.vm_params.settlement_lag = 1;
        let mut second = first.clone();
        second.csa.vm_params.settlement_lag = 2;
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
            let portfolio = builder.build().unwrap();
            let error = build_netting_sets(&portfolio).expect_err("conflicting CSA");
            assert!(error
                .to_string()
                .contains("Conflicting margin specifications"));
        }
    }

    #[test]
    fn different_credit_classifications_share_one_agreement() {
        use finstack_quant_margin::types::{SimmCreditClassification, SimmCreditSector};
        let first = OtcMarginSpec::usd_bilateral()
            .expect("CSA")
            .with_simm_credit_classification(SimmCreditClassification::Qualifying {
                sector: SimmCreditSector::Financial,
            });
        let second =
            first
                .clone()
                .with_simm_credit_classification(SimmCreditClassification::Qualifying {
                    sector: SimmCreditSector::Sovereign,
                });
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
            let portfolio = builder.build().unwrap();
            let (netting_sets, _) = build_netting_sets(&portfolio)
                .expect("instrument classifications may differ within one CSA");
            assert_eq!(netting_sets.len(), 1);
        }
    }

    #[test]
    fn rescale_to_mpor_uses_square_root_of_time_and_rejects_zero_source() {
        assert_eq!(rescale_to_mpor(100.0, 10, 40).unwrap(), 200.0);
        assert_eq!(rescale_to_mpor(100.0, 10, 10).unwrap(), 100.0);
        assert!(rescale_to_mpor(100.0, 0, 10).is_err());
    }

    #[test]
    fn mo17_cross_currency_fx_delta_rebase_fails_fast() {
        let market = MarketContext::new();
        let run = MarginRun {
            market: &market,
            as_of: date!(2024 - 01 - 01),
            base_currency: Currency::USD,
            simm_calculator: SimmCalculator::new(finstack_quant_margin::SimmVersion::default())
                .expect("embedded registry"),
        };
        let mut sensitivities = SimmSensitivities::new(Currency::EUR);
        sensitivities.fx_delta.insert(Currency::USD, 1_000.0);

        let err = run
            .convert_sensitivities_to_base(sensitivities, &PositionId::new("pos-fx"))
            .expect_err("MO-17: FX-delta calc-currency rebase must not silently relabel");
        assert!(err.to_string().contains("MO-17"), "unexpected error: {err}");
    }

    #[test]
    fn test_b2_repeated_runs_do_not_accumulate_simm_sensitivities() {
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
        let run = || {
            portfolio_margin(
                &portfolio,
                &MarketContext::new(),
                as_of,
                &finstack_quant_core::FxHashMap::default(),
            )
        };
        let first = run().expect("first margin run should succeed");
        let second = run().expect("second margin run should succeed");

        assert_eq!(first.by_netting_set.len(), 1);
        assert_eq!(second.by_netting_set.len(), 1);
        assert!(
            (first.total_initial_margin.amount() - second.total_initial_margin.amount()).abs()
                < 1e-9,
            "B-2 repeated margin runs should be idempotent, first IM {}, second IM {}",
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
        let result = portfolio_margin(
            &portfolio,
            &MarketContext::new(),
            as_of,
            &finstack_quant_core::FxHashMap::default(),
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
        assert_eq!(result.total_positions, 0);
        assert_eq!(result.degraded_positions.len(), 1);
        assert_eq!(result.positions_without_margin, 1);
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
        let result = portfolio_margin(
            &portfolio,
            &MarketContext::new(),
            as_of,
            &finstack_quant_core::FxHashMap::default(),
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
            .expect("conservative im")
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
        let result = portfolio_margin(
            &portfolio,
            &MarketContext::new(),
            as_of,
            &finstack_quant_core::FxHashMap::default(),
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
}
