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
//! replaced from the snapshot. Families the snapshot does not model — credit
//! indices, collateral CSA mappings, hierarchy — always survive the restore
//! unchanged.
//!
//! Every curve is owned by exactly one flag family. Attribution execution uses
//! the instrument's declared dependencies to assign risky discount curves to
//! `CREDIT` and scalar volatility to `VOL`; the generic snapshot API uses the
//! storage defaults below. Each [`CurveStorage`] variant otherwise has one default family:
//!
//! | `CurveStorage` variant | Flag family | Attribution factor |
//! |------------------------|-------------|--------------------|
//! | `Discount`             | `DISCOUNT`  | RatesCurves        |
//! | `Forward`              | `FORWARD`   | RatesCurves        |
//! | `BasisSpread`          | `FORWARD`   | RatesCurves        |
//! | `Parametric`           | `FORWARD`   | RatesCurves        |
//! | `Hazard`               | `HAZARD`    | CreditCurves       |
//! | `Inflation`            | `INFLATION` | InflationCurves    |
//! | `BaseCorrelation`      | `CORRELATION` | Correlations     |
//! | `VolIndex`             | `VOL`       | Volatility         |
//! | `Price`                | `SCALARS`   | MarketScalars      |
//!
//! - **Curve families**: flagged curves are replaced from snapshot
//!   (drop-and-replace per family); unflagged curves are preserved from
//!   `current_market`. Credit indices are re-bound after a
//!   hazard/correlation restore so they resolve against the restored curves.
//! - **FX** (`FX` flag): if flagged, the snapshot's FX (possibly `None`) replaces the
//!   market's FX. If the snapshot's FX is `None` with the flag set, FX is cleared.
//!   If not flagged, FX is preserved from `current_market`.
//! - **Volatility** (`VOL` flag): if flagged, the snapshot's vol surfaces, SABR vol
//!   cubes, FX delta-quoted vol surfaces, volatility-index curves and declared
//!   scalar volatility replace the market's. Otherwise they are preserved.
//! - **Inflation** (`INFLATION` flag): inflation curves and published CPI indices
//!   restore together, regardless of their storage representation.
//! - **Scalars** (`SCALARS` flag): prices other than declared scalar volatility,
//!   non-fixing series, dividends and commodity price curves are replaced.
//!   CPI indices, rate fixings and scalar volatility retain their own factor roles.
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
use finstack_quant_core::market_data::term_structures::BasisSpreadCurve;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_core::market_data::term_structures::InflationCurve;
use finstack_quant_core::market_data::term_structures::ParametricCurve;
use finstack_quant_core::market_data::term_structures::PriceCurve;
use finstack_quant_core::money::fx::FxMatrix;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::HashMap;
use finstack_quant_valuations::instruments::MarketDependencies;
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

    /// Restore forward curves from snapshot.
    ///
    /// Basis-spread and parametric (Nelson-Siegel) curves are rate
    /// projection term structures, so they restore with this family too.
    pub const FORWARD: Self = Self(Self::FORWARD_BIT);

    /// Restore hazard curves from snapshot
    pub const HAZARD: Self = Self(Self::HAZARD_BIT);

    /// Restore inflation curves and published CPI indices from snapshot.
    pub const INFLATION: Self = Self(Self::INFLATION_BIT);

    /// Restore base correlation curves from snapshot
    pub const CORRELATION: Self = Self(Self::CORRELATION_BIT);

    /// Restore FX matrix from snapshot.
    ///
    /// If the snapshot has `fx = None`, this flag intentionally clears FX from
    /// the restored market instead of preserving the current market's FX.
    pub const FX: Self = Self(Self::FX_BIT);

    /// Restore volatility surfaces, SABR cubes, FX delta-quoted surfaces,
    /// volatility-index curves, and declared scalar volatility from snapshot.
    pub const VOL: Self = Self(Self::VOL_BIT);

    /// Restore non-volatility prices, non-fixing series, dividend schedules and
    /// commodity price curves. Members of this family absent from the snapshot
    /// are dropped. Published CPI indices and declared scalar volatility belong
    /// to `INFLATION` and `VOL`, respectively.
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
    /// Discount curves serving a credit role for the attributed instrument.
    pub credit_discount_curves: HashMap<CurveId, Arc<DiscountCurve>>,
    /// Declared credit curve IDs, including those absent from this snapshot.
    /// Used to preserve their economic role when dropping a restored family.
    pub credit_curve_ids: Vec<CurveId>,
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
    /// Basis spread curves indexed by curve ID (populated with the `FORWARD`
    /// flag — rates family).
    pub basis_spread_curves: HashMap<CurveId, Arc<BasisSpreadCurve>>,
    /// Parametric (Nelson-Siegel) curves indexed by curve ID (populated with
    /// the `FORWARD` flag — rates family).
    pub parametric_curves: HashMap<CurveId, Arc<ParametricCurve>>,
    /// Volatility-index curves indexed by curve ID (populated when the `VOL`
    /// flag is set).
    pub vol_index_curves: HashMap<CurveId, Arc<PriceCurve>>,
    /// Commodity/price curves indexed by curve ID (populated when the
    /// `SCALARS` flag is set).
    pub price_curves: HashMap<CurveId, Arc<PriceCurve>>,
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
    /// Declared volatility IDs that do not also identify their underlying spot.
    /// Retained even when absent from the snapshot to preserve removal semantics.
    pub volatility_scalar_ids: Vec<CurveId>,
    /// Scalar volatility quotes (populated when the `VOL` flag is set).
    pub volatility_scalars: HashMap<CurveId, MarketScalar>,
    /// Market scalar prices excluding declared volatility (populated with `SCALARS`).
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
    /// Published inflation indices (populated when the `INFLATION` flag is set).
    pub inflation_indices: HashMap<CurveId, Arc<InflationIndex>>,
    /// Dividend schedules (populated when the `SCALARS` flag is set)
    pub dividends: HashMap<CurveId, Arc<DividendSchedule>>,
}

