//! FX quote storage, triangulation, caching, and provider-backed conversion.
//!
use lru::LruCache;
use std::cell::Cell;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::{FxHashMap, recover_lock};

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    recover_lock(mutex.lock())
}

// Non-cryptographic keys (currency pairs, query keys) use the workspace-standard
// FxHash rather than std's SipHash for ~2x faster lookups on the FX hot path.
use crate::currency::Currency;
use crate::dates::Date;

use super::provider::{
    FxProvider, FxProviderSnapshot, directional_rate, validate_fx_rate, validate_stored_quote,
};
use super::types::{
    CurrencyPair, DEFAULT_CACHE_CAPACITY, FxConfig, FxConversionPolicy, FxMatrixState, FxQuery,
    FxRateResult,
};

const MAX_STATE_ATTEMPTS: usize = 3;

/// Provider-observed quote stored with its provenance, so the `triangulated`
/// flag stamped on [`FxRateResult`] does not depend on cache state/call history.
#[derive(Clone, Copy, Debug)]
struct ObservedQuote {
    rate: f64,
    triangulated: bool,
    revision: ObservationRevision,
}

/// Revisions of both quote sources on which a cached observation depends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ObservationRevision {
    matrix: u64,
    provider: u64,
}

/// Whether `error` is the provider's "no rate for this ordered query" answer,
/// the only failure after which a pivot route is attempted.
fn is_not_found(error: &crate::Error) -> bool {
    matches!(
        error,
        crate::Error::Input(crate::InputError::NotFound { .. })
    )
}

/// Simplified FX matrix that stores quotes and computes cross rates on demand.
///
/// Note: `FxMatrix` cannot be directly serialized due to the trait object
/// `Arc<dyn FxProvider>`. To persist FX state, use
/// [`FxMatrix::get_serializable_state`] to extract the config and quotes, then
/// recreate the matrix with [`FxMatrix::try_from_state`].
///
/// # Thread Safety
///
/// Uses interior `Mutex` values for rate caching. Under high concurrency, cache
/// lookups serialize through those locks. For performance-critical parallel
/// pricing, consider pre-fetching rates or using one `FxMatrix` per thread.
pub struct FxMatrix {
    provider: Arc<dyn FxProvider>,
    /// Explicit, pair-global quotes inserted by callers or restored from
    /// serialized state. These are authoritative constant rates (e.g. pegged
    /// currencies), so the store is a plain `FxHashMap` that **never evicts** — a
    /// seeded peg must never be silently dropped under cache pressure and then
    /// re-derived from the provider (which would be a silent mispricing).
    quotes: Mutex<FxHashMap<CurrencyPair, f64>>,
    /// Query-sensitive quotes observed from providers or triangulation. This is
    /// the only genuinely bounded cache (governed by `config.cache_capacity`).
    observed_quotes: Mutex<
        LruCache<FxQuery, ObservedQuote, std::hash::BuildHasherDefault<rustc_hash::FxHasher>>,
    >,
    /// Authoritative date/policy-scoped quotes pinned via [`FxMatrix::set_quote_on`].
    /// Unlike `observed_quotes`, this map never evicts, so a pinned fixing is
    /// never silently replaced by the provider under cache pressure.
    pinned_quotes: Mutex<FxHashMap<FxQuery, f64>>,
    config: FxConfig,
    quote_revision: AtomicU64,
    /// Serializes writers so the revision-exhaustion check and the advance
    /// cover one mutation at a time.
    writer: Mutex<()>,
}

impl FxMatrix {
    /// Create a new [`FxMatrix`] wrapping `provider` with an explicit routing
    /// choice (record CORE-CONVENTIONS-013 #5).
    ///
    /// Retains 256 provider-observed quotes; use [`Self::try_with_config`]
    /// for another capacity. Explicit quotes and pinned fixings start empty.
    ///
    /// # Arguments
    ///
    /// * `provider` - FX quote source consulted when no explicit or pinned quote
    ///   answers the pair. Date and policy behavior come from this provider.
    /// * `pivot_currency` - Single cross-routing pivot, or `None` for direct
    ///   quotes only. There is no default pivot.
    ///
    /// # Examples
    /// ```rust
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
    ///         Ok(1.0)
    ///     }
    /// }
    ///
    /// let matrix = FxMatrix::new(Arc::new(StaticFx), None);
    /// assert_eq!(matrix.config().pivot_currency, None);
    /// // A computed provider cannot be captured as quotes.
    /// assert!(matrix.get_serializable_state().is_err());
    /// ```
    pub fn new(provider: Arc<dyn FxProvider>, pivot_currency: Option<Currency>) -> Self {
        Self::with_capacity(
            provider,
            FxConfig::new(pivot_currency),
            DEFAULT_CACHE_CAPACITY,
        )
    }

    /// Build a matrix over an empty [`SimpleFxProvider`](super::SimpleFxProvider) holding `quotes` as
    /// pair-global quotes.
    ///
    /// This is [`FxMatrix::new`] over an empty provider followed by
    /// [`set_quotes`](Self::set_quotes), the construction both host bindings
    /// expose.
    ///
    /// # Arguments
    ///
    /// * `quotes` - `(base, quote, rate)` triples, where `rate` is the finite,
    ///   strictly positive number of `quote` units per one `base` unit.
    /// * `pivot_currency` - Single cross-routing pivot, or `None` for direct
    ///   quotes only.
    ///
    /// # Errors
    ///
    /// Returns an error if any rate is non-finite or not strictly positive.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::fx::FxMatrix;
    ///
    /// let matrix = FxMatrix::from_quotes(&[(Currency::EUR, Currency::USD, 1.1)], None)?;
    /// assert_eq!(matrix.get_serializable_state()?.quotes.len(), 1);
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn from_quotes(
        quotes: &[(Currency, Currency, f64)],
        pivot_currency: Option<Currency>,
    ) -> crate::Result<Self> {
        let matrix = Self::new(Arc::new(super::SimpleFxProvider::new()), pivot_currency);
        matrix.set_quotes(quotes)?;
        Ok(matrix)
    }

