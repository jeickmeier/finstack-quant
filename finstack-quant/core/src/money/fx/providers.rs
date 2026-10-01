//! Standard FX provider implementations for simple quote storage.
//!
//! This module provides reusable FX provider types that can be shared across
//! bindings (Python, WASM, etc.) without duplication.

use super::{FxConversionPolicy, FxProvider};
use crate::collections::HashMap;
use crate::currency::Currency;
use crate::dates::Date;
use crate::error::InputError;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

fn recover<T>(res: Result<T, std::sync::PoisonError<T>>) -> T {
    res.unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Simple FX provider backed by an in-memory quote store.
///
/// Supports:
/// - Direct quote lookup
/// - Automatic reciprocal calculation
/// - Thread-safe mutable quote insertion
/// - Revision tracking so attached matrices refresh observations after updates
///
/// # Examples
/// ```rust
/// use finstack_quant_core::money::fx::SimpleFxProvider;
/// use finstack_quant_core::money::fx::{FxProvider, FxConversionPolicy};
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::dates::Date;
/// use time::Month;
///
/// let provider = SimpleFxProvider::new();
/// provider.set_quote(Currency::EUR, Currency::USD, 1.1).expect("valid rate");
///
/// let date = Date::from_calendar_date(2024, Month::January, 2).expect("Valid date");
/// let rate = provider.rate(Currency::EUR, Currency::USD, date, FxConversionPolicy::CashflowDate).expect("FX rate lookup should succeed");
/// assert_eq!(rate, 1.1);
///
/// // Reciprocal works automatically
/// let rate_inv = provider.rate(Currency::USD, Currency::EUR, date, FxConversionPolicy::CashflowDate).expect("FX rate lookup should succeed");
/// assert!((rate_inv - 1.0/1.1).abs() < 1e-12);
/// ```
#[derive(Default)]
pub struct SimpleFxProvider {
    quotes: RwLock<HashMap<(Currency, Currency), f64>>,
    pinned_quotes: RwLock<HashMap<(Currency, Currency, Date, FxConversionPolicy), f64>>,
    revision: AtomicU64,
}

impl SimpleFxProvider {
    /// Create a new empty provider.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::money::fx::SimpleFxProvider;
    ///
    /// let provider = SimpleFxProvider::new();
    /// ```
    pub fn new() -> Self {
        Self {
            quotes: RwLock::new(HashMap::default()),
            pinned_quotes: RwLock::new(HashMap::default()),
            revision: AtomicU64::new(0),
        }
    }

    /// Insert or update a single FX quote.
    ///
    /// # Parameters
    /// - `from`: Base currency.
    /// - `to`: Quote currency.
    /// - `rate`: Units of `to` per one unit of `from` (for example, `1.25`
    ///   for GBP/USD when `from` is GBP and `to` is USD).
    ///
    /// Replaces any existing direct quote for the same ordered pair. It does
    /// not insert the reciprocal pair; reciprocal lookup is performed by the
    /// [`FxProvider`] implementation at read time. Successful updates advance
    /// the provider revision, invalidating observations in attached matrices
    /// without removing their explicit quotes or pinned fixings.
    ///
    /// # Errors
    ///
    /// Returns an error if `rate` is non-finite or not strictly positive. On
    /// error the provider is unchanged.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::money::fx::SimpleFxProvider;
    /// use finstack_quant_core::currency::Currency;
    ///
    /// let provider = SimpleFxProvider::new();
    /// provider.set_quote(Currency::GBP, Currency::USD, 1.25);
    /// ```
    pub fn set_quote(&self, from: Currency, to: Currency, rate: f64) -> crate::Result<()> {
        let rate = super::validate_fx_rate(from, to, rate)?;
        let mut quotes = recover(self.quotes.write());
        quotes.insert((from, to), rate);
        self.advance_revision();
        Ok(())
    }

    /// Bulk insert or update FX quotes.
    ///
    /// # Parameters
    /// - `quotes`: Slice of `(from, to, rate)` tuples
    ///
    /// Each rate is units of `to` per unit of `from`. All inputs are validated
    /// before the provider acquires its write lock, so the operation is atomic
    /// with respect to the quote map: if any tuple is invalid, no tuple is
    /// written; otherwise readers observe the full batch together. Later
    /// duplicate pairs in the slice replace earlier entries in that batch.
    ///
    /// # Errors
    ///
    /// Returns an error if any rate is non-finite or not strictly positive.
    /// The provider remains unchanged in that case.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::money::fx::SimpleFxProvider;
    /// use finstack_quant_core::currency::Currency;
    ///
    /// let provider = SimpleFxProvider::new();
    /// provider.set_quotes(&[
    ///     (Currency::EUR, Currency::USD, 1.1),
    ///     (Currency::GBP, Currency::USD, 1.25),
    /// ]);
    /// ```
    pub fn set_quotes(&self, quotes: &[(Currency, Currency, f64)]) -> crate::Result<()> {
        let validated = quotes
            .iter()
            .map(|&(from, to, rate)| {
                super::validate_fx_rate(from, to, rate).map(|rate| ((from, to), rate))
            })
            .collect::<crate::Result<Vec<_>>>()?;
        let mut guard = recover(self.quotes.write());
        for (pair, rate) in validated {
            guard.insert(pair, rate);
        }
        self.advance_revision();
        Ok(())
    }

    /// Retrieve a direct quote if available.
    ///
    /// Returns `None` if no direct quote exists for the pair.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::money::fx::SimpleFxProvider;
    /// use finstack_quant_core::currency::Currency;
    ///
    /// let provider = SimpleFxProvider::new();
    /// provider.set_quote(Currency::EUR, Currency::USD, 1.1).expect("valid rate");
    ///
    /// assert_eq!(provider.get_direct(Currency::EUR, Currency::USD), Some(1.1));
    /// assert_eq!(provider.get_direct(Currency::USD, Currency::EUR), None);
    /// ```
    pub fn get_direct(&self, from: Currency, to: Currency) -> Option<f64> {
        recover(self.quotes.read()).get(&(from, to)).copied()
    }

    /// Load date/policy-scoped provider state without promoting its authority
    /// to the containing matrix's explicit pinned quote store.
    pub(crate) fn set_snapshot_pinned_quotes(
        &self,
        quotes: &[(Currency, Currency, Date, FxConversionPolicy, f64)],
    ) -> crate::Result<()> {
        let mut validated = HashMap::default();
        for &(from, to, on, policy, rate) in quotes {
            let rate = super::validate_fx_rate(from, to, rate)?;
            if validated.insert((from, to, on, policy), rate).is_some() {
                return Err(crate::Error::Validation(format!(
                    "duplicate scoped provider quote for {from}->{to} on {on}/{policy}"
                )));
            }
        }
        let mut quotes = recover(self.pinned_quotes.write());
        quotes.extend(validated);
        self.advance_revision();
        Ok(())
    }

    fn advance_revision(&self) {
        // Exhaustion disables caching instead of reusing an older revision.
        let _ = self
            .revision
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(1)
            });
    }
}

