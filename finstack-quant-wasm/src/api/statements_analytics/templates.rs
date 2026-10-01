//! Model-template bindings: each takes a model, applies one Rust template
//! through `ModelBuilder::from_spec`, and returns the extended model.

use crate::utils::input::{
    from_js_json, js_f64, js_f64_seq, js_opt_string_seq, js_string, js_string_seq, json_text,
};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_statements::builder::{ModelBuilder, Ready};
use finstack_quant_statements::FinancialModelSpec;
use finstack_quant_statements_analytics::templates::{real_estate, roll_forward, vintage};
use wasm_bindgen::prelude::*;

fn builder(model: &JsValue) -> Result<ModelBuilder<Ready>, JsValue> {
    let model = FinancialModelSpec::from_json(&json_text(model, "model")?).map_err(to_js_err)?;
    ModelBuilder::from_spec(model).map_err(to_js_err)
}

fn finish(builder: ModelBuilder<Ready>) -> Result<JsValue, JsValue> {
    to_js_value(&builder.build().map_err(to_js_err)?)
}

fn refs(names: &[String]) -> Vec<&str> {
    names.iter().map(String::as_str).collect()
}

fn present(value: Option<JsValue>) -> Option<JsValue> {
    value.filter(|v| !v.is_null() && !v.is_undefined())
}

/// Add a vintage (cohort) buildup: each period's new volume decays along a curve.
///
/// Twin of Python `add_vintage_buildup` (Rust `add_vintage_buildup`). Adds
/// the node `name`, the sum over lags of `newVolume[t - lag] * decayCurve[lag]`.
/// @param model - `FinancialModelSpec` to extend (object or JSON).
/// @param name - Identifier of the outstanding-balance node to create.
/// @param new_volume_node - Existing node holding each period's new volume.
/// @param decay_curve - Fraction of a cohort still outstanding `lag` periods after origination, as decimals starting at lag 0 (e.g. `[1.0, 0.8, 0.5]`).
/// @returns The extended `FinancialModelSpec` plain object.
///
/// # Errors
///
/// Throws with kind `validation` if the model is malformed, a node identifier
/// is invalid or duplicated, or the generated formula does not compile.
#[wasm_bindgen(js_name = addVintageBuildup)]
pub fn add_vintage_buildup(
    model: JsValue,
    name: JsValue,
    new_volume_node: JsValue,
    decay_curve: JsValue,
) -> Result<JsValue, JsValue> {
    let builder = vintage::add_vintage_buildup(
        builder(&model)?,
        &js_string(&name, "name")?,
        &js_string(&new_volume_node, "newVolumeNode")?,
        &js_f64_seq(&decay_curve, "decayCurve")?,
    )
    .map_err(to_js_err)?;
    finish(builder)
}

/// Add a roll-forward (corkscrew) structure with a zero opening balance.
///
/// Twin of Python `add_roll_forward` (Rust `add_roll_forward`). Adds
/// `<name>_beg` and `<name>_end`, where
/// `end = beg + sum(increases) - sum(decreases)` and each period's opening
/// balance is the prior period's close.
/// @param model - `FinancialModelSpec` to extend (object or JSON).
/// @param name - Prefix of the generated `<name>_beg` and `<name>_end` nodes.
/// @param increases - Existing nodes added to the balance each period.
/// @param decreases - Existing nodes subtracted from the balance each period.
/// @returns The extended `FinancialModelSpec` plain object.
///
/// # Errors
///
/// Throws with kind `validation` if the model is malformed, a node identifier
/// is invalid or duplicated, or a generated formula does not compile.
#[wasm_bindgen(js_name = addRollForward)]
pub fn add_roll_forward(
    model: JsValue,
    name: JsValue,
    increases: JsValue,
    decreases: JsValue,
) -> Result<JsValue, JsValue> {
    let increases = js_string_seq(&increases, "increases")?;
    let decreases = js_string_seq(&decreases, "decreases")?;
    let builder = roll_forward::add_roll_forward(
        builder(&model)?,
        &js_string(&name, "name")?,
        &refs(&increases),
        &refs(&decreases),
    )
    .map_err(to_js_err)?;
    finish(builder)
}

