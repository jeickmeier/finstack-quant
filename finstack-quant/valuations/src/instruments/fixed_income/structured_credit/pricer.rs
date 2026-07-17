//! Unified pricer for all structured credit instruments.
//!
//! The pricing logic is identical across ABS, CLO, CMBS, and RMBS since they all
//! use the shared waterfall implementation via the `StructuredCreditInstrument` trait.
//!
//! # Hedge Swap Integration
//!
//! When hedge swaps are attached to a deal, they are valued alongside the
//! collateral cashflows to provide a hedged NPV. This is important for:
//! - Basis risk management (e.g., SOFR vs Prime mismatches)
//! - Interest rate risk hedging via fixed-for-floating swaps
//! - Cap/floor protection embedded in structures

use super::StructuredCredit;
use crate::instruments::{Instrument, PricingOptions};
use crate::metrics::MetricId;
use crate::results::ValuationResult;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

impl StructuredCredit {
    /// Value the total hedge swap portfolio.
    ///
    /// Returns the net present value of all attached hedge swaps.
    /// This represents the mark-to-market value of the hedging portfolio.
    pub fn hedge_npv(
        &self,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        self.validate_for_pricing()?;
        let effective_as_of = self.resolve_pricing_as_of(context, as_of);
        self.hedge_npv_unchecked(context, effective_as_of)
    }

    fn hedge_npv_unchecked(
        &self,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        let base_ccy = self.pool.base_currency();
        let mut total_hedge_npv = Money::new(0.0, base_ccy);

        for swap in &self.hedge_swaps {
            let swap_npv = swap.value(context, as_of)?;

            // Convert to deal currency if needed (simplified - assumes same currency)
            total_hedge_npv = total_hedge_npv.checked_add(swap_npv)?;
        }

        Ok(total_hedge_npv)
    }

    /// Value the deal plus all hedge swaps (hedged NPV).
    ///
    /// This is the primary valuation method for hedged portfolios, computing:
    /// ```text
    /// Hedged NPV = Deal NPV + Hedge NPV
    /// ```
    ///
    /// # Returns
    /// - `Ok((deal_npv, hedge_npv, total_npv))` on success
    /// - Error if valuation fails
    ///
    /// # Example
    /// ```ignore
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// use finstack_quant_valuations::instruments::fixed_income::structured_credit::StructuredCredit;
    /// use time::macros::date;
    ///
    /// # fn main() -> finstack_quant_core::Result<()> {
    /// let clo = StructuredCredit::example();
    /// let context = MarketContext::new();
    /// let as_of = date!(2025-01-01);
    ///
    /// let (deal, hedges, total) = clo.price_with_hedges(&context, as_of)?;
    /// println!(
    ///     "Deal NPV: {:.2}, Hedge NPV: {:.2}, Total: {:.2}",
    ///     deal.amount(),
    ///     hedges.amount(),
    ///     total.amount()
    /// );
    /// # Ok(())
    /// # }
    /// ```
    pub fn price_with_hedges(
        &self,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<(Money, Money, Money)> {
        let deal_npv = self.value(context, as_of)?;
        let effective_as_of = self.resolve_pricing_as_of(context, as_of);
        let hedge_npv = self.hedge_npv_unchecked(context, effective_as_of)?;
        let total_npv = deal_npv.checked_add(hedge_npv)?;

        Ok((deal_npv, hedge_npv, total_npv))
    }

    /// Check if this deal has any hedge swaps attached.
    pub fn has_hedges(&self) -> bool {
        !self.hedge_swaps.is_empty()
    }

    /// Get the number of hedge swaps attached to this deal.
    pub fn hedge_count(&self) -> usize {
        self.hedge_swaps.len()
    }

    /// Price with additional risk metrics.
    ///
    /// Computes the base NPV plus any requested metrics such as duration, spread, etc.
    /// If hedge swaps are attached, also includes hedge NPV in the results.
    pub fn price_with_metrics_standalone(
        &self,
        context: &MarketContext,
        as_of: Date,
        metrics: &[MetricId],
    ) -> finstack_quant_core::Result<ValuationResult> {
        let mut result = Instrument::price_with_metrics(
            self,
            context,
            as_of,
            metrics,
            PricingOptions::default(),
        )?;

        // Add hedge metrics if swaps are attached
        if !self.hedge_swaps.is_empty() {
            let hedge_npv = self.hedge_npv_unchecked(context, result.as_of)?;
            let total_npv = result.value.checked_add(hedge_npv)?;

            result.measures.insert(
                crate::metrics::MetricId::custom("hedge_npv"),
                hedge_npv.amount(),
            );
            result.measures.insert(
                crate::metrics::MetricId::custom("hedged_npv"),
                total_npv.amount(),
            );
            result.measures.insert(
                crate::metrics::MetricId::custom("hedge_count"),
                self.hedge_swaps.len() as f64,
            );
        }

        Ok(result)
    }

    /// Add a hedge swap to this instrument.
    ///
    /// The swap will be valued alongside the deal for hedged NPV calculations.
    pub fn add_hedge_swap(&mut self, swap: crate::instruments::rates::irs::InterestRateSwap) {
        self.hedge_swaps.push(swap);
    }

    /// Add multiple hedge swaps to this instrument.
    pub fn add_hedge_swaps(
        &mut self,
        swaps: Vec<crate::instruments::rates::irs::InterestRateSwap>,
    ) {
        self.hedge_swaps.extend(swaps);
    }

    /// Builder method to add hedge swap (chainable).
    pub fn with_hedge_swap(
        mut self,
        swap: crate::instruments::rates::irs::InterestRateSwap,
    ) -> Self {
        self.hedge_swaps.push(swap);
        self
    }

    /// Builder method to add multiple hedge swaps (chainable).
    pub fn with_hedge_swaps(
        mut self,
        swaps: Vec<crate::instruments::rates::irs::InterestRateSwap>,
    ) -> Self {
        self.hedge_swaps.extend(swaps);
        self
    }
}

// Generic pricer implementation is used directly via common_impl::GenericInstrumentPricer
