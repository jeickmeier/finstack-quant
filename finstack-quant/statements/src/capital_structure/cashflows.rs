//! Cashflow reporting types for capital structure instruments.
//!
//! This module holds the aggregated cashflow DTOs produced by the evaluator
//! and exposed to the DSL via the `cs.*` namespace.

use crate::error::Result;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::money::Money;
use finstack_quant_core::table::{TableColumn, TableColumnData, TableColumnRole, TableEnvelope};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// One FX conversion applied to consolidate capital-structure totals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FxConversionRecord {
    /// Native currency converted from.
    pub from: Currency,
    /// Reporting currency converted to.
    pub to: Currency,
    /// Units of `to` per unit of `from` (1.20 converts EUR 1 to USD 1.20).
    pub rate: f64,
    /// Inclusive period-close date the rate was read on.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: finstack_quant_core::dates::Date,
    /// How the rate was obtained: `direct` for a quoted pair, `triangulated`
    /// when it was built through the FX matrix's pivot currency.
    pub source: String,
}

/// Aggregated cashflows from capital structure instruments by period.
///
/// Instances of this type are produced by the evaluator and exposed to the DSL
/// via the `cs.*` namespace. It keeps both per-instrument details and totals so
/// that downstream consumers can drill down or report aggregates.
///
/// Monetary fields are stored as [`Money`] to preserve currency identity. The
/// component accessor returns raw `f64` amounts in the instrument's native
/// currency or the reporting currency for totals. Callers that need currency
/// metadata should inspect the underlying maps directly.
///
/// # Example
///
/// ```rust
/// # use finstack_quant_statements::capital_structure::{CapitalStructureCashflows, CashflowBreakdown};
/// # use finstack_quant_core::dates::PeriodId;
/// # use finstack_quant_core::money::Money;
/// # use finstack_quant_core::currency::Currency;
/// let mut cs = CapitalStructureCashflows::new();
/// let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
/// cs.by_instrument
///     .entry("BOND-1".into())
///     .or_default()
///     .insert(period, CashflowBreakdown {
///         interest_expense_cash: Money::from((10_000_i64, Currency::USD)),
///         interest_income_cash: None,
///         interest_expense_pik: Money::from((2_500_i64, Currency::USD)),
///         principal_payment: Money::from((100_000_i64, Currency::USD)),
///         fees: Money::from((0_i64, Currency::USD)),
///         debt_balance: Money::from((4_900_000_i64, Currency::USD)),
///         accrued_interest: Money::from((5_000_i64, Currency::USD)),
///         opening_balance: None,
///         scheduled_principal: None,
///         mandatory_prepayment: None,
///         sweep_prepayment: None,
///         voluntary_prepayment: None,
///         interest_arrears: None,
///         principal_pending: None,
///         principal_arrears: None,
///         principal_advance_reserve: None,
///         fee_rebates: None,
///         repayment_refunds: None,
///         funding_proceeds: None,
///         funding_principal: None,
///         scheduled_cash_interest_due: None,
///         scheduled_principal_due: None,
///     });
/// cs.totals.insert(period, CashflowBreakdown {
///     interest_expense_cash: Money::from((10_000_i64, Currency::USD)),
///     interest_income_cash: None,
///     interest_expense_pik: Money::from((2_500_i64, Currency::USD)),
///     principal_payment: Money::from((100_000_i64, Currency::USD)),
///     fees: Money::from((0_i64, Currency::USD)),
///     debt_balance: Money::from((4_900_000_i64, Currency::USD)),
///     accrued_interest: Money::from((5_000_i64, Currency::USD)),
///     opening_balance: None,
///     scheduled_principal: None,
///     mandatory_prepayment: None,
///     sweep_prepayment: None,
///     voluntary_prepayment: None,
///     interest_arrears: None,
///     principal_pending: None,
///     principal_arrears: None,
///     principal_advance_reserve: None,
///     fee_rebates: None,
///     repayment_refunds: None,
///     funding_proceeds: None,
///     funding_principal: None,
///     scheduled_cash_interest_due: None,
///     scheduled_principal_due: None,
/// });
///
/// assert_eq!(cs.get_component("interest_expense", "total", &period).unwrap(), 12_500.0);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CapitalStructureCashflows {
    /// Map of instrument_id → (period_id → cashflow_type → amount)
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "IndexMap<String, IndexMap<String, CashflowBreakdown>>")
    )]
    pub by_instrument: IndexMap<String, IndexMap<PeriodId, CashflowBreakdown>>,

    /// Total cashflows across all instruments in the reporting currency (if available)
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "IndexMap<String, CashflowBreakdown>")
    )]
    pub totals: IndexMap<PeriodId, CashflowBreakdown>,

    /// Totals bucketed by native instrument currency
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "IndexMap<String, IndexMap<String, CashflowBreakdown>>")
    )]
    pub totals_by_currency: IndexMap<Currency, IndexMap<PeriodId, CashflowBreakdown>>,

    /// Reporting currency used for `totals` (if populated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reporting_currency: Option<Currency>,

    /// Every FX conversion applied to form the consolidated `totals`, by
    /// period: the currency pair, the rate, the date it was read on and how
    /// it was obtained. Empty when every instrument is in the reporting
    /// currency.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "IndexMap<String, Vec<FxConversionRecord>>")
    )]
    pub fx_conversions: IndexMap<PeriodId, Vec<FxConversionRecord>>,

    /// Why a period has no consolidated `totals`, when consolidation was
    /// requested and could not be completed (for example a missing FX quote).
    /// Per-instrument and native-currency values of the period are
    /// unaffected.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    #[cfg_attr(feature = "json-schema", schemars(with = "IndexMap<String, String>"))]
    pub consolidation_unavailable: IndexMap<PeriodId, String>,

    /// Residual cash distributed to equity per period: the cash left after
    /// fees, interest and principal when `Equity` is the waterfall's terminal
    /// priority, and zero when it is not.
    ///
    /// Only populated for periods in which a waterfall ran. With the
    /// per-period allocations,
    /// `fees + cash interest + principal + equity + retained == available cash`
    /// exactly.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    #[cfg_attr(feature = "json-schema", schemars(with = "IndexMap<String, Money>"))]
    pub equity_distribution: IndexMap<PeriodId, Money>,

    /// What each configured prepayment requested, paid and left unpaid in
    /// each period, in the waterfall's priority order.
    ///
    /// Only populated for periods in which a waterfall with a prepayment
    /// priority ran. An unpaid amount is not carried forward: a prepayment
    /// request is not a dated contractual obligation.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "IndexMap<String, Vec<PrepaymentOutcome>>")
    )]
    pub prepayments: IndexMap<PeriodId, Vec<PrepaymentOutcome>>,

    /// Residual cash the company retains per period: the cash left after the
    /// payment stack when `Equity` is not a priority, and zero when it is.
    ///
    /// Only populated for periods in which a waterfall ran.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    #[cfg_attr(feature = "json-schema", schemars(with = "IndexMap<String, Money>"))]
    pub retained_cash: IndexMap<PeriodId, Money>,

    /// Cash the waterfall had to allocate in each period: the value of
    /// `WaterfallSpec.available_cash_node`, floored at zero (a negative pool
    /// is reported as a `NegativeAvailableCashFloored` warning), in the
    /// waterfall's currency.
    ///
    /// Only populated for periods in which a waterfall ran; empty for models
    /// without a waterfall and for JSON written before this field existed.
    /// Reconciles the period's uses:
    /// `fees + cash interest + principal + equity_distribution + retained_cash == available_cash`.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    #[cfg_attr(feature = "json-schema", schemars(with = "IndexMap<String, Money>"))]
    pub available_cash: IndexMap<PeriodId, Money>,

    /// Distress cash by period: recoveries, accrual paid on default and
    /// protection payments, each with its instrument, native currency and
    /// actual payment date, in schedule order.
    ///
    /// These amounts are outside scheduled debt service: they are in no
    /// breakdown, total or waterfall claim, and they change no principal.
    /// Only periods with a distress row have an entry.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "IndexMap<String, Vec<DistressCashflow>>")
    )]
    pub distress_cash: IndexMap<PeriodId, Vec<DistressCashflow>>,
}

