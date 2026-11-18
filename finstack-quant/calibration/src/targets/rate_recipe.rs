//! Conversions between calibration-time rate types and the persisted
//! [`RateCalibrationRecipe`] wire types.
//!
//! The recipe stored on a calibrated curve mirrors the quotes, pillars,
//! OIS compounding convention and solver method used at build time so the
//! curve can be replayed under a quote shock. Both directions live here so
//! the discount/forward targets (curve → recipe) and the recalibration replay
//! (recipe → curve) share one mapping per pair.

use crate::config::CalibrationMethod;
use crate::quotes::RateQuote;
use crate::quotes::{Pillar, QuoteId};
use finstack_quant_cashflows::builder::FloatingLegCompounding;
use finstack_quant_core::Result;
use finstack_quant_core::market_data::term_structures::{
    RateCalibrationFutureContractId, RateCalibrationMethod, RateCalibrationOisCompounding,
    RateCalibrationPillar, RateCalibrationQuote, RateCalibrationRecipe,
};
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations_core::market::conventions::ids::IrFutureContractId;

impl From<&CalibrationMethod> for RateCalibrationMethod {
    fn from(method: &CalibrationMethod) -> Self {
        match method {
            CalibrationMethod::Bootstrap => RateCalibrationMethod::Bootstrap,
            CalibrationMethod::GlobalSolve {
                use_analytical_jacobian,
            } => RateCalibrationMethod::GlobalSolve {
                use_analytical_jacobian: *use_analytical_jacobian,
            },
        }
    }
}

impl From<&RateCalibrationMethod> for CalibrationMethod {
    fn from(method: &RateCalibrationMethod) -> Self {
        match method {
            RateCalibrationMethod::Bootstrap => CalibrationMethod::Bootstrap,
            RateCalibrationMethod::GlobalSolve {
                use_analytical_jacobian,
            } => CalibrationMethod::GlobalSolve {
                use_analytical_jacobian: *use_analytical_jacobian,
            },
        }
    }
}

impl From<&Pillar> for RateCalibrationPillar {
    fn from(pillar: &Pillar) -> Self {
        match pillar {
            Pillar::Tenor(tenor) => RateCalibrationPillar::Tenor(*tenor),
            Pillar::Date(date) => RateCalibrationPillar::Date(*date),
        }
    }
}

impl From<&RateCalibrationPillar> for Pillar {
    fn from(pillar: &RateCalibrationPillar) -> Self {
        match pillar {
            RateCalibrationPillar::Tenor(tenor) => Pillar::Tenor(*tenor),
            RateCalibrationPillar::Date(date) => Pillar::Date(*date),
        }
    }
}

impl From<&RateQuote> for RateCalibrationQuote {
    fn from(quote: &RateQuote) -> Self {
        match quote {
            RateQuote::Deposit {
                index,
                pillar,
                rate,
                ..
            } => RateCalibrationQuote::Deposit {
                index_id: index.clone(),
                pillar: pillar.into(),
                rate: *rate,
            },
            RateQuote::Fra {
                index,
                start,
                end,
                rate,
                ..
            } => RateCalibrationQuote::Fra {
                index_id: index.clone(),
                start: start.into(),
                end: end.into(),
                rate: *rate,
            },
            RateQuote::Futures {
                contract,
                expiry,
                price,
                convexity_adjustment,
                ..
            } => RateCalibrationQuote::Futures {
                contract: RateCalibrationFutureContractId::new(contract.as_str()),
                expiry: *expiry,
                price: *price,
                convexity_adjustment: Some(*convexity_adjustment),
            },
            RateQuote::Swap {
                index,
                pillar,
                rate,
                spread_decimal,
                ..
            } => RateCalibrationQuote::Swap {
                index_id: index.clone(),
                pillar: pillar.into(),
                rate: *rate,
                spread_decimal: *spread_decimal,
            },
        }
    }
}