    /// Create a new [`FxMatrix`] with custom configuration, failing closed on invalid inputs.
    ///
    /// The provider remains the source of date-aware quotes. `cache_capacity`
    /// bounds only provider-observed and triangulated results; explicitly set
    /// pair-global quotes and date/policy-pinned fixings never evict.
    ///
    /// # Errors
    ///
    /// Returns a validation error when `config.cache_capacity` is zero. No
    /// provider lookup is performed during construction, so provider failures
    /// surface only on a later [`rate`](Self::rate) request.
    ///
    /// # Arguments
    ///
    /// * `provider` - FX quote source consulted when no explicit or pinned quote answers the pair.
    /// * `config` - Explicit routing pivot (or `None`) and observed-quote cache capacity. Capacity must be greater than zero.
    pub fn try_with_config(provider: Arc<dyn FxProvider>, config: FxConfig) -> crate::Result<Self> {
        let Some(capacity) = NonZeroUsize::new(config.cache_capacity) else {
            return Err(crate::Error::Validation(
                "FxConfig.cache_capacity must be > 0".to_string(),
            ));
        };
        Ok(Self::with_capacity(provider, config, capacity))
    }

    fn with_capacity(
        provider: Arc<dyn FxProvider>,
        config: FxConfig,
        capacity: NonZeroUsize,
    ) -> Self {
        // Grow storage with observed quotes instead of preallocating the configured limit.
        let mut observed_quotes = LruCache::unbounded_with_hasher(Default::default());
        observed_quotes.resize(capacity);
        Self {
            provider,
            quotes: Mutex::new(FxHashMap::default()),
            observed_quotes: Mutex::new(observed_quotes),
            pinned_quotes: Mutex::new(FxHashMap::default()),
            config,
            quote_revision: AtomicU64::new(0),
            writer: Mutex::new(()),
        }
    }

    /// Return the matrix configuration.
    pub fn config(&self) -> FxConfig {
        self.config
    }

    /// Look up an FX rate (with metadata) using caching and triangulation fallbacks.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::money::fx::{FxMatrix, FxProvider, FxConversionPolicy, FxQuery};
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
    /// let matrix = FxMatrix::new(Arc::new(StaticFx), None);
    /// let query = FxQuery::new(
    ///     Currency::EUR,
    ///     Currency::USD,
    ///     Date::from_calendar_date(2024, Month::March, 1).expect("Valid date"),
    ///     FxConversionPolicy::CashflowDate,
    /// );
    /// let result = matrix.rate(query).expect("FX rate lookup should succeed");
    /// assert!(result.rate > 1.0);
    /// ```
    ///
    /// Lookup precedence (record CORE-CONVENTIONS-013 #3) is: the matrix's
    /// exact date/policy-pinned quote, then its pair-global quote, then the
    /// provider response for this exact ordered query (served from the
    /// observation cache while the matrix and provider revisions are
    /// unchanged). Within each matrix store the supplied direction wins,
    /// otherwise the checked reciprocal of the opposite quote; a higher store's
    /// reciprocal outranks a lower store's direct quote. Providers without a
    /// revision are queried each time.
    ///
    /// Only when the provider answers this exact query with
    /// [`InputError::NotFound`](crate::InputError::NotFound) and the matrix
    /// has a pivot currency does it derive `from -> pivot -> to` with the same
    /// date, policy and precedence for each leg (no recursive routing). The
    /// returned `triangulated` flag records that route, also on cached repeats.
    ///
    /// # Errors
    ///
    /// - The provider's own error for this query when it is not `NotFound`, or
    ///   when no pivot is configured.
    /// - [`InputError::FxTriangulationFailed`](crate::InputError::FxTriangulationFailed)
    ///   naming the ordered leg the provider reported missing; any other leg
    ///   failure propagates with its own error category.
    /// - An invalid FX rate when a direct, reciprocal, provider or crossed rate
    ///   is non-finite or non-positive.
    /// - A validation error when the route needs a second call to a provider
    ///   without a revision (the calls could see different quote states), or
    ///   when repeated updates prevent a coherent result.
    ///
    /// Identity conversion returns exactly `1.0` without querying the provider.
    ///
    /// # Arguments
    ///
    /// * `query` - Currencies, valuation date, and conversion policy; the policy
    ///   scopes pinned and observed quotes.
    pub fn rate(&self, query: FxQuery) -> crate::Result<FxRateResult> {
        if query.from == query.to {
            return Ok(FxRateResult {
                rate: 1.0,
                triangulated: false,
            });
        }
        for _ in 0..MAX_STATE_ATTEMPTS {
            let revision = self.state_revision();
            let result = self.rate_once(query);
            if revision == self.state_revision() {
                return result;
            }
        }
        Err(crate::Error::Validation(
            "FX quote state changed repeatedly during rate resolution".into(),
        ))
    }

