//! AssetPool characteristic metrics for structured credit.

use crate::instruments::fixed_income::structured_credit::assumptions::embedded_registry;
use crate::instruments::fixed_income::structured_credit::utils::rates::psa_to_cpr;
use crate::instruments::fixed_income::structured_credit::StructuredCredit;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::dates::DateExt;
use finstack_quant_core::Result;

/// Calculates WAM (Weighted Average Maturity) for the underlying pool.
///
/// WAM measures the weighted average time until assets in the pool mature,
/// based on the original legal maturities (not expected prepayment life like WAL).
///
/// # Formula
///
/// WAM = Σ(Balance_i × Years_to_Maturity_i) / Σ(Balance_i)
///
/// # Typical Values
///
/// - **CLO**: 5-7 years
/// - **ABS**: 3-5 years
/// - **RMBS**: 25-30 years (legal maturity, much longer than WAL)
/// - **CMBS**: 7-10 years
///
pub struct WamCalculator;

impl MetricCalculator for WamCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        // Try to extract WAM from pool
        // This uses the pool's weighted_avg_maturity method if available

        let as_of = context.as_of;

        // Single check for unified structured credit type
        if let Some(sc) = context
            .instrument
            .as_any()
            .downcast_ref::<StructuredCredit>()
        {
            return Ok(sc.pool.weighted_avg_maturity(as_of));
        }

        // Fallback: return 0
        Ok(0.0)
    }
}

/// Calculates CPR (Constant Prepayment Rate) assumption.
///
/// CPR is the annualized rate at which the pool is assumed to prepay.
/// This is an assumption/input rather than a calculated output.
///
/// # Formula
///
/// CPR = annualized prepayment rate (e.g., 0.15 = 15% CPR)
///
/// # Sources
///
/// - **RMBS**: Derived from PSA speed
/// - **ABS**: From ABS speed assumption
/// - **CMBS**: From open period CPR
/// - **CLO**: Default prepayment assumption
///
pub struct CprCalculator;

impl MetricCalculator for CprCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        // Extract CPR from behavior overrides or use deal-type defaults

        if let Some(sc) = context
            .instrument
            .as_any()
            .downcast_ref::<StructuredCredit>()
        {
            // Check overrides first
            if let Some(cpr) = sc.behavior_overrides.cpr_annual {
                return Ok(cpr);
            }

            if let Some(psa_mult) = sc.behavior_overrides.psa_speed_multiplier {
                return Ok(psa_to_cpr(
                    psa_mult,
                    deal_seasoning_month(sc, context.as_of),
                ));
            }

            use super::super::super::types::DealType;
            let registry = embedded_registry()?;
            let assumptions =
                registry.default_assumptions(registry.profile_id_for_deal_type(sc.deal_type))?;
            return Ok(match sc.deal_type {
                DealType::RMBS => psa_to_cpr(
                    assumptions.psa_speed.unwrap_or(1.0),
                    deal_seasoning_month(sc, context.as_of),
                ),
                _ => assumptions.base_cpr_annual,
            });
        }

        Ok(0.0)
    }
}

/// Calculates CDR (Constant Default Rate) assumption.
///
/// CDR is the annualized rate at which the pool is assumed to default.
/// This is an assumption/input rather than a calculated output.
///
/// # Formula
///
/// CDR = annualized default rate (e.g., 0.02 = 2% CDR)
///
/// # Typical Values
///
/// - **CLO**: 2-3% base case, 5-10% stress
/// - **ABS (auto)**: 1-2% base case
/// - **RMBS**: 0.5-1% base case (agency)
/// - **CMBS**: 0.5-1% base case
///
pub struct CdrCalculator;

impl MetricCalculator for CdrCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        // Extract CDR from behavior overrides or use deal-type defaults

        if let Some(sc) = context
            .instrument
            .as_any()
            .downcast_ref::<StructuredCredit>()
        {
            // Check overrides first
            if let Some(cdr) = sc.behavior_overrides.cdr_annual {
                return Ok(cdr);
            }

            if let Some(sda_mult) = sc.behavior_overrides.sda_speed_multiplier {
                return sda_to_cdr(sda_mult, deal_seasoning_month(sc, context.as_of));
            }

            use super::super::super::types::DealType;
            let registry = embedded_registry()?;
            let assumptions =
                registry.default_assumptions(registry.profile_id_for_deal_type(sc.deal_type))?;
            return Ok(match sc.deal_type {
                DealType::RMBS => sda_to_cdr(
                    assumptions.sda_speed.unwrap_or(1.0),
                    deal_seasoning_month(sc, context.as_of),
                )?,
                _ => assumptions.base_cdr_annual,
            });
        }

        Ok(0.0)
    }
}

fn deal_seasoning_month(sc: &StructuredCredit, as_of: finstack_quant_core::dates::Date) -> u32 {
    if as_of > sc.closing_date {
        sc.closing_date.months_until(as_of).max(1)
    } else {
        1
    }
}

fn sda_to_cdr(speed_multiplier: f64, month: u32) -> Result<f64> {
    let speed_multiplier = speed_multiplier.max(0.0);
    if speed_multiplier == 0.0 {
        return Ok(0.0);
    }

    // Canonical PSA SDA shape (ramp / plateau / decline / terminal) lives on
    // `SdaCurveDefaults::cdr_at` — do not re-implement the segments here.
    let cdr = embedded_registry()?.sda_curve().cdr_at(month);

    Ok((cdr * speed_multiplier).clamp(0.0, 1.0))
}
