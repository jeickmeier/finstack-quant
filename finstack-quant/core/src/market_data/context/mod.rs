//! Market data context for aggregating curves, surfaces, and FX rates.
//!
//! `MarketContext` is the primary container for market data used in valuations.
//! It aggregates discount curves, forward curves, hazard curves, volatility
//! surfaces, FX rates, and market scalars into a single, thread-safe structure.
//!
//! # Design
//!
//! - **Arc-based storage**: Cheap to clone and share across threads
//! - **Type-safe retrieval**: Separate methods for each curve type
//! - **Builder pattern**: Fluent API for constructing contexts
//! - **Scenario support**: Bump curves for risk sensitivities
//!
//! # API boundaries
//!
//! - **Public surface**: `new`, `insert_*`, typed getters (`get_discount`, `surface`,
//!   `price`, `series`, etc.), scenario helpers (`bump`, scratch bump tokens),
//!   stats (`stats`) and serde states (`CurveState`, `MarketContextState`).
//! - **Advanced plumbing**:
//!   [`CurveStorage`][crate::market_data::context::CurveStorage] is
//!   intentionally public because scenario adapters and snapshot serde use it
//!   across crate boundaries, but most callers should prefer the typed
//!   insert/getter methods.
//! - **Internal details**: map layout, caches, and rebind mechanics are not a
//!   stable API. Prefer the public methods above for all access and mutation.
//!
//! # Examples
//! ```rust
//! use finstack_quant_core::market_data::context::MarketContext;
//! use finstack_quant_core::market_data::term_structures::DiscountCurve;
//! use finstack_quant_core::math::interp::InterpStyle;
//! use finstack_quant_core::types::CurveId;
//! use finstack_quant_core::dates::Date;
//! use time::Month;
//!
//! let base_date = Date::from_calendar_date(2024, Month::January, 1).expect("Valid date");
//! let curve = DiscountCurve::builder("USD-OIS")
//!     .base_date(base_date)
//!     .knots([(0.0, 1.0), (1.0, 0.98)])
//!     .interp(InterpStyle::Linear)
//!     .build()
//!     .expect("DiscountCurve builder should succeed");
//!
//! let ctx = MarketContext::new().insert(curve);
//! let retrieved = ctx.get_discount("USD-OIS").expect("Discount curve should exist");
//! assert_eq!(retrieved.id(), &CurveId::from("USD-OIS"));
//! ```

mod ops_bump;
mod ops_roll;
mod stats;

mod state_serde;

pub use stats::ContextStats;

// Re-export bump functionality at the same path as before.
pub use super::bumps::{BumpMode, BumpSpec, BumpUnits};

/// Non-optional observability for market context mutations.
///
/// Returned by operations that can invalidate credit indices or change
/// context state in non-obvious ways. Always populated regardless of
/// subscriber configuration.
#[derive(Clone, Debug, Default)]
pub struct ContextMutationInfo {
    /// Credit indices that were invalidated and removed because their
    /// required curves are no longer present or changed type.
    pub invalidated_credit_indices: Vec<CurveId>,
}

impl ContextMutationInfo {
    /// Returns `true` if any credit indices were invalidated.
    #[must_use]
    pub fn has_invalidations(&self) -> bool {
        !self.invalidated_credit_indices.is_empty()
    }
}

/// Reversible in-place bump token for scratch market-context workflows.
///
/// This is intended for hot-path finite-difference calculations that need to
/// apply a small number of bumps to a reusable scratch `MarketContext` and then
/// restore the previous state without cloning the full context.
#[derive(Clone)]
pub enum ContextScratchBump {
    /// A bumped curve plus the credit-index snapshot needed to restore it.
    Curve {
        /// Curve identifier that was bumped.
        id: CurveId,
        /// Pre-bump curve storage.
        previous: CurveStorage,
        /// Pre-bump credit-index snapshot, captured only when the bumped curve
        /// actually feeds a credit index. `None` means no credit indices were
        /// rebound, so the snapshot is skipped on the common rates path. The
        /// snapshot is a cheap `Arc` clone of the whole map (copy-on-write).
        previous_credit_indices: Option<Arc<HashMap<CurveId, Arc<CreditIndexData>>>>,
    },
    /// A bumped volatility surface.
    Surface {
        /// Surface identifier that was bumped.
        id: CurveId,
        /// Pre-bump surface.
        previous: Arc<VolSurface>,
    },
    /// A bumped scalar/price.
    Price {
        /// Scalar identifier that was bumped.
        id: CurveId,
        /// Pre-bump scalar value.
        previous: MarketScalar,
    },
}

pub use state_serde::{
    build_snapshot_fx_matrix, CreditIndexState, CurveState, MarketContextState,
    MARKET_CONTEXT_STATE_VERSION,
};

use crate::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::currency::Currency;
use crate::dates::Date;
use crate::error::InputError;
use crate::market_data::bumps::{BumpType, Bumpable};
use crate::market_data::traits::Discounting;
use crate::money::fx::{FxMatrix, FxQuery};
use crate::money::Money;
use crate::types::CurveId;
use crate::Result;

use super::{
    dividends::DividendSchedule,
    hierarchy::{CompletenessReport, MarketDataHierarchy, SubtreeCoverage},
    scalars::InflationIndex,
    scalars::{MarketScalar, ScalarTimeSeries},
    surfaces::{FxDeltaVolSurface, VolCube, VolSurface},
    term_structures::{
        BaseCorrelationCurve, BasisSpreadCurve, CreditIndexData, DiscountCurve, ForwardCurve,
        HazardCurve, InflationCurve, ParametricCurve, PriceCurve, VolatilityIndexCurve,
    },
};

/// Unified market data context with enum-based storage.
///
/// The context is constructed fluently (each `insert_*` returns a new context)
/// and is cheap to clone thanks to pervasive `Arc` usage. Typical workflows
/// construct a base context at scenario initialisation and reuse it across
/// pricing engines.
#[derive(Clone, Default)]
pub struct MarketContext {
    // Each map is wrapped in `Arc` so cloning the context (e.g. per finite-
    // difference greek bump) is a refcount bump rather than re-allocating and
    // rehashing every map. Mutators use `Arc::make_mut`, so a bump that touches
    // one map copies only that map (copy-on-write) and leaves the rest shared.
    /// All curves stored in unified enum-based map
    curves: Arc<HashMap<CurveId, CurveStorage>>,

    /// Foreign-exchange matrix
    fx: Option<Arc<FxMatrix>>,

    /// Volatility surfaces
    surfaces: Arc<HashMap<CurveId, Arc<VolSurface>>>,

    /// Market scalars and prices
    prices: Arc<HashMap<CurveId, MarketScalar>>,

    /// Generic time series
    series: Arc<HashMap<CurveId, ScalarTimeSeries>>,

    /// Inflation indices
    inflation_indices: Arc<HashMap<CurveId, Arc<InflationIndex>>>,

    /// Credit index aggregates
    credit_indices: Arc<HashMap<CurveId, Arc<CreditIndexData>>>,

    /// Shared dividend schedules keyed by `CurveId` (e.g., "AAPL-DIVS")
    dividends: Arc<HashMap<CurveId, Arc<DividendSchedule>>>,

    /// FX delta-quoted volatility surfaces
    fx_delta_vol_surfaces: Arc<HashMap<CurveId, Arc<FxDeltaVolSurface>>>,

    /// SABR volatility cubes (expiry x tenor x strike)
    vol_cubes: Arc<HashMap<CurveId, Arc<VolCube>>>,

    /// Collateral CSA code mappings
    collateral: Arc<HashMap<String, CurveId>>,

    /// Optional market data hierarchy for organizational grouping.
    hierarchy: Option<MarketDataHierarchy>,
}

impl std::fmt::Debug for MarketContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MarketContext")
            .field("curves", &self.curves.len())
            .field("fx", &self.fx.as_ref().map(|_| ".."))
            .field("surfaces", &self.surfaces.len())
            .field("prices", &self.prices.len())
            .field("series", &self.series.len())
            .field("inflation_indices", &self.inflation_indices.len())
            .field("credit_indices", &self.credit_indices.len())
            .field("dividends", &self.dividends.len())
            .field("fx_delta_vol_surfaces", &self.fx_delta_vol_surfaces.len())
            .field("vol_cubes", &self.vol_cubes.len())
            .field("collateral", &self.collateral.len())
            .field("hierarchy", &self.hierarchy.is_some())
            .finish()
    }
}

impl MarketContext {
    /// Create an empty market context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Borrow the FX matrix if present.
    #[inline]
    pub fn fx(&self) -> Option<&Arc<FxMatrix>> {
        self.fx.as_ref()
    }

    /// Borrow the FX matrix, returning a `NotFound` error if absent.
    ///
    /// Convenience wrapper around [`Self::fx`] for the common case where an
    /// absent FX matrix should surface as a `NotFound` error keyed by
    /// `"fx_matrix"`. Use this to avoid repeating the
    /// `fx().ok_or_else(|| InputError::NotFound { id: "fx_matrix".into() })`
    /// pattern.
    ///
    /// # Errors
    ///
    /// Returns [`InputError::NotFound`] with id `"fx_matrix"` when this
    /// context has no FX matrix attached.
    #[inline]
    pub fn fx_required(&self) -> crate::Result<&Arc<FxMatrix>> {
        self.fx.as_ref().ok_or_else(|| {
            InputError::NotFound {
                id: "fx_matrix".to_string(),
            }
            .into()
        })
    }

