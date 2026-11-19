//! Model templates applied to active builders.

use crate::api::statements::JsModelBuilder;
use crate::utils::input::{
    from_js_json, js_f64, js_f64_seq, js_opt_string_seq, js_string, js_string_seq,
};
use crate::utils::to_js_err;
use finstack_quant_statements_analytics::templates::{real_estate, roll_forward, vintage};
use wasm_bindgen::prelude::*;

fn refs(names: &[String]) -> Vec<&str> {
    names.iter().map(String::as_str).collect()
}

/// Add a vintage buildup to an active builder.
/// @param builder - ModelBuilder receiving the generated cohort total.
/// @param name - Identifier of the outstanding-balance node to create.
/// @param newVolumeNode - Identifier of the node holding each period's new volume.
/// @param decayCurve - Finite retention multipliers by elapsed model period, starting at inception.
/// @throws TypeError for malformed arguments or FinstackError for a consumed builder or invalid coefficient.
#[wasm_bindgen(js_name = addVintageBuildup)]
pub fn add_vintage_buildup(
    builder: &mut JsModelBuilder,
    #[wasm_bindgen(unchecked_param_type = "string")] name: JsValue,
    #[wasm_bindgen(js_name = "newVolumeNode", unchecked_param_type = "string")]
    new_volume_node: JsValue,
    #[wasm_bindgen(
        js_name = "decayCurve",
        unchecked_param_type = "number[] | Float64Array"
    )]
    decay_curve: JsValue,
) -> Result<(), JsValue> {
    vintage::add_vintage_buildup(
        builder.builder_mut()?,
        &js_string(&name, "name")?,
        &js_string(&new_volume_node, "newVolumeNode")?,
        &js_f64_seq(&decay_curve, "decayCurve")?,
    )
    .map_err(to_js_err)
}

/// Add a roll-forward with a zero opening balance.
/// @param builder - Active ModelBuilder receiving the beginning and ending balance nodes.
/// @param name - Prefix for the generated balance node identifiers.
/// @param increases - Identifiers of flows added to the balance each period.
/// @param decreases - Identifiers of flows deducted from the balance each period.
/// @throws If a parameter has the wrong type or the builder has been consumed.
/// Generated node identifiers, formulas and references are validated by build.
#[wasm_bindgen(js_name = addRollForward)]
pub fn add_roll_forward(
    builder: &mut JsModelBuilder,
    #[wasm_bindgen(unchecked_param_type = "string")] name: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string[]")] increases: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string[]")] decreases: JsValue,
) -> Result<(), JsValue> {
    let increases = js_string_seq(&increases, "increases")?;
    let decreases = js_string_seq(&decreases, "decreases")?;
    roll_forward::add_roll_forward(
        builder.builder_mut()?,
        &js_string(&name, "name")?,
        &refs(&increases),
        &refs(&decreases),
    );
    Ok(())
}

/// Add a roll-forward with an explicit opening balance.
/// @param builder - Active ModelBuilder receiving the beginning and ending balance nodes.
/// @param name - Prefix for the generated balance node identifiers.
/// @param increases - Identifiers of flows added to the balance each period.
/// @param decreases - Identifiers of flows deducted from the balance each period.
/// @param opening - First-period beginning balance in the flows' own units.
/// @throws If a parameter has the wrong type or the builder has been consumed.
/// Generated node identifiers, formulas and references are validated by build.
#[wasm_bindgen(js_name = addRollForwardWithOpening)]
pub fn add_roll_forward_with_opening(
    builder: &mut JsModelBuilder,
    #[wasm_bindgen(unchecked_param_type = "string")] name: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string[]")] increases: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string[]")] decreases: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] opening: JsValue,
) -> Result<(), JsValue> {
    let increases = js_string_seq(&increases, "increases")?;
    let decreases = js_string_seq(&decreases, "decreases")?;
    roll_forward::add_roll_forward_with_opening(
        builder.builder_mut()?,
        &js_string(&name, "name")?,
        &refs(&increases),
        &refs(&decreases),
        js_f64(&opening, "opening")?,
    );
    Ok(())
}

