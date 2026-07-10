use super::engine::CDSPricer;
use super::helpers::{
    date_from_hazard_time, df_asof_to, isda_standard_model_boundaries, settlement_date,
};
use crate::constants::{credit, numerical};
use finstack_quant_core::dates::{Date, HolidayCalendar};
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::{Error, Result};

/// Shared inputs for the conditional protection-leg integrators.
///
/// Groups the parameters common to the supported protection-leg integrators so
/// they can be passed as a single reference.
#[derive(Clone, Copy)]
pub(super) struct ProtectionLegInputs<'a> {
    /// Time (years) to protection window start, measured from `as_of`.
    pub t_start: f64,
    /// Time (years) to protection window end, measured from `as_of`.
    pub t_end: f64,
    /// Recovery rate on default (0..1). LGD = 1 - recovery.
    pub recovery: f64,
    /// Settlement delay in business days applied to the default date.
    pub settlement_delay: u16,
    /// Optional business-day calendar for the settlement-date shift.
    pub calendar: Option<&'a dyn HolidayCalendar>,
    /// Survival probability at `as_of` used to condition later hazard times.
    pub sp_asof: f64,
    /// Pricing valuation date.
    pub as_of: Date,
    /// Discount curve used to value the LGD cashflows.
    pub disc: &'a DiscountCurve,
    /// Hazard / survival curve producing default densities.
    pub surv: &'a HazardCurve,
}

impl CDSPricer {
    /// ISDA Standard Model with conditional survival and relative discounting
    pub(super) fn protection_leg_isda_standard_model_cond(
        &self,
        inputs: &ProtectionLegInputs<'_>,
    ) -> Result<f64> {
        let ProtectionLegInputs {
            t_start,
            t_end,
            recovery,
            settlement_delay,
            calendar,
            sp_asof,
            as_of,
            disc,
            surv,
        } = *inputs;

        if t_start >= t_end {
            return Err(Error::Validation(format!(
                "Protection leg start time ({}) must be before end time ({})",
                t_start, t_end
            )));
        }
        if sp_asof <= credit::SURVIVAL_PROBABILITY_FLOOR {
            return Ok(0.0);
        }

        let lgd = 1.0 - recovery;
        let boundaries = isda_standard_model_boundaries(
            t_start,
            t_end,
            surv,
            disc,
            self.config.protection_leg_substeps_per_year,
        )?;
        let mut integral = 0.0;

        for window in boundaries.windows(2) {
            let t1 = window[0];
            let t2 = window[1];
            let dt = t2 - t1;
            if dt <= numerical::ZERO_TOLERANCE {
                continue;
            }

            // Conditional survival probabilities
            let sp1 = surv.sp(t1) / sp_asof;
            let sp2 = surv.sp(t2) / sp_asof;

            if sp1 > sp2 && sp1 > 0.0 {
                // Piecewise constant hazard rate for this interval
                let hazard_rate = -(sp2 / sp1).ln() / dt;

                // Relative discount factors from as_of
                let d1 = settlement_date(
                    date_from_hazard_time(surv, t1),
                    settlement_delay,
                    calendar,
                    self.config.business_days_per_year,
                )?;
                let d2 = settlement_date(
                    date_from_hazard_time(surv, t2),
                    settlement_delay,
                    calendar,
                    self.config.business_days_per_year,
                )?;
                let df1 = df_asof_to(disc, as_of, d1)?;
                let df2 = df_asof_to(disc, as_of, d2)?;

                // Piecewise constant interest rate (allow negative rates)
                let interest_rate = if df1 > 0.0 && df2 > 0.0 {
                    -(df2 / df1).ln() / dt
                } else {
                    0.0
                };

                // ISDA Standard Model analytical integration
                let lambda_plus_r = hazard_rate + interest_rate;

                if lambda_plus_r.abs() > numerical::ZERO_TOLERANCE {
                    let exp_term = (-lambda_plus_r * dt).exp();
                    integral += df1 * sp1 * (hazard_rate / lambda_plus_r) * (1.0 - exp_term);
                } else {
                    integral += df1 * sp1 * hazard_rate * dt;
                }
            }
        }

        Ok(integral * lgd)
    }
}
