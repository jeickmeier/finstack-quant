//! Guaranteed minimum-return ("return floor") call protection.
//!
//! A [`ReturnFloorSpec`] declares that, if the issuer redeems early, the
//! redemption is floored so the investor's realized return meets a target —
//! either a money multiple (MOIC) or an internal rate of return (XIRR). The
//! spec is lowered into a concrete [`crate::instruments::fixed_income::bond::CallPutSchedule`]
//! at pricing time; see `bond/pricing/return_floor.rs`.
//!
//! # Quick Example
//!
//! ```rust
//! use finstack_quant_valuations::instruments::fixed_income::bond::{
//!     ReturnFloorSpec, ReturnFloorKind, IssuePrice, ProtectionWindow,
//! };
//! use finstack_quant_core::types::Rate;
//!
//! // MOIC floor of 1.25× with default par issue price and full protection window
//! let moic_floor = ReturnFloorSpec::moic(1.25);
//! assert!(matches!(moic_floor.kind, ReturnFloorKind::Moic(_)));
//!
//! // XIRR floor at 12% with OID issue price
//! let xirr_floor = ReturnFloorSpec::xirr(Rate::from_percent(12.0))
//!     .issue_price(IssuePrice::PctOfPar(98.0));
//! assert!(matches!(xirr_floor.issue_price, IssuePrice::PctOfPar(_)));
//! ```

use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Rate;

/// Which return metric is guaranteed by the floor, and its target.
///
/// Use [`ReturnFloorKind::Moic`] for money-multiple protection (common in
/// leveraged loans and private credit) or [`ReturnFloorKind::Xirr`] for
/// annualized IRR protection.
#[derive(
    Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[non_exhaustive]
