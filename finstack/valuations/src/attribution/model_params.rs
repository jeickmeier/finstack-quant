//! Model parameters extraction and modification for P&L attribution.
//!
//! Provides functionality to extract model-specific parameters from instruments,
//! create modified versions with different parameters, and measure parameter shifts.

use crate::cashflow::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::convertible::{ConversionSpec, ConvertibleBond};
use crate::instruments::fixed_income::structured_credit::StructuredCredit;
use finstack_core::Error;
use finstack_core::Result;
use std::sync::Arc;

/// Snapshot of extractable model parameters from an instrument.
///
/// Different instrument types have different model parameters that affect
/// pricing. This enum captures the relevant parameters for each type.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub enum ModelParamsSnapshot {
    /// Structured credit parameters (prepayment, default, recovery).
    StructuredCredit {
        /// Prepayment model specification
        prepayment_spec: PrepaymentModelSpec,
        /// Default model specification
        default_spec: DefaultModelSpec,
        /// Recovery model specification
        recovery_spec: RecoveryModelSpec,
    },

    /// Convertible bond parameters (conversion ratio, policies).
    Convertible {
        /// Conversion specification for convertible bonds
        conversion_spec: ConversionSpec,
    },

    /// No extractable model parameters.
    None,
}

/// Extract model parameters from an instrument.
///
/// Uses downcasting to identify instrument type and extract relevant parameters.
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
/// use finstack_valuations::attribution::extract_model_params;
/// use finstack_valuations::attribution::ModelParamsSnapshot;
/// use finstack_valuations::instruments::fixed_income::structured_credit::StructuredCredit;
/// use finstack_valuations::instruments::Instrument;
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
    // Try downcasting to StructuredCredit
    if let Some(structured) = instrument.as_any().downcast_ref::<StructuredCredit>() {
        return ModelParamsSnapshot::StructuredCredit {
            prepayment_spec: structured.credit_model.prepayment_spec.clone(),
            default_spec: structured.credit_model.default_spec.clone(),
            recovery_spec: structured.credit_model.recovery_spec.clone(),
        };
    }

    // Try downcasting to ConvertibleBond
    if let Some(convertible) = instrument.as_any().downcast_ref::<ConvertibleBond>() {
        return ModelParamsSnapshot::Convertible {
            conversion_spec: convertible.conversion.clone(),
        };
    }

    // Other instrument types don't have model parameters
    ModelParamsSnapshot::None
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
/// use finstack_valuations::attribution::{extract_model_params, with_model_params};
/// use finstack_valuations::instruments::fixed_income::structured_credit::StructuredCredit;
/// use finstack_valuations::instruments::Instrument;
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
    match params {
        ModelParamsSnapshot::StructuredCredit {
            prepayment_spec,
            default_spec,
            recovery_spec,
        } => {
            if let Some(structured) = instrument.as_any().downcast_ref::<StructuredCredit>() {
                let mut modified = structured.clone();
                modified.credit_model.prepayment_spec = prepayment_spec.clone();
                modified.credit_model.default_spec = default_spec.clone();
                modified.credit_model.recovery_spec = recovery_spec.clone();
                Ok(Arc::new(modified) as Arc<dyn Instrument>)
            } else {
                Err(Error::Validation(
                    "Instrument type mismatch: expected StructuredCredit".to_string(),
                ))
            }
        }

        ModelParamsSnapshot::Convertible { conversion_spec } => {
            if let Some(convertible) = instrument.as_any().downcast_ref::<ConvertibleBond>() {
                let mut modified = convertible.clone();
                modified.conversion = conversion_spec.clone();
                Ok(Arc::new(modified) as Arc<dyn Instrument>)
            } else {
                Err(Error::Validation(
                    "Instrument type mismatch: expected ConvertibleBond".to_string(),
                ))
            }
        }

        ModelParamsSnapshot::None => {
            // No model params to modify, return original
            Ok(Arc::clone(instrument))
        }
    }
}

