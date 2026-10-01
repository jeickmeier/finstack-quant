//! FX quote storage, triangulation, caching, and provider-backed conversion.
//!
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::collections::{HashMap, HashSet};

fn recover<T>(res: Result<T, std::sync::PoisonError<T>>) -> T {
    res.unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    recover(mutex.lock())
}

// Non-cryptographic keys (currency pairs, query keys) use the workspace-standard
// FxHash rather than std's SipHash for ~2x faster lookups on the FX hot path.
use crate::currency::Currency;
use crate::dates::Date;

use super::provider::{reciprocal_rate_or_err, validate_fx_rate, FxProvider};
use super::types::{FxConfig, FxConversionPolicy, FxMatrixState, FxQuery, FxRateResult};

const MAX_STATE_ATTEMPTS: usize = 3;

/// Pair key for the explicit-quote cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Pair(Currency, Currency);

/// Caller-pinned and provider-pinned quote stores for one date/policy scope.
type ScopedSnapshotQuotes = (HashMap<Pair, f64>, HashMap<Pair, f64>);

/// Query-sensitive key for the provider-observed quote cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct QueryKey {
    from: Currency,
    to: Currency,
    on: Date,
    policy: FxConversionPolicy,
}

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

/// Return the canonical storage orientation for an unordered currency pair.
///
/// Observed quotes use one orientation so both lookup directions derive from
/// the same stored floating-point value. The boolean is true when the caller's
/// requested direction is the reciprocal of that canonical orientation.
#[inline]
fn canonical_orientation(from: Currency, to: Currency) -> (Currency, Currency, bool) {
    if from <= to {
        (from, to, false)
    } else {
        (to, from, true)
    }
}

fn effective_snapshot_rate(
    global: &HashMap<Pair, f64>,
    pinned: &HashMap<Pair, f64>,
    observed: &HashMap<Pair, f64>,
    from: Currency,
    to: Currency,
) -> Option<f64> {
    let direct = Pair(from, to);
    let reverse = Pair(to, from);
    global
        .get(&direct)
        .copied()
        .or_else(|| global.get(&reverse).map(|rate| 1.0 / rate))
        .or_else(|| pinned.get(&direct).copied())
        .or_else(|| pinned.get(&reverse).map(|rate| 1.0 / rate))
        .or_else(|| observed.get(&direct).copied())
        .or_else(|| observed.get(&reverse).map(|rate| 1.0 / rate))
}

fn validate_fx_snapshot(
    global: &HashMap<Pair, f64>,
    pinned: &HashMap<Pair, f64>,
    observed: &HashMap<Pair, f64>,
    tolerance_bp: f64,
    scope: &str,
) -> crate::Result<()> {
    let mut currencies = HashSet::default();
    for pair in global.keys().chain(pinned.keys()).chain(observed.keys()) {
        currencies.insert(pair.0);
        currencies.insert(pair.1);
    }
    let mut currencies: Vec<_> = currencies.into_iter().collect();
    currencies.sort();

    // Check explicit two-way inconsistencies as well as three-currency cycles.
    for (i, &a) in currencies.iter().enumerate() {
        for &b in currencies.iter().skip(i + 1) {
            let (Some(ab), Some(ba)) = (
                effective_snapshot_rate(global, pinned, observed, a, b),
                effective_snapshot_rate(global, pinned, observed, b, a),
            ) else {
                continue;
            };
            let product = ab * ba;
            let deviation_bp = (product - 1.0).abs() * 10_000.0;
            if deviation_bp > tolerance_bp {
                return Err(crate::Error::Validation(format!(
                    "reciprocal FX inconsistency in {scope} for {a}<->{b}: product {product:.12} ({deviation_bp:.6} bp)"
                )));
            }
        }
    }

    for (i, &a) in currencies.iter().enumerate() {
        for (j, &b) in currencies.iter().enumerate().skip(i + 1) {
            for &c in currencies.iter().skip(j + 1) {
                let (Some(ab), Some(bc), Some(ca)) = (
                    effective_snapshot_rate(global, pinned, observed, a, b),
                    effective_snapshot_rate(global, pinned, observed, b, c),
                    effective_snapshot_rate(global, pinned, observed, c, a),
                ) else {
                    continue;
                };
                let product = ab * bc * ca;
                let deviation_bp = (product - 1.0).abs() * 10_000.0;
                if deviation_bp > tolerance_bp {
                    return Err(crate::Error::Validation(format!(
                        "triangular arbitrage detected in {scope} for {a}->{b}->{c}->{a}: cycle product {product:.12} ({deviation_bp:.6} bp)"
                    )));
                }
            }
        }
    }
    Ok(())
}