    /// Convert a [`Money`] amount into `target_ccy` using the embedded FX matrix.
    ///
    /// Centralizes the "same-ccy shortcut → FX matrix lookup → rate application"
    /// pattern used across valuations, portfolio, margin, and cashflow aggregation.
    /// Returns the input unchanged when it is already denominated in `target_ccy`.
    ///
    /// Callers that need a different error taxonomy (e.g. portfolio's
    /// `MissingMarketData` / `FxConversionFailed` split) should perform their own
    /// [`Self::fx_required`] check and call [`FxMatrix::rate`] directly, preserving
    /// the domain-specific error mapping while still sharing the rate lookup
    /// machinery.
    ///
    /// # Arguments
    ///
    /// * `amount` - Monetary amount to convert.
    /// * `target_ccy` - Destination currency.
    /// * `as_of` - Date used for the FX rate lookup.
    ///
    /// # Errors
    ///
    /// * [`InputError::NotFound`] (id `"fx_matrix"`) when no FX matrix is attached.
    /// * Any error surfaced by [`FxMatrix::rate`] when the requested pair is
    ///   unavailable.
    pub fn convert_money(
        &self,
        amount: Money,
        target_ccy: Currency,
        as_of: Date,
    ) -> crate::Result<Money> {
        if amount.currency() == target_ccy {
            return Ok(amount);
        }
        let fx = self.fx_required()?;
        let rate = fx.rate(FxQuery::new(amount.currency(), target_ccy, as_of))?;
        amount.convert_at_rate(target_ccy, rate.rate)
    }

    /// Snapshot (clone) all stored volatility surfaces.
    ///
    /// This clones the `Arc` handles (cheap) without exposing internal storage.
    #[inline]
    pub fn surfaces_snapshot(&self) -> HashMap<CurveId, Arc<VolSurface>> {
        self.surfaces.as_ref().clone()
    }

    /// Snapshot (clone) all stored market scalars/prices.
    #[inline]
    pub fn prices_snapshot(&self) -> HashMap<CurveId, MarketScalar> {
        self.prices.as_ref().clone()
    }

    /// Snapshot (clone) all stored scalar time series.
    #[inline]
    pub fn series_snapshot(&self) -> HashMap<CurveId, ScalarTimeSeries> {
        self.series.as_ref().clone()
    }

    #[inline]
    pub(crate) fn inflation_index_key_for_insert(
        id: impl AsRef<str>,
        index: &InflationIndex,
    ) -> CurveId {
        let key = CurveId::from(id.as_ref());
        assert!(
            key.as_str() == index.id,
            "MarketContext::insert_inflation_index key '{}' must match InflationIndex.id '{}'",
            key.as_str(),
            index.id
        );
        key
    }

    pub(crate) fn rebind_credit_index_data(
        &self,
        data: &CreditIndexData,
    ) -> crate::Result<CreditIndexData> {
        let mut rebuilt = data.clone();

        rebuilt.index_credit_curve = self.get_hazard(rebuilt.index_credit_curve.id().as_str())?;
        rebuilt.base_correlation_curve =
            self.get_base_correlation(rebuilt.base_correlation_curve.id().as_str())?;
        if let Some(issuer_curves) = rebuilt.issuer_credit_curves.as_mut() {
            for curve in issuer_curves.values_mut() {
                *curve = self.get_hazard(curve.id().as_str())?;
            }
        }

        Ok(rebuilt)
    }

    /// Get the attached hierarchy, if any.
    pub fn hierarchy(&self) -> Option<&MarketDataHierarchy> {
        self.hierarchy.as_ref()
    }

    /// Attach a market data hierarchy.
    pub fn set_hierarchy(&mut self, h: MarketDataHierarchy) {
        self.hierarchy = Some(h);
    }

    /// Generate a completeness report comparing hierarchy declarations against
    /// all `CurveId`-keyed data stores. Returns `None` if no hierarchy is attached.
    pub fn completeness_report(&self) -> Option<CompletenessReport> {
        let hierarchy = self.hierarchy.as_ref()?;

        // Collect all CurveIds present in any store.
        let mut present: HashSet<CurveId> = HashSet::default();
        present.extend(self.curves.keys().cloned());
        present.extend(self.surfaces.keys().cloned());
        present.extend(self.prices.keys().cloned());
        present.extend(self.series.keys().cloned());
        present.extend(self.inflation_indices.keys().cloned());
        present.extend(self.credit_indices.keys().cloned());
        present.extend(self.dividends.keys().cloned());
        present.extend(self.fx_delta_vol_surfaces.keys().cloned());
        present.extend(self.vol_cubes.keys().cloned());

        // Find missing: declared in hierarchy but absent from all stores.
        let declared = hierarchy.all_curve_ids();
        let declared_set: HashSet<CurveId> = declared.iter().cloned().collect();

        let mut missing = Vec::new();
        for id in &declared {
            if !present.contains(id) {
                let path = hierarchy.path_for_curve(id).unwrap_or_default();
                debug_assert!(!path.is_empty(), "CurveId {id:?} found by all_curve_ids but not path_for_curve — tree inconsistency");
                missing.push((path, id.clone()));
            }
        }
        missing.sort_unstable_by(|a, b| a.1.cmp(&b.1));

        // Find unclassified: present in stores but not in hierarchy.
        let mut unclassified: Vec<CurveId> = present
            .iter()
            .filter(|id| !declared_set.contains(*id))
            .cloned()
            .collect();
        unclassified.sort_unstable();

        // Coverage per root subtree.
        let mut coverage = Vec::new();
        for (name, root) in hierarchy.roots() {
            let subtree_ids = root.all_curve_ids();
            let total_expected = subtree_ids.len();
            let total_present = subtree_ids
                .iter()
                .filter(|id| present.contains(*id))
                .count();
            let percent = if total_expected == 0 {
                100.0
            } else {
                (total_present as f64 / total_expected as f64) * 100.0
            };
            coverage.push(SubtreeCoverage {
                path: vec![name.clone()],
                total_expected,
                total_present,
                percent,
            });
        }

        Some(CompletenessReport {
            missing,
            unclassified,
            coverage,
        })
    }

    /// Returns `true` if any credit index references the given curve ID
    /// as its hazard curve, base correlation curve, or issuer curve.
    fn curve_affects_credit_indices(&self, curve_id: &CurveId) -> bool {
        let id_str = curve_id.as_str();
        self.credit_indices.values().any(|data| {
            data.index_credit_curve.id().as_str() == id_str
                || data.base_correlation_curve.id().as_str() == id_str
                || data
                    .issuer_credit_curves
                    .as_ref()
                    .is_some_and(|curves| curves.values().any(|c| c.id().as_str() == id_str))
        })
    }

    /// Rebind all credit indices against the curves currently in the context
    /// (mutable).
    ///
    /// Credit indices hold direct references to their hazard / base
    /// correlation / issuer curves. After replacing any of those curves
    /// wholesale (e.g. a snapshot restore in P&L attribution), call this so
    /// the indices re-resolve against the new curve objects.
    ///
    /// Returns the IDs of any credit indices that were invalidated (removed)
    /// because their required curves are no longer present or changed type.
    pub fn rebind_credit_indices_mut(&mut self) -> Vec<CurveId> {
        self.rebind_all_credit_indices()
    }

    /// Rebind all credit indices to current curves.
    ///
    /// Returns the IDs of any credit indices that were invalidated (removed)
    /// because their required curves are no longer present or changed type.
    /// Callers can inspect this list to detect unexpected state transitions.
    pub(crate) fn rebind_all_credit_indices(&mut self) -> Vec<CurveId> {
        let mut rebuilt = HashMap::default();
        rebuilt.reserve(self.credit_indices.len());
        let mut invalidated = Vec::new();

        for (id, data) in self.credit_indices.iter() {
            match self.rebind_credit_index_data(data) {
                Ok(index) => {
                    rebuilt.insert(id.clone(), Arc::new(index));
                }
                Err(_err) => {
                    invalidated.push(id.clone());
                    tracing::warn!(
                        credit_index_id = id.as_str(),
                        error = %_err,
                        "dropping credit index after failed curve rebinding"
                    );
                }
            }
        }

        self.credit_indices = Arc::new(rebuilt);
        invalidated
    }
}

// -----------------------------------------------------------------------------
// Heterogeneous curve storage
// -----------------------------------------------------------------------------

