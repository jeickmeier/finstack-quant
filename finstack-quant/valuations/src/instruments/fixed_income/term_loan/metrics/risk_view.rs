//! Shared pricing view for term-loan risk metrics.
//!
//! DV01, bucketed DV01, CS01, and bucketed CS01 all reprice through the
//! caller-selected model. Quoted callable loans convert the clean price to
//! OAS once so every bump holds that OAS fixed.

use super::oas::oas_decimal_from_quote_overrides;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::TermLoan;
use crate::metrics::{
    quoted_z_spread, Dv01CalculatorConfig, GenericBucketedCs01, GenericParallelCs01,
    MetricCalculator, MetricContext, UnifiedDv01Calculator, ZSpreadBucketedCs01,
    ZSpreadParallelCs01,
};
use crate::pricer::ModelKey;
use std::sync::Arc;

fn tree_pinned_loan(
    loan: &TermLoan,
    context: &MetricContext,
) -> finstack_quant_core::Result<TermLoan> {
    let oas = oas_decimal_from_quote_overrides(loan, context)?.ok_or_else(|| {
        finstack_quant_core::Error::internal(
            "term-loan option risk found a price quote but could not resolve its OAS",
        )
    })?;
    let mut pinned = loan.clone();
    let quotes = &mut pinned.instrument_pricing_overrides.market_quotes;
    quotes.clear_price_drivers();
    quotes.quoted_oas = Some(oas);
    Ok(pinned)
}

fn discounting_pinned_loan(
    loan: &TermLoan,
    context: &MetricContext,
) -> finstack_quant_core::Result<TermLoan> {
    let spread = if let Some(spread) = loan
        .instrument_pricing_overrides
        .market_quotes
        .quoted_z_spread
    {
        spread
    } else {
        quoted_z_spread(loan, context)?.ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "TermLoan '{}' discounting quote risk requires quoted_clean_price or quoted_z_spread",
                loan.id
            ))
        })?
    };
    let mut pinned = loan.clone();
    let quotes = &mut pinned.instrument_pricing_overrides.market_quotes;
    quotes.clear_price_drivers();
    quotes.quoted_z_spread = Some(spread);
    Ok(pinned)
}

fn pinned_loan(
    loan: &TermLoan,
    context: &MetricContext,
) -> finstack_quant_core::Result<Option<TermLoan>> {
    let quotes = &loan.instrument_pricing_overrides.market_quotes;
    if !quotes.has_price_driver() {
        return Ok(None);
    }
    match context
        .pricing_model()
        .unwrap_or_else(|| loan.default_model())
    {
        ModelKey::Tree => tree_pinned_loan(loan, context).map(Some),
        ModelKey::Discounting => {
            if quotes.quoted_oas.is_some() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "TermLoan '{}' quoted_oas requires the Tree model",
                    loan.id
                )));
            }
            discounting_pinned_loan(loan, context).map(Some)
        }
        model => Err(finstack_quant_core::Error::Validation(format!(
            "TermLoan '{}' has no quote-risk view for model {model:?}",
            loan.id
        ))),
    }
}

pub(super) fn with_term_loan_risk_view<R>(
    context: &mut MetricContext,
    f: impl FnOnce(&mut MetricContext) -> finstack_quant_core::Result<R>,
) -> finstack_quant_core::Result<R> {
    let pinned = {
        let loan: &TermLoan = context.instrument_as()?;
        pinned_loan(loan, context)?
    };
    let Some(risk_loan) = pinned else {
        return f(context);
    };
    let orig_instrument = Arc::clone(&context.instrument);
    context.set_instrument(Arc::new(risk_loan) as Arc<dyn Instrument>);
    let result = f(context);
    context.set_instrument(orig_instrument);
    result
}

fn uses_credit_tree(context: &MetricContext, loan: &TermLoan) -> bool {
    loan.credit_curve_id.is_some()
        && matches!(
            context
                .pricing_model()
                .unwrap_or_else(|| loan.default_model()),
            ModelKey::Tree
        )
}

pub(crate) struct TermLoanParallelDv01Calculator;

impl MetricCalculator for TermLoanParallelDv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        with_term_loan_risk_view(context, |ctx| {
            UnifiedDv01Calculator::<TermLoan>::new(Dv01CalculatorConfig::parallel_combined())
                .calculate(ctx)
        })
    }
}

pub(crate) struct TermLoanBucketedDv01Calculator;

impl MetricCalculator for TermLoanBucketedDv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        with_term_loan_risk_view(context, |ctx| {
            UnifiedDv01Calculator::<TermLoan>::new(Dv01CalculatorConfig::triangular_key_rate())
                .calculate(ctx)
        })
    }
}

/// Parallel CS01: par-spread rebootstrap on the credit tree, z-spread on discounting.
pub(crate) struct TermLoanCs01Calculator;

impl MetricCalculator for TermLoanCs01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        with_term_loan_risk_view(context, |ctx| {
            let use_credit_tree = {
                let loan: &TermLoan = ctx.instrument_as()?;
                uses_credit_tree(ctx, loan)
            };
            if use_credit_tree {
                GenericParallelCs01::<TermLoan>::default().calculate(ctx)
            } else {
                ZSpreadParallelCs01::<TermLoan>::z_spread_only().calculate(ctx)
            }
        })
    }
}

/// Bucketed CS01: par-spread rebootstrap on the credit tree, z-spread on discounting.
pub(crate) struct TermLoanBucketedCs01Calculator;

impl MetricCalculator for TermLoanBucketedCs01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        with_term_loan_risk_view(context, |ctx| {
            let use_credit_tree = {
                let loan: &TermLoan = ctx.instrument_as()?;
                uses_credit_tree(ctx, loan)
            };
            if use_credit_tree {
                GenericBucketedCs01::<TermLoan>::default().calculate(ctx)
            } else {
                ZSpreadBucketedCs01::<TermLoan>::z_spread_only().calculate(ctx)
            }
        })
    }
}
