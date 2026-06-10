//! Duration calculators for structured credit.

use crate::cashflow::traits::DatedFlows;
use crate::constants::ONE_BASIS_POINT;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::dates::{Date, DayCount, DayCountContext};
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::money::Money;
use finstack_core::Result;

/// Calculates Macaulay duration for structured credit.
///
/// Macaulay duration measures the weighted average time to receive cashflows,
/// where weights are the present values of each cashflow. This is the fundamental
/// measure of interest rate sensitivity.
///
/// # Formula
///
/// Macaulay Duration = Σ(PV_i × t_i) / Price
///
/// Where:
/// - PV_i = present value of cashflow i
/// - t_i = time in years to cashflow i
/// - Price = total present value (dirty price)
///
/// # Market Conventions
///
/// - **CLO (floating)**: Typically 0.1-0.3 years (very low IR duration)
/// - **ABS (fixed)**: Typically 2-4 years
/// - **RMBS (fixed)**: Typically 3-6 years (depends on prepayments)
/// - **CMBS (fixed)**: Typically 4-7 years
///
pub struct MacaulayDurationCalculator;

impl MetricCalculator for MacaulayDurationCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        // Get cashflows
        let flows = context.cashflows.as_ref().ok_or_else(|| {
            finstack_core::Error::from(finstack_core::InputError::NotFound {
                id: "context.cashflows".to_string(),
            })
        })?;

        // Get discount curve
        let disc_curve_id = context.discount_curve_id.as_ref().ok_or_else(|| {
            finstack_core::Error::from(finstack_core::InputError::NotFound {
                id: "discount_curve_id".to_string(),
            })
        })?;

        let disc = context.curves.get_discount(disc_curve_id.as_str())?;

        // Use the discount curve's day count for market-standard consistency
        // (e.g., 30/360 for US Agency RMBS, Act/360 for CLOs)
        let day_count = disc.day_count();

        let mut weighted_pv = 0.0;
        let mut total_pv = 0.0;

        for (date, amount) in flows {
            if *date <= context.as_of {
                continue;
            }

            // Calculate time in years
            let years =
                day_count.year_fraction(context.as_of, *date, DayCountContext::default())?;

            // Get discount factor
            let df = disc.df_on_date_curve(*date)?;

            // Calculate present value
            let pv = amount.amount() * df;

            // Accumulate weighted PV
            weighted_pv += pv * years;
            total_pv += pv;
        }

        // Calculate Macaulay duration
        if total_pv > 0.0 {
            Ok(weighted_pv / total_pv)
        } else {
            Ok(0.0)
        }
    }
}

/// Calculates modified duration for structured credit.
///
/// Modified duration measures the percentage price change for a 1% change in yield.
/// It's the primary measure used for interest rate risk management.
///
/// # Formula
///
/// Modified Duration = Macaulay Duration / (1 + y)
///
/// Where y is the yield. For simplicity, we approximate using a small yield bump
/// and measure the actual price sensitivity.
///
/// # Interpretation
///
/// A modified duration of 3.5 means that for a 1% (100bp) increase in yield,
/// the price would decrease by approximately 3.5%.
///
pub struct ModifiedDurationCalculator;

impl MetricCalculator for ModifiedDurationCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        // For structured credit, we use a numerical approach:
        // Calculate price sensitivity to a small yield shift

        // Get base NPV
        let base_npv = context.base_value.amount();

        if base_npv == 0.0 {
            return Ok(0.0);
        }

        // Get cashflows
        let flows = context.cashflows.as_ref().ok_or_else(|| {
            finstack_core::Error::from(finstack_core::InputError::NotFound {
                id: "context.cashflows".to_string(),
            })
        })?;

        // Get discount curve
        let disc_curve_id = context.discount_curve_id.as_ref().ok_or_else(|| {
            finstack_core::Error::from(finstack_core::InputError::NotFound {
                id: "discount_curve_id".to_string(),
            })
        })?;

        let disc = context.curves.get_discount(disc_curve_id.as_str())?;

        // Use the discount curve's day count for market-standard consistency
        // (e.g., 30/360 for US Agency RMBS, Act/360 for CLOs)
        let day_count = disc.day_count();
        let base_date = disc.base_date();

        // Shift yield by 1bp
        let yield_shift = ONE_BASIS_POINT;

        // Calculate PV with shifted discount factors
        let mut shifted_npv = 0.0;

        for (date, amount) in flows {
            if *date <= context.as_of {
                continue;
            }

            // Calculate time from curve base date
            let t = day_count.year_fraction(base_date, *date, DayCountContext::default())?;

            // Get base discount factor
            let df = disc.df_on_date_curve(*date)?;

            // Apply yield shift: df_shifted = df * exp(-shift * t)
            let df_shifted = df * (-yield_shift * t).exp();

            shifted_npv += amount.amount() * df_shifted;
        }

        // Modified duration = -(dP/dy) / P
        // Where dP = shifted_npv - base_npv, dy = yield_shift
        let price_change = shifted_npv - base_npv;
        let modified_duration = -(price_change / base_npv) / yield_shift;

        Ok(modified_duration)
    }
}

/// Calculate tranche-specific modified duration from cashflows and discount curve.
///
/// True modified duration: `-(1/P)·dP/dy` measured by bumping the
/// continuously-compounded discounting of the tranche's cashflows by 1bp
/// (the same approach as [`ModifiedDurationCalculator`]). The previous
/// implementation returned the Macaulay duration (PV-weighted time) under
/// this name.
///
/// # Arguments
///
/// * `cashflows` - The dated cashflows for the tranche
/// * `discount_curve` - The discount curve for PV calculation
/// * `as_of` - The valuation date
/// * `pv` - The present value of the tranche (guards the degenerate case)
///
/// # Returns
///
/// Modified duration in years
pub fn calculate_tranche_duration(
    cashflows: &DatedFlows,
    discount_curve: &DiscountCurve,
    as_of: Date,
    pv: Money,
) -> Result<f64> {
    if pv.amount() <= 0.0 {
        return Ok(0.0);
    }

    let day_count = DayCount::Act365F;
    let yield_shift = ONE_BASIS_POINT;

    let mut base_pv = 0.0;
    let mut shifted_pv = 0.0;

    for (date, amount) in cashflows {
        if *date <= as_of {
            continue;
        }

        let years = day_count.year_fraction(as_of, *date, DayCountContext::default())?;

        let df = discount_curve.df_between_dates(as_of, *date)?;
        let flow_pv = amount.amount() * df;

        base_pv += flow_pv;
        shifted_pv += flow_pv * (-yield_shift * years).exp();
    }

    if base_pv > 0.0 {
        // Modified duration = -(dP/dy) / P on the cashflow-discounted PV, so
        // numerator and denominator come from the same discounting.
        Ok(-(shifted_pv - base_pv) / (base_pv * yield_shift))
    } else {
        Ok(0.0)
    }
}
