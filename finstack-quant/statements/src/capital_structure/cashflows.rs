//! Cashflow reporting types for capital structure instruments.
//!
//! This module holds the aggregated cashflow DTOs produced by the evaluator
//! and exposed to the DSL via the `cs.*` namespace.

use crate::error::Result;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::money::Money;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Aggregated cashflows from capital structure instruments by period.
///
/// Instances of this type are produced by the evaluator and exposed to the DSL
/// via the `cs.*` namespace. It keeps both per-instrument details and totals so
/// that downstream consumers can drill down or report aggregates.
///
/// Monetary fields are stored as [`Money`] to preserve currency identity. The
/// accessor methods return raw `f64` amounts in the reporting currency for
/// convenience; callers that need full currency fidelity should inspect the
/// underlying maps directly.
///
/// # Example
///
/// ```rust
/// # use finstack_quant_statements::capital_structure::{CapitalStructureCashflows, CashflowBreakdown};
/// # use finstack_quant_core::dates::PeriodId;
/// # use finstack_quant_core::money::Money;
/// # use finstack_quant_core::currency::Currency;
/// let mut cs = CapitalStructureCashflows::new();
/// let period = PeriodId::quarter(2025, 1);
/// cs.by_instrument
///     .entry("BOND-1".into())
///     .or_default()
///     .insert(period, CashflowBreakdown {
///         interest_expense_cash: Money::new(10_000.0, Currency::USD),
///         interest_expense_pik: Money::new(2_500.0, Currency::USD),
///         principal_payment: Money::new(100_000.0, Currency::USD),
///         fees: Money::new(0.0, Currency::USD),
///         debt_balance: Money::new(4_900_000.0, Currency::USD),
///         accrued_interest: Money::new(5_000.0, Currency::USD),
///     });
/// cs.totals.insert(period, CashflowBreakdown {
///     interest_expense_cash: Money::new(10_000.0, Currency::USD),
///     interest_expense_pik: Money::new(2_500.0, Currency::USD),
///     principal_payment: Money::new(100_000.0, Currency::USD),
///     fees: Money::new(0.0, Currency::USD),
///     debt_balance: Money::new(4_900_000.0, Currency::USD),
///     accrued_interest: Money::new(5_000.0, Currency::USD),
/// });
///
/// assert_eq!(cs.get_total_interest(&period).unwrap(), 12_500.0);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapitalStructureCashflows {
    /// Map of instrument_id → (period_id → cashflow_type → amount)
    pub by_instrument: IndexMap<String, IndexMap<PeriodId, CashflowBreakdown>>,

    /// Total cashflows across all instruments in the reporting currency (if available)
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub totals: IndexMap<PeriodId, CashflowBreakdown>,

    /// Totals bucketed by native instrument currency
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub totals_by_currency: IndexMap<Currency, IndexMap<PeriodId, CashflowBreakdown>>,

    /// Reporting currency used for `totals` (if populated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reporting_currency: Option<Currency>,

    /// Post-waterfall residual cash distributed to equity per period.
    ///
    /// Only populated by waterfall evaluation when `available_cash_node` is
    /// configured: it is the cash remaining after fees, interest, and
    /// principal allocations. With the per-period allocations this satisfies
    /// `fees + cash interest + principal + equity == available cash`.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub equity_distribution: IndexMap<PeriodId, Money>,
}