pub enum ReturnFloorKind {
    /// Minimum money-on-money multiple (e.g. `1.25` = 1.25× invested capital).
    ///
    /// On an early issuer-called redemption at time `t`, the redemption price is
    /// the minimum amount such that `(received_cashflows + redemption) / V0 >=
    /// multiple`, where `V0` is the invested capital defined by [`IssuePrice`].
    /// The floor binds only on early redemptions, not at maturity.
    Moic(f64),
    /// Minimum annualized internal rate of return.
    ///
    /// On an early issuer-called redemption at time `t`, the redemption price is
    /// computed so that the XIRR of all cashflows from issue equals the target
    /// rate. The floor binds only on early redemptions, not at maturity. Day
    /// count defaults to Act/365F (matching `core::cashflow::xirr`) unless
    /// overridden via [`ReturnFloorSpec::day_count`].
    Xirr(#[schemars(with = "f64")] Rate),
}

/// Issue price = invested capital `V0` (amount funded at issue; IRR initial outflow
/// and MOIC denominator).
///
/// Defaults to [`IssuePrice::Par`] when not specified.
#[derive(
    Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[non_exhaustive]
pub enum IssuePrice {
    /// Par notional (default). `V0 = notional`.
    Par,
    /// Explicit cash amount funded at issue. Must match the bond's notional currency.
    Amount(Money),
    /// Percent of par, e.g. `98.0` for 2 points of original issue discount.
    /// `V0 = notional * pct / 100`.
    PctOfPar(f64),
}

/// When the return-floor protection window applies.
///
/// The window defines the interval over which the issuer can trigger a
/// floor-protected early redemption. Outside this window the floor is
/// inactive (the bond behaves as uncallable or follows its normal call
/// schedule). The floor binds only on early issuer-called redemptions; it
/// never applies at maturity.
#[derive(
    Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[non_exhaustive]
pub enum ProtectionWindow {
    /// From issue (or `as_of`) up to — but not including — maturity (default).
    /// The floor applies to any early issuer-called redemption during the
    /// bond's life, but not to redemption at maturity.
    Full,
    /// Active only on or after this date (encodes a no-call period ending at
    /// maturity). The floor is not applied to redemptions before `from`.
    From(#[schemars(with = "String")] Date),
    /// Explicit closed interval `[start, end]` (both ends inclusive). The floor
    /// applies only for redemptions with `start <= redemption_date <= end`.
    Between {
        /// First date on which the floor-protected call window opens.
        #[schemars(with = "String")]
        start: Date,
        /// Last date on which the floor-protected call window closes (inclusive).
        #[schemars(with = "String")]
        end: Date,
    },
}

/// Guaranteed minimum-return call protection on a bond or loan.
///
/// Attaching this spec declares the instrument **prepayable across the
/// protection window** with the floor as the redemption price — the standard
/// private-credit loan model. The floor is an issuer-side term anchored at the
/// issue date and issue price. The spec is lowered into a concrete call
/// schedule at pricing time; see `bond/pricing/return_floor.rs`.
///
/// # Invariants
///
/// - MOIC multiple must be positive (`> 0`).
/// - XIRR rate must be finite and greater than `-1` (i.e., `-100%`).
/// - A [`ProtectionWindow::Between`] window must have `start < end`.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::fixed_income::bond::{
///     ReturnFloorSpec, IssuePrice,
/// };
/// use finstack_quant_core::types::Rate;
///
/// // 1.20× MOIC floor at par, full protection window
/// let spec = ReturnFloorSpec::moic(1.20);
/// assert!(spec.validate().is_ok());
///
/// // 10% XIRR floor with 98 OID, validated
/// let spec = ReturnFloorSpec::xirr(Rate::from_percent(10.0))
///     .issue_price(IssuePrice::PctOfPar(98.0));
/// assert!(spec.validate().is_ok());
/// ```
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ReturnFloorSpec {
    /// Which return metric is guaranteed, and its target.
    pub kind: ReturnFloorKind,
    /// Issue price = invested capital `V0` (amount funded at issue). Default: par.
    #[serde(default = "default_issue_price")]
    pub issue_price: IssuePrice,
    /// Protection window. Default: [`ProtectionWindow::Full`].
    #[serde(default = "default_window")]
    pub window: ProtectionWindow,
    /// Day count convention for XIRR discounting. Defaults to Act/365F
    /// (matching `core::cashflow::xirr`) when `None`. Ignored for MOIC floors.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub day_count: Option<DayCount>,
}

fn default_issue_price() -> IssuePrice {
    IssuePrice::Par
}

fn default_window() -> ProtectionWindow {
    ProtectionWindow::Full
}

impl ReturnFloorSpec {
    /// Construct a minimum-MOIC floor.
    ///
    /// Defaults: issue price = par, protection window = full lifetime.
    ///
    /// # Arguments
    ///
    /// * `multiple` — target money-on-money multiple, e.g. `1.25` for 1.25×.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::instruments::fixed_income::bond::ReturnFloorSpec;
    ///
    /// let spec = ReturnFloorSpec::moic(1.25);
    /// assert!(spec.validate().is_ok());
    /// ```
    pub fn moic(multiple: f64) -> Self {
        Self {
            kind: ReturnFloorKind::Moic(multiple),
            issue_price: default_issue_price(),
            window: default_window(),
            day_count: None,
        }
    }

    /// Construct a minimum-XIRR floor.
    ///
    /// Defaults: issue price = par, protection window = full lifetime.
    ///
    /// # Arguments
    ///
    /// * `rate` — target annualized internal rate of return (e.g. `Rate::from_percent(12.0)`).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::instruments::fixed_income::bond::ReturnFloorSpec;
    /// use finstack_quant_core::types::Rate;
    ///
    /// let spec = ReturnFloorSpec::xirr(Rate::from_percent(12.0));
    /// assert!(spec.validate().is_ok());
    /// ```
    pub fn xirr(rate: impl Into<Rate>) -> Self {
        Self {
            kind: ReturnFloorKind::Xirr(rate.into()),
            issue_price: default_issue_price(),
            window: default_window(),
            day_count: None,
        }
    }