/// One distress cash amount of a capital-structure instrument.
///
/// Distress cash is a recovery, an accrual paid on default or a protection
/// payment. It is reported separately from scheduled debt service.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DistressCashflow {
    /// Capital-structure identifier of the instrument.
    pub instrument_id: String,
    /// Actual payment date of the cash.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub payment_date: finstack_quant_core::dates::Date,
    /// The cashflow kind: `recovery`, `accrued_on_default` or
    /// `protection_payment`.
    pub kind: finstack_quant_core::cashflow::CFKind,
    /// Signed cash in the company's perspective and the instrument's native
    /// currency: a receipt is positive and a payment is negative.
    pub amount: Money,
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CashflowBreakdown {
    /// Cash interest payments (coupons, floating resets)
    pub interest_expense_cash: Money,

    /// Net cash interest **received** during the period.
    ///
    /// Includes net hedge receipts and receipts from negative-rate debt coupons.
    /// Stored as a positive amount, like the
    /// outflow-oriented fields, and reported through the `cs.interest_income`
    /// namespace.
    ///
    /// `None` means that no income leg applies. Read it via
    /// [`Self::interest_income_cash_or_zero`] when a currency-preserving zero
    /// is more convenient than an optional value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interest_income_cash: Option<Money>,

    /// PIK (payment-in-kind) interest accrued but not paid in cash
    pub interest_expense_pik: Money,

    /// Principal repayments (amortization, maturity)
    pub principal_payment: Money,

    /// Fees (commitment fees, etc.)
    pub fees: Money,

    /// Outstanding debt balance at period end
    pub debt_balance: Money,

    /// Contractual cash interest earned through the period close and due
    /// after it (liability), from the instrument's own earning terms. Interest
    /// already due and unpaid is `interest_arrears`, not part of this amount.
    /// Hedge-leg accrual valuation is outside the debt-service contract and is zero.
    pub accrued_interest: Money,

    /// Outstanding debt balance at period start, before this period's draws,
    /// repayments and PIK capitalization. Positive amount in the breakdown's
    /// currency. On `totals` it is the sum across instruments.
    ///
    /// Always `Some` on breakdowns produced by the evaluator. `None` on
    /// hand-built breakdowns and on JSON written before this field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opening_balance: Option<Money>,

    /// Part of `principal_payment` paid against the instrument's own
    /// contractual schedule: amortization, maturity redemption,
    /// schedule-embedded prepayments and revolver repayments, including
    /// scheduled principal carried in arrears from an earlier period. Under a
    /// waterfall this is the cash allocated at the `Amortization` priority.
    /// Positive amount in the breakdown's currency.
    ///
    /// The four principal components
    /// (`scheduled_principal`, `mandatory_prepayment`, `sweep_prepayment`,
    /// `voluntary_prepayment`) sum to `principal_payment`. Always `Some` on
    /// breakdowns produced by the evaluator or the waterfall; `None` on
    /// hand-built breakdowns and on JSON written before this field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled_principal: Option<Money>,

    /// Part of `principal_payment` paid at the waterfall's
    /// `MandatoryPrepayment` priority (`WaterfallSpec.mandatory_prepayment`).
    /// Zero when no waterfall ran. Same sign, currency and absence rule as
    /// [`Self::scheduled_principal`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mandatory_prepayment: Option<Money>,

    /// Part of `principal_payment` paid by the excess-cash-flow sweep at the
    /// waterfall's `Sweep` priority. Zero when no waterfall ran. Same sign,
    /// currency and absence rule as [`Self::scheduled_principal`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sweep_prepayment: Option<Money>,

    /// Part of `principal_payment` paid at the waterfall's
    /// `VoluntaryPrepayment` priority (`WaterfallSpec.voluntary_prepayment`).
    /// Zero when no waterfall ran. Same sign, currency and absence rule as
    /// [`Self::scheduled_principal`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voluntary_prepayment: Option<Money>,

    /// Scheduled cash interest that fell due in the period under the
    /// borrowing's contractual schedule, recognized once on its due date
    /// whether or not it was paid. It excludes PIK, fees, arrears settled
    /// from an earlier period and hedge flows. Positive amount in the
    /// breakdown's currency. `Some` on evaluator-produced breakdowns of a
    /// borrowing; `None` for a hedge and on hand-built breakdowns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled_cash_interest_due: Option<Money>,

    /// Scheduled principal that fell due in the period under the borrowing's
    /// contractual schedule (amortization and maturity), recognized once on
    /// its due date whether or not it was paid. It excludes prepayments,
    /// sweeps, arrears settled from an earlier period and advance payments.
    /// Same sign, currency and absence rule as
    /// [`Self::scheduled_cash_interest_due`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled_principal_due: Option<Money>,

    /// Cash interest already due and still unpaid at the period close:
    /// overdue arrears carried as a claim into the next period. It is separate
    /// from `accrued_interest`, which is interest earned but not yet due.
    /// Positive amount in the breakdown's currency. Always `Some` on
    /// breakdowns produced by the evaluator or the waterfall; `None` on
    /// hand-built breakdowns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interest_arrears: Option<Money>,

    /// Scheduled principal cash whose face reduction is already effective
    /// and whose payment falls due after the period close: owed, not yet
    /// due. `debt_balance` is already net of that face. Positive amount in
    /// the breakdown's currency; `Some` on evaluator-produced breakdowns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub principal_pending: Option<Money>,

    /// Scheduled principal cash due and still unpaid at the period close,
    /// carried as a claim into the next period. It is a cash obligation
    /// separate from `debt_balance`, which follows the contract's effective
    /// dates whether or not the cash was paid. Positive amount in the
    /// breakdown's currency; `Some` on evaluator-produced breakdowns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub principal_arrears: Option<Money>,

    /// Face already paid for whose reduction takes effect after the period
    /// close. It is still part of `debt_balance` and is reserved from further
    /// prepayment until then. Positive amount in the breakdown's currency;
    /// `Some` on evaluator-produced breakdowns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub principal_advance_reserve: Option<Money>,

    /// Fee cash the company received in the period: a rebate of a fee it
    /// paid. A receipt, reported apart from `fees` and never a waterfall
    /// claim. Positive amount in the breakdown's currency; `Some` on
    /// evaluator-produced breakdowns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fee_rebates: Option<Money>,

    /// Principal cash the company received back in the period: a refund of a
    /// repayment. A receipt, reported apart from `principal_payment`; the
    /// balance moves only through the row's explicit principal effect.
    /// Positive amount in the breakdown's currency; `Some` on
    /// evaluator-produced breakdowns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repayment_refunds: Option<Money>,

    /// Signed company cash of the funding rows that settled in the period:
    /// positive for proceeds received (net of any discount), negative for
    /// cash paid. It is the cash, not the principal it establishes. `Some`
    /// on evaluator-produced breakdowns of a borrowing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub funding_proceeds: Option<Money>,

    /// Principal established by funding that took effect in the period:
    /// draws and issuance at their explicit principal effect, whatever cash
    /// was received. `Some` on evaluator-produced breakdowns of a borrowing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub funding_principal: Option<Money>,
}