/// Breakdown of cashflows by type for a single period.
///
/// Outflow-like fields such as interest, fees, and principal payments are
/// stored as positive amounts representing debt service paid or accrued during
/// the period.
///
/// Interest expense is split into cash and PIK components for visibility
/// into non-cash interest accrual. Use `interest_expense_total()` for the
/// combined value. All monetary fields use the Money type for currency safety.
///
/// Interest *expense* and interest *income* are tracked separately rather than
/// as one signed field: a two-leg instrument (an interest-rate swap) can net to
/// a receipt in a period — a pay-fixed hedge is in the money whenever the
/// floating leg exceeds the fixed leg — and a receipt is not debt service. The
/// waterfall allocates cash against expense claims, so folding a receipt into
/// `interest_expense_cash` as a negative claim would corrupt pro-rata
/// allocation. See [`Self::net_interest_expense_cash`] for the combined view.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CashflowBreakdown {
    /// Cash interest payments (coupons, floating resets)
    pub interest_expense_cash: Money,

    /// Net cash interest **received** during the period.
    ///
    /// Non-zero only for two-leg instruments whose legs net to a receipt (e.g.
    /// an in-the-money pay-fixed swap). Stored as a positive amount, like the
    /// outflow-oriented fields, and reported through the `cs.interest_income`
    /// namespace.
    ///
    /// `None` in payloads written before this field existed, which is
    /// semantically identical to the old behaviour (no income was tracked).
    /// Read it via [`Self::interest_income_cash_or_zero`] rather than
    /// unwrapping, so legacy payloads resolve to a zero in the correct
    /// currency.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interest_income_cash: Option<Money>,

    /// PIK (payment-in-kind) interest accrued but not paid in cash
    pub interest_expense_pik: Money,

    /// Principal repayments (amortization, maturity)
    pub principal_payment: Money,

    /// Fees (commitment fees, etc.)
    pub fees: Money,

    /// Outstanding debt balance at period end
    pub debt_balance: Money,

    /// Accrued interest not yet paid (liability)
    pub accrued_interest: Money,
}

impl CashflowBreakdown {
    /// Create a new breakdown with a specific currency.
    pub fn with_currency(currency: Currency) -> Self {
        Self {
            interest_expense_cash: Money::new(0.0, currency),
            interest_income_cash: Some(Money::new(0.0, currency)),
            interest_expense_pik: Money::new(0.0, currency),
            principal_payment: Money::new(0.0, currency),
            fees: Money::new(0.0, currency),
            debt_balance: Money::new(0.0, currency),
            accrued_interest: Money::new(0.0, currency),
        }
    }

    /// Net cash interest received this period, resolving a legacy `None` to a
    /// zero in this breakdown's currency.
    ///
    /// Payloads written before `interest_income_cash` existed carry `None`;
    /// they tracked no income, so zero is the semantically equivalent value.
    #[must_use]
    pub fn interest_income_cash_or_zero(&self) -> Money {
        self.interest_income_cash
            .unwrap_or_else(|| Money::new(0.0, self.interest_expense_cash.currency()))
    }

    /// Cash interest expense net of cash interest received.
    ///
    /// This is the P&L-relevant view: a hedge receipt offsets the coupon it was
    /// bought to offset. The result may be negative when receipts exceed
    /// expense. Use [`Self::interest_expense_cash`] directly when you need the
    /// gross claim the waterfall allocates cash against.
    ///
    /// # Errors
    ///
    /// Returns an error if the expense and income legs are in different
    /// currencies.
    pub fn net_interest_expense_cash(&self) -> crate::error::Result<Money> {
        Ok(self
            .interest_expense_cash
            .checked_sub(self.interest_income_cash_or_zero())?)
    }

    /// Get total interest expense (cash + PIK).
    ///
    /// # Errors
    ///
    /// Returns an error if the cash and PIK interest currencies do not match.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use finstack_quant_statements::capital_structure::CashflowBreakdown;
    /// # use finstack_quant_core::money::Money;
    /// # use finstack_quant_core::currency::Currency;
    /// let cf = CashflowBreakdown {
    ///     interest_expense_cash: Money::new(10_000.0, Currency::USD),
    ///     interest_expense_pik: Money::new(2_500.0, Currency::USD),
    ///     ..CashflowBreakdown::with_currency(Currency::USD)
    /// };
    /// assert_eq!(cf.interest_expense_total().unwrap().amount(), 12_500.0);
    /// ```
    pub fn interest_expense_total(&self) -> crate::Result<Money> {
        self.interest_expense_cash
            .checked_add(self.interest_expense_pik)
            .map_err(|_| {
                crate::error::Error::capital_structure(format!(
                    "Currency mismatch in interest_expense_total: cash={}, pik={}",
                    self.interest_expense_cash.currency(),
                    self.interest_expense_pik.currency(),
                ))
            })
    }

