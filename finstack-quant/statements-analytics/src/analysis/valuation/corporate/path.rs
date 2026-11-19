//! Reprice coherent statement simulation paths with one DCF contract.

use super::{CorporateDcfRequest, DcfEvalContext, evaluate_dcf_from_results_impl};
use finstack_quant_core::money::Money;
use finstack_quant_statements::evaluator::StatementResult;
use finstack_quant_statements::types::FinancialModelSpec;
use finstack_quant_statements::{Error, Result};
use finstack_quant_valuations::instruments::equity::dcf_equity::TerminalValueSpec;
use serde::{Deserialize, Serialize};

/// One sampled company valuation, with the original Monte Carlo path identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DcfPathValue {
    /// Zero-based path identity shared by all sampled cash flows.
    pub path_id: u32,
    /// Enterprise value in reporting currency.
    pub enterprise_value: Money,
    /// Common equity value after bridge and discounts.
    pub equity_value: Money,
}

/// Same-currency values at an elected Monte Carlo quantile.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DcfPathQuantile {
    /// Requested probability as a fraction between zero and one.
    pub probability: f64,
    /// R-7 enterprise-value sample quantile.
    pub enterprise_value: Money,
    /// R-7 common-equity sample quantile.
    pub equity_value: Money,
}

/// Sample distribution repriced on complete coherent statement forecast paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DcfDistributionResult {
    /// Unchanged model and statement information cutoff shared by the samples.
    pub provenance: finstack_quant_statements::evaluator::RunProvenance,
    /// One enterprise and equity result per retained path.
    pub paths: Vec<DcfPathValue>,
    /// Derived valuation quantiles in the simulation's requested probability order.
    pub quantiles: Vec<DcfPathQuantile>,
}

/// Price a company distribution using coherent retained monetary statement paths.
///
/// # Arguments
/// * `model` - The unchanged model used to generate the baseline and all paths.
/// * `results` - Verified baseline at the same valuation and information cut.
///   Opening actual balances come from this snapshot.
/// * `paths` - Complete simulated path samples under the declared model
///   and all projected DCF metric nodes retained in model currency.
/// * `request` - Common WACC, terminal horizon, currency, forecast and equity terms.
///
/// # Errors
/// Rejects inconsistent provenance, missing or duplicate samples, inadmissible
/// cashflows and failures while repricing any path or deriving its quantiles.
pub fn evaluate_dcf_monte_carlo(
    model: &FinancialModelSpec,
    results: &StatementResult,
    paths: &finstack_quant_statements::evaluator::MonteCarloResults,
    request: &CorporateDcfRequest,
) -> Result<DcfDistributionResult> {
    let provenance = request.cut.check_result(model, results)?;
    if provenance.market_content_hash.is_none() || &paths.provenance != provenance {
        return Err(Error::invalid_input(
            "Monte Carlo paths and baseline statement run require matching model, dates, cutoff and captured market snapshot",
        ));
    }
    let inputs = &request.inputs;
    inputs.validate()?;
    let mut names = vec![inputs.ufcf_node.as_str()];
    if let Some(node) = request.projection.forward_remainder_node.as_deref() {
        names.push(node);
    }
    if matches!(
        inputs.terminal_value,
        TerminalValueSpec::ExitMultiple { .. }
    ) && let Some(node) = request.options.exit_multiple_metric_node.as_deref()
    {
        names.push(node);
    }
    names.sort_unstable();
    names.dedup();
    let currency = request.cut.currency;
    let samples = names
        .iter()
        .map(|&node| Ok((node, paths.monetary_paths(node, currency)?)))
        .collect::<Result<Vec<_>>>()?;
    let mut sampled = results.clone();
    let mut output = Vec::with_capacity(paths.n_paths);
    for path in 0..paths.n_paths {
        for (node, observations) in &samples {
            for (&period, &amount) in &observations[path] {
                let original = sampled.get_cell(node, &period).ok_or_else(|| {
                    Error::eval(format!("Missing baseline cell {node} at {period}"))
                })?;
                let mut updated = *original;
                updated.value = Some(finstack_quant_statements::types::AmountOrScalar::Amount(
                    amount,
                ));
                sampled.set_cell(node, period, updated);
            }
        }
        let valuation = evaluate_dcf_from_results_impl(
            model,
            &sampled,
            inputs.wacc,
            inputs.terminal_value.clone(),
            &inputs.ufcf_node,
            DcfEvalContext {
                net_debt_override: inputs.net_debt_override,
                options: &request.options,
                projection: Some(&request.projection),
            },
            Some(request.cut.as_of),
        )?;
        let path_id = u32::try_from(path)
            .map_err(|_| Error::invalid_input("DCF path ID cannot be represented"))?;
        output.push(DcfPathValue {
            path_id,
            enterprise_value: valuation.enterprise_value,
            equity_value: valuation.equity_value,
        });
    }
    let mut enterprise = output
        .iter()
        .map(|row| row.enterprise_value.amount())
        .collect::<Vec<_>>();
    let mut equity = output
        .iter()
        .map(|row| row.equity_value.amount())
        .collect::<Vec<_>>();
    enterprise.sort_by(f64::total_cmp);
    equity.sort_by(f64::total_cmp);
    let quantiles = paths
        .percentiles
        .iter()
        .map(|&probability| {
            use finstack_quant_core::math::stats::quantile_checked;
            Ok(DcfPathQuantile {
                probability,
                enterprise_value: Money::new(
                    quantile_checked(&enterprise, probability)?,
                    currency,
                )?,
                equity_value: Money::new(quantile_checked(&equity, probability)?, currency)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(DcfDistributionResult {
        provenance: provenance.clone(),
        paths: output,
        quantiles,
    })
}
