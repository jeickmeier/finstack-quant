//! Standard FX provider implementations for simple quote storage.
//!
//! This module provides reusable FX provider types that can be shared across
//! bindings (Python, WASM, etc.) without duplication.

use super::provider::{FxProviderSnapshot, directional_rate, validate_stored_quote};
use super::{FxConversionPolicy, FxProvider};
use crate::InputError;
use crate::currency::Currency;
use crate::dates::Date;
use crate::{FxHashMap, recover_lock as recover};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, RwLock};

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
/// provider.set_quotes(&[(Currency::EUR, Currency::USD, 1.1)]).expect("valid rate");
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
    quotes: RwLock<FxHashMap<(Currency, Currency), f64>>,
    pinned_quotes: RwLock<FxHashMap<(Currency, Currency, Date, FxConversionPolicy), f64>>,
    revision: AtomicU64,
    /// Serializes writers so the exhaustion check and the revision advance
    /// cover one mutation at a time.
    writer: Mutex<()>,
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
        Self::default()
    }

    /// Bulk insert or update FX quotes.
    ///
    /// # Arguments
    ///
    /// * `quotes` - Ordered `(from, to, rate)` tuples with distinct currencies
    ///   and the rate expressed as finite, positive units of `to` per unit of
    ///   `from`.
    ///
    /// Each rate is units of `to` per unit of `from`. All inputs are validated
    /// before the provider acquires its write lock, so the operation is atomic
    /// with respect to the quote map: if any tuple is invalid, no tuple is
    /// written; otherwise readers observe the full batch together. A batch may
    /// not repeat an ordered pair (opposite orientations are distinct);
    /// separate calls may replace an existing quote.
    /// Successful updates advance the provider revision, invalidating cached
    /// observations in attached matrices without removing their explicit
    /// quotes or pinned fixings.
    ///
    /// # Errors
    ///
    /// Returns an error if any tuple is an identity pair, any rate is
    /// non-finite or not strictly positive, the batch repeats an ordered pair,
    /// or the provider revision is exhausted. The provider remains unchanged
    /// in that case.
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
    /// ]).expect("valid quotes");
    /// ```
    pub fn set_quotes(&self, quotes: &[(Currency, Currency, f64)]) -> crate::Result<()> {
        super::matrix::reject_repeated_pairs(quotes)?;
        let validated: Vec<_> = quotes
            .iter()
            .map(|&(from, to, rate)| ((from, to), rate))
            .collect();
        let _writer = recover(self.writer.lock());
        self.ensure_revision_available()?;
        let mut guard = recover(self.quotes.write());
        for (pair, rate) in validated {
            guard.insert(pair, rate);
        }
        self.advance_revision();
        Ok(())
    }

    /// Load date/policy-scoped provider state without promoting its authority
    /// to the containing matrix's explicit pinned quote store.
    pub(crate) fn set_snapshot_pinned_quotes(
        &self,
        quotes: &[(Currency, Currency, Date, FxConversionPolicy, f64)],
    ) -> crate::Result<()> {
        let mut validated = FxHashMap::default();
        for &(from, to, on, policy, rate) in quotes {
            let rate = validate_stored_quote(from, to, rate)?;
            if validated.insert((from, to, on, policy), rate).is_some() {
                return Err(crate::Error::Validation(format!(
                    "duplicate scoped provider quote for {from}->{to} on {on}/{policy}"
                )));
            }
        }
        let _writer = recover(self.writer.lock());
        self.ensure_revision_available()?;
        let mut quotes = recover(self.pinned_quotes.write());
        quotes.extend(validated);
        self.advance_revision();
        Ok(())
    }

    /// Fail before a mutation when no fresh revision remains to publish it.
    /// Callers hold the writer lock, so the check and the advance pair up.
    fn ensure_revision_available(&self) -> crate::Result<()> {
        if self.revision.load(Ordering::Acquire) == u64::MAX {
            return Err(crate::Error::Validation(
                "FX provider revision exhausted; create a new provider".into(),
            ));
        }
        Ok(())
    }

    /// Publish the next revision. `ensure_revision_available` ran under the
    /// same writer lock, so the increment cannot wrap.
    fn advance_revision(&self) {
        self.revision.fetch_add(1, Ordering::AcqRel);
    }
}

