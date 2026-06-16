//! Model parameters extraction and modification for P&L attribution.
//!
//! Provides functionality to delegate model-specific parameter extraction to
//! instruments, create modified versions with different parameters, and measure
//! parameter shifts.

use finstack_quant_core::Result;
use finstack_quant_valuations::instruments::model_params::ModelParamsSnapshot;
use finstack_quant_valuations::instruments::Instrument;
use std::sync::Arc;

/// Extract model parameters from an instrument.
///
/// Delegates through the [`Instrument`] trait so each instrument owns its
/// model-parameter extraction behavior.
///
/// # Arguments
///
/// * `instrument` - Instrument to extract parameters from
///
/// # Returns
///
/// Snapshot of model parameters, or `ModelParamsSnapshot::None` if instrument
/// type doesn't have extractable parameters.
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_attribution::extract_model_params;
/// use finstack_quant_valuations::instruments::fixed_income::structured_credit::StructuredCredit;
/// use finstack_quant_valuations::instruments::model_params::ModelParamsSnapshot;
/// use finstack_quant_valuations::instruments::Instrument;
/// use std::sync::Arc;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let structured_credit = Arc::new(StructuredCredit::example())
///     as Arc<dyn Instrument>;
///
/// let params = extract_model_params(&structured_credit);
/// match params {
///     ModelParamsSnapshot::StructuredCredit { prepayment_spec, .. } => {
///         println!("Prepayment: {:?}", prepayment_spec);
///     }
///     _ => {}
/// }
/// # Ok(())
/// # }
/// ```
pub fn extract_model_params(instrument: &Arc<dyn Instrument>) -> ModelParamsSnapshot {
    instrument.model_params_snapshot()
}

/// Create a modified instrument with different model parameters.
///
/// Clones the instrument and replaces its model parameters with those from
/// the snapshot. Used for isolating model parameter P&L in attribution.
///
/// # Arguments
///
/// * `instrument` - Original instrument
/// * `params` - Model parameters to apply
///
/// # Returns
///
/// New instrument with modified parameters, or original if no params to modify.
///
/// # Errors
///
/// Returns error if instrument type doesn't match snapshot type.
///
/// # Examples
///
/// ```ignore
/// // Extract T₀ parameters
/// use finstack_quant_attribution::{extract_model_params, with_model_params};
/// use finstack_quant_valuations::instruments::fixed_income::structured_credit::StructuredCredit;
/// use finstack_quant_valuations::instruments::Instrument;
/// use std::sync::Arc;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let instrument = Arc::new(StructuredCredit::example())
///     as Arc<dyn Instrument>;
///
/// let params_t0 = extract_model_params(&instrument);
///
/// // Create instrument with T₀ params for attribution
/// let instrument_t0_params = with_model_params(&instrument, &params_t0)?;
/// # let _ = instrument_t0_params;
/// # Ok(())
/// # }
/// ```
pub fn with_model_params(
    instrument: &Arc<dyn Instrument>,
    params: &ModelParamsSnapshot,
) -> Result<Arc<dyn Instrument>> {
    if matches!(params, ModelParamsSnapshot::None) {
        return Ok(Arc::clone(instrument));
    }

    instrument.with_model_params(params).map(Arc::from)
}

/// Compute a prepayment parameter shift between two snapshots.
///
/// Returns the shift in **basis points** of CPR. Pairs directly with the
/// `Prepayment01` metric, which is `$ per 1bp` of CPR:
/// `model_params_pnl ≈ Prepayment01 × measure_prepayment_shift(t0, t1)`.
///
/// # Arguments
///
/// * `snapshot_t0` - Parameters at T₀
/// * `snapshot_t1` - Parameters at T₁
///
fn prepayment_shift(
    snapshot_t0: &ModelParamsSnapshot,
    snapshot_t1: &ModelParamsSnapshot,
) -> Option<f64> {
    match (snapshot_t0, snapshot_t1) {
        (
            ModelParamsSnapshot::StructuredCredit {
                prepayment_spec: prep_t0,
                ..
            },
            ModelParamsSnapshot::StructuredCredit {
                prepayment_spec: prep_t1,
                ..
            },
        ) => {
            use finstack_quant_cashflows::builder::specs::PrepaymentCurve;

            match (&prep_t0.curve, &prep_t1.curve) {
                (
                    Some(PrepaymentCurve::Psa {
                        speed_multiplier: mult_t0,
                    }),
                    Some(PrepaymentCurve::Psa {
                        speed_multiplier: mult_t1,
                    }),
                ) => {
                    // PSA multiplier change (convert to CPR change approximation)
                    // PSA 100% ≈ 6% CPR terminal, so multiply difference by 6%
                    Some((mult_t1 - mult_t0) * 600.0) // Convert to basis points
                }
                (None, None)
                | (Some(PrepaymentCurve::Constant), Some(PrepaymentCurve::Constant)) => {
                    // Direct CPR difference in basis points
                    Some((prep_t1.cpr - prep_t0.cpr) * 10000.0)
                }
                _ => None, // Mixed or unsupported model types
            }
        }
        _ => None,
    }
}