/// What one prepayment priority requested and paid in a period.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PrepaymentOutcome {
    /// The prepayment priority: mandatory prepayment, voluntary prepayment
    /// or sweep.
    pub kind: crate::capital_structure::PaymentPriority,
    /// Amount the priority requested: the mandatory or voluntary amount, or
    /// the sweep percentage of positive excess cash flow.
    pub requested: Money,
    /// Cash paid at par: the request limited by the cash the earlier
    /// priorities left and by the eligible principal net of reserves.
    pub paid: Money,
    /// `requested - paid`.
    pub unpaid: Money,
    /// The part of `unpaid` carried as arrears into the next period: all of
    /// it for a sweep with a dated payment obligation, zero otherwise (an
    /// unpaid amount without one is reported, not carried).
    pub carried: Money,
}

/// Field names of [`CashflowBreakdown::audit_detail`], in the same order.
const AUDIT_DETAIL_LABELS: [&str; 15] = [
    "opening_balance",
    "scheduled_principal",
    "mandatory_prepayment",
    "sweep_prepayment",
    "voluntary_prepayment",
    "interest_arrears",
    "principal_pending",
    "principal_arrears",
    "principal_advance_reserve",
    "scheduled_cash_interest_due",
    "scheduled_principal_due",
    "fee_rebates",
    "repayment_refunds",
    "funding_proceeds",
    "funding_principal",
];

impl CashflowBreakdown {
    /// Every component of the breakdown as a `(flow_type, amount)` pair.
    ///
    /// The labels are the field names (`interest_expense_cash`,
    /// `interest_income_cash`, `interest_expense_pik`, `principal_payment`,
    /// `fees`, `debt_balance`, `accrued_interest`). `interest_income_cash` is
    /// listed only when the instrument reports income, so a pure-debt
    /// breakdown yields six rows and a hedged one seven.
    ///
    /// Breakdowns produced by the evaluator additionally list
    /// `opening_balance` and the four sources of `principal_payment`
    /// (`scheduled_principal`, `mandatory_prepayment`, `sweep_prepayment`,
    /// `voluntary_prepayment`), each only when recorded. Those four rows sum
    /// to the `principal_payment` row, so do not add them to it. The last
    /// rows are `interest_arrears` (cash interest due and unpaid at the
    /// close), the principal settlement amounts `principal_pending`,
    /// `principal_arrears` and `principal_advance_reserve`, the receipts
    /// `fee_rebates` and `repayment_refunds`, and the funding amounts
    /// `funding_proceeds` (signed cash) and `funding_principal`.
    #[must_use]
    pub fn flows(&self) -> Vec<(&'static str, Money)> {
        let mut flows = vec![("interest_expense_cash", self.interest_expense_cash)];
        if let Some(income) = self.interest_income_cash {
            flows.push(("interest_income_cash", income));
        }
        flows.extend([
            ("interest_expense_pik", self.interest_expense_pik),
            ("principal_payment", self.principal_payment),
            ("fees", self.fees),
            ("debt_balance", self.debt_balance),
            ("accrued_interest", self.accrued_interest),
        ]);
        flows.extend(
            AUDIT_DETAIL_LABELS
                .into_iter()
                .zip(self.audit_detail())
                .filter_map(|(label, money)| money.map(|money| (label, money))),
        );
        flows
    }

