//! Quote arithmetic shared by loan-style instruments (term loans, revolving
//! credit facilities): discount-margin repricing, the discount-margin solve,
//! and the running all-in rate. Each instrument supplies its own flows,
//! settlement date and outstanding path; nothing here reads an instrument.
//!
//! # Conventions
//!
//! - A discount margin is a constant additive spread (decimal) over the
//!   instrument's **discount curve** such that the PV of the contractual flows
//!   equals the target price. Each flow is discounted with the margin added to
//!   the periodically compounded zero rate implied by the base discount
//!   factor, the same mechanics as the bond Z-spread and FRN DM paths
//!   (Fabozzi; Bloomberg YAS).
//! - Quotes follow LSTA/LMA practice: a clean price applies to the funded
//!   outstanding at settlement plus accrued cash interest; the undrawn
//!   commitment is not part of the quote.
//!
//! # Quick Example
//! ```rust
//! use finstack_quant_valuations::instruments::fixed_income::loan_quotes::compounding_frequency;
//! use finstack_quant_core::dates::Tenor;
//!
//! assert_eq!(compounding_frequency(Tenor::quarterly()), 4.0);
//! ```

use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext, Tenor};
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

use crate::cashflow::builder::CashFlowSchedule;
use crate::instruments::fixed_income::bond::metrics::price_yield_spread::z_spread::z_spread_discount_factor;

/// Absolute sanity bound on a solved discount margin, decimal (±5000 bp).
/// Distressed loans legitimately solve well above 2000 bp; the bound only
/// guards against solver divergence.
pub const DISCOUNT_MARGIN_BOUND: f64 = 0.50;

/// Periodic compounding frequency for the discount-margin zero-rate shift,
/// from a contractual coupon frequency (quarterly → 4). Mirrors the FRN
/// `bond_z_spread_compounding_frequency` helper.
///
/// # Arguments
///
/// * `frequency` - Contractual coupon or fee payment frequency.
pub fn compounding_frequency(frequency: Tenor) -> f64 {
    let years = frequency.to_years();
    if years > 0.0 && years.is_finite() {
        (1.0 / years).round().max(1.0)
    } else {
        1.0
    }
}

/// Present value of `flows` at `settlement` with `dm` added to the
/// periodically compounded zero rate of each discount factor.
///
/// Flows dated on or before `settlement` are skipped. With `dm = 0` the
/// result reproduces plain curve discounting of the same flows.
///
/// # Arguments
///
/// * `flows` - Dated signed cash amounts in the instrument currency (holder
///   view: receipts positive, funding legs negative).
/// * `settlement` - Anchor date the flows are discounted to.
/// * `disc` - Discount curve of the instrument.
/// * `compounds_per_year` - Compounding frequency of the zero-rate shift, from
///   [`compounding_frequency`].
/// * `dm` - Discount margin, decimal (`0.025` = 250 bp).
///
/// # Errors
///
/// Returns an error when a discount factor cannot be read from the curve or
/// the spread-adjusted compounding base is non-positive.
pub fn pv_with_discount_margin(
    flows: &[(Date, f64)],
    settlement: Date,
    disc: &dyn Discounting,
    compounds_per_year: f64,
    dm: f64,
) -> Result<f64> {
    let mut pv = NeumaierAccumulator::new();
    for (date, amount) in flows {
        if *date <= settlement {
            continue;
        }
        let t = disc
            .day_count()
            .year_fraction(settlement, *date, DayCountContext::default())?;
        let df = disc.df_between_dates(settlement, *date)?;
        let df_dm = z_spread_discount_factor(df, t, dm, compounds_per_year)?;
        pv.add(amount * df_dm);
    }
    Ok(pv.total())
}

/// Solve the discount margin whose repriced PV equals `target`.
///
/// Brent on the decimal spread axis with tolerance `1e-10` (about 0.001 bp)
/// and a ±500 bp starting bracket around `guess`; PV is strictly decreasing
/// in the margin, so the root is unique. The first pricing error inside the
/// objective is returned instead of a margin.
///
/// # Arguments
///
/// * `pv` - Repricing closure, normally [`pv_with_discount_margin`] bound to
///   the instrument's flows.
/// * `target` - Dirty target price in the instrument currency.
/// * `guess` - Initial margin, decimal; the contractual margin is the exact
///   solution for a par-quoted loan on a flat consistent curve.
///
/// # Errors
///
/// Returns the first error `pv` raised, an error when the solver fails, or an
/// error when the solution leaves [`DISCOUNT_MARGIN_BOUND`].
pub fn solve_discount_margin(
    pv: impl Fn(f64) -> Result<f64>,
    target: f64,
    guess: f64,
) -> Result<f64> {
    // Keep the first repricing error and report it instead of a solver
    // failure or a root found around it, as the bond YTM solver does. PV is
    // decreasing in the margin and fails only where it diverges upward (a
    // non-positive compounding base), so the residual there is large and
    // positive.
    let pricing_error: std::cell::RefCell<Option<finstack_quant_core::Error>> =
        std::cell::RefCell::new(None);
    let objective = |dm: f64| -> f64 {
        match pv(dm) {
            Ok(value) => value - target,
            Err(err) => {
                pricing_error.borrow_mut().get_or_insert(err);
                1e12
            }
        }
    };
    let solver = BrentSolver::new()
        .tolerance(1e-10)
        .initial_bracket_size(Some(0.05));
    let solved = solver.solve(objective, guess);
    if let Some(err) = pricing_error.into_inner() {
        return Err(err);
    }
    let dm = solved?;
    if dm.abs() > DISCOUNT_MARGIN_BOUND {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Discount margin {} bp exceeds sanity bounds (±{} bp)",
            dm * 1e4,
            DISCOUNT_MARGIN_BOUND * 1e4
        )));
    }
    Ok(dm)
}

