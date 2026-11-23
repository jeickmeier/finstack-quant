//! Schedule merging and outstanding-balance path reconstruction.

use super::types::*;
use crate::builder::Notional;
use finstack_quant_core::cashflow::CashFlow;
use finstack_quant_core::dates::{Date, DayCount, Period};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;

pub(super) fn merge_representation(
    current: Option<CashflowRepresentation>,
    next: CashflowRepresentation,
) -> CashflowRepresentation {
    use CashflowRepresentation::{Contractual, NoResidual, Placeholder, Projected};

    match (current, next) {
        (None, value) => value,
        (Some(Projected), _) | (_, Projected) => Projected,
        (Some(Placeholder), _) | (_, Placeholder) => Placeholder,
        (Some(Contractual), _) | (_, Contractual) => Contractual,
        (Some(NoResidual), NoResidual) => NoResidual,
    }
}

pub(super) fn merge_matching_option<T: Copy + PartialEq>(
    current: &mut Option<Option<T>>,
    value: Option<T>,
) {
    *current = Some(match *current {
        None => value,
        Some(existing) if existing == value => existing,
        Some(_) => None,
    });
}

/// Outstanding principal on `date` from a balance path.
///
/// The balance is the last after-event balance dated on or before `date`, so
/// an event on `date` counts, or `initial` before the first event.
///
/// # Arguments
///
/// * `path` - After-event balances in date order, as returned by
///   [`CashFlowSchedule::outstanding_by_date`].
/// * `date` - Date the balance is read at.
/// * `initial` - Balance before the first event, in the path's currency.
///
/// # Returns
///
/// The raw outstanding amount in the path's currency without clamping.
pub fn outstanding_on(path: &[(Date, Money)], date: Date, initial: f64) -> f64 {
    path.partition_point(|(event_date, _)| *event_date <= date)
        .checked_sub(1)
        .and_then(|last| path.get(last))
        .map_or(initial, |(_, balance)| balance.amount())
}

/// Merge multiple schedules into one deterministic composite schedule.
///
/// Concatenates flows from every input schedule, including mixed-currency
/// rows, deduplicates the union of
/// their `meta.calendar_ids`, and reduces remaining metadata fields with the
/// rules listed below. The combined flow list is then re-sorted by the crate's
/// canonical flow-ordering helper so the resulting schedule is in canonical
/// order.
///
/// # Arguments
///
/// * `schedules` - Iterable of [`CashFlowSchedule`] values to combine.
/// * `notional` - Notional stamped on the merged schedule. The caller is
///   responsible for choosing a representative notional that makes sense for
///   the composite (this function does not aggregate input notionals or require
///   row currencies to match it).
/// * `day_count` - Day count convention attached to the merged schedule.
///
/// # Returns
///
/// A single [`CashFlowSchedule`] containing every input flow, sorted, with
/// merged metadata.
///
/// # Metadata merge rules
///
/// - `representation`: `Projected` dominates mixed inputs, followed by
///   `Placeholder`, `Contractual`, and `NoResidual`.
/// - `calendar_ids`: union of all inputs, sorted and deduplicated.
/// - `commitment`, `issue_date`, `maturity`: kept only if every input agrees,
///   where an input without a value disagrees with one that has it;
///   mismatches → `None`. A merged schedule without `issue_date` fails
///   accrual and `outstanding_by_date`, so give every input the same
///   `issue_date` (for example by copying the instrument's meta).
/// - `projected_fixings`: union of all inputs, normalized.
///
/// Empty input yields an empty schedule with default metadata.
pub fn merge_cashflow_schedules<I>(
    schedules: I,
    notional: Notional,
    day_count: DayCount,
) -> CashFlowSchedule
where
    I: IntoIterator<Item = CashFlowSchedule>,
{
    let mut flows = Vec::new();
    let mut calendar_ids = Vec::new();
    let mut projected_fixings = Vec::new();
    let mut commitment: Option<Option<Money>> = None;
    let mut issue_date: Option<Option<Date>> = None;
    let mut maturity_date: Option<Option<Date>> = None;
    let mut representation: Option<CashflowRepresentation> = None;

    for schedule in schedules {
        representation = Some(merge_representation(
            representation,
            schedule.meta.representation,
        ));
        let schedule_flows = schedule.flows;
        let schedule_meta = schedule.meta;
        flows.extend(schedule_flows);
        calendar_ids.extend(schedule_meta.calendar_ids);
        projected_fixings.extend(schedule_meta.projected_fixings);
        merge_matching_option(&mut commitment, schedule_meta.commitment);
        merge_matching_option(&mut issue_date, schedule_meta.issue_date);
        merge_matching_option(&mut maturity_date, schedule_meta.maturity);
    }

    calendar_ids.sort_unstable();
    calendar_ids.dedup();
    crate::fixings::normalize_projected_fixings(&mut projected_fixings);

    CashFlowSchedule::from_flows(
        flows,
        notional,
        day_count,
        CashFlowMeta {
            projected_fixings,
            representation: representation.unwrap_or_default(),
            calendar_ids,
            commitment: commitment.unwrap_or(None),
            issue_date: issue_date.unwrap_or(None),
            maturity: maturity_date.unwrap_or(None),
        },
    )
}