impl FxProvider for SimpleFxProvider {
    fn get_revision(&self) -> Option<u64> {
        Some(self.revision.load(Ordering::Acquire))
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
    /// Quotes inserted through `set_quotes` are constant across
    /// observation dates and policies. A restored market-context snapshot can
    /// also contain scoped provider quotes; those are checked first for their
    /// exact `on` date and `policy`. The store does not fetch or interpolate
    /// additional observations. Callers who need live date-aware FX should
    /// implement a custom [`FxProvider`].
    ///
    /// # Errors
    ///
    /// Returns `Err` when:
    /// - [`InputError::NotFound`](crate::InputError::NotFound): No direct quote
    ///   exists for `from→to` and no reciprocal `to→from` is available
    /// - [`InputError::NonFiniteValue`](crate::InputError::NonFiniteValue): The
    ///   stored rate is non-finite
    /// - [`InputError::InvalidFxRate`](crate::InputError::InvalidFxRate): The
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
    /// provider.set_quotes(&[(Currency::EUR, Currency::USD, 1.1)]).expect("valid rate");
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
        let scoped = {
            let pinned = recover(self.pinned_quotes.read());
            directional_rate(|a, b| pinned.get(&(a, b, on, policy)).copied(), from, to)?
        };
        let rate = match scoped {
            Some(rate) => Some(rate),
            None => {
                let quotes = recover(self.quotes.read());
                directional_rate(|a, b| quotes.get(&(a, b)).copied(), from, to)?
            }
        };
        rate.ok_or_else(|| {
            InputError::NotFound {
                id: format!("FX:{from}->{to}"),
            }
            .into()
        })
    }

    fn snapshot(&self) -> crate::Result<Option<FxProviderSnapshot>> {
        let quotes = recover(self.quotes.read())
            .iter()
            .map(|(&(from, to), &rate)| (from, to, rate))
            .collect();
        let pinned_quotes = recover(self.pinned_quotes.read())
            .iter()
            .map(|(&(from, to, on, policy), &rate)| (from, to, on, policy, rate))
            .collect();
        Ok(Some(FxProviderSnapshot {
            quotes,
            pinned_quotes,
        }))
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
    fn exhausted_provider_revision_fails_before_mutation() {
        let provider = SimpleFxProvider::new();
        provider.revision.store(u64::MAX, Ordering::Release);
        assert!(
            provider
                .set_quotes(&[(Currency::EUR, Currency::USD, 1.1)])
                .is_err()
        );
        assert!(
            provider
                .set_snapshot_pinned_quotes(&[(
                    Currency::EUR,
                    Currency::USD,
                    test_date(),
                    FxConversionPolicy::CashflowDate,
                    1.1,
                )])
                .is_err()
        );
        assert_eq!(provider.get_revision(), Some(u64::MAX));
        let snapshot = provider
            .snapshot()
            .expect("valid snapshot")
            .expect("supported snapshot");
        assert!(snapshot.quotes.is_empty() && snapshot.pinned_quotes.is_empty());
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
    fn scoped_snapshot_loader_validates_atomically_and_rejects_duplicate_keys() {
        let provider = SimpleFxProvider::new();
        let on = test_date();
        let policy = FxConversionPolicy::CashflowDate;
        provider
            .set_snapshot_pinned_quotes(&[(Currency::EUR, Currency::USD, on, policy, 1.2)])
            .expect("initial scoped quote");
        for invalid in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
            assert!(
                provider
                    .set_snapshot_pinned_quotes(&[
                        (Currency::EUR, Currency::USD, on, policy, 1.5),
                        (Currency::USD, Currency::GBP, on, policy, invalid),
                    ])
                    .is_err()
            );
            assert_eq!(
                provider
                    .rate(Currency::EUR, Currency::USD, on, policy)
                    .expect("original"),
                1.2
            );
            assert!(
                provider
                    .rate(Currency::USD, Currency::GBP, on, policy)
                    .is_err()
            );
        }
        assert!(
            provider
                .set_snapshot_pinned_quotes(&[
                    (Currency::EUR, Currency::USD, on, policy, 1.5),
                    (Currency::EUR, Currency::USD, on, policy, 1.6),
                ])
                .is_err()
        );
        assert_eq!(
            provider
                .rate(Currency::EUR, Currency::USD, on, policy)
                .expect("original"),
            1.2
        );
    }
}