/// Simplified FX matrix that stores quotes and computes cross rates on demand.
///
/// Note: `FxMatrix` cannot be directly serialized due to the trait object
/// `Arc<dyn FxProvider>`. To persist FX state, use
/// [`FxMatrix::get_serializable_state`] to extract the config and quotes, then
/// recreate the matrix with [`FxMatrix::try_with_config`] and
/// [`FxMatrix::load_from_state`].
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
    /// currencies), so the store is a plain `HashMap` that **never evicts** — a
    /// seeded peg must never be silently dropped under cache pressure and then
    /// re-derived from the provider (which would be a silent mispricing).
    quotes: Mutex<HashMap<Pair, f64>>,
    /// Query-sensitive quotes observed from providers or triangulation. This is
    /// the only genuinely bounded cache (governed by `config.cache_capacity`).
    observed_quotes: Mutex<
        LruCache<QueryKey, ObservedQuote, std::hash::BuildHasherDefault<rustc_hash::FxHasher>>,
    >,
    /// Authoritative date/policy-scoped quotes pinned via [`FxMatrix::set_quote_on`].
    /// Unlike `observed_quotes`, this map never evicts, so a pinned fixing is
    /// never silently replaced by the provider under cache pressure.
    pinned_quotes: Mutex<HashMap<QueryKey, f64>>,
    config: FxConfig,
    quote_revision: AtomicU64,
}

/// Resolve only the shocked pair through the original matrix. Other pairs
/// remain provider lookups so the new matrix can triangulate through bumped legs.
struct ResolvedPairProvider {
    matrix: FxMatrix,
    pair: Pair,
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
        if Pair(from, to) == self.pair || Pair(to, from) == self.pair {
            return self
                .matrix
                .rate(FxQuery::with_policy(from, to, on, policy))
                .map(|result| result.rate);
        }
        self.matrix.provider.rate(from, to, on, policy)
    }

    fn snapshot_quotes(&self) -> Vec<(Currency, Currency, f64)> {
        let mut quotes = self.matrix.provider.snapshot_quotes();
        let global = lock(&self.matrix.quotes);
        let provider: HashMap<Pair, f64> = quotes
            .iter()
            .map(|&(from, to, rate)| (Pair(from, to), rate))
            .collect();
        let empty = HashMap::default();
        let lookup = |from, to| {
            if from == to {
                Some(1.0)
            } else {
                effective_snapshot_rate(&global, &empty, &provider, from, to)
            }
        };
        let Pair(from, to) = self.pair;
        let rate = lookup(from, to).or_else(|| {
            if self.matrix.config.enable_triangulation {
                let pivot = self.matrix.config.pivot_currency;
                Some(lookup(from, pivot)? * lookup(pivot, to)?)
            } else {
                None
            }
        });
        if let Some(rate) = rate {
            // The shocked cross must survive restoration even when only its
            // pivot legs were quoted. Provider authority stays below explicit
            // global quotes and date/policy-pinned fixings in the outer matrix.
            quotes.retain(|&(base, quote, _)| {
                Pair(base, quote) != self.pair && Pair(quote, base) != self.pair
            });
            quotes.push((from, to, rate));
        }
        quotes
    }

    fn snapshot_pinned_quotes(&self) -> Vec<(Currency, Currency, Date, FxConversionPolicy, f64)> {
        let mut quotes = self.matrix.provider.snapshot_pinned_quotes();
        if !self.matrix.config.enable_triangulation {
            return quotes;
        }
        let global = lock(&self.matrix.quotes);
        let pinned = lock(&self.matrix.pinned_quotes);
        let provider: HashMap<Pair, f64> = self
            .matrix
            .provider
            .snapshot_quotes()
            .into_iter()
            .map(|(from, to, rate)| (Pair(from, to), rate))
            .collect();
        let mut scopes: HashMap<_, ScopedSnapshotQuotes> = HashMap::default();
        for &(from, to, on, policy, rate) in &quotes {
            scopes
                .entry((on, policy))
                .or_default()
                .1
                .insert(Pair(from, to), rate);
        }
        for (key, &rate) in pinned.iter() {
            scopes
                .entry((key.on, key.policy))
                .or_default()
                .0
                .insert(Pair(key.from, key.to), rate);
        }
        let empty = HashMap::default();
        let Pair(from, to) = self.pair;
        let pivot = self.matrix.config.pivot_currency;
        for ((on, policy), (pinned, provider_pinned)) in scopes {
            let lookup = |from, to| {
                if from == to {
                    Some(1.0)
                } else {
                    effective_snapshot_rate(&global, &pinned, &provider_pinned, from, to)
                        .or_else(|| effective_snapshot_rate(&empty, &empty, &provider, from, to))
                }
            };
            // An explicit target quote is already copied and scaled by the
            // outer matrix/provider. Only a missing direct target requires
            // persisting its shocked, statically representable pivot cross.
            if lookup(from, to).is_some() {
                continue;
            }
            if let (Some(first), Some(second)) = (lookup(from, pivot), lookup(pivot, to)) {
                quotes.push((from, to, on, policy, first * second));
            }
        }
        quotes
    }
}