/// Add a net-operating-income buildup: total revenue, total expenses and NOI.
///
/// Twin of Python `add_noi_buildup` (Rust `add_noi_buildup`).
/// @param builder - Active ModelBuilder receiving the generated nodes.
/// @param totalRevenueNode - Identifier of the total-revenue node to create.
/// @param revenueNodes - Existing revenue line nodes summed into the total.
/// @param totalExpensesNode - Identifier of the total-expenses node to create.
/// @param expenseNodes - Existing operating-expense line nodes summed into the total.
/// @param noiNode - Identifier of the NOI node to create (`total revenue - total expenses`).
///
/// # Errors
///
/// Throws with kind `validation` if the builder has been consumed, a node list is
/// empty, a node identifier is invalid, or a generated formula
/// does not compile.
#[wasm_bindgen(js_name = addNoiBuildup)]
pub fn add_noi_buildup(
    builder: &mut JsModelBuilder,
    #[wasm_bindgen(js_name = "totalRevenueNode", unchecked_param_type = "string")]
    total_revenue_node: JsValue,
    #[wasm_bindgen(js_name = "revenueNodes", unchecked_param_type = "string[]")]
    revenue_nodes: JsValue,
    #[wasm_bindgen(js_name = "totalExpensesNode", unchecked_param_type = "string")]
    total_expenses_node: JsValue,
    #[wasm_bindgen(js_name = "expenseNodes", unchecked_param_type = "string[]")]
    expense_nodes: JsValue,
    #[wasm_bindgen(js_name = "noiNode", unchecked_param_type = "string")] noi_node: JsValue,
) -> Result<(), JsValue> {
    let revenue_nodes = js_string_seq(&revenue_nodes, "revenueNodes")?;
    let expense_nodes = js_string_seq(&expense_nodes, "expenseNodes")?;
    real_estate::add_noi_buildup(
        builder.builder_mut()?,
        &js_string(&total_revenue_node, "totalRevenueNode")?,
        &refs(&revenue_nodes),
        &js_string(&total_expenses_node, "totalExpensesNode")?,
        &refs(&expense_nodes),
        &js_string(&noi_node, "noiNode")?,
    )
    .map_err(to_js_err)
}

/// Add a net-cash-flow buildup: NOI less capital expenditure.
///
/// Twin of Python `add_ncf_buildup` (Rust `add_ncf_buildup`).
/// @param builder - Active ModelBuilder receiving the generated nodes.
/// @param noiNode - Existing NOI node.
/// @param capexNodes - Existing capital-expenditure nodes subtracted from NOI; an empty list makes NCF equal NOI.
/// @param ncfNode - Identifier of the NCF node to create.
///
/// # Errors
///
/// Throws with kind `validation` if the builder has been consumed, a node identifier
/// is invalid, or the generated formula does not compile.
#[wasm_bindgen(js_name = addNcfBuildup)]
pub fn add_ncf_buildup(
    builder: &mut JsModelBuilder,
    #[wasm_bindgen(js_name = "noiNode", unchecked_param_type = "string")] noi_node: JsValue,
    #[wasm_bindgen(js_name = "capexNodes", unchecked_param_type = "string[]")] capex_nodes: JsValue,
    #[wasm_bindgen(js_name = "ncfNode", unchecked_param_type = "string")] ncf_node: JsValue,
) -> Result<(), JsValue> {
    let capex_nodes = js_string_seq(&capex_nodes, "capexNodes")?;
    real_estate::add_ncf_buildup(
        builder.builder_mut()?,
        &js_string(&noi_node, "noiNode")?,
        &refs(&capex_nodes),
        &js_string(&ncf_node, "ncfNode")?,
    )
    .map_err(to_js_err)
}

/// Add a lease-by-lease rent roll: gross rent, free rent, vacancy and effective rent.
///
/// Twin of Python `add_rent_roll` (Rust `add_rent_roll`).
/// @param builder - Active ModelBuilder receiving the generated nodes.
/// @param leases - Array of `LeaseSpec` objects, at least one (array or JSON).
/// @param nodes - Optional `RentRollOutputNodes` naming the generated total nodes; omitted uses the Rust default names (object or JSON).
///
/// # Errors
///
/// Throws with kind `validation` if an input is malformed, `leases` is empty,
/// a lease fails validation, or a generated node identifier or formula is
/// invalid.
#[wasm_bindgen(js_name = addRentRoll)]
pub fn add_rent_roll(
    builder: &mut JsModelBuilder,
    #[wasm_bindgen(unchecked_param_type = "generated.statements_analytics.LeaseSpec[] | string")]
    leases: JsValue,
    #[wasm_bindgen(
        unchecked_optional_param_type = "generated.statements_analytics.RentRollOutputNodes | string | null"
    )]
    nodes: Option<JsValue>,
) -> Result<(), JsValue> {
    let leases: Vec<real_estate::LeaseSpec> = from_js_json(&leases, "leases")?;
    let nodes: real_estate::RentRollOutputNodes =
        crate::utils::wire::js_opt_wire(nodes.as_ref(), "nodes")?.unwrap_or_default();
    real_estate::add_rent_roll(builder.builder_mut()?, &leases, &nodes).map_err(to_js_err)
}

