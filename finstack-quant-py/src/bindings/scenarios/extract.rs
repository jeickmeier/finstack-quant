//! Input extraction shared by the scenario entry points.
//!
//! Every entry point accepts the typed wrapper or its canonical JSON string:
//! `ScenarioSpec | str`, `FinstackConfig | str | None`, and typed instrument
//! wrappers or envelope JSON. The Rust scenario engine attaches its own
//! recalibration provider, so the bindings only thread the configuration.

use finstack_quant_core::config::FinstackConfig;
use finstack_quant_scenarios::ScenarioSpec;
use finstack_quant_valuations::instruments::Instrument;
use pyo3::prelude::*;

use super::spec::PyScenarioSpec;
use crate::bindings::core::config::PyFinstackConfig;
use crate::bindings::extract::extract_instrument_json;
use crate::errors::{core_to_py, scenarios_to_py, value_error};

/// Extract a validated [`ScenarioSpec`] from a `ScenarioSpec` object or a
/// JSON string.
///
/// JSON parse failures raise `ValueError` prefixed with
/// `Failed to parse ScenarioSpec JSON:`; semantic failures raise exactly what
/// `ScenarioSpec.validate()` raises.
pub(crate) fn extract_scenario_spec(obj: &Bound<'_, PyAny>) -> PyResult<ScenarioSpec> {
    if let Ok(spec) = obj.cast::<PyScenarioSpec>() {
        return Ok(spec.borrow().inner.clone());
    }
    let json: String = obj.extract().map_err(|_| {
        value_error(format!(
            "scenario must be a ScenarioSpec or a JSON string, got {}",
            obj.get_type()
        ))
    })?;
    ScenarioSpec::from_json(&json).map_err(scenarios_to_py)
}

/// Extract a [`FinstackConfig`] from a `FinstackConfig` object, a JSON string,
/// or `None` (library default).
pub(crate) fn extract_config(obj: Option<&Bound<'_, PyAny>>) -> PyResult<FinstackConfig> {
    let Some(obj) = obj else {
        return Ok(FinstackConfig::default());
    };
    if obj.is_none() {
        return Ok(FinstackConfig::default());
    }
    if let Ok(config) = obj.cast::<PyFinstackConfig>() {
        return Ok(config.borrow().inner.clone());
    }
    let json: String = obj.extract().map_err(|_| {
        value_error(format!(
            "config must be a FinstackConfig, a JSON string, or None, got {}",
            obj.get_type()
        ))
    })?;
    serde_json::from_str(&json)
        .map_err(|error| value_error(format!("Failed to parse FinstackConfig JSON: {error}")))
}

/// Extract an optional instrument collection from typed instrument wrappers
/// or canonical envelope JSON strings.
pub(crate) fn extract_instruments(
    objs: Option<Vec<Bound<'_, PyAny>>>,
) -> PyResult<Option<Vec<Box<dyn Instrument>>>> {
    let Some(objs) = objs else {
        return Ok(None);
    };
    objs.iter()
        .map(|obj| {
            let json = extract_instrument_json(obj)?;
            finstack_quant_valuations::pricer::json::parse_boxed_instrument_from_json(&json, None)
                .map(|parsed| parsed.into_boxed())
                .map_err(core_to_py)
        })
        .collect::<PyResult<Vec<_>>>()
        .map(Some)
}
