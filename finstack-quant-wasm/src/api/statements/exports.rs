//! Schedule conversion, Monte Carlo summaries, tables, hashing, and DSL parsing.

use crate::utils::input::{from_js_json, js_string, json_text};
use crate::utils::to_js_err;
use wasm_bindgen::prelude::*;

/// Export every period of one evaluated node as dated cells.
///
/// Free-function twin of Python `StatementResult.to_dated_schedule` (Rust
/// `evaluator::node_to_dated_schedule`). Periods come from the result's own
/// period plan in timeline order. A period without a value stays in the
/// schedule with its status and reason. The dates are period anchors, not
/// payment dates.
/// @param resultJson - The `StatementResult` returned by `Evaluator.evaluate` / `evaluateWithMarket` (object or JSON).
/// @param nodeId - Node identifier to export.
/// @param convention - `"end"` anchors each period on its last included day (`end - 1 day`, since periods are half-open `[start, end)`); `"start"` on its first day. There is no default.
/// @returns One `{ date, period_id, cell }` row per result period, with exact amounts and currencies.
///
/// # Errors
///
/// Throws with kind `not_found` if `nodeId` is not in the result, and kind
/// `validation` if an input is malformed, the result has no period plan or
/// `convention` is not `"start"` / `"end"`.
#[wasm_bindgen(js_name = nodeToDatedSchedule, unchecked_return_type = "generated.statements.DatedCell[]")]
pub fn node_to_dated_schedule(
    #[wasm_bindgen(
        js_name = "resultJson",
        unchecked_param_type = "StatementResult | string"
    )]
    result_json: JsValue,
    #[wasm_bindgen(js_name = "nodeId", unchecked_param_type = "string")] node_id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "'end' | 'start'")] convention: JsValue,
) -> Result<JsValue, JsValue> {
    let result: finstack_quant_statements::evaluator::StatementResult =
        from_js_json(&result_json, "resultJson")?;
    let convention = js_string(&convention, "convention")?
        .parse()
        .map_err(to_js_err)?;
    let rows = finstack_quant_statements::evaluator::node_to_dated_schedule(
        &result,
        &js_string(&node_id, "nodeId")?,
        convention,
    )
    .map_err(to_js_err)?;
    crate::utils::to_js_value(&rows)
}

/// Read one monetary node as signed economic cash flows for pricing.
///
/// Free-function twin of Python `StatementResult.to_pricing_flows` (Rust
/// `evaluator::node_to_pricing_flows`). Every requested period must hold an
/// exact amount in the expected currency. Positive means cash received: an
/// `outflow_magnitude` node is negated once. No FX and no sign inference is
/// applied.
/// @param resultJson - The `StatementResult` returned by `Evaluator.evaluate` / `evaluateWithMarket` (object or JSON).
/// @param selection - The node (`node_id`), expected `currency`, `sign` (`"inflow_positive"` or `"outflow_magnitude"`), required `periods` and `date_convention` (`"start"` or `"end"`).
/// @returns `[isoDate, money]` pairs in timeline order, one per requested period.
///
/// # Errors
///
/// Throws with kind `not_found` if the node is not in the result. Throws with
/// kind `validation` if an input is malformed, the result has no period plan,
/// a requested period is repeated or not in the result, a cell is missing,
/// masked, unavailable or a scalar, an amount is in another currency, or an
/// outflow magnitude is negative.
#[wasm_bindgen(js_name = nodeToPricingFlows, unchecked_return_type = "[string, generated.statements.Money][]")]
pub fn node_to_pricing_flows(
    #[wasm_bindgen(
        js_name = "resultJson",
        unchecked_param_type = "StatementResult | string"
    )]
    result_json: JsValue,
    #[wasm_bindgen(unchecked_param_type = "generated.statements.PricingFlowSelection | string")]
    selection: JsValue,
) -> Result<JsValue, JsValue> {
    let result: finstack_quant_statements::evaluator::StatementResult =
        from_js_json(&result_json, "resultJson")?;
    let selection: finstack_quant_statements::evaluator::PricingFlowSelection =
        from_js_json(&selection, "selection")?;
    let rows: Vec<(String, finstack_quant_core::money::Money)> =
        finstack_quant_statements::evaluator::node_to_pricing_flows(&result, &selection)
            .map_err(to_js_err)?
            .into_iter()
            .map(|(date, amount)| (crate::utils::date_to_iso(date), amount))
            .collect();
    crate::utils::to_js_value(&rows)
}