/// Add a full property operating statement: rent roll, other income, EGI, opex, NOI, capex and NCF.
///
/// Twin of Python `add_property_operating_statement` (Rust
/// `add_property_operating_statement`).
/// @param builder - Active ModelBuilder receiving the generated nodes.
/// @param leases - Array of `LeaseSpec` objects, at least one (array or JSON).
/// @param otherIncomeNodes - Existing other-income nodes added to effective rent; omitted means none.
/// @param opexNodes - Existing operating-expense nodes; omitted means none.
/// @param capexNodes - Existing capital-expenditure nodes; omitted means none.
/// @param managementFee - Optional `ManagementFeeSpec` (`rate` as a decimal of its `base`); omitted adds no fee (object or JSON).
/// @param nodes - Optional `PropertyTemplateNodes` naming the generated nodes; omitted uses the Rust default names (object or JSON).
///
/// # Errors
///
/// Throws with kind `validation` if an input is malformed, `leases` is empty,
/// a lease fails validation, or a generated node identifier or formula is
/// invalid.
#[wasm_bindgen(js_name = addPropertyOperatingStatement)]
pub fn add_property_operating_statement(
    builder: &mut JsModelBuilder,
    #[wasm_bindgen(unchecked_param_type = "generated.statements_analytics.LeaseSpec[] | string")]
    leases: JsValue,
    #[wasm_bindgen(
        js_name = "otherIncomeNodes",
        unchecked_optional_param_type = "string[] | null"
    )]
    other_income_nodes: Option<JsValue>,
    #[wasm_bindgen(
        js_name = "opexNodes",
        unchecked_optional_param_type = "string[] | null"
    )]
    opex_nodes: Option<JsValue>,
    #[wasm_bindgen(
        js_name = "capexNodes",
        unchecked_optional_param_type = "string[] | null"
    )]
    capex_nodes: Option<JsValue>,
    #[wasm_bindgen(
        js_name = "managementFee",
        unchecked_optional_param_type = "generated.statements_analytics.ManagementFeeSpec | string | null"
    )]
    management_fee: Option<JsValue>,
    #[wasm_bindgen(
        unchecked_optional_param_type = "generated.statements_analytics.PropertyTemplateNodes | string | null"
    )]
    nodes: Option<JsValue>,
) -> Result<(), JsValue> {
    let leases: Vec<real_estate::LeaseSpec> = from_js_json(&leases, "leases")?;
    let other_income =
        js_opt_string_seq(other_income_nodes.as_ref(), "otherIncomeNodes")?.unwrap_or_default();
    let opex = js_opt_string_seq(opex_nodes.as_ref(), "opexNodes")?.unwrap_or_default();
    let capex = js_opt_string_seq(capex_nodes.as_ref(), "capexNodes")?.unwrap_or_default();
    let management_fee: Option<real_estate::ManagementFeeSpec> =
        crate::utils::wire::js_opt_wire(management_fee.as_ref(), "managementFee")?;
    let nodes: real_estate::PropertyTemplateNodes =
        crate::utils::wire::js_opt_wire(nodes.as_ref(), "nodes")?.unwrap_or_default();
    real_estate::add_property_operating_statement(
        builder.builder_mut()?,
        &leases,
        &refs(&other_income),
        &refs(&opex),
        &refs(&capex),
        management_fee,
        &nodes,
    )
    .map_err(to_js_err)
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
pub fn validate_lease_spec_json(
    #[wasm_bindgen(unchecked_param_type = "JsonInput")] json: JsValue,
) -> Result<String, JsValue> {
    crate::utils::input::validated_json(&json, real_estate::LeaseSpec::validate)
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
pub fn validate_renewal_spec_json(
    #[wasm_bindgen(unchecked_param_type = "JsonInput")] json: JsValue,
) -> Result<String, JsValue> {
    crate::utils::input::validated_json(&json, real_estate::RenewalSpec::validate)
}
