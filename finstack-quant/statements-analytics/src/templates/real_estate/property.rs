//! Complete property operating statement template.

use super::buildup::sum_expr_or_zero;
use super::rent_roll::{LeaseSpec, RentRollOutputNodes, populate_rent_roll};
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::{Error, Result};

/// Basis for management fee calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum ManagementFeeBase {
    /// Fee is applied to EGI.
    #[default]
    Egi,
    /// Fee is applied to effective rent only (excludes other income).
    EffectiveRent,
}

/// Management fee specification.
///
/// `rate` is a decimal fraction, so `0.03` means `3%`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ManagementFeeSpec {
    /// Management fee rate as a decimal fraction (e.g., 0.03 for 3%).
    pub rate: f64,
    /// Fee base for calculation.
    #[serde(default)]
    pub base: ManagementFeeBase,
}

impl Default for ManagementFeeSpec {
    fn default() -> Self {
        Self {
            rate: 0.0,
            base: ManagementFeeBase::Egi,
        }
    }
}

/// Standard node ids for a full property operating statement template.
///
/// These ids define the aggregate outputs produced by
/// [`add_property_operating_statement`]. Per-lease detail nodes from the rent
/// roll remain derived from the individual lease ids.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PropertyTemplateNodes {
    /// Rent roll output nodes (PGI/free rent/vacancy/effective rent).
    #[serde(default)]
    pub rent_roll: RentRollOutputNodes,
    /// Total other income node id.
    #[serde(default = "default_other_income_total")]
    pub other_income_total_node: String,
    /// Effective gross income (EGI) node id.
    #[serde(default = "default_egi")]
    pub egi_node: String,
    /// Management fee node id (if configured).
    #[serde(default = "default_management_fee")]
    pub management_fee_node: String,
    /// Total operating expenses node id (includes management fee if enabled).
    #[serde(default = "default_opex_total")]
    pub opex_total_node: String,
    /// Net operating income (NOI) node id.
    #[serde(default = "default_noi")]
    pub noi_node: String,
    /// Total CapEx node id.
    #[serde(default = "default_capex_total")]
    pub capex_total_node: String,
    /// Net cash flow (NCF) node id: `noi - capex_total`.
    #[serde(default = "default_ncf")]
    pub ncf_node: String,
}

fn default_other_income_total() -> String {
    "other_income_total".into()
}
fn default_egi() -> String {
    "egi".into()
}
fn default_management_fee() -> String {
    "management_fee".into()
}
fn default_opex_total() -> String {
    "opex_total".into()
}
fn default_noi() -> String {
    "noi".into()
}
fn default_capex_total() -> String {
    "capex_total".into()
}
fn default_ncf() -> String {
    "ncf".into()
}

impl Default for PropertyTemplateNodes {
    fn default() -> Self {
        Self {
            rent_roll: RentRollOutputNodes::default(),
            other_income_total_node: default_other_income_total(),
            egi_node: default_egi(),
            management_fee_node: default_management_fee(),
            opex_total_node: default_opex_total(),
            noi_node: default_noi(),
            capex_total_node: default_capex_total(),
            ncf_node: default_ncf(),
        }
    }
}