impl FxProvider for SimpleFxProvider {
    fn get_revision(&self) -> Option<u64> {
        let revision = self.revision.load(Ordering::Acquire);
        (revision != u64::MAX).then_some(revision)
    }

    /// Return an FX rate with automatic reciprocal fallback.
    ///
    /// The provider:
    /// 1. Returns 1.0 for identical currencies
    /// 2. Checks for a direct quote
    /// 3. Falls back to reciprocal if available
    /// 4. Returns `NotFound` error otherwise
    ///
    /// # Snapshot Date and Policy
    ///
    /// Quotes inserted through `set_quote` and `set_quotes` are constant across
    /// observation dates and policies. A restored market-context snapshot can
    /// also contain scoped provider quotes; those are checked first for their
    /// exact `on` date and `policy`. The store does not fetch or interpolate
    /// additional observations. Callers who need live date-aware FX should
    /// implement a custom [`FxProvider`].
    ///
    /// # Errors
    ///
    /// Returns `Err` when:
    /// - [`InputError::NotFound`](crate::error::InputError::NotFound): No direct quote
    ///   exists for `from→to` and no reciprocal `to→from` is available
    /// - [`InputError::NonFiniteValue`](crate::error::InputError::NonFiniteValue): The
    ///   stored rate is non-finite
    /// - [`InputError::InvalidFxRate`](crate::error::InputError::InvalidFxRate): The
    ///   computed reciprocal is non-finite or non-positive (e.g. the stored
    ///   rate is subnormal, so `1/rate` overflows to infinity)
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::money::fx::SimpleFxProvider;
    /// use finstack_quant_core::money::fx::{FxProvider, FxConversionPolicy};
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::Date;
    /// use time::Month;
    ///
    /// let provider = SimpleFxProvider::new();
    /// provider.set_quote(Currency::EUR, Currency::USD, 1.1).expect("valid rate");
    ///
    /// let date = Date::from_calendar_date(2024, Month::January, 2).expect("Valid date");
    /// let rate = provider.rate(Currency::EUR, Currency::USD, date, FxConversionPolicy::CashflowDate).expect("FX rate lookup should succeed");
    /// assert_eq!(rate, 1.1);
    ///
    /// // Reciprocal works automatically
    /// let rate_inv = provider.rate(Currency::USD, Currency::EUR, date, FxConversionPolicy::CashflowDate).expect("FX rate lookup should succeed");
    /// assert!((rate_inv - 1.0/1.1).abs() < 1e-12);
    /// ```
    fn rate(
        &self,
        from: Currency,
        to: Currency,
        on: Date,
        policy: FxConversionPolicy,
    ) -> crate::Result<f64> {
        if from == to {
            return Ok(1.0);
        }
        {
            let pinned = recover(self.pinned_quotes.read());
            if let Some(&rate) = pinned.get(&(from, to, on, policy)) {
                return Ok(rate);
            }
            if let Some(&rate) = pinned.get(&(to, from, on, policy)) {
                return super::reciprocal_rate_or_err(rate, to, from);
            }
        }
        if let Some(rate) = self.get_direct(from, to) {
            return Ok(rate);
        }
        if let Some(rate) = self.get_direct(to, from) {
            return super::reciprocal_rate_or_err(rate, to, from);
        }
        Err(InputError::NotFound {
            id: format!("FX:{from}->{to}"),
        }
        .into())
    }

