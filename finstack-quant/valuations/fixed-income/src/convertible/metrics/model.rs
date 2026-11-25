//! Credit lattice a convertible metric prices on.

use crate::convertible::ConvertibleBond;
use crate::convertible::pricing::{ConvertibleModel, price_convertible_bond_with_model};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations_core::metrics::MetricContext;

/// Credit lattice selected by the valuation a metric belongs to.
pub(super) fn selected_model(
    context: &MetricContext,
) -> finstack_quant_core::Result<ConvertibleModel> {
    ConvertibleModel::from_model_key(context.pricing_model())
}

/// Value of `bond` on `market`, on the credit lattice the valuation selected.
///
/// Bump-and-reprice metrics use this rather than `Instrument::value`, which
/// always prices with the instrument's default model.
pub(super) fn model_value(
    context: &MetricContext,
    bond: &ConvertibleBond,
    market: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    Ok(price_convertible_bond_with_model(bond, market, as_of, selected_model(context)?)?.amount())
}
