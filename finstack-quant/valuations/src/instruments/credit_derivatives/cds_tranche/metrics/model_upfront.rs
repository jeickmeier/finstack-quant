//! CDS Tranche `model_upfront` metric calculator.
//!
//! Computes the model upfront (net present value at the contractual running
//! coupon) using the Gaussian Copula pricing engine if the required credit
//! index data are available. Distinct from the contractual `upfront` field.

use crate::instruments::credit_derivatives::cds_tranche::CDSTranche;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Model-upfront calculator for CDS Tranche
pub(crate) struct ModelUpfrontCalculator;

impl MetricCalculator for ModelUpfrontCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let tranche: &CDSTranche = context.instrument_as()?;
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
