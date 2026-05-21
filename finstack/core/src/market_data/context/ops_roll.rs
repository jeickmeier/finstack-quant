//! Time-roll helpers for [`MarketContext`](super::MarketContext).
//!
//! These methods advance market-data curves forward in time without applying
//! carry or theta adjustments, which is useful for roll-down and constant-curve
//! scenario analysis.

use crate::collections::HashMap;
use std::sync::Arc;

use super::MarketContext;

impl MarketContext {
    // -----------------------------------------------------------------------------
    // Curve Rolling (Time Roll-Forward Support)
    // -----------------------------------------------------------------------------

    /// Roll all curves forward by a specified number of days.
    ///
    /// This creates a new `MarketContext` with all curves rolled forward:
    /// - Base dates advanced by `days`
    /// - Knot times shifted backwards (expired points filtered out)
    /// - Curve values preserved (no carry/theta adjustment)
    ///
    /// This is the "constant curves" scenario used for roll-down P&L calculations.
    ///
    /// # Arguments
    /// * `days` - Number of days to roll forward
    ///
    /// # Returns
    /// A new `MarketContext` with all curves rolled forward.
    ///
    /// # Errors
    /// Returns an error if any curve cannot be rolled (e.g., too few points remain).
    ///
    /// # Notes
    /// - Surfaces and other market data are cloned without modification
    /// - FX matrices are preserved as-is (assumed static spot rates)
    /// - Curves with insufficient remaining points will cause an error
    ///
    /// # Examples
    /// ```ignore
    /// use finstack_core::market_data::context::MarketContext;
    /// use finstack_core::market_data::term_structures::DiscountCurve;
    /// use time::macros::date;
    /// # fn main() -> finstack_core::Result<()> {
    ///
    /// let base_date = date!(2025 - 01 - 01);
    /// let curve = DiscountCurve::builder("USD_OIS")
    ///     .base_date(base_date)
    ///     .knots(vec![(1.0, 0.98), (2.0, 0.96), (5.0, 0.90)])
    ///     .build()
    ///     ?;
    ///
    /// let ctx = MarketContext::new().insert(curve);
    ///
    /// // Roll 6 months forward
    /// let rolled_ctx = ctx.roll_forward(182)?;
    /// # let _ = rolled_ctx;
    /// # Ok(())
    /// # }
    /// ```
    pub fn roll_forward(&self, days: i64) -> crate::Result<Self> {
        let (ctx, _info) = self.roll_forward_observed(days)?;
        Ok(ctx)
    }

    /// Like [`roll_forward`](Self::roll_forward), but also returns
    /// [`ContextMutationInfo`](super::ContextMutationInfo) describing any
    /// credit indices invalidated by the roll.
    pub fn roll_forward_observed(
        &self,
        days: i64,
    ) -> crate::Result<(Self, super::ContextMutationInfo)> {
        tracing::debug!(
            days,
            curve_count = self.curves.len(),
            credit_index_count = self.credit_indices.len(),
            "rolling MarketContext forward"
        );
        // NOTE: Non-curve fields are shallow-cloned and retain pre-roll state:
        //  - Vol surfaces: no base-date axis; expiry tenors remain relative
        //  - FX matrices: spot rates; no temporal dimension to roll
        //  - Prices: contain a time axis but are cloned for performance
        //  - Series: historical fixings; unchanged by a forward roll
        //
        // Callers that need fully consistent rolled prices or surfaces
        // should rebuild them from rolled curves after this call.
        let mut new_ctx = Self {
            curves: {
                let mut m = HashMap::default();
                m.reserve(self.curves.len());
                m
            },
            fx: self.fx.clone(),
            surfaces: self.surfaces.clone(),
            prices: self.prices.clone(),
            series: self.series.clone(),
            inflation_indices: self.inflation_indices.clone(),
            credit_indices: HashMap::default(),
            dividends: self.dividends.clone(),
            fx_delta_vol_surfaces: self.fx_delta_vol_surfaces.clone(),
            vol_cubes: self.vol_cubes.clone(),
            collateral: self.collateral.clone(),
            hierarchy: self.hierarchy.clone(),
        };

        // Roll each curve forward
        for (id, storage) in &self.curves {
            let rolled_storage = storage.roll_forward_storage(days)?;
            new_ctx.curves.insert(id.clone(), rolled_storage);
        }

        for (id, credit_index) in &self.credit_indices {
            new_ctx
                .credit_indices
                .insert(id.clone(), Arc::clone(credit_index));
        }
        let invalidated = new_ctx.rebind_all_credit_indices();

        Ok((
            new_ctx,
            super::ContextMutationInfo {
                invalidated_credit_indices: invalidated,
            },
        ))
    }
}
