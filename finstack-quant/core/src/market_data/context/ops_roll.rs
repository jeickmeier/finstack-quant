//! Time-roll helpers for [`MarketContext`](super::MarketContext).
//!
//! These methods advance market-data curves forward in time with
//! **realized-forward** semantics: every curve type realizes its forwards as
//! the base date advances (discount curves renormalize by `DF(dt)`, hazard
//! curves preserve hazard rates via conditional survival, forward curves
//! preserve forwards, inflation curves rebase the base CPI, and price /
//! vol-index curves pin the rolled spot to the old forward). This makes a
//! roll-then-reprice theta capture both carry and roll-down. Vol surfaces,
//! FX spot rates, and historical fixings remain static across the roll.

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
    /// - Forwards realized on every curve type: discount curves renormalize
    ///   by `DF(dt)`, hazard curves preserve hazard rates (conditional
    ///   survival), forward curves preserve forwards, inflation curves rebase
    ///   the base CPI, and price / vol-index curves set the rolled spot to
    ///   the old curve's forward at `dt`
    ///
    /// A roll-then-reprice therefore captures carry plus roll-down P&L.
    /// Vol surfaces, FX spot rates, and historical fixings remain static
    /// (see Notes below).
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
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// use finstack_quant_core::market_data::term_structures::DiscountCurve;
    /// use time::macros::date;
    /// # fn main() -> finstack_quant_core::Result<()> {
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
    ///
    /// The returned context owns rolled copies of every curve and rebinds its
    /// credit indices to those copies. `ContextMutationInfo` identifies credit
    /// indices whose derived state was invalidated during that rebinding, so a
    /// caller can recompute or inspect them deliberately. The source context
    /// is never modified.
    ///
    /// Non-curve market data is retained at its original state: FX quotes,
    /// volatility surfaces, scalar prices, historical series, inflation
    /// fixings, dividend schedules, and collateral/hierarchy metadata are
    /// cloned or shared without a time roll. Rebuild those inputs separately
    /// when a scenario requires them to advance consistently with the curves.
    ///
    /// # Errors
    ///
    /// Returns an error if any contained curve cannot roll by `days`, such as
    /// when its day-count calculation fails or insufficient pillars survive.
    /// Because construction occurs in local maps before the result is exposed,
    /// no partially rolled context is returned on error.
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
        // Roll each curve forward, populating plain maps before wrapping them in
        // `Arc` for the new context (the other maps are shared by `Arc` clone).
        let mut rolled_curves = HashMap::default();
        rolled_curves.reserve(self.curves.len());
        for (id, storage) in self.curves.iter() {
            let rolled_storage = storage.roll_forward_storage(days)?;
            rolled_curves.insert(id.clone(), rolled_storage);
        }

        let mut rolled_credit = HashMap::default();
        rolled_credit.reserve(self.credit_indices.len());
        for (id, credit_index) in self.credit_indices.iter() {
            rolled_credit.insert(id.clone(), Arc::clone(credit_index));
        }

        let mut new_ctx = Self {
            curves: Arc::new(rolled_curves),
            fx: self.fx.clone(),
            surfaces: Arc::clone(&self.surfaces),
            prices: Arc::clone(&self.prices),
            series: Arc::clone(&self.series),
            inflation_indices: Arc::clone(&self.inflation_indices),
            credit_indices: Arc::new(rolled_credit),
            dividends: Arc::clone(&self.dividends),
            fx_delta_vol_surfaces: Arc::clone(&self.fx_delta_vol_surfaces),
            vol_cubes: Arc::clone(&self.vol_cubes),
            collateral: Arc::clone(&self.collateral),
            hierarchy: self.hierarchy.clone(),
        };

        let invalidated = new_ctx.rebind_all_credit_indices();

        Ok((
            new_ctx,
            super::ContextMutationInfo {
                invalidated_credit_indices: invalidated,
            },
        ))
    }
}