impl FxMatrix {
    /// Create a new [`FxMatrix`] wrapping the given provider with the default configuration.
    ///
    /// # Parameters
    /// - `provider`: FX quote source implementing [`FxProvider`]
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
    /// let matrix = FxMatrix::new(Arc::new(StaticFx));
    /// assert_eq!(matrix.cache_stats(), 0);
    /// ```
    pub fn new(provider: Arc<dyn FxProvider>) -> Self {
        let mut config = FxConfig::default();
        let capacity = NonZeroUsize::new(config.cache_capacity).unwrap_or_else(|| {
            config.cache_capacity = 1;
            NonZeroUsize::MIN
        });
        // Grow storage with observed quotes instead of preallocating the configured limit.
        let mut observed_quotes = LruCache::unbounded_with_hasher(Default::default());
        observed_quotes.resize(capacity);
        Self {
            provider,
            quotes: Mutex::new(HashMap::default()),
            observed_quotes: Mutex::new(observed_quotes),
            pinned_quotes: Mutex::new(HashMap::default()),
            config,
            quote_revision: AtomicU64::new(0),
        }
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
    /// * `provider` - FX or market-data provider used to resolve missing direct quotes
    /// * `config` - Configuration object controlling validation, rounding, or solver behavior
    pub fn try_with_config(provider: Arc<dyn FxProvider>, config: FxConfig) -> crate::Result<Self> {
        if config.cache_capacity == 0 {
            return Err(crate::Error::Validation(
                "FxConfig.cache_capacity must be > 0".to_string(),
            ));
        }
        let capacity = NonZeroUsize::new(config.cache_capacity).unwrap_or(NonZeroUsize::MIN);
        let mut observed_quotes = LruCache::unbounded_with_hasher(Default::default());
        observed_quotes.resize(capacity);
        Ok(Self {
            provider,
            quotes: Mutex::new(HashMap::default()),
            observed_quotes: Mutex::new(observed_quotes),
            pinned_quotes: Mutex::new(HashMap::default()),
            config,
            quote_revision: AtomicU64::new(0),
        })
    }

    /// Access the underlying FX provider reference.
    pub fn provider(&self) -> Arc<dyn FxProvider> {
        Arc::clone(&self.provider)
    }

    /// Return the matrix configuration.
    pub fn config(&self) -> FxConfig {
        self.config
    }

    /// Look up an FX rate (with metadata) using caching and triangulation fallbacks.
    ///
    /// # Parameters
    /// - `query`: [`FxQuery`] describing the desired conversion
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
    /// );
    /// let result = matrix.rate(query).expect("FX rate lookup should succeed");
    /// assert!(result.rate > 1.0);
    /// ```
    ///
    /// Lookup precedence is: pair-global explicit quote (either orientation),
    /// date/policy-pinned quote (either orientation), provider-observed cache
    /// at the current provider revision, then the provider. Providers without
    /// a revision are queried each time. Source priority is resolved before
    /// taking a reciprocal.
    /// If the direct provider request fails and triangulation is enabled, the
    /// matrix derives the cross through the configured pivot, except that an
    /// explicit invalid FX rate is returned as an error. The returned
    /// `triangulated` flag records whether that final fallback was used, even
    /// when the result is served from the observed cache later.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing provider quote when triangulation is
    /// disabled or cannot construct both pivot legs, or when any direct,
    /// reciprocal, provider, or triangulated rate is non-finite or non-positive.
    /// Identity conversion returns exactly `1.0` without querying the provider.
    /// Versioned provider or matrix updates during resolution trigger a retry;
    /// repeated updates that prevent a coherent result return a validation error.
    ///
    /// # Arguments
    ///
    /// * `query` - FX conversion query containing currencies, date, and lookup policy.
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
        let from = query.from;
        let to = query.to;
        let on = query.on;
        let policy = query.policy;

        if let Some(cached) = self.read_cached_rate(from, to, on, policy)? {
            return Ok(cached);
        }

        // The provider remains direction-sensitive; only cache storage is canonical.
        let revision = self.observation_revision();
        match self.provider.rate(from, to, on, policy) {
            Ok(rate) => {
                let rate = validate_fx_rate(from, to, rate)?;
                let rate = self.insert_observed_quote(query, rate, false, revision)?;
                Ok(FxRateResult {
                    rate,
                    triangulated: false,
                })
            }
            Err(error @ crate::Error::Input(crate::error::InputError::InvalidFxRate { .. })) => {
                Err(error)
            }
            Err(_) if self.config.enable_triangulation => {
                // Try simple triangulation via pivot
                let rate = self.triangulate_rate(from, to, on, policy)?;
                Ok(FxRateResult {
                    rate,
                    triangulated: true,
                })
            }
            Err(e) => Err(e),
        }
    }

