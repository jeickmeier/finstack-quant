//! Forward/horizon conditioning helpers for [`MarketContext`](super::MarketContext).
//!
//! These methods re-anchor term structures to an explicit `reference_date`
//! while preserving source timelines for curves conditioned forward or backward
//! (`DiscountCurve`, `ForwardCurve`, `HazardCurve`, `ParametricCurve`) and
//! keeping frozen curves (`InflationCurve`, `PriceCurve`,
//! `BaseCorrelationCurve`) on their original anchors. Vol surfaces, FX spot
//! rates, scalars, and historical fixings remain unchanged.

use crate::FxHashMap;
use std::sync::Arc;

use super::MarketContext;

impl MarketContext {
    /// Condition all term structures in this context to `reference_date`.
    ///
    /// Conditioning builds a private candidate `MarketContext` and only returns
    /// it once all curve conditioning and credit-index rebinding succeed:
    /// - `DiscountCurve`, `ForwardCurve`, `HazardCurve`, and `ParametricCurve`
    ///   re-anchor to `reference_date` while retaining their source coordinates
    ///   and continuous shocks.
    /// - `InflationCurve`, `PriceCurve`, and
    ///   `BaseCorrelationCurve` remain frozen on their original clocks and
    ///   levels after validating day-count reachability where applicable.
    /// - Vol surfaces, FX matrices, scalars, series, inflation indices,
    ///   dividends, collateral, and hierarchy metadata remain unchanged.
    ///
    /// # Arguments
    ///
    /// * `reference_date` - Target valuation anchor date for conditioned curves.
    ///
    /// # Returns
    ///
    /// A new `MarketContext` with all curves conditioned to `reference_date`
    /// and dependent credit indices rebound.
    ///
    /// # Errors
    ///
    /// Returns an error if any curve rejects `reference_date`, day-count
    /// calculation fails, normalization fails (for example under
    /// `ExtrapolationPolicy::None`), or any dependent credit index fails to
    /// rebind against the conditioned curves.
    ///
    /// # Examples
    ///
    /// ```
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// use finstack_quant_core::market_data::term_structures::DiscountCurve;
    /// use time::macros::date;
    /// # fn main() -> finstack_quant_core::Result<()> {
    ///
    /// let base_date = date!(2025 - 01 - 01);
    /// let curve = DiscountCurve::builder("USD_OIS")
    ///     .base_date(base_date)
    ///     .knots(vec![(1.0, 0.98), (2.0, 0.96), (5.0, 0.90)])
    ///     .day_count(finstack_quant_core::dates::DayCount::Act365F)
    ///     .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
    ///     .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
    ///     .build()?;
    ///
    /// let ctx = MarketContext::new().try_insert(curve).expect("insert market curve");
    ///
    /// // Condition 6 months forward.
    /// let conditioned_ctx = ctx.condition(date!(2025 - 07 - 02))?;
    /// # let _ = conditioned_ctx;
    /// # Ok(())
    /// # }
    /// ```
    pub fn condition(&self, reference_date: crate::dates::Date) -> crate::Result<Self> {
        tracing::debug!(
            %reference_date,
            curve_count = self.curves.len(),
            credit_index_count = self.credit_indices.len(),
            "conditioning MarketContext"
        );
        let mut conditioned_curves = FxHashMap::default();
        conditioned_curves.reserve(self.curves.len());
        for (id, storage) in self.curves.iter() {
            let conditioned_storage = storage.condition_storage(reference_date)?;
            conditioned_curves.insert(id.clone(), conditioned_storage);
        }

        let mut new_ctx = self.clone();
        new_ctx.curves = Arc::new(conditioned_curves);

        new_ctx.rebind_all_credit_indices()?;

        Ok(new_ctx)
    }
}
