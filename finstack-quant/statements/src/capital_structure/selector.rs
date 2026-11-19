//! Typed selectors for capital-structure reporting values.
//!
//! A formula reads a capital value as `cs.<component>.<scope>`. The component
//! names one reported amount and the scope names whose amount it is: one
//! instrument in its own currency, every instrument of one native currency,
//! or the whole structure consolidated into the reporting currency.

use crate::capital_structure::CashflowBreakdown;
use crate::error::{Error, Result};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use std::fmt;
use std::str::FromStr;

/// One reported capital-structure amount.
///
/// Flows are positive payment or receipt magnitudes of the period; balances
/// are period-close amounts. The four principal components sum to
/// `principal_payment`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CsComponent {
    /// Gross interest expense: cash interest expense plus PIK.
    InterestExpense,
    /// Cash interest expense paid.
    InterestExpenseCash,
    /// Interest capitalized into principal.
    InterestExpensePik,
    /// Cash interest income received.
    InterestIncome,
    /// Cash interest expense less cash interest income.
    NetInterestExpenseCash,
    /// All principal cash paid: scheduled, mandatory, voluntary and sweep.
    PrincipalPayment,
    /// Scheduled principal cash paid.
    ScheduledPrincipal,
    /// Mandatory prepayment cash paid.
    MandatoryPrepayment,
    /// Voluntary prepayment cash paid.
    VoluntaryPrepayment,
    /// Excess-cash-flow sweep cash paid.
    SweepPrepayment,
    /// Scheduled cash interest that fell due on its contractual date in the
    /// period, whether or not it was paid.
    ScheduledCashInterestDue,
    /// Scheduled principal that fell due on its contractual date in the
    /// period, whether or not it was paid.
    ScheduledPrincipalDue,
    /// Fees paid.
    Fees,
    /// Economic principal outstanding at the period open.
    OpeningBalance,
    /// Economic principal outstanding at the period close.
    DebtBalance,
    /// Cash interest earned and not yet due at the period close.
    AccruedInterest,
    /// Cash interest due and unpaid at the period close.
    InterestArrears,
    /// Fee cash received back in the period.
    FeeRebates,
    /// Principal cash received back in the period.
    RepaymentRefunds,
    /// Signed company cash of funding settled in the period.
    FundingProceeds,
    /// Principal established by funding effective in the period.
    FundingPrincipal,
    /// Principal cash owed and not yet due at the period close.
    PrincipalPending,
    /// Principal cash due and unpaid at the period close.
    PrincipalArrears,
    /// Face already paid for whose reduction is not yet effective.
    PrincipalAdvanceReserve,
}

impl CsComponent {
    /// Every component, in documentation order.
    pub const ALL: [Self; 24] = [
        Self::InterestExpense,
        Self::InterestExpenseCash,
        Self::InterestExpensePik,
        Self::InterestIncome,
        Self::NetInterestExpenseCash,
        Self::PrincipalPayment,
        Self::ScheduledPrincipal,
        Self::MandatoryPrepayment,
        Self::VoluntaryPrepayment,
        Self::SweepPrepayment,
        Self::ScheduledCashInterestDue,
        Self::ScheduledPrincipalDue,
        Self::Fees,
        Self::OpeningBalance,
        Self::DebtBalance,
        Self::AccruedInterest,
        Self::InterestArrears,
        Self::PrincipalPending,
        Self::PrincipalArrears,
        Self::PrincipalAdvanceReserve,
        Self::FeeRebates,
        Self::RepaymentRefunds,
        Self::FundingProceeds,
        Self::FundingPrincipal,
    ];

