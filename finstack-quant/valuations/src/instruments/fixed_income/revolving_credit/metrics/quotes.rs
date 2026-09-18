//! Quote metrics for revolving credit facilities: discount margin, yield to
//! maturity, price from a quoted discount margin, accrued interest and the
//! running all-in rate.
//!
//! All five run on the facility's contractual schedule (the expected,
//! path-averaged schedule for a stochastic facility) anchored at the
//! settlement date `facility.settlement_date(as_of)`. Quotes follow LSTA
//! practice: a clean price applies to the drawn balance at settlement plus
//! accrued cash interest; the undrawn commitment and letters of credit are
//! not in the quote, though their fees are in the flows being repriced.
//! Without a quote the target is the model value carried from the valuation
//! date to settlement on the discount curve, so with the default
//! `settlement_days = 0` the model discount margin of a par facility on a
//! flat consistent curve is exactly its contractual margin.

use std::sync::Arc;

use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::Result;
use rust_decimal::prelude::ToPrimitive;

use crate::cashflow::builder::CashFlowSchedule;
use crate::cashflow::primitives::is_cash_settlement_kind;
use crate::cashflow::traits::CashflowScheduleSource;
use crate::instruments::fixed_income::loan_quotes::{
    all_in_rate_from_schedule, compounding_frequency, pv_with_discount_margin,
    solve_discount_margin,
};
use crate::instruments::fixed_income::revolving_credit::types::BaseRateSpec;
use crate::instruments::RevolvingCredit;
use crate::metrics::{MetricCalculator, MetricContext};

/// The facility's settlement schedule, built once per metric context.
fn cached_schedule(context: &mut MetricContext) -> Result<Arc<CashFlowSchedule>> {
    if context.internal_schedule.is_none() {
        let inst = Arc::clone(&context.instrument);
        let facility = inst
            .as_any()
            .downcast_ref::<RevolvingCredit>()
            .ok_or(finstack_quant_core::InputError::Invalid)?;
        let schedule = facility.raw_cashflow_schedule(&context.curves, context.as_of)?;
        context.internal_schedule = Some(Arc::new(schedule));
    }
    context
        .internal_schedule
        .as_ref()
        .map(Arc::clone)
        .ok_or_else(|| finstack_quant_core::InputError::Invalid.into())
}

/// Drawn balance on `date`: the deterministic replay after the anchor, or
/// the anchor balance of a stochastic facility.
fn drawn_at(facility: &RevolvingCredit, as_of: Date, date: Date) -> Result<f64> {
    if facility.is_deterministic() {
        Ok(
            super::super::cashflow_engine::calculate_drawn_balance_at_date(facility, as_of, date)?
                .amount(),
        )
    } else {
        Ok(facility.drawn_amount.amount())
    }
}

/// Cash interest accrued from the start of the accrual period containing
/// `settlement` up to `settlement`, pro rata on the facility day count from
/// that period's scheduled interest flow. Zero when no interest flow exists
/// for the period (nothing drawn) or settlement lies past the last period.
pub(crate) fn accrued_interest(
    facility: &RevolvingCredit,
    schedule: &CashFlowSchedule,
    settlement: Date,
) -> Result<f64> {
    let periods = super::super::utils::build_payment_periods(facility)?;
    let Some(period) = periods
        .iter()
        .find(|p| p.accrual_start <= settlement && settlement < p.accrual_end)
    else {
        return Ok(0.0);
    };
    let interest: f64 = schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.date == period.payment_date && cf.kind.is_interest_like())
        .map(|cf| cf.amount.amount())
        .sum();
    if interest == 0.0 {
        return Ok(0.0);
    }
    let elapsed = facility.day_count.year_fraction(
        period.accrual_start,
        settlement,
        DayCountContext::default(),
    )?;
    let full = facility.day_count.year_fraction(
        period.accrual_start,
        period.accrual_end,
        DayCountContext::default(),
    )?;
    if full <= 0.0 {
        return Ok(0.0);
    }
    Ok(interest * elapsed / full)
}

/// Holder-view cash flows strictly after `settlement`: coupons, fees,
/// principal repayments and funding legs (negative draws). PIK is excluded.
fn holder_flows(schedule: &CashFlowSchedule, settlement: Date) -> Vec<(Date, f64)> {
    schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.date > settlement && is_cash_settlement_kind(cf.kind))
        .map(|cf| (cf.date, cf.amount.amount()))
        .collect()
}

/// Dirty target price at settlement: the quoted clean price on the drawn
/// balance plus accrued, else the model value carried to settlement.
fn target_price(
    facility: &RevolvingCredit,
    schedule: &CashFlowSchedule,
    context: &MetricContext,
    settlement: Date,
) -> Result<f64> {
    let as_of = context.as_of;
    if let Some(px) = facility
        .instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price
    {
        let drawn = drawn_at(facility, as_of, settlement)?;
        return Ok(px / 100.0 * drawn + accrued_interest(facility, schedule, settlement)?);
    }
    let disc = context
        .curves
        .get_discount(facility.discount_curve_id.as_str())?;
    let carry = disc.df_between_dates(as_of, settlement)?;
    if carry <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "discount factor from {as_of} to settlement {settlement} must be positive"
        )));
    }
    Ok(context.base_value.amount() / carry)
}

