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
//!     .day_count(finstack_quant_core::dates::DayCount::Act365F)
//!     .knots([(0.0, 1.0), (1.0, 0.98)])
//!     .interp(InterpStyle::Linear)
//!     .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
//!     .build()
//!     .expect("DiscountCurve builder should succeed");
//!
//! let ctx = MarketContext::new().try_insert(curve).expect("insert market curve");
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

#[cfg(test)]
#[path = "../../../tests/performance/context_bump_work.rs"]
mod context_bump_work;

pub use curve_storage::CurveStorage;
pub(crate) use curve_storage::for_each_context_curve;
pub use stats::ContextStats;

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use crate::FxHashMap;
use crate::InputError;
use crate::currency::Currency;
use crate::dates::Date;
use crate::money::Money;
use crate::money::fx::{FxConversionPolicy, FxMatrix, FxQuery};
use crate::types::CurveId;

use super::{
    dividends::DividendSchedule,
    hierarchy::MarketDataHierarchy,
    scalars::{InflationIndex, MarketScalar, ScalarTimeSeries},
    surfaces::{FxDeltaVolSurface, VolCube, VolSurface},
    term_structures::CreditIndexData,
};

static NEXT_CONTEXT_ID: AtomicU64 = AtomicU64::new(1);

fn next_context_id() -> u64 {
    NEXT_CONTEXT_ID.fetch_add(1, Ordering::Relaxed)
}

/// Outcome of [`MarketContext::convert_money`]: the converted amount and the
/// FX rate it used, so callers can stamp the applied policy (including whether
/// the rate was triangulated). `rate` is `None` when no conversion was needed.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConvertedMoney {
    /// The amount in the target currency.
    pub amount: Money,
    /// The rate lookup behind the conversion; `None` for a same-currency input.
    pub rate: Option<crate::money::fx::FxRateResult>,
}

/// Reversible in-place bump token for scratch market-context workflows.
///
/// This is intended for hot-path finite-difference calculations that need to
/// apply a small number of bumps to a reusable scratch `MarketContext` and then
/// restore the previous state without cloning the full context.
pub struct ContextScratchBump {
    pub(super) context_id: u64,
    pub(super) token_id: u64,
    pub(super) state: ScratchBumpState,
}

impl std::fmt::Debug for ContextScratchBump {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContextScratchBump")
            .field("context_id", &self.context_id)
            .field("token_id", &self.token_id)
            .finish()
    }
}

pub(super) enum ScratchBumpState {
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
        previous_credit_indices: Option<Arc<FxHashMap<CurveId, Arc<CreditIndexData>>>>,
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

pub use state_serde::{CreditIndexState, CurveState, MarketContextState};

/// Unified market data context with enum-based storage.
///
/// The context is constructed fluently (each `insert_*` returns a new context)
/// and is cheap to clone thanks to pervasive `Arc` usage. Typical workflows
/// construct a base context at scenario initialisation and reuse it across
/// pricing engines.
pub struct MarketContext {
    // Each map is wrapped in `Arc` so cloning the context (e.g. per finite-
    // difference greek bump) is a refcount bump rather than re-allocating and
    // rehashing every map. Mutators use `Arc::make_mut`, so a bump that touches
    // one map copies only that map (copy-on-write) and leaves the rest shared.
    /// All curves stored in unified enum-based map
    curves: Arc<FxHashMap<CurveId, CurveStorage>>,

    /// Foreign-exchange matrix
    fx: Option<Arc<FxMatrix>>,

    /// Volatility surfaces
    surfaces: Arc<FxHashMap<CurveId, Arc<VolSurface>>>,

    /// Market scalars and prices
    prices: Arc<FxHashMap<CurveId, MarketScalar>>,

    /// Generic time series
    series: Arc<FxHashMap<CurveId, ScalarTimeSeries>>,

    /// Inflation indices
    inflation_indices: Arc<FxHashMap<CurveId, Arc<InflationIndex>>>,

    /// Credit index aggregates
    credit_indices: Arc<FxHashMap<CurveId, Arc<CreditIndexData>>>,