macro_rules! for_each_context_curve {
    ($macro:ident) => {
        $macro! {
            Discount => { accessor: discount, is_accessor: is_discount, ty: DiscountCurve, type_name: "Discount" },
            Forward => { accessor: forward, is_accessor: is_forward, ty: ForwardCurve, type_name: "Forward" },
            Hazard => { accessor: hazard, is_accessor: is_hazard, ty: HazardCurve, type_name: "Hazard" },
            Inflation => { accessor: inflation, is_accessor: is_inflation, ty: InflationCurve, type_name: "Inflation" },
            BaseCorrelation => {
                accessor: base_correlation,
                is_accessor: is_base_correlation,
                ty: BaseCorrelationCurve,
                type_name: "BaseCorrelation"
            },
            Price => { accessor: price, is_accessor: is_price, ty: PriceCurve, type_name: "Price" },
            VolIndex => { accessor: vol_index, is_accessor: is_vol_index, ty: VolatilityIndexCurve, type_name: "VolIndex" },
            BasisSpread => { accessor: basis_spread, is_accessor: is_basis_spread, ty: BasisSpreadCurve, type_name: "BasisSpread" },
            Parametric => { accessor: parametric, is_accessor: is_parametric, ty: ParametricCurve, type_name: "Parametric" }
        }
    };
}
pub(crate) use for_each_context_curve;
// -----------------------------------------------------------------------------
// RebuildableWithId trait for preserving curve ID after bumping
// -----------------------------------------------------------------------------
/// Trait for curves that can be rebuilt with a new ID while preserving all other data.
///
/// This is used during market bumping operations where the bump produces a curve
/// with a modified ID (e.g., "USD-OIS_bump_+10bp") but we want to keep the original ID.
pub(crate) trait RebuildableWithId: Sized {
    /// Rebuild the curve with a new ID, preserving all other data.
    fn rebuild_with_id(&self, id: CurveId) -> Result<Self>;
}
macro_rules! impl_simple_rebuildable_with_id {
    ($($ty:ty),* $(,)?) => {
        $(
            impl RebuildableWithId for $ty {
                fn rebuild_with_id(&self, id: CurveId) -> Result<Self> {
                    self.to_builder_with_id(id).build()
                }
            }
        )*
    };
}
impl_simple_rebuildable_with_id!(
    DiscountCurve,
    ForwardCurve,
    HazardCurve,
    InflationCurve,
    VolatilityIndexCurve,
    PriceCurve,
    BasisSpreadCurve,
    ParametricCurve,
);
impl RebuildableWithId for BaseCorrelationCurve {
    fn rebuild_with_id(&self, id: CurveId) -> Result<Self> {
        BaseCorrelationCurve::builder(id)
            .knots(
                self.detachment_points()
                    .iter()
                    .copied()
                    .zip(self.correlations().iter().copied()),
            )
            .build()
    }
}
macro_rules! define_curve_storage {
    ($( $variant:ident => {
        accessor: $accessor:ident,
        is_accessor: $is_accessor:ident,
        ty: $ty:ident,
        type_name: $type_name:literal
    } ),* $(,)?) => {
        /// Unified storage for all curve types using an enum.
        ///
        /// Downstream code rarely manipulates [`CurveStorage`] directly; it mostly
        /// powers [`super::MarketContext`]'s heterogeneous map. When required, the helper
        /// methods expose the inner `Arc` for each concrete curve type.
        #[derive(Clone, Debug)]
        pub enum CurveStorage {
            $(
                #[doc = concat!($type_name, " curve")]
                $variant(Arc<$ty>),
            )*
        }
        impl CurveStorage {
            /// Return the curve's unique identifier.
            pub fn id(&self) -> &CurveId {
                match self {
                    $( Self::$variant(curve) => curve.id(), )*
                }
            }
            $(
                #[doc = concat!("Borrow the ", $type_name, " curve when the variant matches.")]
                pub fn $accessor(&self) -> Option<&Arc<$ty>> {
                    match self {
                        Self::$variant(curve) => Some(curve),
                        _ => None,
                    }
                }
                #[doc = concat!("Return `true` when this storage contains a ", $type_name, " curve.")]
                pub fn $is_accessor(&self) -> bool {
                    matches!(self, Self::$variant(_))
                }
            )*
            /// Return a human-readable curve type (useful for diagnostics/logging).
            pub fn curve_type(&self) -> &'static str {
                match self {
                    $( Self::$variant(_) => $type_name, )*
                }
            }
        }
        $(
            impl From<$ty> for CurveStorage {
                fn from(curve: $ty) -> Self {
                    Self::$variant(Arc::new(curve))
                }
            }
            impl From<Arc<$ty>> for CurveStorage {
                fn from(curve: Arc<$ty>) -> Self {
                    Self::$variant(curve)
                }
            }
        )*
    };
}
for_each_context_curve!(define_curve_storage);
impl CurveStorage {
    /// Roll this curve storage forward by the provided number of days.
    pub(crate) fn roll_forward_storage(&self, days: i64) -> Result<Self> {
        match self {
            Self::Discount(curve) => Ok(Self::Discount(Arc::new(curve.roll_forward(days)?))),
            Self::Forward(curve) => Ok(Self::Forward(Arc::new(curve.roll_forward(days)?))),
            Self::Hazard(curve) => Ok(Self::Hazard(Arc::new(curve.roll_forward(days)?))),
            Self::Inflation(curve) => Ok(Self::Inflation(Arc::new(curve.roll_forward(days)?))),
            Self::BaseCorrelation(curve) => Ok(Self::BaseCorrelation(Arc::clone(curve))),
            Self::Price(curve) => Ok(Self::Price(Arc::new(curve.roll_forward(days)?))),
            Self::VolIndex(curve) => Ok(Self::VolIndex(Arc::new(curve.roll_forward(days)?))),
            Self::BasisSpread(curve) => Ok(Self::BasisSpread(Arc::new(curve.roll_forward(days)?))),
            Self::Parametric(curve) => Ok(Self::Parametric(Arc::clone(curve))),
        }
    }
    /// Apply a bump to this curve storage, preserving the original ID.
    ///
    /// After bumping, if the bumped curve has a different ID (e.g., "USD-OIS_bump_+10bp"),
    /// it is rebuilt with the original ID to maintain context consistency.
    ///
    /// # Special Cases
    ///
    /// - `InflationCurve` with `TriangularKeyRate` bump: Custom point-level bumping
    ///   that modifies the CPI level at the target bucket.
    pub(crate) fn apply_bump_preserving_id(
        &mut self,
        original_id: &CurveId,
        spec: BumpSpec,
    ) -> Result<()> {
        fn bump_curve_preserving_id<C>(
            original: &C,
            original_id: &CurveId,
            spec: BumpSpec,
            id_of: fn(&C) -> &CurveId,
        ) -> Result<C>
        where
            C: Bumpable + RebuildableWithId,
        {
            let bumped = original.apply_bump(spec)?;
            if id_of(&bumped) != original_id {
                bumped.rebuild_with_id(original_id.clone())
            } else {
                Ok(bumped)
            }
        }
        match self {
            Self::Discount(arc) => {
                // In-place bump: Arc::make_mut deep-clones only if refcount > 1
                Arc::make_mut(arc).bump_in_place(&spec)?;
                Ok(())
            }
            Self::Forward(arc) => {
                Arc::make_mut(arc).bump_in_place(&spec)?;
                Ok(())
            }
            Self::Hazard(arc) => {
                Arc::make_mut(arc).bump_in_place(&spec)?;
                Ok(())
            }
            Self::Inflation(original) => {
                // Special handling for TriangularKeyRate bumps on InflationCurve
                if let BumpType::TriangularKeyRate {
                    prev_bucket,
                    target_bucket,
                    next_bucket,
                } = spec.bump_type
                {
                    let (delta, is_multiplicative) =
                        spec.resolve_standard_values().ok_or_else(|| {
                            crate::error::InputError::UnsupportedBump {
                                reason: "InflationCurve key-rate bump requires additive bump"
                                    .to_string(),
                            }
                        })?;
                    if is_multiplicative {
                        return Err(crate::error::InputError::UnsupportedBump {
                            reason:
                                "InflationCurve key-rate bump does not support multiplicative bumps"
                                    .to_string(),
                        }
                        .into());
                    }
                    let mut points: Vec<(f64, f64)> = original
                        .knots()
                        .iter()
                        .copied()
                        .zip(original.cpi_levels().iter().copied())
                        .collect();
                    for (tenor, level) in &mut points {
                        let weight = crate::market_data::term_structures::common::triangular_weight(
                            *tenor,
                            prev_bucket,
                            target_bucket,
                            next_bucket,
                        );
                        if weight > 0.0 {
                            *level *= 1.0 + delta * weight;
                        }
                    }
                    let rebuilt = InflationCurve::builder(original_id.clone())
                        .base_cpi(original.base_cpi())
                        .base_date(original.base_date())
                        .day_count(original.day_count())
                        .indexation_lag_months(original.indexation_lag_months())
                        .knots(points)
                        .interp(original.interp_style())
                        .build()?;
                    *self = Self::Inflation(Arc::new(rebuilt));
                    return Ok(());
                }
                let curve = bump_curve_preserving_id(
                    original.as_ref(),
                    original_id,
                    spec,
                    InflationCurve::id,
                )?;
                *self = Self::Inflation(Arc::new(curve));
                Ok(())
            }
            Self::BaseCorrelation(original) => {
                let curve = bump_curve_preserving_id(
                    original.as_ref(),
                    original_id,
                    spec,
                    BaseCorrelationCurve::id,
                )?;
                *self = Self::BaseCorrelation(Arc::new(curve));
                Ok(())
            }
            Self::VolIndex(original) => {
                let curve = bump_curve_preserving_id(
                    original.as_ref(),
                    original_id,
                    spec,
                    VolatilityIndexCurve::id,
                )?;
                *self = Self::VolIndex(Arc::new(curve));
                Ok(())
            }
            Self::Price(original) => {
                let curve =
                    bump_curve_preserving_id(original.as_ref(), original_id, spec, PriceCurve::id)?;
                *self = Self::Price(Arc::new(curve));
                Ok(())
            }
            Self::BasisSpread(_) => Err(crate::error::InputError::UnsupportedBump {
                reason: "BasisSpreadCurve does not support bumping".to_string(),
            }
            .into()),
            Self::Parametric(_) => Err(crate::error::InputError::UnsupportedBump {
                reason: "ParametricCurve does not support bumping".to_string(),
            }
            .into()),
        }
    }
}
#[cfg(test)]
mod curve_storage_tests {
    use super::*;
    use crate::dates::{Date, DayCount};
    use crate::math::interp::{ExtrapolationPolicy, InterpStyle};
    use serde_json::Value;
    use time::Month;

