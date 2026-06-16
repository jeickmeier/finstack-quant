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
//! 1. **[`MarketRestoreFlags`]** - Bitflags specifying which market factor
//!    families to snapshot and restore
//! 2. **[`MarketSnapshot`]** - Unified container for curves, FX, surfaces, and scalars
//! 3. **[`MarketSnapshot::extract`]** / **[`MarketSnapshot::restore_market`]** - The
//!    canonical extract/restore entry points for every factor family
//!
//! # Semantics
//!
//! `restore_market` is **clone-and-overwrite**: the result starts as a full
//! clone of `current_market`, then each FLAGGED family is dropped and
//! replaced from the snapshot. Families the snapshot does not model — price /
//! vol-index / basis-spread / parametric curves, credit indices, collateral
//! CSA mappings, hierarchy — always survive the restore unchanged (quant
//! review B2: the previous from-scratch rebuild silently dropped them,
//! breaking every instrument that depends on them).
//!
//! - **Curve families** (discount/forward/hazard/inflation/correlation): flagged curves
//!   are replaced from snapshot (drop-and-replace per family); unflagged curves are
//!   preserved from `current_market`. Credit indices are re-bound after a
//!   hazard/correlation restore so they resolve against the restored curves.
//! - **FX** (`FX` flag): if flagged, the snapshot's FX (possibly `None`) replaces the
//!   market's FX. If the snapshot's FX is `None` with the flag set, FX is cleared.
//!   If not flagged, FX is preserved from `current_market`.
//! - **Volatility** (`VOL` flag): if flagged, the snapshot's vol surfaces, SABR vol
//!   cubes AND FX delta-quoted vol surfaces replace the market's entirely. If not
//!   flagged, all three are preserved.
//! - **Scalars** (`SCALARS` flag): **DROP semantic** — if flagged, ALL scalars from
//!   `current_market` are dropped and ONLY the snapshot's scalars are inserted. This
//!   is load-bearing for factor isolation correctness. If not flagged, scalars are
//!   preserved from `current_market`.
//!
//! # See Also
//!
//! - [`crate::parallel`] - Parallel attribution using this module
//! - [`crate::waterfall`] - Waterfall attribution using this module

use finstack_quant_core::market_data::context::{CurveStorage, MarketContext};
use finstack_quant_core::market_data::dividends::DividendSchedule;
use finstack_quant_core::market_data::scalars::InflationIndex;
use finstack_quant_core::market_data::scalars::{MarketScalar, ScalarTimeSeries};
use finstack_quant_core::market_data::surfaces::{FxDeltaVolSurface, VolCube, VolSurface};
use finstack_quant_core::market_data::term_structures::BaseCorrelationCurve;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_core::market_data::term_structures::InflationCurve;
use finstack_quant_core::money::fx::FxMatrix;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::HashMap;
use std::sync::Arc;

/// Flags indicating which market factor families to restore from snapshot vs. preserve
/// from market.
///
/// Covers all market factor families: curves, FX, volatility surfaces and scalars.
///
/// # Examples
///
/// ```
/// use finstack_quant_attribution::MarketRestoreFlags;
///
/// // Restore only discount curves
/// let flags = MarketRestoreFlags::DISCOUNT;
///
/// // Restore both discount and forward curves (rates)
/// let rates = MarketRestoreFlags::RATES;
/// assert_eq!(rates, MarketRestoreFlags::DISCOUNT | MarketRestoreFlags::FORWARD);
///
/// // Restore FX and volatility surfaces together
/// let fx_vol = MarketRestoreFlags::FX | MarketRestoreFlags::VOL;
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct MarketRestoreFlags(u16);

impl MarketRestoreFlags {
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

impl std::ops::BitOr for MarketRestoreFlags {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for MarketRestoreFlags {
    type Output = Self;