/// Validate and dust-settle the rebased balance in `initial` currency during
/// [`CashFlowSchedule::normalize_public`] at an economic date boundary or end of
/// the pre-`as_of` prefix.
pub(super) fn settle_rebased_balance(
    balance: &mut Money,
    currency: finstack_quant_core::currency::Currency,
    max_scale: f64,
    date: Date,
    as_of: Date,
) -> finstack_quant_core::Result<()> {
    let tol = (1e-12 * max_scale).max(1e-9);
    if balance.amount() < -tol {
        return Err(finstack_quant_core::Error::Validation(format!(
            "principal repayments exceed rebased outstanding before {as_of} on {date}: {}",
            balance.amount()
        )));
    }
    if balance.amount() < 0.0 {
        *balance = Money::zero(currency);
    }
    Ok(())
}

/// Apply one flow's explicit principal delta during *reconstruction* of the
/// outstanding path from `notional.initial` and the finalized flow list.
/// Only the explicit delta moves the balance; nothing is inferred from cash or
/// kind (record CORE-CONVENTIONS-015 #4).
pub(super) fn apply_flow_to_outstanding(
    outstanding: &mut Money,
    cf: &CashFlow,
) -> finstack_quant_core::Result<()> {
    if let Some(delta) = cf.get_principal_delta()
        && delta.amount() != 0.0
    {
        *outstanding = outstanding.checked_add(delta)?;
    }
    Ok(())
}

impl CashFlowSchedule {
    /// Periodized PVs from a market context.
    ///
    /// Cashflows are grouped into the supplied reporting periods using
    /// half-open period boundaries. Plain discounting uses relative discount
    /// factors from `base`. With `credit_curve_id`, flows are also
    /// survival-weighted and funded principal earns the hazard curve's recovery
    /// assumption. Discount and survival times use each curve's own day count.
    ///
    /// # Arguments
    ///
    /// * `periods` - Reporting periods that define the output buckets.
    /// * `market` - Market context containing the discount (and optional hazard) curves.
    /// * `disc_curve_id` - Discount curve identifier resolved against `market`.
    /// * `credit_curve_id` - Optional hazard curve identifier for credit-adjusted PV.
    /// * `base` - Valuation date: discount factors and survival are relative to
    ///   it, and flows on or before it have zero PV.
    ///
    /// # Returns
    ///
    /// [`crate::aggregation::PeriodAggregation`] mapping `PeriodId` to
    /// per-currency present-value totals. Empty periods are omitted.
    ///
    /// # Errors
    ///
    /// Returns an error if curve lookup fails, a discount factor or survival
    /// probability is invalid, or credit-adjusted inputs are internally
    /// inconsistent.
    pub fn pv_by_period(
        &self,
        periods: &[Period],
        market: &MarketContext,
        disc_curve_id: &CurveId,
        credit_curve_id: Option<&CurveId>,
        base: Date,
    ) -> finstack_quant_core::Result<crate::aggregation::PeriodAggregation> {
        let discount = market.get_discount(disc_curve_id.as_str())?;
        let hazard = credit_curve_id
            .map(|id| market.get_hazard(id.as_str()))
            .transpose()?;
        let hazard = hazard.as_deref();
        crate::aggregation::pv_by_period(
            &self.flows,
            periods,
            discount.as_ref(),
            hazard,
            hazard.map(|curve| curve.recovery_rate()),
            base,
        )
    }