/// Add a roll-forward (corkscrew) structure with an explicit opening balance.
///
/// Twin of Python `add_roll_forward_with_opening` (Rust
/// `add_roll_forward_with_opening`).
/// @param model - `FinancialModelSpec` to extend (object or JSON).
/// @param name - Prefix of the generated `<name>_beg` and `<name>_end` nodes.
/// @param increases - Existing nodes added to the balance each period.
/// @param decreases - Existing nodes subtracted from the balance each period.
/// @param opening - Opening balance of the first period, in the balance's own units.
/// @returns The extended `FinancialModelSpec` plain object.
///
/// # Errors
///
/// Throws with kind `validation` if the model is malformed, a node identifier
/// is invalid or duplicated, or a generated formula does not compile.
#[wasm_bindgen(js_name = addRollForwardWithOpening)]
pub fn add_roll_forward_with_opening(
    model: JsValue,
    name: JsValue,
    increases: JsValue,
    decreases: JsValue,
    opening: JsValue,
) -> Result<JsValue, JsValue> {
    let increases = js_string_seq(&increases, "increases")?;
    let decreases = js_string_seq(&decreases, "decreases")?;
    let builder = roll_forward::add_roll_forward_with_opening(
        builder(&model)?,
        &js_string(&name, "name")?,
        &refs(&increases),
        &refs(&decreases),
        js_f64(&opening, "opening")?,
    )
    .map_err(to_js_err)?;
    finish(builder)
}

/// Add a net-operating-income buildup: total revenue, total expenses and NOI.
///
/// Twin of Python `add_noi_buildup` (Rust `add_noi_buildup`).
/// @param model - `FinancialModelSpec` to extend (object or JSON).
/// @param total_revenue_node - Identifier of the total-revenue node to create.
/// @param revenue_nodes - Existing revenue line nodes summed into the total.
/// @param total_expenses_node - Identifier of the total-expenses node to create.
/// @param expense_nodes - Existing operating-expense line nodes summed into the total.
/// @param noi_node - Identifier of the NOI node to create (`total revenue - total expenses`).
/// @returns The extended `FinancialModelSpec` plain object.
///
/// # Errors
///
/// Throws with kind `validation` if the model is malformed, a node list is
/// empty, a node identifier is invalid or duplicated, or a generated formula
/// does not compile.
#[wasm_bindgen(js_name = addNoiBuildup)]
pub fn add_noi_buildup(
    model: JsValue,
    total_revenue_node: JsValue,
    revenue_nodes: JsValue,
    total_expenses_node: JsValue,
    expense_nodes: JsValue,
    noi_node: JsValue,
) -> Result<JsValue, JsValue> {
    let revenue_nodes = js_string_seq(&revenue_nodes, "revenueNodes")?;
    let expense_nodes = js_string_seq(&expense_nodes, "expenseNodes")?;
    let builder = real_estate::add_noi_buildup(
        builder(&model)?,
        &js_string(&total_revenue_node, "totalRevenueNode")?,
        &refs(&revenue_nodes),
        &js_string(&total_expenses_node, "totalExpensesNode")?,
        &refs(&expense_nodes),
        &js_string(&noi_node, "noiNode")?,
    )
    .map_err(to_js_err)?;
    finish(builder)
}

/// Add a net-cash-flow buildup: NOI less capital expenditure.
///
/// Twin of Python `add_ncf_buildup` (Rust `add_ncf_buildup`).
/// @param model - `FinancialModelSpec` to extend (object or JSON).
/// @param noi_node - Existing NOI node.
/// @param capex_nodes - Existing capital-expenditure nodes subtracted from NOI; an empty list makes NCF equal NOI.
/// @param ncf_node - Identifier of the NCF node to create.
/// @returns The extended `FinancialModelSpec` plain object.
///
/// # Errors
///
/// Throws with kind `validation` if the model is malformed, a node identifier
/// is invalid or duplicated, or the generated formula does not compile.
#[wasm_bindgen(js_name = addNcfBuildup)]
pub fn add_ncf_buildup(
    model: JsValue,
    noi_node: JsValue,
    capex_nodes: JsValue,
    ncf_node: JsValue,
) -> Result<JsValue, JsValue> {
    let capex_nodes = js_string_seq(&capex_nodes, "capexNodes")?;
    let builder = real_estate::add_ncf_buildup(
        builder(&model)?,
        &js_string(&noi_node, "noiNode")?,
        &refs(&capex_nodes),
        &js_string(&ncf_node, "ncfNode")?,
    )
    .map_err(to_js_err)?;
    finish(builder)
}

/// Add a lease-by-lease rent roll: gross rent, free rent, vacancy and effective rent.
///
/// Twin of Python `add_rent_roll` (Rust `add_rent_roll`).
/// @param model - `FinancialModelSpec` to extend (object or JSON).
/// @param leases - Array of `LeaseSpec` objects, at least one (array or JSON).
/// @param nodes - Optional `RentRollOutputNodes` naming the generated total nodes; omitted uses the Rust default names (object or JSON).
/// @returns The extended `FinancialModelSpec` plain object.
///
/// # Errors
///
/// Throws with kind `validation` if an input is malformed, `leases` is empty,
/// a lease fails validation, or a generated node identifier or formula is
/// invalid.
#[wasm_bindgen(js_name = addRentRoll)]
pub fn add_rent_roll(
    model: JsValue,
    leases: JsValue,
    nodes: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let leases: Vec<real_estate::LeaseSpec> = from_js_json(&leases, "leases")?;
    let nodes: real_estate::RentRollOutputNodes = match present(nodes) {
        Some(nodes) => from_js_json(&nodes, "nodes")?,
        None => real_estate::RentRollOutputNodes::default(),
    };
    let builder =
        real_estate::add_rent_roll(builder(&model)?, &leases, &nodes).map_err(to_js_err)?;
    finish(builder)
}