    fn rate_once(&self, query: FxQuery) -> crate::Result<FxRateResult> {
        if let Some(cached) = self.read_cached_rate(query)? {
            return Ok(cached);
        }

        let provider_calls = Cell::new(0);
        match self.fetch_from_provider(query, &provider_calls) {
            Ok(rate) => Ok(FxRateResult {
                rate,
                triangulated: false,
            }),
            Err(error) => match self.config.pivot_currency {
                Some(pivot) if is_not_found(&error) => Ok(FxRateResult {
                    rate: self.triangulate_rate(query, pivot, &provider_calls)?,
                    triangulated: true,
                }),
                _ => Err(error),
            },
        }
    }

    /// Seed a quote scoped to a specific date and policy.
    ///
    /// Unlike [`set_quotes`](Self::set_quotes), this keys the quote by
    /// `(from, to, on, policy)`, so it only answers queries for that date and
    /// policy and does **not** shadow the provider across an entire time
    /// series. Use it to pin individual fixings while letting the provider
    /// supply every other date.
    ///
    /// The quote is stored in a dedicated, **non-evicting** pinned-quote map
    /// (separate from the bounded provider-observed cache), so it is never
    /// silently dropped under cache pressure. For its `(on, policy)` it is the
    /// highest authority: it outranks a pair-global
    /// [`set_quotes`](Self::set_quotes) quote and the provider.
    ///
    /// # Errors
    ///
    /// Returns an error if `from == to`, `rate` is non-finite or not strictly
    /// positive, or the matrix revision is exhausted. The existing pinned
    /// fixing for this exact pair/date/policy key is unchanged on error;
    /// reciprocal lookup is derived on demand rather than stored.
    ///
    /// # Arguments
    ///
    /// * `from` - Base currency of this date-and-policy fixing; must differ from `to`.
    /// * `to` - Quote currency. `rate` is units of `to` per one unit of `from`.
    /// * `on` - Valuation date this fixing answers. Other dates still use the provider.
    /// * `policy` - Conversion policy this fixing answers. Other policies still use the provider.
    /// * `rate` - Finite, strictly positive outright quote for this `(from, to, on, policy)` key.
    pub fn set_quote_on(
        &self,
        from: Currency,
        to: Currency,
        on: Date,
        policy: FxConversionPolicy,
        rate: f64,
    ) -> crate::Result<()> {
        let rate = validate_stored_quote(from, to, rate)?;
        self.insert_pinned_quote(
            FxQuery {
                from,
                to,
                on,
                policy,
            },
            rate,
        )
    }

    /// Seed multiple quotes at once.
    ///
    /// # Arguments
    ///
    /// * `quotes` - `(from, to, rate)` triples with distinct currencies. Each rate is units of `to` per one unit of `from` and must be finite and strictly positive.
    ///
    /// This is an atomic batch with respect to the pair-global quote map: every
    /// input is validated before any replacement occurs. These quotes
    /// outrank the provider for every date and policy, rank below a pinned
    /// fixing for that fixing's date and policy, and do not expire under LRU
    /// pressure.
    ///
    /// # Errors
    ///
    /// Returns an error if any tuple is an identity pair, any rate is
    /// non-finite or not strictly positive, the batch repeats an ordered pair
    /// (opposite orientations are distinct), or the matrix revision is
    /// exhausted. In that case the matrix retains every pre-existing explicit
    /// quote and none of the batch is applied. Separate calls may replace an
    /// existing quote.
    pub fn set_quotes(&self, quotes: &[(Currency, Currency, f64)]) -> crate::Result<()> {
        reject_repeated_pairs(quotes)?;
        let writer = lock(&self.writer);
        self.ensure_revision_available()?;
        let mut map = lock(&self.quotes);
        for &(from, to, rate) in quotes {
            map.insert(
                CurrencyPair {
                    base: from,
                    quote: to,
                },
                rate,
            );
        }
        self.quote_revision.fetch_add(1, Ordering::AcqRel);
        drop(map);
        drop(writer);
        self.invalidate_observed_quotes();
        Ok(())
    }

    /// Extract serializable state from the matrix.
    ///
    /// Returns the configuration, the matrix's global and pinned quotes, and
    /// the provider's complete stored quote state from [`FxProvider::snapshot`].
    /// Capturing performs no live rate lookup and omits observation-cache
    /// entries and revisions. Every captured quote is validated (distinct
    /// currencies, finite positive rate, no repeated ordered key within a
    /// store) before the state is returned. Matrix and provider revisions are
    /// checked around the capture; updates during capture trigger up to three
    /// attempts.
    ///
    /// # Errors
    ///
    /// Returns a validation error when the provider reports no revision, does
    /// not support capture (its rates cannot be reproduced by a quote list),
    /// exposes an invalid or repeated quote, or when repeated updates prevent
    /// a coherent snapshot. There is no partial snapshot.
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
    /// # use std::sync::Arc;
    /// let matrix = FxMatrix::new(Arc::new(SimpleFxProvider::new()), None);
    /// let state = matrix.get_serializable_state().expect("stable FX state");
    /// assert!(state.quotes.is_empty());
    /// ```
    pub fn get_serializable_state(&self) -> crate::Result<FxMatrixState> {
        for _ in 0..MAX_STATE_ATTEMPTS {
            let revision = self.state_revision();
            if revision.1.is_none() {
                return Err(crate::Error::Validation(
                    "FX state capture requires a provider that reports a revision".into(),
                ));
            }
            let state = self.capture_state_once()?;
            if revision == self.state_revision() {
                return Ok(state);
            }
        }
        Err(crate::Error::Validation(
            "FX quote state changed repeatedly during snapshot capture".into(),
        ))
    }