    fn snapshot_quotes(&self) -> Vec<(Currency, Currency, f64)> {
        recover(self.quotes.read())
            .iter()
            .map(|(&(from, to), &rate)| (from, to, rate))
            .collect()
    }

    fn snapshot_pinned_quotes(&self) -> Vec<(Currency, Currency, Date, FxConversionPolicy, f64)> {
        recover(self.pinned_quotes.read())
            .iter()
            .map(|(&(from, to, on, policy), &rate)| (from, to, on, policy, rate))
            .collect()
    }
}

/// Wrapper provider that applies a relative bump to one FX pair while
/// delegating all other pairs (and the unbumped per-date rate) to the
/// original provider.
///
/// This is useful for bumping FX rates in finite-difference greeks or
/// scenario analysis without losing the rest of the FX matrix state. The
/// bump is **term-structure preserving**: for the bumped pair the provider
/// returns `original.rate(query) * (1 + bump_pct)`, so a date-aware
/// provider keeps its date/policy structure under the bump (2026-06-09 core
/// the previous implementation returned one frozen absolute
/// rate for every date/policy).
pub struct BumpedFxProvider {
    /// Original provider to delegate to
    original: Arc<dyn FxProvider>,
    /// Bumped pair base currency
    override_from: Currency,
    /// Bumped pair quote currency
    override_to: Currency,
    /// Relative bump multiplier (`1 + bump_pct`), validated positive and finite.
    bump_multiplier: f64,
}

