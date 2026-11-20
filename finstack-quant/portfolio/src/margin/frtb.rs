//! Book-wide mapping onto the qualified ordinary GIRR and single-name CDS CSR grids.

use super::{
    CsrQuoteDelta, CsrQuoteSource, GirrQuoteDelta, GirrQuoteTarget, csr_quote_delta,
    girr_quote_delta,
};
use crate::types::PositionId;
use crate::{Portfolio, Result};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_margin::regulatory::frtb::FrtbSensitivities;
use finstack_quant_valuations::pricer::InstrumentType;
use serde::{Deserialize, Serialize};

/// Exact source coordinates for the portfolio's two qualified ordinary delta grids.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PortfolioFrtbConfig {
    /// One admitted rate quote coordinate per required ordinary GIRR vertex and curve role.
    pub girr_targets: Vec<GirrQuoteTarget>,
    /// Admitted single-name CDS issuer classifications and their original spread quotes.
    pub csr_sources: Vec<CsrQuoteSource>,
}

/// Availability reason for a position that contributes no supported ordinary quote rows.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum OrdinaryDeltaStatus {
    /// The selected instrument family needs a different qualified regulatory bridge.
    UnsupportedInstrument,
    /// The position has zero signed size.
    ZeroQuantity,
}

/// Position reported outside the admitted ordinary delta grids.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct FrtbUnmappedPosition {
    /// Original position identifier in the portfolio.
    pub position_id: PositionId,
    /// Classification of why this position supplies no rows.
    pub reason: OrdinaryDeltaStatus,
}

/// Evidence and amounts for the admitted ordinary delta slice of FRTB.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct PortfolioFrtbSensitivitiesResult {
    /// Ordinary GIRR and non-securitisation CSR deltas in reporting currency per percentage point.
    pub sensitivities: FrtbSensitivities,
    /// Exact rate reprices and the unchanged pricing and FX basis.
    pub girr: GirrQuoteDelta,
    /// Exact single-name spread reprices and the dated issuer classification.
    pub csr: CsrQuoteDelta,
    /// Active positions considered for at least one supported ordinary delta family.
    #[serde(with = "finstack_quant_core::wire::count")]
    #[cfg_attr(feature = "json-schema", schemars(with = "u32"))]
    pub positions_mapped: usize,
    /// Positions that were outside this workflow in portfolio order.
    pub positions_unmapped: Vec<FrtbUnmappedPosition>,
}

/// Extract the ordinary GIRR and single-name CDS CSR quote grids for a trading book.
///
/// The route admits bonds with ordinary rate curves, money-market deposits,
/// interest-rate swaps, and ordinary single-name CDS. Dependent credit curves
/// on instruments besides single-name CDS and inflation-linked cashflows use
/// other regulatory bridges. This result covers only the two named delta
/// families. Pass other required factors separately to the aggregation engine.
///
/// # Arguments
/// * `portfolio` - Trading book at its stored valuation date, with signed positions and reporting currency.
/// * `market` - Immutable market containing the recorded rate and hazard calibrations and the held FX basis.
/// * `pricing` - Pricing and conversion configuration used by both checked quote extractors.
/// * `config` - Exact ordinary GIRR targets and dated single-name CSR quote sources.
///
/// # Errors
/// Propagates mapping, market data, calibration and repricing errors for
/// admitted positions. Unsupported instrument families are reported separately.
pub fn portfolio_frtb_sensitivities(
    portfolio: &Portfolio,
    market: &MarketContext,
    pricing: &FinstackConfig,
    config: &PortfolioFrtbConfig,
) -> Result<PortfolioFrtbSensitivitiesResult> {
    let mut mapped = portfolio.clone();
    let mut positions_unmapped = Vec::new();
    let mut active = Vec::with_capacity(portfolio.positions.len());
    for position in &portfolio.positions {
        let key = position.instrument.key();
        let reason = if position.quantity == 0.0 {
            Some(OrdinaryDeltaStatus::ZeroQuantity)
        } else if matches!(
            key,
            InstrumentType::Bond
                | InstrumentType::Deposit
                | InstrumentType::Irs
                | InstrumentType::Cds
        ) {
            let deps = position.instrument.market_dependencies()?;
            let has_other_credit = key != InstrumentType::Cds
                && (!deps.curves.credit_curves.is_empty() || !deps.credit_index_ids.is_empty());
            (has_other_credit || !deps.curves.inflation_curves.is_empty())
                .then_some(OrdinaryDeltaStatus::UnsupportedInstrument)
        } else {
            Some(OrdinaryDeltaStatus::UnsupportedInstrument)
        };
        if let Some(reason) = reason {
            positions_unmapped.push(FrtbUnmappedPosition {
                position_id: position.position_id.clone(),
                reason,
            });
        } else {
            active.push(position.clone());
        }
    }
    mapped.positions = active;
    mapped.rebuild_index();
    let girr = girr_quote_delta(&mapped, market, pricing, &config.girr_targets)?;
    let csr = csr_quote_delta(&mapped, market, pricing, &config.csr_sources)?;
    let mut sensitivities = FrtbSensitivities::new(portfolio.base_currency);
    sensitivities
        .girr_delta
        .clone_from(&girr.sensitivities.girr_delta);
    sensitivities
        .csr_nonsec_delta
        .clone_from(&csr.sensitivities.csr_nonsec_delta);

    Ok(PortfolioFrtbSensitivitiesResult {
        sensitivities,
        girr,
        csr,
        positions_mapped: mapped.positions.len(),
        positions_unmapped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::{Position, PositionUnit};
    use crate::types::Entity;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_valuations::instruments::{
        Instrument, equity::spot::Equity, fixed_income::bond::Bond,
    };
    use std::sync::Arc;

    #[test]
    fn frtb_ordinary_grids_distinguish_exclusions_and_required_data_failures() {
        let book = Portfolio::builder("ordinary")
            .base_currency(Currency::USD)
            .as_of(time::macros::date!(2025 - 01 - 02))
            .entity(Entity::new("trader"));
        let equity = Equity::example().expect("valid shares");
        let equity = Position::new(
            "equity",
            "trader",
            equity.id(),
            Arc::new(equity.clone()),
            1.0,
            PositionUnit::Units,
        )
        .expect("active equity holding");
        let unmapped_book = book.position(equity).build().expect("empty risk grids");
        let empty = PortfolioFrtbConfig {
            girr_targets: Vec::new(),
            csr_sources: Vec::new(),
        };
        let market = MarketContext::new();
        let pricing = FinstackConfig::default();
        let report = portfolio_frtb_sensitivities(&unmapped_book, &market, &pricing, &empty)
            .expect("unsupported equity need not be priced");
        assert_eq!(report.positions_mapped, 0);
        assert!(matches!(
            report.positions_unmapped[0].reason,
            OrdinaryDeltaStatus::UnsupportedInstrument
        ));
        let bond = Bond::example().expect("valid government bond");
        let bond = Position::new(
            "bond",
            "trader",
            bond.id(),
            Arc::new(bond.clone()),
            1.0,
            PositionUnit::Units,
        )
        .expect("active rate holding");
        let book = unmapped_book.with_position(bond).expect("complete book");
        let error = portfolio_frtb_sensitivities(&book, &market, &pricing, &empty)
            .expect_err("admitted rates must have every qualified factor");
        assert!(
            error.to_string().contains("Missing GIRR delta source")
                && error.to_string().contains("bond")
        );
    }
}
