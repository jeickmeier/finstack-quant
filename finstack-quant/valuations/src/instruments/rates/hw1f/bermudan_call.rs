//! Bermudan call provision shared across callable exotic rate products.

use finstack_quant_core::dates::Date;

/// Bermudan call provision for callable exotics.
///
/// Allows the issuer to terminate the note on specified call dates
/// at a specified call price (typically par). Currently consumed by the
/// Callable Range Accrual note (PRDC is not implemented, and the Snowball
/// pricer rejects callable provisions).
///
/// # Fields
///
/// - `call_dates`: Sorted ascending dates on which the issuer may call.
/// - `price_pct_of_par`: Redemption price in percent of par (`100.0` = par,
///   `102.0` = callable at 102).
/// - `lockout_periods`: Number of initial coupon periods during which
///   the call right cannot be exercised.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BermudanCallProvision {
    /// Dates on which the issuer can call (must be sorted ascending).
    #[serde(with = "finstack_quant_core::wire::dates")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
    )]
    pub call_dates: Vec<Date>,
    /// Redemption price in percent of par (`100.0` = par). The pricer pays
    /// `notional * price_pct_of_par / 100` at exercise.
    pub price_pct_of_par: f64,
    /// Lockout period in number of coupon periods before first call.
    pub lockout_periods: usize,
}

impl BermudanCallProvision {
    /// Create a new Bermudan call provision.
    ///
    /// # Arguments
    ///
    /// * `call_dates` - Dates on which the issuer can call (must be sorted ascending)
    /// * `price_pct_of_par` - Redemption price in percent of par (`100.0` =
    ///   par); must be positive
    /// * `lockout_periods` - Number of initial coupon periods before first call
    pub fn new(call_dates: Vec<Date>, price_pct_of_par: f64, lockout_periods: usize) -> Self {
        Self {
            call_dates,
            price_pct_of_par,
            lockout_periods,
        }
    }

    /// Validate the call provision.
    ///
    /// Checks:
    /// - At least one call date
    /// - Call dates are sorted ascending
    /// - Call price is positive
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        use crate::instruments::common_impl::validation;

        validation::require_with(!self.call_dates.is_empty(), || {
            "BermudanCallProvision requires at least one call date".to_string()
        })?;

        validation::validate_sorted_strict(&self.call_dates, "BermudanCallProvision call_dates")?;

        validation::require_with(self.price_pct_of_par > 0.0, || {
            format!(
                "call_provision.price_pct_of_par ({}) must be a positive percent of par",
                self.price_pct_of_par
            )
        })?;

        Ok(())
    }

    /// Return the call dates that are eligible given the lockout period,
    /// relative to a set of coupon dates.
    ///
    /// Returns only those call dates that fall on or after the coupon date
    /// at index `lockout_periods`.
    pub fn eligible_call_dates(&self, coupon_dates: &[Date]) -> Vec<Date> {
        if self.lockout_periods >= coupon_dates.len() {
            return Vec::new();
        }
        let lockout_end = coupon_dates[self.lockout_periods];
        self.call_dates
            .iter()
            .copied()
            .filter(|d| *d >= lockout_end)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    fn make_dates() -> Vec<Date> {
        vec![
            Date::from_calendar_date(2026, Month::June, 30).expect("valid"),
            Date::from_calendar_date(2027, Month::June, 30).expect("valid"),
            Date::from_calendar_date(2028, Month::June, 30).expect("valid"),
        ]
    }

    #[test]
    fn valid_call_provision() {
        let prov = BermudanCallProvision::new(make_dates(), 100.0, 1);
        assert!(prov.validate().is_ok());
    }

    #[test]
    fn empty_call_dates_fails() {
        let prov = BermudanCallProvision::new(vec![], 100.0, 0);
        assert!(prov.validate().is_err());
    }

    #[test]
    fn negative_price_pct_of_par_fails() {
        let prov = BermudanCallProvision::new(make_dates(), -50.0, 0);
        assert!(prov.validate().is_err());
    }

    #[test]
    // schema-rejection-test
    fn retired_call_price_key_is_rejected() {
        let err = serde_json::from_value::<BermudanCallProvision>(serde_json::json!({
            "call_dates": ["2026-06-30"],
            "call_price": 1.0,
            "price_pct_of_par": 100.0,
            "lockout_periods": 0
        }))
        .expect_err("call_price is retired");
        assert!(err.to_string().contains("call_price"), "{err}");
    }

    #[test]
    fn eligible_dates_respect_lockout() {
        let call_dates = make_dates();
        let coupon_dates = make_dates();
        let prov = BermudanCallProvision::new(call_dates.clone(), 100.0, 1);
        let eligible = prov.eligible_call_dates(&coupon_dates);
        // Lockout 1 means first eligible coupon is index 1 (2027-06-30)
        assert_eq!(eligible.len(), 2);
        assert_eq!(eligible[0], call_dates[1]);
    }

    #[test]
    fn lockout_exceeds_coupon_dates_returns_empty() {
        let prov = BermudanCallProvision::new(make_dates(), 100.0, 10);
        let coupon_dates = make_dates();
        assert!(prov.eligible_call_dates(&coupon_dates).is_empty());
    }
}
