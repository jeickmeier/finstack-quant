//! WASM bindings for accounting-based credit scoring models.
//!
//! Mirrors `finstack-quant-py/src/bindings/models/credit/scoring.rs`. Every
//! scoring function takes its canonical input struct as a plain object and
//! returns the canonical `ScoringResult` (`score`, `zone`, `implied_pd`,
//! `model`).

use crate::utils::input::from_js_json;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_models::credit::scoring::{
    self, AltmanZDoublePrimeInput, AltmanZPrimeInput, AltmanZScoreInput, CreditScoreRequest,
    OhlsonOScoreInput, ZmijewskiInput,
};
use wasm_bindgen::prelude::*;

/// Original Altman (1968) Z-Score for publicly traded manufacturers.
/// @param input - `AltmanZScoreInput` object or JSON text: `working_capital_to_total_assets` (X1), `retained_earnings_to_total_assets` (X2), `ebit_to_total_assets` (X3), `market_equity_to_total_liabilities` (X4, market equity over book liabilities) and `sales_to_total_assets` (X5), all decimal ratios.
/// @returns The `ScoringResult` object (`score`, `zone`, `implied_pd`, `model`); `implied_pd` is `null`.
///
/// # Errors
///
/// Throws a `validation` error if the input is malformed, has an unknown
/// field, or a ratio is non-finite.
#[wasm_bindgen(js_name = altmanZScore, unchecked_return_type = "generated.models.ScoringResult")]
pub fn altman_z_score(
    #[wasm_bindgen(unchecked_param_type = "generated.models.AltmanZScoreInput | string")]
    input: JsValue,
) -> Result<JsValue, JsValue> {
    let input: AltmanZScoreInput = from_js_json(&input, "input")?;
    to_js_value(&scoring::altman_z_score(&input).map_err(to_js_err)?)
}

/// Altman Z'-Score for private firms (book equity replaces market equity).
/// @param input - `AltmanZPrimeInput` object or JSON text: `working_capital_to_total_assets` (X1), `retained_earnings_to_total_assets` (X2), `ebit_to_total_assets` (X3), `book_equity_to_total_liabilities` (X4, book equity over book liabilities) and `sales_to_total_assets` (X5), all decimal ratios.
/// @returns The `ScoringResult` object (`score`, `zone`, `implied_pd`, `model`); `implied_pd` is `null`.
///
/// # Errors
///
/// Throws a `validation` error if the input is malformed, has an unknown
/// field, or a ratio is non-finite.
#[wasm_bindgen(js_name = altmanZPrime, unchecked_return_type = "generated.models.ScoringResult")]
pub fn altman_z_prime(
    #[wasm_bindgen(unchecked_param_type = "generated.models.AltmanZPrimeInput | string")]
    input: JsValue,
) -> Result<JsValue, JsValue> {
    let input: AltmanZPrimeInput = from_js_json(&input, "input")?;
    to_js_value(&scoring::altman_z_prime(&input).map_err(to_js_err)?)
}

/// Altman Z''-Score for non-manufacturers (no sales ratio).
/// @param input - `AltmanZDoublePrimeInput` object or JSON text: `working_capital_to_total_assets` (X1), `retained_earnings_to_total_assets` (X2), `ebit_to_total_assets` (X3) and `book_equity_to_total_liabilities` (X4), all decimal ratios.
/// @returns The `ScoringResult` object (`score`, `zone`, `implied_pd`, `model`); `implied_pd` is `null`.
///
/// # Errors
///
/// Throws a `validation` error if the input is malformed, has an unknown
/// field, or a ratio is non-finite.
#[wasm_bindgen(js_name = altmanZDoublePrime, unchecked_return_type = "generated.models.ScoringResult")]
pub fn altman_z_double_prime(
    #[wasm_bindgen(unchecked_param_type = "generated.models.AltmanZDoublePrimeInput | string")]
    input: JsValue,
) -> Result<JsValue, JsValue> {
    let input: AltmanZDoublePrimeInput = from_js_json(&input, "input")?;
    to_js_value(&scoring::altman_z_double_prime(&input).map_err(to_js_err)?)
}

