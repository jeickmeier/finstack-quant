//! Japanese simple yield (単利) for JGBs.
//!
use crate::instruments::fixed_income::bond::pricing::quote_conversions::japanese_simple_yield;
use crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext;
use crate::instruments::Bond;
use crate::metrics::{MetricCalculator, MetricContext};

/// Calculates the Japanese simple yield (単利), JSDA convention.
///
/// ```text
/// N = Act365F.year_fraction(quote_date, maturity)
/// y = [C + (100 − P) / N] / P
/// ```
///
/// with `C` the annual coupon and `P` the CLEAN price, both in % of par. The
/// clean price is the quoted clean price when set, otherwise the model dirty
/// value at the quote date less accrued. The inverse used by the quote engine
/// is the same closed form, so quotes round-trip at any settlement date.
/// Street `ytm` is unchanged.
///
/// # Dependencies
///
/// None (accrued is computed internally at quote_date).
pub(crate) struct JapaneseSimpleYieldCalculator;

impl MetricCalculator for JapaneseSimpleYieldCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: &Bond = context.instrument_as()?;
        let notional = bond.notional.amount();
        let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;

        let clean_pct = if let Some(clean_px) = bond
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price
        {
            clean_px
        } else {
            let dirty_at_quote =
                crate::instruments::fixed_income::bond::pricing::settlement::model_dirty_at_quote_date(
                    bond,
                    &context.curves,
                    context.as_of,
                    quote_ctx.quote_date,
                    context.base_value.amount(),
                )?;
            (dirty_at_quote - quote_ctx.accrued_at_quote_date) / notional * 100.0
        };
        japanese_simple_yield(bond, quote_ctx.quote_date, clean_pct)
    }
}