/// Probability that a metric exceeds a threshold in any forecast period.
///
/// Free-function twin of Python `MonteCarloResults.breach_probability` (Rust
/// `MonteCarloResults::breach_probability`). A path counts once when any
/// forecast-period value is strictly greater than the threshold; for a floor,
/// simulate a derived node that flips the sign. Per-path values come from the
/// result's `path_data` table, so the simulation must run with
/// `include_path_data: true`.
/// @param resultsJson - `MonteCarloResults` returned by `Evaluator.evaluateMonteCarlo` (object or JSON).
/// @param metric - A requested metric's node identifier.
/// @param threshold - Finite upper threshold in the metric's own units: a number for a scalar metric, or `{amount, currency}` money in the metric's currency for a monetary one.
/// @returns Fraction of paths that breach in at least one forecast period, or `undefined` when no path samples are available: the run is incomplete, the metric was not requested, or the result carries no `path_data`.
///
/// # Errors
///
/// Throws with kind `validation` if the results input is malformed, the
/// threshold is not a finite number or money in the metric's units, or the
/// path table does not hold exactly one finite sample for every path and
/// forecast period.
#[wasm_bindgen(js_name = monteCarloBreachProbability)]
pub fn monte_carlo_breach_probability(
    #[wasm_bindgen(
        js_name = "resultsJson",
        unchecked_param_type = "MonteCarloResults | string"
    )]
    results_json: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] metric: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number | generated.statements.Money")]
    threshold: JsValue,
) -> Result<Option<f64>, JsValue> {
    let results: finstack_quant_statements::evaluator::MonteCarloResults =
        serde_json::from_str(&json_text(&results_json, "resultsJson")?).map_err(to_js_err)?;
    // A number is a scalar threshold; anything else is read as money.
    let threshold = match threshold.as_f64() {
        Some(value) => finstack_quant_statements::types::AmountOrScalar::Scalar(value),
        None => crate::utils::input::from_js_json(&threshold, "threshold")?,
    };
    results
        .breach_probability(&js_string(&metric, "metric")?, threshold)
        .map_err(to_js_err)
}

/// Percentile time series of one metric across the forecast periods.
///
/// Free-function twin of Python `MonteCarloResults.percentile_by_period` (Rust
/// `MonteCarloResults::percentile_by_period`): looks up a percentile that the
/// simulation was configured to report.
/// @param resultsJson - `MonteCarloResults` returned by `Evaluator.evaluateMonteCarlo` (object or JSON).
/// @param metric - Node identifier to read.
/// @param percentile - Percentile as a fraction in `[0, 1]` (e.g. `0.95`); must be one of the configured percentiles.
/// @returns Object mapping period id (e.g. `"2025Q1"`) to the percentile value in the metric's own units, or `undefined` when the metric or percentile is not in the results.
///
/// # Errors
///
/// Throws with kind `validation` if the results input is malformed, and kind
/// `invalid_type` if `percentile` is not a number.
#[wasm_bindgen(js_name = monteCarloPercentileByPeriod, unchecked_return_type = "Record<string, number> | undefined")]
pub fn monte_carlo_percentile_by_period(
    #[wasm_bindgen(
        js_name = "resultsJson",
        unchecked_param_type = "MonteCarloResults | string"
    )]
    results_json: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] metric: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] percentile: JsValue,
) -> Result<JsValue, JsValue> {
    let results: finstack_quant_statements::evaluator::MonteCarloResults =
        serde_json::from_str(&json_text(&results_json, "resultsJson")?).map_err(to_js_err)?;
    match results.percentile_by_period(
        &js_string(&metric, "metric")?,
        crate::utils::input::js_f64(&percentile, "percentile")?,
    ) {
        Some(series) => crate::utils::to_js_value(&series),
        None => Ok(JsValue::UNDEFINED),
    }
}

/// Export a statement result as a long-format table.
///
/// Free-function twin of Python `StatementResult.to_arrow_long`
/// (Rust `StatementResult::to_table_long`): the exact table, with one row per
/// node and period of the result's plan, including cells without a value.
/// @param resultJson - The `StatementResult` returned by `Evaluator.evaluate` / `evaluateWithMarket` (object or JSON).
/// @returns `TableEnvelope` with columns `node_id`, `period_id`, `status`, `value`, `amount`, `currency`, `value_type`, `reason`, `source`. `status` is `value`, `missing`, `masked` or `unavailable`; `value` is the float view of a value cell and null otherwise; `amount` is the exact decimal text of a monetary value cell and `currency` its ISO code, both null for scalar and value-less cells; `reason` says why an unavailable cell has no value; `source` is the value source selected for the cell (`value`, `forecast` or `formula`), null when none was.
///
/// # Errors
///
/// Throws with kind `validation` if the result input is malformed or table
/// construction fails.
#[wasm_bindgen(js_name = statementResultToTableLong, unchecked_return_type = "TableEnvelope")]
pub fn statement_result_to_table_long(
    #[wasm_bindgen(
        js_name = "resultJson",
        unchecked_param_type = "StatementResult | string"
    )]
    result_json: JsValue,
) -> Result<JsValue, JsValue> {
    let result: finstack_quant_statements::evaluator::StatementResult =
        serde_json::from_str(&json_text(&result_json, "resultJson")?).map_err(to_js_err)?;
    crate::utils::to_js_table_numeric(&result.to_table_long().map_err(to_js_err)?)
}