    /// Validate that all `Money` fields share the same currency.
    ///
    /// # Errors
    ///
    /// Returns a capital-structure error naming the first field whose currency
    /// differs from `interest_expense_cash`.
    pub fn validate_currency_invariant(&self) -> crate::Result<()> {
        let expected = self.interest_expense_cash.currency();
        let mut fields = vec![
            ("interest_expense_pik", self.interest_expense_pik.currency()),
            ("principal_payment", self.principal_payment.currency()),
            ("debt_balance", self.debt_balance.currency()),
            ("fees", self.fees.currency()),
            ("accrued_interest", self.accrued_interest.currency()),
        ];
        // Only check the income leg when present: a legacy `None` carries no
        // currency to disagree with (`interest_income_cash_or_zero` derives it
        // from the expense leg, so it can never mismatch).
        if let Some(income) = self.interest_income_cash {
            fields.push(("interest_income_cash", income.currency()));
        }
        for (name, actual) in fields {
            if actual != expected {
                return Err(crate::error::Error::capital_structure(format!(
                    "Currency mismatch in CashflowBreakdown: {name} is {actual}, expected {expected}"
                )));
            }
        }
        Ok(())
    }
}

// NOTE: CashflowBreakdown intentionally does NOT implement Default.
// All construction must go through `with_currency()` to ensure correct
// currency propagation in multi-currency models.

impl CapitalStructureCashflows {
    /// Create empty capital-structure cashflows.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use finstack_quant_statements::capital_structure::CapitalStructureCashflows;
    /// let cashflows = CapitalStructureCashflows::new();
    /// assert!(cashflows.by_instrument.is_empty());
    /// assert!(cashflows.totals.is_empty());
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a single period's cashflows into this accumulator.
    ///
    /// Copies per-instrument breakdowns, totals, and per-currency totals from `period_cs`
    /// into this structure, overwriting any existing entries for the same keys.
    /// Reporting currency is set from the first non-`None` source.
    pub fn set_period(&mut self, period_cs: CapitalStructureCashflows) {
        for (inst_id, period_map) in period_cs.by_instrument {
            let accum_map = self.by_instrument.entry(inst_id).or_default();
            for (pid, breakdown) in period_map {
                accum_map.insert(pid, breakdown);
            }
        }
        for (pid, breakdown) in period_cs.totals {
            self.totals.insert(pid, breakdown);
        }
        for (currency, period_map) in period_cs.totals_by_currency {
            let accum_map = self.totals_by_currency.entry(currency).or_default();
            for (pid, breakdown) in period_map {
                accum_map.insert(pid, breakdown);
            }
        }
        for (pid, equity) in period_cs.equity_distribution {
            self.equity_distribution.insert(pid, equity);
        }
        if self.reporting_currency.is_none() {
            self.reporting_currency = period_cs.reporting_currency;
        }
    }

    /// Generic accessor for instrument-period cashflow data.
    ///
    /// Extracts a value from the `by_instrument` map using the provided extractor function.
    fn get_instrument_field<F>(
        &self,
        instrument_id: &str,
        period_id: &PeriodId,
        field_name: &str,
        extractor: F,
    ) -> Result<f64>
    where
        F: Fn(&CashflowBreakdown) -> f64,
    {
        self.by_instrument
            .get(instrument_id)
            .and_then(|m| m.get(period_id))
            .map(extractor)
            .ok_or_else(|| {
                crate::error::Error::capital_structure(format!(
                    "No {} data for instrument '{}' in period {}",
                    field_name, instrument_id, period_id
                ))
            })
    }

