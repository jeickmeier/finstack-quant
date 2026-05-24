use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;
use std::sync::Arc;

use lru::LruCache;
use parking_lot::Mutex;

use crate::currency::Currency;
use crate::dates::Date;

use super::provider::{reciprocal_rate_or_err, validate_fx_rate, FxProvider};
use super::types::{FxConfig, FxConversionPolicy, FxMatrixState, FxQuery, FxRateResult};

/// Pair key for the explicit-quote cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Pair(Currency, Currency);

/// Query-sensitive key for the provider-observed quote cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct QueryKey {
    from: Currency,
    to: Currency,
    on: Date,
    policy: FxConversionPolicy,
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
    /// Explicit quotes inserted by callers or restored from serialized state.
    quotes: Mutex<LruCache<Pair, f64>>,
    /// Query-sensitive quotes observed from providers or triangulation.
    observed_quotes: Mutex<LruCache<QueryKey, f64>>,
    config: FxConfig,
}

impl FxMatrix {
    /// Create a new [`FxMatrix`] wrapping the given provider with the default configuration.
    ///
    /// # Parameters
    /// - `provider`: FX quote source implementing [`FxProvider`]
    ///
    /// # Examples
    /// ```rust
    /// use finstack_core::money::fx::{FxMatrix, FxProvider, FxConversionPolicy};
    /// use finstack_core::currency::Currency;
    /// use finstack_core::dates::Date;
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
    ///     ) -> finstack_core::Result<f64> {
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
        Self {
            provider,
            quotes: Mutex::new(LruCache::new(capacity)),
            observed_quotes: Mutex::new(LruCache::new(capacity)),
            config,
        }
    }

    /// Create a new [`FxMatrix`] with custom configuration, failing closed on invalid inputs.
    pub fn try_with_config(provider: Arc<dyn FxProvider>, config: FxConfig) -> crate::Result<Self> {
        if config.cache_capacity == 0 {
            return Err(crate::Error::Validation(
                "FxConfig.cache_capacity must be > 0".to_string(),
            ));
        }
        let capacity = NonZeroUsize::new(config.cache_capacity).unwrap_or(NonZeroUsize::MIN);
        let quotes = LruCache::new(capacity);
        let observed_quotes = LruCache::new(capacity);
        Ok(Self {
            provider,
            quotes: Mutex::new(quotes),
            observed_quotes: Mutex::new(observed_quotes),
            config,
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
    /// use finstack_core::money::fx::{FxMatrix, FxProvider, FxConversionPolicy, FxQuery};
    /// use finstack_core::currency::Currency;
    /// use finstack_core::dates::Date;
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
    ///     ) -> finstack_core::Result<f64> {
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
    pub fn rate(&self, query: FxQuery) -> crate::Result<FxRateResult> {
        let from = query.from;
        let to = query.to;
        let on = query.on;
        let policy = query.policy;

        // Handle identity case
        if from == to {
            return Ok(FxRateResult {
                rate: 1.0,
                triangulated: false,
            });
        }

        // Check cache first. Explicit quotes are pair-global, while provider-observed
        // quotes are scoped by date/policy to avoid cross-query contamination.
        let (direct_opt, reciprocal_opt) = self.read_cached_pair_bidir(from, to);
        let (observed_direct_opt, observed_reciprocal_opt) =
            self.read_observed_pair_bidir(from, to, on, policy);

        if let Some(rate) = direct_opt {
            let rate = validate_fx_rate(from, to, rate)?;
            return Ok(FxRateResult {
                rate,
                triangulated: false,
            });
        }
        if let Some(r_rev) = reciprocal_opt {
            return Ok(FxRateResult {
                rate: reciprocal_rate_or_err(r_rev, to, from)?,
                triangulated: false,
            });
        }
        if let Some(rate) = observed_direct_opt {
            let rate = validate_fx_rate(from, to, rate)?;
            return Ok(FxRateResult {
                rate,
                triangulated: false,
            });
        }
        if let Some(r_rev) = observed_reciprocal_opt {
            return Ok(FxRateResult {
                rate: reciprocal_rate_or_err(r_rev, to, from)?,
                triangulated: false,
            });
        }

        // Try provider first
        match self.provider.rate(from, to, on, policy) {
            Ok(rate) => {
                let rate = validate_fx_rate(from, to, rate)?;
                self.insert_observed_quote(from, to, on, policy, rate);
                Ok(FxRateResult {
                    rate,
                    triangulated: false,
                })
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

    /// Seed or update a single quote directly inside the matrix.
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
    /// use finstack_core::money::fx::{FxMatrix, FxProvider, FxConversionPolicy, FxQuery};
    /// use finstack_core::currency::Currency;
    /// use finstack_core::dates::Date;
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
    ///     ) -> finstack_core::Result<f64> {
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
    pub fn set_quote(&self, from: Currency, to: Currency, rate: f64) -> crate::Result<()> {
        let rate = validate_fx_rate(from, to, rate)?;
        self.insert_quote(from, to, rate);
        Ok(())
    }

    /// Seed multiple quotes at once.
    ///
    /// # Parameters
    /// - `quotes`: slice of `(from, to, rate)` tuples
    pub fn set_quotes(&self, quotes: &[(Currency, Currency, f64)]) -> crate::Result<()> {
        let mut map = self.quotes.lock();
        for &(from, to, rate) in quotes {
            validate_fx_rate(from, to, rate)?;
            map.put(Pair(from, to), rate);
        }
        Ok(())
    }

    /// Clear all stored quotes.
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
    /// # use finstack_core::currency::Currency;
    /// # use finstack_core::dates::Date;
    /// # use std::sync::Arc;
    /// # use time::Month;
    /// # struct StaticFx;
    /// # impl FxProvider for StaticFx {
    /// #     fn rate(&self, _from: Currency, _to: Currency, _on: Date, _policy: FxConversionPolicy)
    /// #         -> finstack_core::Result<f64> { Ok(1.0) }
    /// # }
    /// let matrix = FxMatrix::new(Arc::new(StaticFx));
    /// matrix.clear_cache();
    /// ```
    pub fn clear_cache(&self) {
        self.quotes.lock().clear();
        self.observed_quotes.lock().clear();
    }

    /// Return cached quote count for quick diagnostics.
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
    /// # use finstack_core::currency::Currency;
    /// # use finstack_core::dates::Date;
    /// # use std::sync::Arc;
    /// # use time::Month;
    /// # struct StaticFx;
    /// # impl FxProvider for StaticFx {
    /// #     fn rate(&self, _from: Currency, _to: Currency, _on: Date, _policy: FxConversionPolicy)
    /// #         -> finstack_core::Result<f64> { Ok(1.0) }
    /// # }
    /// let matrix = FxMatrix::new(Arc::new(StaticFx));
    /// assert_eq!(matrix.cache_stats(), 0);
    /// ```
    pub fn cache_stats(&self) -> usize {
        let quotes = self.quotes.lock();
        let observed_quotes = self.observed_quotes.lock();
        quotes.len() + observed_quotes.len()
    }

    /// Extract serializable state from the matrix.
    ///
    /// Returns the configuration and current quotes that can be persisted.
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_core::money::fx::{FxMatrix, FxProvider, FxConversionPolicy};
    /// # use finstack_core::currency::Currency;
    /// # use finstack_core::dates::Date;
    /// # use std::sync::Arc;
    /// # use time::Month;
    /// # struct StaticFx;
    /// # impl FxProvider for StaticFx {
    /// #     fn rate(&self, _from: Currency, _to: Currency, _on: Date, _policy: FxConversionPolicy)
    /// #         -> finstack_core::Result<f64> { Ok(1.0) }
    /// # }
    /// let matrix = FxMatrix::new(Arc::new(StaticFx));
    /// let state = matrix.get_serializable_state();
    /// assert!(state.quotes.is_empty());
    /// ```
    pub fn get_serializable_state(&self) -> FxMatrixState {
        let mut seen = std::collections::HashSet::new();
        let mut quote_vec: Vec<(Currency, Currency, f64)> = Vec::new();

        // Explicit LRU quotes take precedence.
        {
            let quotes = self.quotes.lock();
            for (pair, rate) in quotes.iter() {
                seen.insert((pair.0, pair.1));
                quote_vec.push((pair.0, pair.1, *rate));
            }
        }

        // Merge quotes from the underlying provider that were not already
        // present in the LRU cache (e.g. SimpleFxProvider's own store).
        for (from, to, rate) in self.provider.snapshot_quotes() {
            if seen.insert((from, to)) {
                quote_vec.push((from, to, rate));
            }
        }

        // Deterministic snapshots: sort by pair key, not by LRU order.
        quote_vec.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
        FxMatrixState {
            config: self.config,
            quotes: quote_vec,
        }
    }

    /// Load quotes from a serialized state.
    ///
    /// This allows restoring cached quotes after deserialization.
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_core::money::fx::{FxMatrix, FxProvider, FxConversionPolicy, FxMatrixState};
    /// # use finstack_core::currency::Currency;
    /// # use finstack_core::dates::Date;
    /// # use std::sync::Arc;
    /// # use time::Month;
    /// # struct StaticFx;
    /// # impl FxProvider for StaticFx {
    /// #     fn rate(&self, _from: Currency, _to: Currency, _on: Date, _policy: FxConversionPolicy)
    /// #         -> finstack_core::Result<f64> { Ok(1.0) }
    /// # }
    /// let matrix = FxMatrix::new(Arc::new(StaticFx));
    /// let state = FxMatrixState { config: matrix.get_serializable_state().config, quotes: vec![] };
    /// matrix.load_from_state(&state).expect("valid snapshot state");
    /// ```
    pub fn load_from_state(&self, state: &FxMatrixState) -> crate::Result<()> {
        self.set_quotes(&state.quotes)
    }

    /// Create a new FX matrix with a bumped rate for a specific currency pair.
    ///
    /// This is useful for finite difference greek calculations where we need
    /// to bump FX spot while preserving all other market data. Creates a
    /// wrapper provider that overrides the specified rate.
    ///
    /// # Parameters
    /// - `from`: Base currency
    /// - `to`: Quote currency
    /// - `bump_pct`: Relative bump size (e.g., 0.01 for 1% increase)
    /// - `on`: Date for rate lookup (typically as_of date from valuation context)
    ///
    /// # Returns
    /// New FxMatrix with bumped rate
    ///
    /// # Errors
    /// Returns error if rate lookup fails
    pub fn with_bumped_rate(
        &self,
        from: Currency,
        to: Currency,
        bump_pct: f64,
        on: Date,
    ) -> crate::Result<Self> {
        // Get current rate
        let query = FxQuery::new(from, to, on);
        let current_rate = self.rate(query)?.rate;

        // Calculate bumped rate
        let bumped_rate = validate_fx_rate(from, to, current_rate * (1.0 + bump_pct))?;

        // Create bumped provider
        use super::providers::BumpedFxProvider;
        use std::sync::Arc;
        let bumped_provider = Arc::new(BumpedFxProvider::new(
            self.provider.clone(),
            from,
            to,
            bumped_rate,
        ));

        // Create new FX matrix with same config and carry over cached quotes so lookups that
        // rely on seeded values keep working after the bump.
        let bumped = Self::try_with_config(bumped_provider, self.config)?;
        {
            let src = self.quotes.lock();
            let mut dst = bumped.quotes.lock();
            for (pair, rate) in src.iter() {
                if (pair.0 == from && pair.1 == to) || (pair.0 == to && pair.1 == from) {
                    continue;
                }
                dst.put(*pair, *rate);
            }
        }
        // Do not carry over provider-observed quotes. They may be date/policy-sensitive
        // or derived crosses that depend transitively on the bumped leg.
        // IMPORTANT: ensure the bumped pair is not shadowed by copied explicit quotes.
        bumped.set_quote(from, to, bumped_rate)?;

        Ok(bumped)
    }

    /// Validate that stored FX quotes do not imply triangular arbitrage beyond a tolerance.
    pub fn validate_triangular(&self, tolerance_bps: f64) -> crate::Result<()> {
        if !tolerance_bps.is_finite() || tolerance_bps < 0.0 {
            return Err(crate::Error::Validation(format!(
                "triangular validation tolerance must be finite and non-negative, got {tolerance_bps}"
            )));
        }

        let mut rates: HashMap<(Currency, Currency), f64> = HashMap::new();
        let mut currencies: HashSet<Currency> = HashSet::new();

        {
            let quotes = self.quotes.lock();
            for (pair, &rate) in quotes.iter() {
                if rate.is_finite() && rate > 0.0 {
                    rates.entry((pair.0, pair.1)).or_insert(rate);
                    currencies.insert(pair.0);
                    currencies.insert(pair.1);
                }
            }
        }

        {
            let quotes = self.observed_quotes.lock();
            for (query, &rate) in quotes.iter() {
                if rate.is_finite() && rate > 0.0 {
                    rates.entry((query.from, query.to)).or_insert(rate);
                    currencies.insert(query.from);
                    currencies.insert(query.to);
                }
            }
        }

        let currencies: Vec<Currency> = currencies.into_iter().collect();
        for &a in &currencies {
            for &b in &currencies {
                if a == b {
                    continue;
                }
                for &c in &currencies {
                    if a == c || b == c {
                        continue;
                    }

                    let (Some(&ab), Some(&bc), Some(&ca)) =
                        (rates.get(&(a, b)), rates.get(&(b, c)), rates.get(&(c, a)))
                    else {
                        continue;
                    };

                    let cycle_product = ab * bc * ca;
                    let deviation_bps = (cycle_product - 1.0).abs() * 10_000.0;
                    if deviation_bps > tolerance_bps {
                        return Err(crate::Error::Validation(format!(
                            "triangular arbitrage detected for {a}->{b}->{c}->{a}: cycle product {cycle_product:.12} ({deviation_bps:.6} bps)"
                        )));
                    }
                }
            }
        }

        Ok(())
    }

    // Private helper methods

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

        let pivot = self.config.pivot_currency;

        // Try to get first leg: from -> pivot
        let a = match self.get_or_fetch(from, pivot, on, policy) {
            Ok(rate) => rate,
            Err(_) => {
                return Err(InputError::FxTriangulationFailed {
                    from,
                    to,
                    pivot,
                    missing_leg: format!("{from}->{pivot} rate not found"),
                }
                .into());
            }
        };

        // Try to get second leg: pivot -> to
        let b = match self.get_or_fetch(pivot, to, on, policy) {
            Ok(rate) => rate,
            Err(_) => {
                return Err(InputError::FxTriangulationFailed {
                    from,
                    to,
                    pivot,
                    missing_leg: format!("{pivot}->{to} rate not found"),
                }
                .into());
            }
        };

        let rate = a * b;
        let rate = validate_fx_rate(from, to, rate)?;
        self.insert_observed_quote(from, to, on, policy, rate);
        Ok(rate)
    }

    /// Insert an explicit provider quote
    fn insert_quote(&self, from: Currency, to: Currency, rate: f64) {
        // Internal insertion should never persist invalid rates.
        let checked = validate_fx_rate(from, to, rate);
        assert!(
            checked.is_ok(),
            "FxMatrix internal quote must be finite, positive (got {from}->{to}={rate})"
        );
        let mut quotes = self.quotes.lock();
        quotes.put(Pair(from, to), rate);
    }

    /// Insert a query-sensitive provider-observed quote.
    fn insert_observed_quote(
        &self,
        from: Currency,
        to: Currency,
        on: Date,
        policy: FxConversionPolicy,
        rate: f64,
    ) {
        let checked = validate_fx_rate(from, to, rate);
        assert!(
            checked.is_ok(),
            "FxMatrix observed quote must be finite, positive (got {from}->{to}={rate})"
        );
        let mut quotes = self.observed_quotes.lock();
        quotes.put(
            QueryKey {
                from,
                to,
                on,
                policy,
            },
            rate,
        );
    }

    /// Get rate preferring explicit quotes, then provider, then reciprocal.
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
        // Read both direct and reciprocal under a single lock, then drop before any further work
        let (direct_opt, reciprocal_opt) = self.read_cached_pair_bidir(from, to);
        let (observed_direct_opt, observed_reciprocal_opt) =
            self.read_observed_pair_bidir(from, to, on, policy);
        // 1) Explicit quote wins
        if let Some(r) = direct_opt {
            return validate_fx_rate(from, to, r);
        }
        if let Some(r) = observed_direct_opt {
            return validate_fx_rate(from, to, r);
        }
        // 2) Try provider for direct
        let provider_direct = self.provider.rate(from, to, on, policy);
        if let Ok(r) = provider_direct {
            let r = validate_fx_rate(from, to, r)?;
            self.insert_observed_quote(from, to, on, policy, r);
            return Ok(r);
        }
        // 3) Reciprocal fallback if available
        if let Some(r_rev) = reciprocal_opt {
            return reciprocal_rate_or_err(r_rev, to, from);
        }
        if let Some(r_rev) = observed_reciprocal_opt {
            return reciprocal_rate_or_err(r_rev, to, from);
        }
        // 4) As last resort, propagate provider error
        provider_direct
    }

    /// Read direct and reciprocal cached quotes for a pair under a single lock.
    #[inline]
    fn read_cached_pair_bidir(&self, from: Currency, to: Currency) -> (Option<f64>, Option<f64>) {
        let mut quotes = self.quotes.lock();
        let direct_key = Pair(from, to);
        let rev_key = Pair(to, from);

        let direct = quotes.get(&direct_key).copied().and_then(|r| {
            if r.is_finite() && r > 0.0 {
                Some(r)
            } else {
                // Purge invalid cached value.
                let _ = quotes.pop(&direct_key);
                None
            }
        });
        let rev = quotes.get(&rev_key).copied().and_then(|r| {
            if r.is_finite() && r > 0.0 {
                Some(r)
            } else {
                let _ = quotes.pop(&rev_key);
                None
            }
        });
        (direct, rev)
    }

    /// Read provider-observed cached quotes scoped to a specific date/policy query.
    #[inline]
    fn read_observed_pair_bidir(
        &self,
        from: Currency,
        to: Currency,
        on: Date,
        policy: FxConversionPolicy,
    ) -> (Option<f64>, Option<f64>) {
        let mut quotes = self.observed_quotes.lock();
        let direct_key = QueryKey {
            from,
            to,
            on,
            policy,
        };
        let rev_key = QueryKey {
            from: to,
            to: from,
            on,
            policy,
        };

        let direct = quotes.get(&direct_key).copied().and_then(|r| {
            if r.is_finite() && r > 0.0 {
                Some(r)
            } else {
                let _ = quotes.pop(&direct_key);
                None
            }
        });
        let rev = quotes.get(&rev_key).copied().and_then(|r| {
            if r.is_finite() && r > 0.0 {
                Some(r)
            } else {
                let _ = quotes.pop(&rev_key);
                None
            }
        });
        (direct, rev)
    }
}
