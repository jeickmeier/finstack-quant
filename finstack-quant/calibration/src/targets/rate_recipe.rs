//! Conversions between calibration-time rate types and the persisted
//! [`RateCalibrationRecipe`] wire types.
//!
//! The recipe stored on a calibrated curve mirrors the quotes, pillars,
//! OIS compounding convention and solver method used at build time so the
//! curve can be replayed under a quote shock. Both directions live here so
//! the discount/forward targets (curve → recipe) and the recalibration replay
//! (recipe → curve) share one mapping per pair.

use crate::config::CalibrationMethod;
use crate::quotes::ids::{Pillar, QuoteId};
use crate::quotes::rates::RateQuote;
use finstack_quant_core::market_data::term_structures::{
    RateCalibrationFutureContractId, RateCalibrationMethod, RateCalibrationOisCompounding,
    RateCalibrationPillar, RateCalibrationQuote, RateCalibrationRecipe,
};
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use finstack_quant_valuations::instruments::rates::irs::FloatingLegCompounding;
use finstack_quant_valuations::market::conventions::ids::IrFutureContractId;

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

/// Inverse of [`recipe_ois_compounding`].
///
/// # Arguments
///
/// * `compounding` - Recipe OIS compounding convention stored on the curve.
pub(crate) fn ois_compounding_from_recipe(
    compounding: &RateCalibrationOisCompounding,
) -> FloatingLegCompounding {
    match compounding {
        RateCalibrationOisCompounding::Simple => FloatingLegCompounding::Simple,
        RateCalibrationOisCompounding::CompoundedInArrears { lookback_days } => {
            FloatingLegCompounding::CompoundedInArrears {
                lookback_days: *lookback_days,
            }
        }
        RateCalibrationOisCompounding::CompoundedWithObservationShift { shift_days } => {
            FloatingLegCompounding::CompoundedWithObservationShift {
                shift_days: *shift_days,
            }
        }
        RateCalibrationOisCompounding::CompoundedWithRateCutoff { cutoff_days } => {
            FloatingLegCompounding::CompoundedWithRateCutoff {
                cutoff_days: *cutoff_days,
            }
        }
    }
}

/// Rebuild the typed rate quotes a curve was calibrated from.
///
/// # Arguments
///
/// * `recipe` - Persisted calibration recipe read off the curve.
/// * `curve_id` - Curve identifier, used to mint deterministic replay quote
///   ids of the form `{curve_id}-REPLAY-{index}`.
///
/// # Errors
///
/// Returns `Error::Validation` when the recipe holds basis quotes, which
/// replay through the dedicated basis path instead.
pub(crate) fn rate_quotes_from_recipe(
    recipe: &RateCalibrationRecipe,
    curve_id: &CurveId,
) -> Result<Vec<RateQuote>> {
    recipe
        .quotes
        .iter()
        .enumerate()
        .map(|(index, quote)| {
            let id = QuoteId::new(format!("{curve_id}-REPLAY-{index}"));
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
