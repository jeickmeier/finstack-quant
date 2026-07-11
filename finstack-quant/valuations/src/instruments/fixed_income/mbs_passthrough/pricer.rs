//! Agency MBS passthrough pricing.
//!
//! This module provides discounting-based pricing for agency MBS passthroughs,
//! generating projected cashflows with prepayment and payment delay adjustments.
//!
//! # SIFMA Settlement
//!
//! TBA-eligible agency MBS settle on published SIFMA Good Delivery dates. The
//! [`sifma_settlement_for_period`] helper uses an exact embedded date when
//! available and an explicitly approximate nth-weekday date for long-dated
//! projected cash flows. Operational trade settlement must use the published
//! calendar, not the projection estimate.

use super::AgencyMbsPassthrough;
use crate::cashflow::builder::{CashFlowMeta, CashFlowSchedule};
use crate::cashflow::primitives::{CFKind, CashFlow};
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

/// Generated MBS cashflow for a single period.
#[derive(Debug, Clone)]
pub struct MbsCashflow {
    /// Accrual period start date
    pub period_start: Date,
    /// Accrual period end date
    pub period_end: Date,
    /// Actual payment date (after delay)
    pub payment_date: Date,
    /// SIFMA Good Delivery date, estimated outside published calendar coverage.
    pub sifma_date: Date,
    /// Scheduled principal payment
    pub scheduled_principal: f64,
    /// Prepayment (unscheduled principal)
    pub prepayment: f64,
    /// Interest payment
    pub interest: f64,
    /// Total cashflow
    pub total: f64,
    /// Beginning balance for this period
    pub beginning_balance: f64,
    /// Ending balance after this period
    pub ending_balance: f64,
    /// SMM used for this period
    pub smm: f64,
}

/// Derive the SIFMA Good Delivery settlement date for a given accrual period.
pub(crate) fn sifma_settlement_for_period(period_end: Date) -> Result<Date> {
    use finstack_quant_core::dates::{
        estimated_sifma_settlement_date_for_class, sifma_settlement_date, SifmaSettlementClass,
    };

    Ok(
        sifma_settlement_date(period_end.month(), period_end.year()).unwrap_or_else(|| {
            estimated_sifma_settlement_date_for_class(
                period_end.month(),
                period_end.year(),
                SifmaSettlementClass::A,
            )
        }),
    )
}

