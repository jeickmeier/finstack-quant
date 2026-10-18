//! Per-flow cashflow export: the JSON and parsed-instrument entry points over
//! the core exporter.
//!
//! The engine, envelope and row types are defined in
//! `finstack_quant_valuations_core::instruments::cashflow_export` and re-exported
//! here; this module adds the closed-world entry points that start from
//! instrument JSON or a [`ParsedInstrument`](crate::pricer::ParsedInstrument).

pub use finstack_quant_valuations_core::instruments::cashflow_export::*;

use crate::pricer::ParsedInstrument;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::{Error, Result};

// Public entry point

/// Build the enriched cashflow envelope for a tagged instrument and serialize to JSON.
///
/// # Errors
///
/// Returns `Error::Validation` if the model string is not one of
/// `{"discounting", "hazard_rate"}`, if the `(instrument_type, model)` pair is
/// not in the standard pricer registry, if required curves are missing from
/// the market, or if the schedule mixes currencies.
///
/// # Arguments
///
/// * `instrument_json` - UTF-8 canonical v1 instrument envelope.
/// * `market` - Market context supplying discount, credit, index, and FX data
///   required to build and value the cashflow schedule.
/// * `as_of` - ISO-8601 valuation date used for schedule eligibility and
///   cashflow present values.
/// * `model` - Registered pricing-model name: `"discounting"` or
///   `"hazard_rate"` when that instrument/model pair is supported.
pub fn instrument_cashflows_json(
    instrument_json: &str,
    market: &MarketContext,
    as_of: &str,
    model: &str,
) -> Result<String> {
    let instrument = crate::pricer::parse_boxed_instrument_from_json(instrument_json, None)?;
    let envelope = instrument_cashflows(&instrument, market, as_of, model)?;
    serde_json::to_string(&envelope)
        .map_err(|e| Error::Validation(format!("failed to serialize cashflow envelope: {e}")))
}

/// Build the enriched cashflow envelope for an already parsed and validated
/// instrument.
///
/// This is the canonical core behind [`instrument_cashflows_json`]. Host
/// bindings use it after parsing the instrument so validation precedence does
/// not require a second deserialization.
///
/// # Arguments
///
/// * `instrument` - Validated instrument whose cashflows are projected and
///   reconciled to canonical pricing.
/// * `market` - Market context supplying discount, credit, index, and FX data
///   required to build and value the cashflow schedule.
/// * `as_of` - ISO-8601 valuation date used for schedule eligibility and
///   cashflow present values.
/// * `model` - Registered pricing-model name: `"discounting"` or
///   `"hazard_rate"` when that instrument/model pair is supported.
///
/// # Errors
///
/// Returns `Error::Validation` if the model is unsupported, the instrument and
/// model are not registered together, required market data is missing, or the
/// resulting cashflows cannot be reconciled.
pub fn instrument_cashflows(
    instrument: &ParsedInstrument,
    market: &MarketContext,
    as_of: &str,
    model: &str,
) -> Result<InstrumentCashflowEnvelope> {
    export_cashflows(instrument.as_instrument(), market, as_of, model)
}