/// Export a statement result as a wide-format table.
///
/// Free-function twin of Python `StatementResult.to_arrow_wide`
/// (Rust `StatementResult::to_table_wide`): the approximate numeric view, with
/// one row per period of the result's plan and one column per node in
/// declaration order.
/// @param resultJson - The `StatementResult` returned by `Evaluator.evaluate` / `evaluateWithMarket` (object or JSON).
/// @returns `TableEnvelope` with a `period_id` column followed by one nullable number column per node; a cell without a value is `null`, never zero. Each node column's metadata carries its units: `value_type` (`scalar` or `monetary`) and, for money, `currency`.
///
/// # Errors
///
/// Throws with kind `validation` if the result input is malformed or table
/// construction fails.
#[wasm_bindgen(js_name = statementResultToTableWide, unchecked_return_type = "TableEnvelope")]
pub fn statement_result_to_table_wide(
    #[wasm_bindgen(
        js_name = "resultJson",
        unchecked_param_type = "StatementResult | string"
    )]
    result_json: JsValue,
) -> Result<JsValue, JsValue> {
    let result: finstack_quant_statements::evaluator::StatementResult =
        serde_json::from_str(&json_text(&result_json, "resultJson")?).map_err(to_js_err)?;
    crate::utils::to_js_table_numeric(&result.to_table_wide().map_err(to_js_err)?)
}

/// Canonical content hash of a financial model.
///
/// Free-function twin of Python `FinancialModelSpec.content_hash` (Rust
/// `FinancialModelSpec::content_hash`): the model is loaded and validated
/// through `FinancialModelSpec::from_json`, then hashed over its canonical
/// JSON, so the hash does not depend on key order or number spelling of the
/// typed fields. Free-form `meta`/`params` maps keep their JSON spelling.
/// @param modelJson - `FinancialModelSpec` (object or JSON).
/// @returns `"sha256:<hex>"` content hash.
///
/// # Errors
///
/// Throws with kind `validation` if the model is malformed or fails semantic
/// validation, or contains a non-finite number.
#[wasm_bindgen(js_name = financialModelContentHash)]
pub fn financial_model_content_hash(
    #[wasm_bindgen(js_name = "modelJson", unchecked_param_type = "JsonInput")] model_json: JsValue,
) -> Result<String, JsValue> {
    crate::api::statements::load_model(&model_json, "modelJson")?
        .content_hash()
        .map_err(to_js_err)
}

/// Parse a DSL formula and return its canonical source text.
///
/// The formula is parsed into the statements AST and rendered back through
/// the AST's `Display`: whitespace normalised, operators spaced, logical
/// negation written `!`, and parentheses kept only where precedence requires
/// them. `!` binds like unary minus (`!a > b` is `(!a) > b`), keyword `not`
/// binds outside a comparison, and a nested comparison needs explicit
/// parentheses. Parsing the returned
/// text again yields the same AST, so it is a stable form for previewing,
/// diffing, or hashing formulas. Mirrors Python `parse_formula`.
///
/// # Errors
///
/// Rejects trailing tokens, malformed or incomplete syntax, an identifier
/// with an empty dot-separated segment, or a formula over 64 levels of nesting
/// or 256 terms.
/// @param formula - Financial-model formula string to parse into its canonical expression representation.
/// @returns Canonical formula text, e.g. `"(revenue - cogs) / revenue"`.
#[wasm_bindgen(js_name = parseFormula)]
pub fn parse_formula(
    #[wasm_bindgen(unchecked_param_type = "string")] formula: JsValue,
) -> Result<String, JsValue> {
    let formula: &str = &js_string(&formula, "formula")?;
    let ast = finstack_quant_statements::dsl::parse_formula(formula).map_err(to_js_err)?;
    Ok(ast.to_string())
}

/// Parse and compile a DSL formula, throwing if either step fails.
///
/// Compilation lowers the AST onto the core expression engine and rejects
/// unsupported functions, wrong arities (`cumsum`, `cumprod`, `cummin`,
/// `cummax`, `std`, `var` and `median` take exactly one argument), malformed
/// capital-structure references and negative literal offsets to `lag`,
/// `shift`, `diff`, `pct_change` or `growth_rate`, in every branch including
/// one evaluation would skip. Returns `undefined` when the
/// formula is valid; an invalid formula throws a `FinstackError`, so
/// `if (parseAndCompile(f))` is not a validity check. Mirrors Python
/// `parse_and_compile`.
///
/// # Errors
///
/// Rejects any formula that cannot be parsed as one complete DSL expression or
/// compiled because it contains an unsupported component, function, or
/// operator form.
/// @param formula - Financial-model formula string to parse and validate without evaluation.
#[wasm_bindgen(js_name = parseAndCompile)]
pub fn parse_and_compile(
    #[wasm_bindgen(unchecked_param_type = "string")] formula: JsValue,
) -> Result<(), JsValue> {
    let formula: &str = &js_string(&formula, "formula")?;
    finstack_quant_statements::dsl::parse_and_compile(formula).map_err(to_js_err)?;
    Ok(())
}
