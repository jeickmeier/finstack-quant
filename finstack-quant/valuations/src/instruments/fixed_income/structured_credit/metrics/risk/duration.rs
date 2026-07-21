//! Duration calculators for structured credit.

use crate::cashflow::traits::DatedFlows;
use crate::constants::ONE_BASIS_POINT;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

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
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: "context.cashflows".to_string(),
            })
        })?;

        // Get discount curve
        let disc_curve_id = context.discount_curve_id.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: "discount_curve_id".to_string(),
            })
        })?;

        let disc = context.curves.get_discount(disc_curve_id.as_str())?;

        // SC-m02: the shared metric time basis, NOT the curve's own day count.
        // Duration is combined with convexity in the second-order price
        // expansion, and convexity measures time in Act/365F — mixing the two
        // made `D` and `C` incommensurable. See `METRIC_TIME_BASIS`.
        let day_count =
            crate::instruments::fixed_income::structured_credit::metrics::METRIC_TIME_BASIS;

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
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: "context.cashflows".to_string(),
            })
        })?;

        // Get discount curve
        let disc_curve_id = context.discount_curve_id.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: "discount_curve_id".to_string(),
            })
        })?;

        let disc = context.curves.get_discount(disc_curve_id.as_str())?;

        // SC-m02: the shared metric time basis, NOT the curve's own day count.
        // Duration is combined with convexity in the second-order price
        // expansion, and convexity measures time in Act/365F — mixing the two
        // made `D` and `C` incommensurable. See `METRIC_TIME_BASIS`.
        let day_count =
            crate::instruments::fixed_income::structured_credit::metrics::METRIC_TIME_BASIS;

        // Shift yield by 1bp
        let yield_shift = ONE_BASIS_POINT;

        // Calculate PV with shifted discount factors
        let mut shifted_npv = 0.0;

        for (date, amount) in flows {
            if *date <= context.as_of {
                continue;
            }

            // Calculate the spread bump time from valuation, not curve base.
            let t = day_count.year_fraction(context.as_of, *date, DayCountContext::default())?;

            // Get base discount factor
            let df = disc.df_between_dates(context.as_of, *date)?;

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

    let day_count = crate::instruments::fixed_income::structured_credit::metrics::METRIC_TIME_BASIS;
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

#[cfg(test)]
mod time_basis_tests {
    use super::*;
    use crate::instruments::fixed_income::structured_credit::metrics::risk::convexity::calculate_tranche_convexity;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use time::Month;

    fn as_of() -> Date {
        Date::from_calendar_date(2024, Month::January, 1).expect("date")
    }

    /// Two curves identical except for their day-count convention.
    fn curve(day_count: DayCount) -> DiscountCurve {
        DiscountCurve::builder("USD-TEST")
            .base_date(as_of())
            .day_count(day_count)
            .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.80), (10.0, 0.62)])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("curve")
    }

    fn flows() -> DatedFlows {
        (1..=10)
            .map(|y| {
                (
                    Date::from_calendar_date(2024 + y, Month::January, 1).expect("date"),
                    Money::new(if y == 10 { 1_050_000.0 } else { 50_000.0 }, Currency::USD),
                )
            })
            .collect()
    }

    /// SC-m02 — duration and convexity must be measured on the SAME time
    /// basis, so the second-order price expansion is self-consistent:
    ///
    ///     dP/P ~= -D*dy + 0.5*C*dy^2
    ///
    /// Duration used the DISCOUNT CURVE's day count while convexity, z-spread,
    /// CS01, discount margin and OAS all used Act/365F. On an Act/360 curve
    /// that is a 1.39% relative difference in `t`, so `D` and `C` were measured
    /// against different yield units and combining them was meaningless.
    ///
    /// This tests the composition directly rather than comparing two curves:
    /// changing a curve's day count also changes how its knots interpolate, so
    /// two such curves are not economically identical and a direct comparison
    /// would conflate the two effects.
    #[test]
    fn duration_and_convexity_compose_on_a_shared_time_basis() {
        // An Act/360 curve — the case where the two bases used to diverge.
        let disc = curve(DayCount::Act360);
        let cf = flows();
        let basis = crate::instruments::fixed_income::structured_credit::metrics::METRIC_TIME_BASIS;

        // Base PV, and PV under a parallel shift, both measured in the metric
        // time basis so the expansion's `dy` is in those units.
        let pv_at = |shift: f64| -> f64 {
            let mut total = 0.0;
            for (date, amount) in cf.iter() {
                if *date <= as_of() {
                    continue;
                }
                let t = basis
                    .year_fraction(
                        as_of(),
                        *date,
                        finstack_quant_core::dates::DayCountContext::default(),
                    )
                    .expect("year fraction");
                let df = disc.df_between_dates(as_of(), *date).expect("df");
                total += amount.amount() * df * (-shift * t).exp();
            }
            total
        };

        let p0 = pv_at(0.0);
        let pv = Money::new(p0, Currency::USD);
        let d = calculate_tranche_duration(&cf, &disc, as_of(), pv).expect("duration");
        let c = calculate_tranche_convexity(&cf, &disc, as_of()).expect("convexity");

        // 10bp. Sizing matters here: a mismatched basis biases `D` by the
        // Act/360-vs-Act/365F ratio (~1.39%), giving an error of about
        // 0.0139 * D * dy ~ 1.1e-4, while the third-order truncation this
        // expansion neglects is ~3e-7 at this bump. Signal exceeds truncation
        // by two orders of magnitude, so the tolerance below discriminates.
        const DY: f64 = 0.001;
        let actual = (pv_at(DY) - p0) / p0;
        let predicted = -d * DY + 0.5 * c * DY * DY;

        assert!(
            (actual - predicted).abs() < 1e-5,
            "the second-order expansion must reproduce the repriced move: \
             actual {actual:.8}, predicted {predicted:.8} (D={d:.6}, C={c:.6}). \
             A gap means duration and convexity are measured on different time \
             bases (SC-m02)."
        );
    }
}
