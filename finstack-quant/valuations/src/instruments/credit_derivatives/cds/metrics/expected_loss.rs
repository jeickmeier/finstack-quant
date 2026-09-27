//! Expected Loss metric for single-name CDS.
//!
//! Calculates the expected loss at maturity using the market-standard formula:
//! EL = Notional × PD × LGD
//!
//! Where:
//! - PD (Probability of Default) = 1 - Survival Probability to maturity
//! - LGD (Loss Given Default) = 1 - Recovery Rate

use crate::constants::credit;
use crate::instruments::credit_derivatives::cds::CreditDefaultSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::dates::DayCountContext;
use finstack_quant_core::Result;

/// Expected Loss calculator for single-name CDS.
pub(crate) struct ExpectedLossCalculator;

impl MetricCalculator for ExpectedLossCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let cds: &CreditDefaultSwap = context.instrument_as()?;
        let as_of = context.as_of;

        let hazard = context
            .curves
            .get_hazard(cds.protection_leg.credit_curve_id.as_str())?;

        // If already at/after maturity, no forward expected loss (under deterministic recovery).
        if as_of >= cds.premium_leg.end {
            return Ok(0.0);
        }

        // Use hazard curve's day-count for the survival time axis.
        let day_count = hazard.day_count();
        let base_date = hazard.base_date();
        let t_asof = day_count.year_fraction(base_date, as_of, DayCountContext::default())?;
        let t_maturity =
            day_count.year_fraction(base_date, cds.premium_leg.end, DayCountContext::default())?;

        // Conditional survival to maturity given survival to as_of.
        let sp_asof = hazard.sp(t_asof);
        if sp_asof <= credit::SURVIVAL_PROBABILITY_FLOOR {
            return Ok(0.0);
        }
        let sp_maturity = hazard.sp(t_maturity);
        let survival_cond = sp_maturity / sp_asof;

        // Default probability
        let default_prob = (1.0 - survival_cond).clamp(0.0, 1.0);

        // Loss given default
        let lgd = 1.0 - cds.protection_leg.recovery_rate;

        // Expected loss in currency units
        let expected_loss = cds.notional.amount() * default_prob * lgd;

        Ok(expected_loss)
    }
}