/// Running cash all-in rate of a schedule: cash interest and fees after
/// `as_of` divided by the time-weighted outstanding principal over the same
/// horizon. PIK interest is excluded from the numerator; a one-time fee dated
/// on `as_of` is excluded too (use the effective-rate metrics for
/// origination economics).
///
/// # Arguments
///
/// * `schedule` - Full cashflow schedule with `meta.issue_date` set, so the
///   outstanding path can be rebuilt.
/// * `as_of` - Valuation date; only later flows and balances count.
/// * `day_count` - Accrual day count used to time-weight the outstanding.
/// * `maturity` - Horizon the outstanding is integrated to.
///
/// # Errors
///
/// Returns an error when the outstanding path cannot be rebuilt or a year
/// fraction fails.
pub fn all_in_rate_from_schedule(
    schedule: &CashFlowSchedule,
    as_of: Date,
    day_count: DayCount,
    maturity: Date,
) -> Result<f64> {
    let cash_cost: f64 = schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.date > as_of)
        .filter_map(|cf| match cf.kind {
            CFKind::Fixed
            | CFKind::FloatReset
            | CFKind::Stub
            | CFKind::Fee
            | CFKind::CommitmentFee
            | CFKind::UsageFee
            | CFKind::FacilityFee
            | CFKind::LcFee
            | CFKind::FrontingFee => Some(cf.amount.amount()),
            _ => None,
        })
        .sum();

    let out_path = schedule.outstanding_by_date()?;
    let mut time_weighted_outstanding = 0.0;
    let mut prev_date = as_of;
    let mut prev_outstanding = out_path
        .iter()
        .take_while(|(d, _)| *d <= as_of)
        .last()
        .map_or(0.0, |(_, amount)| amount.amount());
    for (d, amt) in &out_path {
        if *d <= as_of {
            continue;
        }
        let target = (*d).min(maturity);
        let yf = day_count.year_fraction(prev_date, target, DayCountContext::default())?;
        time_weighted_outstanding += prev_outstanding * yf;
        prev_date = target;
        prev_outstanding = amt.amount();
    }
    if prev_date < maturity {
        let yf = day_count.year_fraction(prev_date, maturity, DayCountContext::default())?;
        time_weighted_outstanding += prev_outstanding * yf;
    }
    if time_weighted_outstanding <= 0.0 {
        return Ok(0.0);
    }
    Ok(cash_cost / time_weighted_outstanding)
}

/// Period-level effective-interest-rate amortization outputs for reporting.
#[derive(Debug, Clone)]
pub struct OidEirPeriod {
    /// Period end date.
    pub date: Date,
    /// OID amortization for the period: effective interest income less cash
    /// interest and fees.
    pub oid_amortization: Money,
    /// Amortized-cost carrying balance at the period end.
    pub closing_balance: Money,
}

/// Effective-interest-rate amortization schedule.
#[derive(Debug, Clone)]
pub struct OidEirSchedule {
    /// Effective interest rate (XIRR of the full flow set on the instrument
    /// day count), decimal.
    pub effective_rate: f64,
    /// Period-by-period amortization details.
    pub periods: Vec<OidEirPeriod>,
}