    /// Calendar-year ladder of this schedule's flows against caller-supplied PVs.
    ///
    /// Buckets every flow by the Gregorian year of its payment date and sums
    /// principal-like amounts (`CFKind::is_principal_like`), all other amounts,
    /// and the supplied present values per year. Amounts are taken in native
    /// currency units without FX conversion. Every row must use the same
    /// currency; mixed-currency schedules are rejected before aggregation.
    /// The ladder counts cash only: rows that are not
    /// [`CFKind::is_cash_settlement`](finstack_quant_core::cashflow::CFKind::is_cash_settlement) (PIK capitalization, defaulted-notional
    /// write-down) contribute to no column.
    ///
    /// # Arguments
    ///
    /// * `pvs` - Present value of each flow, one entry per flow in
    ///   [`Self::get_flows`] order and in the same currency units as the flow
    ///   amounts; must be finite.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] when `pvs` does not
    /// have one entry per flow or contains a non-finite value, and
    /// [`finstack_quant_core::Error::CurrencyMismatch`] for mixed-currency rows.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::{CashFlowSchedule, FixedCouponSpec, CouponType, ScheduleParams};
    /// use finstack_quant_core::{currency::Currency, dates::Date, money::Money};
    /// use rust_decimal_macros::dec;
    /// use time::Month;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let schedule = CashFlowSchedule::builder()
    ///     .principal(
    ///         Money::from((1_000_000_i64, Currency::USD)),
    ///         Date::from_calendar_date(2025, Month::January, 15)?,
    ///         Date::from_calendar_date(2026, Month::January, 15)?,
    ///     )
    ///     .fixed_cf(FixedCouponSpec {
    ///         coupon_type: CouponType::Cash,
    ///         rate: dec!(0.05),
    ///         schedule: ScheduleParams::semiannual_30360(),
    ///     })
    ///     .build(None)?;
    /// let pvs: Vec<f64> = schedule.get_flows().iter().map(|f| f.get_amount().amount()).collect();
    /// let ladder = schedule.calendar_year_ladder(&pvs)?;
    /// assert_eq!(ladder.first().map(|row| row.year), Some(2025));
    /// # Ok(())
    /// # }
    /// ```
    pub fn calendar_year_ladder(
        &self,
        pvs: &[f64],
    ) -> finstack_quant_core::Result<Vec<crate::aggregation::CalendarYearLadderRow>> {
        if let Some(first) = self.flows.first() {
            let expected = first.get_amount().currency();
            for flow in &self.flows {
                if flow.get_amount().currency() != expected {
                    return Err(finstack_quant_core::Error::CurrencyMismatch {
                        expected,
                        actual: flow.get_amount().currency(),
                    });
                }
            }
        }
        if self.flows.len() != pvs.len() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "calendar_year_ladder requires equal lengths, got flows={}, pvs={}",
                self.flows.len(),
                pvs.len()
            )));
        }
        crate::aggregation::calendar_year_ladder_rows(self.flows.iter().zip(pvs).map(
            |(flow, &pv)| {
                (
                    flow.get_date(),
                    Ok(flow.get_kind()),
                    flow.get_amount().amount(),
                    pv,
                )
            },
        ))
    }
}
