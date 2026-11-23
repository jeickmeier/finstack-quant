//! NOI/NCF buildup templates and shared formula-expression helpers.

use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::{Error, Result};

pub(super) fn sum_expr(nodes: &[&str]) -> Result<String> {
    if nodes.is_empty() {
        return Err(Error::build("Expected at least one node".to_string()));
    }
    Ok(nodes.join(" + "))
}

/// Maximum allowed lease-growth period index. Beyond this the `u32`
/// period count risks truncation when cast to `i32` for `f64::powi`
/// (values ≥ 2^31 wrap to negative exponents, silently turning a
/// growing rent into a decaying one). At monthly cadence this is
/// ~833 years, which is well beyond any real underwriting horizon.
pub(super) const MAX_LEASE_GROWTH_PERIODS: u32 = 10_000;

/// Convert a period count into a non-negative `i32` exponent suitable
/// for `f64::powi`, erroring out if the count exceeds the cap.
///
/// Direct `as i32` casts are unsafe here: a `u32` above `i32::MAX`
/// wraps silently to a negative exponent, which would collapse a
/// growth factor into a decay factor on long horizons. Returning an
/// error forces the caller to acknowledge the pathological input.
pub(super) fn checked_growth_exponent(n: u32) -> core::result::Result<i32, String> {
    if n > MAX_LEASE_GROWTH_PERIODS {
        return Err(format!(
            "lease growth exponent {} exceeds safe limit {} (≈ 10k periods)",
            n, MAX_LEASE_GROWTH_PERIODS
        ));
    }
    Ok(n as i32)
}

pub(super) fn sum_expr_or_zero(nodes: &[&str]) -> String {
    if nodes.is_empty() {
        "0".to_string()
    } else {
        nodes.join(" + ")
    }
}

/// Add a standard NOI buildup:
/// `total_revenue = sum(revenue_nodes)`
/// `total_expenses = sum(expense_nodes)`
/// `noi = total_revenue - total_expenses`
///
/// # Arguments
///
/// * `builder` - Ready builder whose periods already define the model cadence
/// * `total_revenue_node` - Node id for aggregated revenue
/// * `revenue_nodes` - Revenue node ids to sum
/// * `total_expenses_node` - Node id for aggregated operating expenses
/// * `expense_nodes` - Expense node ids to sum
/// * `noi_node` - Node id for net operating income
///
/// # Errors
///
/// Returns an error if `revenue_nodes` or `expense_nodes` is empty, or if any
/// computed node cannot be added to the builder.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::PeriodId;
/// use finstack_quant_statements::builder::ModelBuilder;
/// use finstack_quant_statements::types::AmountOrScalar;
/// use finstack_quant_statements_analytics::templates::real_estate::add_noi_buildup;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
/// let mut builder = {
///     let mut model_builder = ModelBuilder::new("property");
///     model_builder.periods("2025Q1..Q1", None)?;
///     model_builder.value("rent", &[(period, AmountOrScalar::scalar(100.0))]);
///     model_builder.value("opex", &[(period, AmountOrScalar::scalar(40.0))]);
///     model_builder
/// };
///
/// add_noi_buildup(
///     &mut builder,
///     "total_revenue",
///     &["rent"],
///     "total_expenses",
///     &["opex"],
///     "noi",
/// )?;
/// let model = builder.build()?;
/// assert!(model.nodes.contains_key("noi"));
/// # Ok(())
/// # }
/// ```
pub fn add_noi_buildup(
    builder: &mut ModelBuilder,
    total_revenue_node: &str,
    revenue_nodes: &[&str],
    total_expenses_node: &str,
    expense_nodes: &[&str],
    noi_node: &str,
) -> Result<()> {
    let mut draft = builder.clone();
    let total_rev_expr = sum_expr(revenue_nodes)?;
    let total_exp_expr = sum_expr(expense_nodes)?;

    draft
        .compute(total_revenue_node, &total_rev_expr)?
        .compute(total_expenses_node, &total_exp_expr)?
        .compute(
            noi_node,
            format!("{total_revenue_node} - {total_expenses_node}"),
        )?;

    *builder = draft;
    Ok(())
}

/// Add a standard NCF (net cash flow) buildup:
/// `ncf = noi - sum(capex_nodes)`
///
/// `capex_nodes` are assumed to be positive outflows. When no CapEx nodes are
/// supplied, the resulting `ncf_node` is simply an alias of `noi_node`.
///
/// # Arguments
///
/// * `builder` - Ready builder whose periods already define the model cadence
/// * `noi_node` - Existing NOI node id
/// * `capex_nodes` - CapEx node ids expressed as positive outflows
/// * `ncf_node` - Node id for net cash flow after CapEx
///
/// # Errors
///
/// Returns an error if CapEx nodes are provided but cannot be combined into a
/// valid formula, or if the new node cannot be added.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::PeriodId;
/// use finstack_quant_statements::builder::ModelBuilder;
/// use finstack_quant_statements::types::AmountOrScalar;
/// use finstack_quant_statements_analytics::templates::real_estate::add_ncf_buildup;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
/// let mut builder = {
///     let mut model_builder = ModelBuilder::new("property");
///     model_builder.periods("2025Q1..Q1", None)?;
///     model_builder.value("noi", &[(period, AmountOrScalar::scalar(60.0))]);
///     model_builder.value("capex", &[(period, AmountOrScalar::scalar(10.0))]);
///     model_builder
/// };
///
/// add_ncf_buildup(&mut builder, "noi", &["capex"], "ncf")?;
/// let model = builder.build()?;
/// assert!(model.nodes.contains_key("ncf"));
/// # Ok(())
/// # }
/// ```
pub fn add_ncf_buildup(
    builder: &mut ModelBuilder,
    noi_node: &str,
    capex_nodes: &[&str],
    ncf_node: &str,
) -> Result<()> {
    if capex_nodes.is_empty() {
        builder.compute(ncf_node, noi_node)?;
    } else {
        let capex_expr = sum_expr(capex_nodes)?;
        builder.compute(ncf_node, format!("{noi_node} - ({capex_expr})"))?;
    }
    Ok(())
}