impl MarketSnapshot {
    /// Extract factor families from a market context based on which flags are set.
    ///
    /// Only the families corresponding to set flags are populated into the snapshot;
    /// other fields remain empty (or `None` for FX).
    ///
    /// # Arguments
    ///
    /// * `market` - Market context providing curves, surfaces, and fixing data for pricing
    /// * `flags` - Feature flags controlling optional attribution components.
    pub fn extract(market: &MarketContext, flags: MarketRestoreFlags) -> Self {
        Self::extract_with_dependencies(market, flags, &MarketDependencies::new())
    }

    /// Extract economic factor families using the instrument's declared input roles.
    pub(crate) fn extract_with_dependencies(
        market: &MarketContext,
        flags: MarketRestoreFlags,
        dependencies: &MarketDependencies,
    ) -> Self {
        let credit_curve_ids = &dependencies.curves.credit_curves;
        let volatility_scalar_ids = dependencies
            .volatility_dependencies
            .iter()
            .filter(|dependency| {
                dependency.underlying_id.as_ref().is_none_or(|underlying| {
                    underlying.as_str() != dependency.vol_surface_id.as_str()
                })
            })
            .map(|dependency| dependency.vol_surface_id.clone())
            .collect();
        let mut snapshot = Self {
            credit_curve_ids: credit_curve_ids.to_vec(),
            volatility_scalar_ids,
            ..Self::default()
        };

        let extract_curves = flags.contains(MarketRestoreFlags::DISCOUNT)
            || flags.contains(MarketRestoreFlags::FORWARD)
            || flags.contains(MarketRestoreFlags::HAZARD)
            || flags.contains(MarketRestoreFlags::INFLATION)
            || flags.contains(MarketRestoreFlags::CORRELATION)
            || flags.contains(MarketRestoreFlags::VOL)
            || flags.contains(MarketRestoreFlags::SCALARS);
        if extract_curves {
            for (curve_id, storage) in market.iter_curves() {
                match storage {
                    CurveStorage::Discount(curve) if credit_curve_ids.contains(curve_id) => {
                        if flags.contains(MarketRestoreFlags::CREDIT) {
                            snapshot
                                .credit_discount_curves
                                .insert(curve_id.clone(), Arc::clone(curve));
                        }
                    }
                    CurveStorage::Discount(curve)
                        if flags.contains(MarketRestoreFlags::DISCOUNT) =>
                    {
                        snapshot
                            .discount_curves
                            .insert(curve_id.clone(), Arc::clone(curve));
                    }
                    CurveStorage::Forward(curve) if flags.contains(MarketRestoreFlags::FORWARD) => {
                        snapshot
                            .forward_curves
                            .insert(curve_id.clone(), Arc::clone(curve));
                    }
                    CurveStorage::BasisSpread(curve)
                        if flags.contains(MarketRestoreFlags::FORWARD) =>
                    {
                        snapshot
                            .basis_spread_curves
                            .insert(curve_id.clone(), Arc::clone(curve));
                    }
                    CurveStorage::Parametric(curve)
                        if flags.contains(MarketRestoreFlags::FORWARD) =>
                    {
                        snapshot
                            .parametric_curves
                            .insert(curve_id.clone(), Arc::clone(curve));
                    }
                    CurveStorage::Hazard(curve) if flags.contains(MarketRestoreFlags::HAZARD) => {
                        snapshot
                            .hazard_curves
                            .insert(curve_id.clone(), Arc::clone(curve));
                    }
                    CurveStorage::Inflation(curve)
                        if flags.contains(MarketRestoreFlags::INFLATION) =>
                    {
                        snapshot
                            .inflation_curves
                            .insert(curve_id.clone(), Arc::clone(curve));
                    }
                    CurveStorage::BaseCorrelation(curve)
                        if flags.contains(MarketRestoreFlags::CORRELATION) =>
                    {
                        snapshot
                            .base_correlation_curves
                            .insert(curve_id.clone(), Arc::clone(curve));
                    }
                    CurveStorage::VolIndex(curve) if flags.contains(MarketRestoreFlags::VOL) => {
                        snapshot
                            .vol_index_curves
                            .insert(curve_id.clone(), Arc::clone(curve));
                    }
                    CurveStorage::Price(curve) if flags.contains(MarketRestoreFlags::SCALARS) => {
                        snapshot
                            .price_curves
                            .insert(curve_id.clone(), Arc::clone(curve));
                    }
                    _ => {}
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
            snapshot.volatility_scalars = market
                .prices_iter()
                .filter(|(id, _)| snapshot.volatility_scalar_ids.contains(id))
                .map(|(id, value)| (id.clone(), value.clone()))
                .collect();
        }

        if flags.contains(MarketRestoreFlags::INFLATION) {
            snapshot.inflation_indices = market
                .inflation_indices_iter()
                .map(|(id, index)| (id.clone(), Arc::clone(index)))
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
                .filter(|(id, _)| !snapshot.volatility_scalar_ids.contains(id))
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
    /// `current_market`, so every store the snapshot does not model (credit
    /// indices, collateral CSA mappings, hierarchy) is preserved. For each
    /// family:
    ///
    /// - **Curves**: each flagged family is dropped and replaced from
    ///   `snapshot`; unflagged families are preserved from `current_market`.
    ///   Every [`CurveStorage`] variant belongs to exactly one flag family
    ///   (see the module-level table): basis-spread and parametric curves
    ///   restore with `FORWARD`, vol-index curves with `VOL`, and price
    ///   curves with `SCALARS`. Credit indices are re-bound after a
    ///   hazard/correlation restore.
    /// - **FX**: if flagged, replaced by `snapshot.fx` (which may be `None`,
    ///   clearing FX); otherwise preserved from `current_market`.
    /// - **Volatility**: if flagged, vol surfaces, SABR cubes, FX-delta
    ///   surfaces, vol-index curves and declared scalar volatility are replaced;
    ///   otherwise preserved from `current_market`.
    /// - **Inflation**: curves and published CPI indices are replaced together.
    /// - **Scalars**: non-volatility prices, non-fixing series, dividends and
    ///   price curves are replaced. Other scalar-store families are preserved.
    ///
    /// # Arguments
    ///
    /// * `current_market` - Market whose unflagged economic factors are preserved.
    /// * `snapshot` - Replacement factor values and declared input roles. A
    ///   missing value removes that input when its family is flagged.
    /// * `restore_flags` - Economic families to replace from `snapshot`.
    pub fn restore_market(
        current_market: &MarketContext,
        snapshot: &MarketSnapshot,
        restore_flags: MarketRestoreFlags,
    ) -> MarketContext {
        let mut new_market = current_market.clone();

        // --- Curves: drop-and-replace each FLAGGED family. The clone keeps
        // unflagged families. The match is deliberately EXHAUSTIVE (no `_`
        // arm): every `CurveStorage` variant must be owned by exactly one
        // flag family, so adding a tenth variant is a compile error here
        // instead of a silent restore gap.
        new_market.retain_curves_mut(|id, curve| match curve {
            CurveStorage::Discount(_) if snapshot.credit_curve_ids.contains(id) => {
                !restore_flags.contains(MarketRestoreFlags::CREDIT)
            }
            CurveStorage::Discount(_) => !restore_flags.contains(MarketRestoreFlags::DISCOUNT),
            CurveStorage::Forward(_)
            | CurveStorage::BasisSpread(_)
            | CurveStorage::Parametric(_) => !restore_flags.contains(MarketRestoreFlags::FORWARD),
            CurveStorage::Hazard(_) => !restore_flags.contains(MarketRestoreFlags::HAZARD),
            CurveStorage::Inflation(_) => !restore_flags.contains(MarketRestoreFlags::INFLATION),
            CurveStorage::BaseCorrelation(_) => {
                !restore_flags.contains(MarketRestoreFlags::CORRELATION)
            }
            CurveStorage::VolIndex(_) => !restore_flags.contains(MarketRestoreFlags::VOL),
            CurveStorage::Price(_) => !restore_flags.contains(MarketRestoreFlags::SCALARS),
        });
        for curve in snapshot.discount_curves.values() {
            new_market.insert_mut(Arc::clone(curve));
        }
        for curve in snapshot.credit_discount_curves.values() {
            new_market.insert_mut(Arc::clone(curve));
        }
        for curve in snapshot.forward_curves.values() {
            new_market.insert_mut(Arc::clone(curve));
        }
        for curve in snapshot.basis_spread_curves.values() {
            new_market.insert_mut(Arc::clone(curve));
        }
        for curve in snapshot.parametric_curves.values() {
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
        for curve in snapshot.vol_index_curves.values() {
            new_market.insert_mut(Arc::clone(curve));
        }
        for curve in snapshot.price_curves.values() {
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

        // --- Scalar stores: replace only the flagged economic families.
        let restore_scalars = restore_flags.contains(MarketRestoreFlags::SCALARS);
        let restore_inflation = restore_flags.contains(MarketRestoreFlags::INFLATION);
        let restore_scalar_vol = restore_flags.contains(MarketRestoreFlags::VOL)
            && !snapshot.volatility_scalar_ids.is_empty();
        if restore_scalars || restore_inflation || restore_scalar_vol {
            new_market.clear_market_scalars_mut();
            for (id, price) in current_market.prices_iter() {
                let replace = if snapshot.volatility_scalar_ids.contains(id) {
                    restore_scalar_vol
                } else {
                    restore_scalars
                };
                if !replace {
                    new_market.insert_price_mut(id.as_str(), price.clone());
                }
            }
            if restore_scalars {
                for (id, price) in &snapshot.prices {
                    new_market.insert_price_mut(id.as_str(), price.clone());
                }
            }
            if restore_scalar_vol {
                for (id, price) in &snapshot.volatility_scalars {
                    new_market.insert_price_mut(id.as_str(), price.clone());
                }
            }
            // Rate fixings are restored by FORWARD below, independently of
            // SCALARS, so preserve them through this store rebuild.
            for (id, series) in current_market.series_iter() {
                if !restore_scalars
                    || id
                        .as_str()
                        .starts_with(finstack_quant_core::market_data::fixings::FIXING_PREFIX)
                {
                    new_market.insert_series_mut(series.clone());
                }
            }
            if restore_scalars {
                for series in snapshot.series.values() {
                    new_market.insert_series_mut(series.clone());
                }
            }
            if restore_inflation {
                for (id, index) in &snapshot.inflation_indices {
                    new_market.insert_inflation_index_mut(id.as_str(), Arc::clone(index));
                }
            } else {
                for (id, index) in current_market.inflation_indices_iter() {
                    new_market.insert_inflation_index_mut(id.as_str(), Arc::clone(index));
                }
            }
            if restore_scalars {
                for schedule in snapshot.dividends.values() {
                    new_market.insert_dividends_mut(Arc::clone(schedule));
                }
            } else {
                for (_, schedule) in current_market.dividends_iter() {
                    new_market.insert_dividends_mut(Arc::clone(schedule));
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::types::PriceId;
    use finstack_quant_valuations::instruments::VolatilityDependency;
    use time::macros::date;

    fn dependencies() -> MarketDependencies {
        let mut dependencies = MarketDependencies::new();
        dependencies.add_volatility_dependency(VolatilityDependency::new(
            "EQ-VOL",
            Some(PriceId::new("EQ")),
            None,
        ));
        dependencies.add_volatility_dependency(VolatilityDependency::new(
            "EQ",
            Some(PriceId::new("EQ")),
            None,
        ));
        dependencies
    }

    fn market(spot: f64, vol: f64, cpi: f64) -> MarketContext {
        MarketContext::new()
            .insert_price("EQ", MarketScalar::Unitless(spot))
            .insert_price("EQ-VOL", MarketScalar::Unitless(vol))
            .insert_inflation_index(
                "CPI",
                InflationIndex::new("CPI", vec![(date!(2025 - 01 - 01), cpi)], Currency::USD)
                    .unwrap(),
            )
    }

    fn scalar(market: &MarketContext, id: &str) -> f64 {
        match market.get_price(id).unwrap() {
            MarketScalar::Unitless(value) => *value,
            MarketScalar::Price(_) => panic!("expected unitless scalar"),
        }
    }

    #[test]
    fn scalar_stores_restore_only_their_economic_factors() {
        let opening = market(100.0, 0.2, 300.0);
        let closing = market(110.0, 0.3, 310.0);
        let dependencies = dependencies();
        for mask in 0..8u8 {
            let mut flags = MarketRestoreFlags::empty();
            if mask & 1 != 0 {
                flags = flags | MarketRestoreFlags::SCALARS;
            }
            if mask & 2 != 0 {
                flags = flags | MarketRestoreFlags::VOL;
            }
            if mask & 4 != 0 {
                flags = flags | MarketRestoreFlags::INFLATION;
            }
            let snapshot =
                MarketSnapshot::extract_with_dependencies(&opening, flags, &dependencies);
            let restored = MarketSnapshot::restore_market(&closing, &snapshot, flags);
            assert_eq!(
                scalar(&restored, "EQ"),
                if flags.contains(MarketRestoreFlags::SCALARS) {
                    100.0
                } else {
                    110.0
                }
            );
            assert_eq!(
                scalar(&restored, "EQ-VOL"),
                if flags.contains(MarketRestoreFlags::VOL) {
                    0.2
                } else {
                    0.3
                }
            );
            assert_eq!(
                restored
                    .get_inflation_index("CPI")
                    .unwrap()
                    .value_on(date!(2025 - 01 - 01))
                    .unwrap(),
                if flags.contains(MarketRestoreFlags::INFLATION) {
                    300.0
                } else {
                    310.0
                }
            );
        }
    }

    #[test]
    fn missing_opening_quotes_are_removed_only_by_their_own_factor() {
        let closing = market(110.0, 0.3, 310.0);
        let dependencies = dependencies();
        for flags in [
            MarketRestoreFlags::SCALARS,
            MarketRestoreFlags::VOL,
            MarketRestoreFlags::INFLATION,
        ] {
            let snapshot = MarketSnapshot::extract_with_dependencies(
                &MarketContext::new(),
                flags,
                &dependencies,
            );
            let restored = MarketSnapshot::restore_market(&closing, &snapshot, flags);
            assert_eq!(
                restored.get_price("EQ").is_err(),
                flags.contains(MarketRestoreFlags::SCALARS)
            );
            assert_eq!(
                restored.get_price("EQ-VOL").is_err(),
                flags.contains(MarketRestoreFlags::VOL)
            );
            assert_eq!(
                restored.get_inflation_index("CPI").is_err(),
                flags.contains(MarketRestoreFlags::INFLATION)
            );
        }
    }
}
