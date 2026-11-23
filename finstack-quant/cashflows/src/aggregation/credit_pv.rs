//! Credit-adjusted present values, WAL, and period-level PV aggregation.

use super::periods::*;
use finstack_quant_core::cashflow::{CFKind, CashFlow};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DateExt, Period, PeriodId};
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_core::money::Money;

use indexmap::IndexMap;

/// Survival-weighted settlement PV, excluding any estimated principal recovery.
pub(super) fn credit_adjusted_period_pv(cf: &CashFlow, df: f64, sp: f64, base: Date) -> f64 {
    if !cf.get_kind().is_cash_settlement() || cf.get_date() <= base {
        return 0.0;
    }

    // Recovery / AccruedOnDefault / ProtectionPayment are already post-default;
    // discount without survival.
    if matches!(
        cf.get_kind(),
        CFKind::Recovery | CFKind::AccruedOnDefault | CFKind::ProtectionPayment
    ) {
        return cf.get_amount().amount() * df;
    }

    cf.get_amount().amount() * df * sp
}

/// Partition a protection interval `(eff_start, pay_date]` along monthly steps
/// and any interior hazard-curve knots so midpoint default-time discounting
/// captures both rate convexity and hazard term structure.
pub(super) fn build_recovery_grid(
    eff_start: Date,
    pay_date: Date,
    hazard: Option<&HazardCurve>,
) -> finstack_quant_core::Result<Vec<Date>> {
    let mut grid = vec![eff_start];
    let mut month = 1_i32;
    loop {
        let step_date = eff_start.add_months(month)?;
        if step_date >= pay_date {
            break;
        }
        grid.push(step_date);
        month += 1;
    }
    if let Some(h) = hazard {
        let h_base = h.base_date();
        for (k_t, _) in h.iter_knots() {
            if !k_t.is_finite() || k_t <= 0.0 {
                continue;
            }
            for basis in [360.0_f64, 365.0, 365.25] {
                let days = (k_t * basis).round();
                if days.abs() > 4_000_000.0 {
                    continue;
                }
                let offset = time::Duration::days(days as i64);
                if let Some(d) = h_base.checked_add(offset)
                    && eff_start < d
                    && d < pay_date
                {
                    grid.push(d);
                }
            }
        }
    }
    grid.push(pay_date);
    grid.sort();
    grid.dedup();
    Ok(grid)
}

/// Integrate midpoint-discounted default-time recovery (per unit of
/// `recovery_rate`) for one funded lot `(start, amount)` up to `pay_date`.
pub(super) fn lot_recovery_pv(
    start: Date,
    amount: f64,
    pay_date: Date,
    curve: &dyn Discounting,
    df_as_of: f64,
    hazard: Option<&HazardCurve>,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    let eff_start = start.max(as_of);
    if eff_start >= pay_date || amount <= 0.0 {
        return Ok(0.0);
    }
    let grid = build_recovery_grid(eff_start, pay_date, hazard)?;
    let curve_dc = curve.day_count();
    let curve_base = curve.base_date();
    let curve_ctx = curve.day_count_context();

    let mut q0 = survival_on_date(grid[0], hazard, as_of)?;
    let mut t0 = curve_dc.signed_year_fraction(curve_base, grid[0], curve_ctx)?;
    let mut acc = NeumaierAccumulator::new();
    for &d1 in &grid[1..] {
        let q1 = survival_on_date(d1, hazard, as_of)?;
        let dq = default_mass(q0, q1)?;
        let t1 = curve_dc.signed_year_fraction(curve_base, d1, curve_ctx)?;
        if dq > 0.0 {
            let df_mid = curve.df(0.5 * (t0 + t1))?;
            if !df_mid.is_finite() || df_mid <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Invalid discount factor at midpoint before {d1}: {df_mid}"
                )));
            }
            let rel_df = df_mid / df_as_of;
            if !rel_df.is_finite() || rel_df <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Invalid relative discount factor before {d1}: {rel_df}"
                )));
            }
            acc.add(amount * rel_df * dq)?;
        }
        q0 = q1;
        t0 = t1;
    }
    acc.get_total()
}

