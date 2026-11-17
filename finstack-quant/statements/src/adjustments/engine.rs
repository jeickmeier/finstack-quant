//! Normalization bridge engine.

use crate::adjustments::types::{
    AdjustmentCap, AdjustmentValue, AppliedAdjustment, CapBaseMode, NormalizationConfig,
    NormalizationResult,
};
use crate::error::{Error, Result};
use crate::evaluator::cell::Cell;
use crate::evaluator::{ResultCell, StatementResult, UnavailableReason};
use crate::types::{AmountOrScalar, NodeValueType};
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_core::money::Money;

/// Build the ordered bridge from a reported node to its adjusted value for
/// every period of the result's plan.
///
/// The whole configuration is validated before any period is calculated:
/// identifiers, the output node, every referenced node, units and
/// currencies, the period keys of fixed amounts and every number. Each
/// period then starts from the reported target value and applies the
/// adjustments in declaration order. A fixed adjustment that does not list
/// the period adjusts by zero. A percentage adjustment multiplies its
/// reference node's reported value in the same period. Each cap limits its
/// own adjustment's magnitude and keeps its sign; a reference cap is
/// `fraction` times the positive part of its base, where a base equal to the
/// target is the reported value, or the reported value plus the earlier
/// applied adjustments under [`CapBaseMode::Progressive`]. Money is added
/// and scaled exactly with checked arithmetic and scalars with a compensated
/// sum; every row's raw amount, cap base, limit, applied amount and running
/// value is checked. There is no currency conversion.
///
/// A period whose target, percentage reference or cap base is missing,
/// masked or unavailable has no adjusted value: its result carries the
/// reason and no rows. Nothing is written to `results`; use
/// [`merge_into_results`] to publish the adjusted node.
///
/// # Arguments
///
/// * `results` - Evaluated or rolled-up statement results with their period
///   plan and typed cells. They are only read.
/// * `config` - The reported node, the new output node and the ordered
///   adjustments with their caps.
///
/// # Returns
///
/// One [`NormalizationResult`] per period of the plan, in timeline order.
///
/// # Errors
///
/// Returns an invalid-input error when the results carry no period plan,
/// the output node is empty or equals the target, an adjustment id repeats,
/// the target or a referenced node is absent or has no recorded units,
/// units or currencies differ, a fixed amount names a period outside the
/// plan, or a number is non-finite or a cap negative. Returns an evaluation
/// error when a consumed value is not finite or any row's arithmetic cannot
/// be represented; no partial bridge is returned.
pub fn normalize(
    results: &StatementResult,
    config: &NormalizationConfig,
) -> Result<Vec<NormalizationResult>> {
    let _span = tracing::info_span!(
        "statements.normalize",
        target_node = config.target_node.as_str(),
        adjustments = config.adjustments.len(),
    )
    .entered();

    let value_type = validate(results, config)?;
    let periods = results.get_periods();
    periods
        .iter()
        .map(|period| bridge(results, config, value_type, period.id))
        .collect()
}

