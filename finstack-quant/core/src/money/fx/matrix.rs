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

/// Provider-observed quote stored with its provenance, so the `triangulated`
/// flag stamped on [`FxRateResult`] does not depend on cache state/call history.
#[derive(Clone, Copy, Debug)]
struct ObservedQuote {
    rate: f64,
    triangulated: bool,
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
    observed_quotes: Mutex<LruCache<QueryKey, ObservedQuote>>,
    /// Authoritative date/policy-scoped quotes pinned via [`FxMatrix::set_quote_on`].
    /// Unlike `observed_quotes`, this map never evicts, so a pinned fixing is
    /// never silently replaced by the provider under cache pressure.
    pinned_quotes: Mutex<HashMap<QueryKey, f64>>,
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
        Self {
            provider,
            quotes: Mutex::new(HashMap::new()),
            observed_quotes: Mutex::new(LruCache::new(capacity)),
            pinned_quotes: Mutex::new(HashMap::new()),
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
        let observed_quotes = LruCache::new(capacity);
        Ok(Self {
            provider,
            quotes: Mutex::new(HashMap::new()),
            observed_quotes: Mutex::new(observed_quotes),
            pinned_quotes: Mutex::new(HashMap::new()),
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
        let (pinned_direct_opt, pinned_reciprocal_opt) =
            self.read_pinned_pair_bidir(from, to, on, policy);
        let (observed_direct_opt, observed_reciprocal_opt) =
            self.read_observed_pair_bidir(from, to, on, policy);

        if let Some(rate) = direct_opt {
            let rate = validate_fx_rate(from, to, rate)?;
            return Ok(FxRateResult {
                rate,
                triangulated: false,
            });
        }
        // Pinned fixings are authoritative and outrank the transient provider
        // cache, reciprocal pair-global quotes, and the provider itself for
        // their `(on, policy)`.
        if let Some(rate) = pinned_direct_opt {
            let rate = validate_fx_rate(from, to, rate)?;
            return Ok(FxRateResult {
                rate,
                triangulated: false,
            });
        }
        if let Some(r_rev) = pinned_reciprocal_opt {
            return Ok(FxRateResult {
                rate: reciprocal_rate_or_err(r_rev, to, from)?,
                triangulated: false,
            });
        }
        if let Some(r_rev) = reciprocal_opt {
            return Ok(FxRateResult {
                rate: reciprocal_rate_or_err(r_rev, to, from)?,
                triangulated: false,
            });
        }
        if let Some(q) = observed_direct_opt {
            let rate = validate_fx_rate(from, to, q.rate)?;
            return Ok(FxRateResult {
                rate,
                triangulated: q.triangulated,
            });
        }
        if let Some(q_rev) = observed_reciprocal_opt {
            return Ok(FxRateResult {
                rate: reciprocal_rate_or_err(q_rev.rate, to, from)?,
                triangulated: q_rev.triangulated,
            });
        }

        // Try provider first
        match self.provider.rate(from, to, on, policy) {
            Ok(rate) => {
                let rate = validate_fx_rate(from, to, rate)?;
                self.insert_observed_quote(from, to, on, policy, rate, false);
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
    pub fn set_quotes(&self, quotes: &[(Currency, Currency, f64)]) -> crate::Result<()> {
        for &(from, to, rate) in quotes {
            validate_fx_rate(from, to, rate)?;
        }
        let mut map = self.quotes.lock();
        for &(from, to, rate) in quotes {
            map.insert(Pair(from, to), rate);
        }
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
        self.quotes.lock().clear();
        self.observed_quotes.lock().clear();
        self.pinned_quotes.lock().clear();
    }

    /// Return cached quote count for quick diagnostics.
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
        let quotes = self.quotes.lock();
        let observed_quotes = self.observed_quotes.lock();
        // Pinned (date/policy-scoped) quotes are included so diagnostics
        // reflect every stored quote .
        let pinned_quotes = self.pinned_quotes.lock();
        quotes.len() + observed_quotes.len() + pinned_quotes.len()
    }

    /// Extract serializable state from the matrix.
    ///
    /// Returns the configuration and current quotes that can be persisted.
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
    /// let state = matrix.get_serializable_state();
    /// assert!(state.quotes.is_empty());
    /// ```
    pub fn get_serializable_state(&self) -> FxMatrixState {
        let mut seen = std::collections::HashSet::new();
        let mut quote_vec: Vec<(Currency, Currency, f64)> = Vec::new();

        // Explicit pair-global quotes take precedence.
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

        // Pinned (date/policy-scoped) fixings are authoritative state and must
        // survive a snapshot/restore round-trip (        // persistence previously dropped pinned fixings).
        let mut pinned_vec: Vec<(
            Currency,
            Currency,
            Date,
            super::types::FxConversionPolicy,
            f64,
        )> = {
            let pinned = self.pinned_quotes.lock();
            pinned
                .iter()
                .map(|(key, &rate)| (key.from, key.to, key.on, key.policy, rate))
                .collect()
        };

        // Deterministic snapshots: sort by pair key, not by LRU order.
        quote_vec.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
        pinned_vec.sort_by_key(|q| (q.0, q.1, q.2, q.3 as u8));
        FxMatrixState {
            config: self.config,
            quotes: quote_vec,
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
    ///     config: matrix.get_serializable_state().config,
    ///     quotes: vec![],
    ///     pinned_quotes: vec![],
    /// };
    /// matrix.load_from_state(&state).expect("valid snapshot state");
    /// ```
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
    /// date-aware provider keeps its term structure under the bump
    /// (the previous implementation froze one
    /// absolute rate for every date/policy, flattening the FX term structure).
    ///
    /// Explicit pair-global quotes and pinned (date/policy-scoped) fixings for
    /// the bumped pair are carried over **scaled by the same multiplier**
    /// (reciprocal-direction quotes are divided), so every source of the
    /// bumped pair moves coherently. Quotes for other pairs are carried over
    /// unchanged.
    ///
    /// # Parameters
    /// - `from`: Base currency
    /// - `to`: Quote currency
    /// - `bump_pct`: Relative bump size (e.g., 0.01 for 1% increase)
    /// - `on`: Date used to verify a rate exists and the bumped value is valid
    ///
    /// # Returns
    /// New FxMatrix with the relatively bumped pair
    ///
    /// # Errors
    /// Returns an error if the rate lookup on `on` fails, if `bump_pct` is
    /// non-finite, or if the bump multiplier `1 + bump_pct` is not positive.
    pub fn with_bumped_rate(
        &self,
        from: Currency,
        to: Currency,
        bump_pct: f64,
        on: Date,
    ) -> crate::Result<Self> {
        // Verify a rate exists on the reference date and the bumped value is valid.
        let query = FxQuery::new(from, to, on);
        let current_rate = self.rate(query)?.rate;
        validate_fx_rate(from, to, current_rate * (1.0 + bump_pct))?;

        // Create bumped provider applying the relative bump per query.
        use super::providers::BumpedFxProvider;
        use std::sync::Arc;
        let bumped_provider = Arc::new(BumpedFxProvider::new(
            Arc::clone(&self.provider),
            from,
            to,
            bump_pct,
        )?);
        let multiplier = 1.0 + bump_pct;

        // Create new FX matrix with same config and carry over cached quotes so lookups that
        // rely on seeded values keep working after the bump. Quotes on the
        // bumped pair are scaled so they stay consistent with the bumped provider.
        let bumped = Self::try_with_config(bumped_provider, self.config)?;
        {
            let src = self.quotes.lock();
            let mut dst = bumped.quotes.lock();
            for (pair, rate) in src.iter() {
                let rate = if pair.0 == from && pair.1 == to {
                    *rate * multiplier
                } else if pair.0 == to && pair.1 == from {
                    *rate / multiplier
                } else {
                    *rate
                };
                dst.insert(*pair, rate);
            }
        }
        // Carry over authoritative pinned fixings; fixings on the bumped pair
        // are scaled by the bump multiplier so the relative bump applies on
        // every date, not only where the provider answers.
        {
            let src = self.pinned_quotes.lock();
            let mut dst = bumped.pinned_quotes.lock();
            for (key, rate) in src.iter() {
                let rate = if key.from == from && key.to == to {
                    *rate * multiplier
                } else if key.from == to && key.to == from {
                    *rate / multiplier
                } else {
                    *rate
                };
                dst.insert(*key, rate);
            }
        }
        // Do not carry over provider-observed quotes. They may be date/policy-sensitive
        // or derived crosses that depend transitively on the bumped leg.

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
            for (query, &quote) in quotes.iter() {
                if quote.rate.is_finite() && quote.rate > 0.0 {
                    rates.entry((query.from, query.to)).or_insert(quote.rate);
                    currencies.insert(query.from);
                    currencies.insert(query.to);
                }
            }
        }

        {
            let quotes = self.pinned_quotes.lock();
            for (query, &rate) in quotes.iter() {
                if rate.is_finite() && rate > 0.0 {
                    rates.entry((query.from, query.to)).or_insert(rate);
                    currencies.insert(query.from);
                    currencies.insert(query.to);
                }
            }
        }

        // Deterministic iteration order so a violating cycle is reported
        // identically across runs .
        let mut currencies: Vec<Currency> = currencies.into_iter().collect();
        currencies.sort();
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
        // repeat queries stamp the same metadata as the first lookup.
        self.insert_observed_quote(from, to, on, policy, rate, true);
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
        quotes.insert(Pair(from, to), rate);
    }

    /// Insert a query-sensitive provider-observed quote, recording whether the
    /// rate was derived via triangulation.
    fn insert_observed_quote(
        &self,
        from: Currency,
        to: Currency,
        on: Date,
        policy: FxConversionPolicy,
        rate: f64,
        triangulated: bool,
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
            ObservedQuote { rate, triangulated },
        );
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
        self.pinned_quotes.lock().insert(
            QueryKey {
                from,
                to,
                on,
                policy,
            },
            rate,
        );
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
        let quotes = self.pinned_quotes.lock();
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
        // Read direct and reciprocal under a single lock per store, then drop
        // the locks before any further work.
        let (direct_opt, reciprocal_opt) = self.read_cached_pair_bidir(from, to);
        let (pinned_direct_opt, pinned_reciprocal_opt) =
            self.read_pinned_pair_bidir(from, to, on, policy);
        let (observed_direct_opt, observed_reciprocal_opt) =
            self.read_observed_pair_bidir(from, to, on, policy);
        // 1) Explicit direct pair-global quote wins
        if let Some(r) = direct_opt {
            return validate_fx_rate(from, to, r);
        }
        // 2) Pinned fixings outrank reciprocal pair-global quotes, the
        // transient observed cache, and the provider.
        if let Some(r) = pinned_direct_opt {
            return validate_fx_rate(from, to, r);
        }
        if let Some(r_rev) = pinned_reciprocal_opt {
            return reciprocal_rate_or_err(r_rev, to, from);
        }
        // 3) Reciprocal pair-global quote
        if let Some(r_rev) = reciprocal_opt {
            return reciprocal_rate_or_err(r_rev, to, from);
        }
        // 4) Provider-observed cache
        if let Some(q) = observed_direct_opt {
            return validate_fx_rate(from, to, q.rate);
        }
        if let Some(q_rev) = observed_reciprocal_opt {
            return reciprocal_rate_or_err(q_rev.rate, to, from);
        }
        // 5) Fetch from the provider
        let r = self.provider.rate(from, to, on, policy)?;
        let r = validate_fx_rate(from, to, r)?;
        self.insert_observed_quote(from, to, on, policy, r, false);
        Ok(r)
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

    /// Read provider-observed cached quotes scoped to a specific date/policy query.
    #[inline]
    fn read_observed_pair_bidir(
        &self,
        from: Currency,
        to: Currency,
        on: Date,
        policy: FxConversionPolicy,
    ) -> (Option<ObservedQuote>, Option<ObservedQuote>) {
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

        let direct = quotes.get(&direct_key).copied().and_then(|q| {
            if q.rate.is_finite() && q.rate > 0.0 {
                Some(q)
            } else {
                let _ = quotes.pop(&direct_key);
                None
            }
        });
        let rev = quotes.get(&rev_key).copied().and_then(|q| {
            if q.rate.is_finite() && q.rate > 0.0 {
                Some(q)
            } else {
                let _ = quotes.pop(&rev_key);
                None
            }
        });
        (direct, rev)
    }
}
