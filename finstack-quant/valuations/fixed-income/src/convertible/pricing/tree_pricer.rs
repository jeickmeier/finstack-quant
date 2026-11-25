//! Registry adapters for the two convertible lattice models.

use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext, expect_inst,
};
use finstack_quant_valuations_core::results::ValuationResult;

use super::engine::{ConvertibleModel, price_convertible_bond_with_model};

fn price_convertible_lattice(
    instrument: &dyn Instrument,
    market: &MarketContext,
    as_of: finstack_quant_core::dates::Date,
    model: ConvertibleModel,
) -> std::result::Result<ValuationResult, PricingError> {
    let convertible = expect_inst::<crate::convertible::ConvertibleBond>(
        instrument,
        InstrumentType::Convertible,
    )?;

    let pv = price_convertible_bond_with_model(convertible, market, as_of, model).map_err(|e| {
        PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
    })?;

    Ok(ValuationResult::stamped(convertible.id(), as_of, pv))
}

/// Registry pricer for Convertible Bond using a defaultable-equity tree with recovery of principal.
pub(crate) struct ConvertibleTreePricer;

impl Pricer for ConvertibleTreePricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::Convertible, ModelKey::Tree)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        price_convertible_lattice(
            instrument,
            market,
            as_of,
            ConvertibleModel::DefaultableEquity,
        )
    }
}

/// Registry pricer for Convertible Bond using the Tsiveriotis-Fernandes lattice: cash-only
/// value discounted at the risk-free rate plus the credit spread, with no default event.
pub(crate) struct ConvertibleTsiveriotisFernandesPricer;

impl Pricer for ConvertibleTsiveriotisFernandesPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::Convertible, ModelKey::TsiveriotisFernandes)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        price_convertible_lattice(
            instrument,
            market,
            as_of,
            ConvertibleModel::TsiveriotisFernandes,
        )
    }
}