    /// Seed or update a single **date- and policy-independent constant** quote.
    ///
    /// # ⚠️ Shadows the provider for every query
    ///
    /// An explicit quote set here is checked *before* the underlying
    /// [`FxProvider`] in [`rate`](Self::rate) and is **not** keyed by `on` or
    /// [`FxConversionPolicy`]. Once set, the pair is pinned to this single rate
    /// for **all** valuation dates and policies — a date-aware provider is
    /// silently bypassed. Use this only for genuinely constant rates (e.g.
    /// pegged currencies or deterministic tests).
    ///
    /// For a rate that should vary across a time series, do **not** call this:
    /// either rely on the date-aware provider, or seed a specific date with
    /// [`set_quote_on`](Self::set_quote_on), which is scoped by `(on, policy)`.
    ///
    /// The quote is stored in a **non-evicting** map, so a seeded peg is never
    /// silently dropped under cache pressure (the bounded LRU governs only the
    /// transient provider-observed cache).
    ///
    /// Note: This does not automatically insert a reciprocal. Lookups will use
    /// the reciprocal on demand if the opposite direction is requested.
    ///
    /// # Parameters
    /// - `from`: base currency for the quote
    /// - `to`: quote currency
    /// - `rate`: raw FX rate (`from → to`)
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
    ///         Ok(1.2)
    ///     }
    /// }
    ///
    /// let matrix = FxMatrix::new(Arc::new(StaticFx));
    /// matrix.set_quote(Currency::GBP, Currency::USD, 1.3)
    ///     .expect("finite, positive explicit quote");
    /// let res = matrix.rate(FxQuery::new(
    ///     Currency::GBP,
    ///     Currency::USD,
    ///     Date::from_calendar_date(2024, Month::April, 1).expect("Valid date"),
    /// )).expect("FX rate lookup should succeed");
    /// assert_eq!(res.rate, 1.3);
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if `rate` is non-finite or not strictly positive. The
    /// existing quote remains unchanged on error; no reciprocal quote is
    /// inserted automatically.
    ///
    /// # Arguments
    ///
    /// * `from` - Source currency or start identifier for the conversion or transition
    /// * `to` - Destination currency or end identifier for the conversion or transition
    /// * `rate` - Rate applied by the operation; representation and compounding follow the receiving type convention.
    pub fn set_quote(&self, from: Currency, to: Currency, rate: f64) -> crate::Result<()> {
        let rate = validate_fx_rate(from, to, rate)?;
        self.insert_quote(from, to, rate);
        Ok(())
    }

    /// Seed a quote scoped to a specific date and policy.
    ///
    /// Unlike [`set_quote`](Self::set_quote), this keys the quote by
    /// `(from, to, on, policy)`, so it only answers queries for that date and
    /// policy and does **not** shadow the provider across an entire time
    /// series. Use it to pin individual fixings while letting the provider
    /// supply every other date.
    ///
    /// The quote is stored in a dedicated, **non-evicting** pinned-quote map
    /// (separate from the bounded provider-observed cache), so it is never
    /// silently dropped under cache pressure and always wins over the provider
    /// for its `(on, policy)`. It is, however, outranked by a pair-global
    /// [`set_quote`](Self::set_quote).
    ///
    /// # Parameters
    /// - `from`: base currency for the quote
    /// - `to`: quote currency
    /// - `on`: the date the quote applies to
    /// - `policy`: the conversion policy the quote applies to
    /// - `rate`: raw FX rate (`from → to`)
    ///
    /// # Errors
    ///
    /// Returns an error if `rate` is non-finite or not strictly positive. The
    /// existing pinned fixing for this exact pair/date/policy key is unchanged
    /// on error; reciprocal lookup is derived on demand rather than stored.
    ///
    /// # Arguments
    ///
    /// * `from` - Source currency or start identifier for the conversion or transition
    /// * `to` - Destination currency or end identifier for the conversion or transition
    /// * `on` - Market date on which the quote or lookup applies.
    /// * `policy` - Policy enum controlling error handling, unmatched keys, or fallbacks
    /// * `rate` - Rate applied by the operation; representation and compounding follow the receiving type convention.
    pub fn set_quote_on(
        &self,
        from: Currency,
        to: Currency,
        on: Date,
        policy: FxConversionPolicy,
        rate: f64,
    ) -> crate::Result<()> {
        let rate = validate_fx_rate(from, to, rate)?;
        self.insert_pinned_quote(from, to, on, policy, rate);
        Ok(())
    }

    /// Seed multiple quotes at once.
    ///
    /// # Parameters
    /// - `quotes`: slice of `(from, to, rate)` tuples
    ///
    /// This is an atomic batch with respect to the pair-global quote map: every
    /// input is validated before any replacement occurs. Like
    /// [`set_quote`](Self::set_quote),
    /// these quotes shadow the provider for every date and policy and do not
    /// expire under LRU pressure.
    ///
    /// # Errors
    ///
    /// Returns an error if any rate is non-finite or not strictly positive. In
    /// that case the matrix retains every pre-existing explicit quote and none
    /// of the batch is applied.
    pub fn set_quotes(&self, quotes: &[(Currency, Currency, f64)]) -> crate::Result<()> {
        for &(from, to, rate) in quotes {
            validate_fx_rate(from, to, rate)?;
        }
        let mut map = lock(&self.quotes);
        for &(from, to, rate) in quotes {
            map.insert(Pair(from, to), rate);
        }
        self.quote_revision.fetch_add(1, Ordering::AcqRel);
        drop(map);
        self.invalidate_observed_quotes();
        Ok(())
    }

    /// Clear all stored quotes.
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
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
    /// matrix.clear_cache();
    /// ```
    pub fn clear_cache(&self) {
        {
            let mut quotes = lock(&self.quotes);
            let mut pinned = lock(&self.pinned_quotes);
            quotes.clear();
            pinned.clear();
            self.quote_revision.fetch_add(1, Ordering::AcqRel);
        }
        self.invalidate_observed_quotes();
    }