    /// Get total interest expense (cash + PIK) for a specific instrument and period.
    ///
    /// # Arguments
    ///
    /// * `instrument_id` - Identifier supplied when the instrument was added to the model
    /// * `period_id` - Period for which the cashflow should be returned
    ///
    /// # Returns
    ///
    /// Returns the total interest expense in the instrument's native currency as
    /// a scalar amount.
    ///
    /// # Errors
    ///
    /// Returns a capital-structure error when no breakdown exists for the
    /// `(instrument_id, period_id)` pair or when the cash and PIK currencies
    /// differ.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use finstack_quant_statements::capital_structure::{CapitalStructureCashflows, CashflowBreakdown};
    /// # use finstack_quant_core::dates::PeriodId;
    /// # use finstack_quant_core::money::Money;
    /// # use finstack_quant_core::currency::Currency;
    /// let mut cashflows = CapitalStructureCashflows::new();
    /// let period = PeriodId::quarter(2025, 1);
    /// cashflows.by_instrument.insert(
    ///     "BOND-1".into(),
    ///     [(period, CashflowBreakdown { interest_expense_cash: Money::new(5_000.0, Currency::USD), ..CashflowBreakdown::with_currency(Currency::USD) })]
    ///         .into_iter()
    ///         .collect(),
    /// );
    /// assert_eq!(cashflows.get_interest("BOND-1", &period).unwrap(), 5_000.0);
    /// ```
    pub fn get_interest(&self, instrument_id: &str, period_id: &PeriodId) -> Result<f64> {
        let cf = self
            .by_instrument
            .get(instrument_id)
            .and_then(|m| m.get(period_id))
            .ok_or_else(|| {
                crate::error::Error::capital_structure(format!(
                    "No interest data for instrument '{}' in period {}",
                    instrument_id, period_id
                ))
            })?;
        Ok(cf.interest_expense_total()?.amount())
    }

    /// Get cash interest expense for a specific instrument and period.
    ///
    /// # Arguments
    /// * `instrument_id` - Instrument identifier
    /// * `period_id` - Period to inspect
    ///
    /// # Returns
    ///
    /// Cash interest as a scalar in the instrument's native currency.
    ///
    /// # Errors
    ///
    /// Returns a capital-structure error when the requested instrument-period
    /// breakdown is absent.
    pub fn get_interest_cash(&self, instrument_id: &str, period_id: &PeriodId) -> Result<f64> {
        self.get_instrument_field(instrument_id, period_id, "cash interest", |cf| {
            cf.interest_expense_cash.amount()
        })
    }

    /// Get cash interest **received** for a specific instrument and period.
    ///
    /// Non-zero only for two-leg instruments whose legs net to a receipt (e.g.
    /// an in-the-money pay-fixed swap).
    ///
    /// # Arguments
    /// * `instrument_id` - Instrument identifier
    /// * `period_id` - Period to inspect
    pub fn get_interest_income(&self, instrument_id: &str, period_id: &PeriodId) -> Result<f64> {
        self.get_instrument_field(instrument_id, period_id, "interest income", |cf| {
            cf.interest_income_cash_or_zero().amount()
        })
    }

    /// Get PIK interest expense for a specific instrument and period.
    ///
    /// # Arguments
    /// * `instrument_id` - Instrument identifier
    /// * `period_id` - Period to inspect
    ///
    /// # Returns
    ///
    /// PIK interest accrued during the period in the instrument's native currency.
    ///
    /// # Errors
    ///
    /// Returns a capital-structure error when the requested instrument-period
    /// breakdown is absent.
    pub fn get_interest_pik(&self, instrument_id: &str, period_id: &PeriodId) -> Result<f64> {
        self.get_instrument_field(instrument_id, period_id, "PIK interest", |cf| {
            cf.interest_expense_pik.amount()
        })
    }

    /// Get principal payment for a specific instrument and period.
    ///
    /// # Arguments
    /// * `instrument_id` - Instrument identifier
    /// * `period_id` - Period to inspect
    ///
    /// # Returns
    ///
    /// Principal paid during the period in the instrument's native currency.
    ///
    /// # Errors
    ///
    /// Returns a capital-structure error when the requested instrument-period
    /// breakdown is absent.
    pub fn get_principal(&self, instrument_id: &str, period_id: &PeriodId) -> Result<f64> {
        self.get_instrument_field(instrument_id, period_id, "principal", |cf| {
            cf.principal_payment.amount()
        })
    }

