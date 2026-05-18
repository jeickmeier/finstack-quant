//! Capital Structure Integration Logic
//!
//! Aggregation driver that walks every instrument in a
//! [`crate::types::CapitalStructureSpec`], pulls per-instrument cashflows via
//! [`finstack_cashflows::CashflowProvider`], classifies them by `CFKind` into
//! the [`CashflowBreakdown`] buckets, and rolls them into per-period totals
//! both per-currency and (when FX is available) in the reporting currency.
//!
//! The single-instrument / single-period extraction lives in
//! [`crate::capital_structure::period_flows`]. The JSON-spec → instrument
//! constructor [`build_any_instrument_from_spec`] lives in this module.

use crate::capital_structure::cashflows::{CapitalStructureCashflows, CashflowBreakdown};
use crate::capital_structure::period_flows::period_snapshot_date;
use crate::error::Result;
use finstack_cashflows::primitives::CFKind;
use finstack_cashflows::CashflowProvider;
use finstack_cashflows::{accrued_interest_amount, AccrualConfig};
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, Period, PeriodId};
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::{fx::FxQuery, Money};
use indexmap::IndexMap;
use std::sync::Arc;

// Re-export the moved helper so existing call sites that import via this
// module continue to compile.
pub use crate::capital_structure::period_flows::calculate_period_flows;

/// Build a runtime instrument from a [`crate::types::DebtInstrumentSpec`].
///
/// Delegates entirely to the canonical valuations instrument registry via
/// [`finstack_valuations::instruments::cashflow_provider_from_value`]. The
/// `spec` payload must be the registry's tagged form
/// (`{"type": "...", "spec": {...}}`).
///
/// # Errors
///
/// Returns an error when the payload does not match a registered instrument
/// type or fails spec validation.
pub fn build_any_instrument_from_spec(
    spec: &crate::types::DebtInstrumentSpec,
) -> Result<Arc<dyn CashflowProvider + Send + Sync>> {
    finstack_valuations::instruments::cashflow_provider_from_value(spec.spec.clone()).map_err(
        |e| {
            crate::error::Error::build(format!(
                "Failed to build debt instrument '{}': {e}",
                spec.id
            ))
        },
    )
}