/// Build an effective-interest-rate amortization schedule from a full
/// cashflow schedule plus extra dated flows (an upfront fee or OID the
/// schedule itself does not carry).
///
/// Flows are bucketed by date: interest (`Fixed`, `FloatReset`, `Stub`),
/// fees when `include_fees`, amortization, and notional funding legs. The
/// effective rate is the XIRR of the bucket totals on `day_count`; the
/// opening carrying amount is the first bucket's notional funding, and each
/// later period accretes it at the effective rate less the period's cash.
///
/// # Arguments
///
/// * `schedule` - Full cashflow schedule of the instrument.
/// * `extra_fees` - Dated signed amounts merged into the buckets as fees
///   (positive receipts to the holder), for example the upfront fee on the
///   funding date.
/// * `opening_funding` - Optional notional funding leg `(date, amount)` the
///   schedule does not carry (negative for a lender funding at the anchor);
///   it sets the opening carrying amount.
/// * `day_count` - Day count for the XIRR and the period accretion.
/// * `currency` - Currency of the reported money amounts.
/// * `include_fees` - Whether fee flows enter the effective yield.
///
/// # Errors
///
/// Returns an error when there are no flows, the XIRR does not converge, a
/// year fraction fails or an amount is not representable as `Money`.
pub fn oid_eir_schedule_from_flows(
    schedule: &CashFlowSchedule,
    extra_fees: &[(Date, f64)],
    opening_funding: Option<(Date, f64)>,
    day_count: DayCount,
    currency: finstack_quant_core::currency::Currency,
    include_fees: bool,
) -> Result<OidEirSchedule> {
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Bucket {
        total: f64,
        interest: f64,
        notional: f64,
    }

    let mut buckets: BTreeMap<Date, Bucket> = BTreeMap::new();
    for cf in schedule.get_flows() {
        let amount = cf.amount.amount();
        match cf.kind {
            CFKind::Fixed | CFKind::FloatReset | CFKind::Stub => {
                let b = buckets.entry(cf.date).or_default();
                b.total += amount;
                b.interest += amount;
            }
            CFKind::Fee
            | CFKind::CommitmentFee
            | CFKind::UsageFee
            | CFKind::FacilityFee
            | CFKind::LcFee
            | CFKind::FrontingFee
                if include_fees =>
            {
                let b = buckets.entry(cf.date).or_default();
                b.total += amount;
                b.interest += amount;
            }
            CFKind::Amortization => buckets.entry(cf.date).or_default().total += amount,
            CFKind::Notional => {
                let b = buckets.entry(cf.date).or_default();
                b.total += amount;
                b.notional += amount;
            }
            _ => {}
        }
    }
    if include_fees {
        for (date, amount) in extra_fees {
            let b = buckets.entry(*date).or_default();
            b.total += amount;
            b.interest += amount;
        }
    }
    if let Some((date, amount)) = opening_funding {
        let b = buckets.entry(date).or_default();
        b.total += amount;
        b.notional += amount;
    }

    let flows: Vec<(Date, f64)> = buckets
        .iter()
        .map(|(d, b)| (*d, b.total))
        .filter(|(_, amt)| amt.abs() > 0.0)
        .collect();
    let effective_rate =
        finstack_quant_core::cashflow::xirr_with_daycount(flows.as_slice(), day_count, None)?;

    let mut periods = Vec::new();
    let mut iter = buckets.iter();
    let (start_date, start_bucket) = iter
        .next()
        .ok_or(finstack_quant_core::InputError::TooFewPoints)?;
    // The opening carrying amount is the notional funding only; fees or
    // interest in the first bucket would overstate it.
    let mut opening_balance = -start_bucket.notional;
    let mut prev = *start_date;
    for (date, bucket) in iter {
        let yf = day_count.year_fraction(prev, *date, DayCountContext::default())?;
        let interest_income = opening_balance * effective_rate * yf;
        let closing_balance = opening_balance + interest_income - bucket.total;
        let oid_amortization = interest_income - bucket.interest;
        periods.push(OidEirPeriod {
            date: *date,
            oid_amortization: Money::new(oid_amortization, currency)?,
            closing_balance: Money::new(closing_balance, currency)?,
        });
        opening_balance = closing_balance;
        prev = *date;
    }
    Ok(OidEirSchedule {
        effective_rate,
        periods,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use time::macros::date;

    /// A repricing failure surfaces as that error, not as a solver failure.
    #[test]
    fn discount_margin_solve_propagates_the_pricing_error() {
        let err = solve_discount_margin(
            |_| {
                Err(finstack_quant_core::Error::Validation(
                    "discount curve 'USD-OIS' not found".to_string(),
                ))
            },
            100.0,
            0.02,
        )
        .expect_err("pricing always fails");
        assert!(
            err.to_string()
                .contains("discount curve 'USD-OIS' not found"),
            "{err}"
        );
    }

    #[test]
    fn zero_margin_reproduces_curve_discounting_and_solve_recovers_it() {
        let settlement = date!(2025 - 01 - 15);
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(settlement)
            .day_count(DayCount::Act365F)
            .knots([
                (0.0, 1.0),
                (1.0, (-0.04_f64).exp()),
                (5.0, (-0.20_f64).exp()),
            ])
            .build()
            .expect("curve");
        let flows = vec![
            (date!(2025 - 07 - 15), 30.0),
            (date!(2026 - 01 - 15), 1_030.0),
        ];
        let plain: f64 = flows
            .iter()
            .map(|(d, a)| a * disc.df_between_dates(settlement, *d).expect("df"))
            .sum();
        let pv0 = pv_with_discount_margin(&flows, settlement, &disc, 2.0, 0.0).expect("pv");
        assert!((pv0 - plain).abs() < 1e-9);
        let target = pv_with_discount_margin(&flows, settlement, &disc, 2.0, 0.0125).expect("pv");
        let dm = solve_discount_margin(
            |dm| pv_with_discount_margin(&flows, settlement, &disc, 2.0, dm),
            target,
            0.0,
        )
        .expect("solve");
        assert!((dm - 0.0125).abs() < 1e-9, "{dm}");
    }
}
