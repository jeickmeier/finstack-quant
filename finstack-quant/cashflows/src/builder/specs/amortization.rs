//! Amortization specification types for principal schedules.
//!
//! Defines how principal amortizes over time for instruments and cashflow legs.

use std::hash::{Hash, Hasher};

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;

/// Amortization specification for principal over time.
///
/// Describes how principal amortizes or is exchanged during the life of the contract.
/// Used by instruments (e.g., bonds) and cashflow legs for consistent behavior.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub enum AmortizationSpec {
    /// No amortization – principal remains constant until final redemption.
    #[default]
    None,
    /// Linear principal paydown towards a target final notional amount over all periods.
    LinearTo {
        /// Target remaining principal at the end of the amortization schedule.
        final_notional: Money,
    },
    /// Explicit schedule of remaining principal amounts after given economic dates.
    /// Targets include PIK capitalized and principal events effective on that date.
    /// A target may exceed initial or earlier remaining principal when funded by
    /// intervening draws or PIK, but must not exceed the live balance on its date.
    /// Each pair stores `(date, remaining_principal_after_date)`.
    StepRemaining {
        /// Ordered list of `(date, remaining_principal_after_date)`.
        #[serde(with = "finstack_quant_core::wire::dated_money_values")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "Vec<(finstack_quant_core::wire::DateWire, Money)>")
        )]
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
    /// Fixed percentage of the **remaining** outstanding paid each period
    /// (declining balance): the payment is `outstanding * pct`, so it falls
    /// geometrically from period to period.
    PercentOfRemainingPerPeriod {
        /// Fraction of the outstanding principal paid per period (e.g.,
        /// 0.025 = 2.5%), in `[0, 1]`.
        pct: f64,
    },
    /// Equal principal installments on coupon accrual boundaries in `(start, end]`.
    /// The installment is fixed from outstanding after all start-date movements,
    /// including PIK and explicit principal events. The final installment pays
    /// the live remaining balance, including subsequent PIK or principal changes.
    /// Principal changes economically on each accrual boundary; cash settles on
    /// that coupon's adjusted, lagged payment date.
    LinearBetween {
        /// Economic amortization start, on or after issue; installments fall
        /// on coupon accrual boundaries strictly after it.
        #[serde(with = "finstack_quant_core::wire::date")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DateWire")
        )]
        start: Date,
        /// Economic end of amortization (full repayment), which must be an
        /// actual coupon accrual boundary on or before the effective terminal
        /// accrual date. Cash settlement may follow this date due to payment lag.
        #[serde(with = "finstack_quant_core::wire::date")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DateWire")
        )]
        end: Date,
    },
    /// Custom principal exchanges on specific dates (absolute cash amounts).
    /// Positive amounts reduce outstanding (i.e., principal paid by issuer).
    /// Each payment must be covered by the live balance, including prior draws
    /// and PIK. Lifetime repayments may therefore exceed initial principal.
    CustomPrincipal {
        /// List of `(date, principal_amount)` exchanges; amounts are absolute cashflows.
        #[serde(with = "finstack_quant_core::wire::dated_money_values")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "Vec<(finstack_quant_core::wire::DateWire, Money)>")
        )]
        items: Vec<(Date, Money)>,
    },
}

/// [`Hash`] is manual because the percentage variants carry an `f64`, which
/// cannot participate in a derived [`Eq`]/[`Hash`] impl.
impl Hash for AmortizationSpec {
    fn hash<H: Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
        match self {
            Self::None => {}
            Self::LinearTo { final_notional } => final_notional.hash(state),
            Self::StepRemaining { schedule } => schedule.hash(state),
            Self::PercentOfOriginalPerPeriod { pct }
            | Self::PercentOfRemainingPerPeriod { pct } => {
                // `f64` PartialEq treats `-0.0 == 0.0`; canonicalize before hashing.
                (pct + 0.0).to_bits().hash(state);
            }
            Self::LinearBetween { start, end } => {
                start.hash(state);
                end.hash(state);
            }
            Self::CustomPrincipal { items } => items.hash(state),
        }
    }
}