/// Generate projected cashflows for an agency MBS.
///
/// Exposed publicly for binding-layer consumers (e.g. the `instrument_cashflows`
/// export, which joins pool-state metadata — SMM, beginning/ending balance —
/// onto the canonical `CashFlowSchedule` rows). Internal details of
/// [`MbsCashflow`] may evolve between minor releases; callers should treat the
/// struct as a read-only diagnostic view.
pub fn generate_cashflows(
    mbs: &AgencyMbsPassthrough,
    as_of: Date,
    max_periods: Option<u32>,
) -> Result<Vec<MbsCashflow>> {
    use time::Duration;

    if mbs.wam == 0 {
        return Err(finstack_quant_core::Error::Validation(
            "WAM must be positive".to_string(),
        ));
    }

    let cap = max_periods.unwrap_or(mbs.wam) as usize;
    let mut cashflows = Vec::with_capacity(cap);
    let mut balance = mbs.current_face.amount();

    // Start from the active accrual period containing as_of, unless the pool
    // has a forward issue date inside the month, in which case the partial
    // pre-issue accrual month is skipped entirely.
    let effective_start = as_of.max(mbs.issue_date);
    let mut period_start =
        Date::from_calendar_date(effective_start.year(), effective_start.month(), 1)
            .map_err(|e| finstack_quant_core::Error::Validation(e.to_string()))?;
    if mbs.issue_date > period_start && as_of < mbs.issue_date {
        period_start = period_start
            .checked_add(Duration::days(32))
            .and_then(|d| Date::from_calendar_date(d.year(), d.month(), 1).ok())
            .unwrap_or(period_start);
    }

    let max_periods = max_periods.unwrap_or(mbs.wam);
    // Level-pay mortgage amortization is, by construction, computed on the
    // monthly mortgage rate (WAC / 12) — that is the contractual amortization
    // convention and is intentionally not day-counted.
    let monthly_mortgage_rate = mbs.wac / 12.0;

    let mut projected_count: u32 = 0;
    loop {
        if balance < 0.01 || projected_count >= max_periods {
            break;
        }

        let period_end = end_of_month(period_start)?;
        let payment_date = mbs.payment_date_for_accrual_period(period_start)?;

        if payment_date <= as_of {
            period_start = next_month_start(period_start)?;
            continue;
        }

        projected_count += 1;

        // PSA/CDR seasoning ramps key off pool age (WALA), measured from the
        // issue date.
        let seasoning = mbs.seasoning_months(period_end);
        let raw_smm = mbs.prepayment_model.smm(seasoning)?;
        if !raw_smm.is_finite() || !(0.0..=1.0).contains(&raw_smm) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "MBS prepayment model returned invalid SMM={raw_smm} at seasoning {seasoning} months; expected finite value in [0.0, 1.0]"
            )));
        }
        let smm = raw_smm;

        // `wam` is the *remaining* WAM at the valuation date, so the level-pay
        // amortization horizon shrinks by one per projected period — not by
        // the pool's seasoning, which would double-count the age already
        // netted out of a remaining WAM (matching the MC-OAS pricer).
        let remaining_months = mbs.wam.saturating_sub(projected_count - 1);
        let remaining_months = if remaining_months == 0 {
            1
        } else {
            remaining_months
        };
        let scheduled_principal = if remaining_months == 1 {
            balance
        } else if monthly_mortgage_rate > 1e-12 {
            let factor = (1.0 + monthly_mortgage_rate).powi(remaining_months as i32);
            let payment = balance * monthly_mortgage_rate * factor / (factor - 1.0);
            let interest_component = balance * monthly_mortgage_rate;
            (payment - interest_component).max(0.0).min(balance)
        } else {
            balance / remaining_months as f64
        };

        let prepayment = (balance - scheduled_principal).max(0.0) * smm;
        // Investor interest accrues at the pass-through rate over the actual
        // accrual-period day-count fraction, not a flat 1/12. The accrual
        // period is the full calendar month `[period_start, next_month_start)`.
        // For 30/360 this recovers exactly 1/12 each month, but Act/360 and
        // Act/365F vary by month length (28–31 days), so a fixed 1/12
        // mis-states accrued interest.
        let accrual_period_end = next_month_start(period_start)?;
        let period_yf = mbs.day_count.year_fraction(
            period_start,
            accrual_period_end,
            DayCountContext::default(),
        )?;
        let interest = balance * mbs.pass_through_rate * period_yf;
        let total_principal = scheduled_principal + prepayment;
        let ending_balance = (balance - total_principal).max(0.0);

        let sifma_date = sifma_settlement_for_period(period_end)?;

        cashflows.push(MbsCashflow {
            period_start,
            period_end,
            payment_date,
            sifma_date,
            scheduled_principal,
            prepayment,
            interest,
            total: total_principal + interest,
            beginning_balance: balance,
            ending_balance,
            smm,
        });

        balance = ending_balance;
        period_start = next_month_start(period_start)?;

        if period_end >= mbs.maturity {
            break;
        }
    }

    Ok(cashflows)
}