/// Compute a prepayment parameter shift between two snapshots.
///
/// Returns shift in basis points for use with Prepayment01 metric.
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
            use crate::cashflow::builder::specs::PrepaymentCurve;

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
/// Returns shift in basis points, or 0.0 if not applicable.
pub fn measure_prepayment_shift(
    snapshot_t0: &ModelParamsSnapshot,
    snapshot_t1: &ModelParamsSnapshot,
) -> f64 {
    if let Some(shift) = prepayment_shift(snapshot_t0, snapshot_t1) {
        return shift;
    }
    if let Some(diagnostic) = prepayment_shift_diagnostic(snapshot_t0, snapshot_t1) {
        tracing::warn!(
            reason = diagnostic.reason,
            "Model parameter prepayment shift defaulted to zero"
        );
        return diagnostic.shift;
    }

    0.0
}

/// Compute a default rate parameter shift between two snapshots.
///
/// Returns shift in basis points for use with Default01 metric.
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
/// Returns shift in basis points, or 0.0 if not applicable.
pub fn measure_default_shift(
    snapshot_t0: &ModelParamsSnapshot,
    snapshot_t1: &ModelParamsSnapshot,
) -> f64 {
    if let Some(shift) = default_shift(snapshot_t0, snapshot_t1) {
        return shift;
    }
    if let Some(diagnostic) = default_shift_diagnostic(snapshot_t0, snapshot_t1) {
        tracing::warn!(
            reason = diagnostic.reason,
            "Model parameter default shift defaulted to zero"
        );
        return diagnostic.shift;
    }

    0.0
}

/// Compute a recovery rate parameter shift between two snapshots.
///
/// Returns shift in percentage points (not basis points) for use with Recovery01 metric.
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
/// Returns shift in percentage points, or 0.0 if not applicable.
pub fn measure_recovery_shift(
    snapshot_t0: &ModelParamsSnapshot,
    snapshot_t1: &ModelParamsSnapshot,
) -> f64 {
    if let Some(shift) = recovery_shift(snapshot_t0, snapshot_t1) {
        return shift;
    }
    if let Some(diagnostic) = recovery_shift_diagnostic(snapshot_t0, snapshot_t1) {
        tracing::warn!(
            reason = diagnostic.reason,
            "Model parameter recovery shift defaulted to zero"
        );
        return diagnostic.shift;
    }

    0.0
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

#[derive(Debug, Clone, Copy, PartialEq)]
struct ShiftDiagnostic {
    shift: f64,
    reason: &'static str,
}

fn prepayment_shift_diagnostic(
    snapshot_t0: &ModelParamsSnapshot,
    snapshot_t1: &ModelParamsSnapshot,
) -> Option<ShiftDiagnostic> {
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
            use crate::cashflow::builder::specs::PrepaymentCurve;

            match (&prep_t0.curve, &prep_t1.curve) {
                (Some(PrepaymentCurve::Psa { .. }), Some(PrepaymentCurve::Psa { .. }))
                | (None, None)
                | (Some(PrepaymentCurve::Constant), Some(PrepaymentCurve::Constant)) => None,
                _ => Some(ShiftDiagnostic {
                    shift: 0.0,
                    reason: "unsupported prepayment model type transition",
                }),
            }
        }
        _ => Some(ShiftDiagnostic {
            shift: 0.0,
            reason: "snapshot type mismatch for prepayment shift",
        }),
    }
}

fn default_shift_diagnostic(
    snapshot_t0: &ModelParamsSnapshot,
    snapshot_t1: &ModelParamsSnapshot,
) -> Option<ShiftDiagnostic> {
    match (snapshot_t0, snapshot_t1) {
        (
            ModelParamsSnapshot::StructuredCredit { .. },
            ModelParamsSnapshot::StructuredCredit { .. },
        ) => None,
        _ => Some(ShiftDiagnostic {
            shift: 0.0,
            reason: "snapshot type mismatch for default shift",
        }),
    }
}