/// Notional amount with an optional amortisation rule.
///
/// Combines initial principal with amortization behavior for complete
/// notional lifecycle management.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
    /// use finstack_quant_cashflows::builder::{Notional, AmortizationSpec};
    /// use finstack_quant_core::currency::Currency;
    ///
    /// let notional = Notional::par(1_000_000.0, Currency::USD).expect("valid notional fixture");
    /// assert_eq!(notional.initial.amount(), 1_000_000.0);
    /// assert!(matches!(notional.amort, AmortizationSpec::None));
    /// ```
    pub fn par(amount: f64, currency: Currency) -> finstack_quant_core::Result<Self> {
        Ok(Self {
            initial: Money::new(amount, currency)?,
            amort: AmortizationSpec::None,
        })
    }

    /// Convenience accessor for currency.
    pub fn currency(&self) -> Currency {
        self.initial.currency()
    }

    /// Validates the notional and the structure of its amortization specification.
    ///
    /// Available principal for dated repayments and remaining-balance targets is
    /// checked during schedule building, after draws and PIK have been applied.
    ///
    /// # Validation Rules
    ///
    /// - `LinearTo`: Currency must match initial; final_notional must not exceed initial.
    /// - `StepRemaining`: Dates must be strictly increasing (sorted, no duplicates);
    ///   currencies must match; remaining amounts must be finite and non-negative.
    /// - `PercentOfOriginalPerPeriod` / `PercentOfRemainingPerPeriod`: Percentage must
    ///   be finite and in range `[0.0, 1.0]`.
    /// - `LinearBetween`: `start` must precede `end`.
    /// - `CustomPrincipal`: Amounts must be finite and non-negative; all
    ///   currencies must match initial. Repayment coverage depends on the live balance.
    ///
    /// # Errors
    ///
    /// Returns an error if any validation rule is violated.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        if !self.initial.amount().is_finite() {
            return Err(finstack_quant_core::Error::Validation(
                "initial notional must be finite".into(),
            ));
        }
        if self.initial.amount() < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "initial notional must be non-negative; got {}",
                self.initial.amount()
            )));
        }
        let currency = self.initial.currency();

        match &self.amort {
            AmortizationSpec::None => Ok(()),
            AmortizationSpec::LinearTo { final_notional } => {
                if final_notional.currency() != currency {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "LinearTo final_notional currency ({}) must match initial currency ({})",
                        final_notional.currency(),
                        currency
                    )));
                }
                if final_notional.amount() < 0.0 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "LinearTo final_notional ({}) must be non-negative",
                        final_notional.amount()
                    )));
                }
                if final_notional.amount() > self.initial.amount() {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "LinearTo final_notional ({}) cannot exceed initial notional ({})",
                        final_notional.amount(),
                        self.initial.amount()
                    )));
                }
                Ok(())
            }
            AmortizationSpec::StepRemaining { schedule } => {
                let mut prev_date: Option<Date> = None;

                for (date, remaining) in schedule {
                    if remaining.currency() != currency {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "StepRemaining currency ({}) must match initial currency ({})",
                            remaining.currency(),
                            currency
                        )));
                    }

                    if let Some(pd) = prev_date {
                        if *date <= pd {
                            return Err(finstack_quant_core::Error::Validation(format!(
                                "StepRemaining dates must be strictly increasing; found {} after {}",
                                date, pd
                            )));
                        }
                    }

                    if !remaining.amount().is_finite() || remaining.amount() < 0.0 {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "StepRemaining target {} must be finite and non-negative",
                            remaining.amount()
                        )));
                    }

                    prev_date = Some(*date);
                }
                Ok(())
            }
            AmortizationSpec::PercentOfOriginalPerPeriod { pct }
            | AmortizationSpec::PercentOfRemainingPerPeriod { pct } => {
                if !pct.is_finite() {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "amortization pct must be finite; got {}",
                        pct
                    )));
                }
                if *pct < 0.0 || *pct > 1.0 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "amortization pct must be in [0.0, 1.0]; got {}",
                        pct
                    )));
                }
                Ok(())
            }
            AmortizationSpec::LinearBetween { start, end } => {
                if start >= end {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "LinearBetween start {start} must precede end {end}"
                    )));
                }
                Ok(())
            }
            AmortizationSpec::CustomPrincipal { items } => {
                for (_date, amount) in items {
                    if amount.currency() != currency {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "CustomPrincipal currency ({}) must match initial currency ({})",
                            amount.currency(),
                            currency
                        )));
                    }
                    if !amount.amount().is_finite() || amount.amount() < 0.0 {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "CustomPrincipal amounts must be finite and non-negative; got {}",
                            amount.amount()
                        )));
                    }
                }
                Ok(())
            }
        }
    }
}
