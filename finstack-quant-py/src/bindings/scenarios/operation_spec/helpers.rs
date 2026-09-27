//! Shared input-extraction helpers for scenario operation bindings.
//!
//! Every enum-valued parameter accepts either the typed wrapper
//! (`CurveKind.discount()`) or its canonical snake-case wire label
//! (`"discount"`); attribute maps accept a mapping or a sequence of pairs.

use std::str::FromStr;

use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::hierarchy::HierarchyTarget;
use finstack_quant_scenarios::spec::{Compounding, CurveKind, TenorMatchMode, TimeRollMode};
use finstack_quant_valuations::pricer::InstrumentType;
use indexmap::IndexMap;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use super::hierarchy::PyHierarchyTarget;
use super::kinds::{PyCompounding, PyCurveKind, PyTenorMatchMode, PyTimeRollMode};

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

/// Parse a serde string-enum from its snake-case wire label.
pub(super) fn label_to_enum<T: serde::de::DeserializeOwned>(
    type_name: &str,
    label: &str,
    accepted: &str,
) -> PyResult<T> {
    serde_json::from_value(serde_json::Value::String(label.to_string())).map_err(|_| {
        crate::errors::value_error(format!(
            "Unknown {type_name} label {label:?}; expected one of: {accepted}"
        ))
    })
}

/// Render a serde string-enum as its snake-case wire label.
pub(super) fn enum_to_label<T: serde::Serialize>(value: &T) -> PyResult<String> {
    match serde_json::to_value(value).map_err(crate::errors::display_to_py)? {
        serde_json::Value::String(label) => Ok(label),
        _ => Err(crate::errors::value_error(
            "scenario enum did not serialize to a string",
        )),
    }
}

macro_rules! typed_or_label {
    ($fn_name:ident, $wrapper:ty, $inner:ty, $type_name:literal, $accepted:literal) => {
        /// Accept the typed wrapper or its snake-case wire label.
        pub(super) fn $fn_name(obj: &Bound<'_, PyAny>) -> PyResult<$inner> {
            if let Ok(typed) = obj.cast::<$wrapper>() {
                return Ok(typed.borrow().inner);
            }
            let label: String = obj.extract().map_err(|_| {
                crate::errors::value_error(format!(
                    "{} must be a {} or one of: {}; got {}",
                    $type_name,
                    $type_name,
                    $accepted,
                    obj.get_type()
                ))
            })?;
            label_to_enum::<$inner>($type_name, &label, $accepted)
        }
    };
}

typed_or_label!(
    extract_curve_kind,
    PyCurveKind,
    CurveKind,
    "CurveKind",
    "discount, forward, par_cds, inflation, commodity"
);
typed_or_label!(
    extract_tenor_match_mode,
    PyTenorMatchMode,
    TenorMatchMode,
    "TenorMatchMode",
    "exact, interpolate"
);
typed_or_label!(
    extract_time_roll_mode,
    PyTimeRollMode,
    TimeRollMode,
    "TimeRollMode",
    "business_days, calendar_days, approximate"
);
/// Accept the typed `Compounding` wrapper or its canonical label.
pub(super) fn extract_compounding(obj: &Bound<'_, PyAny>) -> PyResult<Compounding> {
    if let Ok(typed) = obj.cast::<PyCompounding>() {
        return Ok(typed.borrow().inner);
    }
    let label: String = obj.extract().map_err(|_| {
        crate::errors::value_error(format!(
            "Compounding must be a Compounding or one of: {}; got {}",
            super::kinds::COMPOUNDING_LABELS,
            obj.get_type()
        ))
    })?;
    super::kinds::parse_compounding(&label)
}

/// Accept `Mapping[str, str]` or `Sequence[tuple[str, str]]`, preserving
/// insertion order.
pub(super) fn extract_attrs(obj: &Bound<'_, PyAny>) -> PyResult<IndexMap<String, String>> {
    let pairs: Vec<(String, String)> = if let Ok(dict) = obj.cast::<PyDict>() {
        dict.iter()
            .map(|(k, v)| Ok((k.extract::<String>()?, v.extract::<String>()?)))
            .collect::<PyResult<Vec<_>>>()?
    } else if obj.hasattr("items")? {
        obj.call_method0("items")?
            .try_iter()?
            .map(|item| item?.extract::<(String, String)>())
            .collect::<PyResult<Vec<_>>>()?
    } else {
        obj.extract().map_err(|_| {
            crate::errors::value_error(format!(
                "attrs must be a mapping of str -> str or a sequence of (key, value) pairs; got {}",
                obj.get_type()
            ))
        })?
    };
    let mut map = IndexMap::with_capacity(pairs.len());
    for (k, v) in pairs {
        map.insert(k, v);
    }
    Ok(map)
}

/// Accept a `HierarchyTarget` or its JSON string.
pub(super) fn extract_hierarchy_target(obj: &Bound<'_, PyAny>) -> PyResult<HierarchyTarget> {
    if let Ok(target) = obj.cast::<PyHierarchyTarget>() {
        return Ok(target.borrow().inner.clone());
    }
    let json: String = obj.extract().map_err(|_| {
        crate::errors::value_error(format!(
            "target must be a HierarchyTarget or a JSON string; got {}",
            obj.get_type()
        ))
    })?;
    serde_json::from_str(&json)
        .map_err(|e| crate::errors::value_error(format!("Invalid HierarchyTarget JSON: {e}")))
}
