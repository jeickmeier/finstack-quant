//! JSON entry points shared by Python and WASM bindings.

use crate::{CovenantEngine, CovenantReport, CovenantSpec, HashMapMetricSource};
use finstack_quant_core::Result;
use finstack_quant_core::canonical::to_canonical_bytes;
use finstack_quant_core::dates::parse_iso_date;
use serde::Serialize;
use serde::de::DeserializeOwned;

fn roundtrip_json<T>(json: &str) -> Result<String>
where
    T: DeserializeOwned + Serialize,
{
    let value: T = serde_json::from_str(json).map_err(|e| {
        finstack_quant_core::Error::Validation(format!("Invalid covenant JSON: {e}"))
    })?;
    canonical_json(&value)
}

fn canonical_json<T: Serialize>(value: &T) -> Result<String> {
    let bytes = to_canonical_bytes(value)?;
    String::from_utf8(bytes).map_err(|error| {
        finstack_quant_core::Error::Validation(format!(
            "Canonical covenant JSON was not UTF-8: {error}"
        ))
    })
}

/// Validate and canonicalize one [`CovenantSpec`] encoded as JSON.
///
/// `json` must describe a complete covenant specification, including a
/// compatible covenant type, test, measurement frequency, and any required
/// metric identifier. The returned string is semantically equivalent to the
/// input but is encoded with [`finstack_quant_core::canonical`]: object keys are
/// sorted recursively, arrays retain semantic order, and insignificant
/// whitespace is removed.
///
/// # Arguments
///
/// * `json` - UTF-8 JSON document representing one complete covenant
///   specification; it is parsed and validated before canonical serialization.
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] if `json` is malformed,
/// cannot be decoded as a [`CovenantSpec`], violates its business invariants
/// (for example, an invalid threshold or frequency), or cannot be serialized
/// back to JSON.
pub fn validate_covenant_spec_json(json: &str) -> Result<String> {
    let value: CovenantSpec = serde_json::from_str(json).map_err(|e| {
        finstack_quant_core::Error::Validation(format!("Invalid covenant JSON: {e}"))
    })?;
    value.validate()?;
    canonical_json(&value)
}

/// Decode and canonicalize one [`CovenantReport`] encoded as JSON.
///
/// This is a structural round trip for a report that has already been
/// produced or stored. Unlike [`validate_covenant_spec_json`] and
/// [`validate_covenant_engine_json`], it does not re-run business-rule
/// validation because a report is an observed evaluation result, not a
/// configuration. The returned string uses the core canonical JSON
/// representation with recursively sorted object keys.
///
/// # Arguments
///
/// * `json` - UTF-8 JSON document representing a previously produced covenant
///   report; this function validates its schema but does not re-evaluate it.
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] if `json` is malformed,
/// does not match the report schema, or cannot be serialized after decoding.
pub fn validate_covenant_report_json(json: &str) -> Result<String> {
    roundtrip_json::<CovenantReport>(json)
}

/// Validate and canonicalize a [`CovenantEngine`] encoded as JSON.
///
/// An engine includes the full covenant package and its configuration. Only
/// `specs` is required; `breach_history`, `windows` and `waivers` default to
/// empty. Use this boundary to reject an invalid imported package before it is
/// evaluated; the returned JSON is the stable serialized representation of the
/// validated engine.
///
/// # Arguments
///
/// * `json` - UTF-8 JSON document representing a full covenant engine and its
///   specifications; business invariants are validated before serialization.
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] if `json` is malformed,
/// cannot be decoded as an engine, contains an invalid covenant package, or
/// cannot be serialized back to JSON.
pub fn validate_covenant_engine_json(json: &str) -> Result<String> {
    let value: CovenantEngine = serde_json::from_str(json).map_err(|e| {
        finstack_quant_core::Error::Validation(format!("Invalid covenant JSON: {e}"))
    })?;
    value.validate()?;
    canonical_json(&value)
}

/// Evaluate a covenant engine against a JSON metric map, returning typed reports.
///
/// `engine_json` must contain a valid [`CovenantEngine`]. `metrics_json` must
/// be a JSON object whose values are JSON numbers; its keys are the metric
/// identifiers referenced by the engine, such as `debt_to_ebitda`, `dscr`, or
/// `liquidity`. The function evaluates every covenant as of the ISO-8601
/// calendar date in `as_of` and returns an [`indexmap::IndexMap`] keyed by
/// stable covenant instance key.
///
/// A metric value has the unit required by its covenant test: leverage and
/// coverage metrics are ratios (for example, `5.0` means 5.0x), while monetary
/// thresholds such as capex and liquidity use the engine's reporting currency
/// convention. This API deliberately does not infer units or metric aliases.
/// Language bindings that surface typed report objects call this so the
/// parsing, validation, and unit conventions stay owned by the crate rather
/// than duplicated per binding.
///
/// # Arguments
///
/// * `engine_json` - UTF-8 JSON document for a valid covenant engine whose
///   metric identifiers define the required keys in `metrics_json`.
/// * `metrics_json` - UTF-8 JSON object mapping metric IDs to finite numeric
///   values in the units required by their individual covenant tests.
/// * `as_of` - ISO-8601 calendar date at which all covenant tests are
///   evaluated.
///
/// # Errors
///
/// Returns an error if the engine or metric map is malformed, any metric value
/// is not a JSON number, `as_of` is not an ISO-8601 date, the engine fails
/// validation, or a required metric is absent or unsuitable for its covenant.
pub fn evaluate_engine(
    engine_json: &str,
    metrics_json: &str,
    as_of: &str,
) -> Result<indexmap::IndexMap<String, CovenantReport>> {
    let engine = parse_engine(engine_json)?;
    let source = HashMapMetricSource::from_json(metrics_json)?;
    let test_date = parse_iso_date(as_of)?;
    engine.evaluate_specs(&engine.applicable_specs(test_date), &source, test_date)
}

/// Evaluate a covenant engine document against an already-built metric source.
///
/// Same evaluation as [`evaluate_engine`], for hosts that hold the metrics as
/// numbers rather than JSON text (a Python `dict`). The values are not
/// serialized, so finiteness is checked only by the covenant tests that read
/// a metric, exactly as in [`CovenantEngine::evaluate`].
///
/// # Arguments
///
/// * `engine_json` - UTF-8 JSON document for a valid covenant engine.
/// * `source` - Metric values keyed by the metric identifiers the engine
///   reads, in the units required by each covenant test (ratios in turns,
///   amounts in the engine's reporting currency).
/// * `as_of` - ISO-8601 calendar date at which all covenant tests are
///   evaluated.
///
/// # Returns
///
/// Reports keyed by covenant instance key, in spec order.
///
/// # Errors
///
/// Returns an error if the engine document is malformed or fails validation,
/// `as_of` is not an ISO-8601 date, or a required metric is absent or not
/// finite.
pub fn evaluate_engine_with_source(
    engine_json: &str,
    source: &HashMapMetricSource,
    as_of: &str,
) -> Result<indexmap::IndexMap<String, CovenantReport>> {
    parse_engine(engine_json)?.evaluate(source, parse_iso_date(as_of)?)
}

fn parse_engine(engine_json: &str) -> Result<CovenantEngine> {
    let engine: CovenantEngine = serde_json::from_str(engine_json).map_err(|e| {
        finstack_quant_core::Error::Validation(format!("Invalid covenant engine JSON: {e}"))
    })?;
    engine.validate()?;
    Ok(engine)
}
