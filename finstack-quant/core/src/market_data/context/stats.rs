//! Introspection and summary statistics for [`MarketContext`](super::MarketContext).
//!
//! This submodule exposes lightweight queries over stored market data, such as
//! counts, grouped identifiers, and other read-only diagnostics intended for
//! validation, debugging, and tests.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::types::CurveId;

use super::{CurveStorage, MarketContext};

use crate::market_data::{
    dividends::DividendSchedule,
    scalars::InflationIndex,
    scalars::{MarketScalar, ScalarTimeSeries},
    surfaces::{FxDeltaVolSurface, VolCube, VolSurface},
};

impl MarketContext {
    // -----------------------------------------------------------------------------
    // Introspection and statistics
    // -----------------------------------------------------------------------------

    /// Get curve storage by ID (for generic access)
    pub fn curve(&self, id: impl AsRef<str>) -> Option<&CurveStorage> {
        self.curves.get(id.as_ref())
    }

    /// Get all curve IDs
    pub fn curve_ids(&self) -> impl Iterator<Item = &CurveId> {
        self.curves.keys()
    }

    /// Iterate over curves matching a specific type name.
    ///
    /// # Parameters
    /// - `curve_type`: string as returned by [`CurveStorage::curve_type`]
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_quant_core::market_data::context::MarketContext;
    /// # use finstack_quant_core::market_data::term_structures::DiscountCurve;
    /// # use finstack_quant_core::dates::Date;
    /// # use time::Month;
    /// # let curve = DiscountCurve::builder("USD-OIS")
    /// #     .base_date(Date::from_calendar_date(2024, Month::January, 1).expect("Valid date"))
    /// #     .knots([(0.0, 1.0), (1.0, 0.99)])
    /// #     .build()
    /// #     .expect("... builder should succeed");
    /// # let ctx = MarketContext::new().insert(curve);
    /// let mut iter = ctx.curves_of_type("Discount");
    /// assert!(iter.next().is_some());
    /// ```
    pub fn curves_of_type<'a>(
        &'a self,
        curve_type: &'a str,
    ) -> impl Iterator<Item = (&'a CurveId, &'a CurveStorage)> + 'a {
        self.curves
            .iter()
            .filter(move |(_, storage)| storage.curve_type() == curve_type)
    }

    /// Count curves grouped by type string.
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_quant_core::market_data::context::MarketContext;
    /// # use finstack_quant_core::market_data::term_structures::DiscountCurve;
    /// # use finstack_quant_core::dates::Date;
    /// # use time::Month;
    /// # let curve = DiscountCurve::builder("USD-OIS")
    /// #     .base_date(Date::from_calendar_date(2024, Month::January, 1).expect("Valid date"))
    /// #     .knots([(0.0, 1.0), (1.0, 0.99)])
    /// #     .build()
    /// #     .expect("... builder should succeed");
    /// # let ctx = MarketContext::new().insert(curve);
    /// let counts = ctx.count_by_type();
    /// assert_eq!(counts.get("Discount"), Some(&1));
    /// ```
    pub fn count_by_type(&self) -> BTreeMap<&'static str, usize> {
        let mut counts = BTreeMap::new();
        for storage in self.curves.values() {
            *counts.entry(storage.curve_type()).or_insert(0) += 1;
        }
        counts
    }

    /// Compute aggregate statistics about the context contents.
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_quant_core::market_data::context::MarketContext;
    /// # let stats = MarketContext::new().stats();
    /// assert_eq!(stats.total_curves, 0);
    /// ```
    pub fn stats(&self) -> ContextStats {
        ContextStats {
            curve_counts: self.count_by_type(),
            total_curves: self.curves.len(),
            has_fx: self.fx.is_some(),
            surface_count: self.surfaces.len(),
            vol_cube_count: self.vol_cubes.len(),
            price_count: self.prices.len(),
            series_count: self.series.len(),
            inflation_index_count: self.inflation_indices.len(),
            credit_index_count: self.credit_indices.len(),
            dividend_schedule_count: self.dividends.len(),
            fx_delta_vol_surface_count: self.fx_delta_vol_surfaces.len(),
            collateral_mapping_count: self.collateral.len(),
        }
    }

    /// Check whether any data (curve, surface, price, series, index, dividend, or collateral mapping) is registered under the given id.
    pub fn contains(&self, id: impl AsRef<str>) -> bool {
        let id = id.as_ref();
        self.curves.contains_key(id)
            || self.surfaces.contains_key(id)
            || self.vol_cubes.contains_key(id)
            || self.prices.contains_key(id)
            || self.series.contains_key(id)
            || self.inflation_indices.contains_key(id)
            || self.credit_indices.contains_key(id)
            || self.dividends.contains_key(id)
            || self.fx_delta_vol_surfaces.contains_key(id)
            || self.collateral.contains_key(id)
    }

    /// Return `true` when no market data has been inserted.
    pub fn is_empty(&self) -> bool {
        self.curves.is_empty()
            && self.fx.is_none()
            && self.surfaces.is_empty()
            && self.vol_cubes.is_empty()
            && self.prices.is_empty()
            && self.series.is_empty()
            && self.inflation_indices.is_empty()
            && self.credit_indices.is_empty()
            && self.fx_delta_vol_surfaces.is_empty()
            && self.collateral.is_empty()
    }

    /// Get total number of objects
    pub fn total_objects(&self) -> usize {
        self.curves.len()
            + self.surfaces.len()
            + self.vol_cubes.len()
            + self.prices.len()
            + self.series.len()
            + self.inflation_indices.len()
            + self.credit_indices.len()
            + self.fx_delta_vol_surfaces.len()
            + if self.fx.is_some() { 1 } else { 0 }
    }

    // -----------------------------------------------------------------------------
    // Iterators for Market Scalars (P&L Attribution Support)
    // -----------------------------------------------------------------------------

    /// Iterate over all market prices/scalars.
    ///
    /// Returns an iterator over (CurveId, MarketScalar) pairs.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// use finstack_quant_core::market_data::scalars::MarketScalar;
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::currency::Currency;
    ///
    /// let ctx = MarketContext::new()
    ///     .insert_price("AAPL", MarketScalar::Price(Money::new(180.0, Currency::USD)));
    ///
    /// for (id, scalar) in ctx.prices_iter() {
    ///     println!("{}: {:?}", id, scalar);
    /// }
    /// ```
    pub fn prices_iter(&self) -> impl Iterator<Item = (&CurveId, &MarketScalar)> {
        self.prices.iter()
    }

    /// Iterate over all time series.
    ///
    /// Returns an iterator over (CurveId, ScalarTimeSeries) pairs.
    pub fn series_iter(&self) -> impl Iterator<Item = (&CurveId, &ScalarTimeSeries)> {
        self.series.iter()
    }

    /// Iterate over all inflation indices.
    ///
    /// Returns an iterator over `(CurveId, Arc<InflationIndex>)` pairs.
    pub fn inflation_indices_iter(&self) -> impl Iterator<Item = (&CurveId, &Arc<InflationIndex>)> {
        self.inflation_indices.iter()
    }

    /// Iterate over all dividend schedules.
    ///
    /// Returns an iterator over `(CurveId, Arc<DividendSchedule>)` pairs.
    pub fn dividends_iter(&self) -> impl Iterator<Item = (&CurveId, &Arc<DividendSchedule>)> {
        self.dividends.iter()
    }

    /// Replace all volatility surfaces (mutable).
    ///
    /// This is intended for snapshot restore workflows.
    pub fn replace_surfaces_mut<I>(&mut self, surfaces: I) -> &mut Self
    where
        I: IntoIterator<Item = (CurveId, Arc<VolSurface>)>,
    {
        self.surfaces.clear();
        self.surfaces.extend(surfaces);
        self
    }

    /// Retain only the curves for which `pred` returns `true` (mutable).
    ///
    /// Intended for snapshot-restore workflows that need drop-and-replace
    /// semantics for a single curve family (e.g. P&L attribution factor
    /// isolation): drop the flagged family's curves, then re-insert the
    /// snapshot's, leaving every other family untouched.
    pub fn retain_curves_mut(
        &mut self,
        mut pred: impl FnMut(&CurveId, &CurveStorage) -> bool,
    ) -> &mut Self {
        self.curves.retain(|id, curve| pred(id, curve));
        self
    }

    /// Iterate over the SABR volatility cubes.
    pub fn vol_cubes_iter(&self) -> impl Iterator<Item = (&CurveId, &Arc<VolCube>)> {
        self.vol_cubes.iter()
    }

    /// Iterate over the FX delta-quoted volatility surfaces.
    pub fn fx_delta_vol_surfaces_iter(
        &self,
    ) -> impl Iterator<Item = (&CurveId, &Arc<FxDeltaVolSurface>)> {
        self.fx_delta_vol_surfaces.iter()
    }

    /// Replace all SABR volatility cubes (mutable).
    ///
    /// This is intended for snapshot restore workflows.
    pub fn replace_vol_cubes_mut<I>(&mut self, cubes: I) -> &mut Self
    where
        I: IntoIterator<Item = (CurveId, Arc<VolCube>)>,
    {
        self.vol_cubes.clear();
        self.vol_cubes.extend(cubes);
        self
    }

    /// Replace all FX delta-quoted volatility surfaces (mutable).
    ///
    /// This is intended for snapshot restore workflows.
    pub fn replace_fx_delta_vol_surfaces_mut<I>(&mut self, surfaces: I) -> &mut Self
    where
        I: IntoIterator<Item = (CurveId, Arc<FxDeltaVolSurface>)>,
    {
        self.fx_delta_vol_surfaces.clear();
        self.fx_delta_vol_surfaces.extend(surfaces);
        self
    }

    /// Retain only the scalar time series for which `pred` returns `true`
    /// (mutable).
    ///
    /// Intended for snapshot-restore workflows that partition the series
    /// store by convention (e.g. `FIXING:`-prefixed rate fixings belong to
    /// the rates family, everything else to market scalars).
    pub fn retain_series_mut(
        &mut self,
        mut pred: impl FnMut(&CurveId, &ScalarTimeSeries) -> bool,
    ) -> &mut Self {
        self.series.retain(|id, series| pred(id, series));
        self
    }

    /// Clear all market scalars — prices, time series, inflation indices and
    /// dividend schedules (mutable).
    ///
    /// Intended for snapshot restore workflows with drop-and-replace scalar
    /// semantics: a scalar present in the current market but absent from the
    /// snapshot must not survive the restore.
    pub fn clear_market_scalars_mut(&mut self) -> &mut Self {
        self.prices.clear();
        self.series.clear();
        self.inflation_indices.clear();
        self.dividends.clear();
        self
    }
}

