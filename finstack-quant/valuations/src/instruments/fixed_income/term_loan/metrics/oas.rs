//! Oas implementation used by the metrics subsystem.
//!
use crate::instruments::TermLoan;
use crate::metrics::{MetricCalculator, MetricContext};

/// Calculates Option-Adjusted Spread (OAS) for callable term loans.
///
/// Uses tree-based callable pricing and solves for the constant spread (returned in **decimal**
/// units, e.g. `0.01 = 100bp`) that makes the model price equal to the market price.
///
/// # OAS Convention
///
/// OAS is a **parallel shift to the calibrated risk-free short rate lattice** (in basis points).
/// When the rates+credit two-factor tree is used (i.e. a hazard curve is present in
/// the market context), the hazard tree captures the credit spread independently, so
/// the OAS represents the spread **over the risk-free curve** — consistent with the
/// Bloomberg OAS convention for risky instruments.
///
/// # Dependencies
///
/// Requires `quoted_clean_price` to be set in `loan.pricing_overrides` (as percent of par).
pub(crate) struct OasCalculator;

impl MetricCalculator for OasCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let loan: &TermLoan = context.instrument_as()?;

        let clean_price = loan
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "term_loan.pricing_overrides.quoted_clean_price".to_string(),
                })
            })?;

        let market_context = context.curves.as_ref().clone();
        let pricer =
            crate::instruments::fixed_income::term_loan::pricing::TermLoanTreePricer::new();
        let oas_bp = pricer.calculate_oas(loan, &market_context, context.as_of, clean_price)?;
        Ok(oas_bp / 10_000.0)
    }
}