/// Build the canonical projected collateral schedule for an agency MBS.
pub(crate) fn build_projected_schedule(
    mbs: &AgencyMbsPassthrough,
    as_of: Date,
    max_periods: Option<u32>,
) -> Result<CashFlowSchedule> {
    let projected = generate_cashflows(mbs, as_of, max_periods)?;
    let mut flows = Vec::with_capacity(projected.len() * 3);

    for cf in projected {
        if cf.interest.abs() > f64::EPSILON {
            flows.push(CashFlow {
                date: cf.payment_date,
                reset_date: None,
                amount: Money::new(cf.interest, mbs.current_face.currency()),
                kind: CFKind::Fixed,
                accrual_factor: 0.0,
                rate: Some(mbs.pass_through_rate),
            });
        }
        if cf.scheduled_principal.abs() > f64::EPSILON {
            flows.push(CashFlow {
                date: cf.payment_date,
                reset_date: None,
                amount: Money::new(cf.scheduled_principal, mbs.current_face.currency()),
                kind: CFKind::Amortization,
                accrual_factor: 0.0,
                rate: None,
            });
        }
        if cf.prepayment.abs() > f64::EPSILON {
            flows.push(CashFlow {
                date: cf.payment_date,
                reset_date: None,
                amount: Money::new(cf.prepayment, mbs.current_face.currency()),
                kind: CFKind::PrePayment,
                accrual_factor: 0.0,
                rate: None,
            });
        }
    }

    Ok(crate::cashflow::traits::schedule_from_classified_flows(
        flows,
        mbs.day_count,
        crate::cashflow::traits::ScheduleBuildOpts {
            notional_hint: Some(mbs.current_face),
            meta: Some(CashFlowMeta {
                representation: crate::cashflow::builder::CashflowRepresentation::Projected,
                calendar_ids: Vec::new(),
                facility_limit: None,
                issue_date: Some(mbs.issue_date),
                maturity_date: None,
                accrual_periods: Vec::new(),
                accrual_day_counts: Vec::new(),
            }),
            ..Default::default()
        },
    ))
}

fn end_of_month(date: Date) -> Result<Date> {
    let year = date.year();
    let month = date.month();
    let days_in_month = month.length(year);
    Date::from_calendar_date(year, month, days_in_month)
        .map_err(|e| finstack_quant_core::Error::Validation(e.to_string()))
}

fn next_month_start(date: Date) -> Result<Date> {
    use time::Duration;
    let end = end_of_month(date)?;
    let next = end + Duration::days(1);
    Ok(next)
}

/// Discount a set of MBS cashflows to present value.
///
/// Uses the curve's own day count for time calculation, applying an optional
/// spread adjustment: `DF_spread = DF_base × exp(-spread × t)`.
fn discount_schedule(
    schedule: &CashFlowSchedule,
    curve: &DiscountCurve,
    as_of: Date,
    spread: f64,
) -> Result<f64> {
    let dc = curve.day_count();
    let mut pv = 0.0;
    for cf in &schedule.flows {
        let years = dc.year_fraction(as_of, cf.date, DayCountContext::default())?;
        let base_df = curve.df(years);
        let df = if spread.abs() > f64::EPSILON {
            base_df * (-spread * years).exp()
        } else {
            base_df
        };
        pv += cf.amount.amount() * df;
    }
    Ok(pv)
}

/// Price an agency MBS using discounting.
///
/// Uses the discount curve's own day count convention for computing
/// year fractions, ensuring consistency with the curve's interpolation.
pub(crate) fn price_mbs(
    mbs: &AgencyMbsPassthrough,
    market: &MarketContext,
    as_of: Date,
) -> Result<Money> {
    let schedule = build_projected_schedule(mbs, as_of, Some(mbs.wam + 12))?;

    if schedule.flows.is_empty() {
        return Ok(Money::new(0.0, mbs.current_face.currency()));
    }

    let discount_curve = market.get_discount(&mbs.discount_curve_id)?;
    let pv = discount_schedule(&schedule, &discount_curve, as_of, 0.0)?;

    Ok(Money::new(pv, mbs.current_face.currency()))
}