/// Build the replay recipe stored on a calibrated discount or projection curve.
///
/// # Arguments
///
/// * `step` - Executed discount or projection evaluator, including its pricing curve identifiers.
/// * `curve_day_count` - Resolved clock used to place calibration instruments.
/// * `quotes` - Rate quotes the curve was calibrated to.
/// * `config` - Executed numerical solver settings and residual acceptance policy.
/// * `report` - Fit outcome for the curve actually adopted by the target.
///
/// # Errors
///
/// Returns `Error::Validation` if the OIS compounding override cannot be
/// replayed.
pub(crate) fn rate_calibration_recipe(
    step: crate::api::schema::StepParams,
    curve_day_count: finstack_quant_core::dates::DayCount,
    quotes: &[crate::build::prepared::PreparedQuote<RateQuote>],
    config: &crate::CalibrationConfig,
    report: &crate::CalibrationReport,
) -> Result<RateCalibrationRecipe> {
    use crate::api::schema::StepParams;
    use finstack_quant_core::market_data::term_structures::RateCalibrationCurveRole;
    let (currency, method, conventions, role) = match &step {
        StepParams::Discount(params) => (
            params.currency,
            &params.method,
            &params.conventions,
            RateCalibrationCurveRole::Discount {
                projection_curve_id: params
                    .pricing_forward_id
                    .as_ref()
                    .unwrap_or(&params.curve_id)
                    .clone(),
            },
        ),
        StepParams::Forward(params) => (
            params.currency,
            &params.method,
            &params.conventions,
            RateCalibrationCurveRole::Projection {
                discount_curve_id: params.discount_curve_id.clone(),
            },
        ),
        _ => {
            return Err(finstack_quant_core::Error::Validation(
                "expected a discount or forward rate calibration step".into(),
            ));
        }
    };
    // This recipe replays one curve. Scheduling independent plan steps does
    // not affect its evaluator and must not change the curve's identity.
    let mut replay_config = config.clone();
    replay_config.use_parallel = false;
    Ok(RateCalibrationRecipe {
        execution: Some(
            finstack_quant_core::market_data::term_structures::RateCalibrationExecution {
                accepted: report.success,
                step_params: serde_json::to_value(&step).map_err(|error| {
                    finstack_quant_core::Error::Validation(format!(
                        "cannot record executed rates evaluator: {error}"
                    ))
                })?,
                bindings: quotes
                    .iter()
                    .map(|prepared| {
                        finstack_quant_core::market_data::term_structures::RateQuoteBinding {
                            quote_id: prepared.quote.id().as_str().to_owned(),
                            pillar_date: prepared.pillar_date,
                            pillar_time: prepared.pillar_time,
                        }
                    })
                    .collect(),
                calibration_config: serde_json::to_value(replay_config).map_err(|error| {
                    finstack_quant_core::Error::Validation(format!(
                        "cannot record rates calibration configuration: {error}"
                    ))
                })?,
            },
        ),
        currency,
        method: method.into(),
        curve_day_count,
        ois_compounding: conventions
            .ois_compounding
            .as_ref()
            .map(recipe_ois_compounding)
            .transpose()?,
        role,
        quotes: quotes
            .iter()
            .map(|prepared| prepared.quote.as_ref().into())
            .collect(),
    })
}