    /// Get debt balance for a specific instrument and period.
    ///
    /// # Arguments
    /// * `instrument_id` - Instrument identifier
    /// * `period_id` - Period to inspect
    ///
    /// # Returns
    ///
    /// End-of-period debt balance in the instrument's native currency.
    ///
    /// # Errors
    ///
    /// Returns a capital-structure error when the requested instrument-period
    /// breakdown is absent.
    pub fn get_debt_balance(&self, instrument_id: &str, period_id: &PeriodId) -> Result<f64> {
        self.get_instrument_field(instrument_id, period_id, "debt balance", |cf| {
            cf.debt_balance.amount()
        })
    }

    /// Get fees for a specific instrument and period.
    ///
    /// # Arguments
    /// * `instrument_id` - Instrument identifier
    /// * `period_id` - Period to inspect
    ///
    /// # Returns
    ///
    /// Fees charged during the period in the instrument's native currency.
    ///
    /// # Errors
    ///
    /// Returns a capital-structure error when the requested instrument-period
    /// breakdown is absent.
    pub fn get_fees(&self, instrument_id: &str, period_id: &PeriodId) -> Result<f64> {
        self.get_instrument_field(instrument_id, period_id, "fees", |cf| cf.fees.amount())
    }

    /// Get accrued interest for a specific instrument and period.
    ///
    /// # Arguments
    /// * `instrument_id` - Instrument identifier
    /// * `period_id` - Period to inspect
    ///
    /// # Returns
    ///
    /// Accrued unpaid interest in the instrument's native currency.
    ///
    /// # Errors
    ///
    /// Returns a capital-structure error when the requested instrument-period
    /// breakdown is absent.
    pub fn get_accrued_interest(&self, instrument_id: &str, period_id: &PeriodId) -> Result<f64> {
        self.get_instrument_field(instrument_id, period_id, "accrued interest", |cf| {
            cf.accrued_interest.amount()
        })
    }

    /// Get total interest expense (cash + PIK) across all instruments for a period.
    ///
    /// # Arguments
    ///
    /// * `period_id` - Period to inspect
    ///
    /// # Returns
    ///
    /// Returns total interest in the reporting currency. If reporting totals are
    /// unavailable because multiple currencies are present and no FX conversion
    /// was supplied, this function returns an error.
    ///
    /// # Errors
    ///
    /// Returns a capital-structure error when multi-currency totals lack a
    /// reporting-currency conversion, no total exists for `period_id`, or the
    /// cash and PIK total currencies differ.
    pub fn get_total_interest(&self, period_id: &PeriodId) -> Result<f64> {
        if self.reporting_currency.is_none()
            && self.totals.is_empty()
            && self.totals_by_currency.len() > 1
        {
            return Err(crate::error::Error::capital_structure(
                "Multiple currencies present in capital structure totals and no FX provided. Supply FX in MarketContext or limit to a single currency.",
            ));
        }
        let cf = self.totals.get(period_id).ok_or_else(|| {
            crate::error::Error::capital_structure(format!(
                "No total cashflow data for period {}",
                period_id
            ))
        })?;
        Ok(cf.interest_expense_total()?.amount())
    }

    /// Get total cash interest expense across all instruments for a period.
    ///
    /// # Arguments
    /// * `period_id` - Period to inspect
    ///
    /// # Returns
    ///
    /// Cash interest in the reporting currency.
    ///
    /// # Errors
    ///
    /// Returns a capital-structure error when multi-currency totals lack a
    /// reporting-currency conversion or no total exists for `period_id`.
    pub fn get_total_interest_cash(&self, period_id: &PeriodId) -> Result<f64> {
        self.reporting_total(period_id, |cf| cf.interest_expense_cash.amount())
    }

    /// Get total cash interest **received** across all instruments for a period.
    ///
    /// Non-zero only when the structure holds two-leg instruments whose legs
    /// net to a receipt (e.g. an in-the-money pay-fixed swap).
    ///
    /// # Arguments
    /// * `period_id` - Period to inspect
    pub fn get_total_interest_income(&self, period_id: &PeriodId) -> Result<f64> {
        self.reporting_total(period_id, |cf| cf.interest_income_cash_or_zero().amount())
    }

