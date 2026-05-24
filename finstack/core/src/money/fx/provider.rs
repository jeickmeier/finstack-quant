use crate::currency::Currency;
use crate::dates::Date;

use super::types::FxConversionPolicy;

/// Helper to compute reciprocal rate safely, checking for zero division.
///
/// Returns `1.0 / rate` if `rate != 0.0`, otherwise returns an error.
/// This consolidates the reciprocal logic used across FX providers and matrix lookups.
#[inline]
pub(crate) fn reciprocal_rate_or_err(
    rate: f64,
    from: Currency,
    to: Currency,
) -> crate::Result<f64> {
    if !rate.is_finite() {
        let kind = if rate.is_nan() {
            crate::error::NonFiniteKind::NaN
        } else if rate.is_sign_positive() {
            crate::error::NonFiniteKind::PosInfinity
        } else {
            crate::error::NonFiniteKind::NegInfinity
        };
        return Err(crate::error::InputError::NonFiniteValue { kind }.into());
    }
    if rate != 0.0 {
        Ok(1.0 / rate)
    } else {
        Err(crate::error::InputError::NotFound {
            id: format!("FX:{from}->{to} (zero reciprocal)"),
        }
        .into())
    }
}

#[inline]
pub(crate) fn validate_fx_rate(from: Currency, to: Currency, rate: f64) -> crate::Result<f64> {
    if !rate.is_finite() || rate <= 0.0 {
        return Err(crate::error::InputError::InvalidFxRate { from, to, rate }.into());
    }
    Ok(rate)
}

/// Trait for obtaining FX rates.
///
/// Implementations can be as simple as hard-coded tables or as complex as
/// feed handlers. Providers should respect the supplied
/// [`FxConversionPolicy`].
///
/// # Required Methods
///
/// Implementors must provide:
/// - [`rate`](Self::rate): Look up an FX rate for a currency pair
///
/// # Implementation Guide
///
/// When implementing this trait:
/// 1. Return `1.0` when `from == to` (identity conversion)
/// 2. Consider supporting reciprocal lookups (if `A→B` exists, compute `B→A = 1/rate`)
/// 3. Validate rates are finite and positive before returning
/// 4. Use the `policy` hint to select between spot, forward, or averaged rates
///
/// # Errors
///
/// Implementations should return errors when:
/// - [`InputError::NotFound`](crate::error::InputError::NotFound): No rate available for the requested pair
/// - [`InputError::InvalidFxRate`](crate::error::InputError::InvalidFxRate): Rate is non-finite or non-positive
/// - [`InputError::NonFiniteValue`](crate::error::InputError::NonFiniteValue): Computed rate is NaN or infinity
///
/// # Examples
///
/// ## Using the trait
///
/// ```rust
/// use finstack_core::money::fx::{FxConversionPolicy, FxProvider};
/// use finstack_core::currency::Currency;
/// use finstack_core::dates::Date;
/// use time::Month;
///
/// fn convert_amount<P: FxProvider>(
///     provider: &P,
///     amount: f64,
///     from: Currency,
///     to: Currency,
///     on: Date,
/// ) -> finstack_core::Result<f64> {
///     let rate = provider.rate(from, to, on, FxConversionPolicy::CashflowDate)?;
///     Ok(amount * rate)
/// }
/// ```
///
/// ## Implementing the trait
///
/// ```rust
/// use finstack_core::money::fx::{FxConversionPolicy, FxProvider};
/// use finstack_core::currency::Currency;
/// use finstack_core::dates::Date;
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
///         Ok(1.25)
///     }
/// }
///
/// let trade_date = Date::from_calendar_date(2024, Month::January, 10).expect("Valid date");
/// let quote = StaticFx.rate(
///     Currency::EUR,
///     Currency::USD,
///     trade_date,
///     FxConversionPolicy::CashflowDate,
/// ).expect("FX rate lookup should succeed");
/// assert_eq!(quote, 1.25);
/// ```
pub trait FxProvider: Send + Sync {
    /// Return an FX rate to convert `from` → `to` applicable on `on` per `policy`.
    ///
    /// # Arguments
    ///
    /// * `from` - Source currency
    /// * `to` - Target currency
    /// * `on` - Valuation date for the rate lookup
    /// * `policy` - Hint for which rate type to use (spot, forward, average)
    ///
    /// # Returns
    ///
    /// The FX rate such that `amount_in_from * rate = amount_in_to`.
    ///
    /// # Errors
    ///
    /// Returns `Err` when:
    /// - No rate is available for the requested currency pair
    /// - The computed rate is non-finite or non-positive
    fn rate(
        &self,
        from: Currency,
        to: Currency,
        on: Date,
        policy: FxConversionPolicy,
    ) -> crate::Result<f64>;

    /// Return all stored quotes for serialization.
    ///
    /// The default implementation returns an empty vec (appropriate for
    /// providers that compute rates on-the-fly). Providers that hold a
    /// quote map should override this to enable full FxMatrix round-trips.
    fn snapshot_quotes(&self) -> Vec<(Currency, Currency, f64)> {
        Vec::new()
    }
}
