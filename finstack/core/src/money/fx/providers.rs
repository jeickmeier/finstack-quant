//! Standard FX provider implementations for simple quote storage.
//!
//! This module provides reusable FX provider types that can be shared across
//! bindings (Python, WASM, etc.) without duplication.

use super::{FxConversionPolicy, FxProvider};
use crate::collections::HashMap;
use crate::currency::Currency;
use crate::dates::Date;
use crate::error::InputError;
use parking_lot::RwLock;
use std::sync::Arc;

/// Simple FX provider backed by an in-memory quote store.
///
/// Supports:
/// - Direct quote lookup
/// - Automatic reciprocal calculation
/// - Thread-safe mutable quote insertion
///
/// # Examples
/// ```rust
/// use finstack_core::money::fx::SimpleFxProvider;
/// use finstack_core::money::fx::{FxProvider, FxConversionPolicy};
/// use finstack_core::currency::Currency;
/// use finstack_core::dates::Date;
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
}

impl SimpleFxProvider {
    /// Create a new empty provider.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_core::money::fx::SimpleFxProvider;
    ///
    /// let provider = SimpleFxProvider::new();
    /// ```
    pub fn new() -> Self {
        Self {
            quotes: RwLock::new(HashMap::default()),
        }
    }

    /// Insert or update a single FX quote.
    ///
    /// # Parameters
    /// - `from`: Base currency
    /// - `to`: Quote currency
    /// - `rate`: FX rate (from → to)
    ///
    /// # Examples
    /// ```rust
    /// use finstack_core::money::fx::SimpleFxProvider;
    /// use finstack_core::currency::Currency;
    ///
    /// let provider = SimpleFxProvider::new();
    /// provider.set_quote(Currency::GBP, Currency::USD, 1.25);
    /// ```
    pub fn set_quote(&self, from: Currency, to: Currency, rate: f64) -> crate::Result<()> {
        let rate = super::validate_fx_rate(from, to, rate)?;
        self.quotes.write().insert((from, to), rate);
        Ok(())
    }

    /// Bulk insert or update FX quotes.
    ///
    /// # Parameters
    /// - `quotes`: Slice of `(from, to, rate)` tuples
    ///
    /// # Examples
    /// ```rust
    /// use finstack_core::money::fx::SimpleFxProvider;
    /// use finstack_core::currency::Currency;
    ///
    /// let provider = SimpleFxProvider::new();
    /// provider.set_quotes(&[
    ///     (Currency::EUR, Currency::USD, 1.1),
    ///     (Currency::GBP, Currency::USD, 1.25),
    /// ]);
    /// ```
    pub fn set_quotes(&self, quotes: &[(Currency, Currency, f64)]) -> crate::Result<()> {
        let mut guard = self.quotes.write();
        for &(from, to, rate) in quotes {
            let rate = super::validate_fx_rate(from, to, rate)?;
            guard.insert((from, to), rate);
        }
        Ok(())
    }

    /// Retrieve a direct quote if available.
    ///
    /// Returns `None` if no direct quote exists for the pair.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_core::money::fx::SimpleFxProvider;
    /// use finstack_core::currency::Currency;
    ///
    /// let provider = SimpleFxProvider::new();
    /// provider.set_quote(Currency::EUR, Currency::USD, 1.1).expect("valid rate");
    ///
    /// assert_eq!(provider.get_direct(Currency::EUR, Currency::USD), Some(1.1));
    /// assert_eq!(provider.get_direct(Currency::USD, Currency::EUR), None);
    /// ```
    pub fn get_direct(&self, from: Currency, to: Currency) -> Option<f64> {
        self.quotes.read().get(&(from, to)).copied()
    }
}