    /// Get total PIK interest expense across all instruments for a period.
    ///
    /// # Arguments
    /// * `period_id` - Period to inspect
    ///
    /// # Returns
    ///
    /// PIK interest in the reporting currency.
    ///
    /// # Errors
    ///
    /// Returns a capital-structure error when multi-currency totals lack a
    /// reporting-currency conversion or no total exists for `period_id`.
    pub fn get_total_interest_pik(&self, period_id: &PeriodId) -> Result<f64> {
        self.reporting_total(period_id, |cf| cf.interest_expense_pik.amount())
    }

    /// Get total principal payments across all instruments for a period.
    ///
    /// # Arguments
    /// * `period_id` - Period to inspect
    ///
    /// # Returns
    ///
    /// Principal payments in the reporting currency.
    ///
    /// # Errors
    ///
    /// Returns a capital-structure error when multi-currency totals lack a
    /// reporting-currency conversion or no total exists for `period_id`.
    pub fn get_total_principal(&self, period_id: &PeriodId) -> Result<f64> {
        self.reporting_total(period_id, |cf| cf.principal_payment.amount())
    }

    /// Get total debt balance across all instruments for a period.
    ///
    /// # Arguments
    /// * `period_id` - Period to inspect
    ///
    /// # Returns
    ///
    /// End-of-period debt balance in the reporting currency.
    ///
    /// # Errors
    ///
    /// Returns a capital-structure error when multi-currency totals lack a
    /// reporting-currency conversion or no total exists for `period_id`.
    pub fn get_total_debt_balance(&self, period_id: &PeriodId) -> Result<f64> {
        self.reporting_total(period_id, |cf| cf.debt_balance.amount())
    }

    /// Get total fees across all instruments for a period.
    ///
    /// # Arguments
    /// * `period_id` - Period to inspect
    ///
    /// # Returns
    ///
    /// Fees in the reporting currency.
    ///
    /// # Errors
    ///
    /// Returns a capital-structure error when multi-currency totals lack a
    /// reporting-currency conversion or no total exists for `period_id`.
    pub fn get_total_fees(&self, period_id: &PeriodId) -> Result<f64> {
        self.reporting_total(period_id, |cf| cf.fees.amount())
    }

    /// Get total accrued interest across all instruments for a period.
    ///
    /// # Arguments
    /// * `period_id` - Period to inspect
    ///
    /// # Returns
    ///
    /// Accrued interest in the reporting currency.
    ///
    /// # Errors
    ///
    /// Returns a capital-structure error when multi-currency totals lack a
    /// reporting-currency conversion or no total exists for `period_id`.
    pub fn get_total_accrued_interest(&self, period_id: &PeriodId) -> Result<f64> {
        self.reporting_total(period_id, |cf| cf.accrued_interest.amount())
    }

