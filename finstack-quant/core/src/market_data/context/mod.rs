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

mod curve_storage;
mod getters;
mod insert;
mod ops_bump;
mod ops_roll;
mod state_serde;
mod stats;

pub(crate) use curve_storage::for_each_context_curve;
pub use curve_storage::CurveStorage;
pub use stats::ContextStats;

pub use super::bumps::{BumpMode, BumpSpec, BumpUnits};

use std::sync::Arc;

use crate::collections::HashMap;
use crate::currency::Currency;
use crate::dates::Date;
use crate::error::InputError;
use crate::money::fx::{FxMatrix, FxQuery};
use crate::money::Money;
use crate::types::CurveId;

use super::{
    dividends::DividendSchedule,
    hierarchy::MarketDataHierarchy,
    scalars::{InflationIndex, MarketScalar, ScalarTimeSeries},
    surfaces::{FxDeltaVolSurface, VolCube, VolSurface},
    term_structures::CreditIndexData,
};

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
    /// One bumped volatility-surface grid point.
    SurfacePoint {
        /// Surface identifier that was bumped.
        id: CurveId,
        /// Expiry coordinate used to select the nearest grid point.
        expiry: f64,
        /// Strike coordinate used to select the nearest grid point.
        strike: f64,
        /// Original volatility restored when the token is reverted.
        original_vol: f64,
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
    MARKET_CONTEXT_STATE_CONTRACT, MARKET_CONTEXT_STATE_VERSION,
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

    /// Convert a [`Money`] amount into `target_currency` using the embedded FX matrix.
    ///
    /// Centralizes the "same-ccy shortcut → FX matrix lookup → rate application"
    /// pattern used across valuations, portfolio, margin, and cashflow aggregation.
    /// Returns the input unchanged when it is already denominated in `target_currency`.
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
    /// * `target_currency` - Destination currency.
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
        target_currency: Currency,
        as_of: Date,
    ) -> crate::Result<Money> {
        if amount.currency() == target_currency {
            return Ok(amount);
        }
        let fx = self.fx_required()?;
        let rate = fx.rate(FxQuery::new(amount.currency(), target_currency, as_of))?;
        amount.convert_at_rate(target_currency, rate.rate)
    }

    /// Snapshot (clone) all stored volatility surfaces.
    ///
    /// This clones the `Arc` handles (cheap) without exposing internal storage.
    #[inline]
    pub fn surfaces_snapshot(&self) -> HashMap<CurveId, Arc<VolSurface>> {
        self.surfaces.as_ref().clone()
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
        let amount = Money::from((1_000_i64, Currency::USD));
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
        let eur = Money::from((1_000_i64, Currency::EUR));
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
        let eur = Money::from((1_000_i64, Currency::EUR));
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