fn recovery_shift_diagnostic(
    snapshot_t0: &ModelParamsSnapshot,
    snapshot_t1: &ModelParamsSnapshot,
) -> Option<ShiftDiagnostic> {
    match (snapshot_t0, snapshot_t1) {
        (
            ModelParamsSnapshot::StructuredCredit { .. },
            ModelParamsSnapshot::StructuredCredit { .. },
        ) => None,
        _ => Some(ShiftDiagnostic {
            shift: 0.0,
            reason: "snapshot type mismatch for recovery shift",
        }),
    }
}

fn conversion_shift_diagnostic(
    snapshot_t0: &ModelParamsSnapshot,
    snapshot_t1: &ModelParamsSnapshot,
) -> Option<ShiftDiagnostic> {
    match (snapshot_t0, snapshot_t1) {
        (
            ModelParamsSnapshot::Convertible {
                conversion_spec: conv_t0,
            },
            ModelParamsSnapshot::Convertible {
                conversion_spec: conv_t1,
            },
        ) => match (conv_t0.ratio, conv_t1.ratio) {
            (Some(ratio_t0), Some(_)) if ratio_t0 != 0.0 => None,
            _ => Some(ShiftDiagnostic {
                shift: 0.0,
                reason: "missing or zero conversion ratio for conversion shift",
            }),
        },
        _ => Some(ShiftDiagnostic {
            shift: 0.0,
            reason: "snapshot type mismatch for conversion shift",
        }),
    }
}

/// Measure conversion ratio shift between two snapshots.
///
/// Returns shift in percentage points, or 0.0 if not applicable.
pub fn measure_conversion_shift(
    snapshot_t0: &ModelParamsSnapshot,
    snapshot_t1: &ModelParamsSnapshot,
) -> f64 {
    if let Some(shift) = conversion_shift(snapshot_t0, snapshot_t1) {
        return shift;
    }
    if let Some(diagnostic) = conversion_shift_diagnostic(snapshot_t0, snapshot_t1) {
        tracing::warn!(
            reason = diagnostic.reason,
            "Model parameter conversion shift defaulted to zero"
        );
        return diagnostic.shift;
    }

    0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::convertible::{
        AntiDilutionPolicy, ConversionPolicy, DividendAdjustment,
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

    #[test]
    fn test_measure_prepayment_shift_diagnostic_for_mismatched_snapshot_types() {
        let params_t0 = ModelParamsSnapshot::StructuredCredit {
            prepayment_spec: PrepaymentModelSpec::psa(1.0),
            default_spec: DefaultModelSpec::constant_cdr(0.02),
            recovery_spec: RecoveryModelSpec::with_lag(0.60, 12),
        };
        let params_t1 = ModelParamsSnapshot::Convertible {
            conversion_spec: test_conversion_spec(None),
        };

        let diagnostic = prepayment_shift_diagnostic(&params_t0, &params_t1)
            .expect("mismatched snapshots should produce a diagnostic");

        assert_eq!(diagnostic.shift, 0.0);
        assert!(diagnostic.reason.contains("snapshot type mismatch"));
    }

    #[test]
    fn test_measure_conversion_shift_diagnostic_for_missing_ratios() {
        let params_t0 = ModelParamsSnapshot::Convertible {
            conversion_spec: test_conversion_spec(None),
        };
        let params_t1 = ModelParamsSnapshot::Convertible {
            conversion_spec: test_conversion_spec(None),
        };

        let diagnostic = conversion_shift_diagnostic(&params_t0, &params_t1)
            .expect("missing conversion ratios should produce a diagnostic");

        assert_eq!(diagnostic.shift, 0.0);
        assert!(diagnostic.reason.contains("conversion ratio"));
    }

    fn test_conversion_spec(ratio: Option<f64>) -> ConversionSpec {
        use crate::instruments::fixed_income::convertible::{
            AntiDilutionPolicy, ConversionPolicy, DividendAdjustment,
        };

        ConversionSpec {
            ratio,
            price: None,
            policy: ConversionPolicy::Voluntary,
            anti_dilution: AntiDilutionPolicy::None,
            dividend_adjustment: DividendAdjustment::None,
            dilution_events: Vec::new(),
        }
    }
}