/// Measure prepayment parameter shift between two snapshots.
///
/// Returns the shift in **basis points** of CPR (0.0 if not applicable),
/// pairing directly with the `$ per 1bp` `Prepayment01` metric.
pub fn measure_prepayment_shift(
    snapshot_t0: &ModelParamsSnapshot,
    snapshot_t1: &ModelParamsSnapshot,
) -> f64 {
    prepayment_shift(snapshot_t0, snapshot_t1).unwrap_or_else(|| {
        tracing::warn!("Model parameter prepayment shift defaulted to zero");
        0.0
    })
}

/// Compute a default rate parameter shift between two snapshots.
///
/// Returns the shift in **basis points** of CDR. Pairs directly with the
/// `Default01` metric (`$ per 1bp` of CDR).
fn default_shift(
    snapshot_t0: &ModelParamsSnapshot,
    snapshot_t1: &ModelParamsSnapshot,
) -> Option<f64> {
    match (snapshot_t0, snapshot_t1) {
        (
            ModelParamsSnapshot::StructuredCredit {
                default_spec: def_t0,
                ..
            },
            ModelParamsSnapshot::StructuredCredit {
                default_spec: def_t1,
                ..
            },
        ) => {
            // CDR difference in basis points (works for both constant and SDA curves)
            Some((def_t1.cdr - def_t0.cdr) * 10000.0)
        }
        _ => None,
    }
}

/// Measure default rate parameter shift between two snapshots.
///
/// Returns the shift in **basis points** of CDR (0.0 if not applicable),
/// pairing directly with the `$ per 1bp` `Default01` metric.
pub fn measure_default_shift(
    snapshot_t0: &ModelParamsSnapshot,
    snapshot_t1: &ModelParamsSnapshot,
) -> f64 {
    default_shift(snapshot_t0, snapshot_t1).unwrap_or_else(|| {
        tracing::warn!("Model parameter default shift defaulted to zero");
        0.0
    })
}

/// Compute a recovery rate parameter shift between two snapshots.
///
/// Returns the shift in **percentage points** (not basis points). Pairs
/// directly with the `Recovery01` metric (`$ per 1%` recovery move).
fn recovery_shift(
    snapshot_t0: &ModelParamsSnapshot,
    snapshot_t1: &ModelParamsSnapshot,
) -> Option<f64> {
    match (snapshot_t0, snapshot_t1) {
        (
            ModelParamsSnapshot::StructuredCredit {
                recovery_spec: rec_t0,
                ..
            },
            ModelParamsSnapshot::StructuredCredit {
                recovery_spec: rec_t1,
                ..
            },
        ) => {
            // Direct recovery rate difference in percentage points
            Some((rec_t1.rate - rec_t0.rate) * 100.0)
        }
        _ => None,
    }
}

/// Measure recovery rate parameter shift between two snapshots.
///
/// Returns the shift in **percentage points** (0.0 if not applicable),
/// pairing directly with the `$ per 1%` `Recovery01` metric.
pub fn measure_recovery_shift(
    snapshot_t0: &ModelParamsSnapshot,
    snapshot_t1: &ModelParamsSnapshot,
) -> f64 {
    recovery_shift(snapshot_t0, snapshot_t1).unwrap_or_else(|| {
        tracing::warn!("Model parameter recovery shift defaulted to zero");
        0.0
    })
}