/// Present values of a complete cashflow stream under default-time recovery.
///
/// Recovery is estimated only while principal is funded, integrated over
/// monthly steps and interior hazard-curve knots using midpoint discount
/// factors from each row's discount curve. Future draws and PIK capitalization
/// establish principal on their economic balance dates. Within each currency,
/// repayments consume funded principal in first-in, first-out order. Coupons
/// have zero recovery and no accrued-on-default term; this is not the ISDA CDS
/// standard model.
///
/// The stream must include all future funding, capitalization, and principal
/// repayments. Opening funded principal is the remaining repayments less future
/// increases, independently per currency. Omitted historical funding is
/// therefore treated as already funded at `as_of`; historical cash itself has
/// zero PV. For each funded lot `N` active over `(max(F, as_of), T]`, recovery
/// sums `N * R * D(as_of, t_mid,m) * (S(t_{m-1}) - S(t_m))` across sub-steps
/// where `D(as_of, t) = df(t) / df(as_of)`. A single row cannot establish this
/// funding history. Explicit principal deltas determine face amounts
/// independently of discounted or premium cash settlements. Once the economic
/// principal reduction is before valuation, an unpaid settlement is a cash
/// receivable: its cash amount is the recoverable claim, consistently for raw
/// and normalized schedules. Capitalization paid early is paired with
/// reductions on the same economic principal date before allocating the
/// remaining FIFO lots, and has zero funded-principal recovery. Zero-cash
/// balance replay rows never create another recoverable settlement claim.
///
/// # Arguments
///
/// * `cashflows` - Complete classified cashflow stream, in any order. Principal
///   currency is preserved; non-cash rows have zero settlement PV.
/// * `discount_curves` - One discount curve reference per cashflow, used to
///   compute relative discount factors from `as_of` to each row's payment
///   date and midpoint default-time recovery discount factors in its native
///   currency.
/// * `hazard` - Optional survival curve. Probabilities are conditional on survival
///   at the valuation date; `None` gives risk-free cashflows and requires no recovery.
/// * `recovery_rate` - Optional recovery rate in `[0, 1]` applied to defaulted
///   funded principal; `None` excludes estimated recovery. Explicit defaulted
///   principal cannot be combined with this estimated recovery term.
/// * `as_of` - Valuation date. Survival is conditional on survival at this date,
///   and flows on or before it have zero PV.
///
/// # Returns
///
/// Native-currency present values in exactly the same order as `cashflows`.
///
/// # Errors
///
/// Returns an error for mismatched lengths, non-finite or out-of-range inputs,
/// increasing future survival, inconsistent funding/repayment amounts, or a
/// recovery assumption without a hazard curve.
pub fn credit_adjusted_cashflow_pvs(
    cashflows: &[CashFlow],
    discount_curves: &[&dyn Discounting],
    hazard: Option<&HazardCurve>,
    recovery_rate: Option<f64>,
    as_of: Date,
) -> finstack_quant_core::Result<Vec<f64>> {
    if cashflows.len() != discount_curves.len() {
        return Err(finstack_quant_core::Error::Validation(
            "cashflows and discount curves must have matching lengths".into(),
        ));
    }
    validate_recovery(cashflows, hazard, recovery_rate)?;
    let mut pvs = Vec::with_capacity(cashflows.len());
    for (cashflow, &curve) in cashflows.iter().zip(discount_curves) {
        let discount_factor = if cashflow.get_date() <= as_of {
            1.0
        } else {
            curve.df_between_dates(as_of, cashflow.get_date())?
        };
        let survival = if cashflow.get_date() <= as_of {
            1.0
        } else {
            survival_on_date(cashflow.get_date(), hazard, as_of)?
        };
        pvs.push(credit_adjusted_period_pv(
            cashflow,
            discount_factor,
            survival,
            as_of,
        ));
    }
    if let Some(recovery) = recovery_rate.filter(|r| *r > 0.0) {
        let funding = funded_principal(cashflows, as_of)?;
        for (index, lots) in funding.iter().enumerate() {
            if lots.is_empty() {
                continue;
            }
            let pay_date = cashflows[index].get_date();
            let curve = discount_curves[index];
            let t_as_of = curve.day_count().signed_year_fraction(
                curve.base_date(),
                as_of,
                curve.day_count_context(),
            )?;
            let df_as_of = curve.df(t_as_of)?;
            if !df_as_of.is_finite() || df_as_of <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Invalid discount factor on 'from' date ({as_of}): {df_as_of}"
                )));
            }
            let mut row_acc = NeumaierAccumulator::new();
            for &(start, amount) in lots {
                row_acc.add(lot_recovery_pv(
                    start, amount, pay_date, curve, df_as_of, hazard, as_of,
                )?)?;
            }
            pvs[index] += recovery * row_acc.get_total()?;
        }
    }
    Ok(pvs)
}