/// Contractual margin at `as_of` for the discount-margin guess and the
/// floating-only guard.
fn contractual_margin(facility: &RevolvingCredit, as_of: Date) -> Result<f64> {
    match &facility.base_rate_spec {
        BaseRateSpec::Floating(spec) => Ok(spec
            .spread_bp
            .to_f64()
            .ok_or(finstack_quant_core::InputError::Invalid)?
            * 1e-4
            + facility.margin_delta_bp_at(as_of) * 1e-4),
        BaseRateSpec::Fixed { .. } => Err(finstack_quant_core::Error::Validation(format!(
            "RevolvingCredit {}: discount margin is defined for floating-rate facilities only",
            facility.id
        ))),
    }
}

/// Discount margin of a floating-rate facility: the constant spread over the
/// discount curve at which the contractual flows after settlement reprice to
/// the target (quoted clean price on the drawn balance plus accrued, else the
/// model value). Decimal, comparable with the contractual margin.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct DiscountMarginCalculator;

impl MetricCalculator for DiscountMarginCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let schedule = cached_schedule(context)?;
        let facility: &RevolvingCredit = context.instrument_as()?;
        let as_of = context.as_of;
        let margin = contractual_margin(facility, as_of)?;
        let settlement = facility.settlement_date(as_of)?;
        let target = target_price(facility, &schedule, context, settlement)?;
        let flows = holder_flows(&schedule, settlement);
        let disc = context
            .curves
            .get_discount(facility.discount_curve_id.as_str())?;
        let compounds = compounding_frequency(facility.frequency);
        solve_discount_margin(
            |dm| pv_with_discount_margin(&flows, settlement, disc.as_ref(), compounds, dm),
            target,
            margin,
        )
    }
}

/// Clean price per 100 of the drawn balance at settlement implied by the
/// quoted discount margin (`market_quotes.quoted_discount_margin`, decimal):
/// `(PV of the flows after settlement at that margin − accrued) / drawn × 100`.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct PriceFromDmCalculator;

impl MetricCalculator for PriceFromDmCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let schedule = cached_schedule(context)?;
        let facility: &RevolvingCredit = context.instrument_as()?;
        let as_of = context.as_of;
        contractual_margin(facility, as_of)?;
        let dm = facility
            .instrument_pricing_overrides
            .market_quotes
            .quoted_discount_margin
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "RevolvingCredit {}: price_from_dm requires \
                     instrument_pricing_overrides.market_quotes.quoted_discount_margin (decimal)",
                    facility.id
                ))
            })?;
        let settlement = facility.settlement_date(as_of)?;
        let drawn = drawn_at(facility, as_of, settlement)?;
        if drawn <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "RevolvingCredit {}: price_from_dm needs a positive drawn balance at settlement \
                 {settlement} (a quote applies to funded principal only)",
                facility.id
            )));
        }
        let flows = holder_flows(&schedule, settlement);
        let disc = context
            .curves
            .get_discount(facility.discount_curve_id.as_str())?;
        let pv = pv_with_discount_margin(
            &flows,
            settlement,
            disc.as_ref(),
            compounding_frequency(facility.frequency),
            dm,
        )?;
        let accrued = accrued_interest(facility, &schedule, settlement)?;
        Ok((pv - accrued) / drawn * 100.0)
    }
}

/// Yield to maturity: the IRR on the facility day count of the holder-view
/// flows after settlement against the target price paid at settlement.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct YtmCalculator;

impl MetricCalculator for YtmCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let schedule = cached_schedule(context)?;
        let facility: &RevolvingCredit = context.instrument_as()?;
        let as_of = context.as_of;
        let settlement = facility.settlement_date(as_of)?;
        let target = target_price(facility, &schedule, context, settlement)?;
        let mut flows = vec![(settlement, -target)];
        flows.extend(holder_flows(&schedule, settlement));
        finstack_quant_core::cashflow::xirr_with_daycount(&flows, facility.day_count, None)
    }
}

/// Running cash all-in rate: cash interest and every fee after the
/// valuation date over the time-weighted drawn balance to maturity.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct AllInRateCalculator;

impl MetricCalculator for AllInRateCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let schedule = cached_schedule(context)?;
        let facility: &RevolvingCredit = context.instrument_as()?;
        let mut with_opening = (*schedule).clone();
        // The outstanding path needs the opening funding leg; a seasoned
        // facility's schedule starts at the anchor balance without one.
        if facility.commitment_date <= context.as_of && facility.drawn_amount.amount() > 0.0 {
            let mut flows = with_opening.get_flows().to_vec();
            flows.push(finstack_quant_core::cashflow::CashFlow::new(
                facility.commitment_date,
                None,
                facility.drawn_amount * -1.0,
                CFKind::Notional,
                0.0,
                None,
            ));
            with_opening = crate::cashflow::traits::schedule_from_classified_flows(
                flows,
                facility.day_count,
                crate::cashflow::traits::ScheduleBuildOpts {
                    notional_hint: Some(finstack_quant_core::money::Money::from((
                        0_i64,
                        facility.commitment_amount.currency(),
                    ))),
                    meta: with_opening.get_meta().clone(),
                },
            );
        }
        all_in_rate_from_schedule(
            &with_opening,
            context.as_of,
            facility.day_count,
            facility.maturity,
        )
    }
}

/// Cash interest accrued to the settlement date on the drawn balance, in the
/// facility currency.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct AccruedInterestCalculator;

impl MetricCalculator for AccruedInterestCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let schedule = cached_schedule(context)?;
        let facility: &RevolvingCredit = context.instrument_as()?;
        let settlement = facility.settlement_date(context.as_of)?;
        accrued_interest(facility, &schedule, settlement)
    }
}
