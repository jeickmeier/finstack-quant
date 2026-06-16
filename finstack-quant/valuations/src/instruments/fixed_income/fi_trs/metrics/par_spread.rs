//! Par spread calculator for fixed income index TRS.

use crate::instruments::common_impl::pricing::swap_legs::ANNUITY_EPSILON;
use crate::instruments::common_impl::pricing::TrsEngine;
use crate::instruments::fixed_income::fi_trs::FIIndexTotalReturnSwap;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::{Error, Result};

/// Calculates the par spread for a fixed income index TRS under the carry-only return model.
///
/// The par spread solves `NPV_receiver = 0`:
///
/// ```text
/// NPV = PV(total_return) - PV(financing_float) - s * Annuity = 0
/// s_par = (PV(total_return) - PV(financing_float)) / Annuity
/// ```
///
/// where `PV(financing_float)` is the financing leg PV excluding the spread
/// component. This is a market-level quote (analogous to a swap rate) computed
/// from the total-return-receiver's perspective regardless of trade side.
pub(crate) struct ParSpreadCalculator;

impl MetricCalculator for ParSpreadCalculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::FinancingAnnuity]
    }

    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        tracing::warn!(
            "FIIndexTotalReturnSwap par spread is computed from a carry-only analytic model, \
             not a full fixed-income index mark-to-market model"
        );
        let trs: &FIIndexTotalReturnSwap = context.instrument_as()?;
        let curves = context.curves.as_ref();
        let as_of = context.as_of;

        let annuity = trs.financing_annuity(curves, as_of)?;
        if annuity.abs() < ANNUITY_EPSILON {
            return Err(Error::Validation(format!(
                "FI TRS par spread: financing annuity {annuity:.3e} below \
                 ANNUITY_EPSILON ({ANNUITY_EPSILON:.0e}). Division would amplify rounding \
                 noise into the par spread."
            )));
        }

        let tr_pv = trs.pv_total_return_leg(curves, as_of)?;

        let float_pv = TrsEngine::pv_financing_float_only(
            &trs.financing,
            &trs.schedule,
            trs.notional,
            curves,
            as_of,
        )?;

        // s_par = (PV(TR) - PV(float)) / Annuity, converted to basis points
        let par_spread = (tr_pv.amount() - float_pv) / annuity * 10000.0;

        if par_spread.is_nan() || par_spread.is_infinite() {
            return Err(Error::Validation(format!(
                "Par spread calculation produced invalid value: {}. \
                 Check that financing annuity is non-zero.",
                par_spread
            )));
        }

        Ok(par_spread)
    }
}
