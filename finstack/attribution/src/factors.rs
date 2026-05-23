//! Factor decomposition logic for P&L attribution.
//! Market factor manipulation for P&L attribution analysis.
//!
//! This module provides functions to selectively freeze and restore specific market
//! factors (curves, FX, volatility surfaces, scalars) while manipulating a
//! [`MarketContext`]. This is essential for attribution analysis, where we need to
//! isolate the impact of individual market moves on instrument valuations.
//!
//! # Architecture
//!
//! The module uses a **unified snapshot and restoration framework** based on bitflags
//! to eliminate code duplication. All market factors — curves, FX, volatility surfaces
//! and scalars — flow through a single pair of helpers:
//!
//! 1. **[`CurveRestoreFlags`]** (a.k.a. [`MarketRestoreFlags`]) - Bitflags specifying
//!    which market factor families to snapshot and restore
//! 2. **[`MarketSnapshot`]** - Unified container for curves, FX, surfaces, and scalars
//! 3. **[`MarketSnapshot::extract`]** / **[`MarketSnapshot::restore_market`]** - The
//!    canonical extract/restore entry points for every factor family
//!
//! # Semantics
//!
//! - **Curve families** (discount/forward/hazard/inflation/correlation): flagged curves
//!   are replaced from snapshot; unflagged curves are preserved from `current_market`.
//! - **FX** (`FX` flag): if flagged, the snapshot's FX (possibly `None`) replaces the
//!   market's FX. If the snapshot's FX is `None` with the flag set, FX is cleared.
//!   If not flagged, FX is preserved from `current_market`.
//! - **Vol surfaces** (`VOL` flag): if flagged, the snapshot's surface map replaces the
//!   market's surfaces entirely. If not flagged, surfaces are preserved.
//! - **Scalars** (`SCALARS` flag): **DROP semantic** — if flagged, ALL scalars from
//!   `current_market` are dropped and ONLY the snapshot's scalars are inserted. This
//!   is load-bearing for factor isolation correctness. If not flagged, scalars are
//!   preserved from `current_market`.
//!
//! # See Also
//!
//! - [`crate::parallel`] - Parallel attribution using this module
//! - [`crate::waterfall`] - Waterfall attribution using this module

use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::dividends::DividendSchedule;
use finstack_core::market_data::scalars::InflationIndex;
use finstack_core::market_data::scalars::{MarketScalar, ScalarTimeSeries};
use finstack_core::market_data::surfaces::VolSurface;
use finstack_core::market_data::term_structures::BaseCorrelationCurve;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::market_data::term_structures::ForwardCurve;
use finstack_core::market_data::term_structures::HazardCurve;
use finstack_core::market_data::term_structures::InflationCurve;
use finstack_core::money::fx::FxMatrix;
use finstack_core::types::CurveId;
use finstack_core::HashMap;
use std::sync::Arc;

/// Flags indicating which market factor families to restore from snapshot vs. preserve
/// from market.
///
/// Despite the historical name `CurveRestoreFlags`, this struct now covers all market
/// factor families: curves, FX, volatility surfaces and scalars. The alias
/// [`MarketRestoreFlags`] is provided for call sites that prefer the broader name.
///
/// # Examples
///
/// ```
/// use finstack_attribution::CurveRestoreFlags;
///
/// // Restore only discount curves
/// let flags = CurveRestoreFlags::DISCOUNT;
///
/// // Restore both discount and forward curves (rates)
/// let rates = CurveRestoreFlags::RATES;
/// assert_eq!(rates, CurveRestoreFlags::DISCOUNT | CurveRestoreFlags::FORWARD);
///
/// // Restore FX and volatility surfaces together
/// let fx_vol = CurveRestoreFlags::FX | CurveRestoreFlags::VOL;
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CurveRestoreFlags(u16);

/// Broader-name alias for [`CurveRestoreFlags`]. Prefer this name in new code where
/// the flags mix curve and non-curve (FX/VOL/SCALARS) families.
pub type MarketRestoreFlags = CurveRestoreFlags;