impl BumpedFxProvider {
    /// Create a new bumped provider that relatively bumps one pair.
    ///
    /// # Arguments
    ///
    /// * `original` - Provider supplying per-date rates and any stored snapshot quotes; its live rates are not fetched during construction.
    /// * `from` - Base currency of the bumped pair; must differ from `to`.
    /// * `to` - Quote currency, whose units are paid per unit of `from`.
    /// * `bump_pct` - Relative decimal shock (for example, `0.01` increases the rate by 1%); must be finite and greater than -1.
    ///
    /// # Errors
    ///
    /// Returns an error when `from == to`, the bump multiplier is invalid, or
    /// any statically known shocked snapshot quote (including another date or
    /// policy scope) is outside the finite, positive FX range.
    pub fn new(
        original: Arc<dyn FxProvider>,
        from: Currency,
        to: Currency,
        bump_pct: f64,
    ) -> crate::Result<Self> {
        if from == to {
            return Err(crate::Error::Validation(
                "cannot bump an identity FX pair".into(),
            ));
        }
        let bump_multiplier = 1.0 + bump_pct;
        if !bump_pct.is_finite() || bump_multiplier <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "BumpedFxProvider bump_pct must be finite with 1 + bump_pct > 0 (got {bump_pct})"
            )));
        }
        let bumped = Self {
            original,
            override_from: from,
            override_to: to,
            bump_multiplier,
        };
        // Validate every statically known shocked scope, not just a caller's
        // reference-date lookup. A snapshot must not turn an overflowing
        // scoped cross into an invalid serialized quote or an unshocked rate.
        for (base, quote, rate) in bumped.snapshot_quotes() {
            super::validate_fx_rate(base, quote, rate)?;
        }
        for (base, quote, _, _, rate) in bumped.snapshot_pinned_quotes() {
            super::validate_fx_rate(base, quote, rate)?;
        }
        Ok(bumped)
    }
}

impl FxProvider for BumpedFxProvider {
    fn get_revision(&self) -> Option<u64> {
        self.original.get_revision()
    }

    /// Return an FX rate, relatively bumping the overridden pair.
    ///
    /// The provider:
    /// 1. Returns `original_rate * (1 + bump_pct)` for the bumped `from→to` pair
    /// 2. Returns the reciprocal of the bumped direct rate for `to→from`
    /// 3. Delegates to the original provider for all other pairs
    ///
    /// # Errors
    ///
    /// Returns `Err` when:
    /// - The original provider fails for the queried pair
    /// - The bumped rate is non-finite or non-positive
    /// - Any error propagated from [`FxProvider::rate`] on the underlying provider
    fn rate(
        &self,
        from: Currency,
        to: Currency,
        on: Date,
        policy: FxConversionPolicy,
    ) -> crate::Result<f64> {
        // Check if this is the overridden pair (or its reciprocal)
        if from == self.override_from && to == self.override_to {
            let base = self.original.rate(from, to, on, policy)?;
            return super::validate_fx_rate(from, to, base * self.bump_multiplier);
        }
        if from == self.override_to && to == self.override_from {
            let base = self
                .original
                .rate(self.override_from, self.override_to, on, policy)?;
            let bumped = super::validate_fx_rate(
                self.override_from,
                self.override_to,
                base * self.bump_multiplier,
            )?;
            return super::reciprocal_rate_or_err(bumped, self.override_to, self.override_from);
        }

        // Delegate to original provider for all other pairs
        self.original.rate(from, to, on, policy)
    }