/// Full property operating statement template:
/// - rent roll (PGI/free rent/vacancy/effective rent)
/// - EGI = effective rent + other income
/// - OpEx total (optionally adds management fee)
/// - NOI = EGI - OpEx
/// - CapEx total
/// - NCF = NOI - CapEx
///
/// `other_income_nodes`, `opex_nodes`, and `capex_nodes` are all assumed to be
/// positive per-period amounts. CapEx is subtracted from NOI when computing
/// NCF. Management fees are computed as a positive expense and added to OpEx.
///
/// # Arguments
///
/// * `builder` - Ready builder whose periods define the property model cadence
/// * `leases` - Rich lease specs used to build the rent roll
/// * `other_income_nodes` - Additional income nodes added to effective rent to
///   form EGI
/// * `opex_nodes` - Operating-expense nodes expressed as positive outflows
/// * `capex_nodes` - Capital-expenditure nodes expressed as positive outflows
/// * `management_fee` - Optional fee spec applied to EGI or effective rent
/// * `nodes` - Aggregate output node ids for the operating statement
///
/// # Errors
///
/// Returns an error if the rent roll cannot be generated, if management-fee
/// inputs are invalid, or if any generated node conflicts with existing model
/// state.
///
/// # References
///
/// - Property cashflow and discounting context: `docs/REFERENCES.md#hull-options-futures`
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::PeriodId;
/// use finstack_quant_statements::builder::ModelBuilder;
/// use finstack_quant_statements::types::AmountOrScalar;
/// use finstack_quant_statements_analytics::templates::real_estate::{
///     add_property_operating_statement, LeaseGrowthConvention, LeaseSpec,
///     ManagementFeeBase, ManagementFeeSpec, PropertyTemplateNodes,
/// };
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
/// let mut builder = {
///     let mut model_builder = ModelBuilder::new("property");
///     model_builder.periods("2025Q1..Q4", None)?;
///     model_builder.value("parking_income", &[(period, AmountOrScalar::scalar(5_000.0))]);
///     model_builder.value("repairs", &[(period, AmountOrScalar::scalar(8_000.0))]);
///     model_builder.value("capex", &[(period, AmountOrScalar::scalar(10_000.0))]);
///     model_builder
/// };
/// let lease = LeaseSpec {
///     node_id: "tenant_a".to_string(),
///     start: PeriodId::quarter(2025, 1).expect("valid period fixture"),
///     end: Some(PeriodId::quarter(2025, 4).expect("valid period fixture")),
///     base_rent: 25_000.0,
///     growth_rate: 0.0,
///     growth_convention: LeaseGrowthConvention::PerPeriod,
///     rent_steps: vec![],
///     free_rent_periods: 0,
///     free_rent_windows: vec![],
///     occupancy: 1.0,
///     renewal: None,
/// };
/// let fee = ManagementFeeSpec { rate: 0.03, base: ManagementFeeBase::Egi };
///
/// add_property_operating_statement(
///     &mut builder,
///     &[lease],
///     &["parking_income"],
///     &["repairs"],
///     &["capex"],
///     Some(fee),
///     &PropertyTemplateNodes::default(),
/// )?;
/// let model = builder.build()?;
/// assert!(model.nodes.contains_key("ncf"));
/// # Ok(())
/// # }
/// ```
pub fn add_property_operating_statement(
    builder: &mut ModelBuilder,
    leases: &[LeaseSpec],
    other_income_nodes: &[&str],
    opex_nodes: &[&str],
    capex_nodes: &[&str],
    management_fee: Option<ManagementFeeSpec>,
    nodes: &PropertyTemplateNodes,
) -> Result<()> {
    let mut draft = builder.clone();
    populate_rent_roll(&mut draft, leases, &nodes.rent_roll)?;

    // Other income total (optional).
    let other_income_expr = sum_expr_or_zero(other_income_nodes);
    draft.compute(nodes.other_income_total_node.as_str(), other_income_expr)?;

    // EGI.
    draft.compute(
        nodes.egi_node.as_str(),
        format!(
            "{} + {}",
            nodes.rent_roll.rent_effective_node, nodes.other_income_total_node
        ),
    )?;

    // Optional management fee.
    let mut opex_all: Vec<&str> = opex_nodes.to_vec();
    if let Some(spec) = management_fee {
        if !spec.rate.is_finite() || spec.rate < 0.0 {
            return Err(Error::build(
                "add_property_operating_statement: management_fee.rate must be finite and >= 0",
            ));
        }

        let base_expr = match spec.base {
            ManagementFeeBase::Egi => nodes.egi_node.as_str(),
            ManagementFeeBase::EffectiveRent => nodes.rent_roll.rent_effective_node.as_str(),
        };
        draft.compute(
            nodes.management_fee_node.as_str(),
            format!("{base_expr} * {}", spec.rate),
        )?;
        opex_all.push(nodes.management_fee_node.as_str());
    }

    // OpEx total.
    let opex_expr = sum_expr_or_zero(&opex_all);
    draft.compute(nodes.opex_total_node.as_str(), opex_expr)?;

    // NOI.
    draft.compute(
        nodes.noi_node.as_str(),
        format!("{} - {}", nodes.egi_node, nodes.opex_total_node),
    )?;

    // CapEx total and NCF.
    let capex_expr = sum_expr_or_zero(capex_nodes);
    draft.compute(nodes.capex_total_node.as_str(), capex_expr)?;
    draft.compute(
        nodes.ncf_node.as_str(),
        format!("{} - {}", nodes.noi_node, nodes.capex_total_node),
    )?;

    *builder = draft;
    Ok(())
}