/// Publish the adjusted metric of `config` as the new node
/// `config.output_node`.
///
/// The bridge is calculated from `config` by [`normalize`], so the published
/// values always agree with the configuration recorded for them in
/// [`StatementResult::normalizations`]. A period without an adjusted value
/// is published as an unavailable cell with its reason. The reported target,
/// every other node and the check report are left as they are: source checks
/// describe the reported figures, and a ratio or covenant on the adjusted
/// basis must reference the output node explicitly.
///
/// # Arguments
///
/// * `results` - Result of an evaluation run, or a rollup of one, that
///   receives the new node, its units and the configuration.
/// * `config` - The bridge to calculate and publish.
///
/// # Returns
///
/// The bridge that was published, one [`NormalizationResult`] per period.
///
/// # Errors
///
/// Returns an invalid-input error when `results` record no source run, or
/// already hold a node named `config.output_node` (including one published
/// earlier), and every error of [`normalize`]. On error nothing is
/// published.
pub fn merge_into_results(
    results: &mut StatementResult,
    config: &NormalizationConfig,
) -> Result<Vec<NormalizationResult>> {
    if results.provenance.is_none() && results.rollup_provenance.is_none() {
        return Err(Error::invalid_input(format!(
            "Cannot publish '{}': the results record no source run to link it to",
            config.output_node
        )));
    }
    if results.cells.contains_key(&config.output_node) {
        return Err(Error::invalid_input(format!(
            "Cannot publish '{}': the results already hold a node with that id. An adjusted \
             metric is published under a new id and never overwrites a node",
            config.output_node
        )));
    }
    let rows = normalize(results, config)?;
    let value_type = validate(results, config)?;
    for row in &rows {
        let cell = match (row.final_value, row.unavailable) {
            (Some(AmountOrScalar::Amount(money)), _) => ResultCell::money(money),
            (Some(AmountOrScalar::Scalar(value)), _) => ResultCell::scalar(value)?,
            (None, reason) => {
                ResultCell::unavailable(reason.unwrap_or(UnavailableReason::MissingInput))
            }
        };
        results.set_cell(&config.output_node, row.period, cell);
    }
    results
        .node_value_types
        .insert(config.output_node.clone(), value_type);
    results
        .normalizations
        .insert(config.output_node.clone(), config.clone());
    Ok(rows)
}

/// Validate the whole configuration against the result before any period is
/// calculated, and return the target's units.
fn validate(results: &StatementResult, config: &NormalizationConfig) -> Result<NodeValueType> {
    config.validate()?;
    let plan = results.periods.as_ref().ok_or_else(|| {
        Error::invalid_input("Normalization needs results that carry their period plan")
    })?;
    let units = |node: &str, role: &str| -> Result<NodeValueType> {
        if !results.cells.contains_key(node) {
            return Err(Error::invalid_input(format!(
                "Normalization {role} '{node}' is not a node of the results"
            )));
        }
        results.node_value_types.get(node).copied().ok_or_else(|| {
            Error::invalid_input(format!(
                "Normalization {role} '{node}' has no recorded units in the results"
            ))
        })
    };
    let value_type = units(&config.target_node, "target")?;
    let same_units = |node: &str, role: &str| -> Result<()> {
        let other = units(node, role)?;
        if other == value_type {
            Ok(())
        } else {
            Err(Error::invalid_input(format!(
                "Normalization target '{}' ({value_type:?}) and {role} '{node}' ({other:?}) \
                 have different units; there is no currency conversion",
                config.target_node
            )))
        }
    };
    let typed = |amount: AmountOrScalar, what: &str| -> Result<()> {
        let matches = match (amount, value_type) {
            (AmountOrScalar::Scalar(value), NodeValueType::Scalar) => value.is_finite(),
            (AmountOrScalar::Amount(money), NodeValueType::Monetary { currency }) => {
                money.currency() == currency
            }
            _ => false,
        };
        if matches {
            Ok(())
        } else {
            Err(Error::invalid_input(format!(
                "{what} must be a finite amount in the units of target '{}' ({value_type:?})",
                config.target_node
            )))
        }
    };

    for adjustment in &config.adjustments {
        let id = &adjustment.id;
        match &adjustment.value {
            AdjustmentValue::Fixed { amounts } => {
                for (period, amount) in amounts {
                    if !plan.periods.iter().any(|known| known.id == *period) {
                        return Err(Error::invalid_input(format!(
                            "Adjustment '{id}' has an amount for {period}, which is not a \
                             period of the results"
                        )));
                    }
                    typed(*amount, &format!("Adjustment '{id}' amount for {period}"))?;
                }
            }
            AdjustmentValue::PercentageOfNode {
                node_id,
                percentage,
            } => {
                same_units(node_id, "reference")?;
                if !percentage.is_finite() {
                    return Err(Error::invalid_input(format!(
                        "Adjustment '{id}' percentage must be finite"
                    )));
                }
            }
        }
        match &adjustment.cap {
            None => {}
            Some(AdjustmentCap::Absolute { amount }) => {
                typed(*amount, &format!("Adjustment '{id}' absolute cap"))?;
                if is_negative(*amount) {
                    return Err(Error::invalid_input(format!(
                        "Adjustment '{id}' absolute cap must be non-negative: a cap limits \
                         magnitude and the adjustment keeps its sign"
                    )));
                }
            }
            Some(AdjustmentCap::Reference {
                base_node,
                fraction,
                ..
            }) => {
                same_units(base_node, "cap base")?;
                if !fraction.is_finite() || *fraction < 0.0 {
                    return Err(Error::invalid_input(format!(
                        "Adjustment '{id}' cap fraction must be finite and non-negative: a \
                         cap limits magnitude and the adjustment keeps its sign"
                    )));
                }
            }
        }
    }
    Ok(value_type)
}

