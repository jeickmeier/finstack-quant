//! Fixed-statement DCF sensitivity with explicitly requested shock endpoints.

use super::{
    CorporateDcfRequest, SENSITIVITY_PARAM_EXIT_MULTIPLE, SENSITIVITY_PARAM_TERMINAL_GROWTH,
    SENSITIVITY_PARAM_WACC, evaluate_dcf_from_results,
};
use finstack_quant_core::money::Money;
use finstack_quant_core::wire::DecimalWire;
use finstack_quant_statements::evaluator::StatementResult;
use finstack_quant_statements::types::FinancialModelSpec;
use finstack_quant_statements::{Error, Result};
use finstack_quant_valuations::instruments::equity::dcf_equity::{
    DiscountedCashFlow, TerminalValueSpec,
};
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};

/// Decimal-rate distances in the low and high directions from an admitted rate.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DcfRateShocks {
    /// Positive difference from the baseline to the requested lower rate.
    pub low: DecimalWire,
    /// Positive difference from the baseline to the requested higher rate.
    pub high: DecimalWire,
}

impl DcfRateShocks {
    fn levels(self, baseline: f64) -> Result<[f64; 2]> {
        self.scaled_levels(baseline, 1.0)
    }

    fn scaled_levels(self, baseline: f64, scale: f64) -> Result<[f64; 2]> {
        let distance = |value: DecimalWire| {
            value
                .0
                .to_f64()
                .filter(|rate| rate.is_finite() && *rate > 0.0)
                .ok_or_else(|| {
                    Error::invalid_input(
                        "DCF sensitivity distances must be positive representable rates",
                    )
                })
        };
        let low = baseline - scale * distance(self.low)?;
        let high = baseline + scale * distance(self.high)?;
        if low.is_finite() && high.is_finite() && low < baseline && baseline < high {
            Ok([low, high])
        } else {
            Err(Error::invalid_input(
                "DCF sensitivity levels must be finite and distinct",
            ))
        }
    }
}

/// An explicitly requested corporate DCF sensitivity endpoint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum DcfSensitivityEndpoint {
    /// Requested endpoint that can be priced on the baseline statement run.
    Priced {
        /// Requested parameter level, in decimal rates or multiple turns.
        level: f64,
        /// Repriced enterprise value in the baseline model currency.
        enterprise_value: Money,
        /// Signed difference from baseline enterprise value in the same currency.
        change: Money,
    },
    /// Requested endpoint outside the admitted valuation domain.
    Unavailable {
        /// Requested parameter level, in decimal rates or multiple turns.
        level: f64,
        /// Reason the requested level cannot be valued.
        reason: String,
    },
}

/// The two independent directions for a named corporate DCF parameter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DcfSensitivityPair {
    /// Canonical name of the parameter changed by this pair.
    pub parameter_id: String,
    /// Endpoint at the requested lower parameter level.
    pub low: DcfSensitivityEndpoint,
    /// Endpoint at the requested higher parameter level.
    pub high: DcfSensitivityEndpoint,
}

impl DcfSensitivityPair {
    fn swing(&self) -> Option<f64> {
        let DcfSensitivityEndpoint::Priced {
            enterprise_value: low,
            ..
        } = self.low
        else {
            return None;
        };
        let DcfSensitivityEndpoint::Priced {
            enterprise_value: high,
            ..
        } = self.high
        else {
            return None;
        };
        Some((high.amount() - low.amount()).abs())
    }
}

