//! Japanese simple yield (単利) for JGBs.
//!
use crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext;
use crate::instruments::Bond;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::money::Money;

/// Calculates Japanese simple yield (単利) from dirty price and remaining life.
///
/// Closed form using ACT/365F remaining life and entitled remaining cashflows:
///
/// ```text
/// n = Act365F.year_fraction(quote_date, maturity)
/// y = (Σ remaining CF − dirty) / (n × dirty)
/// ```
///
/// For a regular bullet this is the market formula
/// `y = (C + (1 − P/100) / n) / (P/100)` with dirty price `P` as a percent of
/// par. Street `ytm` is unchanged.
///
/// # Dependencies
///
/// None (accrued is computed internally at quote_date).
pub(crate) struct JapaneseSimpleYieldCalculator;

impl MetricCalculator for JapaneseSimpleYieldCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: &Bond = context.instrument_as()?;
        let maybe_clean_px = bond
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price;
        let notional = bond.notional;
        let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;

        let n = DayCount::Act365F.year_fraction(
            quote_ctx.quote_date,
            bond.maturity,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if n <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "Japanese simple yield requires positive ACT/365F remaining life".to_string(),
            ));
        }

        let dirty: Money = if let Some(clean_px) = maybe_clean_px {
            Money::new(
                quote_ctx.dirty_from_clean_pct(clean_px, notional.amount()),
                notional.currency(),
            )?
        } else {
            let pv_at_quote =
                crate::instruments::fixed_income::bond::pricing::settlement::model_dirty_at_quote_date(
                    bond,
                    &context.curves,
                    context.as_of,
                    quote_ctx.quote_date,
                    context.base_value.amount(),
                )?;
            Money::new(pv_at_quote, notional.currency())?
        };
        if dirty.amount() <= 0.0 {
            return Err(finstack_quant_core::Error::from(
                finstack_quant_core::InputError::Invalid,
            ));
        }

        if context.cashflows.is_none() {
            let bond: &Bond = context.instrument_as()?;
            let flows = quote_ctx.entitled_flows(bond, &context.curves, context.as_of)?;
            context.cashflows = Some(flows);
        }
        let flows = context.cashflows.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: "cashflows".to_string(),
            })
        })?;

        let remaining: f64 = flows
            .iter()
            .filter(|(date, _)| *date > quote_ctx.quote_date)
            .map(|(_, amount)| amount.amount())
            .sum();
        Ok((remaining - dirty.amount()) / (n * dirty.amount()))
    }
}