// -----------------------------------------------------------------------------
// Context Statistics
// -----------------------------------------------------------------------------

/// Statistics about the contents of a [`MarketContext`].
///
/// Obtain via [`MarketContext::stats`] to feed dashboards or diagnostics.
///
/// # Examples
/// ```rust
/// use finstack_quant_core::market_data::context::MarketContext;
///
/// let stats = MarketContext::new().stats();
/// assert_eq!(stats.total_curves, 0);
/// assert!(!stats.has_fx);
/// ```
#[derive(Debug, Clone)]
pub struct ContextStats {
    /// Count of curves by type
    pub curve_counts: BTreeMap<&'static str, usize>,
    /// Total number of curves
    pub total_curves: usize,
    /// Whether FX matrix is present
    pub has_fx: bool,
    /// Number of volatility surfaces
    pub surface_count: usize,
    /// Number of SABR volatility cubes
    pub vol_cube_count: usize,
    /// Number of market prices/scalars
    pub price_count: usize,
    /// Number of time series
    pub series_count: usize,
    /// Number of inflation indices
    pub inflation_index_count: usize,
    /// Number of credit indices
    pub credit_index_count: usize,
    /// Number of dividend schedules
    pub dividend_schedule_count: usize,
    /// Number of FX delta-quoted volatility surfaces
    pub fx_delta_vol_surface_count: usize,
    /// Number of collateral mappings
    pub collateral_mapping_count: usize,
}

impl core::fmt::Display for ContextStats {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "MarketContext Statistics:")?;
        writeln!(f, "  Total curves: {}", self.total_curves)?;
        for (curve_type, count) in &self.curve_counts {
            writeln!(f, "    {}: {}", curve_type, count)?;
        }
        writeln!(f, "  Surfaces: {}", self.surface_count)?;
        writeln!(f, "  Vol cubes: {}", self.vol_cube_count)?;
        writeln!(f, "  Prices: {}", self.price_count)?;
        writeln!(f, "  Series: {}", self.series_count)?;
        writeln!(f, "  Inflation indices: {}", self.inflation_index_count)?;
        writeln!(f, "  Credit indices: {}", self.credit_index_count)?;
        writeln!(f, "  Dividend schedules: {}", self.dividend_schedule_count)?;
        writeln!(
            f,
            "  FX delta vol surfaces: {}",
            self.fx_delta_vol_surface_count
        )?;
        writeln!(
            f,
            "  Collateral mappings: {}",
            self.collateral_mapping_count
        )?;
        writeln!(f, "  Has FX: {}", self.has_fx)?;
        Ok(())
    }
}
