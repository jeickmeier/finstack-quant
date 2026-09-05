//! Oas implementation used by the metrics subsystem.
//!
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::TermLoan;
use crate::metrics::{MetricCalculator, MetricContext};
use crate::pricer::ModelKey;

pub(super) fn require_tree_model(
    loan: &TermLoan,
    context: &MetricContext,
) -> finstack_quant_core::Result<()> {
    let model = context
        .pricing_model()
        .unwrap_or_else(|| loan.default_model());
    if model == ModelKey::Tree {
        Ok(())
    } else {
        Err(finstack_quant_core::Error::Validation(format!(
            "TermLoan '{}' OAS and embedded option value require the Tree model",
            loan.id
        )))
    }
}

/// Quoted OAS if present, otherwise the OAS inverted from `quoted_clean_price`.
pub(crate) fn oas_decimal_from_quote_overrides(
    loan: &TermLoan,
    context: &MetricContext,
) -> finstack_quant_core::Result<Option<f64>> {
    if let Some(oas) = loan.instrument_pricing_overrides.market_quotes.quoted_oas {
        return Ok(Some(oas));
    }
    let Some(clean_price) = loan
        .instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price
    else {
        return Ok(None);
    };

    let pricer = crate::instruments::fixed_income::term_loan::pricing::TermLoanTreePricer::new();
    let oas_bp = pricer.calculate_oas(loan, context.curves.as_ref(), context.as_of, clean_price)?;
    Ok(Some(oas_bp / 10_000.0))
}

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
/// Uses `quoted_oas` directly when supplied. Otherwise requires `quoted_clean_price`
/// (percent of funded outstanding) so the tree can invert the market dirty price.
pub(crate) struct OasCalculator;

impl MetricCalculator for OasCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let loan: &TermLoan = context.instrument_as()?;
        require_tree_model(loan, context)?;
        oas_decimal_from_quote_overrides(loan, context)?.ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: "term_loan.pricing_overrides.quoted_oas_or_quoted_clean_price".to_string(),
            })
        })
    }
}