    /// Helper to fetch reporting totals with better error messages.
    fn reporting_total(
        &self,
        period_id: &PeriodId,
        f: impl Fn(&CashflowBreakdown) -> f64,
    ) -> Result<f64> {
        if self.reporting_currency.is_none()
            && self.totals.is_empty()
            && self.totals_by_currency.len() > 1
        {
            return Err(crate::error::Error::capital_structure(
                "Multiple currencies present in capital structure totals and no FX provided. Supply FX in MarketContext or limit to a single currency.",
            ));
        }

        self.totals.get(period_id).map(f).ok_or_else(|| {
            crate::error::Error::capital_structure(format!(
                "No total cashflow data for period {}",
                period_id
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cashflow_breakdown_with_currency() {
        let cf = CashflowBreakdown::with_currency(Currency::USD);
        assert_eq!(cf.interest_expense_cash.amount(), 0.0);
        assert_eq!(cf.interest_expense_pik.amount(), 0.0);
        assert_eq!(
            cf.interest_expense_total().expect("same currency").amount(),
            0.0
        );
        assert_eq!(cf.principal_payment.amount(), 0.0);
        assert_eq!(cf.fees.amount(), 0.0);
        assert_eq!(cf.debt_balance.amount(), 0.0);
        assert_eq!(cf.accrued_interest.amount(), 0.0);
        assert_eq!(cf.interest_expense_cash.currency(), Currency::USD);

        let cf_eur = CashflowBreakdown::with_currency(Currency::EUR);
        assert_eq!(cf_eur.interest_expense_cash.currency(), Currency::EUR);
    }

    #[test]
    fn test_cashflow_breakdown_interest_total() {
        let cf = CashflowBreakdown {
            interest_expense_cash: Money::new(10_000.0, Currency::USD),
            interest_income_cash: Some(Money::new(0.0, Currency::USD)),
            interest_expense_pik: Money::new(2_500.0, Currency::USD),
            ..CashflowBreakdown::with_currency(Currency::USD)
        };
        assert_eq!(
            cf.interest_expense_total().expect("same currency").amount(),
            12_500.0
        );
    }

    /// Payloads written before `interest_income_cash` existed must still
    /// deserialize, and must behave exactly as they did before the field was
    /// added (no income tracked). This is the additive-change contract in
    /// `docs/SERDE_STABILITY.md`: an older payload has to produce a value
    /// semantically equivalent to the pre-change behaviour.
    #[test]
    fn legacy_payload_without_interest_income_deserializes_as_zero_income() {
        // Exactly the field set that existed before the change.
        let legacy = r#"{
            "interest_expense_cash": {"amount": "100.00", "currency": "EUR"},
            "interest_expense_pik": {"amount": "0.00", "currency": "EUR"},
            "principal_payment": {"amount": "0.00", "currency": "EUR"},
            "fees": {"amount": "0.00", "currency": "EUR"},
            "debt_balance": {"amount": "1000.00", "currency": "EUR"},
            "accrued_interest": {"amount": "0.00", "currency": "EUR"}
        }"#;

        let breakdown: CashflowBreakdown =
            serde_json::from_str(legacy).expect("a pre-change payload must still deserialize");

        assert!(breakdown.interest_income_cash.is_none());
        // Resolves to zero in the breakdown's own currency, not a hardcoded one.
        let income = breakdown.interest_income_cash_or_zero();
        assert_eq!(income.amount(), 0.0);
        assert_eq!(income.currency(), Currency::EUR);
        // Net equals gross when no income was tracked — the old behaviour.
        assert_eq!(
            breakdown
                .net_interest_expense_cash()
                .expect("same currency")
                .amount(),
            100.0
        );
        // A legacy `None` carries no currency, so it cannot trip the invariant.
        breakdown
            .validate_currency_invariant()
            .expect("legacy payload must satisfy the currency invariant");
    }

    /// The field is omitted when absent, so a legacy reader is not handed a
    /// key it would reject under `deny_unknown_fields`.
    #[test]
    fn absent_interest_income_is_omitted_from_serialized_output() {
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.interest_income_cash = None;
        let json = serde_json::to_string(&breakdown).expect("serializes");
        assert!(
            !json.contains("interest_income_cash"),
            "an absent income leg must not be emitted: {json}"
        );
    }

    #[test]
    fn validate_currency_invariant_catches_mismatch() {
        let mut cf = CashflowBreakdown::with_currency(Currency::USD);
        cf.interest_expense_pik = Money::new(100.0, Currency::EUR);
        let result = cf.validate_currency_invariant();
        assert!(result.is_err());
        let err_str = result.expect_err("expected mismatch error").to_string();
        assert!(
            err_str.contains("Currency mismatch"),
            "Expected currency mismatch error, got: {err_str}"
        );
    }

    #[test]
    fn validate_currency_invariant_passes_for_valid() {
        let cf = CashflowBreakdown::with_currency(Currency::USD);
        assert!(cf.validate_currency_invariant().is_ok());
    }

    #[test]
    fn test_capital_structure_cashflows_new() {
        let cs_cf = CapitalStructureCashflows::new();
        assert!(cs_cf.by_instrument.is_empty());
        assert!(cs_cf.totals.is_empty());
    }

    #[test]
    fn test_capital_structure_cashflows_accessors() {
        let mut cs_cf = CapitalStructureCashflows::new();

        let period_id = PeriodId::quarter(2025, 1);
        let breakdown = CashflowBreakdown {
            interest_expense_cash: Money::new(45_000.0, Currency::USD),
            interest_income_cash: Some(Money::new(0.0, Currency::USD)),
            interest_expense_pik: Money::new(5_000.0, Currency::USD),
            principal_payment: Money::new(100_000.0, Currency::USD),
            debt_balance: Money::new(1_000_000.0, Currency::USD),
            fees: Money::new(0.0, Currency::USD),
            accrued_interest: Money::new(2_500.0, Currency::USD),
        };

        let mut period_map = IndexMap::new();
        period_map.insert(period_id, breakdown.clone());

        cs_cf
            .by_instrument
            .insert("BOND-001".to_string(), period_map);
        cs_cf.totals.insert(period_id, breakdown);

        assert_eq!(
            cs_cf
                .get_interest("BOND-001", &period_id)
                .expect("interest"),
            50_000.0
        );
        assert_eq!(
            cs_cf
                .get_principal("BOND-001", &period_id)
                .expect("principal"),
            100_000.0
        );
        assert_eq!(
            cs_cf
                .get_debt_balance("BOND-001", &period_id)
                .expect("balance"),
            1_000_000.0
        );
        assert_eq!(
            cs_cf
                .get_accrued_interest("BOND-001", &period_id)
                .expect("accrued"),
            2_500.0
        );

        assert_eq!(
            cs_cf
                .get_total_interest(&period_id)
                .expect("total interest"),
            50_000.0
        );
        assert_eq!(
            cs_cf
                .get_total_principal(&period_id)
                .expect("total principal"),
            100_000.0
        );
        assert_eq!(
            cs_cf
                .get_total_debt_balance(&period_id)
                .expect("total balance"),
            1_000_000.0
        );
        assert_eq!(
            cs_cf
                .get_total_accrued_interest(&period_id)
                .expect("total accrued"),
            2_500.0
        );

        assert!(cs_cf.get_interest("NONEXISTENT", &period_id).is_err());
    }

    #[test]
    fn cashflow_breakdown_serde_round_trips_and_denies_unknown_fields() {
        let cf = CashflowBreakdown {
            interest_expense_cash: Money::new(10_000.0, Currency::USD),
            interest_income_cash: Some(Money::new(0.0, Currency::USD)),
            interest_expense_pik: Money::new(2_500.0, Currency::USD),
            ..CashflowBreakdown::with_currency(Currency::USD)
        };
        let json = serde_json::to_string(&cf).expect("serialize");
        let back: CashflowBreakdown = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            back.interest_expense_cash.amount(),
            cf.interest_expense_cash.amount()
        );
        assert_eq!(
            back.interest_expense_pik.amount(),
            cf.interest_expense_pik.amount()
        );

        let mut value: serde_json::Value = serde_json::from_str(&json).expect("json value");
        value["bogus_field"] = serde_json::json!(1.0);
        let err = serde_json::from_value::<CashflowBreakdown>(value);
        assert!(err.is_err(), "unknown fields must be rejected");
    }

    #[test]
    fn capital_structure_cashflows_serde_round_trips_and_denies_unknown_fields() {
        let mut cs = CapitalStructureCashflows::new();
        let period = PeriodId::quarter(2025, 1);
        cs.totals
            .insert(period, CashflowBreakdown::with_currency(Currency::USD));
        cs.equity_distribution
            .insert(period, Money::new(680.0, Currency::USD));

        let json = serde_json::to_string(&cs).expect("serialize");
        let back: CapitalStructureCashflows = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            back.equity_distribution
                .get(&period)
                .expect("equity round-trips")
                .amount(),
            680.0
        );

        let mut value: serde_json::Value = serde_json::from_str(&json).expect("json value");
        value["bogus_field"] = serde_json::json!(1.0);
        let err = serde_json::from_value::<CapitalStructureCashflows>(value);
        assert!(err.is_err(), "unknown fields must be rejected");
    }

    #[test]
    fn test_multi_currency_without_fx_errors_for_totals() {
        let mut cs = CapitalStructureCashflows::new();
        cs.totals_by_currency.insert(Currency::USD, IndexMap::new());
        cs.totals_by_currency.insert(Currency::EUR, IndexMap::new());

        let period = PeriodId::quarter(2025, 1);
        let err = cs.get_total_interest(&period);
        assert!(err.is_err());
    }
}