    /// The component's name in a `cs.<component>.<scope>` reference.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InterestExpense => "interest_expense",
            Self::InterestExpenseCash => "interest_expense_cash",
            Self::InterestExpensePik => "interest_expense_pik",
            Self::InterestIncome => "interest_income",
            Self::NetInterestExpenseCash => "net_interest_expense_cash",
            Self::PrincipalPayment => "principal_payment",
            Self::ScheduledPrincipal => "scheduled_principal",
            Self::MandatoryPrepayment => "mandatory_prepayment",
            Self::VoluntaryPrepayment => "voluntary_prepayment",
            Self::SweepPrepayment => "sweep_prepayment",
            Self::ScheduledCashInterestDue => "scheduled_cash_interest_due",
            Self::ScheduledPrincipalDue => "scheduled_principal_due",
            Self::Fees => "fees",
            Self::OpeningBalance => "opening_balance",
            Self::DebtBalance => "debt_balance",
            Self::AccruedInterest => "accrued_interest",
            Self::InterestArrears => "interest_arrears",
            Self::PrincipalPending => "principal_pending",
            Self::PrincipalArrears => "principal_arrears",
            Self::PrincipalAdvanceReserve => "principal_advance_reserve",
            Self::FeeRebates => "fee_rebates",
            Self::RepaymentRefunds => "repayment_refunds",
            Self::FundingProceeds => "funding_proceeds",
            Self::FundingPrincipal => "funding_principal",
        }
    }

    /// Read this component from one breakdown, exactly.
    ///
    /// # Arguments
    ///
    /// * `flow` - The breakdown of one instrument or one total in one period.
    ///
    /// # Errors
    ///
    /// Returns a capital-structure error when the breakdown does not record
    /// the component (an unknown amount is never read as zero), or when cash
    /// and PIK interest disagree on currency.
    pub fn read(self, flow: &CashflowBreakdown) -> Result<Money> {
        let recorded = |amount: Option<Money>| {
            amount.ok_or_else(|| {
                Error::capital_structure(format!(
                    "{} is not recorded on this cashflow breakdown",
                    self.as_str()
                ))
            })
        };
        match self {
            Self::InterestExpense => flow.interest_expense_total(),
            Self::InterestExpenseCash => Ok(flow.interest_expense_cash),
            Self::InterestExpensePik => Ok(flow.interest_expense_pik),
            Self::InterestIncome => Ok(flow.interest_income_cash_or_zero()),
            Self::NetInterestExpenseCash => flow.net_interest_expense_cash(),
            Self::PrincipalPayment => Ok(flow.principal_payment),
            Self::ScheduledPrincipal => recorded(flow.scheduled_principal),
            Self::MandatoryPrepayment => recorded(flow.mandatory_prepayment),
            Self::VoluntaryPrepayment => recorded(flow.voluntary_prepayment),
            Self::SweepPrepayment => recorded(flow.sweep_prepayment),
            Self::ScheduledCashInterestDue => recorded(flow.scheduled_cash_interest_due),
            Self::ScheduledPrincipalDue => recorded(flow.scheduled_principal_due),
            Self::Fees => Ok(flow.fees),
            Self::OpeningBalance => recorded(flow.opening_balance),
            Self::DebtBalance => Ok(flow.debt_balance),
            Self::AccruedInterest => Ok(flow.accrued_interest),
            Self::InterestArrears => recorded(flow.interest_arrears),
            Self::PrincipalPending => recorded(flow.principal_pending),
            Self::PrincipalArrears => recorded(flow.principal_arrears),
            Self::PrincipalAdvanceReserve => recorded(flow.principal_advance_reserve),
            Self::FeeRebates => recorded(flow.fee_rebates),
            Self::RepaymentRefunds => recorded(flow.repayment_refunds),
            Self::FundingProceeds => recorded(flow.funding_proceeds),
            Self::FundingPrincipal => recorded(flow.funding_principal),
        }
    }
}

impl fmt::Display for CsComponent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for CsComponent {
    type Err = Error;

    fn from_str(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|component| component.as_str() == name)
            .ok_or_else(|| {
                let valid: Vec<&str> = Self::ALL.iter().map(|c| c.as_str()).collect();
                Error::capital_structure(format!(
                    "Unknown capital structure component: '{name}'. Valid components: {}",
                    valid.join(", ")
                ))
            })
    }
}

/// Whose amount a capital-structure reference reads.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CsScope {
    /// One instrument, in its own currency: `cs.<component>.<instrument_id>`.
    Instrument(String),
    /// Every instrument of one native currency, in that currency:
    /// `cs.<component>.total.<CCY>`.
    NativeTotal(Currency),
    /// The whole structure in the run's reporting currency:
    /// `cs.<component>.total`. With more than one native currency it needs an
    /// explicit reporting currency and period-close FX.
    Consolidated,
}

impl CsScope {
    /// The scope segment that selects the totals.
    pub const TOTAL: &'static str = "total";
}

impl fmt::Display for CsScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Instrument(id) => f.write_str(id),
            Self::NativeTotal(currency) => write!(f, "{}.{currency}", Self::TOTAL),
            Self::Consolidated => f.write_str(Self::TOTAL),
        }
    }
}

impl FromStr for CsScope {
    type Err = Error;

    fn from_str(scope: &str) -> Result<Self> {
        if scope == Self::TOTAL {
            return Ok(Self::Consolidated);
        }
        if let Some(code) = scope.strip_prefix("total.") {
            return code
                .parse::<Currency>()
                .map(Self::NativeTotal)
                .map_err(|_| {
                    Error::capital_structure(format!(
                        "invalid capital-structure scope '{scope}': '{code}' is not a currency code"
                    ))
                });
        }
        let valid = !scope.is_empty()
            && scope
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-');
        if !valid {
            return Err(Error::capital_structure(format!(
                "invalid capital-structure scope '{scope}': use an instrument id of letters, \
                 digits, underscores and hyphens, `total`, or `total.<CCY>`"
            )));
        }
        Ok(Self::Instrument(scope.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn components_and_scopes_round_trip_their_text() {
        for component in CsComponent::ALL {
            assert_eq!(
                component.as_str().parse::<CsComponent>().ok(),
                Some(component)
            );
        }
        assert!("coupon".parse::<CsComponent>().is_err());
        for (text, scope) in [
            ("TL-A", CsScope::Instrument("TL-A".into())),
            ("total", CsScope::Consolidated),
            ("total.EUR", CsScope::NativeTotal(Currency::EUR)),
        ] {
            assert_eq!(text.parse::<CsScope>().ok().as_ref(), Some(&scope));
            assert_eq!(scope.to_string(), text);
        }
        for text in ["", "total.euro", "a b", "total.EUR.x"] {
            assert!(text.parse::<CsScope>().is_err(), "{text}");
        }
    }
}