    fn test_date() -> Date {
        Date::from_calendar_date(2025, Month::January, 1).expect("valid test date")
    }
    fn json(curve: &impl serde::Serialize) -> Value {
        serde_json::to_value(curve).expect("curve should serialize")
    }
    #[test]
    fn forward_bump_preserves_interp_and_extrapolation() {
        let curve = ForwardCurve::builder("FWD", 0.25)
            .base_date(test_date())
            .reset_lag(0)
            .day_count(DayCount::Act365F)
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::FlatZero)
            .knots([(0.5, 0.02), (1.0, 0.025), (2.0, 0.03)])
            .build()
            .expect("curve builds");
        let original = json(&curve);
        let mut storage = CurveStorage::from(curve);
        storage
            .apply_bump_preserving_id(&CurveId::from("FWD"), BumpSpec::parallel_bp(1.0))
            .expect("bump succeeds");
        let bumped_curve = storage.forward().expect("forward curve");
        let bumped_json = json(bumped_curve.as_ref());
        assert_eq!(bumped_curve.interp_style(), InterpStyle::LogLinear);
        assert_eq!(bumped_json["reset_lag"], original["reset_lag"]);
        assert_eq!(bumped_json["day_count"], original["day_count"]);
        assert_eq!(bumped_json["interp_style"], original["interp_style"]);
        assert_eq!(bumped_json["extrapolation"], original["extrapolation"]);
    }
    #[test]
    fn inflation_bump_preserves_lag_day_count_and_interp() {
        let curve = InflationCurve::builder("CPI")
            .base_date(test_date())
            .base_cpi(300.0)
            .day_count(DayCount::Act360)
            .indexation_lag_months(2)
            .interp(InterpStyle::LogLinear)
            .knots([(0.0, 300.0), (5.0, 325.0), (10.0, 350.0)])
            .build()
            .expect("curve builds");
        let original = json(&curve);
        let mut storage = CurveStorage::from(curve);
        storage
            .apply_bump_preserving_id(&CurveId::from("CPI"), BumpSpec::inflation_shift_pct(1.0))
            .expect("bump succeeds");
        let bumped_curve = storage.inflation().expect("inflation curve");
        let bumped_json = json(bumped_curve.as_ref());
        assert_eq!(bumped_curve.day_count(), DayCount::Act360);
        assert_eq!(bumped_curve.indexation_lag_months(), 2);
        assert_eq!(bumped_curve.interp_style(), InterpStyle::LogLinear);
        assert_eq!(bumped_json["base_date"], original["base_date"]);
        assert_eq!(bumped_json["day_count"], original["day_count"]);
        assert_eq!(
            bumped_json["indexation_lag_months"],
            original["indexation_lag_months"]
        );
        assert_eq!(bumped_json["interp_style"], original["interp_style"]);
        assert_eq!(bumped_json["extrapolation"], original["extrapolation"]);
    }
    #[test]
    fn inflation_triangular_key_rate_bump_weights_neighboring_knots() {
        let curve = InflationCurve::builder("CPI")
            .base_date(test_date())
            .base_cpi(300.0)
            .day_count(DayCount::Act360)
            .indexation_lag_months(3)
            .interp(InterpStyle::Linear)
            .knots([
                (0.0, 300.0),
                (2.5, 306.0),
                (5.0, 312.0),
                (7.5, 318.0),
                (10.0, 324.0),
            ])
            .build()
            .expect("curve builds");
        let mut storage = CurveStorage::from(curve);
        storage
            .apply_bump_preserving_id(
                &CurveId::from("CPI"),
                BumpSpec::triangular_key_rate_bp(0.0, 5.0, 10.0, 100.0),
            )
            .expect("triangular bump succeeds");
        let bumped = storage.inflation().expect("inflation curve");
        let levels = bumped.cpi_levels();
        assert!(
            (levels[0] - 300.0).abs() < 1e-12,
            "left boundary should stay unchanged"
        );
        assert!(
            (levels[4] - 324.0).abs() < 1e-12,
            "right boundary should stay unchanged"
        );
        assert!(
            (levels[2] - 315.12).abs() < 1e-10,
            "target knot should receive the full bump"
        );
        assert!(
            (levels[1] - 307.53).abs() < 1e-10,
            "left neighbor should receive half the bump"
        );
        assert!(
            (levels[3] - 319.59).abs() < 1e-10,
            "right neighbor should receive half the bump"
        );
    }
    #[test]
    fn discount_bump_preserves_forward_controls() {
        let curve = DiscountCurve::builder("DISC")
            .base_date(test_date())
            .day_count(DayCount::Act365F)
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::FlatForward)
            .knots([(0.5, 1.0), (1.0, 1.001), (2.0, 1.002)])
            .validation(
                crate::market_data::term_structures::ValidationMode::NegativeRateFriendly {
                    forward_floor: -0.05,
                },
            )
            .min_forward_tenor(1e-8)
            .build()
            .expect("curve builds");
        let original = json(&curve);
        let mut storage = CurveStorage::from(curve);
        storage
            .apply_bump_preserving_id(&CurveId::from("DISC"), BumpSpec::parallel_bp(1.0))
            .expect("bump succeeds");
        let bumped_curve = storage.discount().expect("discount curve");
        let bumped_json = json(bumped_curve.as_ref());
        assert_eq!(bumped_curve.interp_style(), InterpStyle::Linear);
        assert_eq!(
            bumped_json["allow_non_monotonic"],
            original["allow_non_monotonic"]
        );
        assert_eq!(
            bumped_json["min_forward_rate"],
            original["min_forward_rate"]
        );
        assert_eq!(
            bumped_json["min_forward_tenor"],
            original["min_forward_tenor"]
        );
    }
}

// -----------------------------------------------------------------------------
// MarketContext typed getters
// -----------------------------------------------------------------------------

