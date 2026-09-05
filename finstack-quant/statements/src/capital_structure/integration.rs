//! Capital Structure Integration Logic
//!
//! Aggregation driver that walks every instrument in a
//! [`crate::types::CapitalStructureSpec`], pulls per-instrument cashflows via
//! [`finstack_quant_cashflows::CashflowProvider`], classifies them by `CFKind` into
//! the [`CashflowBreakdown`] buckets, and rolls them into per-period totals
//! both per-currency and (when FX is available) in the reporting currency.
//!
//! The single-instrument / single-period extraction lives in
//! [`crate::capital_structure::period_flows`]. The optional JSON-spec →
//! instrument constructor `build_instrument_from_spec()` resolves the typed
//! instrument payload through the valuations registry.

use crate::error::Result;
use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::fx::FxQuery;
use std::sync::Arc;

/// Build a runtime instrument from a [`crate::types::DebtInstrumentSpec`].
///
/// Delegates to the canonical valuations instrument registry. The `spec`
/// payload must be the registry's tagged form (`{"type": "...", "spec": {...}}`).
///
/// # Arguments
///
/// * `spec` - Debt-instrument specification with an ID and a registry-tagged
///   JSON payload used to construct the runtime cashflow provider.
///
/// # Errors
///
/// Returns an error when the payload does not match a registered instrument
/// type or fails spec validation.
pub fn build_instrument_from_spec(
    spec: &crate::types::DebtInstrumentSpec,
) -> Result<Arc<dyn CashflowProvider + Send + Sync>> {
    let instrument: finstack_quant_valuations::instruments::InstrumentJson =
        spec.spec.clone().into();
    instrument.into_cashflow_provider().map_err(|e| {
        crate::error::Error::build(format!(
            "Failed to build debt instrument '{}': {e}",
            spec.id
        ))
    })
}

/// Convert a money amount into the reporting currency when FX data is available.
pub(crate) fn convert_to_reporting(
    money: finstack_quant_core::money::Money,
    on: Date,
    reporting_currency: Option<Currency>,
    fx_matrix: Option<&Arc<finstack_quant_core::money::fx::FxMatrix>>,
    fx_policy: finstack_quant_core::money::fx::FxConversionPolicy,
) -> Result<Option<finstack_quant_core::money::Money>> {
    let Some(rc) = reporting_currency else {
        return Ok(None);
    };

    if rc == money.currency() {
        return Ok(Some(money));
    }

    let Some(fx) = fx_matrix else {
        return Err(crate::error::Error::capital_structure(format!(
            "Cannot convert {} to reporting currency {} on {}: no FX matrix present. \
             Supply FX in MarketContext (or remove reporting_currency / keep single-currency portfolios).",
            money.currency(),
            rc,
            on
        )));
    };

    let rate = fx
        .rate(FxQuery::with_policy(money.currency(), rc, on, fx_policy))?
        .rate;
    Ok(Some(finstack_quant_core::money::Money::new(
        money.amount() * rate,
        rc,
    )?))
}
