//! Amortization specification types for principal schedules.
//!
//! Defines how principal amortizes over time for instruments and cashflow legs.

use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::money::Money;

/// Amortization specification for principal over time.
///
/// Describes how principal amortizes or is exchanged during the life of the contract.
/// Used by instruments (e.g., bonds) and cashflow legs for consistent behavior.
#[derive(
    Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub enum AmortizationSpec {
    /// No amortization – principal remains constant until final redemption.
    #[default]
    None,
    /// Linear principal paydown towards a target final notional amount over all periods.
    LinearTo {
        /// Target remaining principal at the end of the amortization schedule.
        final_notional: Money,
    },
    /// Explicit schedule of remaining principal amounts after given dates.
    /// Each pair stores `(date, remaining_principal_after_date)`.
    StepRemaining {
        /// Ordered list of `(date, remaining_principal_after_date)`.
        #[schemars(with = "Vec<(String, Money)>")]
        schedule: Vec<(Date, Money)>,
    },
    /// Fixed percentage of **original** notional paid each period (capped by remaining outstanding).
    ///
    /// This is a sinking-fund style amortization where the payment amount is constant
    /// across periods: `initial_notional * pct`. It does **not** compound (i.e., it is
    /// NOT percentage-of-remaining / declining-balance / mortgage-style).
    PercentOfOriginalPerPeriod {
        /// Fraction of original notional paid per period (e.g., 0.05 = 5%).
        pct: f64,
    },
    /// Custom principal exchanges on specific dates (absolute cash amounts).
    /// Positive amounts reduce outstanding (i.e., principal paid by issuer).
    CustomPrincipal {
        /// List of `(date, principal_amount)` exchanges; amounts are absolute cashflows.
        #[schemars(with = "Vec<(String, Money)>")]
        items: Vec<(Date, Money)>,
    },
}

/// Notional amount with an optional amortisation rule.
///
/// Combines initial principal with amortization behavior for complete
/// notional lifecycle management.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Notional {
    /// Initial principal amount outstanding at leg inception.
    pub initial: Money,
    /// Amortisation rule applied after each period.
    pub amort: AmortizationSpec,
}

impl Notional {
    /// Plain (non-amortising) notional helper.
    ///
    /// # Example
    /// ```rust
    /// use finstack_cashflows::builder::{Notional, AmortizationSpec};
    /// use finstack_core::currency::Currency;
    ///
    /// let notional = Notional::par(1_000_000.0, Currency::USD);
    /// assert_eq!(notional.initial.amount(), 1_000_000.0);
    /// assert!(matches!(notional.amort, AmortizationSpec::None));
    /// ```
    pub fn par(amount: f64, currency: Currency) -> Self {
        Self {
            initial: Money::new(amount, currency),
            amort: AmortizationSpec::None,
        }
    }

    /// Convenience accessor for currency.
    pub fn currency(&self) -> Currency {
        self.initial.currency()
    }

    /// Validates the notional and its amortization specification.
    ///
    /// # Validation Rules
    ///
    /// - `LinearTo`: Currency must match initial; final_notional must not exceed initial.
    /// - `StepRemaining`: Dates must be strictly increasing (sorted, no duplicates);
    ///   currencies must match; remaining amounts must be non-increasing.
    /// - `PercentOfOriginalPerPeriod`: Percentage must be finite and in range `[0.0, 1.0]`.
    /// - `CustomPrincipal`: All currencies must match initial.
    ///
    /// # Errors
    ///
    /// Returns an error if any validation rule is violated.
    pub fn validate(&self) -> finstack_core::Result<()> {
        let currency = self.initial.currency();

        match &self.amort {
            AmortizationSpec::None => Ok(()),
            AmortizationSpec::LinearTo { final_notional } => {
                if final_notional.currency() != currency {
                    return Err(finstack_core::Error::Validation(format!(
                        "LinearTo final_notional currency ({}) must match initial currency ({})",
                        final_notional.currency(),
                        currency
                    )));
                }
                if final_notional.amount() < 0.0 {
                    return Err(finstack_core::Error::Validation(format!(
                        "LinearTo final_notional ({}) must be non-negative",
                        final_notional.amount()
                    )));
                }
                if final_notional.amount() > self.initial.amount() {
                    return Err(finstack_core::Error::Validation(format!(
                        "LinearTo final_notional ({}) cannot exceed initial notional ({})",
                        final_notional.amount(),
                        self.initial.amount()
                    )));
                }
                Ok(())
            }
            AmortizationSpec::StepRemaining { schedule } => {
                // Check dates are strictly increasing and currencies match
                let mut prev_date: Option<Date> = None;
                let mut prev_amount: Option<f64> = None;

                for (date, remaining) in schedule {
                    // Currency check
                    if remaining.currency() != currency {
                        return Err(finstack_core::Error::Validation(format!(
                            "StepRemaining currency ({}) must match initial currency ({})",
                            remaining.currency(),
                            currency
                        )));
                    }

                    // Date ordering check (must be strictly increasing)
                    if let Some(pd) = prev_date {
                        if *date <= pd {
                            return Err(finstack_core::Error::Validation(format!(
                                "StepRemaining dates must be strictly increasing; found {} after {}",
                                date, pd
                            )));
                        }
                    }

                    // Amount ordering check (must be non-increasing)
                    if let Some(pa) = prev_amount {
                        if remaining.amount() > pa {
                            return Err(finstack_core::Error::Validation(format!(
                                "StepRemaining amounts must be non-increasing; found {} after {}",
                                remaining.amount(),
                                pa
                            )));
                        }
                    }

                    prev_date = Some(*date);
                    prev_amount = Some(remaining.amount());
                }
                Ok(())
            }
            AmortizationSpec::PercentOfOriginalPerPeriod { pct } => {
                if !pct.is_finite() {
                    return Err(finstack_core::Error::Validation(format!(
                        "PercentOfOriginalPerPeriod pct must be finite; got {}",
                        pct
                    )));
                }
                if *pct < 0.0 || *pct > 1.0 {
                    return Err(finstack_core::Error::Validation(format!(
                        "PercentOfOriginalPerPeriod pct must be in [0.0, 1.0]; got {}",
                        pct
                    )));
                }
                Ok(())
            }
            AmortizationSpec::CustomPrincipal { items } => {
                let mut total_amort = 0.0;
                for (_date, amount) in items {
                    if amount.currency() != currency {
                        return Err(finstack_core::Error::Validation(format!(
                            "CustomPrincipal currency ({}) must match initial currency ({})",
                            amount.currency(),
                            currency
                        )));
                    }
                    // Track total amortization (positive amounts reduce outstanding)
                    if amount.amount() > 0.0 {
                        total_amort += amount.amount();
                    }
                }
                // Validate total amortization doesn't exceed initial notional
                if total_amort > self.initial.amount() {
                    return Err(finstack_core::Error::Validation(format!(
                        "CustomPrincipal total amortization ({:.2}) exceeds initial notional ({:.2})",
                        total_amort,
                        self.initial.amount()
                    )));
                }
                Ok(())
            }
        }
    }
}
