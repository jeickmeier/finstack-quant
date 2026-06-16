//! Expected Loss metric for CDS Index.
//!
//! Calculates the expected loss at maturity for a CDS index using:
//! EL = Notional × PD × LGD
//!
//! For a CDS index, we use the index-level hazard curve and average recovery rate,
//! which represents the expected loss across all constituents assuming equal weighting.

use crate::constants::credit;
use crate::instruments::credit_derivatives::cds_index::CDSIndex;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::dates::DayCountContext;
use finstack_quant_core::Result;

/// Expected Loss calculator for CDS Index.
pub(crate) struct ExpectedLossCalculator;

impl MetricCalculator for ExpectedLossCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let index: &CDSIndex = context.instrument_as()?;

        let as_of = context.as_of;

        // If already at/after maturity, no forward expected loss.
        if as_of >= index.premium.end {
            return Ok(0.0);
        }

        let scale = index.index_factor;
        let notional = index.notional.amount() * scale;

        if !index.constituents.is_empty() {
            // Defaulted constituents have already been settled and are
            // removed from forward exposure (their notional is captured by
            // `index_factor`). Exclude them from the forward EL and weight
            // surviving names so they sum to 1.
            let active: Vec<_> = index.constituents.iter().filter(|c| !c.defaulted).collect();
            if active.is_empty() {
                return Ok(0.0);
            }
            let sum_w: f64 = active.iter().map(|c| c.weight).sum();
            if sum_w <= 0.0 {
                return Ok(0.0);
            }

            let mut weighted_el = 0.0;
            for constituent in active {
                let hazard = context
                    .curves
                    .get_hazard(constituent.credit.credit_curve_id.as_str())?;
                let dc = hazard.day_count();
                let base_date = hazard.base_date();
                let t_asof = dc.year_fraction(base_date, as_of, DayCountContext::default())?;
                let t_maturity =
                    dc.year_fraction(base_date, index.premium.end, DayCountContext::default())?;

                let sp_asof = hazard.sp(t_asof);
                let sp_maturity = hazard.sp(t_maturity);
                // Use survival probability floor to prevent division by near-zero
                let survival_cond = if sp_asof > credit::SURVIVAL_PROBABILITY_FLOOR {
                    sp_maturity / sp_asof
                } else {
                    0.0
                };

                let default_prob = (1.0 - survival_cond).clamp(0.0, 1.0);
                let lgd = 1.0 - constituent.credit.recovery_rate;
                weighted_el += (constituent.weight / sum_w) * default_prob * lgd;
            }

            return Ok(notional * weighted_el);
        }

        // Index-level expected loss using the index hazard curve and recovery.
        let hazard = context
            .curves
            .get_hazard(index.protection.credit_curve_id.as_str())?;
        let dc = hazard.day_count();
        let base_date = hazard.base_date();
        let t_asof = dc.year_fraction(base_date, as_of, DayCountContext::default())?;
        let t_maturity =
            dc.year_fraction(base_date, index.premium.end, DayCountContext::default())?;

        let sp_asof = hazard.sp(t_asof);
        let sp_maturity = hazard.sp(t_maturity);
        // Use survival probability floor to prevent division by near-zero
        let survival_cond = if sp_asof > credit::SURVIVAL_PROBABILITY_FLOOR {
            sp_maturity / sp_asof
        } else {
            0.0
        };

        let default_prob = (1.0 - survival_cond).clamp(0.0, 1.0);
        let lgd = 1.0 - index.protection.recovery_rate;

        Ok(notional * default_prob * lgd)
    }
}
