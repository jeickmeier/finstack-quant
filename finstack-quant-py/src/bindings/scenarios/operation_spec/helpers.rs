//! Shared helpers for scenario operation bindings.

use std::str::FromStr;

use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::hierarchy::HierarchyTarget;
use finstack_quant_valuations::pricer::InstrumentType;
use indexmap::IndexMap;
use pyo3::prelude::*;

pub(super) fn parse_currency(code: &str) -> PyResult<Currency> {
    Currency::from_str(code)
        .map_err(|e| crate::errors::value_error(format!("Invalid currency code {code:?}: {e}")))
}

pub(super) fn parse_instrument_type(name: &str) -> PyResult<InstrumentType> {
    InstrumentType::from_str(name)
        .map_err(|e| crate::errors::value_error(format!("Invalid instrument type {name:?}: {e}")))
}

pub(super) fn parse_instrument_types(names: Vec<String>) -> PyResult<Vec<InstrumentType>> {
    names.iter().map(|s| parse_instrument_type(s)).collect()
}

pub(super) fn parse_attrs(pairs: Vec<(String, String)>) -> IndexMap<String, String> {
    let mut map = IndexMap::with_capacity(pairs.len());
    for (k, v) in pairs {
        map.insert(k, v);
    }
    map
}

pub(super) fn parse_hierarchy_target(json: &str) -> PyResult<HierarchyTarget> {
    serde_json::from_str(json)
        .map_err(|e| crate::errors::value_error(format!("Invalid HierarchyTarget JSON: {e}")))
}