pub(super) fn validate_recovery(
    cashflows: &[CashFlow],
    hazard: Option<&HazardCurve>,
    recovery_rate: Option<f64>,
) -> finstack_quant_core::Result<()> {
    if let Some(rate) = recovery_rate {
        if !rate.is_finite() || !(0.0..=1.0).contains(&rate) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "recovery rate must be finite and in [0, 1]; got {rate}"
            )));
        }
        if hazard.is_none() {
            return Err(finstack_quant_core::Error::Validation(
                "estimated principal recovery requires a hazard curve".into(),
            ));
        }
        if rate > 0.0
            && cashflows.iter().any(|cf| {
                matches!(
                    cf.get_kind(),
                    CFKind::DefaultedNotional | CFKind::ProtectionPayment
                )
            })
        {
            return Err(finstack_quant_core::Error::Validation(
                "schedule contains explicit DefaultedNotional or ProtectionPayment flows; pass recovery_rate=None \
                 (or 0.0) to avoid double-counting recovery from explicit events and the hazard curve"
                    .into(),
            ));
        }
    }
    Ok(())
}

/// Calculate Act/365F weighted average life from dated principal reductions.
///
/// Only positive principal amounts strictly after `as_of` contribute. Each
/// contribution is weighted by its Act/365F year fraction, and the result is
/// expressed in years. Non-positive or already-realized entries are ignored;
/// when no eligible principal remains, this returns `0.0`.
///
/// # Arguments
///
/// * `principal` - Dated principal reductions as `(date, amount)` pairs; only
///   positive amounts strictly after `as_of` contribute. The caller chooses
///   the date convention: [`CashFlowSchedule::wal`](crate::builder::CashFlowSchedule::wal)
///   passes each repayment's balance-effective date and par amount.
/// * `as_of` - Valuation date from which Act/365F time weights are measured.
///
/// # Errors
///
/// Returns an error if eligible principal reductions use more than one
/// currency or the Act/365F year-fraction calculation fails. Currencies of
/// ignored entries are not examined.
pub fn weighted_average_life_from_principal<I>(
    principal: I,
    as_of: Date,
) -> finstack_quant_core::Result<f64>
where
    I: IntoIterator<Item = (Date, Money)>,
{
    let mut currency = None;
    let mut weighted = NeumaierAccumulator::default();
    let mut total = NeumaierAccumulator::default();
    for (date, amount) in principal {
        if date <= as_of || amount.amount() <= 0.0 {
            continue;
        }
        if let Some(expected) = currency {
            if amount.currency() != expected {
                return Err(finstack_quant_core::Error::CurrencyMismatch {
                    expected,
                    actual: amount.currency(),
                });
            }
        } else {
            currency = Some(amount.currency());
        }
        let years = finstack_quant_core::dates::DayCount::Act365F.year_fraction(
            as_of,
            date,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        weighted.add(amount.amount() * years)?;
        total.add(amount.amount())?;
    }
    let principal_total = total.get_total()?;
    Ok(if principal_total > 0.0 {
        weighted.get_total()? / principal_total
    } else {
        0.0
    })
}

pub(super) fn principal_repayment_amount(
    cf: &CashFlow,
    base: Date,
) -> finstack_quant_core::Result<f64> {
    if !cf.get_kind().is_repayment_like() || cf.get_amount().amount() <= 0.0 {
        return Ok(0.0);
    }
    let balance_date = cf
        .get_balance_date()
        .ok_or_else(|| missing_principal_delta(cf))?;
    if balance_date < base {
        return Ok(cf.get_amount().amount());
    }
    Ok(match cf.get_principal_delta() {
        Some(delta) if delta.currency() != cf.get_amount().currency() => {
            return Err(finstack_quant_core::Error::CurrencyMismatch {
                expected: cf.get_amount().currency(),
                actual: delta.currency(),
            });
        }
        Some(delta) if delta.amount() < 0.0 => -delta.amount(),
        Some(delta) if delta.amount() > 0.0 => 0.0,
        // A zero delta can preserve an unsettled cash claim after its economic
        // balance movement has already been incorporated into opening notional.
        Some(_) => cf.get_amount().amount().max(0.0),
        None => return Err(missing_principal_delta(cf)),
    })
}

/// A principal row without its explicit delta or balance date: nothing is
/// inferred from its cash or kind (record CORE-CONVENTIONS-015 #4).
pub(crate) fn missing_principal_delta(cf: &CashFlow) -> finstack_quant_core::Error {
    finstack_quant_core::Error::Validation(format!(
        "{} row on {} has no principal_delta or principal_date",
        cf.get_kind(),
        cf.get_date()
    ))
}

pub(super) fn principal_increase(cf: &CashFlow) -> finstack_quant_core::Result<f64> {
    if !cf.get_kind().is_principal_like() {
        return Ok(0.0);
    }
    let delta = cf
        .get_principal_delta()
        .ok_or_else(|| missing_principal_delta(cf))?;
    if delta.currency() != cf.get_amount().currency() {
        return Err(finstack_quant_core::Error::CurrencyMismatch {
            expected: cf.get_amount().currency(),
            actual: delta.currency(),
        });
    }
    Ok(delta.amount().max(0.0))
}

/// Assign each remaining repayment to FIFO dated funding lots, independently
/// per currency. Already-funded opening principal is inferred from the complete
/// remaining principal stream, so historical funding is never counted twice.
pub(super) fn funded_principal(
    cashflows: &[CashFlow],
    base: Date,
) -> finstack_quant_core::Result<Vec<Vec<(Date, f64)>>> {
    let mut result = vec![Vec::new(); cashflows.len()];
    let mut by_currency: IndexMap<Currency, Vec<usize>> = IndexMap::new();
    for (index, cf) in cashflows.iter().enumerate() {
        by_currency
            .entry(cf.get_amount().currency())
            .or_default()
            .push(index);
    }
    for (currency, indices) in by_currency {
        let mut increases = Vec::new();
        let mut capitalizations: std::collections::BTreeMap<Date, f64> =
            std::collections::BTreeMap::new();
        let mut repayments = Vec::new();
        let mut opening = NeumaierAccumulator::new();
        for &index in &indices {
            let cf = &cashflows[index];
            // Only principal rows carry a balance date and fund principal.
            let Some(balance_date) = cf.get_balance_date() else {
                continue;
            };
            if balance_date > base {
                let increase = principal_increase(cf)?;
                if increase > 0.0 {
                    if cf.get_kind() == CFKind::Pik || cf.get_amount().amount() == 0.0 {
                        *capitalizations.entry(balance_date).or_default() += increase;
                    } else {
                        increases.push((balance_date, increase));
                    }
                }
            }
        }
        for index in indices {
            let cf = &cashflows[index];
            let Some(balance_date) = cf.get_balance_date() else {
                continue;
            };
            let mut repayment = principal_repayment_amount(cf, base)?;
            // An early payment of capitalized interest extinguishes that
            // same-date capitalization, even when other funded principal is
            // available. Economic-only residual replay rows retain this pairing
            // after the cash itself has settled and been filtered away.
            if balance_date > base
                && (cf.get_date() < balance_date || cf.get_amount().amount() == 0.0)
            {
                let early_face = if cf.get_amount().amount() == 0.0 {
                    cf.get_principal_delta()
                        .map_or(0.0, |delta| (-delta.amount()).max(0.0))
                } else {
                    repayment
                };
                if let Some(increase) = capitalizations.get_mut(&balance_date) {
                    let capitalized = early_face.min(*increase);
                    *increase -= capitalized;
                    if cf.get_date() > base && repayment > 0.0 && capitalized > 0.0 {
                        result[index].push((cf.get_date(), capitalized));
                        repayment -= capitalized;
                    }
                }
            }
            if cf.get_date() > base && repayment > 0.0 {
                repayments.push((index, repayment));
                opening.add(repayment)?;
            }
        }
        if repayments.is_empty() {
            // Economic-only replay rows after the final cash settlement do not
            // create another recoverable cash claim.
            continue;
        }
        increases.extend(
            capitalizations
                .into_iter()
                .filter(|&(_, amount)| amount > 0.0),
        );
        for &(_, increase) in &increases {
            opening.add(-increase)?;
        }
        increases.sort_by_key(|&(date, _)| date);
        repayments.sort_by_key(|&(index, _)| cashflows[index].get_date());
        let scale = repayments
            .iter()
            .map(|&(_, amount)| amount)
            .fold(1.0, f64::max);
        let tolerance = scale * 1e-12;
        let opening_total = opening.get_total()?;
        if opening_total < -tolerance {
            return Err(finstack_quant_core::Error::Validation(format!(
                "future {currency} funding exceeds remaining principal repayments; \
                 recovery requires a complete principal stream"
            )));
        }
        let mut lots = std::collections::VecDeque::new();
        if opening_total > 0.0 {
            lots.push_back((base, opening_total));
        }
        lots.extend(increases);
        for (index, mut remaining) in repayments {
            let payment = &cashflows[index];
            while remaining > 0.0 {
                let Some((funded_at, available)) = lots.front_mut() else {
                    if remaining <= tolerance {
                        break;
                    }
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "{currency} principal repayment on {} exceeds principal funded by that date",
                        payment.get_date()
                    )));
                };
                if *funded_at > payment.get_date() {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "{currency} principal repayment on {} precedes its cash funding date {funded_at}",
                        payment.get_date()
                    )));
                }
                let amount = remaining.min(*available);
                result[index].push((*funded_at, amount));
                remaining -= amount;
                *available -= amount;
                if *available <= 0.0 {
                    lots.pop_front();
                }
            }
        }
    }
    Ok(result)
}

