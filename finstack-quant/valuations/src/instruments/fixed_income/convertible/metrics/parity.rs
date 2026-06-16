//! Parity metric for `ConvertibleBond`.
//!
//! Computes the parity ratio: equity conversion value divided by bond face value.
//! Leverages pricing helpers from `pricing`.

use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

use crate::instruments::fixed_income::convertible::types::ConvertibleBond;

/// Calculator for convertible bond parity.
pub(crate) struct ParityCalculator;

impl MetricCalculator for ParityCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bond = context.instrument_as::<ConvertibleBond>()?;
        bond.parity(&context.curves)
    }
}
