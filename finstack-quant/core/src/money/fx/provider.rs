//! Provider traits and validation helpers for date-specific foreign-exchange rates.
//!
use crate::currency::Currency;
use crate::dates::Date;

use super::types::FxConversionPolicy;

/// Helper to compute reciprocal rate safely, validating the output.
///
/// Returns `1.0 / rate` if the reciprocal is a valid FX rate (finite and
/// strictly positive), otherwise returns an error. Validating the **output**
/// matters: a subnormal input such as `1e-320` passes a finiteness check on
/// the input but its reciprocal overflows to `+inf` (2026-06-09 core quant
/// review). This consolidates the reciprocal logic used across FX providers
/// and matrix lookups.
#[inline]
pub(crate) fn reciprocal_rate_or_err(
    rate: f64,
    from: Currency,
    to: Currency,
) -> crate::Result<f64> {
    if !rate.is_finite() {
        return Err(crate::error::InputError::NonFiniteValue {
            kind: crate::error::NonFiniteKind::classify(rate),
        }
        .into());
    }
    if rate == 0.0 {
        return Err(crate::error::InputError::NotFound {
            id: format!("FX:{from}->{to} (zero reciprocal)"),
        }
        .into());
    }
    let reciprocal = 1.0 / rate;
    // Validate the OUTPUT: tiny (subnormal) or negative inputs produce a
    // reciprocal that is infinite, zero, or negative — never a valid FX rate.
    validate_fx_rate(to, from, reciprocal)
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
/// use finstack_quant_core::money::fx::{FxConversionPolicy, FxProvider};
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::dates::Date;
/// use time::Month;
///
/// fn convert_amount<P: FxProvider>(
///     provider: &P,
///     amount: f64,
///     from: Currency,
///     to: Currency,
///     on: Date,
/// ) -> finstack_quant_core::Result<f64> {
///     let rate = provider.rate(from, to, on, FxConversionPolicy::CashflowDate)?;
///     Ok(amount * rate)
/// }
/// ```
///
/// ## Implementing the trait
///
/// ```rust
/// use finstack_quant_core::money::fx::{FxConversionPolicy, FxProvider};
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::dates::Date;
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

    /// Return the revision of the provider's current quote state, if cacheable.
    ///
    /// `Some(revision)` permits matrices to reuse observations only while this
    /// revision remains unchanged. Every change that can affect a rate must
    /// publish a different revision before the mutation completes; a revision
    /// must never be reused for different quote state. Immutable providers can
    /// return `Some(0)`.
    ///
    /// The default `None` disables observation caching. Use it for live or
    /// externally updated providers that cannot track every quote change. The
    /// matrix still honors its explicit global quotes and pinned fixings.
    fn get_revision(&self) -> Option<u64> {
        None
    }

    /// Return all stored quotes for serialization.
    ///
    /// The default implementation returns an empty vec (appropriate for
    /// providers that compute rates on-the-fly). Providers that hold a
    /// quote map should override this and [`get_revision`](Self::get_revision)
    /// to enable coherent FxMatrix round-trips.
    fn snapshot_quotes(&self) -> Vec<(Currency, Currency, f64)> {
        Vec::new()
    }

    /// Return stored date- and policy-scoped quotes for serialization.
    ///
    /// Each tuple contains the source currency, target currency, observation
    /// date, conversion policy, and units of target currency per source unit.
    /// These quotes override this provider's pair-global snapshot quotes for
    /// their scope, but remain below explicit quotes in the containing matrix.
    /// Implementations must use stored snapshot data without querying live
    /// rates or freezing a reference-date observation. The default is empty
    /// for providers without serializable scoped state. Providers exposing
    /// stored quotes must also implement [`get_revision`](Self::get_revision)
    /// so matrices can detect updates between snapshot hooks.
    fn snapshot_pinned_quotes(&self) -> Vec<(Currency, Currency, Date, FxConversionPolicy, f64)> {
        Vec::new()
    }
}
