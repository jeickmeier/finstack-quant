//! Core [`MarketContext`] container, scratch-bump state, and identity tracking.

use super::CurveStorage;
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

use super::super::{
    dividends::DividendSchedule,
    hierarchy::MarketDataHierarchy,
    scalars::{InflationIndex, MarketScalar, ScalarTimeSeries},
    surfaces::{FxDeltaVolSurface, VolCube, VolSurface},
    term_structures::CreditIndexData,
};

static NEXT_CONTEXT_ID: AtomicU64 = AtomicU64::new(1);

pub(super) fn next_context_id() -> u64 {
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

pub(in super::super) enum ScratchBumpState {
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
    pub(super) curves: Arc<FxHashMap<CurveId, CurveStorage>>,

    /// Foreign-exchange matrix
    pub(super) fx: Option<Arc<FxMatrix>>,

    /// Volatility surfaces
    pub(super) surfaces: Arc<FxHashMap<CurveId, Arc<VolSurface>>>,

    /// Market scalars and prices
    pub(super) prices: Arc<FxHashMap<CurveId, MarketScalar>>,

    /// Generic time series
    pub(super) series: Arc<FxHashMap<CurveId, ScalarTimeSeries>>,

    /// Inflation indices
    pub(super) inflation_indices: Arc<FxHashMap<CurveId, Arc<InflationIndex>>>,

    /// Credit index aggregates
    pub(super) credit_indices: Arc<FxHashMap<CurveId, Arc<CreditIndexData>>>,

    /// Shared dividend schedules keyed by `CurveId` (e.g., "AAPL-DIVS")
    pub(super) dividends: Arc<FxHashMap<CurveId, Arc<DividendSchedule>>>,

    /// FX delta-quoted volatility surfaces
    pub(super) fx_delta_vol_surfaces: Arc<FxHashMap<CurveId, Arc<FxDeltaVolSurface>>>,

    /// SABR volatility cubes (expiry x tenor x strike)
    pub(super) vol_cubes: Arc<FxHashMap<CurveId, Arc<VolCube>>>,

    /// Optional market data hierarchy for organizational grouping.
    pub(super) hierarchy: Option<MarketDataHierarchy>,

    /// Unique identifier for scratch-token ownership verification.
    pub(super) context_id: u64,

    /// Monotonic token counter for in-place scratch bumps.
    pub(super) next_scratch_token: u64,

    /// LIFO stack of active scratch bump token IDs on this context instance.
    pub(super) active_scratch_tokens: Vec<u64>,
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
    pub(super) fn curve_affects_credit_indices(&self, curve_id: &CurveId) -> bool {
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