/// Map a floating-leg compounding convention onto its recipe representation.
///
/// # Arguments
///
/// * `compounding` - Step-level OIS floating-leg compounding convention.
///   Only `Simple`, `CompoundedInArrears`, `CompoundedWithObservationShift`
///   and `CompoundedWithRateCutoff` have a recipe form.
///
/// # Errors
///
/// Returns `Error::Validation` for conventions that cannot be replayed.
pub(crate) fn recipe_ois_compounding(
    compounding: &FloatingLegCompounding,
) -> Result<RateCalibrationOisCompounding> {
    match compounding {
        FloatingLegCompounding::Simple => Ok(RateCalibrationOisCompounding::Simple),
        FloatingLegCompounding::CompoundedInArrears { lookback_days } => {
            Ok(RateCalibrationOisCompounding::CompoundedInArrears {
                lookback_days: *lookback_days,
            })
        }
        FloatingLegCompounding::CompoundedWithObservationShift { shift_days } => Ok(
            RateCalibrationOisCompounding::CompoundedWithObservationShift {
                shift_days: *shift_days,
            },
        ),
        FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days } => {
            Ok(RateCalibrationOisCompounding::CompoundedWithRateCutoff {
                cutoff_days: *cutoff_days,
            })
        }
        FloatingLegCompounding::SimpleAverage => Err(finstack_quant_core::Error::Validation(
            "unsupported floating-leg compounding for calibration replay".to_string(),
        )),
    }
}

pub(crate) fn original_execution(
    recipe: &RateCalibrationRecipe,
) -> finstack_quant_core::Result<
    &finstack_quant_core::market_data::term_structures::RateCalibrationExecution,
> {
    recipe
        .execution
        .as_ref()
        .filter(|execution| execution.accepted)
        .ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "rate quote replay requires an accepted source calibration".into(),
            )
        })
}

/// Rebuild the typed rate quotes a curve was calibrated from.
///
/// # Arguments
///
/// * `recipe` - Persisted calibration recipe read off the curve.
/// * `curve_id` - Source curve identifier used to report invalid quote bindings.
///
/// # Errors
///
/// Returns `Error::Validation` when the recipe holds basis quotes, which
/// replay through the dedicated basis path instead.
pub(crate) fn rate_quotes_from_recipe(
    recipe: &RateCalibrationRecipe,
    curve_id: &CurveId,
) -> Result<Vec<RateQuote>> {
    let execution = original_execution(recipe)?;
    let mut identifiers = std::collections::HashSet::new();
    if execution.bindings.len() != recipe.quotes.len()
        || execution.bindings.iter().any(|binding| {
            binding.quote_id.trim().is_empty() || !identifiers.insert(&binding.quote_id)
        })
    {
        return Err(finstack_quant_core::Error::Validation(format!(
            "curve {curve_id} has missing or duplicate rate quote bindings"
        )));
    }
    recipe
        .quotes
        .iter()
        .zip(&execution.bindings)
        .map(|(quote, binding)| {
            let id = QuoteId::new(binding.quote_id.as_str());
            Ok(match quote {
                RateCalibrationQuote::Deposit {
                    index_id,
                    pillar,
                    rate,
                } => RateQuote::Deposit {
                    id,
                    index: index_id.clone(),
                    pillar: pillar.into(),
                    rate: *rate,
                },
                RateCalibrationQuote::Fra {
                    index_id,
                    start,
                    end,
                    rate,
                } => RateQuote::Fra {
                    id,
                    index: index_id.clone(),
                    start: start.into(),
                    end: end.into(),
                    rate: *rate,
                },
                RateCalibrationQuote::Futures {
                    contract,
                    expiry,
                    price,
                    convexity_adjustment,
                } => RateQuote::Futures {
                    id,
                    contract: IrFutureContractId::new(contract.as_str()),
                    expiry: *expiry,
                    price: *price,
                    convexity_adjustment: convexity_adjustment.unwrap_or(0.0),
                },
                RateCalibrationQuote::Swap {
                    index_id,
                    pillar,
                    rate,
                    spread_decimal,
                } => RateQuote::Swap {
                    id,
                    index: index_id.clone(),
                    pillar: pillar.into(),
                    rate: *rate,
                    spread_decimal: *spread_decimal,
                },
                RateCalibrationQuote::Basis { .. } => {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "curve {curve_id} uses basis quotes, which require the dedicated basis replay path"
                    )));
                }
            })
        })
        .collect()
}