impl FxProvider for SimpleFxProvider {
    /// Return an FX rate with automatic reciprocal fallback.
    ///
    /// The provider:
    /// 1. Returns 1.0 for identical currencies
    /// 2. Checks for a direct quote
    /// 3. Falls back to reciprocal if available
    /// 4. Returns `NotFound` error otherwise
    ///
    /// # Date and Policy Are Ignored
    ///
    /// `SimpleFxProvider` is a snapshot store: it does **not** honor the `on`
    /// observation date or the `policy` (cashflow date / settlement date /
    /// closing rate / average rate). All quotes are treated as the current
    /// snapshot regardless of the query parameters. Callers who need
    /// date-aware or policy-aware FX should compose this provider with a
    /// time-series store at a higher layer, or implement a custom
    /// [`FxProvider`].
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
    /// use finstack_core::money::fx::SimpleFxProvider;
    /// use finstack_core::money::fx::{FxProvider, FxConversionPolicy};
    /// use finstack_core::currency::Currency;
    /// use finstack_core::dates::Date;
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
        _on: Date,
        _policy: FxConversionPolicy,
    ) -> crate::Result<f64> {
        if from == to {
            return Ok(1.0);
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
        self.quotes
            .read()
            .iter()
            .map(|(&(from, to), &rate)| (from, to, rate))
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
    /// # Parameters
    /// - `original`: Original FX provider to delegate to
    /// - `from`: Base currency of the bumped pair
    /// - `to`: Quote currency of the bumped pair
    /// - `bump_pct`: Relative bump size (e.g., `0.01` for a 1% increase)
    ///
    /// # Errors
    ///
    /// Returns `Err(Error::Validation)` when `bump_pct` is non-finite or the
    /// resulting multiplier `1 + bump_pct` is not strictly positive (such a
    /// bump would produce zero/negative FX rates).
    pub fn new(
        original: Arc<dyn FxProvider>,
        from: Currency,
        to: Currency,
        bump_pct: f64,
    ) -> crate::Result<Self> {
        let bump_multiplier = 1.0 + bump_pct;
        if !bump_pct.is_finite() || bump_multiplier <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "BumpedFxProvider bump_pct must be finite with 1 + bump_pct > 0 (got {bump_pct})"
            )));
        }
        Ok(Self {
            original,
            override_from: from,
            override_to: to,
            bump_multiplier,
        })
    }
}

impl FxProvider for BumpedFxProvider {
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
    fn simple_provider_direct_quote() {
        let provider = SimpleFxProvider::new();
        provider
            .set_quote(Currency::EUR, Currency::USD, 1.1)
            .expect("valid test quote");

        let rate = provider
            .rate(
                Currency::EUR,
                Currency::USD,
                test_date(),
                FxConversionPolicy::CashflowDate,
            )
            .expect("FX rate query should succeed in test");
        assert_eq!(rate, 1.1);
    }

    #[test]
    fn simple_provider_reciprocal() {
        let provider = SimpleFxProvider::new();
        provider
            .set_quote(Currency::EUR, Currency::USD, 1.1)
            .expect("valid test quote");

        let rate = provider
            .rate(
                Currency::USD,
                Currency::EUR,
                test_date(),
                FxConversionPolicy::CashflowDate,
            )
            .expect("FX rate query should succeed in test");
        assert!((rate - 1.0 / 1.1).abs() < 1e-12);
    }

    #[test]
    fn simple_provider_identity() {
        let provider = SimpleFxProvider::new();

        let rate = provider
            .rate(
                Currency::USD,
                Currency::USD,
                test_date(),
                FxConversionPolicy::CashflowDate,
            )
            .expect("FX rate query should succeed in test");
        assert_eq!(rate, 1.0);
    }

    #[test]
    fn simple_provider_not_found() {
        let provider = SimpleFxProvider::new();

        let result = provider.rate(
            Currency::EUR,
            Currency::USD,
            test_date(),
            FxConversionPolicy::CashflowDate,
        );
        assert!(result.is_err());
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
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, -1.5] {
            assert!(
                BumpedFxProvider::new(Arc::clone(&original), Currency::EUR, Currency::USD, bad)
                    .is_err(),
                "bump_pct {bad} must be rejected"
            );
        }
    }
}