impl MarketContext {
    // -----------------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------------
    #[inline]
    fn not_found_error(id: &str) -> crate::Error {
        crate::error::InputError::NotFound { id: id.to_string() }.into()
    }
    fn missing_curve_error(&self, id: &str) -> crate::Error {
        // Sort for deterministic error messages (HashMap iteration order varies).
        let mut available: Vec<&str> = self.curves.keys().map(|k| k.as_str()).collect();
        available.sort_unstable();
        crate::error::Error::missing_curve_with_suggestions(id, &available)
    }
    #[inline]
    fn get_cloned<T>(&self, map: &HashMap<CurveId, Arc<T>>, id: &str) -> Result<Arc<T>> {
        map.get(id)
            .cloned()
            .ok_or_else(|| Self::not_found_error(id))
    }
    #[inline]
    fn get_ref<'a, T>(&self, map: &'a HashMap<CurveId, T>, id: &str) -> Result<&'a T> {
        map.get(id).ok_or_else(|| Self::not_found_error(id))
    }
    /// Helper method to extract curve with type checking and error handling
    fn get_curve_with_type_check<T, F>(
        &self,
        id: &str,
        expected_type: &'static str,
        extractor: F,
    ) -> Result<T>
    where
        F: FnOnce(&CurveStorage) -> Option<T>,
    {
        match self.curves.get(id) {
            Some(storage) => extractor(storage).ok_or_else(|| {
                crate::error::InputError::WrongCurveType {
                    id: id.to_string(),
                    expected: expected_type.to_string(),
                    actual: storage.curve_type().to_string(),
                }
                .into()
            }),
            None => Err(self.missing_curve_error(id)),
        }
    }
    // -----------------------------------------------------------------------------
    // Public API: typed getters
    // -----------------------------------------------------------------------------
    /// Get a discount curve by identifier.
    ///
    /// Returns an [`Arc`] clone of the stored curve so callers can retain the
    /// curve independently of the context. The identifier should match the
    /// curve's logical market-data key such as `"USD-OIS"`.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is missing or exists under a different
    /// curve type.
    pub fn get_discount(&self, id: impl AsRef<str>) -> Result<Arc<DiscountCurve>> {
        let id_str = id.as_ref();
        self.get_curve_with_type_check(id_str, "Discount", |storage| {
            storage.discount().map(Arc::clone)
        })
    }
    /// Get a forward curve by identifier.
    ///
    /// Forward-curve identifiers usually encode both the market and tenor, such
    /// as `"USD-SOFR3M"` or `"EUR-EURIBOR6M"`.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is missing or refers to a non-forward curve.
    pub fn get_forward(&self, id: impl AsRef<str>) -> Result<Arc<ForwardCurve>> {
        let id_str = id.as_ref();
        self.get_curve_with_type_check(id_str, "Forward", |storage| {
            storage.forward().map(Arc::clone)
        })
    }
    /// Get a hazard curve by identifier.
    ///
    /// Hazard curves are stored as annualized default intensities keyed by a
    /// credit-specific identifier.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is missing or refers to a different curve type.
    pub fn get_hazard(&self, id: impl AsRef<str>) -> Result<Arc<HazardCurve>> {
        let id_str = id.as_ref();
        self.get_curve_with_type_check(id_str, "Hazard", |storage| storage.hazard().map(Arc::clone))
    }
    /// Get an inflation curve by identifier.
    ///
    /// Inflation curves are typically keyed by an index family such as
    /// `"US-CPI"` or `"UK-RPI"` and represent CPI-linked term structures rather
    /// than spot index observations.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is missing or refers to a different curve type.
    pub fn get_inflation_curve(&self, id: impl AsRef<str>) -> Result<Arc<InflationCurve>> {
        let id_str = id.as_ref();
        self.get_curve_with_type_check(id_str, "Inflation", |storage| {
            storage.inflation().map(Arc::clone)
        })
    }
    /// Get a base correlation curve by identifier.
    ///
    /// Use this for tranche-style credit workflows where correlation is quoted
    /// by detachment point.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is missing or refers to a different curve type.
    pub fn get_base_correlation(&self, id: impl AsRef<str>) -> Result<Arc<BaseCorrelationCurve>> {
        let id_str = id.as_ref();
        self.get_curve_with_type_check(id_str, "BaseCorrelation", |storage| {
            storage.base_correlation().map(Arc::clone)
        })
    }
    /// Get a volatility index curve by identifier.
    ///
    /// These curves typically store forward levels for instruments such as VIX futures.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is missing or refers to a different curve type.
    pub fn get_vol_index_curve(&self, id: impl AsRef<str>) -> Result<Arc<VolatilityIndexCurve>> {
        let id_str = id.as_ref();
        self.get_curve_with_type_check(id_str, "VolIndex", |storage| {
            storage.vol_index().map(Arc::clone)
        })
    }
    /// Get a price curve by identifier.
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_quant_core::market_data::context::MarketContext;
    /// # use finstack_quant_core::market_data::term_structures::PriceCurve;
    /// # use finstack_quant_core::dates::Date;
    /// # use time::Month;
    /// # let base = Date::from_calendar_date(2024, Month::January, 1).expect("Valid date");
    /// # let curve = PriceCurve::builder("WTI-FORWARD")
    /// #     .base_date(base)
    /// #     .spot_price(75.0)
    /// #     .knots([(0.0, 75.0), (0.5, 77.0)])
    /// #     .build()
    /// #     .expect("PriceCurve builder should succeed");
    /// # let ctx = MarketContext::new().insert(curve);
    /// let price_curve = ctx.get_price_curve("WTI-FORWARD").expect("Price curve should exist");
    /// assert!(price_curve.price(0.25) > 0.0);
    /// ```
    pub fn get_price_curve(&self, id: impl AsRef<str>) -> Result<Arc<PriceCurve>> {
        let id_str = id.as_ref();
        self.get_curve_with_type_check(id_str, "Price", |storage| storage.price().map(Arc::clone))
    }
    /// Get a basis spread curve by identifier.
    ///
    /// Basis spread curves store cross-currency or multi-curve spread data.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is missing or refers to a different curve type.
    pub fn get_basis_spread(&self, id: impl AsRef<str>) -> Result<Arc<BasisSpreadCurve>> {
        let id_str = id.as_ref();
        self.get_curve_with_type_check(id_str, "BasisSpread", |storage| {
            storage.basis_spread().map(Arc::clone)
        })
    }
    /// Get a parametric curve by identifier.
    ///
    /// Parametric curves use Nelson-Siegel or Nelson-Siegel-Svensson models.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is missing or refers to a different curve type.
    pub fn get_parametric(&self, id: impl AsRef<str>) -> Result<Arc<ParametricCurve>> {
        let id_str = id.as_ref();
        self.get_curve_with_type_check(id_str, "Parametric", |storage| {
            storage.parametric().map(Arc::clone)
        })
    }
    /// Iterate over all stored discount curves without cloning the context.
    ///
    /// Yields `(curve_id, Arc<DiscountCurve>)` pairs in arbitrary order. Use this
    /// when you need to scan discount curves (e.g. for currency-prefix matching)
    /// in a hot path where materializing a
    /// [`MarketContextState`] would be wasteful.
    pub fn iter_discount_curves(
        &self,
    ) -> impl Iterator<Item = (&CurveId, Arc<DiscountCurve>)> + '_ {
        self.curves
            .iter()
            .filter_map(|(id, storage)| storage.discount().map(|c| (id, Arc::clone(c))))
    }
    /// Clone a volatility surface by identifier.
    ///
    /// Returns the strike-grid surface stored under `id`. Use
    /// [`Self::get_fx_delta_vol_surface`] for FX smiles quoted in delta space.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is not present.
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_quant_core::market_data::context::MarketContext;
    /// # use finstack_quant_core::market_data::surfaces::VolSurface;
    /// # let surface = VolSurface::builder("IR-Swaption")
    /// #     .expiries(&[1.0, 2.0])
    /// #     .strikes(&[90.0, 100.0])
    /// #     .row(&[0.2, 0.2])
    /// #     .row(&[0.2, 0.2])
    /// #     .build()
    /// #     .expect("... builder should succeed");
    /// # let ctx = MarketContext::new().insert_surface(surface);
    /// let surface = ctx.get_surface("IR-Swaption").expect("Surface should exist");
    /// assert!((surface.value_clamped(1.5, 95.0) - 0.2).abs() < 1e-12);
    /// ```
    pub fn get_surface(&self, id: impl AsRef<str>) -> Result<Arc<VolSurface>> {
        self.get_cloned(&self.surfaces, id.as_ref())
    }
    /// Borrow a market price/scalar by identifier.
    ///
    /// Scalars are borrowed rather than cloned because they are typically small
    /// immutable values already owned by the context.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is not present.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// use finstack_quant_core::market_data::scalars::MarketScalar;
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::currency::Currency;
    ///
    /// let ctx = MarketContext::new()
    ///     .insert_price("AAPL", MarketScalar::Price(Money::new(180.0, Currency::USD)));
    /// if let MarketScalar::Price(price) = ctx.get_price("AAPL").expect("Price should exist") {
    ///     assert_eq!(price.currency(), Currency::USD);
    /// }
    /// ```
    pub fn get_price(&self, id: impl AsRef<str>) -> Result<&MarketScalar> {
        self.get_ref(&self.prices, id.as_ref())
    }
    /// Borrow a scalar time series by identifier.
    ///
    /// Time series remain borrowed so callers can inspect them without copying
    /// the stored observation history.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is not present.
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_quant_core::market_data::context::MarketContext;
    /// # use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
    /// # use finstack_quant_core::dates::Date;
    /// # use time::Month;
    /// # let series = ScalarTimeSeries::new(
    /// #     "VOL-TS",
    /// #     vec![
    /// #         (Date::from_calendar_date(2024, Month::January, 1).expect("Valid date"), 0.2),
    /// #         (Date::from_calendar_date(2024, Month::February, 1).expect("Valid date"), 0.25),
    /// #     ],
    /// #     None,
    /// # ).expect("... creation should succeed");
    /// # let ctx = MarketContext::new().insert_series(series);
    /// let series = ctx.get_series("VOL-TS").expect("Series should exist");
    /// assert_eq!(series.id().as_str(), "VOL-TS");
    /// ```
    pub fn get_series(&self, id: impl AsRef<str>) -> Result<&ScalarTimeSeries> {
        self.get_ref(&self.series, id.as_ref())
    }
    /// Clone an inflation index by identifier.
    ///
    /// Inflation indices represent historical CPI-style observations, not
    /// forward-looking inflation curves.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is not present.
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_quant_core::market_data::context::MarketContext;
    /// # use finstack_quant_core::market_data::scalars::{InflationIndex, InflationInterpolation};
    /// # use finstack_quant_core::currency::Currency;
    /// # use finstack_quant_core::dates::Date;
    /// # use time::Month;
    /// # let observations = vec![
    /// #     (Date::from_calendar_date(2024, Month::January, 31).expect("Valid date"), 100.0),
    /// #     (Date::from_calendar_date(2024, Month::February, 29).expect("Valid date"), 101.0),
    /// # ];
    /// # let index = InflationIndex::new("US-CPI", observations, Currency::USD)
    /// #     .expect("... creation should succeed")
    /// #     .with_interpolation(InflationInterpolation::Linear);
    /// # let ctx = MarketContext::new().insert_inflation_index("US-CPI", index);
    /// let idx = ctx.get_inflation_index("US-CPI").expect("Inflation index should exist");
    /// assert_eq!(idx.id, "US-CPI");
    /// ```
    pub fn get_inflation_index(&self, id: impl AsRef<str>) -> Result<Arc<InflationIndex>> {
        self.get_cloned(&self.inflation_indices, id.as_ref())
    }
    /// Clone an FX delta-quoted volatility surface by identifier.
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_quant_core::market_data::context::MarketContext;
    /// # use finstack_quant_core::market_data::surfaces::FxDeltaVolSurface;
    /// # let surface = FxDeltaVolSurface::new(
    /// #     "EURUSD-DELTA-VOL",
    /// #     vec![0.25, 0.5, 1.0],
    /// #     vec![0.08, 0.085, 0.09],
    /// #     vec![0.01, 0.012, 0.015],
    /// #     vec![0.005, 0.006, 0.007],
    /// # ).expect("surface should build");
    /// # let ctx = MarketContext::new().insert_fx_delta_vol_surface(surface);
    /// let surf = ctx.get_fx_delta_vol_surface("EURUSD-DELTA-VOL")
    ///     .expect("surface should exist");
    /// assert_eq!(surf.id().as_str(), "EURUSD-DELTA-VOL");
    /// ```
    pub fn get_fx_delta_vol_surface(&self, id: impl AsRef<str>) -> Result<Arc<FxDeltaVolSurface>> {
        self.get_cloned(&self.fx_delta_vol_surfaces, id.as_ref())
    }
    /// Clone a dividend schedule by identifier.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is not present.
    pub fn get_dividend_schedule(&self, id: impl AsRef<str>) -> Result<Arc<DividendSchedule>> {
        self.get_cloned(&self.dividends, id.as_ref())
    }
    /// Clone a credit index aggregate by identifier.
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_quant_core::market_data::context::MarketContext;
    /// # use finstack_quant_core::market_data::term_structures::{BaseCorrelationCurve, CreditIndexData, HazardCurve};
    /// # use finstack_quant_core::dates::Date;
    /// # use std::sync::Arc;
    /// # use time::Month;
    /// # let hazard = Arc::new(HazardCurve::builder("CDX")
    /// #     .base_date(Date::from_calendar_date(2024, Month::January, 1).expect("Valid date"))
    /// #     .knots([(0.0, 0.01), (5.0, 0.015)])
    /// #     .build()
    /// #     .expect("... creation should succeed"));
    /// # let base_corr = Arc::new(BaseCorrelationCurve::builder("CDX")
    /// #     .knots([(3.0, 0.25), (10.0, 0.55)])
    /// #     .build()
    /// #     .expect("... creation should succeed"));
    /// # let data = CreditIndexData::builder()
    /// #     .num_constituents(125)
    /// #     .recovery_rate(0.4)
    /// #     .index_credit_curve(Arc::clone(&hazard))
    /// #     .base_correlation_curve(base_corr)
    /// #     .build()
    /// #     .expect("... builder should succeed");
    /// # let ctx = MarketContext::new().insert_credit_index("CDX-IG", data);
    /// let idx = ctx.get_credit_index("CDX-IG").expect("Credit index should exist");
    /// assert_eq!(idx.num_constituents, 125);
    /// ```
    pub fn get_credit_index(&self, id: impl AsRef<str>) -> Result<Arc<CreditIndexData>> {
        self.get_cloned(&self.credit_indices, id.as_ref())
    }
    /// Clone a SABR volatility cube by identifier.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is not present.
    pub fn get_vol_cube(&self, id: impl AsRef<str>) -> Result<Arc<VolCube>> {
        self.get_cloned(&self.vol_cubes, id.as_ref())
    }
    /// Look up a vol provider by identifier.
    ///
    /// Checks vol cubes first, then falls back to vol surfaces. This enables
    /// pricing code to accept either a 3D cube or a 2D surface through the
    /// [`VolProvider`](crate::market_data::traits::VolProvider) trait.
    ///
    /// # Errors
    ///
    /// Returns an error if neither a vol cube nor a vol surface exists under the
    /// given identifier.
    pub fn get_vol_provider(
        &self,
        id: impl AsRef<str>,
    ) -> Result<Arc<dyn crate::market_data::traits::VolProvider>> {
        let id_str = id.as_ref();
        if let Some(cube) = self.vol_cubes.get(id_str) {
            return Ok(Arc::clone(cube) as Arc<dyn crate::market_data::traits::VolProvider>);
        }
        if let Some(surface) = self.surfaces.get(id_str) {
            return Ok(Arc::clone(surface) as Arc<dyn crate::market_data::traits::VolProvider>);
        }
        Err(Self::not_found_error(id_str))
    }
    /// Resolve a collateral discount curve for a CSA code.
    ///
    /// This performs the indirection from a CSA or collateral agreement code to
    /// the discount curve configured for that collateral set. The returned trait
    /// object exposes the generic [`Discounting`] interface rather than the
    /// concrete [`DiscountCurve`] type.
    ///
    /// # Errors
    ///
    /// Returns an error if the CSA code is not mapped or if the mapped curve is missing.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// use finstack_quant_core::market_data::term_structures::DiscountCurve;
    /// use finstack_quant_core::math::interp::InterpStyle;
    /// use finstack_quant_core::dates::Date;
    /// use finstack_quant_core::types::CurveId;
    /// use time::Month;
    ///
    /// let curve = DiscountCurve::builder("USD-OIS")
    ///     .base_date(Date::from_calendar_date(2024, Month::January, 1).expect("Valid date"))
    ///     .knots([(0.0, 1.0), (1.0, 0.99)])
    ///     .build()
    ///     .expect("... builder should succeed");
    /// let ctx = MarketContext::new()
    ///     .insert(curve)
    ///     .map_collateral("USD-CSA", CurveId::from("USD-OIS"));
    /// let discount = ctx.get_collateral("USD-CSA").expect("Collateral curve should exist");
    /// assert!(discount.df(0.5) <= 1.0);
    /// ```
    pub fn get_collateral(&self, csa_code: &str) -> Result<Arc<dyn Discounting + Send + Sync>> {
        let curve_id = self
            .collateral
            .get(csa_code)
            .ok_or(crate::error::InputError::NotFound {
                id: format!("collateral:{}", csa_code),
            })?;
        self.get_discount(curve_id.as_str())
            .map(|arc| arc as Arc<dyn Discounting + Send + Sync>)
    }
    // -----------------------------------------------------------------------------
    // Update methods for special cases
    // -----------------------------------------------------------------------------
    /// Update only the base correlation curve for a credit index.
    ///
    /// Handy for calibration loops that tweak base correlation while leaving
    /// other index data intact. Returns `false` if the index identifier cannot
    /// be found.
    pub fn update_base_correlation_curve(
        &mut self,
        id: impl AsRef<str>,
        new_curve: Arc<BaseCorrelationCurve>,
    ) -> bool {
        let cid = CurveId::from(id.as_ref());
        // Get the existing index data
        let Some(existing_index) = self.credit_indices.get(&cid) else {
            return false;
        };
        let curve_id = new_curve.id().to_owned();
        Arc::make_mut(&mut self.curves).insert(
            curve_id,
            CurveStorage::BaseCorrelation(Arc::clone(&new_curve)),
        );
        let mut updated_index = (**existing_index).clone();
        updated_index.base_correlation_curve = new_curve;
        // Update the context
        Arc::make_mut(&mut self.credit_indices).insert(cid, Arc::new(updated_index));
        let _invalidated = self.rebind_all_credit_indices();
        true
    }
}