    fn capture_state_once(&self) -> crate::Result<FxMatrixState> {
        let Some(FxProviderSnapshot {
            quotes: mut provider_quotes,
            pinned_quotes: mut provider_pinned_quotes,
        }) = self.provider.snapshot()?
        else {
            return Err(crate::Error::Validation(
                "FX provider does not support state capture; freeze its rates into a \
                 quote-backed provider before snapshotting"
                    .into(),
            ));
        };
        let mut quote_vec: Vec<(Currency, Currency, f64)> = lock(&self.quotes)
            .iter()
            .map(|(pair, &rate)| (pair.base, pair.quote, rate))
            .collect();
        let mut pinned_vec: Vec<_> = lock(&self.pinned_quotes)
            .iter()
            .map(|(key, &rate)| (key.from, key.to, key.on, key.policy, rate))
            .collect();

        // Deterministic snapshots: sort by ordered key, not by map order.
        quote_vec.sort_by_key(|quote| (quote.0, quote.1));
        provider_quotes.sort_by_key(|quote| (quote.0, quote.1));
        provider_pinned_quotes.sort_by_key(|q| (q.0, q.1, q.2, q.3 as u8));
        pinned_vec.sort_by_key(|q| (q.0, q.1, q.2, q.3 as u8));
        validate_captured_global(&quote_vec)?;
        validate_captured_global(&provider_quotes)?;
        validate_captured_pinned(&pinned_vec)?;
        validate_captured_pinned(&provider_pinned_quotes)?;
        Ok(FxMatrixState {
            config: self.config,
            quotes: quote_vec,
            provider_quotes,
            provider_pinned_quotes,
            pinned_quotes: pinned_vec,
        })
    }

    /// Build a fresh matrix from a serialized FX snapshot.
    ///
    /// The snapshot's provider quotes become a quote-only
    /// [`SimpleFxProvider`](super::SimpleFxProvider) and its global and pinned
    /// quotes fill the new matrix's own stores, so the restored matrix resolves
    /// rates exactly as the captured one did, with the captured routing choice
    /// and cache capacity. There is no merge into an existing matrix.
    ///
    /// # Arguments
    ///
    /// * `state` - Snapshot whose `config`, provider quotes, provider-pinned
    ///   quotes, global `quotes` and `pinned_quotes` are restored.
    ///
    /// # Errors
    ///
    /// Returns an error when the configuration is invalid, any quote is an
    /// identity pair or has a non-finite or non-positive rate, or a store
    /// repeats an ordered key (opposite orientations and different stores are
    /// distinct keys). Nothing is published on error.
    pub fn try_from_state(state: &FxMatrixState) -> crate::Result<Self> {
        let provider = super::SimpleFxProvider::new();
        provider.set_quotes(&state.provider_quotes)?;
        provider.set_snapshot_pinned_quotes(&state.provider_pinned_quotes)?;
        let matrix = Self::try_with_config(Arc::new(provider), state.config)?;
        matrix.set_quotes(&state.quotes)?;
        matrix.insert_pinned_batch(&state.pinned_quotes)?;
        Ok(matrix)
    }

    /// Create a frozen scenario matrix with one currency pair relatively
    /// shocked (record CORE-CONVENTIONS-013 #11-#12).
    ///
    /// The bump is fractional: `0.01` raises the `reference.from ->
    /// reference.to` rate by 1%. The matrix state is captured (so the provider
    /// must support [`FxProvider::snapshot`]) and the shock is applied to that
    /// frozen copy, so later changes to the original provider or matrix do not
    /// alter the scenario. The shock covers the pair on every date and policy:
    ///
    /// - every stored quote of the pair keeps its store and scope; `from ->
    ///   to` quotes are multiplied by `1 + bump` and `to -> from` quotes are
    ///   divided by it, so an independently supplied reverse rate stays
    ///   independent. Where a store holds only one orientation, the other is
    ///   written next to it from its own unshocked reciprocal;
    /// - where the pair is only reachable through the pivot, each orientation's
    ///   unshocked route result is shocked once and stored at provider
    ///   authority for the global scope and for every date/policy scope;
    /// - other quotes are unchanged, and crosses routed through a shocked leg
    ///   are recomputed from it.
    ///
    /// The result holds only quotes, so it can itself be captured and shocked
    /// again. Its shocked pivot crosses are stored quotes, so they report
    /// `triangulated == false`.
    ///
    /// # Arguments
    ///
    /// * `reference` - Pair to shock (`from` must differ from `to`) plus the
    ///   date and policy of the reference lookup that must succeed on the
    ///   unshocked baseline. The shock itself is not limited to that scope.
    /// * `bump` - Fractional relative change; finite and greater than -1. A
    ///   non-zero bump must change `1 + bump` and every shocked rate; `0.0` is
    ///   a valid identity scenario.
    ///
    /// # Errors
    ///
    /// Returns a validation error for an identity pair or an invalid or
    /// ineffective bump (checked before any lookup), when the state cannot be
    /// captured or restored, when the reference lookup fails on the baseline,
    /// or when any shocked rate is non-finite, non-positive or unchanged by a
    /// non-zero bump.
    pub fn with_bumped_rate(&self, reference: FxQuery, bump: f64) -> crate::Result<Self> {
        let shock = PairShock::new(reference.from, reference.to, bump)?;
        let state = self.get_serializable_state()?;
        let baseline = Self::try_from_state(&state)?;
        let reference_rate = baseline.rate(reference)?.rate;
        shock.apply(reference.from, reference.to, reference_rate)?;
        Self::try_from_state(&shock.shocked_state(&baseline, &state)?)
    }