    /// Return the number of authoritative quotes and current observations.
    ///
    /// Observations from an earlier provider revision are excluded even when
    /// their storage has not yet been reclaimed by the bounded cache.
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
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
    /// assert_eq!(matrix.cache_stats(), 0);
    /// ```
    pub fn cache_stats(&self) -> usize {
        let quotes = lock(&self.quotes);
        let revision = self.observation_revision();
        let observed_quotes = lock(&self.observed_quotes);
        // Authoritative pinned fixings remain valid across provider updates.
        let pinned_quotes = lock(&self.pinned_quotes);
        quotes.len()
            + observed_quotes
                .iter()
                .filter(|(_, quote)| Some(quote.revision) == revision)
                .count()
            + pinned_quotes.len()
    }

    /// Extract serializable state from the matrix.
    ///
    /// Returns the configuration and current quotes that can be persisted.
    /// Capturing the state performs no live provider lookup. Serializing the
    /// returned state rejects non-finite or non-positive captured rates,
    /// including a shock that overflows after a mutable provider update.
    /// Provider and matrix revisions are checked across all snapshot hooks;
    /// updates during capture trigger up to three attempts.
    ///
    /// # Errors
    ///
    /// Returns a validation error when repeated updates prevent a coherent
    /// snapshot, or when a provider exposes snapshot quotes without a revision.
    /// A provider without snapshot quotes can still persist matrix overrides.
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_quant_core::money::fx::{FxMatrix, FxProvider, FxConversionPolicy};
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
    /// let state = matrix.get_serializable_state().expect("stable FX state");
    /// assert!(state.quotes.is_empty());
    /// ```
    pub fn get_serializable_state(&self) -> crate::Result<FxMatrixState> {
        for _ in 0..MAX_STATE_ATTEMPTS {
            let revision = self.state_revision();
            let state = self.capture_state_once();
            if revision != self.state_revision() {
                continue;
            }
            if revision.1.is_none()
                && (!state.provider_quotes.is_empty() || !state.provider_pinned_quotes.is_empty())
            {
                return Err(crate::Error::Validation(
                    "FX providers with snapshot quotes must expose a revision".into(),
                ));
            }
            return Ok(state);
        }
        Err(crate::Error::Validation(
            "FX quote state changed repeatedly during snapshot capture".into(),
        ))
    }

    fn capture_state_once(&self) -> FxMatrixState {
        let mut quote_vec: Vec<(Currency, Currency, f64)> = Vec::new();

        // Explicit pair-global quotes take precedence.
        {
            let quotes = lock(&self.quotes);
            for (pair, rate) in quotes.iter() {
                quote_vec.push((pair.0, pair.1, *rate));
            }
        }

        // Provider snapshots retain their lower authority after restoration.
        let mut provider_quotes = self.provider.snapshot_quotes();

        // Providers can preserve statically derived shocked crosses in their
        // original date/policy scope without pinning the live bumped matrix.
        let mut provider_pinned_quotes = self.provider.snapshot_pinned_quotes();
        // Keep explicit fixings in their own store so later matrix overrides
        // also outrank synthetic provider crosses after restoration.
        let mut pinned_vec: Vec<_> = {
            let pinned = lock(&self.pinned_quotes);
            pinned
                .iter()
                .map(|(key, &rate)| (key.from, key.to, key.on, key.policy, rate))
                .collect()
        };

        // Deterministic snapshots: sort by pair key, not by LRU order.
        quote_vec.sort_by_key(|quote| (quote.0, quote.1));
        provider_quotes.sort_by_key(|quote| (quote.0, quote.1));
        provider_pinned_quotes.sort_by_key(|q| (q.0, q.1, q.2, q.3 as u8));
        pinned_vec.sort_by_key(|q| (q.0, q.1, q.2, q.3 as u8));
        FxMatrixState {
            config: self.config,
            quotes: quote_vec,
            provider_quotes,
            provider_pinned_quotes,
            pinned_quotes: pinned_vec,
        }
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
    /// * `state` - Serializable state used to restore the documented object.
    pub fn load_from_state(&self, state: &FxMatrixState) -> crate::Result<()> {
        self.set_quotes(&state.quotes)?;
        // Restore pinned (date/policy-scoped) fixings so they keep outranking
        // the provider after a snapshot/restore round-trip.
        for &(from, to, on, policy, rate) in &state.pinned_quotes {
            self.set_quote_on(from, to, on, policy, rate)?;
        }
        Ok(())
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
        let query = FxQuery::new(from, to, on);
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
                pair: Pair(from, to),
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
                let rate = if pair.0 == from && pair.1 == to {
                    *rate * multiplier
                } else if pair.0 == to && pair.1 == from {
                    *rate / multiplier
                } else {
                    *rate
                };
                let rate = validate_fx_rate(pair.0, pair.1, rate)?;
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
                let rate = if key.from == from && key.to == to {
                    *rate * multiplier
                } else if key.from == to && key.to == from {
                    *rate / multiplier
                } else {
                    *rate
                };
                let rate = validate_fx_rate(key.from, key.to, rate)?;
                dst.insert(*key, rate);
            }
        }
        // Do not carry over provider-observed quotes. They may be date/policy-sensitive
        // or derived crosses that depend transitively on the bumped leg.