    /// Create a new breakdown with a specific currency.
    pub fn with_currency(currency: Currency) -> Self {
        Self {
            interest_expense_cash: Money::from((0_i64, currency)),
            interest_income_cash: Some(Money::from((0_i64, currency))),
            interest_expense_pik: Money::from((0_i64, currency)),
            principal_payment: Money::from((0_i64, currency)),
            fees: Money::from((0_i64, currency)),
            debt_balance: Money::from((0_i64, currency)),
            accrued_interest: Money::from((0_i64, currency)),
            opening_balance: None,
            scheduled_principal: None,
            mandatory_prepayment: None,
            sweep_prepayment: None,
            voluntary_prepayment: None,
            interest_arrears: None,
            principal_pending: None,
            principal_arrears: None,
            principal_advance_reserve: None,
            scheduled_cash_interest_due: None,
            scheduled_principal_due: None,
            fee_rebates: None,
            repayment_refunds: None,
            funding_proceeds: None,
            funding_principal: None,
        }
    }

    /// Opening balance, the four principal components, interest arrears, the
    /// three principal settlement amounts, the two contractual due-date
    /// service amounts, the two receipts and the two funding amounts, in the
    /// order of
    /// [`AUDIT_DETAIL_LABELS`].
    pub(crate) fn audit_detail(&self) -> [Option<Money>; 15] {
        [
            self.opening_balance,
            self.scheduled_principal,
            self.mandatory_prepayment,
            self.sweep_prepayment,
            self.voluntary_prepayment,
            self.interest_arrears,
            self.principal_pending,
            self.principal_arrears,
            self.principal_advance_reserve,
            self.scheduled_cash_interest_due,
            self.scheduled_principal_due,
            self.fee_rebates,
            self.repayment_refunds,
            self.funding_proceeds,
            self.funding_principal,
        ]
    }

    /// Mutable view of the fields returned by [`Self::audit_detail`], in the
    /// same order.
    pub(crate) fn audit_detail_mut(&mut self) -> [&mut Option<Money>; 15] {
        [
            &mut self.opening_balance,
            &mut self.scheduled_principal,
            &mut self.mandatory_prepayment,
            &mut self.sweep_prepayment,
            &mut self.voluntary_prepayment,
            &mut self.interest_arrears,
            &mut self.principal_pending,
            &mut self.principal_arrears,
            &mut self.principal_advance_reserve,
            &mut self.scheduled_cash_interest_due,
            &mut self.scheduled_principal_due,
            &mut self.fee_rebates,
            &mut self.repayment_refunds,
            &mut self.funding_proceeds,
            &mut self.funding_principal,
        ]
    }

    /// Record that the contractual schedule is the only source of
    /// `principal_payment`: all of it is scheduled and the three prepayment
    /// components are zero.
    pub(crate) fn mark_principal_as_scheduled(&mut self) {
        let zero = Money::from((0_i64, self.principal_payment.currency()));
        self.scheduled_principal = Some(self.principal_payment);
        self.mandatory_prepayment = Some(zero);
        self.sweep_prepayment = Some(zero);
        self.voluntary_prepayment = Some(zero);
        // No cash cap has applied: nothing due is unpaid.
        self.interest_arrears = Some(zero);
        self.principal_arrears = Some(zero);
    }