/// Aggregate cashflows from instruments by period using valuations infrastructure.
///
/// The integration leverages `cashflow_schedule()` for CFKind-aware classification and
/// `outstanding_by_date()` for accurate debt balances. Results are normalized into
/// [`CapitalStructureCashflows`] so downstream code (including the DSL via `cs.*`) can
/// consume totals or per-instrument breakdowns.
///
/// # Arguments
/// * `instruments` - Map of instrument IDs to `CashflowProvider` trait objects
/// * `periods` - Ordered list of model periods used for bucketing cashflows
/// * `market_ctx` - Market context containing curves and other pricing data
/// * `as_of` - Valuation date used when generating cashflows
///
/// # Example
///
/// ```rust,no_run
/// use finstack_statements::capital_structure::{aggregate_instrument_cashflows, CapitalStructureCashflows};
/// use finstack_statements::types::CapitalStructureSpec;
/// use finstack_core::dates::build_periods;
/// use finstack_core::market_data::context::MarketContext;
/// use finstack_cashflows::CashflowProvider;
/// use indexmap::IndexMap;
/// use std::sync::Arc;
/// use time::macros::date;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let periods = build_periods("2025Q1..2025Q4", None)?.periods;
/// let instruments: IndexMap<String, Arc<dyn CashflowProvider + Send + Sync>> = IndexMap::new();
/// let spec = CapitalStructureSpec {
///     debt_instruments: vec![],
///     equity_instruments: vec![],
///     meta: IndexMap::new(),
///     reporting_currency: None,
///     fx_policy: None,
///     waterfall: None,
/// };
/// let cashflows: CapitalStructureCashflows =
///     aggregate_instrument_cashflows(&spec, &instruments, &periods, &MarketContext::new(), date!(2025-01-01))?;
/// assert!(cashflows.totals.is_empty());
/// # Ok(())
/// # }
/// ```
pub fn aggregate_instrument_cashflows(
    spec: &crate::types::CapitalStructureSpec,
    instruments: &IndexMap<String, Arc<dyn CashflowProvider + Send + Sync>>,
    periods: &[Period],
    market_ctx: &MarketContext,
    as_of: Date,
) -> Result<CapitalStructureCashflows> {
    let mut result = CapitalStructureCashflows::new();

    // Determine reporting currency: explicit override > FX pivot > set later if single-currency
    let fx_matrix = market_ctx.fx();
    let mut reporting_currency = spec.reporting_currency;
    if reporting_currency.is_none() {
        reporting_currency = fx_matrix.map(|fx| fx.config().pivot_currency);
    }
    // FX policy override (default CashflowDate)
    let fx_policy = spec
        .fx_policy
        .unwrap_or(finstack_core::money::fx::FxConversionPolicy::CashflowDate);

    // Initialize reporting totals if we know the reporting currency up-front
    let mut reporting_totals: Option<IndexMap<PeriodId, CashflowBreakdown>> = reporting_currency
        .map(|rc| {
            let mut map = IndexMap::new();
            for period in periods {
                map.insert(period.id, CashflowBreakdown::with_currency(rc));
            }
            map
        });

    // Always accumulate per-currency totals to avoid panics on mixed-currency portfolios
    let mut totals_by_currency: IndexMap<Currency, IndexMap<PeriodId, CashflowBreakdown>> =
        IndexMap::new();

    // Process each instrument
    for (instrument_id, instrument) in instruments {
        // Use enhanced cashflow_schedule() for precise CFKind classification
        let full_schedule = instrument.cashflow_schedule(market_ctx, as_of)?;

        // Determine currency from first cashflow (all cashflows should be same currency)
        let currency = full_schedule.notional.initial.currency();

        // Initialize period map for this instrument
        let mut instrument_periods: IndexMap<PeriodId, CashflowBreakdown> = IndexMap::new();
        for period in periods {
            instrument_periods.insert(period.id, CashflowBreakdown::with_currency(currency));
        }

        // Pre-initialize per-currency totals for this currency
        totals_by_currency.entry(currency).or_insert_with(|| {
            let mut map = IndexMap::new();
            for period in periods {
                map.insert(period.id, CashflowBreakdown::with_currency(currency));
            }
            map
        });

        // Classify cashflows using precise CFKind information (NO MORE HEURISTICS!)
        // Note: We use full_schedule.flows directly rather than aggregate_by_period because
        // we need access to CFKind metadata for precise classification, which is preserved
        // in the full schedule but lost in simple aggregation.
        for cf in &full_schedule.flows {
            if let Some(period_id) = periods
                .iter()
                .find(|p| cf.date >= p.start && cf.date < p.end)
                .map(|p| p.id)
            {
                if let Some(breakdown) = instrument_periods.get_mut(&period_id) {
                    // Keep as Money, convert to issuer perspective (absolute value)
                    let abs_value = if cf.amount.amount() < 0.0 {
                        finstack_core::money::Money::new(-cf.amount.amount(), cf.amount.currency())
                    } else {
                        cf.amount
                    };

                    let converted_abs = convert_to_reporting(
                        abs_value,
                        cf.date,
                        reporting_currency,
                        fx_matrix,
                        fx_policy,
                    )?;

                    match cf.kind {
                        CFKind::Fixed | CFKind::Stub | CFKind::FloatReset => {
                            // Cash interest payments (coupons, floating resets)
                            breakdown.interest_expense_cash += abs_value;
                            if let (Some(map), Some(money)) =
                                (reporting_totals.as_mut(), converted_abs)
                            {
                                if let Some(total) = map.get_mut(&period_id) {
                                    total.interest_expense_cash += money;
                                }
                            }
                        }
                        CFKind::Amortization => {
                            // Principal amortization payments
                            breakdown.principal_payment += abs_value;
                            if let (Some(map), Some(money)) =
                                (reporting_totals.as_mut(), converted_abs)
                            {
                                if let Some(total) = map.get_mut(&period_id) {
                                    total.principal_payment += money;
                                }
                            }
                        }
                        CFKind::PrePayment | CFKind::RevolvingRepayment => {
                            // Principal repayments (unscheduled prepayments, revolving repayments)
                            breakdown.principal_payment += abs_value;
                            if let (Some(map), Some(money)) =
                                (reporting_totals.as_mut(), converted_abs)
                            {
                                if let Some(total) = map.get_mut(&period_id) {
                                    total.principal_payment += money;
                                }
                            }
                        }
                        CFKind::Notional if cf.amount.amount() > 0.0 => {
                            // Principal redemption (bullet payment)
                            breakdown.principal_payment += abs_value;
                            if let (Some(map), Some(money)) =
                                (reporting_totals.as_mut(), converted_abs)
                            {
                                if let Some(total) = map.get_mut(&period_id) {
                                    total.principal_payment += money;
                                }
                            }
                        }
                        CFKind::Fee
                        | CFKind::CommitmentFee
                        | CFKind::UsageFee
                        | CFKind::FacilityFee => {
                            // Commitment fees, facility fees, etc.
                            breakdown.fees += abs_value;
                            if let (Some(map), Some(money)) =
                                (reporting_totals.as_mut(), converted_abs)
                            {
                                if let Some(total) = map.get_mut(&period_id) {
                                    total.fees += money;
                                }
                            }
                        }
                        CFKind::PIK => {
                            // PIK (payment-in-kind) interest accrued but not paid in cash
                            // This increases the outstanding balance and is tracked separately
                            breakdown.interest_expense_pik += abs_value;
                            if let (Some(map), Some(money)) =
                                (reporting_totals.as_mut(), converted_abs)
                            {
                                if let Some(total) = map.get_mut(&period_id) {
                                    total.interest_expense_pik += money;
                                }
                            }
                        }
                        CFKind::DefaultedNotional | CFKind::Recovery => {
                            // Credit events are not modeled as part of standard debt service in statements.
                            tracing::warn!(
                                "Ignoring credit-event CFKind={:?} in CS aggregation (instrument={}, date={:?})",
                                cf.kind,
                                instrument_id,
                                cf.date
                            );
                        }
                        CFKind::Notional if cf.amount.amount() <= 0.0 => {
                            // Negative notional flows (initial exchange) - typically netted against principal
                            // For simplicity, we ignore these as they represent the initial funding, not ongoing cashflows
                            // The debt_balance is tracked separately via outstanding_by_date()
                        }
                        CFKind::RevolvingDraw => {
                            // Funding / draws are not treated as principal payments in statements.
                            // The debt_balance is tracked separately via outstanding_by_date().
                        }
                        _ => {
                            // CFKind is non-exhaustive; ignore unknown variants to avoid misclassification.
                            tracing::warn!(
                                "Unhandled CFKind={:?} in CS aggregation (instrument={}, date={:?}); ignoring",
                                cf.kind,
                                instrument_id,
                                cf.date
                            );
                        }
                    }
                }
            }
        }

        // Build outstanding path for debt balance lookups.
        // Note: outstanding_path only has entries on dates when cashflows occur,
        // so we need to interpolate for periods without explicit entries.
        let outstanding_path = full_schedule.outstanding_by_date()?;

        // Calculate accrued interest and debt balance for ALL periods, not just
        // periods with cashflow entries. This ensures proper accrual accumulation
        // between coupon payment dates.
        for period in periods {
            let period_id = period.id;
            let snapshot_date = period_snapshot_date(period);

            if let Some(breakdown) = instrument_periods.get_mut(&period_id) {
                // Find the most recent outstanding balance at or before period end.
                // Use rev().find() to efficiently get the latest entry <= snapshot_date.
                let outstanding_at_period = outstanding_path
                    .iter()
                    .rev()
                    .find(|(date, _)| *date <= snapshot_date)
                    .map(|(_, amount)| *amount)
                    .unwrap_or(full_schedule.notional.initial);

                // Keep as Money, use absolute value for issuer perspective
                let issuer_balance = if outstanding_at_period.amount() < 0.0 {
                    finstack_core::money::Money::new(
                        -outstanding_at_period.amount(),
                        outstanding_at_period.currency(),
                    )
                } else {
                    outstanding_at_period
                };
                breakdown.debt_balance = issuer_balance;

                // Accrued interest is measured at the period snapshot date (`end - 1 day`)
                // to align with half-open `[start, end)` attribution.
                let accrued_scalar = accrued_interest_amount(
                    &full_schedule,
                    snapshot_date,
                    &AccrualConfig::default(),
                )?;
                let accrued_money = Money::new(accrued_scalar, currency);
                breakdown.accrued_interest = accrued_money;

                // Convert to reporting currency for totals
                if let (Some(map), Some(money)) = (
                    reporting_totals.as_mut(),
                    convert_to_reporting(
                        issuer_balance,
                        snapshot_date,
                        reporting_currency,
                        fx_matrix,
                        fx_policy,
                    )?,
                ) {
                    if let Some(total) = map.get_mut(&period_id) {
                        total.debt_balance += money;
                    }
                }

                if let (Some(map), Some(money)) = (
                    reporting_totals.as_mut(),
                    convert_to_reporting(
                        accrued_money,
                        snapshot_date,
                        reporting_currency,
                        fx_matrix,
                        fx_policy,
                    )?,
                ) {
                    if let Some(total) = map.get_mut(&period_id) {
                        total.accrued_interest += money;
                    }
                }
            }
        }

        // Store instrument's period breakdown
        result
            .by_instrument
            .insert(instrument_id.clone(), instrument_periods.clone());

        // Aggregate into totals (handling Money addition which returns Result)
        if let Some(currency_totals) = totals_by_currency.get_mut(&currency) {
            for (period_id, breakdown) in &instrument_periods {
                if let Some(total) = currency_totals.get_mut(period_id) {
                    total.interest_expense_cash += breakdown.interest_expense_cash;
                    total.interest_expense_pik += breakdown.interest_expense_pik;
                    total.principal_payment += breakdown.principal_payment;
                    total.debt_balance += breakdown.debt_balance;
                    total.fees += breakdown.fees;
                    total.accrued_interest += breakdown.accrued_interest;
                }
            }
        }
    }

    // Finalize totals and reporting currency selection
    result.totals_by_currency = totals_by_currency;

    if let Some(reporting_totals) = reporting_totals {
        result.reporting_currency = reporting_currency;
        result.totals = reporting_totals;
    } else if result.totals_by_currency.len() == 1 {
        if let Some((ccy, per_period)) = result.totals_by_currency.first() {
            result.reporting_currency = Some(*ccy);
            result.totals = per_period.clone();
        }
    }

    Ok(result)
}

/// Convert a money amount into the reporting currency when FX data is available.
pub(crate) fn convert_to_reporting(
    money: finstack_core::money::Money,
    on: Date,
    reporting_currency: Option<Currency>,
    fx_matrix: Option<&Arc<finstack_core::money::fx::FxMatrix>>,
    fx_policy: finstack_core::money::fx::FxConversionPolicy,
) -> Result<Option<finstack_core::money::Money>> {
    let Some(rc) = reporting_currency else {
        return Ok(None);
    };

    if rc == money.currency() {
        return Ok(Some(money));
    }

    let Some(fx) = fx_matrix else {
        return Err(crate::error::Error::capital_structure(format!(
            "Cannot convert {} to reporting currency {} on {}: no FX matrix present. \
             Supply FX in MarketContext (or remove reporting_currency / keep single-currency portfolios).",
            money.currency(),
            rc,
            on
        )));
    };

    let rate = fx
        .rate(FxQuery::with_policy(money.currency(), rc, on, fx_policy))?
        .rate;
    Ok(Some(finstack_core::money::Money::new(
        money.amount() * rate,
        rc,
    )))
}