impl CurveRestoreFlags {
    const DISCOUNT_BIT: u16 = 1 << 0;
    const FORWARD_BIT: u16 = 1 << 1;
    const HAZARD_BIT: u16 = 1 << 2;
    const INFLATION_BIT: u16 = 1 << 3;
    const CORRELATION_BIT: u16 = 1 << 4;
    const FX_BIT: u16 = 1 << 5;
    const VOL_BIT: u16 = 1 << 6;
    const SCALARS_BIT: u16 = 1 << 7;
    const ALL_BITS: u16 = Self::DISCOUNT_BIT
        | Self::FORWARD_BIT
        | Self::HAZARD_BIT
        | Self::INFLATION_BIT
        | Self::CORRELATION_BIT
        | Self::FX_BIT
        | Self::VOL_BIT
        | Self::SCALARS_BIT;

    /// Restore discount curves from snapshot
    pub const DISCOUNT: Self = Self(Self::DISCOUNT_BIT);

    /// Restore forward curves from snapshot
    pub const FORWARD: Self = Self(Self::FORWARD_BIT);

    /// Restore hazard curves from snapshot
    pub const HAZARD: Self = Self(Self::HAZARD_BIT);

    /// Restore inflation curves from snapshot
    pub const INFLATION: Self = Self(Self::INFLATION_BIT);

    /// Restore base correlation curves from snapshot
    pub const CORRELATION: Self = Self(Self::CORRELATION_BIT);

    /// Restore FX matrix from snapshot.
    ///
    /// If the snapshot has `fx = None`, this flag intentionally clears FX from
    /// the restored market instead of preserving the current market's FX.
    pub const FX: Self = Self(Self::FX_BIT);

    /// Restore volatility surfaces from snapshot
    pub const VOL: Self = Self(Self::VOL_BIT);

    /// Restore market scalars (prices, series, inflation indices, dividends) from
    /// snapshot. Scalars present in the current market but absent from the snapshot
    /// are **dropped** (see module docs).
    pub const SCALARS: Self = Self(Self::SCALARS_BIT);

    /// Convenience combination: restore both discount and forward curves (rates family)
    pub const RATES: Self = Self(Self::DISCOUNT_BIT | Self::FORWARD_BIT);

    /// Convenience combination: restore hazard curves (credit family)
    pub const CREDIT: Self = Self(Self::HAZARD_BIT);

    /// Returns flags with all market factor families enabled.
    #[inline]
    pub const fn all() -> Self {
        Self(Self::ALL_BITS)
    }

