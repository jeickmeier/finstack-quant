//! YTM (Yield to Maturity) calculator for structured credit.

use crate::instruments::fixed_income::structured_credit::types::constants::YTM_SOLVER_TOLERANCE;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::dates::DayCountContext;
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use finstack_quant_core::Result;
use serde::Deserialize;

/// Extension key for structured-credit YTM settings.
pub(crate) const STRUCTURED_CREDIT_YTM_CONFIG_KEY_V1: &str = "valuations.structured_credit.ytm.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
enum YtmCompounding {
    #[default]
    Annual,
    SemiAnnual,
    Quarterly,
    Monthly,
}

impl YtmCompounding {
    fn periods_per_year(self) -> f64 {
        match self {
            Self::Annual => 1.0,
            Self::SemiAnnual => 2.0,
            Self::Quarterly => 4.0,
            Self::Monthly => 12.0,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct StructuredCreditYtmConfigV1 {
    #[serde(default)]
    compounding: Option<YtmCompounding>,
}

/// Calculates YTM (Yield to Maturity) for structured credit.
///
/// YTM is the internal rate of return that equates the present value of
/// all future cashflows to the current price. For structured credit, this
/// is most relevant for fixed-rate tranches.
///
/// # Formula
///
/// Solve for y such that:
/// ```text
/// Σ CF_i / (1 + y)^t_i = Dirty Price
/// ```
///
/// # Compounding Convention
///
/// Uses **annual compounding** by default: discount factor = `(1 + y)^(-t)`.
/// Market-specific conventions may differ:
/// - **ABS**: Often semi-annual (bond-equivalent yield)
/// - **RMBS**: Often monthly (mortgage-equivalent yield)
/// - **CLO**: Often quarterly (matching coupon frequency)
/// - **CMBS**: Often semi-annual
///
/// # Typical Yield Ranges
///
/// - **ABS (fixed)**: 4-7% typical for AAA
/// - **RMBS (fixed)**: 4-6% typical for agency
/// - **CMBS (fixed)**: 5-7% typical
/// - **CLO (floating)**: Less meaningful (use Z-spread instead)
///
/// # Note
///
/// For structured credit, **Z-spread is generally more important than YTM**
/// because it properly accounts for the term structure of rates.
///
pub struct YtmCalculator;

impl YtmCalculator {
    fn compounding_from_config(
        context: &MetricContext,
    ) -> finstack_quant_core::Result<YtmCompounding> {
        if let Some(raw) = context
            .config()
            .extensions
            .get(STRUCTURED_CREDIT_YTM_CONFIG_KEY_V1)
        {
            let cfg: StructuredCreditYtmConfigV1 =
                serde_json::from_value(raw.clone()).map_err(|e| {
                    finstack_quant_core::Error::Calibration {
                        message: format!(
                            "Failed to parse extension '{}': {}",
                            STRUCTURED_CREDIT_YTM_CONFIG_KEY_V1, e
                        ),
                        category: "config".to_string(),
                    }
                })?;
            return Ok(cfg.compounding.unwrap_or_default());
        }
        Ok(YtmCompounding::default())
    }
}

impl MetricCalculator for YtmCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        // Get dirty price (target value in percentage)
        let dirty_price = context
            .computed
            .get(&MetricId::DirtyPrice)
            .copied()
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "metric:DirtyPrice".to_string(),
                })
            })?;

        // Get cashflows
        let flows = context.cashflows.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: "context.cashflows".to_string(),
            })
        })?;

        // Convert price points back to currency using original notional.
        let notional = crate::instruments::fixed_income::structured_credit::metrics::pricing::prices::get_original_notional(context)?;
        let target_value = notional * (dirty_price / 100.0);

        if flows.is_empty() {
            return Ok(0.0);
        }

        // Day count for year fractions
        let day_count = finstack_quant_core::dates::DayCount::Act365F;
        let compounding = Self::compounding_from_config(context)?;
        let periods_per_year = compounding.periods_per_year();

        // Objective function: PV(y) - target = 0
        let objective = |y: f64| -> f64 {
            let base = 1.0 + y / periods_per_year;
            if base <= 0.0 {
                return f64::INFINITY;
            }
            let mut pv = finstack_quant_core::math::summation::NeumaierAccumulator::new();
            for (date, amount) in flows {
                if *date <= context.as_of {
                    continue;
                }

                let t = day_count
                    .year_fraction(context.as_of, *date, DayCountContext::default())
                    .unwrap_or(0.0);

                if t > 0.0 {
                    let df = base.powf(-periods_per_year * t);
                    pv.add(amount.amount() * df);
                }
            }
            pv.total() - target_value
        };

        // Solve for YTM using Brent solver
        // Tolerance: 1e-6 = 0.01 bps precision (market standard)
        let solver = BrentSolver::new().tolerance(YTM_SOLVER_TOLERANCE);

        // Initial guess: 5% is reasonable for structured credit
        let ytm = solver.solve(objective, 0.05)?;

        Ok(ytm)
    }

    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::DirtyPrice]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ytm_compounding_periods_per_year() {
        assert_eq!(YtmCompounding::Annual.periods_per_year(), 1.0);
        assert_eq!(YtmCompounding::SemiAnnual.periods_per_year(), 2.0);
        assert_eq!(YtmCompounding::Quarterly.periods_per_year(), 4.0);
        assert_eq!(YtmCompounding::Monthly.periods_per_year(), 12.0);
    }

    #[test]
    fn ytm_config_deserializes_compounding() {
        let raw = serde_json::json!({ "compounding": "quarterly" });
        let parsed: std::result::Result<StructuredCreditYtmConfigV1, _> =
            serde_json::from_value(raw);
        assert!(
            parsed.is_ok(),
            "config should deserialize, got {:?}",
            parsed.err()
        );
        if let Ok(cfg) = parsed {
            assert_eq!(cfg.compounding, Some(YtmCompounding::Quarterly));
        }
    }
}