    /// Derive `query` through `pivot` after the provider reported it missing.
    ///
    /// This is intentionally a one-pivot fallback, not a general graph search.
    /// Each leg uses the same date, policy and source precedence and never
    /// routes again. A leg the provider reports missing becomes
    /// `FxTriangulationFailed` naming that ordered leg; any other leg error
    /// propagates with its own category.
    fn triangulate_rate(
        &self,
        query: FxQuery,
        pivot: Currency,
        provider_calls: &Cell<usize>,
    ) -> crate::Result<f64> {
        let FxQuery {
            from,
            to,
            on,
            policy,
        } = query;
        let revision = self.observation_revision();
        let leg = |leg_from, leg_to| {
            self.get_or_fetch(FxQuery::new(leg_from, leg_to, on, policy), provider_calls)
                .map_err(|error| {
                    if is_not_found(&error) {
                        crate::InputError::FxTriangulationFailed {
                            from,
                            to,
                            pivot,
                            missing_leg: format!(
                                "{leg_from}->{leg_to} rate not found on {on} ({policy})"
                            ),
                        }
                        .into()
                    } else {
                        error
                    }
                })
        };
        let first = leg(from, pivot)?;
        let second = leg(pivot, to)?;
        let rate = validate_fx_rate(from, to, first * second)?;
        // Cache the derived rate together with its triangulated provenance so
        // repeat queries stamp the same metadata and value as the first lookup.
        self.insert_observed_quote(query, rate, true, revision)
    }

    fn invalidate_observed_quotes(&self) {
        // Publish the revision while the authoritative store is still locked.
        // Cache reclamation follows after releasing that store, avoiding an
        // observed-cache -> authoritative-store lock dependency.
        lock(&self.observed_quotes).clear();
    }

    /// Fail before a mutation when no fresh revision remains to publish it.
    /// Callers hold the writer lock, so the check and the advance pair up.
    fn ensure_revision_available(&self) -> crate::Result<()> {
        if self.quote_revision.load(Ordering::Acquire) == u64::MAX {
            return Err(crate::Error::Validation(
                "FX matrix revision exhausted; build a new matrix".into(),
            ));
        }
        Ok(())
    }

    fn observation_revision(&self) -> Option<ObservationRevision> {
        let (matrix, provider) = self.state_revision();
        provider.map(|provider| ObservationRevision { matrix, provider })
    }

    fn state_revision(&self) -> (u64, Option<u64>) {
        (
            self.quote_revision.load(Ordering::Acquire),
            self.provider.get_revision(),
        )
    }

    /// Record a provider or pivot-derived answer for its exact ordered query.
    ///
    /// The rate bits and triangulation flag are stored unchanged; the opposite
    /// query is a separate key. Nothing is stored when either revision moved
    /// since `observed_at`.
    fn insert_observed_quote(
        &self,
        query: FxQuery,
        rate: f64,
        triangulated: bool,
        observed_at: Option<ObservationRevision>,
    ) -> crate::Result<f64> {
        let rate = validate_fx_rate(query.from, query.to, rate)?;
        let mut quotes = lock(&self.observed_quotes);
        if let Some(revision) =
            observed_at.filter(|&revision| Some(revision) == self.observation_revision())
        {
            quotes.put(
                query,
                ObservedQuote {
                    rate,
                    triangulated,
                    revision,
                },
            );
        }
        Ok(rate)
    }

    /// Insert an authoritative, non-evicting pinned quote (via `set_quote_on`).
    /// The rate is validated by the caller.
    fn insert_pinned_quote(&self, key: FxQuery, rate: f64) -> crate::Result<()> {
        {
            let _writer = lock(&self.writer);
            self.ensure_revision_available()?;
            let mut pinned = lock(&self.pinned_quotes);
            pinned.insert(key, rate);
            self.quote_revision.fetch_add(1, Ordering::AcqRel);
        }
        self.invalidate_observed_quotes();
        Ok(())
    }

    /// Insert a batch of pinned quotes atomically: every quote is validated
    /// and no ordered `(from, to, on, policy)` key may repeat before any write.
    fn insert_pinned_batch(
        &self,
        quotes: &[(Currency, Currency, Date, FxConversionPolicy, f64)],
    ) -> crate::Result<()> {
        let mut batch = FxHashMap::default();
        for &(from, to, on, policy, rate) in quotes {
            let rate = validate_stored_quote(from, to, rate)?;
            if batch
                .insert(FxQuery::new(from, to, on, policy), rate)
                .is_some()
            {
                return Err(crate::Error::Validation(format!(
                    "pinned FX quotes repeat {from}->{to} on {on} ({policy})"
                )));
            }
        }
        {
            let _writer = lock(&self.writer);
            self.ensure_revision_available()?;
            lock(&self.pinned_quotes).extend(batch);
            self.quote_revision.fetch_add(1, Ordering::AcqRel);
        }
        self.invalidate_observed_quotes();
        Ok(())
    }