    fn snapshot_quotes(&self) -> Vec<(Currency, Currency, f64)> {
        let mut quotes = self.original.snapshot_quotes();
        let from = self.override_from;
        let to = self.override_to;
        let rate = quotes
            .iter()
            .find(|&&(base, quote, _)| base == from && quote == to)
            .map(|&(_, _, rate)| rate)
            .or_else(|| {
                quotes
                    .iter()
                    .find(|&&(base, quote, _)| base == to && quote == from)
                    .map(|&(_, _, rate)| 1.0 / rate)
            });
        if let Some(rate) = rate {
            quotes.retain(|&(base, quote, _)| {
                !((base == from && quote == to) || (base == to && quote == from))
            });
            quotes.push((from, to, rate * self.bump_multiplier));
        }
        quotes
    }

    fn snapshot_pinned_quotes(&self) -> Vec<(Currency, Currency, Date, FxConversionPolicy, f64)> {
        let mut quotes = self.original.snapshot_pinned_quotes();
        let from = self.override_from;
        let to = self.override_to;
        let mut rates = HashMap::default();
        for &(base, quote, on, policy, rate) in &quotes {
            if base == from && quote == to {
                rates.insert((on, policy), rate);
            }
        }
        for &(base, quote, on, policy, rate) in &quotes {
            if base == to && quote == from {
                rates.entry((on, policy)).or_insert(1.0 / rate);
            }
        }
        quotes.retain(|&(base, quote, _, _, _)| {
            !((base == from && quote == to) || (base == to && quote == from))
        });
        quotes.extend(
            rates
                .into_iter()
                .map(|((on, policy), rate)| (from, to, on, policy, rate * self.bump_multiplier)),
        );
        quotes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::currency::Currency;
    use time::Month;

    fn test_date() -> Date {
        Date::from_calendar_date(2024, Month::January, 2).expect("Valid test date")
    }

    #[test]
    fn simple_provider_bulk_quotes() {
        let provider = SimpleFxProvider::new();
        provider
            .set_quotes(&[
                (Currency::EUR, Currency::USD, 1.1),
                (Currency::GBP, Currency::USD, 1.25),
            ])
            .expect("valid test quotes");

        let eur_usd = provider
            .rate(
                Currency::EUR,
                Currency::USD,
                test_date(),
                FxConversionPolicy::CashflowDate,
            )
            .expect("FX rate query should succeed in test");
        assert_eq!(eur_usd, 1.1);

        let gbp_usd = provider
            .rate(
                Currency::GBP,
                Currency::USD,
                test_date(),
                FxConversionPolicy::CashflowDate,
            )
            .expect("FX rate query should succeed in test");
        assert_eq!(gbp_usd, 1.25);
    }

    #[test]
    fn bumped_provider_applies_relative_bump() {
        let original = Arc::new(SimpleFxProvider::new());
        original
            .set_quote(Currency::EUR, Currency::USD, 1.1)
            .expect("valid test quote");
        original
            .set_quote(Currency::GBP, Currency::USD, 1.25)
            .expect("valid test quote");

        // Create bumped provider that bumps EUR/USD by 1% (relative).
        let bumped = BumpedFxProvider::new(original, Currency::EUR, Currency::USD, 0.01)
            .expect("valid bump");

        // Bumped pair returns the delegated rate scaled by (1 + bump_pct).
        let eur_usd = bumped
            .rate(
                Currency::EUR,
                Currency::USD,
                test_date(),
                FxConversionPolicy::CashflowDate,
            )
            .expect("FX rate query should succeed in test");
        assert!((eur_usd - 1.1 * 1.01).abs() < 1e-12);

        // Reciprocal direction is the inverse of the bumped direct rate.
        let usd_eur = bumped
            .rate(
                Currency::USD,
                Currency::EUR,
                test_date(),
                FxConversionPolicy::CashflowDate,
            )
            .expect("FX rate query should succeed in test");
        assert!((usd_eur - 1.0 / (1.1 * 1.01)).abs() < 1e-12);

        // Other rates should delegate to original
        let gbp_usd = bumped
            .rate(
                Currency::GBP,
                Currency::USD,
                test_date(),
                FxConversionPolicy::CashflowDate,
            )
            .expect("FX rate query should succeed in test");
        assert_eq!(gbp_usd, 1.25);
    }

    #[test]
    fn bumped_provider_preserves_term_structure() {
        // bumping must not flatten a date-aware
        // provider's FX term structure. Both dates must move by the same
        // relative bump, not collapse to one frozen absolute rate.
        struct DateAwareFx;
        impl FxProvider for DateAwareFx {
            fn rate(
                &self,
                _from: Currency,
                _to: Currency,
                on: Date,
                _policy: FxConversionPolicy,
            ) -> crate::Result<f64> {
                if on == Date::from_calendar_date(2024, Month::January, 2).expect("valid") {
                    Ok(1.10)
                } else {
                    Ok(1.20)
                }
            }
        }

        let bumped =
            BumpedFxProvider::new(Arc::new(DateAwareFx), Currency::EUR, Currency::USD, 0.01)
                .expect("valid bump");

        let d1 = Date::from_calendar_date(2024, Month::January, 2).expect("valid");
        let d2 = Date::from_calendar_date(2024, Month::June, 2).expect("valid");
        let r1 = bumped
            .rate(
                Currency::EUR,
                Currency::USD,
                d1,
                FxConversionPolicy::CashflowDate,
            )
            .expect("rate on d1");
        let r2 = bumped
            .rate(
                Currency::EUR,
                Currency::USD,
                d2,
                FxConversionPolicy::CashflowDate,
            )
            .expect("rate on d2");
        assert!((r1 - 1.10 * 1.01).abs() < 1e-12);
        assert!((r2 - 1.20 * 1.01).abs() < 1e-12);
        assert!(
            (r1 - r2).abs() > 1e-6,
            "term structure must not be flattened"
        );
    }

    #[test]
    fn bumped_provider_rejects_invalid_bumps() {
        let original: Arc<dyn FxProvider> = Arc::new(SimpleFxProvider::new());
        assert!(
            BumpedFxProvider::new(Arc::clone(&original), Currency::USD, Currency::USD, 0.1)
                .is_err()
        );
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, -1.5] {
            assert!(
                BumpedFxProvider::new(Arc::clone(&original), Currency::EUR, Currency::USD, bad)
                    .is_err(),
                "bump_pct {bad} must be rejected"
            );
        }
    }