fn sensitivity_endpoint(
    baseline: &DiscountedCashFlow,
    baseline_ev: Money,
    level: f64,
    max_stable_growth_rate: Option<f64>,
    shock: impl FnOnce(&mut DiscountedCashFlow, f64),
) -> Result<DcfSensitivityEndpoint> {
    let mut dcf = baseline.clone();
    shock(&mut dcf, level);
    let growth_terms = match dcf.terminal_value {
        TerminalValueSpec::GordonGrowth { stable_growth_rate } => Some((stable_growth_rate, None)),
        TerminalValueSpec::HModel {
            stable_growth_rate,
            high_growth_rate,
            ..
        } => Some((stable_growth_rate, Some(high_growth_rate))),
        TerminalValueSpec::ExitMultiple { .. } => None,
    };
    let outside_domain = if dcf.wacc <= -1.0 {
        Some("WACC must be above -100%")
    } else if let Some((stable, high)) = growth_terms {
        if stable <= -1.0 || stable >= dcf.wacc {
            Some("Stable growth must be above -100% and below WACC")
        } else if high.is_some_and(|high| high < stable) {
            Some("Stable growth exceeds the baseline H-model high growth")
        } else if max_stable_growth_rate.is_some_and(|ceiling| stable > ceiling) {
            Some("Stable growth exceeds the elected ceiling")
        } else {
            None
        }
    } else if matches!(dcf.terminal_value, TerminalValueSpec::ExitMultiple { multiple, .. } if multiple < 0.0)
    {
        Some("Exit multiple must be nonnegative")
    } else {
        None
    };
    if let Some(reason) = outside_domain {
        return Ok(DcfSensitivityEndpoint::Unavailable {
            level,
            reason: reason.to_owned(),
        });
    }
    let terminal_value = dcf.calculate_terminal_value()?;
    let pv = dcf.calculate_pv_explicit_flows()? + dcf.discount_terminal_value(terminal_value)?;
    let enterprise_value = Money::new(pv, baseline_ev.currency())?;
    let change = enterprise_value.checked_sub(baseline_ev)?;
    Ok(DcfSensitivityEndpoint::Priced {
        level,
        enterprise_value,
        change,
    })
}

/// Explicit axes for an enterprise-value sensitivity grid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DcfSensitivityGridSpec {
    /// Annual effective WACC decimal rates, in row order.
    pub wacc_values: Vec<f64>,
    /// Stable growth decimal rates or exit multiple turns, in column order.
    pub terminal_values: Vec<f64>,
}

/// An enterprise-value sensitivity grid calculated on a single statement run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DcfSensitivityGridResult {
    /// Reused verified baseline enterprise value before discounts.
    pub baseline_enterprise_value: Money,
    /// Row and column axes as requested, including economically unavailable levels.
    pub axes: DcfSensitivityGridSpec,
    /// Rows indexed by WACC and columns indexed by the terminal parameter.
    pub cells: Vec<Vec<DcfSensitivityEndpoint>>,
}

impl DcfSensitivityGridSpec {
    /// Verify the two axes contain nonempty unique sets of finite levels.
    ///
    /// # Errors
    /// Rejects missing axes, non-finite levels and duplicates.
    pub fn validate(&self) -> Result<()> {
        let axis = |values: &[f64], name: &str| -> Result<()> {
            if values.is_empty() || values.iter().any(|value| !value.is_finite()) {
                return Err(Error::invalid_input(format!(
                    "DCF {name} axis requires finite levels"
                )));
            }
            let mut sorted = values.to_vec();
            sorted.sort_by(f64::total_cmp);
            if sorted.windows(2).any(|pair| {
                pair[0].total_cmp(&pair[1]).is_eq()
                    || pair
                        .iter()
                        .all(|rate| rate.classify() == std::num::FpCategory::Zero)
            }) {
                return Err(Error::invalid_input(format!(
                    "DCF {name} axis contains duplicate levels"
                )));
            }
            Ok(())
        };
        axis(&self.wacc_values, "WACC")?;
        axis(&self.terminal_values, "terminal")
    }
}

fn set_dcf_terminal_level(dcf: &mut DiscountedCashFlow, level: f64) {
    match &mut dcf.terminal_value {
        TerminalValueSpec::GordonGrowth { stable_growth_rate }
        | TerminalValueSpec::HModel {
            stable_growth_rate, ..
        } => *stable_growth_rate = level,
        TerminalValueSpec::ExitMultiple { multiple, .. } => *multiple = level,
    }
}