    /// Resolve one pivot leg with the same precedence as [`rate`](Self::rate)
    /// (pinned, global, observed, provider) but without routing again.
    ///
    /// Pinned fixings are authoritative for their `(on, policy)`, so crosses
    /// honor them exactly like direct lookups do; otherwise a cross would
    /// contradict a pinned leg.
    fn get_or_fetch(&self, query: FxQuery, provider_calls: &Cell<usize>) -> crate::Result<f64> {
        if query.from == query.to {
            return Ok(1.0);
        }
        if let Some(cached) = self.read_cached_rate(query)? {
            return Ok(cached.rate);
        }
        self.fetch_from_provider(query, provider_calls)
    }

    /// Fetch `query` from the provider, validate the rate and record it as an
    /// observation of that exact ordered query.
    ///
    /// A provider without a revision answers at most one call per resolution
    /// attempt: a second call could observe a different quote state.
    fn fetch_from_provider(
        &self,
        query: FxQuery,
        provider_calls: &Cell<usize>,
    ) -> crate::Result<f64> {
        let revision = self.observation_revision();
        if revision.is_none() && provider_calls.get() > 0 {
            return Err(crate::Error::Validation(format!(
                "FX provider without a revision cannot answer a second call ({}->{}) within \
                 one resolution; give it a revision or freeze its quotes",
                query.from, query.to
            )));
        }
        provider_calls.set(provider_calls.get() + 1);
        let rate = self
            .provider
            .rate(query.from, query.to, query.on, query.policy)?;
        let rate = validate_fx_rate(query.from, query.to, rate)?;
        self.insert_observed_quote(query, rate, false, revision)
    }

    /// Resolve the matrix-held and observed sources for `query`, in authority
    /// order: exact date/policy pin, pair-global quote, then the observation
    /// of this exact ordered query. Each matrix store prefers the supplied
    /// direction, otherwise the checked reciprocal of its opposite quote.
    fn read_cached_rate(&self, query: FxQuery) -> crate::Result<Option<FxRateResult>> {
        let FxQuery {
            from,
            to,
            on,
            policy,
        } = query;
        // Lock each store lazily so the common case acquires a single lock.
        let pinned = {
            let pinned = lock(&self.pinned_quotes);
            directional_rate(
                |a, b| pinned.get(&FxQuery::new(a, b, on, policy)).copied(),
                from,
                to,
            )?
        };
        let authoritative = match pinned {
            Some(rate) => Some(rate),
            None => {
                let quotes = lock(&self.quotes);
                directional_rate(
                    |a, b| quotes.get(&CurrencyPair { base: a, quote: b }).copied(),
                    from,
                    to,
                )?
            }
        };
        if let Some(rate) = authoritative {
            return Ok(Some(FxRateResult {
                rate,
                triangulated: false,
            }));
        }
        Ok(self.read_observed(query).map(|observed| FxRateResult {
            rate: observed.rate,
            triangulated: observed.triangulated,
        }))
    }

    /// Read the observation stored for this exact ordered query at the
    /// current revisions, discarding a stale entry.
    fn read_observed(&self, query: FxQuery) -> Option<ObservedQuote> {
        let revision = self.observation_revision()?;
        let mut quotes = lock(&self.observed_quotes);
        let observed = quotes.get(&query).copied()?;
        if observed.revision != revision {
            let _ = quotes.pop(&query);
            return None;
        }
        Some(observed)
    }
}

/// Validate a batch of global quotes and reject a repeated ordered pair.
pub(super) fn reject_repeated_pairs(quotes: &[(Currency, Currency, f64)]) -> crate::Result<()> {
    let mut seen = crate::FxHashSet::default();
    for &(from, to, rate) in quotes {
        validate_stored_quote(from, to, rate)?;
        if !seen.insert((from, to)) {
            return Err(crate::Error::Validation(format!(
                "FX quote batch repeats {from}->{to}"
            )));
        }
    }
    Ok(())
}

/// One date/policy-scoped quote: `(from, to, on, policy, rate)`.
type ScopedQuote = (Currency, Currency, Date, FxConversionPolicy, f64);

/// One validated relative pair shock: `from -> to` rates are multiplied by
/// `factor`, `to -> from` rates are divided by it.
struct PairShock {
    from: Currency,
    to: Currency,
    bump: f64,
    factor: f64,
}