pub(super) fn default_mass(start_sp: f64, end_sp: f64) -> finstack_quant_core::Result<f64> {
    if end_sp > start_sp + 1e-12 {
        return Err(finstack_quant_core::Error::Validation(
            "survival probability must not increase over a funded principal interval".into(),
        ));
    }
    Ok((start_sp - end_sp).max(0.0))
}

/// Survival at `d` conditional on survival at `as_of`; one without a curve.
pub(super) fn survival_on_date(
    d: Date,
    hazard: Option<&HazardCurve>,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    if d == as_of {
        return Ok(1.0);
    }
    let sp = match hazard {
        Some(h) => {
            let sp_asof = h.sp_on_date(as_of)?;
            if !sp_asof.is_finite() || sp_asof <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "survival curve returned invalid base survival ({sp_asof}) at {as_of}"
                )));
            }
            h.sp_on_date(d)? / sp_asof
        }
        None => 1.0,
    };
    if !sp.is_finite() || sp < 0.0 || (d > as_of && sp > 1.0) {
        return Err(finstack_quant_core::Error::Validation(format!(
            "survival probability must be finite and in [0, 1] at future date {d}; got {sp}"
        )));
    }
    Ok(sp)
}

/// Reporting-period PV totals of a date-sorted cashflow stream.
///
/// Discount factors are relative to `as_of` (1.0 for rows on or before it);
/// with a hazard curve, survival weighting and `recovery_rate` on funded
/// principal follow [`credit_adjusted_cashflow_pvs`]. Non-cash rows and
/// rows on or before `as_of` have zero PV but keep their period bucket.
///
/// # Errors
///
/// Returns an error for invalid periods, curves, recovery assumptions, or an
/// inconsistent funding/repayment stream.
pub(crate) fn pv_by_period(
    sorted: &[CashFlow],
    periods: &[Period],
    disc: &dyn Discounting,
    hazard: Option<&HazardCurve>,
    recovery_rate: Option<f64>,
    as_of: Date,
) -> finstack_quant_core::Result<PeriodAggregation> {
    validate_periods(periods)?;
    if sorted.is_empty() || periods.is_empty() {
        return Ok(PeriodAggregation::default());
    }
    let curves: Vec<&dyn Discounting> = vec![disc; sorted.len()];
    let pvs = credit_adjusted_cashflow_pvs(sorted, &curves, hazard, recovery_rate, as_of)?;
    let mut out: IndexMap<PeriodId, IndexMap<Currency, Money>> =
        IndexMap::with_capacity(periods.len());
    let mut per_currency: IndexMap<Currency, NeumaierAccumulator> = IndexMap::new();
    for_each_period_bucket(sorted, periods, CashFlow::get_date, |p, start, bucket| {
        per_currency.clear();
        for (offset, cf) in bucket.iter().enumerate() {
            per_currency
                .entry(cf.get_amount().currency())
                .or_default()
                .add(pvs[start + offset])?;
        }
        let mut result: IndexMap<Currency, Money> = IndexMap::with_capacity(per_currency.len());
        for (&ccy, acc) in &per_currency {
            result.insert(ccy, Money::new(acc.get_total()?, ccy)?);
        }
        out.insert(p.id, result);
        Ok(())
    })?;
    Ok(PeriodAggregation(out))
}