        Ok(bumped)
    }

    /// Validate stored FX quotes for reciprocal and triangular consistency.
    ///
    /// Pair-global quotes are checked together. Provider-observed and pinned
    /// quotes are checked only within the same `(date, policy)` snapshot, then
    /// combined with pair-global quotes; this avoids false arbitrage signals
    /// from comparing moving market data across dates. `tolerance_bp` is the
    /// permitted absolute deviation of a reciprocal or three-leg cycle product
    /// from one, expressed in basis points.
    ///
    /// # Errors
    ///
    /// Returns a validation error when `tolerance_bp` is non-finite or
    /// negative, when a reciprocal product differs from one beyond tolerance,
    /// or when a three-currency cycle implies triangular arbitrage beyond
    /// tolerance. It validates the current stored snapshot only and never
    /// performs provider I/O.
    pub fn validate_triangular(&self, tolerance_bp: f64) -> crate::Result<()> {
        if !tolerance_bp.is_finite() || tolerance_bp < 0.0 {
            return Err(crate::Error::Validation(format!(
                "triangular validation tolerance must be finite and non-negative, got {tolerance_bp}"
            )));
        }

        let global: HashMap<Pair, f64> = lock(&self.quotes).clone();
        validate_fx_snapshot(
            &global,
            &HashMap::default(),
            &HashMap::default(),
            tolerance_bp,
            "global",
        )?;

        // Date/policy-sensitive quotes must only be compared within the same
        // market snapshot. Combining observations from different dates creates
        // false arbitrage signals in a moving FX market.
        type Scope = (Date, FxConversionPolicy);
        type ScopedRates = (HashMap<Pair, f64>, HashMap<Pair, f64>);
        let mut scoped: HashMap<Scope, ScopedRates> = HashMap::default();
        let revision = self.observation_revision();
        {
            let observed = lock(&self.observed_quotes);
            for (query, quote) in observed.iter() {
                if Some(quote.revision) == revision && quote.rate.is_finite() && quote.rate > 0.0 {
                    scoped
                        .entry((query.on, query.policy))
                        .or_default()
                        .1
                        .insert(Pair(query.from, query.to), quote.rate);
                }
            }
        }
        {
            let pinned = lock(&self.pinned_quotes);
            for (query, &rate) in pinned.iter() {
                if rate.is_finite() && rate > 0.0 {
                    scoped
                        .entry((query.on, query.policy))
                        .or_default()
                        .0
                        .insert(Pair(query.from, query.to), rate);
                }
            }
        }

        let mut scopes: Vec<_> = scoped.into_iter().collect();
        scopes.sort_by_key(|((date, policy), _)| (*date, *policy as u8));
        for ((date, policy), (pinned, observed)) in scopes {
            validate_fx_snapshot(
                &global,
                &pinned,
                &observed,
                tolerance_bp,
                &format!("{date}/{policy}"),
            )?;
        }
        Ok(())
    }

    /// Attempt to triangulate FX rate via the single configured pivot currency.
    ///
    /// This is intentionally a one-pivot fallback, not a general graph search.
    /// It keeps lookup behavior deterministic and auditable, but it can miss
    /// valid market crosses that would require a different routing currency.
    fn triangulate_rate(
        &self,
        from: Currency,
        to: Currency,
        on: Date,
        policy: FxConversionPolicy,
    ) -> crate::Result<f64> {
        use crate::error::InputError;

        let revision = self.observation_revision();
        let pivot = self.config.pivot_currency;

        // Try to get first leg: from -> pivot
        let Ok(a) = self.get_or_fetch(from, pivot, on, policy) else {
            return Err(InputError::FxTriangulationFailed {
                from,
                to,
                pivot,
                missing_leg: format!("{from}->{pivot} rate not found"),
            }
            .into());
        };

        // Try to get second leg: pivot -> to
        let Ok(b) = self.get_or_fetch(pivot, to, on, policy) else {
            return Err(InputError::FxTriangulationFailed {
                from,
                to,
                pivot,
                missing_leg: format!("{pivot}->{to} rate not found"),
            }
            .into());
        };

        let rate = a * b;
        let rate = validate_fx_rate(from, to, rate)?;
        // Cache the derived rate together with its triangulated provenance so
        // repeat queries stamp the same metadata and value as the first lookup.
        self.insert_observed_quote(
            FxQuery::with_policy(from, to, on, policy),
            rate,
            true,
            revision,
        )
    }

    /// Insert an explicit provider quote
    fn insert_quote(&self, from: Currency, to: Currency, rate: f64) {
        // Internal insertion should never persist invalid rates.
        let checked = validate_fx_rate(from, to, rate);
        assert!(
            checked.is_ok(),
            "FxMatrix internal quote must be finite, positive (got {from}->{to}={rate})"
        );
        {
            let mut quotes = lock(&self.quotes);
            quotes.insert(Pair(from, to), rate);
            self.quote_revision.fetch_add(1, Ordering::AcqRel);
        }
        self.invalidate_observed_quotes();
    }

    fn invalidate_observed_quotes(&self) {
        // Publish the revision while the authoritative store is still locked.
        // Cache reclamation follows after releasing that store, avoiding an
        // observed-cache -> authoritative-store lock dependency.
        lock(&self.observed_quotes).clear();
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

    /// Insert a provider-observed quote in the pair's canonical orientation.
    ///
    /// Returns the rate as future cache reads will serve it in the requested
    /// direction, keeping cold and warm lookups bit-identical.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid input rate or an invalid reciprocal.
    fn insert_observed_quote(
        &self,
        query: FxQuery,
        rate: f64,
        triangulated: bool,
        observed_at: Option<ObservationRevision>,
    ) -> crate::Result<f64> {
        let FxQuery {
            from,
            to,
            on,
            policy,
        } = query;
        let rate = validate_fx_rate(from, to, rate)?;
        let (base, quote, inverted) = canonical_orientation(from, to);
        let (stored, served) = if inverted {
            let stored = reciprocal_rate_or_err(rate, from, to)?;
            let served = reciprocal_rate_or_err(stored, base, quote)?;
            (stored, served)
        } else {
            (rate, rate)
        };

        let mut quotes = lock(&self.observed_quotes);
        if let Some(revision) =
            observed_at.filter(|&revision| Some(revision) == self.observation_revision())
        {
            quotes.put(
                QueryKey {
                    from: base,
                    to: quote,
                    on,
                    policy,
                },
                ObservedQuote {
                    rate: stored,
                    triangulated,
                    revision,
                },
            );
        }
        Ok(served)
    }

    /// Insert an authoritative, non-evicting pinned quote (via `set_quote_on`).
    /// The rate is validated by the caller.
    fn insert_pinned_quote(
        &self,
        from: Currency,
        to: Currency,
        on: Date,
        policy: FxConversionPolicy,
        rate: f64,
    ) {
        {
            let mut pinned = lock(&self.pinned_quotes);
            pinned.insert(
                QueryKey {
                    from,
                    to,
                    on,
                    policy,
                },
                rate,
            );
            self.quote_revision.fetch_add(1, Ordering::AcqRel);
        }
        self.invalidate_observed_quotes();
    }

    /// Read pinned `(direct, reciprocal)` quotes for a pair on `(on, policy)`.
    /// Never evicts; only positive, finite rates are returned.
    fn read_pinned_pair_bidir(
        &self,
        from: Currency,
        to: Currency,
        on: Date,
        policy: FxConversionPolicy,
    ) -> (Option<f64>, Option<f64>) {
        let quotes = lock(&self.pinned_quotes);
        let direct = quotes
            .get(&QueryKey {
                from,
                to,
                on,
                policy,
            })
            .copied()
            .filter(|r| r.is_finite() && *r > 0.0);
        let rev = quotes
            .get(&QueryKey {
                from: to,
                to: from,
                on,
                policy,
            })
            .copied()
            .filter(|r| r.is_finite() && *r > 0.0);
        (direct, rev)
    }

    /// Get a single-leg rate using the same precedence as [`rate`](Self::rate):
    /// explicit → pinned → observed → provider (each with reciprocal fallback).
    ///
    /// Pinned fixings are authoritative for their `(on, policy)`, so triangulated
    /// crosses must honor them exactly like direct lookups do — otherwise a
    /// cross would contradict a pinned leg (internal triangular arbitrage), and
    /// triangulation would fail when the only source for a leg is a pinned quote.
    fn get_or_fetch(
        &self,
        from: Currency,
        to: Currency,
        on: Date,
        policy: FxConversionPolicy,
    ) -> crate::Result<f64> {
        if from == to {
            return Ok(1.0);
        }
        if let Some(cached) = self.read_cached_rate(from, to, on, policy)? {
            return Ok(cached.rate);
        }
        // Fetch from the provider
        let revision = self.observation_revision();
        let r = self.provider.rate(from, to, on, policy)?;
        let r = validate_fx_rate(from, to, r)?;
        self.insert_observed_quote(
            FxQuery::with_policy(from, to, on, policy),
            r,
            false,
            revision,
        )
    }

    /// Resolve cache source priority before pair orientation for every lookup path.
    fn read_cached_rate(
        &self,
        from: Currency,
        to: Currency,
        on: Date,
        policy: FxConversionPolicy,
    ) -> crate::Result<Option<FxRateResult>> {
        // Check caches in precedence order, locking each store lazily so the common
        // case (an explicit direct quote, e.g. a pegged pair) acquires a single lock
        // instead of all three. Explicit quotes are pair-global, while provider-observed
        // quotes are scoped by date/policy to avoid cross-query contamination.
        let (direct_opt, reciprocal_opt) = self.read_cached_pair_bidir(from, to);
        if let Some(rate) = direct_opt {
            let rate = validate_fx_rate(from, to, rate)?;
            return Ok(Some(FxRateResult {
                rate,
                triangulated: false,
            }));
        }

        if let Some(r_rev) = reciprocal_opt {
            return Ok(Some(FxRateResult {
                rate: reciprocal_rate_or_err(r_rev, to, from)?,
                triangulated: false,
            }));
        }

        // Pinned fixings outrank provider observations for their `(on, policy)`.
        let (pinned_direct_opt, pinned_reciprocal_opt) =
            self.read_pinned_pair_bidir(from, to, on, policy);
        if let Some(rate) = pinned_direct_opt {
            let rate = validate_fx_rate(from, to, rate)?;
            return Ok(Some(FxRateResult {
                rate,
                triangulated: false,
            }));
        }
        if let Some(r_rev) = pinned_reciprocal_opt {
            return Ok(Some(FxRateResult {
                rate: reciprocal_rate_or_err(r_rev, to, from)?,
                triangulated: false,
            }));
        }

        if let Some(q) = self.read_observed(from, to, on, policy)? {
            let rate = validate_fx_rate(from, to, q.rate)?;
            return Ok(Some(FxRateResult {
                rate,
                triangulated: q.triangulated,
            }));
        }

        Ok(None)
    }

    /// Read direct and reciprocal cached quotes for a pair under a single lock.
    #[inline]
    fn read_cached_pair_bidir(&self, from: Currency, to: Currency) -> (Option<f64>, Option<f64>) {
        let mut quotes = lock(&self.quotes);
        let direct_key = Pair(from, to);
        let rev_key = Pair(to, from);

        let direct = quotes.get(&direct_key).copied().and_then(|r| {
            if r.is_finite() && r > 0.0 {
                Some(r)
            } else {
                // Purge invalid cached value.
                let _ = quotes.remove(&direct_key);
                None
            }
        });
        let rev = quotes.get(&rev_key).copied().and_then(|r| {
            if r.is_finite() && r > 0.0 {
                Some(r)
            } else {
                let _ = quotes.remove(&rev_key);
                None
            }
        });
        (direct, rev)
    }

    /// Read the canonically stored observed quote in the requested direction.
    ///
    /// # Errors
    ///
    /// Returns an error if resolving the reciprocal yields an invalid FX rate.
    #[inline]
    fn read_observed(
        &self,
        from: Currency,
        to: Currency,
        on: Date,
        policy: FxConversionPolicy,
    ) -> crate::Result<Option<ObservedQuote>> {
        let Some(revision) = self.observation_revision() else {
            return Ok(None);
        };
        let (base, quote, inverted) = canonical_orientation(from, to);
        let key = QueryKey {
            from: base,
            to: quote,
            on,
            policy,
        };
        let mut quotes = lock(&self.observed_quotes);
        let Some(observed) = quotes.get(&key).copied() else {
            return Ok(None);
        };
        if observed.revision != revision || !observed.rate.is_finite() || observed.rate <= 0.0 {
            let _ = quotes.pop(&key);
            return Ok(None);
        }
        drop(quotes);

        if inverted {
            Ok(Some(ObservedQuote {
                rate: reciprocal_rate_or_err(observed.rate, base, quote)?,
                triangulated: observed.triangulated,
                revision: observed.revision,
            }))
        } else {
            Ok(Some(observed))
        }
    }
}

