//! Pricing-engine components for fixed-income bonds.
//!
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
#[cfg(test)]
use finstack_quant_core::math::summation::kahan_sum;
#[cfg(test)]
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

use super::super::super::types::Bond;

/// Bond pricing engine providing core valuation methods.
///
/// Uses `Bond::pricing_dated_cashflows` (internal helper) for discount flows:
/// coupons, amortization, and positive notional (redemption). Negative
/// notionals and PIK are excluded as they are not discounted receipt flows.
///
/// # Pricing Formula
///
/// The present value is computed by discounting all future holder-view cashflows:
/// ```text
/// PV = Σ CF_i · DF(as_of → t_i)
/// ```
/// where:
/// - `CF_i` are holder-view cashflows (coupons, amortization, redemption)
/// - `DF(as_of → t_i)` is the discount factor from valuation date to cashflow date
///
/// # Settlement Convention
///
/// Settlement days (`bond.settlement_days`) affect how market **quotes** are
/// interpreted (e.g., accrued interest at settlement date), but the instrument
/// PV is always anchored at `as_of`. The quote engine handles settlement-date
/// accrued interest separately when computing quote-derived metrics (YTM, Z-spread, etc.).
///
/// # Examples
///
/// Bond pricing is performed via the [`Instrument`] trait or the pricer registry:
///
/// ```
/// use finstack_quant_valuations::instruments::Bond;
/// use finstack_quant_valuations::instruments::Instrument;
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_core::market_data::term_structures::DiscountCurve;
/// use time::macros::date;
///
/// # fn main() -> finstack_quant_core::Result<()> {
/// let bond = Bond::example()?;
/// let as_of = date!(2024-01-15);
/// let market = MarketContext::new().insert(
///     DiscountCurve::builder("USD-TREASURY")
///         .base_date(as_of)
///         .knots([(0.0, 1.0), (30.0, 0.40)])
///         .build()?,
/// );
///
/// // Use Instrument trait for public API
/// let pv = bond.value(&market, as_of)?;
/// assert!(pv.amount() > 0.0);
/// # Ok(())
/// # }
/// ```
///
/// [`Instrument`]: crate::instruments::common_impl::traits::Instrument
pub struct BondEngine;

impl BondEngine {
    /// Price a bond using discount curve present value calculation.
    ///
    /// Computes the present value by discounting all future holder-view cashflows
    /// from the valuation date (`as_of`) using the bond's discount curve.
    ///
    /// # Arguments
    ///
    /// * `bond` - The bond to price
    /// * `context` - Market context containing the discount curve
    /// * `as_of` - Valuation date
    ///
    /// # Returns
    ///
    /// Present value of the bond in the bond's currency, discounted from `as_of`.
    ///
    /// # Errors
    ///
    /// Returns `Err` when:
    /// - The bond has embedded call, put, or return-floor rights
    /// - Discount curve is not found in market context
    /// - Bond has no future cashflows
    /// - Cashflow schedule building fails
    #[cfg(test)]
    pub(crate) fn price(bond: &Bond, market: &MarketContext, as_of: Date) -> Result<Money> {
        if bond.has_exercise_rights() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Bond '{}' has embedded call, put, or return-floor rights; discounting is a non-callable model. Use 'tree' for rates-only optional pricing or 'rates_credit' for joint rates-credit optional pricing.",
                bond.id
            )));
        }
        let flows = bond.pricing_dated_cashflows(market, as_of)?;
        let disc = market.get_discount(bond.discount_curve_id.as_str())?;
        let Some((_, first)) = flows.first() else {
            return Ok(Money::from((0_i64, bond.notional.currency())));
        };
        let ccy = first.currency();

        // PV is anchored at as_of (valuation date), not settlement. Same-day
        // cashflows are included with DF(as_of, as_of) = 1.0.
        let mut pv_values: Vec<f64> = Vec::with_capacity(flows.len());
        for (d, amt) in &flows {
            if *d < as_of {
                continue;
            }
            pv_values.push((*amt * disc.df_between_dates(as_of, *d)?).amount());
        }
        Money::new(kahan_sum(pv_values), ccy)
    }

    /// Price a non-callable bond with a constant OAS applied to discounting.
    ///
    /// The input uses the bond's configured OAS quote compounding and is
    /// converted to a continuously compounded spread before it is applied to
    /// each contractual cashflow. Embedded rights are validated by the
    /// model-dispatch boundary before this scalar kernel is called.
    pub(crate) fn price_with_oas(
        bond: &Bond,
        context: &MarketContext,
        as_of: Date,
        oas_quote_decimal: f64,
    ) -> Result<f64> {
        if !oas_quote_decimal.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "discounting OAS must be finite, got {oas_quote_decimal}"
            )));
        }
        let continuous_oas =
            crate::instruments::pricing_overrides::oas_continuous_from_quote_decimal(
                bond.instrument_pricing_overrides
                    .model_config
                    .oas_quote_compounding,
                oas_quote_decimal,
            )?;
        let flows = bond.pricing_dated_cashflows(context, as_of)?;
        let discount = context.get_discount(bond.discount_curve_id.as_str())?;
        let mut pv = finstack_quant_core::math::summation::NeumaierAccumulator::default();
        for (date, amount) in flows {
            let time = discount.day_count().year_fraction(
                as_of,
                date,
                finstack_quant_core::dates::DayCountContext::default(),
            )?;
            pv.add(
                amount.amount()
                    * discount.df_between_dates(as_of, date)?
                    * (-continuous_oas * time).exp(),
            );
        }
        Ok(pv.total())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::bond::{CallPut, CallPutSchedule};
    use time::macros::date;

    #[test]
    fn public_discount_engine_rejects_embedded_rights() {
        let mut bond = Bond::example().expect("example bond");
        bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start: date!(2027 - 01 - 01),
                end: date!(2027 - 01 - 01),
                price_pct_of_par: 100.0,
                make_whole: None,
            }],
            puts: Vec::new(),
        });

        let error = BondEngine::price(&bond, &MarketContext::new(), date!(2025 - 01 - 01))
            .expect_err("discounting must not silently ignore embedded rights");
        assert!(error.to_string().contains("non-callable"), "{error}");
    }
}
