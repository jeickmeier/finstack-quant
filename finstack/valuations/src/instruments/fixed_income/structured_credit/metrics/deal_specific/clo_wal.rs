//! Weighted Average Life calculator for CLO with prepayments

use crate::metrics::{MetricCalculator, MetricContext};

/// CLO WAL calculator.
///
/// Delegates to the standard structured-credit `WalCalculator`, which
/// computes a true principal-weighted average life
/// `Σ(Principal_i × t_i) / Σ(Principal_i)` from projected tranche cashflows
/// (capturing prepayments, amortization, and defaults).
///
/// # Errors
///
/// Returns an error when no projected cashflows are available in the metric
/// context — pool WAM is *not* substituted, since remaining term and WAL are
/// different quantities.
pub struct CloWalCalculator;

impl MetricCalculator for CloWalCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_core::Result<f64> {
        // Validate the instrument type, then compute the true WAL.
        context
            .instrument
            .as_any()
            .downcast_ref::<crate::instruments::fixed_income::structured_credit::StructuredCredit>()
            .ok_or(finstack_core::InputError::Invalid)?;

        crate::instruments::fixed_income::structured_credit::metrics::pricing::wal::WalCalculator
            .calculate(context)
    }
}
