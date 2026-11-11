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

/// Caller-pinned and provider-pinned quote stores for one date/policy scope.
type ScopedSnapshotQuotes = (FxHashMap<CurrencyPair, f64>, FxHashMap<CurrencyPair, f64>);

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

/// Resolve `from → to` through the stores in priority order (scoped pin,
/// pair-global, provider), each with reciprocal fallback.
fn effective_snapshot_rate(
    global: &FxHashMap<CurrencyPair, f64>,
    pinned: &FxHashMap<CurrencyPair, f64>,
    observed: &FxHashMap<CurrencyPair, f64>,
    from: Currency,
    to: Currency,
) -> crate::Result<Option<f64>> {
    for quotes in [pinned, global, observed] {
        let rate = directional_rate(
            |a, b| quotes.get(&CurrencyPair { base: a, quote: b }).copied(),
            from,
            to,
        )?;
        if rate.is_some() {
            return Ok(rate);
        }
    }
    Ok(None)
}

/// Cross `from → pivot → to` when both legs resolve; a missing leg yields
/// `None` and an invalid leg propagates its error.
fn pivot_cross(
    lookup: impl Fn(Currency, Currency) -> crate::Result<Option<f64>>,
    from: Currency,
    to: Currency,
    pivot: Currency,
) -> crate::Result<Option<f64>> {
    let (Some(first), Some(second)) = (lookup(from, pivot)?, lookup(pivot, to)?) else {
        return Ok(None);
    };
    validate_fx_rate(from, to, first * second).map(Some)
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

/// Resolve only the shocked pair through the original matrix. Other pairs
/// remain provider lookups so the new matrix can triangulate through bumped legs.
struct ResolvedPairProvider {
    matrix: FxMatrix,
    pair: CurrencyPair,
}

impl FxProvider for ResolvedPairProvider {
    fn get_revision(&self) -> Option<u64> {
        // This wrapper owns a fixed copy of the matrix overrides. Only its
        // shared provider can change after construction.
        self.matrix.provider.get_revision()
    }

    fn rate(
        &self,
        from: Currency,
        to: Currency,
        on: Date,
        policy: FxConversionPolicy,
    ) -> crate::Result<f64> {
        if (CurrencyPair {
            base: from,
            quote: to,
        }) == self.pair
            || (CurrencyPair {
                base: to,
                quote: from,
            }) == self.pair
        {
            return self
                .matrix
                .rate(FxQuery::new(from, to, on, policy))
                .map(|result| result.rate);
        }
        self.matrix.provider.rate(from, to, on, policy)
    }

    fn snapshot(&self) -> crate::Result<Option<FxProviderSnapshot>> {
        let Some(FxProviderSnapshot {
            mut quotes,
            pinned_quotes: mut provider_pinned,
        }) = self.matrix.provider.snapshot()?
        else {
            return Ok(None);
        };
        let global = lock(&self.matrix.quotes);
        let pinned = lock(&self.matrix.pinned_quotes);
        let provider: FxHashMap<CurrencyPair, f64> = quotes
            .iter()
            .map(|&(from, to, rate)| {
                (
                    CurrencyPair {
                        base: from,
                        quote: to,
                    },
                    rate,
                )
            })
            .collect();
        let empty = FxHashMap::default();
        let CurrencyPair {
            base: from,
            quote: to,
        } = self.pair;
        let pivot = self.matrix.config.pivot_currency;

        let global_lookup = |from, to| -> crate::Result<Option<f64>> {
            if from == to {
                Ok(Some(1.0))
            } else {
                effective_snapshot_rate(&global, &empty, &provider, from, to)
            }
        };
        let global_rate = match global_lookup(from, to)? {
            Some(rate) => Some(rate),
            None => match pivot {
                Some(pivot) => pivot_cross(global_lookup, from, to, pivot)?,
                None => None,
            },
        };
        if let Some(rate) = global_rate {
            // The shocked cross must survive restoration even when only its
            // pivot legs were quoted. Provider authority stays below explicit
            // pinned fixings and global quotes in the outer matrix.
            quotes.retain(|&(base, quote, _)| {
                CurrencyPair { base, quote } != self.pair
                    && CurrencyPair {
                        base: quote,
                        quote: base,
                    } != self.pair
            });
            quotes.push((from, to, rate));
        }

        let Some(pivot) = pivot else {
            return Ok(Some(FxProviderSnapshot {
                quotes,
                pinned_quotes: provider_pinned,
            }));
        };
        let mut scopes: FxHashMap<_, ScopedSnapshotQuotes> = FxHashMap::default();
        for &(from, to, on, policy, rate) in &provider_pinned {
            scopes.entry((on, policy)).or_default().1.insert(
                CurrencyPair {
                    base: from,
                    quote: to,
                },
                rate,
            );
        }
        for (key, &rate) in pinned.iter() {
            scopes.entry((key.on, key.policy)).or_default().0.insert(
                CurrencyPair {
                    base: key.from,
                    quote: key.to,
                },
                rate,
            );
        }
        let mut scoped_crosses = Vec::new();
        for ((on, policy), (pinned, scoped_provider)) in scopes {
            let lookup = |from, to| -> crate::Result<Option<f64>> {
                if from == to {
                    return Ok(Some(1.0));
                }
                if let Some(rate) =
                    effective_snapshot_rate(&global, &pinned, &scoped_provider, from, to)?
                {
                    return Ok(Some(rate));
                }
                effective_snapshot_rate(&empty, &empty, &provider, from, to)
            };
            // An explicit target quote is already copied and scaled by the
            // outer matrix/provider. Only a missing direct target requires
            // persisting its shocked, statically representable pivot cross.
            if lookup(from, to)?.is_some() {
                continue;
            }
            if let Some(rate) = pivot_cross(lookup, from, to, pivot)? {
                scoped_crosses.push((from, to, on, policy, rate));
            }
        }
        provider_pinned.extend(scoped_crosses);
        Ok(Some(FxProviderSnapshot {
            quotes,
            pinned_quotes: provider_pinned,
        }))
    }
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
    /// assert!(matrix.get_serializable_state().expect("state").quotes.is_empty());
    /// assert_eq!(matrix.config().pivot_currency, None);
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
    /// let matrix = FxMatrix::new(Arc::new(StaticFx));
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
    /// non-finite or not strictly positive, or the matrix revision is
    /// exhausted. In that case the matrix retains every pre-existing explicit
    /// quote and none of the batch is applied.
    pub fn set_quotes(&self, quotes: &[(Currency, Currency, f64)]) -> crate::Result<()> {
        for &(from, to, rate) in quotes {
            validate_stored_quote(from, to, rate)?;
        }
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

    /// Load quotes from a serialized state.
    ///
    /// This allows restoring cached quotes after deserialization.
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_quant_core::money::fx::{FxMatrix, FxProvider, FxConversionPolicy, FxMatrixState};
    /// # use finstack_quant_core::currency::Currency;
    /// # use finstack_quant_core::dates::Date;
    /// # use std::sync::Arc;
    /// # use time::Month;
    /// # struct StaticFx;
    /// # impl FxProvider for StaticFx {
    /// #     fn rate(&self, _from: Currency, _to: Currency, _on: Date, _policy: FxConversionPolicy)
    /// #         -> finstack_quant_core::Result<f64> { Ok(1.0) }
    /// # }
    /// let matrix = FxMatrix::new(Arc::new(StaticFx));
    /// let state = FxMatrixState {
    ///     config: matrix.get_serializable_state().expect("stable FX state").config,
    ///     quotes: vec![],
    ///     provider_quotes: vec![],
    ///     provider_pinned_quotes: vec![],
    ///     pinned_quotes: vec![],
    /// };
    /// matrix.load_from_state(&state).expect("valid snapshot state");
    /// ```
    ///
    /// Pair-global quotes are restored first, followed by date/policy-pinned
    /// fixings. The state configuration is informational here: construct the
    /// matrix with [`try_with_config`](Self::try_with_config) using
    /// `state.config` when restoring cache capacity and triangulation policy.
    /// The existing provider is retained. `state.provider_quotes` and
    /// `state.provider_pinned_quotes` are used by `MarketContext` snapshot
    /// restoration to construct a quote-only provider;
    /// this method loads only the explicit global and pinned quote stores.
    ///
    /// # Errors
    ///
    /// Returns an error if any persisted rate is non-finite or not strictly
    /// positive. Pair-global quotes are batch-validated before insertion, but a
    /// failure in a later pinned fixing leaves successfully restored earlier
    /// quotes/fixings in place; create a fresh matrix when all-or-nothing
    /// restoration is required.
    ///
    /// # Arguments
    ///
    /// * `state` - Pair-global `quotes` and date/policy `pinned_quotes` to load. Provider snapshot fields are ignored here.
    pub fn load_from_state(&self, state: &FxMatrixState) -> crate::Result<()> {
        self.set_quotes(&state.quotes)?;
        // Restore pinned (date/policy-scoped) fixings so they keep outranking
        // the provider after a snapshot/restore round-trip.
        for &(from, to, on, policy, rate) in &state.pinned_quotes {
            self.set_quote_on(from, to, on, policy, rate)?;
        }
        Ok(())
    }

    /// Restore a matrix from a serialized FX snapshot, including the provider
    /// quotes it was captured with.
    ///
    /// The snapshot's provider and provider-pinned quotes become a quote-only
    /// provider, and the global and pinned fixings are then loaded on top, so
    /// the restored matrix looks rates up as the captured one did.
    ///
    /// # Arguments
    ///
    /// * `state` - Snapshot whose `config`, provider quotes, provider-pinned
    ///   quotes, global `quotes`, and `pinned_quotes` are restored.
    ///
    /// # Errors
    ///
    /// Returns an error when the configuration is invalid or any persisted rate
    /// is non-finite or not strictly positive.
    pub fn try_from_state(state: &FxMatrixState) -> crate::Result<Self> {
        let provider = super::SimpleFxProvider::new();
        provider.set_quotes(&state.provider_quotes)?;
        provider.set_snapshot_pinned_quotes(&state.provider_pinned_quotes)?;
        let matrix = Self::try_with_config(Arc::new(provider), state.config)?;
        matrix.load_from_state(state)?;
        Ok(matrix)
    }

    /// Create a new FX matrix with a relatively bumped rate for a currency pair.
    ///
    /// This is useful for finite difference greek calculations where we need
    /// to bump FX spot while preserving all other market data. The bump is
    /// **relative and term-structure preserving**: the wrapper provider applies
    /// `rate(query) * (1 + bump_pct)` to the delegated per-date rate, so a
    /// date-aware provider keeps its term structure under the bump. Derived
    /// pairs are resolved through the original matrix for each date/policy
    /// before applying the multiplier.
    ///
    /// Explicit pair-global quotes and pinned (date/policy-scoped) fixings for
    /// the bumped pair are carried over **scaled by the same multiplier**
    /// (reciprocal-direction quotes are divided), so every source of the
    /// bumped pair moves coherently. Quotes for other pairs are carried over
    /// unchanged.
    ///
    /// Quote-backed providers retain their quotes and the shocked pair in
    /// serialized snapshots, including a pair derived through triangulation.
    /// Crosses that depend on pinned pivot legs retain their date and policy
    /// scope in the snapshot. These persistence quotes do not alter lookup
    /// authority in the live bumped matrix.
    /// Live providers that do not expose snapshot quotes retain their
    /// date/policy-aware lookup behavior; no reference-date quote is frozen
    /// into a global rate for persistence.
    ///
    /// # Arguments
    /// - `from` - Base currency of the pair to shock; must differ from `to`.
    /// - `to` - Quote currency, whose units are paid per unit of `from`.
    /// - `bump_pct` - Relative decimal shock; 0.01 increases the rate by 1%.
    ///   Must be finite and greater than -1.
    /// - `on` - Reference date used to validate the shocked CashflowDate quote.
    ///
    /// # Returns
    /// New FxMatrix with the relatively bumped pair
    ///
    /// # Errors
    /// Returns an error if the rate lookup on `on` fails, if `bump_pct` is
    /// non-finite, if the bump multiplier `1 + bump_pct` is not positive, or
    /// if any statically known shocked quote/scoped cross is outside the
    /// finite, positive FX range.
    pub fn with_bumped_rate(
        &self,
        from: Currency,
        to: Currency,
        bump_pct: f64,
        on: Date,
    ) -> crate::Result<Self> {
        if from == to {
            return Err(crate::Error::Validation(
                "cannot bump an identity FX pair".to_string(),
            ));
        }
        // Verify a rate exists on the reference date and the bumped value is valid.
        let query = FxQuery::new(from, to, on, FxConversionPolicy::CashflowDate);
        let current = self.rate(query)?;
        validate_fx_rate(from, to, current.rate * (1.0 + bump_pct))?;

        // Preserve authoritative quotes without freezing date-aware provider
        // observations into pair-global rates.
        let source = Self::try_with_config(Arc::clone(&self.provider), self.config)?;
        *lock(&source.quotes) = lock(&self.quotes).clone();
        *lock(&source.pinned_quotes) = lock(&self.pinned_quotes).clone();

        use super::providers::BumpedFxProvider;
        let bumped_provider = Arc::new(BumpedFxProvider::new(
            Arc::new(ResolvedPairProvider {
                matrix: source,
                pair: CurrencyPair {
                    base: from,
                    quote: to,
                },
            }),
            from,
            to,
            bump_pct,
        )?);
        let multiplier = 1.0 + bump_pct;

        let bumped = Self::try_with_config(bumped_provider, self.config)?;
        {
            let src = lock(&self.quotes);
            let mut dst = lock(&bumped.quotes);
            for (pair, rate) in src.iter() {
                let rate =
                    Self::scale_bumped_rate(pair.base, pair.quote, from, to, *rate, multiplier)?;
                dst.insert(*pair, rate);
            }
        }
        // Carry over authoritative pinned fixings; fixings on the bumped pair
        // are scaled by the bump multiplier so the relative bump applies on
        // every date, not only where the provider answers.
        {
            let src = lock(&self.pinned_quotes);
            let mut dst = lock(&bumped.pinned_quotes);
            for (key, rate) in src.iter() {
                let rate = Self::scale_bumped_rate(key.from, key.to, from, to, *rate, multiplier)?;
                dst.insert(*key, rate);
            }
        }
        // Do not carry over provider-observed quotes. They may be date/policy-sensitive
        // or derived crosses that depend transitively on the bumped leg.

        Ok(bumped)
    }

    /// Scale one stored orientation of the shocked pair by the bump multiplier.
    ///
    /// The direct orientation is multiplied. The opposite orientation is divided
    /// so both stored directions move together. Every other pair is unchanged.
    fn scale_bumped_rate(
        stored_from: Currency,
        stored_to: Currency,
        bump_from: Currency,
        bump_to: Currency,
        rate: f64,
        multiplier: f64,
    ) -> crate::Result<f64> {
        let scaled = if stored_from == bump_from && stored_to == bump_to {
            rate * multiplier
        } else if stored_from == bump_to && stored_to == bump_from {
            rate / multiplier
        } else {
            rate
        };
        validate_fx_rate(stored_from, stored_to, scaled)
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