/// Add a full property operating statement: rent roll, other income, EGI, opex, NOI, capex and NCF.
///
/// Twin of Python `add_property_operating_statement` (Rust
/// `add_property_operating_statement`).
/// @param model - `FinancialModelSpec` to extend (object or JSON).
/// @param leases - Array of `LeaseSpec` objects, at least one (array or JSON).
/// @param other_income_nodes - Existing other-income nodes added to effective rent; omitted means none.
/// @param opex_nodes - Existing operating-expense nodes; omitted means none.
/// @param capex_nodes - Existing capital-expenditure nodes; omitted means none.
/// @param management_fee - Optional `ManagementFeeSpec` (`rate` as a decimal of its `base`); omitted adds no fee (object or JSON).
/// @param nodes - Optional `PropertyTemplateNodes` naming the generated nodes; omitted uses the Rust default names (object or JSON).
/// @returns The extended `FinancialModelSpec` plain object.
///
/// # Errors
///
/// Throws with kind `validation` if an input is malformed, `leases` is empty,
/// a lease fails validation, or a generated node identifier or formula is
/// invalid.
#[wasm_bindgen(js_name = addPropertyOperatingStatement)]
#[allow(clippy::too_many_arguments)]
pub fn add_property_operating_statement(
    model: JsValue,
    leases: JsValue,
    other_income_nodes: Option<JsValue>,
    opex_nodes: Option<JsValue>,
    capex_nodes: Option<JsValue>,
    management_fee: Option<JsValue>,
    nodes: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let leases: Vec<real_estate::LeaseSpec> = from_js_json(&leases, "leases")?;
    let other_income =
        js_opt_string_seq(other_income_nodes.as_ref(), "otherIncomeNodes")?.unwrap_or_default();
    let opex = js_opt_string_seq(opex_nodes.as_ref(), "opexNodes")?.unwrap_or_default();
    let capex = js_opt_string_seq(capex_nodes.as_ref(), "capexNodes")?.unwrap_or_default();
    let management_fee: Option<real_estate::ManagementFeeSpec> = present(management_fee)
        .map(|fee| from_js_json(&fee, "managementFee"))
        .transpose()?;
    let nodes: real_estate::PropertyTemplateNodes = match present(nodes) {
        Some(nodes) => from_js_json(&nodes, "nodes")?,
        None => real_estate::PropertyTemplateNodes::default(),
    };
    let builder = real_estate::add_property_operating_statement(
        builder(&model)?,
        &leases,
        &refs(&other_income),
        &refs(&opex),
        &refs(&capex),
        management_fee,
        &nodes,
    )
    .map_err(to_js_err)?;
    finish(builder)
}

/// Validate a `LeaseSpec` and return its canonical JSON.
///
/// JSON wire twin of Python `LeaseSpec.validate` (Rust `LeaseSpec::validate`).
/// @param json - `LeaseSpec` (object or JSON).
/// @returns Canonical lease JSON.
///
/// # Errors
///
/// Throws with kind `validation` if `json` is malformed or the lease is
/// invalid: an empty node identifier, a non-finite base rent, growth rate or
/// rent step, an occupancy outside `[0, 1]`, or an invalid renewal.
#[wasm_bindgen(js_name = validateLeaseSpecJson)]
pub fn validate_lease_spec_json(json: JsValue) -> Result<String, JsValue> {
    let spec: real_estate::LeaseSpec = from_js_json(&json, "json")?;
    spec.validate().map_err(to_js_err)?;
    serde_json::to_string(&spec).map_err(to_js_err)
}

/// Validate a `RenewalSpec` and return its canonical JSON.
///
/// JSON wire twin of Python `RenewalSpec.validate` (Rust
/// `RenewalSpec::validate`).
/// @param json - `RenewalSpec` (object or JSON).
/// @returns Canonical renewal JSON.
///
/// # Errors
///
/// Throws with kind `validation` if `json` is malformed or the renewal is
/// invalid: a zero term, a probability outside `[0, 1]`, or a non-positive
/// rent factor.
#[wasm_bindgen(js_name = validateRenewalSpecJson)]
pub fn validate_renewal_spec_json(json: JsValue) -> Result<String, JsValue> {
    let spec: real_estate::RenewalSpec = from_js_json(&json, "json")?;
    spec.validate().map_err(to_js_err)?;
    serde_json::to_string(&spec).map_err(to_js_err)
}