/// Altman emerging-market score: the Z''-Score plus a constant of 3.25.
/// @param input - `AltmanZDoublePrimeInput` object or JSON text: `working_capital_to_total_assets` (X1), `retained_earnings_to_total_assets` (X2), `ebit_to_total_assets` (X3) and `book_equity_to_total_liabilities` (X4), all decimal ratios.
/// @returns The `ScoringResult` object (`score`, `zone`, `implied_pd`, `model`); `implied_pd` is `null`.
///
/// # Errors
///
/// Throws a `validation` error if the input is malformed, has an unknown
/// field, or a ratio is non-finite.
#[wasm_bindgen(js_name = altmanEmScore, unchecked_return_type = "generated.models.ScoringResult")]
pub fn altman_em_score(
    #[wasm_bindgen(unchecked_param_type = "generated.models.AltmanZDoublePrimeInput | string")]
    input: JsValue,
) -> Result<JsValue, JsValue> {
    let input: AltmanZDoublePrimeInput = from_js_json(&input, "input")?;
    to_js_value(&scoring::altman_em_score(&input).map_err(to_js_err)?)
}

/// Ohlson (1980) O-Score logit bankruptcy model.
/// @param input - `OhlsonOScoreInput` object or JSON text: `log_total_assets_adjusted` (log of total assets over the GNP price-level index), `total_liabilities_to_total_assets`, `working_capital_to_total_assets`, `current_liabilities_to_current_assets`, `liabilities_exceed_assets` (indicator, exactly 0 or 1), `net_income_to_total_assets`, `funds_from_operations_to_total_liabilities`, `negative_net_income_two_years` (indicator, exactly 0 or 1) and `net_income_change` (`(NI_t - NI_{t-1}) / (|NI_t| + |NI_{t-1}|)`).
/// @returns The `ScoringResult` object (`score`, `zone`, `implied_pd`, `model`); `implied_pd` is the logistic probability.
///
/// # Errors
///
/// Throws a `validation` error if the input is malformed, has an unknown
/// field, a ratio is non-finite, or an indicator is not exactly 0 or 1.
#[wasm_bindgen(js_name = ohlsonOScore, unchecked_return_type = "generated.models.ScoringResult")]
pub fn ohlson_o_score(
    #[wasm_bindgen(unchecked_param_type = "generated.models.OhlsonOScoreInput | string")]
    input: JsValue,
) -> Result<JsValue, JsValue> {
    let input: OhlsonOScoreInput = from_js_json(&input, "input")?;
    to_js_value(&scoring::ohlson_o_score(&input).map_err(to_js_err)?)
}

/// Zmijewski (1984) probit financial-distress model.
/// @param input - `ZmijewskiInput` object or JSON text: `net_income_to_total_assets` (ROA), `total_liabilities_to_total_assets` (debt ratio) and `current_assets_to_current_liabilities` (current ratio), all decimal ratios.
/// @returns The `ScoringResult` object (`score`, `zone`, `implied_pd`, `model`); `implied_pd` is the probit probability.
///
/// # Errors
///
/// Throws a `validation` error if the input is malformed, has an unknown
/// field, or a ratio is non-finite.
#[wasm_bindgen(js_name = zmijewskiScore, unchecked_return_type = "generated.models.ScoringResult")]
pub fn zmijewski_score(
    #[wasm_bindgen(unchecked_param_type = "generated.models.ZmijewskiInput | string")]
    input: JsValue,
) -> Result<JsValue, JsValue> {
    let input: ZmijewskiInput = from_js_json(&input, "input")?;
    to_js_value(&scoring::zmijewski_score(&input).map_err(to_js_err)?)
}

/// Score one model and grade its implied PD on the default PD master scale.
///
/// The implied PD of a model that has one (Ohlson, Zmijewski) is graded on the
/// default PD master scale, so every host gets the same grade; the Altman
/// family has no implied PD and no grade.
/// @param request - `CreditScoreRequest` object or JSON text: `model` (`"altman_z_score"`, `"altman_z_prime"`, `"altman_z_double_prime"`, `"altman_em_score"`, `"ohlson_o_score"` or `"zmijewski_score"`) and `input` (that model's input ratios by Rust field name; `"altman_em_score"` takes the Z''-Score ratios).
/// @returns `CreditScoreReport` plain object: `result` (the `ScoringResult`) and `grade` (the default master-scale `MasterScaleResult` of the implied PD, or `null` for the Altman family).
///
/// # Errors
///
/// Throws a `validation` error if the request is malformed or has an unknown
/// model or field, a ratio is non-finite, or an Ohlson indicator is not
/// exactly 0 or 1.
#[wasm_bindgen(js_name = creditScoreReport, unchecked_return_type = "generated.models.CreditScoreReport")]
pub fn credit_score_report(
    #[wasm_bindgen(unchecked_param_type = "generated.models.CreditScoreRequest | string")] request: JsValue,
) -> Result<JsValue, JsValue> {
    let request: CreditScoreRequest = from_js_json(&request, "request")?;
    to_js_value(&scoring::credit_score_report(&request).map_err(to_js_err)?)
}