fn dcf_grid_from_instrument(
    dcf: &DiscountedCashFlow,
    baseline_enterprise_value: Money,
    grid: &DcfSensitivityGridSpec,
    max_stable_growth_rate: Option<f64>,
) -> Result<DcfSensitivityGridResult> {
    grid.validate()?;
    let cells = grid
        .wacc_values
        .iter()
        .map(|&wacc| {
            grid.terminal_values
                .iter()
                .map(|&level| {
                    sensitivity_endpoint(
                        dcf,
                        baseline_enterprise_value,
                        level,
                        max_stable_growth_rate,
                        |endpoint, level| {
                            endpoint.wacc = wacc;
                            set_dcf_terminal_level(endpoint, level);
                        },
                    )
                })
                .collect::<Result<Vec<_>>>()
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(DcfSensitivityGridResult {
        baseline_enterprise_value,
        axes: grid.clone(),
        cells,
    })
}

/// Explicit low and high directions for an enterprise exit multiple.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DcfMultipleShocks {
    /// Apply positive differences measured in multiple turns.
    Absolute {
        /// Difference from baseline to the lower multiple in turns.
        low: DecimalWire,
        /// Difference from baseline to the higher multiple in turns.
        high: DecimalWire,
    },
    /// Apply positive fractions of the nonzero baseline multiple.
    Relative {
        /// Proportion of baseline subtracted from it.
        low: DecimalWire,
        /// Proportion of baseline added to it.
        high: DecimalWire,
    },
}

impl DcfMultipleShocks {
    fn levels(self, baseline: f64) -> Result<[f64; 2]> {
        let (distances, scale) = match self {
            Self::Absolute { low, high } => (DcfRateShocks { low, high }, 1.0),
            Self::Relative { low, high } if baseline > 0.0 => {
                (DcfRateShocks { low, high }, baseline)
            }
            Self::Relative { .. } => {
                return Err(Error::invalid_input(
                    "Relative exit-multiple shocks need a positive baseline multiple",
                ));
            }
        };
        distances.scaled_levels(baseline, scale)
    }
}

/// Terminal dimension selected for fixed-statement DCF sensitivity.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(
    tag = "parameter",
    content = "shocks",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum DcfTerminalShocks {
    /// Shock stable growth while holding high growth and half-life fixed.
    StableGrowth(DcfRateShocks),
    /// Shock the exit multiple while holding the same enterprise metric fixed.
    ExitMultiple(DcfMultipleShocks),
}

/// Independent elected shocks for discount rate and terminal assumptions.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DcfSensitivityShocks {
    /// Positive low and high WACC distances in decimal rate units.
    pub wacc: DcfRateShocks,
    /// Low and high terminal shocks in the compatible terminal dimension.
    pub terminal: DcfTerminalShocks,
}

/// Repriced points on the same baseline statement snapshot and forecast.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DcfShockReport {
    /// Enterprise value at the original WACC and terminal assumptions.
    pub baseline_enterprise_value: Money,
    /// Complete pairs ranked by absolute swing, then identifier.
    /// Pairs with an unavailable endpoint follow complete pairs.
    pub pairs: Vec<DcfSensitivityPair>,
}