    /// Returns flags with no factor families enabled.
    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Returns true if the specified flags are all set.
    #[inline]
    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl std::ops::BitOr for CurveRestoreFlags {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for CurveRestoreFlags {
    type Output = Self;

    #[inline]
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl std::ops::Not for CurveRestoreFlags {
    type Output = Self;

    #[inline]
    fn not(self) -> Self::Output {
        Self(!self.0 & Self::ALL_BITS)
    }
}

/// Unified market snapshot that can hold any combination of factor families.
///
/// Holds curves, FX, volatility surfaces, and market scalars. Extract only the
/// families whose flags are set via [`MarketSnapshot::extract`]; the remaining
/// fields stay empty/`None`.
#[derive(Clone, Default)]
pub struct MarketSnapshot {
    /// Discount curves indexed by curve ID
    pub discount_curves: HashMap<CurveId, Arc<DiscountCurve>>,
    /// Forward curves indexed by curve ID
    pub forward_curves: HashMap<CurveId, Arc<ForwardCurve>>,
    /// Hazard curves indexed by curve ID
    pub hazard_curves: HashMap<CurveId, Arc<HazardCurve>>,
    /// Inflation curves indexed by curve ID
    pub inflation_curves: HashMap<CurveId, Arc<InflationCurve>>,
    /// Base correlation curves indexed by curve ID
    pub base_correlation_curves: HashMap<CurveId, Arc<BaseCorrelationCurve>>,
    /// FX matrix (populated when the `FX` flag is set during extract).
    ///
    /// `None` is a meaningful value on restore: with `FX` flagged it clears FX
    /// from the target market.
    pub fx: Option<Arc<FxMatrix>>,
    /// Volatility surfaces (populated when the `VOL` flag is set during extract).
    pub surfaces: HashMap<CurveId, Arc<VolSurface>>,
    /// Market scalar prices (populated when the `SCALARS` flag is set)
    pub prices: HashMap<CurveId, MarketScalar>,
    /// Scalar time series (populated when the `SCALARS` flag is set)
    pub series: HashMap<CurveId, ScalarTimeSeries>,
    /// Inflation indices (populated when the `SCALARS` flag is set)
    pub inflation_indices: HashMap<CurveId, Arc<InflationIndex>>,
    /// Dividend schedules (populated when the `SCALARS` flag is set)
    pub dividends: HashMap<CurveId, Arc<DividendSchedule>>,
}

impl MarketSnapshot {
    /// Extract factor families from a market context based on which flags are set.
    ///
    /// Only the families corresponding to set flags are populated into the snapshot;
    /// other fields remain empty (or `None` for FX).
    pub fn extract(market: &MarketContext, flags: CurveRestoreFlags) -> Self {
        let mut snapshot = Self::default();

        for curve_id in market.curve_ids() {
            if flags.contains(CurveRestoreFlags::DISCOUNT) {
                if let Ok(curve) = market.get_discount(curve_id) {
                    snapshot.discount_curves.insert(curve_id.clone(), curve);
                }
            }
            if flags.contains(CurveRestoreFlags::FORWARD) {
                if let Ok(curve) = market.get_forward(curve_id) {
                    snapshot.forward_curves.insert(curve_id.clone(), curve);
                }
            }
            if flags.contains(CurveRestoreFlags::HAZARD) {
                if let Ok(curve) = market.get_hazard(curve_id) {
                    snapshot.hazard_curves.insert(curve_id.clone(), curve);
                }
            }
            if flags.contains(CurveRestoreFlags::INFLATION) {
                if let Ok(curve) = market.get_inflation_curve(curve_id) {
                    snapshot.inflation_curves.insert(curve_id.clone(), curve);
                }
            }
            if flags.contains(CurveRestoreFlags::CORRELATION) {
                if let Ok(curve) = market.get_base_correlation(curve_id) {
                    snapshot
                        .base_correlation_curves
                        .insert(curve_id.clone(), curve);
                }
            }
        }

        if flags.contains(CurveRestoreFlags::FX) {
            snapshot.fx = market.fx().cloned();
        }

        if flags.contains(CurveRestoreFlags::VOL) {
            snapshot.surfaces = market.surfaces_snapshot();
        }

        if flags.contains(CurveRestoreFlags::SCALARS) {
            snapshot.prices = market
                .prices_iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            snapshot.series = market
                .series_iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            snapshot.inflation_indices = market
                .inflation_indices_iter()
                .map(|(k, v)| (k.clone(), Arc::clone(v)))
                .collect();
            snapshot.dividends = market
                .dividends_iter()
                .map(|(k, v)| (k.clone(), Arc::clone(v)))
                .collect();
        }

        snapshot
    }

    /// Restore market by applying snapshot factors and preserving non-snapshot factors.
    ///
    /// For each family:
    /// - **Curves**: flagged families come from `snapshot`; unflagged families are
    ///   preserved from `current_market` (curve-by-curve).
    /// - **FX**: if flagged, replaced by `snapshot.fx` (which may be `None`, clearing
    ///   FX); otherwise preserved from `current_market`.
    /// - **Vol surfaces**: if flagged, replaced wholesale by `snapshot.surfaces`;
    ///   otherwise preserved from `current_market`.
    /// - **Scalars**: if flagged, **all** scalars from `current_market` are dropped
    ///   and only the snapshot's scalars are inserted (this is load-bearing for
    ///   factor isolation). Otherwise scalars are preserved from `current_market`.
    pub fn restore_market(
        current_market: &MarketContext,
        snapshot: &MarketSnapshot,
        restore_flags: CurveRestoreFlags,
    ) -> MarketContext {
        let mut new_market = MarketContext::new();

        // --- Curves: preserve unflagged families, restore flagged ones ---
        let curve_mask = CurveRestoreFlags::DISCOUNT
            | CurveRestoreFlags::FORWARD
            | CurveRestoreFlags::HAZARD
            | CurveRestoreFlags::INFLATION
            | CurveRestoreFlags::CORRELATION;
        let preserve_curve_flags = !restore_flags & curve_mask;
        let preserved = MarketSnapshot::extract(current_market, preserve_curve_flags);

        // Preserved curves first, then snapshot curves (snapshot overrides).
        for curve in preserved.discount_curves.values() {
            new_market = new_market.insert((**curve).clone());
        }
        for curve in preserved.forward_curves.values() {
            new_market = new_market.insert((**curve).clone());
        }
        for curve in preserved.hazard_curves.values() {
            new_market = new_market.insert((**curve).clone());
        }
        for curve in preserved.inflation_curves.values() {
            new_market = new_market.insert((**curve).clone());
        }
        for curve in preserved.base_correlation_curves.values() {
            new_market = new_market.insert((**curve).clone());
        }

        for curve in snapshot.discount_curves.values() {
            new_market = new_market.insert((**curve).clone());
        }
        for curve in snapshot.forward_curves.values() {
            new_market = new_market.insert((**curve).clone());
        }
        for curve in snapshot.hazard_curves.values() {
            new_market = new_market.insert((**curve).clone());
        }
        for curve in snapshot.inflation_curves.values() {
            new_market = new_market.insert((**curve).clone());
        }
        for curve in snapshot.base_correlation_curves.values() {
            new_market = new_market.insert((**curve).clone());
        }

        // --- FX ---
        if restore_flags.contains(CurveRestoreFlags::FX) {
            match &snapshot.fx {
                Some(fx) => {
                    new_market = new_market.insert_fx(Arc::clone(fx));
                }
                None => {
                    new_market = new_market.clear_fx();
                }
            }
        } else if let Some(fx) = current_market.fx() {
            new_market = new_market.insert_fx(Arc::clone(fx));
        }

        // --- Volatility surfaces ---
        if restore_flags.contains(CurveRestoreFlags::VOL) {
            new_market.replace_surfaces_mut(snapshot.surfaces.clone());
        } else {
            new_market.replace_surfaces_mut(current_market.surfaces_snapshot());
        }

        // --- Scalars: DROP-and-replace if flagged, else preserve from current_market.
        //
        // Drop semantic is intentional: a scalar present in `current_market` but
        // absent from `snapshot` must NOT appear in the result. This keeps factor
        // isolation correct for the attribution call paths.
        if restore_flags.contains(CurveRestoreFlags::SCALARS) {
            for (id, price) in &snapshot.prices {
                new_market = new_market.insert_price(id.as_str(), price.clone());
            }
            for series in snapshot.series.values() {
                new_market = new_market.insert_series(series.clone());
            }
            for (id, index) in &snapshot.inflation_indices {
                new_market = new_market.insert_inflation_index(id.as_str(), Arc::clone(index));
            }
            for schedule in snapshot.dividends.values() {
                new_market = new_market.insert_dividends(Arc::clone(schedule));
            }
        } else {
            new_market = copy_scalars(current_market, new_market);
        }

        new_market
    }
}

fn copy_scalars(from: &MarketContext, mut to: MarketContext) -> MarketContext {
    for (id, price) in from.prices_iter() {
        to = to.insert_price(id.as_str(), price.clone());
    }
    for (_id, series) in from.series_iter() {
        to = to.insert_series(series.clone());
    }
    for (id, index) in from.inflation_indices_iter() {
        to = to.insert_inflation_index(id.as_str(), Arc::clone(index));
    }
    for (_id, schedule) in from.dividends_iter() {
        to = to.insert_dividends(Arc::clone(schedule));
    }
    to
}
