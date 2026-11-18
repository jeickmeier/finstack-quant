//! Capital Structure Integration Logic
//!
//! Builds the typed instruments of a [`crate::types::CapitalStructureSpec`].
//! Period aggregation and consolidation run in the evaluator's
//! capital-structure runtime; the single-instrument / single-period
//! extraction lives in
//! [`crate::capital_structure::period_flows`]. The optional JSON-spec →
//! instrument constructor `build_instrument_from_spec()` resolves the typed
//! instrument payload through the valuations registry.

use crate::error::Result;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::fx::FxQuery;
use finstack_quant_valuations::instruments::Instrument;
use std::sync::Arc;

/// Build a runtime instrument from a [`crate::types::DebtInstrumentSpec`].
///
/// Delegates to the canonical valuations instrument registry. The `spec`
/// payload must be the registry's tagged form (`{"type": "...", "spec": {...}}`).
///
/// # Arguments
///
/// * `spec` - Debt-instrument specification with an ID and a registry-tagged
///   JSON payload used to construct the runtime instrument. Its declared kind
///   determines whether positive interest cashflows are debt payments or hedge receipts.
///
/// # Errors
///
/// Returns an error when the payload does not match a registered instrument
/// type or fails spec validation.
pub fn build_instrument_from_spec(
    spec: &crate::types::DebtInstrumentSpec,
) -> Result<Arc<dyn Instrument>> {
    let instrument: finstack_quant_valuations::instruments::json_loader::InstrumentJson =
        spec.spec.clone().into();
    instrument.into_boxed().map(Arc::from).map_err(|e| {
        crate::error::Error::build(format!(
            "Failed to build debt instrument '{}': {e}",
            spec.id
        ))
    })
}

/// The period-close FX rate from `from` into the reporting currency `to`.
///
/// # Errors
///
/// Returns the reason consolidation is unavailable: no FX matrix in the
/// market, or no quote for the pair on `on`.
pub(crate) fn period_close_rate(
    from: Currency,
    to: Currency,
    on: Date,
    fx_matrix: Option<&Arc<finstack_quant_core::money::fx::FxMatrix>>,
) -> std::result::Result<crate::capital_structure::FxConversionRecord, String> {
    let Some(fx) = fx_matrix else {
        return Err(format!(
            "no FX matrix in the market to convert {from} to {to} on {on}"
        ));
    };
    let quote = fx
        .rate(FxQuery::new(
            from,
            to,
            on,
            finstack_quant_core::money::fx::FxConversionPolicy::PeriodEnd,
        ))
        .map_err(|error| format!("no {from}/{to} quote on {on}: {error}"))?;
    Ok(crate::capital_structure::FxConversionRecord {
        from,
        to,
        rate: quote.rate,
        date: on,
        source: if quote.triangulated {
            "triangulated".to_string()
        } else {
            "direct".to_string()
        },
    })
}