#[cfg(test)]
mod revision_tests {
    use super::*;
    use crate::money::fx::SimpleFxProvider;

    #[test]
    fn quote_mutations_publish_revision_before_waiting_for_cache_reclamation() {
        let on = Date::from_ordinal_date(2025, 1).expect("date");
        let key = QueryKey {
            from: Currency::EUR,
            to: Currency::USD,
            on,
            policy: FxConversionPolicy::CashflowDate,
        };
        for mutation in ["single", "batch", "pinned", "clear"] {
            let matrix = FxMatrix::new(Arc::new(SimpleFxProvider::new()));
            matrix.set_quote(key.from, key.to, 1.1).expect("global");
            matrix
                .set_quote_on(key.from, key.to, on, key.policy, 1.1)
                .expect("pinned");
            let before = matrix.quote_revision.load(Ordering::Acquire);
            // Hold reclamation closed while the writer publishes its quote.
            // The authoritative store remains readable during this interval.
            let observed = lock(&matrix.observed_quotes);
            std::thread::scope(|scope| {
                let writer = scope.spawn(|| match mutation {
                    "single" => matrix.set_quote(key.from, key.to, 1.2).expect("update"),
                    "batch" => matrix
                        .set_quotes(&[(key.from, key.to, 1.2)])
                        .expect("batch update"),
                    "pinned" => matrix
                        .set_quote_on(key.from, key.to, on, key.policy, 1.2)
                        .expect("pinned update"),
                    "clear" => matrix.clear_cache(),
                    _ => unreachable!(),
                });
                loop {
                    let published = match mutation {
                        "single" | "batch" => {
                            lock(&matrix.quotes).get(&Pair(key.from, key.to)) == Some(&1.2)
                        }
                        "pinned" => lock(&matrix.pinned_quotes).get(&key) == Some(&1.2),
                        "clear" => {
                            lock(&matrix.quotes).is_empty()
                                && lock(&matrix.pinned_quotes).is_empty()
                        }
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