    #[inline]
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl std::ops::Not for MarketRestoreFlags {
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
    /// SABR volatility cubes (populated when the `VOL` flag is set during extract).
    pub vol_cubes: HashMap<CurveId, Arc<VolCube>>,
    /// FX delta-quoted volatility surfaces (populated when the `VOL` flag is set).
    pub fx_delta_vol_surfaces: HashMap<CurveId, Arc<FxDeltaVolSurface>>,
    /// Market scalar prices (populated when the `SCALARS` flag is set)
    pub prices: HashMap<CurveId, MarketScalar>,
    /// Scalar time series excluding rate fixings (populated when the
    /// `SCALARS` flag is set). `FIXING:`-prefixed series belong to the rates
    /// family — see [`Self::fixing_series`].
    pub series: HashMap<CurveId, ScalarTimeSeries>,
    /// Historical rate fixing series (`FIXING:{forward_curve_id}` convention,
    /// populated when the `FORWARD` flag is set). Quant review Note: a
    /// floating-rate reset observed between T0 and T1 is RATES P&L, so fixing
    /// series restore with the forward curves, not with market scalars —
    /// otherwise a single economic rate move is split across two factor lines
    /// with a cross term.
    pub fixing_series: HashMap<CurveId, ScalarTimeSeries>,
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
    pub fn extract(market: &MarketContext, flags: MarketRestoreFlags) -> Self {
        let mut snapshot = Self::default();

        for curve_id in market.curve_ids() {
            if flags.contains(MarketRestoreFlags::DISCOUNT) {
                if let Ok(curve) = market.get_discount(curve_id) {
                    snapshot.discount_curves.insert(curve_id.clone(), curve);
                }
            }
            if flags.contains(MarketRestoreFlags::FORWARD) {
                if let Ok(curve) = market.get_forward(curve_id) {
                    snapshot.forward_curves.insert(curve_id.clone(), curve);
                }
            }
            if flags.contains(MarketRestoreFlags::HAZARD) {
                if let Ok(curve) = market.get_hazard(curve_id) {
                    snapshot.hazard_curves.insert(curve_id.clone(), curve);
                }
            }
            if flags.contains(MarketRestoreFlags::INFLATION) {
                if let Ok(curve) = market.get_inflation_curve(curve_id) {
                    snapshot.inflation_curves.insert(curve_id.clone(), curve);
                }
            }
            if flags.contains(MarketRestoreFlags::CORRELATION) {
                if let Ok(curve) = market.get_base_correlation(curve_id) {
                    snapshot
                        .base_correlation_curves
                        .insert(curve_id.clone(), curve);
                }
            }
        }

        if flags.contains(MarketRestoreFlags::FX) {
            snapshot.fx = market.fx().cloned();
        }

        if flags.contains(MarketRestoreFlags::VOL) {
            snapshot.surfaces = market.surfaces_snapshot();
            snapshot.vol_cubes = market
                .vol_cubes_iter()
                .map(|(k, v)| (k.clone(), Arc::clone(v)))
                .collect();
            snapshot.fx_delta_vol_surfaces = market
                .fx_delta_vol_surfaces_iter()
                .map(|(k, v)| (k.clone(), Arc::clone(v)))
                .collect();
        }

        if flags.contains(MarketRestoreFlags::FORWARD) {
            snapshot.fixing_series = market
                .series_iter()
                .filter(|(k, _)| {
                    k.as_str()
                        .starts_with(finstack_quant_core::market_data::fixings::FIXING_PREFIX)
                })
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
        }

        if flags.contains(MarketRestoreFlags::SCALARS) {
            snapshot.prices = market
                .prices_iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            snapshot.series = market
                .series_iter()
                .filter(|(k, _)| {
                    !k.as_str()
                        .starts_with(finstack_quant_core::market_data::fixings::FIXING_PREFIX)
                })
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
    /// Clone-and-overwrite: the result starts as a full clone of
    /// `current_market`, so every store the snapshot does not model (price /
    /// vol-index / basis-spread / parametric curves, credit indices,
    /// collateral CSA mappings, hierarchy) is preserved. For each family:
    ///
    /// - **Curves**: each flagged family is dropped and replaced from
    ///   `snapshot`; unflagged families are preserved from `current_market`.
    ///   Credit indices are re-bound after a hazard/correlation restore.
    /// - **FX**: if flagged, replaced by `snapshot.fx` (which may be `None`,
    ///   clearing FX); otherwise preserved from `current_market`.
    /// - **Volatility**: if flagged, vol surfaces, SABR cubes and FX-delta
    ///   surfaces are all replaced wholesale by the snapshot's; otherwise
    ///   preserved from `current_market`.
    /// - **Scalars**: if flagged, **all** scalars from `current_market` are
    ///   dropped and only the snapshot's scalars are inserted (this is
    ///   load-bearing for factor isolation). Otherwise scalars are preserved
    ///   from `current_market`.
    pub fn restore_market(
        current_market: &MarketContext,
        snapshot: &MarketSnapshot,
        restore_flags: MarketRestoreFlags,
    ) -> MarketContext {
        let mut new_market = current_market.clone();

        // --- Curves: drop-and-replace each FLAGGED family. The clone keeps
        // unflagged families and every family the snapshot does not model.
        new_market.retain_curves_mut(|_, curve| match curve {
            CurveStorage::Discount(_) => !restore_flags.contains(MarketRestoreFlags::DISCOUNT),
            CurveStorage::Forward(_) => !restore_flags.contains(MarketRestoreFlags::FORWARD),
            CurveStorage::Hazard(_) => !restore_flags.contains(MarketRestoreFlags::HAZARD),
            CurveStorage::Inflation(_) => !restore_flags.contains(MarketRestoreFlags::INFLATION),
            CurveStorage::BaseCorrelation(_) => {
                !restore_flags.contains(MarketRestoreFlags::CORRELATION)
            }
            _ => true,
        });
        for curve in snapshot.discount_curves.values() {
            new_market.insert_mut(Arc::clone(curve));
        }
        for curve in snapshot.forward_curves.values() {
            new_market.insert_mut(Arc::clone(curve));
        }
        for curve in snapshot.hazard_curves.values() {
            new_market.insert_mut(Arc::clone(curve));
        }
        for curve in snapshot.inflation_curves.values() {
            new_market.insert_mut(Arc::clone(curve));
        }
        for curve in snapshot.base_correlation_curves.values() {
            new_market.insert_mut(Arc::clone(curve));
        }

        // Credit indices hold direct references to hazard / base-correlation
        // curves; re-bind them so they resolve against the restored curves.
        if restore_flags.contains(MarketRestoreFlags::HAZARD)
            || restore_flags.contains(MarketRestoreFlags::CORRELATION)
        {
            let invalidated = new_market.rebind_credit_indices_mut();
            if !invalidated.is_empty() {
                tracing::warn!(
                    invalidated = ?invalidated,
                    "credit indices invalidated during snapshot restore (their                      curves are absent from the restored state)"
                );
            }
        }

        // --- FX ---
        if restore_flags.contains(MarketRestoreFlags::FX) {
            match &snapshot.fx {
                Some(fx) => {
                    new_market.insert_fx_mut(Arc::clone(fx));
                }
                None => {
                    new_market.clear_fx_mut();
                }
            }
        }

        // --- Volatility: surfaces, SABR cubes and FX-delta surfaces all
        // belong to the VOL family.
        if restore_flags.contains(MarketRestoreFlags::VOL) {
            new_market.replace_surfaces_mut(snapshot.surfaces.clone());
            new_market.replace_vol_cubes_mut(
                snapshot
                    .vol_cubes
                    .iter()
                    .map(|(k, v)| (k.clone(), Arc::clone(v))),
            );
            new_market.replace_fx_delta_vol_surfaces_mut(
                snapshot
                    .fx_delta_vol_surfaces
                    .iter()
                    .map(|(k, v)| (k.clone(), Arc::clone(v))),
            );
        }

        // --- Scalars: DROP-and-replace if flagged, else preserved by the clone.
        //
        // Drop semantic is intentional: a scalar present in `current_market` but
        // absent from `snapshot` must NOT appear in the result. This keeps factor
        // isolation correct for the attribution call paths.
        //
        // `FIXING:`-prefixed series are EXCLUDED from the scalars family: they
        // belong to the rates (FORWARD) family below (prior fix).
        if restore_flags.contains(MarketRestoreFlags::SCALARS) {
            let preserved_fixings: Vec<ScalarTimeSeries> = new_market
                .series_iter()
                .filter(|(k, _)| {
                    k.as_str()
                        .starts_with(finstack_quant_core::market_data::fixings::FIXING_PREFIX)
                })
                .map(|(_, v)| v.clone())
                .collect();
            new_market.clear_market_scalars_mut();
            for series in preserved_fixings {
                new_market.insert_series_mut(series);
            }
            for (id, price) in &snapshot.prices {
                new_market.insert_price_mut(id.as_str(), price.clone());
            }
            for series in snapshot.series.values() {
                new_market.insert_series_mut(series.clone());
            }
            for (id, index) in &snapshot.inflation_indices {
                new_market.insert_inflation_index_mut(id.as_str(), Arc::clone(index));
            }
            for schedule in snapshot.dividends.values() {
                new_market.insert_dividends_mut(Arc::clone(schedule));
            }
        }

        // --- Rate fixings: restored with the FORWARD (rates) family so a
        // floating-rate reset between T0 and T1 is attributed to rates, not
        // market scalars.
        if restore_flags.contains(MarketRestoreFlags::FORWARD) {
            new_market.retain_series_mut(|id, _| {
                !id.as_str()
                    .starts_with(finstack_quant_core::market_data::fixings::FIXING_PREFIX)
            });
            for series in snapshot.fixing_series.values() {
                new_market.insert_series_mut(series.clone());
            }
        }

        new_market
    }
}