/// The bridge of one period.
fn bridge(
    results: &StatementResult,
    config: &NormalizationConfig,
    value_type: NodeValueType,
    period: PeriodId,
) -> Result<NormalizationResult> {
    let unavailable = |base_value, reason| NormalizationResult {
        value_type,
        period,
        base_value,
        adjustments: Vec::new(),
        final_value: None,
        unavailable: Some(reason),
    };
    let base = match read(results, &config.target_node, period)? {
        Ok(base) => base,
        Err(reason) => return Ok(unavailable(None, reason)),
    };

    let mut rows = Vec::with_capacity(config.adjustments.len());
    let mut running = Running::new(base);
    for adjustment in &config.adjustments {
        let at = |what: &str| format!("adjustment '{}' {what} in {period}", adjustment.id);
        let raw = match &adjustment.value {
            AdjustmentValue::Fixed { amounts } => {
                amounts.get(&period).copied().unwrap_or(zero(value_type))
            }
            AdjustmentValue::PercentageOfNode {
                node_id,
                percentage,
            } => match read(results, node_id, period)? {
                Ok(reference) => scale(reference, *percentage, &at("amount"))?,
                Err(reason) => return Ok(unavailable(Some(base), reason)),
            },
        };
        let (cap_base, cap_limit) = match &adjustment.cap {
            None => (None, None),
            Some(AdjustmentCap::Absolute { amount }) => (None, Some(*amount)),
            Some(AdjustmentCap::Reference {
                base_node,
                fraction,
                base_mode,
            }) => {
                let cap_base = if *base_node != config.target_node {
                    match read(results, base_node, period)? {
                        Ok(value) => value,
                        Err(reason) => return Ok(unavailable(Some(base), reason)),
                    }
                } else if *base_mode == CapBaseMode::Progressive {
                    running.value()
                } else {
                    base
                };
                let room = if is_negative(cap_base) {
                    zero(value_type)
                } else {
                    cap_base
                };
                (
                    Some(cap_base),
                    Some(scale(room, *fraction, &at("cap limit"))?),
                )
            }
        };
        let (applied, is_capped) = match cap_limit {
            Some(limit) if exceeds(raw, limit)? => {
                let signed = if is_negative(raw) {
                    negate(limit)
                } else {
                    limit
                };
                (signed, true)
            }
            _ => (raw, false),
        };
        running.add(applied, &at("running value"))?;
        rows.push(AppliedAdjustment {
            adjustment_id: adjustment.id.clone(),
            name: adjustment.name.clone(),
            raw_amount: raw,
            cap_base,
            cap_limit,
            capped_amount: applied,
            is_capped,
            adjusted_value: running.value(),
        });
    }

    Ok(NormalizationResult {
        value_type,
        period,
        base_value: Some(base),
        adjustments: rows,
        final_value: Some(running.value()),
        unavailable: None,
    })
}

