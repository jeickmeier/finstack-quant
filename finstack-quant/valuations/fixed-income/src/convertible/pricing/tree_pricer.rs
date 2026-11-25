//! Registry adapters for the two convertible lattice models.

use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations_core::pricer::expect_inst;

use finstack_quant_valuations_core::instruments::Instrument;

use super::engine::{ConvertibleModel, price_convertible_bond_with_model};

/// Registry pricer for Convertible Bond using a defaultable-equity tree with recovery of principal.
pub(crate) struct ConvertibleTreePricer;

impl finstack_quant_valuations_core::pricer::Pricer for ConvertibleTreePricer {
    fn key(&self) -> finstack_quant_valuations_core::pricer::PricerKey {
        finstack_quant_valuations_core::pricer::PricerKey::new(
            finstack_quant_valuations_core::pricer::InstrumentType::Convertible,
            finstack_quant_valuations_core::pricer::ModelKey::Tree,
        )
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<
        finstack_quant_valuations_core::results::ValuationResult,
        finstack_quant_valuations_core::pricer::PricingError,
    > {
        let convertible = expect_inst::<crate::convertible::ConvertibleBond>(
            instrument,
            finstack_quant_valuations_core::pricer::InstrumentType::Convertible,
        )?;

        let pv = price_convertible_bond_with_model(
            convertible,
            market,
            as_of,
            ConvertibleModel::DefaultableEquity,
        )
        .map_err(|e| {
            finstack_quant_valuations_core::pricer::PricingError::model_failure_with_context(
                e.to_string(),
                finstack_quant_valuations_core::pricer::PricingErrorContext::default(),
            )
        })?;

        Ok(
            finstack_quant_valuations_core::results::ValuationResult::stamped(
                convertible.id(),
                as_of,
                pv,
            ),
        )
    }
}

/// Registry pricer for Convertible Bond using the Tsiveriotis-Fernandes lattice: cash-only
/// value discounted at the risk-free rate plus the credit spread, with no default event.
pub(crate) struct ConvertibleTsiveriotisFernandesPricer;

impl finstack_quant_valuations_core::pricer::Pricer for ConvertibleTsiveriotisFernandesPricer {
    fn key(&self) -> finstack_quant_valuations_core::pricer::PricerKey {
        finstack_quant_valuations_core::pricer::PricerKey::new(
            finstack_quant_valuations_core::pricer::InstrumentType::Convertible,
            finstack_quant_valuations_core::pricer::ModelKey::TsiveriotisFernandes,
        )
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<
        finstack_quant_valuations_core::results::ValuationResult,
        finstack_quant_valuations_core::pricer::PricingError,
    > {
        let convertible = expect_inst::<crate::convertible::ConvertibleBond>(
            instrument,
            finstack_quant_valuations_core::pricer::InstrumentType::Convertible,
        )?;

        let pv = price_convertible_bond_with_model(
            convertible,
            market,
            as_of,
            ConvertibleModel::TsiveriotisFernandes,
        )
        .map_err(|e| {
            finstack_quant_valuations_core::pricer::PricingError::model_failure_with_context(
                e.to_string(),
                finstack_quant_valuations_core::pricer::PricingErrorContext::default(),
            )
        })?;

        Ok(
            finstack_quant_valuations_core::results::ValuationResult::stamped(
                convertible.id(),
                as_of,
                pv,
            ),
        )
    }
}