    /// Shared dividend schedules keyed by `CurveId` (e.g., "AAPL-DIVS")
    dividends: Arc<FxHashMap<CurveId, Arc<DividendSchedule>>>,

    /// FX delta-quoted volatility surfaces
    fx_delta_vol_surfaces: Arc<FxHashMap<CurveId, Arc<FxDeltaVolSurface>>>,

    /// SABR volatility cubes (expiry x tenor x strike)
    vol_cubes: Arc<FxHashMap<CurveId, Arc<VolCube>>>,

    /// Optional market data hierarchy for organizational grouping.
    hierarchy: Option<MarketDataHierarchy>,

    /// Unique identifier for scratch-token ownership verification.
    context_id: u64,

    /// Monotonic token counter for in-place scratch bumps.
    next_scratch_token: u64,

    /// LIFO stack of active scratch bump token IDs on this context instance.
    active_scratch_tokens: Vec<u64>,
}

impl Default for MarketContext {
    fn default() -> Self {
        Self {
            curves: Arc::default(),
            fx: None,
            surfaces: Arc::default(),
            prices: Arc::default(),
            series: Arc::default(),
            inflation_indices: Arc::default(),
            credit_indices: Arc::default(),
            dividends: Arc::default(),
            fx_delta_vol_surfaces: Arc::default(),
            vol_cubes: Arc::default(),
            hierarchy: None,
            context_id: next_context_id(),
            next_scratch_token: 0,
            active_scratch_tokens: Vec::new(),
        }
    }
}

impl Clone for MarketContext {
    fn clone(&self) -> Self {
        Self {
            curves: Arc::clone(&self.curves),
            fx: self.fx.clone(),
            surfaces: Arc::clone(&self.surfaces),
            prices: Arc::clone(&self.prices),
            series: Arc::clone(&self.series),
            inflation_indices: Arc::clone(&self.inflation_indices),
            credit_indices: Arc::clone(&self.credit_indices),
            dividends: Arc::clone(&self.dividends),
            fx_delta_vol_surfaces: Arc::clone(&self.fx_delta_vol_surfaces),
            vol_cubes: Arc::clone(&self.vol_cubes),
            hierarchy: self.hierarchy.clone(),
            context_id: next_context_id(),
            next_scratch_token: 0,
            active_scratch_tokens: Vec::new(),
        }
    }
}

impl std::fmt::Debug for MarketContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MarketContext")
            .field("stats", &self.stats())
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
    /// Returns the input unchanged (with no rate) when it is already denominated
    /// in `target_currency`.
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
    /// * `policy` - Point-date FX provider-selection policy, either
    ///   [`FxConversionPolicy::CashflowDate`] or [`FxConversionPolicy::PeriodEnd`].
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
        policy: FxConversionPolicy,
    ) -> crate::Result<ConvertedMoney> {
        if amount.currency() == target_currency {
            return Ok(ConvertedMoney { amount, rate: None });
        }
        let fx = self.fx_required()?;
        let rate = fx.rate(FxQuery::new(
            amount.currency(),
            target_currency,
            as_of,
            policy,
        ))?;
        Ok(ConvertedMoney {
            amount: amount.convert_at_rate_f64(target_currency, rate.rate)?,
            rate: Some(rate),
        })
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
        self.invalidate_scratch_tokens();
        self.hierarchy = Some(h);
    }

    pub(super) fn invalidate_scratch_tokens(&mut self) {
        self.active_scratch_tokens.clear();
    }

    pub(super) fn allocate_scratch_token(&mut self) -> (u64, u64) {
        let token_id = self.next_scratch_token;
        self.next_scratch_token = self.next_scratch_token.wrapping_add(1);
        self.active_scratch_tokens.push(token_id);
        (self.context_id, token_id)
    }

    pub(super) fn pop_scratch_token(
        &mut self,
        context_id: u64,
        token_id: u64,
    ) -> crate::Result<()> {
        if context_id != self.context_id {
            return Err(crate::Error::Validation(format!(
                "Cannot revert scratch bump token {token_id} from context {context_id} on context {}",
                self.context_id
            )));
        }
        if self.active_scratch_tokens.last().copied() != Some(token_id) {
            return Err(crate::Error::Validation(format!(
                "Cannot revert scratch bump token {token_id} on context {}: token is stale, already reverted, or out of LIFO order",
                self.context_id
            )));
        }
        self.active_scratch_tokens.pop();
        Ok(())
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
}

