use crate::metrics::{MetricCalculator, MetricContext, MetricId};

/// Minimum year fraction for par rate calculation.
///
/// Year fractions below this threshold are considered too small for meaningful
/// par rate calculation (approximately 8.6 seconds).
const MIN_YEAR_FRACTION: f64 = 1e-6;

/// Calculates par rate for deposits.
///
/// Computes the simple rate that makes the deposit worth par (face value) at
/// inception using: (DF(start) / DF(end) - 1) / year_fraction.
///
/// # Dependencies
/// Requires `DfStart`, `DfEnd`, and `Yf` metrics to be computed first.
///
/// # Errors
/// Returns an error if:
/// - Any required metric is missing
/// - Year fraction is too small (< 1e-6) for meaningful calculation
/// - Division would produce extreme values
pub(crate) struct DepositParRateCalculator;

impl MetricCalculator for DepositParRateCalculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::DfStart, MetricId::DfEnd, MetricId::Yf]
    }

    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let df_s = context
            .computed
            .get(&MetricId::DfStart)
            .copied()
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "DfStart (required for par rate calculation)".to_string(),
                })
            })?;
        let df_e = context
            .computed
            .get(&MetricId::DfEnd)
            .copied()
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "DfEnd (required for par rate calculation)".to_string(),
                })
            })?;
        let yf = context
            .computed
            .get(&MetricId::Yf)
            .copied()
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "Yf (required for par rate calculation)".to_string(),
                })
            })?;

        // Guard against near-zero year fractions that would produce extreme rates
        if yf.abs() < MIN_YEAR_FRACTION {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Year fraction {:.2e} is too small for par rate calculation (minimum: {:.0e})",
                yf, MIN_YEAR_FRACTION
            )));
        }

        // Guard against zero or negative discount factors
        if df_e <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "End discount factor must be positive, got {:.6}",
                df_e
            )));
        }

        Ok((df_s / df_e - 1.0) / yf)
    }
}
