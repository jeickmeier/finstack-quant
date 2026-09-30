//! JSON entry points shared by Python and WASM bindings.

use crate::templates;
use crate::{CovenantEngine, CovenantReport, CovenantSpec, HashMapMetricSource};
use finstack_quant_core::canonical::to_canonical_bytes;
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
    let engine: CovenantEngine = serde_json::from_str(engine_json).map_err(|e| {
        finstack_quant_core::Error::Validation(format!("Invalid covenant engine JSON: {e}"))
    })?;
    engine.validate()?;
    let source = HashMapMetricSource::from_json(metrics_json)?;
    engine.evaluate(&source, parse_iso_date(as_of)?)
}

/// Build the standard leveraged-buyout covenant package as compact JSON.
///
/// The package contains quarterly maintenance tests for maximum Debt / EBITDA
/// (`initial_leverage`), minimum interest coverage (`interest_coverage`), and
/// minimum fixed-charge coverage (`fixed_charge_coverage`), plus an annual
/// maximum-capex test (`max_capex`). Ratios are expressed in turns (for
/// example, `5.0` is 5.0x); capex uses the caller's reporting-currency amount.
/// The leverage and interest-coverage tests have 30-day cure periods. A
/// leverage breach increases the rate by 200 basis points, while an
/// interest-coverage breach blocks distributions.
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] if any threshold is
/// `NaN`, infinite, or negative, or if the generated template cannot be
/// serialized to JSON.
///
/// # Arguments
///
/// * `initial_leverage` - Maximum debt-to-EBITDA threshold in turns, where
///   `5.0` represents 5.0x leverage.
/// * `interest_coverage` - Minimum interest-coverage threshold in turns.
/// * `fixed_charge_coverage` - Minimum fixed-charge-coverage threshold in
///   turns.
/// * `max_capex` - Maximum annual capital-expenditure amount in the caller's
///   reporting-currency convention.
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
    )?)
    .map_err(|e| {
        finstack_quant_core::Error::Validation(format!("Serialize covenant template: {e}"))
    })
}

/// Build the covenant-lite leveraged-loan package as compact JSON.
///
/// The package contains quarterly *incurrence* tests, not maintenance tests:
/// maximum total leverage (`max_leverage`) and maximum senior leverage
/// (`max_senior_leverage`), both expressed as debt / EBITDA turns. It also
/// includes an annual negative covenant that restricts additional secured debt
/// without consent. These thresholds are tested only when the caller evaluates
/// the relevant incurrence action.
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] if any threshold is
/// `NaN`, infinite, or negative, or if the generated template cannot be
/// serialized to JSON.
///
/// # Arguments
///
/// * `max_leverage` - Maximum total debt-to-EBITDA threshold in turns.
/// * `max_senior_leverage` - Maximum senior-debt-to-EBITDA threshold in
///   turns.
pub fn cov_lite_json(max_leverage: f64, max_senior_leverage: f64) -> Result<String> {
    serde_json::to_string(&templates::cov_lite(max_leverage, max_senior_leverage)?).map_err(|e| {
        finstack_quant_core::Error::Validation(format!("Serialize covenant template: {e}"))
    })
}

/// Build the commercial-real-estate covenant package as compact JSON.
///
/// The package defines quarterly maintenance tests for minimum debt-service
/// coverage ratio (`min_dscr`), minimum debt yield (`min_debt_yield`), and
/// maximum loan-to-value (`max_ltv`). All three inputs are decimal ratios, not
/// percentages: `0.10` means a 10% debt yield or LTV threshold where
/// applicable, and `1.25` means 1.25x DSCR. A DSCR breach sweeps 100% of cash;
/// an LTV breach sweeps 50%. The debt-yield and LTV covenants use distinct
/// labels so their reports cannot collide despite both being custom tests.
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] if any threshold is
/// `NaN`, infinite, or negative, or if the generated template cannot be
/// serialized to JSON.
///
/// # Arguments
///
/// * `min_dscr` - Minimum debt-service-coverage ratio in turns, such as
///   `1.25` for 1.25x coverage.
/// * `min_debt_yield` - Minimum debt yield as a decimal ratio, such as `0.10`
///   for 10%.
/// * `max_ltv` - Maximum loan-to-value ratio as a decimal fraction, such as
///   `0.65` for 65%.
pub fn real_estate_json(min_dscr: f64, min_debt_yield: f64, max_ltv: f64) -> Result<String> {
    serde_json::to_string(&templates::real_estate(min_dscr, min_debt_yield, max_ltv)?).map_err(
        |e| finstack_quant_core::Error::Validation(format!("Serialize covenant template: {e}")),
    )
}

/// Build the infrastructure or project-finance package as compact JSON.
///
/// The package contains quarterly maintenance tests for: a minimum default
/// DSCR (`min_dscr`), a higher minimum DSCR that locks distributions
/// (`distribution_lockup_dscr`), minimum debt-service-reserve liquidity
/// (`min_liquidity`), and maximum net debt / EBITDA (`max_net_leverage`). DSCR
/// and leverage inputs are turns; liquidity is a reporting-currency amount.
/// A default-DSCR breach has a 60-day cure period and an event-of-default
/// consequence. The distribution-lockup DSCR test only blocks distributions.
/// Distinct labels make the two DSCR reports and breach states unambiguous.
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] if any threshold is
/// `NaN`, infinite, or negative, or if the generated template cannot be
/// serialized to JSON.
///
/// # Arguments
///
/// * `min_dscr` - Minimum default debt-service-coverage ratio in turns.
/// * `distribution_lockup_dscr` - Higher DSCR threshold in turns that blocks
///   distributions without declaring a default.
/// * `min_liquidity` - Minimum debt-service-reserve amount in the caller's
///   reporting-currency convention.
/// * `max_net_leverage` - Maximum net-debt-to-EBITDA threshold in turns.
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
    )?)
    .map_err(|e| {
        finstack_quant_core::Error::Validation(format!("Serialize covenant template: {e}"))
    })
}