impl MarketContext {
    pub(crate) fn unsupported_vol_surface_bump_error(
        &self,
        id: &str,
        operation: &str,
    ) -> Option<crate::Error> {
        if self.surfaces.contains_key(id) {
            return None;
        }
        if self.vol_cubes.contains_key(id) {
            return Some(crate::Error::Validation(format!(
                "Cannot apply {operation} to '{id}': registered as VolCube, which does not support VolSurface grid bumps"
            )));
        }
        if self.fx_delta_vol_surfaces.contains_key(id) {
            return Some(crate::Error::Validation(format!(
                "Cannot apply {operation} to '{id}': registered as FxDeltaVolSurface, which does not support VolSurface grid bumps"
            )));
        }
        None
    }

    /// Rebind all credit indices to current curves atomically.
    ///
    /// Returns an error without mutating `self.credit_indices` if any credit
    /// index fails to rebind against the updated curves.
    pub(crate) fn rebind_all_credit_indices(&mut self) -> crate::Result<()> {
        if self.credit_indices.is_empty() {
            return Ok(());
        }
        let mut rebuilt = FxHashMap::default();
        rebuilt.reserve(self.credit_indices.len());

        for (id, data) in self.credit_indices.iter() {
            let index = self.rebind_credit_index_data(data).map_err(|err| {
                crate::Error::Validation(format!("Failed to rebind credit index '{id}': {err}"))
            })?;
            rebuilt.insert(id.clone(), Arc::new(index));
        }

        self.credit_indices = Arc::new(rebuilt);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::MarketContext;
    use crate::InputError;
    use crate::currency::Currency;
    use crate::money::Money;
    use crate::money::fx::{FxConversionPolicy, FxMatrix, SimpleFxProvider};
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
            .convert_money(
                amount,
                Currency::USD,
                date!(2025 - 01 - 15),
                FxConversionPolicy::CashflowDate,
            )
            .expect("same-ccy conversion should not consult FX")
            .amount;
        assert_eq!(out, amount);
    }

    #[test]
    fn convert_money_uses_fx_matrix_rate() {
        let provider = SimpleFxProvider::new();
        provider
            .set_quotes(&[(Currency::EUR, Currency::USD, 1.1)])
            .expect("valid rate");
        let ctx = MarketContext::new().insert_fx(FxMatrix::new(Arc::new(provider), None));
        let eur = Money::from((1_000_i64, Currency::EUR));
        let usd = ctx
            .convert_money(
                eur,
                Currency::USD,
                date!(2025 - 01 - 15),
                FxConversionPolicy::CashflowDate,
            )
            .expect("EUR->USD should succeed")
            .amount;
        assert_eq!(usd.currency(), Currency::USD);
        assert!((usd.amount() - 1_100.0).abs() < 1e-9);
    }

    #[test]
    fn convert_money_preserves_decimal_amount_precision() {
        use core::str::FromStr;

        let provider = SimpleFxProvider::new();
        provider
            .set_quotes(&[(Currency::EUR, Currency::USD, 2.0)])
            .expect("valid rate");
        let ctx = MarketContext::new().insert_fx(FxMatrix::new(Arc::new(provider), None));
        let amount = rust_decimal::Decimal::from_str("10000000000000000.1").expect("valid decimal");
        let eur = Money::from_decimal(amount, Currency::EUR);

        let usd = ctx
            .convert_money(
                eur,
                Currency::USD,
                date!(2025 - 01 - 15),
                FxConversionPolicy::CashflowDate,
            )
            .expect("EUR->USD should succeed")
            .amount;
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
            .convert_money(
                eur,
                Currency::USD,
                date!(2025 - 01 - 15),
                FxConversionPolicy::CashflowDate,
            )
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
