//! Pricer infrastructure: the core dispatch and registry types plus the
//! closed-world assembly.
//!
//! The dispatch types ([`InstrumentType`](crate::pricer::InstrumentType),
//! [`ModelKey`](crate::pricer::ModelKey), [`PricerKey`](crate::pricer::PricerKey),
//! [`Pricer`](crate::pricer::Pricer), [`PricerRegistry`](crate::pricer::PricerRegistry),
//! [`PricingDispatch`](crate::pricer::PricingDispatch), [`Family`](crate::pricer::Family))
//! are defined in `finstack_quant_valuations_core::pricer` and re-exported here.
//! This module adds what needs every instrument family: the
//! [`FAMILIES`](crate::pricer::FAMILIES) list, the [`STANDARD`](crate::pricer::STANDARD)
//! umbrella family and its registries, and the JSON pricing entry points
//! ([`parse_boxed_instrument_from_json`](crate::pricer::parse_boxed_instrument_from_json),
//! [`price_instrument`](crate::pricer::price_instrument) and friends).

pub use finstack_quant_valuations_core::pricer::*;

mod catalog;
mod json;
mod standard;
#[cfg(test)]
mod tests;

pub use catalog::{
    MetricCoordinate, MetricMetadata, list_models, list_models_grouped, list_standard_metrics,
    list_standard_metrics_grouped, metric_metadata, metrics_for_instrument,
};
pub use json::dispatch::{
    ParsedInstrument, PriceExtras, STANDARD_OPTION_GREEKS, instrument_envelope_from_spec,
    instrument_from_spec, parse_boxed_instrument_from_json, parse_boxed_instruments_from_json,
    parse_instrument_from_json, parse_typed_instrument_json, present_standard_option_greeks,
    pretty_instrument_json, price_instrument, validate_instrument_json,
    validate_typed_instrument_json,
};
pub use standard::{FAMILIES, STANDARD};