    #[test]
    fn scoped_snapshot_loader_validates_atomically_and_rejects_duplicate_keys() {
        let provider = SimpleFxProvider::new();
        let on = test_date();
        let policy = FxConversionPolicy::CashflowDate;
        provider
            .set_snapshot_pinned_quotes(&[(Currency::EUR, Currency::USD, on, policy, 1.2)])
            .expect("initial scoped quote");
        for invalid in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
            assert!(provider
                .set_snapshot_pinned_quotes(&[
                    (Currency::EUR, Currency::USD, on, policy, 1.5),
                    (Currency::USD, Currency::GBP, on, policy, invalid),
                ])
                .is_err());
            assert_eq!(
                provider
                    .rate(Currency::EUR, Currency::USD, on, policy)
                    .expect("original"),
                1.2
            );
            assert!(provider
                .rate(Currency::USD, Currency::GBP, on, policy)
                .is_err());
        }
        assert!(provider
            .set_snapshot_pinned_quotes(&[
                (Currency::EUR, Currency::USD, on, policy, 1.5),
                (Currency::EUR, Currency::USD, on, policy, 1.6),
            ])
            .is_err());
        assert_eq!(
            provider
                .rate(Currency::EUR, Currency::USD, on, policy)
                .expect("original"),
            1.2
        );
    }
}
