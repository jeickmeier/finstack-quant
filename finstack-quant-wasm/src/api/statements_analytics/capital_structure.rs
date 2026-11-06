//! Debt stack by seniority over an evaluated capital structure.

use super::parse_model;
use crate::utils::input::from_js_json;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_statements::evaluator::StatementResult;
use finstack_quant_statements_analytics::analysis::{
    CapitalStructureSummary, CapitalStructureSummaryConfig,
};
use wasm_bindgen::prelude::*;

/// Summarize the debt stack by seniority at one period (mirrors Rust `capital_structure_summary`).
///
/// Only bonds, convertible bonds, term loans and revolvers are debt; swaps,
/// cap/floors and swaptions are left out of balances and interest, including
/// when a payment class names them. Seniority is the rank of the waterfall's
/// payment classes; a model with no waterfall or no payment classes has one
/// implicit class, every tranche at rank `0` with a null class.
///
/// LTM EBITDA and LTM interest sum the trailing periods of one year ending at
/// the reporting period (4 quarters, 12 months), the slots `ttm()` reads in a
/// formula. A window with a slot missing from the model or a slot where the
/// EBITDA node has no value is never annualised or partially summed: LTM EBITDA
/// and every ratio are null and `ltm_periods_found` says how many slots held. A
/// zero or negative LTM EBITDA also gives null ratios, not a negative multiple.
/// Interest is cash plus PIK.
///
/// @param results - Evaluated `StatementResult` (object or JSON) holding `cs_cashflows` and both configured nodes. A scalar EBITDA or cash node is read as an amount in the debt currency.
/// @param model - `FinancialModelSpec` (object or JSON) whose capital structure gives the instruments, their declaration order and the payment classes, and whose periods bound the LTM window.
/// @param config - `CapitalStructureSummaryConfig` (object or JSON): `period` (the reporting period, such as `"2025Q4"`), `ebitda_node` (required; the node holding EBITDA for each single period, never inferred from a conventional name) and `cash_node` (optional; the node holding the period-end cash balance, omitted means null cash, net debt and net leverage).
/// @returns `CapitalStructureSummary` plain object: `as_of`, `currency`, `total_debt`, `cash`, `net_debt` and `ltm_ebitda` as money, `ltm_periods_required`, `ltm_periods_found`, `total_leverage`, `net_leverage` and `total_coverage` in turns, `tranches` by rank then declaration order (`id`, `kind`, `rank`, `payment_class`, `balance` with a revolver at its drawn balance, `share_of_debt` as a decimal, `maturity`, `cumulative_debt`, `leverage` and `interest_coverage` through the class, `ltm_interest`) and `maturity_wall` by calendar year of final maturity, ascending (`year`, `amount`, `share_of_debt`), using the balance at the period and ignoring any scheduled amortization after it.
/// @throws If an argument is malformed, the model has no capital structure or no debt instruments once hedges are excluded, `cs_cashflows` is absent, the period is not in the model, a configured node is not in the results or the cash node has no value at the period, an instrument has no cashflows at the period or is in no class while payment classes are declared, a consumed value or ratio is non-finite, tranches are in more than one currency (no FX is applied), or the EBITDA or cash node is monetary in another currency (kind `validation`).
#[wasm_bindgen(js_name = capitalStructureSummary)]
pub fn capital_structure_summary(
    results: JsValue,
    model: JsValue,
    config: JsValue,
) -> Result<JsValue, JsValue> {
    let results: StatementResult = from_js_json(&results, "results")?;
    let model = parse_model(&model)?;
    let config: CapitalStructureSummaryConfig = from_js_json(&config, "config")?;
    to_js_value(
        &finstack_quant_statements_analytics::analysis::capital_structure_summary(
            &results, &model, &config,
        )
        .map_err(to_js_err)?,
    )
}

/// One row per tranche of a capital structure summary (mirrors Rust `CapitalStructureSummary::to_table`).
///
/// Python reaches the same table through `CapitalStructureSummary.to_dataframe`.
/// @param summary - `CapitalStructureSummary` plain object or JSON string, as `capitalStructureSummary` returns it.
/// @returns `TableEnvelope` in tranche order (rank, then model declaration order) with columns `id`, `kind` (`bond`, `convertible_bond`, `term_loan` or `revolving_credit`), `rank` (integer), `payment_class` (nullable string), `balance`, `share_of_debt` (nullable decimal), `maturity` (ISO date string), `cumulative_debt`, `leverage` and `interest_coverage` (nullable, in turns) and `ltm_interest` (nullable); amounts are floats in the summary's `currency`, which the table does not state. The columns remain when there are no tranches.
/// @throws Error - Throws with kind `validation` if `summary` does not match the `CapitalStructureSummary` schema.
#[wasm_bindgen(js_name = capitalStructureSummaryTable)]
pub fn capital_structure_summary_table(summary: JsValue) -> Result<JsValue, JsValue> {
    let summary: CapitalStructureSummary = from_js_json(&summary, "summary")?;
    to_js_value(&summary.to_table())
}
