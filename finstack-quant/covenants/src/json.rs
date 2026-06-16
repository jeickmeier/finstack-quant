//! JSON entry points shared by Python and WASM bindings.

use crate::templates;
use crate::{CovenantEngine, CovenantReport, CovenantSpec, HashMapMetricSource};
use finstack_quant_core::dates::parse_iso_date;
use finstack_quant_core::Result;
use serde::de::DeserializeOwned;
use serde::Serialize;

fn roundtrip_json<T>(json: &str) -> Result<String>
where
    T: DeserializeOwned + Serialize,
{
    let value: T = serde_json::from_str(json).map_err(|e| {
        finstack_quant_core::Error::Validation(format!("Invalid covenant JSON: {e}"))
    })?;
    serde_json::to_string(&value).map_err(|e| {
        finstack_quant_core::Error::Validation(format!("Serialize covenant JSON: {e}"))
    })
}

/// Validate and canonicalize a covenant spec JSON string.
pub fn validate_covenant_spec_json(json: &str) -> Result<String> {
    let value: CovenantSpec = serde_json::from_str(json).map_err(|e| {
        finstack_quant_core::Error::Validation(format!("Invalid covenant JSON: {e}"))
    })?;
    value.validate()?;
    serde_json::to_string(&value).map_err(|e| {
        finstack_quant_core::Error::Validation(format!("Serialize covenant JSON: {e}"))
    })
}

/// Validate and canonicalize a covenant report JSON string.
pub fn validate_covenant_report_json(json: &str) -> Result<String> {
    roundtrip_json::<CovenantReport>(json)
}

/// Validate and canonicalize a covenant engine JSON string.
pub fn validate_covenant_engine_json(json: &str) -> Result<String> {
    let value: CovenantEngine = serde_json::from_str(json).map_err(|e| {
        finstack_quant_core::Error::Validation(format!("Invalid covenant JSON: {e}"))
    })?;
    value.validate()?;
    serde_json::to_string(&value).map_err(|e| {
        finstack_quant_core::Error::Validation(format!("Serialize covenant JSON: {e}"))
    })
}

/// Evaluate a covenant engine JSON string against a string-keyed metric map.
pub fn evaluate_engine_json(engine_json: &str, metrics_json: &str, as_of: &str) -> Result<String> {
    let engine: CovenantEngine = serde_json::from_str(engine_json).map_err(|e| {
        finstack_quant_core::Error::Validation(format!("Invalid covenant engine JSON: {e}"))
    })?;
    engine.validate()?;
    let metrics: Vec<(String, f64)> = serde_json::from_str::<
        serde_json::Map<String, serde_json::Value>,
    >(metrics_json)
    .map_err(|e| finstack_quant_core::Error::Validation(format!("Invalid metric map JSON: {e}")))?
    .into_iter()
    .map(|(key, value)| {
        value
            .as_f64()
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "Metric '{key}' must be a finite JSON number"
                ))
            })
            .map(|number| (key, number))
    })
    .collect::<Result<_>>()?;
    let mut source = HashMapMetricSource::from_pairs(metrics);
    let reports = engine.evaluate(&mut source, parse_iso_date(as_of)?)?;
    serde_json::to_string(&reports).map_err(|e| {
        finstack_quant_core::Error::Validation(format!("Serialize covenant reports: {e}"))
    })
}

/// Standard leveraged-buyout covenant package as JSON.
pub fn lbo_standard_json(
    initial_leverage: f64,
    interest_coverage: f64,
    fixed_charge_coverage: f64,
    max_capex: f64,
) -> Result<String> {
    serde_json::to_string(&templates::lbo_standard(
        initial_leverage,
        interest_coverage,
        fixed_charge_coverage,
        max_capex,
    ))
    .map_err(|e| {
        finstack_quant_core::Error::Validation(format!("Serialize covenant template: {e}"))
    })
}

/// Covenant-lite package as JSON.
pub fn cov_lite_json(max_leverage: f64, max_senior_leverage: f64) -> Result<String> {
    serde_json::to_string(&templates::cov_lite(max_leverage, max_senior_leverage)).map_err(|e| {
        finstack_quant_core::Error::Validation(format!("Serialize covenant template: {e}"))
    })
}

/// Real-estate covenant package as JSON.
pub fn real_estate_json(min_dscr: f64, min_debt_yield: f64, max_ltv: f64) -> Result<String> {
    serde_json::to_string(&templates::real_estate(min_dscr, min_debt_yield, max_ltv)).map_err(|e| {
        finstack_quant_core::Error::Validation(format!("Serialize covenant template: {e}"))
    })
}

/// Project-finance covenant package as JSON.
pub fn project_finance_json(
    min_dscr: f64,
    distribution_lockup_dscr: f64,
    min_liquidity: f64,
    max_net_leverage: f64,
) -> Result<String> {
    serde_json::to_string(&templates::project_finance(
        min_dscr,
        distribution_lockup_dscr,
        min_liquidity,
        max_net_leverage,
    ))
    .map_err(|e| {
        finstack_quant_core::Error::Validation(format!("Serialize covenant template: {e}"))
    })
}