    /// Net cash interest received this period, resolving `None` to a zero in
    /// this breakdown's currency.
    #[must_use]
    pub fn interest_income_cash_or_zero(&self) -> Money {
        self.interest_income_cash
            .unwrap_or_else(|| Money::from((0_i64, self.interest_expense_cash.currency())))
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
    ///     interest_expense_cash: Money::from((10_000_i64, Currency::USD)),
    ///     interest_expense_pik: Money::from((2_500_i64, Currency::USD)),
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
        // Only check the income leg when present: `None` carries no currency
        // to disagree with (`interest_income_cash_or_zero` derives it
        // from the expense leg, so it can never mismatch).
        if let Some(income) = self.interest_income_cash {
            fields.push(("interest_income_cash", income.currency()));
        }
        for (name, money) in AUDIT_DETAIL_LABELS.into_iter().zip(self.audit_detail()) {
            if let Some(money) = money {
                fields.push((name, money.currency()));
            }
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

/// Add `part` into `total`, treating an unrecorded (`None`) total as zero. An
/// unrecorded `part` leaves `total` untouched.
pub(crate) fn add_optional_money(total: &mut Option<Money>, part: Option<Money>) -> Result<()> {
    if let Some(part) = part {
        *total = Some(match *total {
            Some(current) => current.checked_add(part)?,
            None => part,
        });
    }
    Ok(())
}

// NOTE: CashflowBreakdown intentionally does NOT implement Default.
// All construction must go through `with_currency()` to ensure correct
// currency propagation in multi-currency models.

/// Instrument label carried by the rows of
/// [`CapitalStructureCashflows::to_totals_table`].
pub const TOTAL_ROW_LABEL: &str = "__total__";

/// Build the long `(instrument, period, flow_type, amount, currency)` table.
fn flows_table<'a>(
    rows: impl Iterator<Item = (&'a str, &'a PeriodId, &'a CashflowBreakdown)>,
    layout: &str,
) -> Result<TableEnvelope> {
    let mut instruments = Vec::new();
    let mut periods = Vec::new();
    let mut flow_types = Vec::new();
    let mut values = Vec::new();
    let mut amounts = Vec::new();
    let mut currencies = Vec::new();
    for (instrument, period, breakdown) in rows {
        for (flow_type, money) in breakdown.flows() {
            instruments.push(instrument.to_string());
            periods.push(period.to_string());
            flow_types.push(flow_type.to_string());
            values.push(money.amount());
            amounts.push(money.amount_decimal().to_string());
            currencies.push(money.currency().to_string());
        }
    }
    let mut metadata = IndexMap::new();
    metadata.insert("layout".to_string(), serde_json::json!(layout));
    metadata.insert(
        "source".to_string(),
        serde_json::json!("capital_structure_cashflows"),
    );
    TableEnvelope::new(
        vec![
            TableColumn::new("instrument", TableColumnData::String(instruments))
                .with_role(TableColumnRole::Dimension),
            TableColumn::new("period", TableColumnData::String(periods))
                .with_role(TableColumnRole::Index),
            TableColumn::new("flow_type", TableColumnData::String(flow_types))
                .with_role(TableColumnRole::Dimension),
            TableColumn::new("value", TableColumnData::Float64(values))
                .with_role(TableColumnRole::Measure),
            TableColumn::new("amount", TableColumnData::String(amounts))
                .with_role(TableColumnRole::Attribute),
            TableColumn::new("currency", TableColumnData::String(currencies))
                .with_role(TableColumnRole::Attribute),
        ],
        metadata,
    )
    .map_err(Into::into)
}

impl CapitalStructureCashflows {
    /// Periods covered by these cashflows, in ascending order: every period
    /// any instrument has a breakdown for, whether or not that period could
    /// be consolidated.
    #[must_use]
    pub fn periods(&self) -> Vec<PeriodId> {
        let mut periods: Vec<PeriodId> = self
            .by_instrument
            .values()
            .chain(self.totals_by_currency.values())
            .flat_map(|by_period| by_period.keys().copied())
            .chain(self.totals.keys().copied())
            .collect();
        periods.sort();
        periods.dedup();
        periods
    }

    /// Export per-instrument cashflows as a long table.
    ///
    /// Columns: `instrument`, `period` (period identifier such as `2025Q1`),
    /// `flow_type` (see [`CashflowBreakdown::flows`]), `value` (the amount's
    /// float view), `amount` (the exact amount as decimal text) and
    /// `currency` (ISO 4217 code). One row per instrument, period and flow
    /// type; amounts are in each instrument's native currency.
    ///
    /// # Errors
    ///
    /// Returns an error only if the table columns fail envelope validation.
    pub fn to_table(&self) -> Result<TableEnvelope> {
        flows_table(
            self.by_instrument
                .iter()
                .flat_map(|(instrument, by_period)| {
                    by_period
                        .iter()
                        .map(move |(period, breakdown)| (instrument.as_str(), period, breakdown))
                }),
            "by_instrument",
        )
    }

    /// Export the reporting-currency totals as a long table.
    ///
    /// Same columns as [`to_table`](Self::to_table); every row carries
    /// [`TOTAL_ROW_LABEL`] in `instrument`. Empty when no reporting-currency
    /// total could be formed (see `totals`).
    ///
    /// # Errors
    ///
    /// Returns an error only if the table columns fail envelope validation.
    pub fn to_totals_table(&self) -> Result<TableEnvelope> {
        flows_table(
            self.totals
                .iter()
                .map(|(period, breakdown)| (TOTAL_ROW_LABEL, period, breakdown)),
            "totals",
        )
    }

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

    /// Build the compact cashflow snapshot used to evaluate one model period.
    pub(crate) fn period_snapshot(&self, period_id: PeriodId) -> Self {
        let mut snapshot = Self::new();
        for (instrument_id, period_map) in &self.by_instrument {
            if let Some(breakdown) = period_map.get(&period_id) {
                snapshot
                    .by_instrument
                    .entry(instrument_id.clone())
                    .or_default()
                    .insert(period_id, breakdown.clone());
            }
        }
        if let Some(breakdown) = self.totals.get(&period_id) {
            snapshot.totals.insert(period_id, breakdown.clone());
        }
        for (currency, period_map) in &self.totals_by_currency {
            if let Some(breakdown) = period_map.get(&period_id) {
                snapshot
                    .totals_by_currency
                    .entry(*currency)
                    .or_default()
                    .insert(period_id, breakdown.clone());
            }
        }
        if let Some(equity) = self.equity_distribution.get(&period_id) {
            snapshot.equity_distribution.insert(period_id, *equity);
        }
        if let Some(retained) = self.retained_cash.get(&period_id) {
            snapshot.retained_cash.insert(period_id, *retained);
        }
        if let Some(prepayments) = self.prepayments.get(&period_id) {
            snapshot.prepayments.insert(period_id, prepayments.clone());
        }
        if let Some(cash) = self.available_cash.get(&period_id) {
            snapshot.available_cash.insert(period_id, *cash);
        }
        if let Some(rows) = self.distress_cash.get(&period_id) {
            snapshot.distress_cash.insert(period_id, rows.clone());
        }
        if let Some(rows) = self.fx_conversions.get(&period_id) {
            snapshot.fx_conversions.insert(period_id, rows.clone());
        }
        if let Some(reason) = self.consolidation_unavailable.get(&period_id) {
            snapshot
                .consolidation_unavailable
                .insert(period_id, reason.clone());
        }
        snapshot.reporting_currency = self.reporting_currency;
        snapshot
    }

    /// Replace every result of the periods `period_cs` covers.
    ///
    /// Replacement is atomic per period: each period present in `period_cs`
    /// first loses all of its rows here (per-instrument, native-currency and
    /// consolidated values and their provenance), so a row the replacement
    /// omits does not survive as stale data, and then receives the new rows.
    /// Nothing changes when the replacement is rejected.
    ///
    /// # Arguments
    ///
    /// * `period_cs` - The complete results of one or more periods, as
    ///   produced by a period evaluation.
    ///
    /// # Errors
    ///
    /// Returns a capital-structure error when `period_cs` reports in a
    /// different currency than the results already held: the reporting
    /// currency is fixed for a run.
    pub fn set_period(&mut self, period_cs: CapitalStructureCashflows) -> Result<()> {
        if let (Some(held), Some(new)) = (self.reporting_currency, period_cs.reporting_currency)
            && held != new
        {
            return Err(crate::error::Error::capital_structure(format!(
                "capital-structure results report in {held}; a period reporting in {new} \
                 cannot replace them. The reporting currency is fixed for a run."
            )));
        }
        let mut replaced: Vec<PeriodId> = period_cs
            .by_instrument
            .values()
            .flat_map(|periods| periods.keys().copied())
            .chain(period_cs.totals.keys().copied())
            .chain(
                period_cs
                    .totals_by_currency
                    .values()
                    .flat_map(|periods| periods.keys().copied()),
            )
            .chain(period_cs.equity_distribution.keys().copied())
            .chain(period_cs.retained_cash.keys().copied())
            .chain(period_cs.prepayments.keys().copied())
            .chain(period_cs.available_cash.keys().copied())
            .chain(period_cs.distress_cash.keys().copied())
            .chain(period_cs.fx_conversions.keys().copied())
            .chain(period_cs.consolidation_unavailable.keys().copied())
            .collect();
        replaced.sort();
        replaced.dedup();
        for period in &replaced {
            for periods in self.by_instrument.values_mut() {
                periods.shift_remove(period);
            }
            for periods in self.totals_by_currency.values_mut() {
                periods.shift_remove(period);
            }
            self.totals.shift_remove(period);
            self.equity_distribution.shift_remove(period);
            self.retained_cash.shift_remove(period);
            self.prepayments.shift_remove(period);
            self.available_cash.shift_remove(period);
            self.distress_cash.shift_remove(period);
            self.fx_conversions.shift_remove(period);
            self.consolidation_unavailable.shift_remove(period);
        }
        self.by_instrument.retain(|_, periods| !periods.is_empty());
        self.totals_by_currency
            .retain(|_, periods| !periods.is_empty());

        for (inst_id, period_map) in period_cs.by_instrument {
            self.by_instrument
                .entry(inst_id)
                .or_default()
                .extend(period_map);
        }
        self.totals.extend(period_cs.totals);
        for (currency, period_map) in period_cs.totals_by_currency {
            self.totals_by_currency
                .entry(currency)
                .or_default()
                .extend(period_map);
        }
        self.equity_distribution
            .extend(period_cs.equity_distribution);
        self.retained_cash.extend(period_cs.retained_cash);
        self.prepayments.extend(period_cs.prepayments);
        self.available_cash.extend(period_cs.available_cash);
        self.distress_cash.extend(period_cs.distress_cash);
        self.fx_conversions.extend(period_cs.fx_conversions);
        self.consolidation_unavailable
            .extend(period_cs.consolidation_unavailable);
        if self.reporting_currency.is_none() {
            self.reporting_currency = period_cs.reporting_currency;
        }
        // Accessors and exports read periods chronologically.
        for periods in self.by_instrument.values_mut() {
            periods.sort_keys();
        }
        for periods in self.totals_by_currency.values_mut() {
            periods.sort_keys();
        }
        self.totals.sort_keys();
        self.equity_distribution.sort_keys();
        self.retained_cash.sort_keys();
        self.prepayments.sort_keys();
        self.available_cash.sort_keys();
        self.distress_cash.sort_keys();
        self.fx_conversions.sort_keys();
        self.consolidation_unavailable.sort_keys();
        Ok(())
    }

    /// Read one reported amount for one scope and period, exactly.
    ///
    /// This is the lookup behind `cs.<component>.<scope>` in statement
    /// formulas. It reads values already computed for the scope and performs
    /// no FX conversion.
    ///
    /// # Arguments
    ///
    /// * `component` - The reported amount to read.
    /// * `scope` - One instrument (its native currency), one native
    ///   currency's total, or the consolidated total in the reporting
    ///   currency.
    /// * `period_id` - Reporting period whose amount is returned.
    ///
    /// # Errors
    ///
    /// Returns a capital-structure error when the scope has no data for the
    /// period (an unknown instrument or currency, or a consolidated total
    /// that could not be formed, with the reason), or when the breakdown does
    /// not record the component.
    pub fn get_money(
        &self,
        component: crate::capital_structure::CsComponent,
        scope: &crate::capital_structure::CsScope,
        period_id: &PeriodId,
    ) -> Result<Money> {
        use crate::capital_structure::CsScope;
        let flow = match scope {
            CsScope::Consolidated => self.totals.get(period_id).ok_or_else(|| {
                crate::error::Error::capital_structure(
                    match self.consolidation_unavailable.get(period_id) {
                        Some(reason) => format!(
                            "The consolidated capital-structure total is unavailable for period \
                             {period_id}: {reason}"
                        ),
                        None => format!("No total cashflow data for period {period_id}"),
                    },
                )
            })?,
            CsScope::NativeTotal(currency) => self
                .totals_by_currency
                .get(currency)
                .and_then(|periods| periods.get(period_id))
                .ok_or_else(|| {
                    crate::error::Error::capital_structure(format!(
                        "No {currency} capital-structure total for period {period_id}"
                    ))
                })?,
            CsScope::Instrument(instrument) => self
                .by_instrument
                .get(instrument)
                .and_then(|periods| periods.get(period_id))
                .ok_or_else(|| {
                    crate::error::Error::capital_structure(format!(
                        "No {component} data for instrument '{instrument}' in period {period_id}"
                    ))
                })?,
        };
        component.read(flow)
    }

    /// Read one reported amount as a number, by selector text.
    ///
    /// The text form of [`get_money`](Self::get_money): `component` is a
    /// component name and `scope` is an instrument id, `total` (consolidated)
    /// or `total.<CCY>` (one native currency's total).
    ///
    /// # Arguments
    ///
    /// * `component` - Component name, for example `interest_expense`,
    ///   `principal_payment`, `scheduled_principal`, `debt_balance` or
    ///   `principal_arrears`; see
    ///   [`CsComponent`](crate::capital_structure::CsComponent).
    /// * `scope` - Instrument identifier for its native-currency value,
    ///   `total` for the consolidated reporting-currency total, or
    ///   `total.<CCY>` for one native currency's total.
    /// * `period_id` - Reporting period whose amount is returned, in major
    ///   currency units of the scope's currency.
    ///
    /// # Errors
    ///
    /// Returns a capital-structure error for an unknown component or a
    /// malformed scope, and the errors of [`get_money`](Self::get_money).
    pub fn get_component(&self, component: &str, scope: &str, period_id: &PeriodId) -> Result<f64> {
        Ok(self
            .get_money(component.parse()?, &scope.parse()?, period_id)?
            .amount())
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
            interest_expense_cash: Money::from((10_000_i64, Currency::USD)),
            interest_income_cash: Some(Money::from((0_i64, Currency::USD))),
            interest_expense_pik: Money::from((2_500_i64, Currency::USD)),
            ..CashflowBreakdown::with_currency(Currency::USD)
        };
        assert_eq!(
            cf.interest_expense_total().expect("same currency").amount(),
            12_500.0
        );
    }

    #[test]
    fn absent_interest_income_resolves_to_zero_in_expense_currency() {
        let canonical = r#"{
            "interest_expense_cash": {"amount": "100.00", "currency": "EUR"},
            "interest_expense_pik": {"amount": "0.00", "currency": "EUR"},
            "principal_payment": {"amount": "0.00", "currency": "EUR"},
            "fees": {"amount": "0.00", "currency": "EUR"},
            "debt_balance": {"amount": "1000.00", "currency": "EUR"},
            "accrued_interest": {"amount": "0.00", "currency": "EUR"}
        }"#;

        let breakdown: CashflowBreakdown =
            serde_json::from_str(canonical).expect("optional income leg may be absent");

        assert!(breakdown.interest_income_cash.is_none());
        // Resolves to zero in the breakdown's own currency, not a hardcoded one.
        let income = breakdown.interest_income_cash_or_zero();
        assert_eq!(income.amount(), 0.0);
        assert_eq!(income.currency(), Currency::EUR);
        // Net equals gross when no income applies.
        assert_eq!(
            breakdown
                .net_interest_expense_cash()
                .expect("same currency")
                .amount(),
            100.0
        );
        breakdown
            .validate_currency_invariant()
            .expect("an absent optional income leg satisfies the currency invariant");
    }

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
        cf.interest_expense_pik = Money::from((100_i64, Currency::EUR));
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

        let period_id = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let breakdown = CashflowBreakdown {
            interest_expense_cash: Money::from((45_000_i64, Currency::USD)),
            interest_income_cash: Some(Money::from((0_i64, Currency::USD))),
            interest_expense_pik: Money::from((5_000_i64, Currency::USD)),
            principal_payment: Money::from((100_000_i64, Currency::USD)),
            debt_balance: Money::from((1_000_000_i64, Currency::USD)),
            fees: Money::from((0_i64, Currency::USD)),
            accrued_interest: Money::from((2_500_i64, Currency::USD)),
            opening_balance: None,
            scheduled_principal: None,
            mandatory_prepayment: None,
            sweep_prepayment: None,
            voluntary_prepayment: None,
            interest_arrears: None,
            principal_pending: None,
            principal_arrears: None,
            principal_advance_reserve: None,
            scheduled_cash_interest_due: None,
            scheduled_principal_due: None,
            fee_rebates: None,
            repayment_refunds: None,
            funding_proceeds: None,
            funding_principal: None,
        };

        let mut period_map = IndexMap::new();
        period_map.insert(period_id, breakdown.clone());

        cs_cf
            .by_instrument
            .insert("BOND-001".to_string(), period_map);
        cs_cf.totals.insert(period_id, breakdown);

        for scope in ["BOND-001", "total"] {
            for (component, expected) in [
                ("interest_expense", 50_000.0),
                ("interest_expense_cash", 45_000.0),
                ("interest_expense_pik", 5_000.0),
                ("interest_income", 0.0),
                ("principal_payment", 100_000.0),
                ("debt_balance", 1_000_000.0),
                ("fees", 0.0),
                ("accrued_interest", 2_500.0),
            ] {
                assert_eq!(
                    cs_cf
                        .get_component(component, scope, &period_id)
                        .expect("component"),
                    expected
                );
            }
        }
        assert!(
            cs_cf
                .get_component("unknown", "BOND-001", &period_id)
                .is_err()
        );
        let missing = PeriodId::quarter(2025, 2).expect("period");
        assert!(cs_cf.get_component("fees", "total", &missing).is_err());
        let mut incompatible = CashflowBreakdown::with_currency(Currency::USD);
        incompatible.interest_expense_pik = Money::from((1_i64, Currency::EUR));
        cs_cf
            .by_instrument
            .entry("BAD".into())
            .or_default()
            .insert(period_id, incompatible);
        assert!(
            cs_cf
                .get_component("interest_expense", "BAD", &period_id)
                .is_err()
        );

        assert_eq!(
            cs_cf
                .get_component("interest_expense", "BOND-001", &period_id)
                .expect("interest"),
            50_000.0
        );
        assert_eq!(
            cs_cf
                .get_component("principal_payment", "BOND-001", &period_id)
                .expect("principal"),
            100_000.0
        );
        assert_eq!(
            cs_cf
                .get_component("debt_balance", "BOND-001", &period_id)
                .expect("balance"),
            1_000_000.0
        );
        assert_eq!(
            cs_cf
                .get_component("accrued_interest", "BOND-001", &period_id)
                .expect("accrued"),
            2_500.0
        );

        assert_eq!(
            cs_cf
                .get_component("interest_expense", "total", &period_id)
                .expect("total interest"),
            50_000.0
        );
        assert_eq!(
            cs_cf
                .get_component("principal_payment", "total", &period_id)
                .expect("total principal"),
            100_000.0
        );
        assert_eq!(
            cs_cf
                .get_component("debt_balance", "total", &period_id)
                .expect("total balance"),
            1_000_000.0
        );
        assert_eq!(
            cs_cf
                .get_component("accrued_interest", "total", &period_id)
                .expect("total accrued"),
            2_500.0
        );

        assert!(
            cs_cf
                .get_component("interest_expense", "NONEXISTENT", &period_id)
                .is_err()
        );
    }

