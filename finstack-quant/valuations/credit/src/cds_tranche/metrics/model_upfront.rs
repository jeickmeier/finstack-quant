//! CDS Tranche `model_upfront` metric calculator.
//!
//! Computes the model upfront (net present value at the contractual running
//! coupon) using the Gaussian Copula pricing engine if the required credit
//! index data are available. Distinct from the contractual `upfront` field.

use crate::cds_tranche::CdsTranche;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Model-upfront calculator for CDS Tranche
pub(crate) struct ModelUpfrontCalculator;

impl MetricCalculator for ModelUpfrontCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let tranche: &CdsTranche = context.instrument_as()?;
        if context
            .curves
            .as_ref()
            .get_credit_index(&tranche.credit_index_id)
            .is_ok()
        {
            tranche.model_upfront(&context.curves, context.as_of)
        } else {
            Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::NotFound {
                    id: format!("credit_index:{}", tranche.credit_index_id),
                },
            ))
        }
    }
}