/// Compute a conversion ratio shift between two snapshots.
///
/// Returns shift in percentage points for use with Conversion01 metric.
fn conversion_shift(
    snapshot_t0: &ModelParamsSnapshot,
    snapshot_t1: &ModelParamsSnapshot,
) -> Option<f64> {
    match (snapshot_t0, snapshot_t1) {
        (
            ModelParamsSnapshot::Convertible {
                conversion_spec: conv_t0,
            },
            ModelParamsSnapshot::Convertible {
                conversion_spec: conv_t1,
            },
        ) => {
            match (conv_t0.ratio, conv_t1.ratio) {
                (Some(ratio_t0), Some(ratio_t1)) if ratio_t0 != 0.0 => {
                    // Conversion ratio change as percentage
                    Some(((ratio_t1 - ratio_t0) / ratio_t0) * 100.0)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Measure conversion ratio shift between two snapshots.
///
/// Returns shift in percentage points, or 0.0 if not applicable.
pub fn measure_conversion_shift(
    snapshot_t0: &ModelParamsSnapshot,
    snapshot_t1: &ModelParamsSnapshot,
) -> f64 {
    conversion_shift(snapshot_t0, snapshot_t1).unwrap_or_else(|| {
        tracing::warn!("Model parameter conversion shift defaulted to zero");
        0.0
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_cashflows::builder::{
        DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec,
    };
    use finstack_quant_valuations::instruments::fixed_income::convertible::{
        AntiDilutionPolicy, ConversionPolicy, ConversionSpec, DividendAdjustment,
    };

    #[test]
    fn test_extract_none_for_unsupported_instrument() {
        // For instruments without model params, should return None
        // Testing will be done with actual instruments in integration tests
    }

    #[test]
    fn test_measure_prepayment_shift_psa() {
        let params_t0 = ModelParamsSnapshot::StructuredCredit {
            prepayment_spec: PrepaymentModelSpec::psa(1.0),
            default_spec: DefaultModelSpec::constant_cdr(0.02),
            recovery_spec: RecoveryModelSpec::with_lag(0.60, 12),
        };

        let params_t1 = ModelParamsSnapshot::StructuredCredit {
            prepayment_spec: PrepaymentModelSpec::psa(1.5),
            default_spec: DefaultModelSpec::constant_cdr(0.02),
            recovery_spec: RecoveryModelSpec::with_lag(0.60, 12),
        };

        let shift = measure_prepayment_shift(&params_t0, &params_t1);
        // PSA increased by 0.5, which is 0.5 * 600bp = 300bp
        assert_eq!(shift, 300.0);
    }

    #[test]
    fn test_measure_shift_defaults_to_zero_for_snapshot_type_mismatch() {
        let structured = ModelParamsSnapshot::StructuredCredit {
            prepayment_spec: PrepaymentModelSpec::psa(1.0),
            default_spec: DefaultModelSpec::constant_cdr(0.02),
            recovery_spec: RecoveryModelSpec::with_lag(0.60, 12),
        };
        let convertible = ModelParamsSnapshot::Convertible {
            conversion_spec: ConversionSpec {
                ratio: Some(20.0),
                price: None,
                policy: ConversionPolicy::Voluntary,
                anti_dilution: AntiDilutionPolicy::None,
                dividend_adjustment: DividendAdjustment::None,
                dilution_events: Vec::new(),
            },
        };

        assert_eq!(measure_prepayment_shift(&structured, &convertible), 0.0);
        assert_eq!(measure_default_shift(&structured, &convertible), 0.0);
        assert_eq!(measure_recovery_shift(&structured, &convertible), 0.0);
        assert_eq!(measure_conversion_shift(&structured, &convertible), 0.0);
    }

    #[test]
    fn test_measure_default_shift_cdr() {
        let params_t0 = ModelParamsSnapshot::StructuredCredit {
            prepayment_spec: PrepaymentModelSpec::psa(1.0),
            default_spec: DefaultModelSpec::constant_cdr(0.02),
            recovery_spec: RecoveryModelSpec::with_lag(0.60, 12),
        };

        let params_t1 = ModelParamsSnapshot::StructuredCredit {
            prepayment_spec: PrepaymentModelSpec::psa(1.0),
            default_spec: DefaultModelSpec::constant_cdr(0.03),
            recovery_spec: RecoveryModelSpec::with_lag(0.60, 12),
        };

        let shift = measure_default_shift(&params_t0, &params_t1);
        // CDR increased by 1% = 100bp
        assert!((shift - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_measure_recovery_shift() {
        let params_t0 = ModelParamsSnapshot::StructuredCredit {
            prepayment_spec: PrepaymentModelSpec::psa(1.0),
            default_spec: DefaultModelSpec::constant_cdr(0.02),
            recovery_spec: RecoveryModelSpec::with_lag(0.60, 12),
        };

        let params_t1 = ModelParamsSnapshot::StructuredCredit {
            prepayment_spec: PrepaymentModelSpec::psa(1.0),
            default_spec: DefaultModelSpec::constant_cdr(0.02),
            recovery_spec: RecoveryModelSpec::with_lag(0.65, 12),
        };

        let shift = measure_recovery_shift(&params_t0, &params_t1);
        // Recovery rate increased from 60% to 65% (5 percentage points)
        assert!((shift - 5.0).abs() < 0.01);
    }
}
