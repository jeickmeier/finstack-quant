//! Severity01 calculator for StructuredCredit.
//!
//! Severity01 is the PV change per 1% (0.01 absolute) move in loss severity,
//! in currency.
//!
//! # Formula
//! ```text
//! Severity01 = -Recovery01
//! ```
//! Loss severity is `1 - recovery rate` (LGD), so a +1% severity move is a
//! −1% recovery move. Bumping severity reprices the same two recovery-bumped
//! deals as [`Recovery01`](super::recovery01) with the direction swapped, so
//! the result is the exact negative of Recovery01, including its clamping of
//! recovery to `[0, 1]`. The calculator reads the Recovery01 already computed
//! for the request instead of repricing twice more.

use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext, MetricId};

/// Severity01 calculator for StructuredCredit.
pub(crate) struct Severity01Calculator;

impl MetricCalculator for Severity01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let recovery01 = context
            .computed
            .get(&MetricId::Recovery01)
            .copied()
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "metric:recovery01".to_string(),
                })
            })?;
        Ok(-recovery01)
    }

    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::Recovery01]
    }
}