/// The reported value plus the applied adjustments so far: exact for money,
/// a compensated sum for scalars.
struct Running {
    base: AmountOrScalar,
    value: AmountOrScalar,
    scalar_total: NeumaierAccumulator,
}

impl Running {
    fn new(base: AmountOrScalar) -> Self {
        Self {
            base,
            value: base,
            scalar_total: NeumaierAccumulator::new(),
        }
    }

    fn value(&self) -> AmountOrScalar {
        self.value
    }

    fn add(&mut self, applied: AmountOrScalar, what: &str) -> Result<()> {
        self.value = match (self.value, self.base, applied) {
            (AmountOrScalar::Amount(value), _, AmountOrScalar::Amount(applied)) => {
                AmountOrScalar::Amount(value.checked_add(applied)?)
            }
            (_, AmountOrScalar::Scalar(base), AmountOrScalar::Scalar(applied)) => {
                self.scalar_total.add(applied)?;
                AmountOrScalar::Scalar(finite(base + self.scalar_total.get_total()?, what)?)
            }
            _ => return Err(Error::eval(format!("The {what} mixes money and scalars"))),
        };
        Ok(())
    }
}

/// The value of a node in a period, or why it has none. A value cell that
/// holds a non-finite number is an error.
fn read(
    results: &StatementResult,
    node: &str,
    period: PeriodId,
) -> Result<std::result::Result<AmountOrScalar, UnavailableReason>> {
    let cell = results
        .get_cell(node, &period)
        .map_or(Cell::Missing, |cell| cell.to_cell());
    Ok(match cell {
        Cell::Scalar(value) => Ok(AmountOrScalar::Scalar(finite(
            value,
            &format!("value of '{node}' in {period}"),
        )?)),
        Cell::Money(money) => Ok(AmountOrScalar::Amount(money)),
        Cell::Missing | Cell::Masked => Err(UnavailableReason::MissingInput),
        Cell::Unavailable(reason) => Err(reason),
    })
}

fn finite(value: f64, what: &str) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(Error::eval(format!(
            "The {what} is not finite or cannot be represented"
        )))
    }
}

fn zero(value_type: NodeValueType) -> AmountOrScalar {
    match value_type {
        NodeValueType::Monetary { currency } => AmountOrScalar::Amount(Money::zero(currency)),
        NodeValueType::Scalar => AmountOrScalar::Scalar(0.0),
    }
}

fn scale(amount: AmountOrScalar, factor: f64, what: &str) -> Result<AmountOrScalar> {
    Ok(match amount {
        AmountOrScalar::Amount(money) => AmountOrScalar::Amount(money.checked_mul_f64(factor)?),
        AmountOrScalar::Scalar(value) => AmountOrScalar::Scalar(finite(value * factor, what)?),
    })
}

fn is_negative(amount: AmountOrScalar) -> bool {
    match amount {
        AmountOrScalar::Amount(money) => {
            money.amount_decimal().is_sign_negative() && !money.amount_decimal().is_zero()
        }
        AmountOrScalar::Scalar(value) => value < 0.0,
    }
}

fn negate(amount: AmountOrScalar) -> AmountOrScalar {
    match amount {
        AmountOrScalar::Amount(money) => AmountOrScalar::Amount(money.checked_neg()),
        AmountOrScalar::Scalar(value) => AmountOrScalar::Scalar(-value),
    }
}

/// Whether the magnitude of `raw` is above the non-negative `limit`.
fn exceeds(raw: AmountOrScalar, limit: AmountOrScalar) -> Result<bool> {
    Ok(match (raw, limit) {
        (AmountOrScalar::Amount(raw), AmountOrScalar::Amount(limit)) => {
            raw.abs().checked_cmp(limit)? == std::cmp::Ordering::Greater
        }
        (AmountOrScalar::Scalar(raw), AmountOrScalar::Scalar(limit)) => raw.abs() > limit,
        _ => {
            return Err(Error::eval(
                "A cap limit and its adjustment mix money and scalars",
            ));
        }
    })
}