    #[test]
    fn cashflow_breakdown_serde_round_trips_and_denies_unknown_fields() {
        let cf = CashflowBreakdown {
            interest_expense_cash: Money::from((10_000_i64, Currency::USD)),
            interest_income_cash: Some(Money::from((0_i64, Currency::USD))),
            interest_expense_pik: Money::from((2_500_i64, Currency::USD)),
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
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        cs.totals
            .insert(period, CashflowBreakdown::with_currency(Currency::USD));
        cs.equity_distribution
            .insert(period, Money::from((680_i64, Currency::USD)));

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

        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let err = cs.get_component("interest_expense", "total", &period);
        assert!(err.is_err());
    }

    fn period_results(
        period: PeriodId,
        instruments: &[&str],
        reporting: Option<Currency>,
    ) -> CapitalStructureCashflows {
        let mut results = CapitalStructureCashflows::new();
        for id in instruments {
            results
                .by_instrument
                .entry((*id).to_string())
                .or_default()
                .insert(period, CashflowBreakdown::with_currency(Currency::USD));
        }
        results
            .totals
            .insert(period, CashflowBreakdown::with_currency(Currency::USD));
        results
            .equity_distribution
            .insert(period, Money::from((5_i64, Currency::USD)));
        results.reporting_currency = reporting;
        results
    }

    /// A period is replaced as a whole: a row the replacement omits does not
    /// survive, a conflicting reporting currency is rejected before anything
    /// changes, and periods read back chronologically.
    #[test]
    fn a_period_is_replaced_atomically_and_read_chronologically() {
        let q1 = PeriodId::quarter(2025, 1).expect("quarter");
        let q2 = PeriodId::quarter(2025, 2).expect("quarter");
        let mut held = CapitalStructureCashflows::new();
        // The later period arrives first.
        held.set_period(period_results(q2, &["A", "B"], Some(Currency::USD)))
            .expect("second quarter");
        held.set_period(period_results(q1, &["A", "B"], Some(Currency::USD)))
            .expect("first quarter");
        assert_eq!(held.periods(), vec![q1, q2]);
        assert_eq!(
            held.by_instrument["A"].keys().copied().collect::<Vec<_>>(),
            vec![q1, q2]
        );
        assert_eq!(
            held.equity_distribution.keys().copied().collect::<Vec<_>>(),
            vec![q1, q2]
        );

        // The replacement for the first quarter no longer has instrument B
        // or an equity distribution.
        let mut replacement = period_results(q1, &["A"], Some(Currency::USD));
        replacement.equity_distribution.clear();
        held.set_period(replacement).expect("replacement");
        assert!(!held.by_instrument["B"].contains_key(&q1));
        assert!(held.by_instrument["B"].contains_key(&q2));
        assert!(!held.equity_distribution.contains_key(&q1));
        assert!(held.equity_distribution.contains_key(&q2));

        let before = serde_json::to_value(&held).expect("serialize");
        let error = held
            .set_period(period_results(q1, &["A", "B"], Some(Currency::EUR)))
            .expect_err("conflicting reporting currency")
            .to_string();
        assert!(error.contains("USD") && error.contains("EUR"), "{error}");
        assert_eq!(serde_json::to_value(&held).expect("serialize"), before);
    }

    /// Selector text reads the same values as the typed selectors, and
    /// monetary amounts export as exact decimal strings with their currency.
    #[test]
    fn selector_text_reads_each_scope_and_money_exports_exactly() {
        use crate::capital_structure::{CsComponent, CsScope};
        let period = PeriodId::quarter(2025, 1).expect("quarter");
        let mut results = period_results(period, &["TL"], Some(Currency::USD));
        let amount = Money::new(1234.56, Currency::USD).expect("amount");
        results
            .by_instrument
            .get_mut("TL")
            .and_then(|periods| periods.get_mut(&period))
            .expect("row")
            .debt_balance = amount;
        results
            .totals_by_currency
            .entry(Currency::USD)
            .or_default()
            .insert(period, results.by_instrument["TL"][&period].clone());
        assert_eq!(
            results
                .get_money(
                    CsComponent::DebtBalance,
                    &CsScope::Instrument("TL".into()),
                    &period
                )
                .expect("instrument"),
            amount
        );
        for scope in ["TL", "total.USD"] {
            assert_eq!(
                results
                    .get_component("debt_balance", scope, &period)
                    .expect("text selector"),
                1234.56
            );
        }
        for (component, scope) in [("coupon", "TL"), ("debt_balance", "total.dollars")] {
            assert!(results.get_component(component, scope, &period).is_err());
        }
        assert!(
            results
                .get_component("debt_balance", "GHOST", &period)
                .is_err()
        );
        let wire = serde_json::to_value(&results).expect("serialize");
        assert_eq!(
            wire["by_instrument"]["TL"]["2025Q1"]["debt_balance"],
            serde_json::json!({"amount": "1234.56", "currency": "USD"})
        );
    }
}