    /// Set the issue price (invested capital `V0`).
    ///
    /// # Returns
    ///
    /// `self` with `issue_price` updated (fluent builder).
    #[must_use]
    pub fn issue_price(mut self, price: IssuePrice) -> Self {
        self.issue_price = price;
        self
    }

    /// Set the protection window.
    ///
    /// # Returns
    ///
    /// `self` with `window` updated (fluent builder).
    #[must_use]
    pub fn window(mut self, window: ProtectionWindow) -> Self {
        self.window = window;
        self
    }

    /// Set the XIRR discounting day count convention.
    ///
    /// Has no effect on MOIC floors.
    ///
    /// # Returns
    ///
    /// `self` with `day_count` updated (fluent builder).
    #[must_use]
    pub fn day_count(mut self, dc: DayCount) -> Self {
        self.day_count = Some(dc);
        self
    }

    /// Validate the spec independently of any bond.
    ///
    /// # Errors
    ///
    /// Returns `Err` if:
    /// - The MOIC multiple is `<= 0`.
    /// - The XIRR rate is not finite or is `<= -1` (i.e., `-100%` or worse).
    /// - A [`ProtectionWindow::Between`] window has `start >= end`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::instruments::fixed_income::bond::ReturnFloorSpec;
    ///
    /// assert!(ReturnFloorSpec::moic(1.25).validate().is_ok());
    /// assert!(ReturnFloorSpec::moic(0.0).validate().is_err());
    /// ```
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        match self.kind {
            ReturnFloorKind::Moic(m) if m <= 0.0 => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "return floor MOIC must be > 0, got {m}"
                )));
            }
            ReturnFloorKind::Xirr(r) if r.as_decimal() <= -1.0 || !r.as_decimal().is_finite() => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "return floor XIRR must be finite and > -1, got {}",
                    r.as_decimal()
                )));
            }
            _ => {}
        }
        if let ProtectionWindow::Between { start, end } = self.window {
            if start >= end {
                return Err(finstack_quant_core::Error::Validation(
                    "return floor window: start must be before end".to_string(),
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::types::Rate;

    #[test]
    fn moic_constructor_defaults() {
        let spec = ReturnFloorSpec::moic(1.25);
        assert!(matches!(spec.kind, ReturnFloorKind::Moic(m) if (m - 1.25).abs() < 1e-12));
        assert!(matches!(spec.issue_price, IssuePrice::Par));
        assert!(matches!(spec.window, ProtectionWindow::Full));
        assert!(spec.day_count.is_none());
    }

    #[test]
    fn xirr_constructor_and_fluent_setters() {
        let spec =
            ReturnFloorSpec::xirr(Rate::from_percent(12.0)).issue_price(IssuePrice::PctOfPar(98.0));
        assert!(matches!(spec.kind, ReturnFloorKind::Xirr(_)));
        assert!(matches!(spec.issue_price, IssuePrice::PctOfPar(p) if (p - 98.0).abs() < 1e-12));
    }

    #[test]
    fn validate_rejects_nonpositive_multiple() {
        assert!(ReturnFloorSpec::moic(0.0).validate().is_err());
    }

    #[test]
    fn validate_rejects_xirr_at_or_below_minus_one() {
        assert!(ReturnFloorSpec::xirr(Rate::from_decimal(-1.0))
            .validate()
            .is_err());
    }

    #[test]
    fn validate_rejects_degenerate_between_window() {
        use time::macros::date;

        // start == end
        let equal = ReturnFloorSpec::moic(1.25).window(ProtectionWindow::Between {
            start: date!(2027 - 01 - 01),
            end: date!(2027 - 01 - 01),
        });
        assert!(equal.validate().is_err());

        // start > end
        let inverted = ReturnFloorSpec::moic(1.25).window(ProtectionWindow::Between {
            start: date!(2028 - 01 - 01),
            end: date!(2027 - 01 - 01),
        });
        assert!(inverted.validate().is_err());
    }

    #[test]
    fn validate_accepts_valid_spec() {
        assert!(ReturnFloorSpec::moic(1.25).validate().is_ok());
    }
}