/// Price an agency MBS with a spread adjustment.
///
/// Adds a spread (in decimal) to the discount rate when computing present value.
pub(crate) fn price_with_spread(
    mbs: &AgencyMbsPassthrough,
    market: &MarketContext,
    as_of: Date,
    spread: f64,
) -> Result<f64> {
    let schedule = build_projected_schedule(mbs, as_of, Some(mbs.wam + 12))?;

    if schedule.flows.is_empty() {
        return Ok(0.0);
    }

    let discount_curve = market.get_discount(&mbs.discount_curve_id)?;
    discount_schedule(&schedule, &discount_curve, as_of, spread)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::builder::specs::PrepaymentModelSpec;
    use crate::cashflow::primitives::CFKind;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use time::Month;

    fn create_test_mbs() -> AgencyMbsPassthrough {
        AgencyMbsPassthrough::builder()
            .id(InstrumentId::new("TEST-MBS"))
            .pool_id("TEST-POOL".into())
            .agency(super::super::AgencyProgram::Fnma)
            .pool_type(super::super::PoolType::Generic)
            .original_face(Money::new(1_000_000.0, Currency::USD))
            .current_face(Money::new(1_000_000.0, Currency::USD))
            .current_factor(1.0)
            .wac(0.045)
            .pass_through_rate(0.04)
            .servicing_fee_rate(0.0025)
            .guarantee_fee_rate(0.0025)
            .wam(360)
            .issue_date(Date::from_calendar_date(2024, Month::January, 1).expect("valid"))
            .maturity(Date::from_calendar_date(2054, Month::January, 1).expect("valid"))
            .prepayment_model(PrepaymentModelSpec::psa(1.0))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Thirty360)
            .build()
            .expect("valid mbs")
    }

    fn create_test_market(as_of: Date) -> MarketContext {
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([
                (0.0, 1.0),
                (1.0, 0.96),
                (5.0, 0.80),
                (10.0, 0.60),
                (30.0, 0.30),
            ])
            .interp(InterpStyle::Linear)
            .build()
            .expect("valid curve");

        MarketContext::new().insert(disc)
    }

    #[test]
    fn test_generate_cashflows() {
        let mbs = create_test_mbs();
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");

        let cashflows = generate_cashflows(&mbs, as_of, Some(12)).expect("should generate");

        assert!(!cashflows.is_empty());
        assert!(cashflows.len() <= 12);
        assert!((cashflows[0].beginning_balance - 1_000_000.0).abs() < 1.0);
        assert_eq!(
            cashflows[0].period_start,
            Date::from_calendar_date(2024, Month::January, 1).expect("valid")
        );

        for cf in &cashflows {
            assert!(cf.interest > 0.0);
            assert!(cf.total > 0.0);
        }

        for i in 1..cashflows.len() {
            assert!(cashflows[i].beginning_balance <= cashflows[i - 1].beginning_balance);
        }
    }

    /// Item 12 regression: monthly investor interest must use the pool's
    /// day-count fraction over the full calendar accrual month, not a flat
    /// `rate / 12`.
    ///
    /// Under Act/360 a 31-day accrual month accrues `31/360` of the annual
    /// pass-through rate (≈ 0.08611), materially larger than the flat `1/12`
    /// (≈ 0.08333). This test builds an Act/360 pool and checks the first
    /// month's interest matches the day-counted accrual over the full month
    /// `[1st, 1st-of-next-month)`, not `rate/12`.
    #[test]
    fn monthly_interest_uses_pool_day_count_not_flat_twelfth() {
        let mut mbs = create_test_mbs();
        mbs.day_count = DayCount::Act360;
        // January 2024 — a 31-day accrual month.
        let as_of = Date::from_calendar_date(2024, Month::January, 1).expect("valid");

        let cashflows = generate_cashflows(&mbs, as_of, Some(1)).expect("should generate");
        let first = cashflows.first().expect("at least one cashflow");

        let balance = first.beginning_balance;
        // The accrual month spans [period_start, first-of-next-month) = 31 days.
        let accrual_end = Date::from_calendar_date(2024, Month::February, 1).expect("valid");
        let expected_yf = DayCount::Act360
            .year_fraction(first.period_start, accrual_end, DayCountContext::default())
            .expect("yf");
        let expected_interest = balance * mbs.pass_through_rate * expected_yf;
        let flat_twelfth = balance * mbs.pass_through_rate / 12.0;

        assert!(
            (first.interest - expected_interest).abs() < 1e-6,
            "interest {} should equal day-counted accrual {expected_interest} (31/360)",
            first.interest
        );
        // The day-counted value must differ from the flat 1/12 — otherwise the
        // test is not actually exercising the day-count path.
        assert!(
            (expected_interest - flat_twelfth).abs() > 1.0,
            "Act/360 31-day month accrual should differ from flat rate/12, \
             got day-counted {expected_interest} vs flat {flat_twelfth}"
        );
    }

    /// Finding 15 regression: `wam` is the *remaining* WAM at the valuation
    /// date. A seasoned pool (issued 60 months before `as_of`) with a
    /// remaining WAM of 240 must amortize over 240 projected months — not
    /// `wam − seasoning = 180`, which double-counts the age already netted
    /// out of a remaining WAM. This matches the MC-OAS projection horizon
    /// (`wam − projection_step`).
    #[test]
    fn seasoned_pool_amortizes_over_remaining_wam_not_wam_minus_seasoning() {
        let wam: u32 = 240;
        let mbs = AgencyMbsPassthrough::builder()
            .id(InstrumentId::new("SEASONED-MBS"))
            .pool_id("SEASONED-POOL".into())
            .agency(super::super::AgencyProgram::Fnma)
            .pool_type(super::super::PoolType::Generic)
            .original_face(Money::new(1_000_000.0, Currency::USD))
            .current_face(Money::new(850_000.0, Currency::USD))
            .current_factor(0.85)
            .wac(0.045)
            .pass_through_rate(0.04)
            .servicing_fee_rate(0.0025)
            .guarantee_fee_rate(0.0025)
            .wam(wam)
            // Issued 5 years (60 months) before the valuation date.
            .issue_date(Date::from_calendar_date(2019, Month::January, 1).expect("valid"))
            .maturity(Date::from_calendar_date(2044, Month::January, 1).expect("valid"))
            // Zero prepayment so the payoff horizon is purely scheduled
            // level-pay amortization.
            .prepayment_model(PrepaymentModelSpec::constant_cpr(0.0))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Thirty360)
            .build()
            .expect("valid mbs");

        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
        assert!(
            mbs.seasoning_months(as_of) >= 59,
            "fixture must be seasoned"
        );

        let cashflows = generate_cashflows(&mbs, as_of, Some(wam + 12)).expect("should generate");

        assert_eq!(
            cashflows.len(),
            wam as usize,
            "zero-prepay pool with remaining WAM {wam} must amortize over exactly {wam} months"
        );
        let last = cashflows.last().expect("non-empty");
        assert!(
            last.ending_balance.abs() < 0.01,
            "pool must be fully paid off at the remaining WAM, residual {}",
            last.ending_balance
        );
    }

    #[test]
    fn test_projected_schedule_preserves_classified_rows() {
        let mbs = create_test_mbs();
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
        let schedule = build_projected_schedule(&mbs, as_of, Some(3))
            .expect("projected schedule should build");

        assert!(!schedule.flows.is_empty());
        assert!(schedule.flows.iter().any(|cf| cf.kind == CFKind::Fixed));
        assert!(schedule
            .flows
            .iter()
            .any(|cf| matches!(cf.kind, CFKind::Amortization | CFKind::PrePayment)));
    }

    #[test]
    fn test_forward_start_no_cashflows_before_issue() {
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
        let future_issue = Date::from_calendar_date(2024, Month::March, 20).expect("valid");

        let mbs = AgencyMbsPassthrough::builder()
            .id(InstrumentId::new("FORWARD-POOL"))
            .pool_id("FWD-POOL".into())
            .agency(super::super::AgencyProgram::Fnma)
            .pool_type(super::super::PoolType::Generic)
            .original_face(Money::new(1_000_000.0, Currency::USD))
            .current_face(Money::new(1_000_000.0, Currency::USD))
            .current_factor(1.0)
            .wac(0.045)
            .pass_through_rate(0.04)
            .servicing_fee_rate(0.0025)
            .guarantee_fee_rate(0.0025)
            .wam(360)
            .issue_date(future_issue)
            .maturity(Date::from_calendar_date(2054, Month::March, 20).expect("valid"))
            .prepayment_model(PrepaymentModelSpec::psa(1.0))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Thirty360)
            .build()
            .expect("valid mbs");

        let cashflows = generate_cashflows(&mbs, as_of, Some(12)).expect("should generate");

        for cf in &cashflows {
            assert!(
                cf.period_start >= Date::from_calendar_date(2024, Month::April, 1).expect("valid"),
                "Forward-start pool should not generate cashflows before issue_date; got {}",
                cf.period_start
            );
        }
    }

    #[test]
    fn test_payment_delay_in_cashflows() {
        let mbs = create_test_mbs();
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");

        let cashflows = generate_cashflows(&mbs, as_of, Some(3)).expect("should generate");

        for cf in &cashflows {
            let expected_payment = mbs
                .payment_date_for_accrual_period(cf.period_start)
                .expect("payment date should resolve");
            assert_eq!(cf.payment_date, expected_payment);
        }
    }

    #[test]
    fn test_price_mbs() {
        let mbs = create_test_mbs();
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
        let market = create_test_market(as_of);

        let pv = price_mbs(&mbs, &market, as_of).expect("should price");

        assert!(pv.amount() > 0.0);
        assert!(pv.amount() > 500_000.0);
        assert!(pv.amount() < 1_500_000.0);
    }

    #[test]
    fn test_price_with_spread() {
        let mbs = create_test_mbs();
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
        let market = create_test_market(as_of);

        let pv_base = price_with_spread(&mbs, &market, as_of, 0.0).expect("should price");
        let pv_spread = price_with_spread(&mbs, &market, as_of, 0.01).expect("should price");

        assert!(pv_spread < pv_base);
    }

    #[test]
    fn test_prepayment_impact() {
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
        let market = create_test_market(as_of);

        let mut mbs_slow = create_test_mbs();
        mbs_slow.prepayment_model = PrepaymentModelSpec::psa(0.5);

        let mut mbs_fast = create_test_mbs();
        mbs_fast.prepayment_model = PrepaymentModelSpec::psa(2.0);

        let pv_slow = price_mbs(&mbs_slow, &market, as_of).expect("should price");
        let pv_fast = price_mbs(&mbs_fast, &market, as_of).expect("should price");

        assert!((pv_slow.amount() - pv_fast.amount()).abs() > 1.0);
    }

    /// Regression test for the SMM-clamp removal at the MBS-pricer layer.
    ///
    /// `PrepaymentModelSpec::smm` now treats 100% CPR as 100% SMM and rejects
    /// values above 100%. This test pins the SMM that arrives at
    /// `generate_cashflows` so the pricer layer does not reintroduce its own
    /// clamp.
    #[test]
    fn smm_at_max_cpr_passes_through_pricer_unchanged() {
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
        let mut mbs = create_test_mbs();
        mbs.prepayment_model = PrepaymentModelSpec::constant_cpr(1.0);

        let expected_smm = 1.0;
        let cashflows = generate_cashflows(&mbs, as_of, Some(1)).expect("should generate");
        let first = cashflows.first().expect("at least one cashflow");

        // Pricer must propagate exactly what cpr_to_smm produced (no extra clamp).
        assert!(
            (first.smm - expected_smm).abs() < 1e-12,
            "SMM must reach the pricer unclamped: expected {expected_smm}, got {}",
            first.smm
        );
    }
}