// -----------------------------------------------------------------------------
// MarketContext insert and mutation APIs
// -----------------------------------------------------------------------------

impl MarketContext {
    // -----------------------------------------------------------------------------
    // Insert methods (canonical: builder-by-value)
    // -----------------------------------------------------------------------------
    /// Insert a generic curve storage entry.
    ///
    /// This is primarily intended for downstream crates that operate on heterogeneous
    /// curve types (e.g., calibration pipelines) and want to update the context
    /// without matching on concrete curve variants.
    pub fn insert<C>(mut self, curve: C) -> Self
    where
        C: Into<CurveStorage>,
    {
        let curve: CurveStorage = curve.into();
        let id = curve.id().to_owned();
        Arc::make_mut(&mut self.curves).insert(id, curve);
        if !self.credit_indices.is_empty() {
            let _invalidated = self.rebind_all_credit_indices();
        }
        self
    }
    /// Insert a volatility surface.
    ///
    /// Accepts either an owned [`VolSurface`] or an `Arc<VolSurface>`.
    /// When passing an owned value, it will be wrapped in an `Arc` automatically.
    /// When passing an `Arc`, it is used directly (enabling surface sharing between contexts).
    ///
    /// # Parameters
    /// - `surface`: a [`VolSurface`] or `Arc<VolSurface>`
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_quant_core::market_data::context::MarketContext;
    /// # use finstack_quant_core::market_data::surfaces::VolSurface;
    /// # use std::sync::Arc;
    /// # let surface = VolSurface::builder("IR-Swaption")
    /// #     .expiries(&[1.0, 2.0])
    /// #     .strikes(&[90.0, 100.0])
    /// #     .row(&[0.2, 0.2])
    /// #     .row(&[0.2, 0.2])
    /// #     .build()
    /// #     .expect("... builder should succeed");
    /// // Owned value (wrapped in Arc automatically)
    /// let ctx = MarketContext::new().insert_surface(surface);
    /// assert_eq!(ctx.stats().surface_count, 1);
    ///
    /// // Pre-wrapped Arc (for sharing across contexts)
    /// # let surface2 = VolSurface::builder("EQ-Vol")
    /// #     .expiries(&[1.0, 2.0])
    /// #     .strikes(&[90.0, 100.0])
    /// #     .row(&[0.2, 0.2])
    /// #     .row(&[0.2, 0.2])
    /// #     .build()
    /// #     .expect("... builder should succeed");
    /// let shared = Arc::new(surface2);
    /// let ctx2 = MarketContext::new().insert_surface(Arc::clone(&shared));
    /// ```
    pub fn insert_surface(mut self, surface: impl Into<Arc<VolSurface>>) -> Self {
        let arc_surface = surface.into();
        let id = arc_surface.id().to_owned();
        Arc::make_mut(&mut self.surfaces).insert(id, arc_surface);
        self
    }
    /// Insert an FX delta-quoted volatility surface.
    ///
    /// Accepts either an owned [`FxDeltaVolSurface`] or an `Arc<FxDeltaVolSurface>`.
    /// When passing an owned value, it will be wrapped in an `Arc` automatically.
    /// When passing an `Arc`, it is used directly (enabling surface sharing between contexts).
    ///
    /// # Parameters
    /// - `surface`: a [`FxDeltaVolSurface`] or `Arc<FxDeltaVolSurface>`
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_quant_core::market_data::context::MarketContext;
    /// # use finstack_quant_core::market_data::surfaces::FxDeltaVolSurface;
    /// let surface = FxDeltaVolSurface::new(
    ///     "EURUSD-DELTA-VOL",
    ///     vec![0.25, 0.5, 1.0],
    ///     vec![0.08, 0.085, 0.09],
    ///     vec![0.01, 0.012, 0.015],
    ///     vec![0.005, 0.006, 0.007],
    /// ).expect("surface should build");
    /// let ctx = MarketContext::new().insert_fx_delta_vol_surface(surface);
    /// assert!(ctx.get_fx_delta_vol_surface("EURUSD-DELTA-VOL").is_ok());
    /// ```
    pub fn insert_fx_delta_vol_surface(
        mut self,
        surface: impl Into<Arc<FxDeltaVolSurface>>,
    ) -> Self {
        let arc_surface = surface.into();
        let id = arc_surface.id().to_owned();
        Arc::make_mut(&mut self.fx_delta_vol_surfaces).insert(id, arc_surface);
        self
    }
    /// Insert a SABR volatility cube.
    ///
    /// Accepts either an owned [`VolCube`] or an `Arc<VolCube>`.
    /// When passing an owned value, it will be wrapped in an `Arc` automatically.
    /// When passing an `Arc`, it is used directly (enabling cube sharing between contexts).
    ///
    /// # Parameters
    /// - `cube`: a [`VolCube`] or `Arc<VolCube>`
    pub fn insert_vol_cube(mut self, cube: impl Into<Arc<VolCube>>) -> Self {
        let arc = cube.into();
        let id = arc.id().to_owned();
        Arc::make_mut(&mut self.vol_cubes).insert(id, arc);
        self
    }
    /// Insert a dividend schedule.
    ///
    /// Accepts either an owned [`DividendSchedule`] or an `Arc<DividendSchedule>`.
    /// When passing an owned value, it will be wrapped in an `Arc` automatically.
    /// When passing an `Arc`, it is used directly (enabling schedule sharing between contexts).
    ///
    /// # Parameters
    /// - `schedule`: a [`DividendSchedule`] or `Arc<DividendSchedule>` built via its builder
    pub fn insert_dividends(mut self, schedule: impl Into<Arc<DividendSchedule>>) -> Self {
        let arc_schedule = schedule.into();
        let id = arc_schedule.id.to_owned();
        Arc::make_mut(&mut self.dividends).insert(id, arc_schedule);
        self
    }
    /// Insert a market scalar/price.
    ///
    /// # Parameters
    /// - `id`: identifier (string-like) stored as [`CurveId`]
    /// - `price`: scalar value to store
    pub fn insert_price(mut self, id: impl AsRef<str>, price: MarketScalar) -> Self {
        Arc::make_mut(&mut self.prices).insert(CurveId::from(id.as_ref()), price);
        self
    }
    /// Insert a scalar time series.
    ///
    /// # Parameters
    /// - `series`: [`ScalarTimeSeries`] to store
    pub fn insert_series(mut self, series: ScalarTimeSeries) -> Self {
        let id = series.id().to_owned();
        Arc::make_mut(&mut self.series).insert(id, series);
        self
    }
    /// Insert an inflation index.
    ///
    /// Accepts either an owned [`InflationIndex`] or an `Arc<InflationIndex>`.
    /// When passing an owned value, it will be wrapped in an `Arc` automatically.
    /// When passing an `Arc`, it is used directly (enabling index sharing between contexts).
    ///
    /// # Parameters
    /// - `id`: identifier stored as [`CurveId`]
    /// - `index`: an [`InflationIndex`] or `Arc<InflationIndex>`
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// use finstack_quant_core::market_data::scalars::{InflationIndex, InflationInterpolation};
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::Date;
    /// use std::sync::Arc;
    /// use time::Month;
    ///
    /// let observations = vec![
    ///     (Date::from_calendar_date(2024, Month::January, 31).expect("Valid date"), 100.0),
    ///     (Date::from_calendar_date(2024, Month::February, 29).expect("Valid date"), 101.0),
    /// ];
    /// let index = InflationIndex::new("US-CPI", observations, Currency::USD)
    ///     .expect("InflationIndex creation should succeed")
    ///     .with_interpolation(InflationInterpolation::Linear);
    /// let ctx = MarketContext::new().insert_inflation_index("US-CPI", index);
    /// assert!(ctx.get_inflation_index("US-CPI").is_ok());
    ///
    /// // With Arc for sharing
    /// # let observations2 = vec![
    /// #     (Date::from_calendar_date(2024, Month::January, 31).expect("Valid date"), 100.0),
    /// #     (Date::from_calendar_date(2024, Month::February, 29).expect("Valid date"), 101.0),
    /// # ];
    /// # let index2 = InflationIndex::new("EU-HICP", observations2, Currency::EUR)
    /// #     .expect("InflationIndex creation should succeed");
    /// let shared = Arc::new(index2);
    /// let ctx2 = MarketContext::new().insert_inflation_index("EU-HICP", Arc::clone(&shared));
    /// ```
    pub fn insert_inflation_index(
        mut self,
        id: impl AsRef<str>,
        index: impl Into<Arc<InflationIndex>>,
    ) -> Self {
        let index = index.into();
        let key = Self::inflation_index_key_for_insert(id, index.as_ref());
        Arc::make_mut(&mut self.inflation_indices).insert(key, index);
        self
    }
    /// Insert a credit index aggregate.
    ///
    /// # Parameters
    /// - `id`: identifier stored as [`CurveId`]
    /// - `data`: [`CreditIndexData`] bundle
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// use finstack_quant_core::market_data::term_structures::{BaseCorrelationCurve, CreditIndexData, HazardCurve};
    /// use finstack_quant_core::dates::Date;
    /// use std::sync::Arc;
    /// use time::Month;
    ///
    /// let hazard = Arc::new(HazardCurve::builder("CDX")
    ///     .base_date(Date::from_calendar_date(2024, Month::January, 1).expect("Valid date"))
    ///     .knots([(0.0, 0.01), (5.0, 0.015)])
    ///     .build()
    ///     .expect("HazardCurve builder should succeed"));
    /// let base_corr = Arc::new(BaseCorrelationCurve::builder("CDX")
    ///     .knots([(3.0, 0.25), (10.0, 0.55)])
    ///     .build()
    ///     .expect("BaseCorrelationCurve builder should succeed"));
    /// let data = CreditIndexData::builder()
    ///     .num_constituents(125)
    ///     .recovery_rate(0.4)
    ///     .index_credit_curve(Arc::clone(&hazard))
    ///     .base_correlation_curve(base_corr)
    ///     .build()
    ///     .expect("CreditIndexData builder should succeed");
    /// let ctx = MarketContext::new().insert_credit_index("CDX-IG", data);
    /// assert!(ctx.get_credit_index("CDX-IG").is_ok());
    /// ```
    pub fn insert_credit_index(mut self, id: impl AsRef<str>, data: CreditIndexData) -> Self {
        let key = CurveId::from(id.as_ref());
        Arc::make_mut(&mut self.credit_indices).insert(key, Arc::new(data));
        self
    }
    /// Insert an FX matrix.
    ///
    /// Accepts either an owned [`FxMatrix`] or an `Arc<FxMatrix>`.
    /// When passing an owned value, it will be wrapped in an `Arc` automatically.
    /// When passing an `Arc`, it is used directly (enabling FX matrix sharing between contexts).
    ///
    /// # Parameters
    /// - `fx`: [`FxMatrix`] or `Arc<FxMatrix>` instance used for currency conversions
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// use finstack_quant_core::money::fx::{FxMatrix, FxProvider, FxConversionPolicy};
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::Date;
    /// use std::sync::Arc;
    /// use time::Month;
    ///
    /// struct StaticFx;
    /// impl FxProvider for StaticFx {
    ///     fn rate(
    ///         &self,
    ///         _from: Currency,
    ///         _to: Currency,
    ///         _on: Date,
    ///         _policy: FxConversionPolicy,
    ///     ) -> finstack_quant_core::Result<f64> {
    ///         Ok(1.1)
    ///     }
    /// }
    ///
    /// // Owned value
    /// let fx = FxMatrix::new(Arc::new(StaticFx));
    /// let ctx = MarketContext::new().insert_fx(fx);
    /// assert!(ctx.fx().is_some());
    ///
    /// // Pre-wrapped Arc for sharing
    /// # struct StaticFx2;
    /// # impl FxProvider for StaticFx2 {
    /// #     fn rate(&self, _from: Currency, _to: Currency, _on: Date, _policy: FxConversionPolicy) -> finstack_quant_core::Result<f64> { Ok(1.2) }
    /// # }
    /// let shared_fx = Arc::new(FxMatrix::new(Arc::new(StaticFx2)));
    /// let ctx2 = MarketContext::new().insert_fx(Arc::clone(&shared_fx));
    /// ```
    pub fn insert_fx(mut self, fx: impl Into<Arc<FxMatrix>>) -> Self {
        self.fx = Some(fx.into());
        self
    }
    /// Clear the FX matrix from this context.
    ///
    /// After calling this method, `ctx.fx()` will return `None`.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// use finstack_quant_core::money::fx::{FxMatrix, FxProvider, FxConversionPolicy};
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::Date;
    /// use std::sync::Arc;
    ///
    /// struct StaticFx;
    /// impl FxProvider for StaticFx {
    ///     fn rate(&self, _: Currency, _: Currency, _: Date, _: FxConversionPolicy) -> finstack_quant_core::Result<f64> { Ok(1.0) }
    /// }
    ///
    /// let fx = FxMatrix::new(Arc::new(StaticFx));
    /// let ctx = MarketContext::new().insert_fx(fx);
    /// assert!(ctx.fx().is_some());
    ///
    /// let ctx = ctx.clear_fx();
    /// assert!(ctx.fx().is_none());
    /// ```
    pub fn clear_fx(mut self) -> Self {
        self.fx = None;
        self
    }
    /// Map collateral CSA code to a discount curve identifier.
    ///
    /// # Parameters
    /// - `csa_code`: CSA identifier (e.g., "USD-CSA")
    /// - `discount_id`: target discount curve [`CurveId`]
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// use finstack_quant_core::market_data::term_structures::DiscountCurve;
    /// use finstack_quant_core::dates::Date;
    /// use finstack_quant_core::types::CurveId;
    /// use time::Month;
    ///
    /// let curve = DiscountCurve::builder("USD-OIS")
    ///     .base_date(Date::from_calendar_date(2024, Month::January, 1).expect("Valid date"))
    ///     .knots([(0.0, 1.0), (1.0, 0.99)])
    ///     .build()
    ///     .expect("... builder should succeed");
    /// let ctx = MarketContext::new()
    ///     .insert(curve)
    ///     .map_collateral("USD-CSA", CurveId::from("USD-OIS"));
    /// assert!(ctx.get_collateral("USD-CSA").is_ok());
    /// ```
    pub fn map_collateral(mut self, csa_code: impl Into<String>, discount_id: CurveId) -> Self {
        Arc::make_mut(&mut self.collateral).insert(csa_code.into(), discount_id);
        self
    }
    // -----------------------------------------------------------------------------
    // Insert methods (mutable variants for binding layers)
    //
    // These `&mut self` variants mirror the consuming `insert_*` methods above but
    // mutate in place. They exist primarily so that Python/WASM binding wrappers
    // can avoid the `self.inner = std::mem::take(&mut self.inner).insert(..)` dance
    // that is required to bridge a fluent builder API across an FFI boundary.
    //
    // The behaviour is identical to the fluent variants — same storage layout,
    // same credit-index rebinding — just with `&mut self` instead of `mut self`.
    // -----------------------------------------------------------------------------
    /// Insert a generic curve storage entry, mutating in place.
    ///
    /// Mirrors [`Self::insert`] but takes `&mut self`.
    pub fn insert_mut<C>(&mut self, curve: C) -> &mut Self
    where
        C: Into<CurveStorage>,
    {
        let curve: CurveStorage = curve.into();
        let id = curve.id().to_owned();
        Arc::make_mut(&mut self.curves).insert(id, curve);
        if !self.credit_indices.is_empty() {
            let _invalidated = self.rebind_all_credit_indices();
        }
        self
    }
    /// Insert a volatility surface, mutating in place.
    ///
    /// Mirrors [`Self::insert_surface`] but takes `&mut self`.
    pub fn insert_surface_mut(&mut self, surface: impl Into<Arc<VolSurface>>) -> &mut Self {
        let arc_surface = surface.into();
        let id = arc_surface.id().to_owned();
        Arc::make_mut(&mut self.surfaces).insert(id, arc_surface);
        self
    }
    /// Insert an FX delta-quoted volatility surface, mutating in place.
    ///
    /// Mirrors [`Self::insert_fx_delta_vol_surface`] but takes `&mut self`.
    pub fn insert_fx_delta_vol_surface_mut(
        &mut self,
        surface: impl Into<Arc<FxDeltaVolSurface>>,
    ) -> &mut Self {
        let arc_surface = surface.into();
        let id = arc_surface.id().to_owned();
        Arc::make_mut(&mut self.fx_delta_vol_surfaces).insert(id, arc_surface);
        self
    }
    /// Insert a SABR volatility cube, mutating in place.
    ///
    /// Mirrors [`Self::insert_vol_cube`] but takes `&mut self`.
    pub fn insert_vol_cube_mut(&mut self, cube: impl Into<Arc<VolCube>>) -> &mut Self {
        let arc = cube.into();
        let id = arc.id().to_owned();
        Arc::make_mut(&mut self.vol_cubes).insert(id, arc);
        self
    }
    /// Insert a dividend schedule, mutating in place.
    ///
    /// Mirrors [`Self::insert_dividends`] but takes `&mut self`.
    pub fn insert_dividends_mut(
        &mut self,
        schedule: impl Into<Arc<DividendSchedule>>,
    ) -> &mut Self {
        let arc_schedule = schedule.into();
        let id = arc_schedule.id.to_owned();
        Arc::make_mut(&mut self.dividends).insert(id, arc_schedule);
        self
    }
    /// Insert a market scalar/price, mutating in place.
    ///
    /// Mirrors [`Self::insert_price`] but takes `&mut self`.
    pub fn insert_price_mut(&mut self, id: impl AsRef<str>, price: MarketScalar) -> &mut Self {
        Arc::make_mut(&mut self.prices).insert(CurveId::from(id.as_ref()), price);
        self
    }
    /// Insert a scalar time series, mutating in place.
    ///
    /// Mirrors [`Self::insert_series`] but takes `&mut self`.
    pub fn insert_series_mut(&mut self, series: ScalarTimeSeries) -> &mut Self {
        let id = series.id().to_owned();
        Arc::make_mut(&mut self.series).insert(id, series);
        self
    }
    /// Insert an inflation index, mutating in place.
    ///
    /// Mirrors [`Self::insert_inflation_index`] but takes `&mut self`.
    pub fn insert_inflation_index_mut(
        &mut self,
        id: impl AsRef<str>,
        index: impl Into<Arc<InflationIndex>>,
    ) -> &mut Self {
        let index = index.into();
        let key = Self::inflation_index_key_for_insert(id, index.as_ref());
        Arc::make_mut(&mut self.inflation_indices).insert(key, index);
        self
    }
    /// Insert a credit index aggregate, mutating in place.
    ///
    /// Mirrors [`Self::insert_credit_index`] but takes `&mut self`.
    pub fn insert_credit_index_mut(
        &mut self,
        id: impl AsRef<str>,
        data: CreditIndexData,
    ) -> &mut Self {
        let key = CurveId::from(id.as_ref());
        Arc::make_mut(&mut self.credit_indices).insert(key, Arc::new(data));
        self
    }
    /// Insert an FX matrix, mutating in place.
    ///
    /// Mirrors [`Self::insert_fx`] but takes `&mut self`.
    pub fn insert_fx_mut(&mut self, fx: impl Into<Arc<FxMatrix>>) -> &mut Self {
        self.fx = Some(fx.into());
        self
    }
    /// Clear the FX matrix, mutating in place.
    ///
    /// Mirrors [`Self::clear_fx`] but takes `&mut self`.
    pub fn clear_fx_mut(&mut self) -> &mut Self {
        self.fx = None;
        self
    }
    /// Map collateral CSA code to a discount curve identifier, mutating in place.
    ///
    /// Mirrors [`Self::map_collateral`] but takes `&mut self`.
    pub fn map_collateral_mut(
        &mut self,
        csa_code: impl Into<String>,
        discount_id: CurveId,
    ) -> &mut Self {
        Arc::make_mut(&mut self.collateral).insert(csa_code.into(), discount_id);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::MarketContext;
    use crate::currency::Currency;
    use crate::error::InputError;
    use crate::money::fx::{FxMatrix, SimpleFxProvider};
    use crate::money::Money;
    use std::sync::Arc;
    use time::macros::date;

    #[test]
    fn market_context_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MarketContext>();
    }

