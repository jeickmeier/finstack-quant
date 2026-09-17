//! Assumed optional redemption ("price to call") for structured-credit
//! notes.
//!
//! CLO and ABS notes are callable by the equity holder after the
//! non-call period, so desks quote them to an assumed call date as well as
//! to maturity. A [`CallAssumption`] on the deal makes the projection redeem
//! on the first payment date at or after `date`: a deal-scope call sells the
//! collateral at the deal's `liquidation_price_pct` and pays every note at
//! `price_pct` plus accrued and deferred interest; a tranche-scope call
//! prices one class to its refinancing while the deal's own cashflows are
//! unchanged. `TrancheMetrics` reports the to-maturity figures alongside
//! their `*_to_call` twins.

use finstack_quant_core::dates::Date;
use serde::{Deserialize, Serialize};

/// Which notes an assumed call redeems.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CallScope {
    /// The whole deal is called: collateral is liquidated and every note is
    /// redeemed; the projection ends on the call date.
    Deal,
    /// One class is refinanced at the call price; the deal's cashflows are
    /// unchanged and only that class's `*_to_call` metrics truncate.
    Tranche(String),
}

/// Assumed optional redemption on a date at a price.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::Date;
/// use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
///     CallAssumption, CallScope,
/// };
/// use time::Month;
///
/// let date = Date::from_calendar_date(2027, Month::January, 15).unwrap();
/// let call = CallAssumption::new(date, 100.0);
/// assert_eq!(call.scope, CallScope::Deal);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CallAssumption {
    /// Earliest redemption date; the call settles on the first payment date
    /// at or after it.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// Redemption price as a percent of the notes' current balance
    /// (`100.0` = par; above par the premium is paid as interest, below par
    /// the shortfall is a principal write-down).
    pub price_pct: f64,
    /// Whole deal or a single class.
    pub scope: CallScope,
}

impl CallAssumption {
    /// Deal-scope call on `date` at `price_pct`.
    ///
    /// # Arguments
    ///
    /// * `date` - Earliest redemption date.
    /// * `price_pct` - Redemption price as a percent of current balance.
    ///
    /// # Returns
    ///
    /// A call that redeems every note.
    pub fn new(date: Date, price_pct: f64) -> Self {
        Self {
            date,
            price_pct,
            scope: CallScope::Deal,
        }
    }

    /// Tranche-scope call on `date` at `price_pct`.
    ///
    /// # Arguments
    ///
    /// * `date` - Earliest redemption date.
    /// * `price_pct` - Redemption price as a percent of current balance.
    /// * `tranche_id` - Class priced to the call.
    ///
    /// # Returns
    ///
    /// A call that only truncates the named class's `*_to_call` metrics.
    pub fn for_tranche(date: Date, price_pct: f64, tranche_id: impl Into<String>) -> Self {
        Self {
            date,
            price_pct,
            scope: CallScope::Tranche(tranche_id.into()),
        }
    }

    /// Validate the price and, for a tranche-scope call, the class.
    ///
    /// # Arguments
    ///
    /// * `tranche_ids` - Debt classes of the deal the call may name.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when the price is not finite and
    /// positive or the named class is unknown.
    pub fn validate<'a>(
        &self,
        mut tranche_ids: impl Iterator<Item = &'a str>,
    ) -> finstack_quant_core::Result<()> {
        if !self.price_pct.is_finite() || self.price_pct <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "call_assumption.price_pct ({}) must be a finite positive percent of par",
                self.price_pct
            )));
        }
        if let CallScope::Tranche(id) = &self.scope {
            if !tranche_ids.any(|candidate| candidate == id) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "call_assumption names unknown debt tranche '{id}'"
                )));
            }
        }
        Ok(())
    }
}