impl PairShock {
    fn new(from: Currency, to: Currency, bump: f64) -> crate::Result<Self> {
        if from == to {
            return Err(crate::Error::Validation(
                "cannot bump an identity FX pair".to_string(),
            ));
        }
        let factor = 1.0 + bump;
        if !bump.is_finite() || bump <= -1.0 || !factor.is_finite() || factor <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "FX bump must be finite and greater than -1 (got {bump})"
            )));
        }
        // Exact identity: factor is finite and positive here.
        if bump != 0.0 && factor.to_bits() == 1.0_f64.to_bits() {
            return Err(crate::Error::Validation(format!(
                "FX bump {bump} is too small: 1 + bump rounds to 1"
            )));
        }
        Ok(Self {
            from,
            to,
            bump,
            factor,
        })
    }

    fn is_target(&self, a: Currency, b: Currency) -> bool {
        (a, b) == (self.from, self.to) || (a, b) == (self.to, self.from)
    }

    /// Shock the `a -> b` rate once; other pairs pass through unchanged.
    fn apply(&self, a: Currency, b: Currency, rate: f64) -> crate::Result<f64> {
        let shocked = if (a, b) == (self.from, self.to) {
            rate * self.factor
        } else if (a, b) == (self.to, self.from) {
            rate / self.factor
        } else {
            return Ok(rate);
        };
        let shocked = validate_fx_rate(a, b, shocked)?;
        // Exact identity: both rates are finite and positive here.
        if self.bump != 0.0 && shocked.to_bits() == rate.to_bits() {
            return Err(crate::Error::Validation(format!(
                "FX bump {} leaves the {a}->{b} rate {rate} unchanged",
                self.bump
            )));
        }
        Ok(shocked)
    }

    /// Shock one store's quotes for one scope. When only one orientation of
    /// the target is stored, the other is written beside it from its own
    /// unshocked reciprocal so each orientation is shocked exactly once.
    fn shock_store(
        &self,
        quotes: &[(Currency, Currency, f64)],
    ) -> crate::Result<Vec<(Currency, Currency, f64)>> {
        let mut out = Vec::with_capacity(quotes.len() + 1);
        let mut forward = None;
        let mut reverse = None;
        for &(a, b, rate) in quotes {
            if (a, b) == (self.from, self.to) {
                forward = Some(rate);
            } else if (a, b) == (self.to, self.from) {
                reverse = Some(rate);
            }
            out.push((a, b, self.apply(a, b, rate)?));
        }
        match (forward, reverse) {
            (Some(rate), None) => {
                let unshocked = super::provider::reciprocal_rate_or_err(rate, self.from, self.to)?;
                out.push((
                    self.to,
                    self.from,
                    self.apply(self.to, self.from, unshocked)?,
                ));
            }
            (None, Some(rate)) => {
                let unshocked = super::provider::reciprocal_rate_or_err(rate, self.to, self.from)?;
                out.push((
                    self.from,
                    self.to,
                    self.apply(self.from, self.to, unshocked)?,
                ));
            }
            _ => {}
        }
        Ok(out)
    }

    /// Shock a date/policy-scoped store scope by scope.
    fn shock_scoped_store(
        &self,
        quotes: &[(Currency, Currency, Date, FxConversionPolicy, f64)],
    ) -> crate::Result<Vec<ScopedQuote>> {
        let mut scopes: std::collections::BTreeMap<(Date, u8), (FxConversionPolicy, Vec<_>)> =
            std::collections::BTreeMap::new();
        for &(a, b, on, policy, rate) in quotes {
            scopes
                .entry((on, policy as u8))
                .or_insert_with(|| (policy, Vec::new()))
                .1
                .push((a, b, rate));
        }
        let mut out = Vec::with_capacity(quotes.len());
        for ((on, _), (policy, scoped)) in scopes {
            for (a, b, rate) in self.shock_store(&scoped)? {
                out.push((a, b, on, policy, rate));
            }
        }
        Ok(out)
    }

    /// Shock the baseline's answer for both target orientations at one scope.
    /// A missing route stores nothing; any other failure propagates.
    fn shock_route(
        &self,
        baseline: &FxMatrix,
        on: Date,
        policy: FxConversionPolicy,
    ) -> crate::Result<Vec<(Currency, Currency, f64)>> {
        let mut out = Vec::with_capacity(2);
        for (a, b) in [(self.from, self.to), (self.to, self.from)] {
            match baseline.rate(FxQuery::new(a, b, on, policy)) {
                Ok(result) => out.push((a, b, self.apply(a, b, result.rate)?)),
                Err(crate::Error::Input(
                    crate::InputError::NotFound { .. }
                    | crate::InputError::FxTriangulationFailed { .. },
                )) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(out)
    }

    /// Rewrite a captured state into the frozen shocked state.
    fn shocked_state(
        &self,
        baseline: &FxMatrix,
        state: &FxMatrixState,
    ) -> crate::Result<FxMatrixState> {
        let mut provider_quotes = self.shock_store(&state.provider_quotes)?;
        let mut provider_pinned_quotes = self.shock_scoped_store(&state.provider_pinned_quotes)?;
        let stores_target = |quotes: &[(Currency, Currency, f64)]| {
            quotes.iter().any(|&(a, b, _)| self.is_target(a, b))
        };
        let globally_stored = stores_target(&state.quotes) || stores_target(&state.provider_quotes);
        if !globally_stored {
            // The pair is only reachable through the pivot: freeze each
            // orientation's route result, globally and in every scope that
            // does not store the pair itself.
            let mut scopes: Vec<(Date, FxConversionPolicy)> = state
                .pinned_quotes
                .iter()
                .chain(&state.provider_pinned_quotes)
                .map(|&(_, _, on, policy, _)| (on, policy))
                .collect();
            scopes.sort_by_key(|&(on, policy)| (on, policy as u8));
            scopes.dedup();
            let global_scope = unscoped_probe(&scopes)?;
            provider_quotes.extend(self.shock_route(baseline, global_scope.0, global_scope.1)?);
            for &(on, policy) in &scopes {
                let scope_stores_target = state
                    .pinned_quotes
                    .iter()
                    .chain(&state.provider_pinned_quotes)
                    .any(|&(a, b, q_on, q_policy, _)| {
                        (q_on, q_policy) == (on, policy) && self.is_target(a, b)
                    });
                if scope_stores_target {
                    continue;
                }
                for (a, b, rate) in self.shock_route(baseline, on, policy)? {
                    provider_pinned_quotes.push((a, b, on, policy, rate));
                }
            }
        }
        Ok(FxMatrixState {
            config: state.config,
            quotes: self.shock_store(&state.quotes)?,
            provider_quotes,
            provider_pinned_quotes,
            pinned_quotes: self.shock_scoped_store(&state.pinned_quotes)?,
        })
    }
}

/// A date/policy outside every captured scope, where only global quotes
/// apply; global quotes answer every date and policy alike.
fn unscoped_probe(
    scopes: &[(Date, FxConversionPolicy)],
) -> crate::Result<(Date, FxConversionPolicy)> {
    let mut on = Date::MIN;
    for _ in 0..=scopes.len() {
        if !scopes.contains(&(on, FxConversionPolicy::CashflowDate)) {
            return Ok((on, FxConversionPolicy::CashflowDate));
        }
        on = on.next_day().ok_or_else(|| {
            crate::Error::Validation("no unscoped FX probe date available".into())
        })?;
    }
    Err(crate::Error::Validation(
        "no unscoped FX probe date available".into(),
    ))
}

/// Reject an invalid captured global quote or a repeated ordered key; `quotes`
/// is sorted by key.
fn validate_captured_global(quotes: &[(Currency, Currency, f64)]) -> crate::Result<()> {
    for (index, &(from, to, rate)) in quotes.iter().enumerate() {
        validate_stored_quote(from, to, rate)?;
        if index > 0 && (quotes[index - 1].0, quotes[index - 1].1) == (from, to) {
            return Err(crate::Error::Validation(format!(
                "captured FX state repeats the quote {from}->{to}"
            )));
        }
    }
    Ok(())
}

/// Reject an invalid captured scoped quote or a repeated ordered key; `quotes`
/// is sorted by key.
fn validate_captured_pinned(
    quotes: &[(Currency, Currency, Date, FxConversionPolicy, f64)],
) -> crate::Result<()> {
    for (index, &(from, to, on, policy, rate)) in quotes.iter().enumerate() {
        validate_stored_quote(from, to, rate)?;
        if index > 0 {
            let (pf, pt, pon, ppolicy, _) = quotes[index - 1];
            if (pf, pt, pon, ppolicy) == (from, to, on, policy) {
                return Err(crate::Error::Validation(format!(
                    "captured FX state repeats the quote {from}->{to} on {on} ({policy})"
                )));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod revision_tests {
    use super::*;
    use crate::money::fx::SimpleFxProvider;

    #[test]
    fn exhausted_matrix_revision_fails_before_mutation() {
        let on = Date::from_ordinal_date(2025, 1).expect("date");
        let matrix = FxMatrix::new(Arc::new(SimpleFxProvider::new()), None);
        matrix.quote_revision.store(u64::MAX, Ordering::Release);
        assert!(
            matrix
                .set_quotes(&[(Currency::EUR, Currency::USD, 1.1)])
                .is_err()
        );
        assert!(
            matrix
                .set_quote_on(
                    Currency::EUR,
                    Currency::USD,
                    on,
                    FxConversionPolicy::CashflowDate,
                    1.1
                )
                .is_err()
        );
        assert!(lock(&matrix.quotes).is_empty());
        assert!(lock(&matrix.pinned_quotes).is_empty());
        assert_eq!(matrix.quote_revision.load(Ordering::Acquire), u64::MAX);
    }

    #[test]
    fn quote_mutations_publish_revision_before_waiting_for_cache_reclamation() {
        let on = Date::from_ordinal_date(2025, 1).expect("date");
        let key = FxQuery {
            from: Currency::EUR,
            to: Currency::USD,
            on,
            policy: FxConversionPolicy::CashflowDate,
        };
        for mutation in ["single", "batch", "pinned"] {
            let matrix = FxMatrix::new(Arc::new(SimpleFxProvider::new()), None);
            matrix
                .set_quotes(&[(key.from, key.to, 1.1)])
                .expect("global");
            matrix
                .set_quote_on(key.from, key.to, on, key.policy, 1.1)
                .expect("pinned");
            let before = matrix.quote_revision.load(Ordering::Acquire);
            // Hold reclamation closed while the writer publishes its quote.
            // The authoritative store remains readable during this interval.
            let observed = lock(&matrix.observed_quotes);
            std::thread::scope(|scope| {
                let writer = scope.spawn(|| match mutation {
                    "single" => matrix
                        .set_quotes(&[(key.from, key.to, 1.2)])
                        .expect("update"),
                    "batch" => matrix
                        .set_quotes(&[(key.from, key.to, 1.2)])
                        .expect("batch update"),
                    "pinned" => matrix
                        .set_quote_on(key.from, key.to, on, key.policy, 1.2)
                        .expect("pinned update"),
                    _ => unreachable!(),
                });
                loop {
                    let published = match mutation {
                        "single" | "batch" => {
                            lock(&matrix.quotes).get(&CurrencyPair {
                                base: key.from,
                                quote: key.to,
                            }) == Some(&1.2)
                        }
                        "pinned" => lock(&matrix.pinned_quotes).get(&key) == Some(&1.2),
                        _ => unreachable!(),
                    };
                    if published {
                        break;
                    }
                    std::thread::yield_now();
                }
                let published_revision = matrix.quote_revision.load(Ordering::Acquire);
                // Release before asserting so a failing test cannot strand the writer.
                drop(observed);
                writer.join().expect("writer");
                assert_ne!(before, published_revision, "{mutation}");
            });
        }
    }
}