    #[test]
    fn fx_required_errors_when_matrix_absent() {
        let ctx = MarketContext::new();
        // `FxMatrix` does not impl Debug, so we can't use `unwrap_err` directly.
        let Err(err) = ctx.fx_required() else {
            panic!("expected fx_required to error on empty context");
        };
        assert!(
            matches!(
                err,
                crate::Error::Input(InputError::NotFound { ref id }) if id == "fx_matrix"
            ),
            "unexpected error: {err:?}",
        );
    }

    #[test]
    fn convert_money_same_currency_is_identity() {
        let ctx = MarketContext::new();
        let amount = Money::new(1_000.0, Currency::USD);
        let out = ctx
            .convert_money(amount, Currency::USD, date!(2025 - 01 - 15))
            .expect("same-ccy conversion should not consult FX");
        assert_eq!(out, amount);
    }

    #[test]
    fn convert_money_uses_fx_matrix_rate() {
        let provider = SimpleFxProvider::new();
        provider
            .set_quote(Currency::EUR, Currency::USD, 1.1)
            .expect("valid rate");
        let ctx = MarketContext::new().insert_fx(FxMatrix::new(Arc::new(provider)));
        let eur = Money::new(1_000.0, Currency::EUR);
        let usd = ctx
            .convert_money(eur, Currency::USD, date!(2025 - 01 - 15))
            .expect("EUR->USD should succeed");
        assert_eq!(usd.currency(), Currency::USD);
        assert!((usd.amount() - 1_100.0).abs() < 1e-9);
    }