/// Price each sensitivity endpoint from one verified evaluated statement run.
///
/// # Arguments
/// * `model` - The unchanged model that produced the evaluated statement run.
/// * `results` - Statement observations on the requested valuation and availability cut.
/// * `request` - Point-in-time corporate DCF assumptions and valuation horizon.
/// * `shocks` - Independent, direction-specific WACC and terminal parameter distances.
///
/// # Errors
/// Rejects malformed shocks, a terminal dimension mismatch, invalid baseline
/// inputs or pricing failures. Requested levels outside the valuation domain
/// appear as unavailable endpoints.
pub fn dcf_sensitivity_on_results(
    model: &FinancialModelSpec,
    results: &StatementResult,
    request: &CorporateDcfRequest,
    shocks: &DcfSensitivityShocks,
) -> Result<DcfShockReport> {
    let baseline = evaluate_dcf_from_results(model, results, request)?;
    let baseline_enterprise_value = baseline.enterprise_value;
    let dcf = baseline
        .dcf_instrument
        .as_ref()
        .ok_or_else(|| Error::eval("Missing DCF instrument"))?;
    let ceiling = request.projection.stable_growth_ceiling;
    let [low, high] = shocks.wacc.levels(dcf.wacc)?;
    let wacc = |level| {
        sensitivity_endpoint(
            dcf,
            baseline_enterprise_value,
            level,
            ceiling,
            |point, rate| point.wacc = rate,
        )
    };
    let (parameter_id, [terminal_low, terminal_high]) = match (&dcf.terminal_value, shocks.terminal)
    {
        (
            TerminalValueSpec::GordonGrowth { stable_growth_rate }
            | TerminalValueSpec::HModel {
                stable_growth_rate, ..
            },
            DcfTerminalShocks::StableGrowth(shock),
        ) => (
            SENSITIVITY_PARAM_TERMINAL_GROWTH,
            shock.levels(*stable_growth_rate)?,
        ),
        (
            TerminalValueSpec::ExitMultiple { multiple, .. },
            DcfTerminalShocks::ExitMultiple(shock),
        ) => (SENSITIVITY_PARAM_EXIT_MULTIPLE, shock.levels(*multiple)?),
        _ => {
            return Err(Error::invalid_input(
                "DCF shock dimension must match the terminal method",
            ));
        }
    };
    let terminal = |level| {
        sensitivity_endpoint(
            dcf,
            baseline_enterprise_value,
            level,
            ceiling,
            set_dcf_terminal_level,
        )
    };
    let mut pairs = vec![
        DcfSensitivityPair {
            parameter_id: SENSITIVITY_PARAM_WACC.to_owned(),
            low: wacc(low)?,
            high: wacc(high)?,
        },
        DcfSensitivityPair {
            parameter_id: parameter_id.to_owned(),
            low: terminal(terminal_low)?,
            high: terminal(terminal_high)?,
        },
    ];
    pairs.sort_by(|left, right| {
        right
            .swing()
            .unwrap_or(f64::NEG_INFINITY)
            .total_cmp(&left.swing().unwrap_or(f64::NEG_INFINITY))
            .then_with(|| left.parameter_id.cmp(&right.parameter_id))
    });
    Ok(DcfShockReport {
        baseline_enterprise_value,
        pairs,
    })
}

/// Calculate a WACC-row, terminal-column grid from one verified statement run.
///
/// # Arguments
/// * `model` - Unchanged model producing the verified statement observations.
/// * `results` - Statement results on the requested valuation and observation dates.
/// * `request` - Checked DCF baseline assumptions and valuation horizon.
/// * `axes` - Independent axes in requested display order. WACC rows are decimal rates;
///   terminal columns are stable growth rates or exit multiple turns.
///
/// # Errors
/// Rejects duplicate or non-finite axes, invalid baselines and pricing failures.
/// Individual requested points outside the DCF domain are marked unavailable.
pub fn dcf_sensitivity_grid(
    model: &FinancialModelSpec,
    results: &StatementResult,
    request: &CorporateDcfRequest,
    axes: &DcfSensitivityGridSpec,
) -> Result<DcfSensitivityGridResult> {
    let baseline = evaluate_dcf_from_results(model, results, request)?;
    let dcf = baseline
        .dcf_instrument
        .as_ref()
        .ok_or_else(|| Error::eval("Missing DCF instrument"))?;
    dcf_grid_from_instrument(
        dcf,
        baseline.enterprise_value,
        axes,
        request.projection.stable_growth_ceiling,
    )
}
