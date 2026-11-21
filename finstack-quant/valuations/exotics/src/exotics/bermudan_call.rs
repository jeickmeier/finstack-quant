//! Bermudan call provision shared across callable exotic rate products.

use finstack_quant_core::dates::Date;

/// Bermudan call provision for callable exotics.
///
/// Allows the issuer to terminate the note on specified call dates
/// at a specified call price (typically par).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BermudanCallProvision {
    /// Dates on which the issuer can call (must be sorted ascending).
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
    )]
    pub call_dates: Vec<Date>,
    /// Redemption price in percent of par (`100.0` = par). The pricer pays
    /// `notional * price_pct_of_par / 100` at exercise.
    pub price_pct_of_par: f64,
    /// End of the no-call (lockout) period. Call dates on or before this
    /// date are dropped (exclusive bound); `None` means no lockout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
    pub lockout_end: Option<Date>,
}

impl BermudanCallProvision {
    /// Create a new Bermudan call provision.
    ///
    /// # Arguments
    ///
    /// * `call_dates` - Dates on which the issuer can call (must be sorted ascending)
    /// * `price_pct_of_par` - Redemption price in percent of par (`100.0` =
    ///   par); must be positive
    /// * `lockout_end` - End of the no-call period; call dates on or before
    ///   it are not exercisable. `None` means every call date is eligible.
    pub fn new(call_dates: Vec<Date>, price_pct_of_par: f64, lockout_end: Option<Date>) -> Self {
        Self {
            call_dates,
            price_pct_of_par,
            lockout_end,
        }
    }

    /// Validate the call provision.
    ///
    /// Checks:
    /// - At least one call date
    /// - Call dates are sorted ascending
    /// - Call price is finite and positive
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        use finstack_quant_core::validation;

        validation::require_with(!self.call_dates.is_empty(), || {
            "BermudanCallProvision requires at least one call date".to_string()
        })?;

        validation::validate_sorted_strict(&self.call_dates, "BermudanCallProvision call_dates")?;

        validation::require_with(
            self.price_pct_of_par.is_finite() && self.price_pct_of_par > 0.0,
            || {
                format!(
                    "call_provision.price_pct_of_par ({}) must be a finite positive percent of par",
                    self.price_pct_of_par
                )
            },
        )?;

        Ok(())
    }

    /// Return the call dates that are eligible given the lockout.
    ///
    /// Returns only those call dates strictly after `lockout_end`, or every
    /// call date when there is no lockout.
    pub fn eligible_call_dates(&self) -> Vec<Date> {
        self.call_dates
            .iter()
            .copied()
            .filter(|d| self.lockout_end.is_none_or(|end| *d > end))
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
        let prov = BermudanCallProvision::new(make_dates(), 100.0, None);
        assert!(prov.validate().is_ok());
    }

    #[test]
    fn empty_call_dates_fails() {
        let prov = BermudanCallProvision::new(vec![], 100.0, None);
        assert!(prov.validate().is_err());
    }

    #[test]
    fn non_finite_or_negative_price_pct_of_par_fails() {
        for bad in [-50.0, 0.0, f64::INFINITY, f64::NAN] {
            let prov = BermudanCallProvision::new(make_dates(), bad, None);
            assert!(prov.validate().is_err(), "expected rejection for {bad}");
        }
    }

    #[test]
    // schema-rejection-test
    fn retired_call_price_key_is_rejected() {
        let err = serde_json::from_value::<BermudanCallProvision>(serde_json::json!({
            "call_dates": ["2026-06-30"],
            "call_price": 1.0,
            "price_pct_of_par": 100.0
        }))
        .expect_err("call_price is retired");
        assert!(err.to_string().contains("call_price"), "{err}");
    }

    #[test]
    // schema-rejection-test
    fn retired_lockout_periods_key_is_rejected() {
        let err = serde_json::from_value::<BermudanCallProvision>(serde_json::json!({
            "call_dates": ["2026-06-30"],
            "price_pct_of_par": 100.0,
            "lockout_periods": 1
        }))
        .expect_err("lockout_periods is retired");
        assert!(err.to_string().contains("lockout_periods"), "{err}");
    }

    #[test]
    fn eligible_dates_respect_lockout() {
        let call_dates = make_dates();
        let prov = BermudanCallProvision::new(call_dates.clone(), 100.0, Some(call_dates[0]));
        assert_eq!(prov.eligible_call_dates(), call_dates[1..].to_vec());
        let open = BermudanCallProvision::new(call_dates.clone(), 100.0, None);
        assert_eq!(open.eligible_call_dates(), call_dates);
    }

    #[test]
    fn lockout_after_last_call_date_returns_empty() {
        let prov = BermudanCallProvision::new(
            make_dates(),
            100.0,
            Some(Date::from_calendar_date(2030, Month::June, 30).expect("valid")),
        );
        assert!(prov.eligible_call_dates().is_empty());
    }
}