    #[test]
    fn convert_money_preserves_decimal_amount_precision() {
        use core::str::FromStr;

        let provider = SimpleFxProvider::new();
        provider
            .set_quote(Currency::EUR, Currency::USD, 2.0)
            .expect("valid rate");
        let ctx = MarketContext::new().insert_fx(FxMatrix::new(Arc::new(provider)));
        let amount = rust_decimal::Decimal::from_str("10000000000000000.1").expect("valid decimal");
        let eur = Money::from_decimal(amount, Currency::EUR).expect("representable amount");

        let usd = ctx
            .convert_money(eur, Currency::USD, date!(2025 - 01 - 15))
            .expect("EUR->USD should succeed");
        assert_eq!(
            usd.amount_decimal(),
            rust_decimal::Decimal::from_str("20000000000000000.2").expect("valid decimal")
        );
    }

    #[test]
    fn convert_money_missing_matrix_returns_not_found() {
        let ctx = MarketContext::new();
        let eur = Money::new(1_000.0, Currency::EUR);
        let err = ctx
            .convert_money(eur, Currency::USD, date!(2025 - 01 - 15))
            .unwrap_err();
        assert!(
            matches!(
                err,
                crate::Error::Input(InputError::NotFound { ref id }) if id == "fx_matrix"
            ),
            "unexpected error: {err:?}",
        );
    }
}
