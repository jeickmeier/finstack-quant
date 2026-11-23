//! Preparation and bucketing of contractual instrument schedules onto legal periods.

use super::super::exercise::{ExerciseTerms, ExerciseWindow, worst_call_period};
use super::super::state::WRITEDOWN_DE_MINIMIS;
use super::super::*;
use super::types::{
    CollateralKind, FeeComponent, FeeKind, InstrumentSchedule, InterestComponent, MarginTerms,
    NameState, PeriodBucket, PeriodDefaultSource, RevolverTerms, SpreadTerms, UtilizationTerms,
};
use crate::structured_credit::types::StructuredCredit;
use crate::structured_credit::types::{
    CallExercisePolicy, CollateralInstrument, InstrumentCollateral, PutExercisePolicy,
};
use finstack_quant_cashflows::CashflowScheduleSource;
use finstack_quant_cashflows::builder::RateSpec;
use finstack_quant_cashflows::builder::{CashFlowSchedule, FloatingRateParams};
use finstack_quant_core::Result;
use finstack_quant_core::cashflow::{CFKind, CashFlow};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::units::BASIS_POINTS_PER_UNIT;
use finstack_quant_models::monte_carlo::process::cir::CirProcess;
use finstack_quant_valuations_fixed_income::bond::CashflowSpec;
use finstack_quant_valuations_fixed_income::revolving_credit::{
    CreditSpreadProcessSpec, DEFAULT_CREDIT_SPREAD_IMPLIED_VOL, DEFAULT_UTIL_CREDIT_CORR,
    DrawRepaySpec, McConfig, RevolvingCredit, StochasticUtilizationSpec, UtilizationProcess,
    build_credit_spread_params,
};
use rust_decimal::prelude::ToPrimitive;
use std::sync::Arc;

/// Bucketed instrument schedules prepared once per valuation and shared by
/// every path.
pub(crate) struct PreparedInstrumentSchedules {
    /// One schedule per pool row, in pool order.
    pub(in super::super) schedules: Vec<InstrumentSchedule>,
    /// Valuation date the schedules were projected at; fixings on or before
    /// it are observed and never re-projected.
    pub(in super::super) as_of: Date,
    /// Whether any instrument uses the refinancing-incentive call rule, which
    /// needs the deal discount curve each period.
    pub(in super::super) needs_discount_curve: bool,
}

impl PreparedInstrumentSchedules {
    /// Bucket every instrument's contractual schedule onto the prepared legal
    /// periods and resolve exercise, default and revolver terms.
    ///
    /// # Arguments
    ///
    /// * `instrument` - Resolved deal whose pool carries instrument collateral.
    /// * `context` - Market context used to project each instrument's schedule.
    /// * `prepared` - Prepared simulation supplying the valuation date and periods.
    pub(crate) fn prepare(
        instrument: &StructuredCredit,
        context: &MarketContext,
        prepared: &PreparedDealSimulation,
    ) -> Result<Self> {
        let collateral = instrument.pool.instruments.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "instrument-schedule pool flows require instrument collateral".into(),
            )
        })?;
        let as_of = prepared.valuation_date;
        let period_dates: Vec<Date> = prepared.periods.iter().map(|p| p.payment_date).collect();
        if period_dates.is_empty() {
            return Err(finstack_quant_core::Error::Validation(
                "instrument-schedule pool flows require at least one future payment period".into(),
            ));
        }

        let mut schedules = Vec::with_capacity(collateral.len());
        for held in collateral.iter() {
            schedules.push(build_schedule(
                held,
                collateral,
                context,
                as_of,
                &period_dates,
            )?);
        }
        // The incentive exercise rules and the margin of a fixed-rate
        // revolver with simulated draws all read the deal discount curve.
        let needs_discount_curve = schedules.iter().any(|s| {
            matches!(
                s.exercise.call_policy,
                CallExercisePolicy::RefinancingIncentive { .. }
            ) || matches!(
                s.exercise.put_policy,
                PutExercisePolicy::ReinvestmentIncentive { .. }
            ) || s.revolver.as_ref().is_some_and(|r| {
                r.utilization.is_some()
                    && r.margins
                        .iter()
                        .any(|m| matches!(m, MarginTerms::FixedRate(_)))
            })
        });
        Ok(Self {
            schedules,
            as_of,
            needs_discount_curve,
        })
    }

    /// Whether any revolver has simulated draws to value against its path's
    /// fair spread.
    pub(crate) fn has_draw_option_cost(&self) -> bool {
        self.schedules
            .iter()
            .any(|s| s.revolver.as_ref().is_some_and(|r| r.utilization.is_some()))
    }

    /// Fresh per-name states for one run.
    pub(in super::super) fn initial_names(&self) -> Vec<NameState> {
        self.schedules
            .iter()
            .map(|schedule| {
                let (utilization, spread) = match &schedule.revolver {
                    Some(terms) => (
                        if terms.commitment_at(0) > 0.0 {
                            (schedule.opening_balance / terms.commitment_at(0)).clamp(0.0, 1.0)
                        } else {
                            0.0
                        },
                        terms
                            .utilization
                            .as_ref()
                            .map_or(0.0, |u| u.spread.initial()),
                    ),
                    None => (0.0, 0.0),
                };
                NameState {
                    scale: 1.0,
                    retired: false,
                    utilization,
                    spread,
                    period_draw: 0.0,
                }
            })
            .collect()
    }

    /// The deal discount curve when any instrument's exercise rule needs it.
    pub(in super::super) fn discount_curve(
        &self,
        instrument: &StructuredCredit,
        context: &MarketContext,
    ) -> Result<Option<Arc<DiscountCurve>>> {
        if self.needs_discount_curve {
            Ok(Some(
                context.get_discount(instrument.discount_curve_id.as_str())?,
            ))
        } else {
            Ok(None)
        }
    }

    /// Verify the contractual draw calendar can be funded period by period,
    /// so a deterministic run never silently caps a draw.
    ///
    /// # Arguments
    ///
    /// * `reserve` - Opening reserve balance.
    /// * `target` - Reserve target that revolver repayments replenish toward.
    /// * `period_dates` - Payment date of each legal period.
    pub(in super::super) fn check_deterministic_funding(
        &self,
        reserve: f64,
        target: Option<f64>,
        period_dates: &[Date],
    ) -> Result<()> {
        let mut reserve = reserve.max(0.0);
        for (k, pay_date) in period_dates.iter().enumerate() {
            let mut requested = 0.0;
            let mut collections = 0.0;
            let mut revolver_repayments = 0.0;
            let mut drawing: Vec<&str> = Vec::new();
            for schedule in &self.schedules {
                let Some(bucket) = schedule.periods.get(k) else {
                    continue;
                };
                if bucket.draw > 0.0 {
                    drawing.push(schedule.id.as_str());
                }
                requested += bucket.draw;
                collections += bucket.repayment;
                if schedule.kind == CollateralKind::Revolver {
                    revolver_repayments += bucket.repayment;
                }
            }
            let available = reserve + collections;
            if requested > available + WRITEDOWN_DE_MINIMIS {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "collateral draws of {requested:.2} on {pay_date} by [{}] exceed the reserve \
                     account ({reserve:.2}) plus that period's principal collections \
                     ({collections:.2}); fund the reserve or reduce the commitments",
                    drawing.join(", ")
                )));
            }
            let from_reserve = requested.min(reserve);
            reserve -= from_reserve;
            let from_principal = requested - from_reserve;
            if let Some(target) = target {
                let room = (collections - from_principal).max(0.0);
                let top_up = revolver_repayments
                    .min(room)
                    .min((target - reserve).max(0.0));
                reserve += top_up;
            }
        }
        Ok(())
    }
}

/// Bucket one instrument's schedule and resolve its terms.
fn build_schedule(
    held: CollateralInstrument<'_>,
    collateral: &InstrumentCollateral,
    context: &MarketContext,
    as_of: Date,
    period_dates: &[Date],
) -> Result<InstrumentSchedule> {
    let id = held.id().clone();
    let (kind, schedule, opening_balance, maturity) = match held {
        CollateralInstrument::Bond(bond) => {
            let schedule = bond.raw_cashflow_schedule(context, as_of)?;
            let opening = bond.notional.amount() + past_principal_delta(&schedule, as_of, false)?;
            (
                CollateralKind::Bond,
                schedule,
                opening.max(0.0),
                bond.maturity,
            )
        }
        CollateralInstrument::TermLoan(loan) => {
            let schedule = loan.raw_cashflow_schedule(context, as_of)?;
            let opening = past_principal_delta(&schedule, as_of, true)?;
            (
                CollateralKind::TermLoan,
                schedule,
                opening.max(0.0),
                loan.maturity,
            )
        }
        CollateralInstrument::Revolver(facility) => {
            let schedule = facility.raw_cashflow_schedule(context, as_of)?;
            // `drawn` is the balance at the simulation anchor in both
            // modes; deterministic events are future-only.
            let opening = facility.drawn.amount();
            (
                CollateralKind::Revolver,
                schedule,
                opening,
                facility.maturity,
            )
        }
    };

    let periods = bucket_flows(&schedule, as_of, period_dates)?;

    // Exercise terms.
    let (calls, puts): (Vec<ExerciseWindow>, Vec<ExerciseWindow>) = match held {
        CollateralInstrument::Bond(bond) => match &bond.call_put {
            Some(schedule) => (
                schedule
                    .calls
                    .iter()
                    .map(|c| ExerciseWindow {
                        start: c.start,
                        end: c.end,
                        price_pct: c.price_pct_of_par,
                    })
                    .collect(),
                schedule
                    .puts
                    .iter()
                    .map(|p| ExerciseWindow {
                        start: p.start,
                        end: p.end,
                        price_pct: p.price_pct_of_par,
                    })
                    .collect(),
            ),
            None => (Vec::new(), Vec::new()),
        },
        CollateralInstrument::TermLoan(loan) => (
            loan.call_schedule
                .as_ref()
                .map(|s| {
                    s.calls
                        .iter()
                        .map(|c| ExerciseWindow {
                            start: c.date,
                            end: loan.maturity,
                            price_pct: c.price_pct_of_par,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            Vec::new(),
        ),
        CollateralInstrument::Revolver(_) => (Vec::new(), Vec::new()),
    };
    let fixed_coupon = match (held, held.loan_rate()) {
        (CollateralInstrument::Bond(bond), _) => fixed_coupon_of(&bond.cashflow_spec)?,
        (_, Some(RateSpec::Fixed { rate })) => Some(*rate),
        (_, _) => None,
    };
    let call_policy = collateral.call_policy_for(&id);
    let put_policy = collateral.put_policy_for(&id);
    let worst_period = if call_policy == CallExercisePolicy::Worst && !calls.is_empty() {
        let price = held.quoted_clean_price_pct().ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "call policy 'worst' requires a quoted clean price on instrument '{id}'"
            ))
        })?;
        let mut cash_by_period = Vec::with_capacity(periods.len());
        let mut balance_by_period = Vec::with_capacity(periods.len());
        let mut running = opening_balance;
        for bucket in &periods {
            cash_by_period.push(bucket.interest() + bucket.fees() + bucket.repayment);
            running = (running - bucket.repayment + bucket.draw + bucket.pik).max(0.0);
            balance_by_period.push(running);
        }
        worst_call_period(
            as_of,
            price,
            opening_balance,
            period_dates,
            &cash_by_period,
            &balance_by_period,
            &calls,
        )?
    } else {
        None
    };
    let exercise = ExerciseTerms {
        calls,
        puts,
        call_policy,
        put_policy,
        fixed_coupon,
        maturity,
        worst_period,
    };

    let default_source = held
        .credit_curve_id()
        .map_or(PeriodDefaultSource::DealModel, |c| {
            PeriodDefaultSource::HazardCurve(c.to_string())
        });
    let recovery_rate = match held {
        CollateralInstrument::Revolver(facility) => Some(facility.recovery_rate),
        CollateralInstrument::Bond(_) | CollateralInstrument::TermLoan(_) => None,
    };
    let floating_params = match (held, held.loan_rate()) {
        (CollateralInstrument::Bond(bond), _) => floating_params_of(&bond.cashflow_spec)?,
        (_, Some(RateSpec::Floating(spec))) => Some(FloatingRateParams::try_from(spec)?),
        (_, _) => None,
    };
    let revolver = match held {
        CollateralInstrument::Revolver(facility) => Some(RevolverTerms {
            commitments: (0..period_dates.len())
                .map(|k| {
                    let start = if k == 0 { as_of } else { period_dates[k - 1] };
                    facility.commitment_at(start).amount()
                })
                .collect(),
            leq: facility.leq,
            lc_exposure: (0..period_dates.len())
                .map(|k| {
                    let start = if k == 0 { as_of } else { period_dates[k - 1] };
                    facility.lc_exposure_at_default(start)
                })
                .collect(),
            lc_outstanding: (0..period_dates.len())
                .map(|k| {
                    let start = if k == 0 { as_of } else { period_dates[k - 1] };
                    facility.lc_outstanding_at(start).amount()
                })
                .collect(),
            day_count: facility.day_count,
            margins: (0..period_dates.len())
                .map(|k| {
                    let start = if k == 0 { as_of } else { period_dates[k - 1] };
                    let delta = facility.margin_delta_bp_at(start) / BASIS_POINTS_PER_UNIT;
                    match &facility.rate {
                        RateSpec::Fixed { rate } => MarginTerms::FixedRate(*rate + delta),
                        RateSpec::Floating(_) => MarginTerms::Spread,
                    }
                })
                .collect(),
            utilization: match &facility.draw_repay_spec {
                DrawRepaySpec::Deterministic(_) => None,
                DrawRepaySpec::Stochastic(spec) => {
                    Some(utilization_terms(facility, spec, context, as_of)?)
                }
            },
        }),
        CollateralInstrument::Bond(_) | CollateralInstrument::TermLoan(_) => None,
    };

    Ok(InstrumentSchedule {
        id: id.to_string(),
        kind,
        opening_balance,
        maturity,
        periods,
        exercise,
        default_source,
        recovery_rate,
        floating_params,
        revolver,
    })
}

/// Utilization and spread process terms of a stochastic revolver, mirroring
/// the standalone facility pricer's configuration (including its synthesized
/// market-anchored spread and utilization–credit correlation when the
/// facility carries a hazard curve but no explicit `McConfig`).
fn utilization_terms(
    facility: &RevolvingCredit,
    spec: &StochasticUtilizationSpec,
    context: &MarketContext,
    as_of: Date,
) -> Result<UtilizationTerms> {
    let UtilizationProcess::MeanReverting {
        theta,
        kappa,
        sigma,
        spread_sensitivity,
    } = &spec.utilization_process;
    let anchor = as_of.max(facility.issue_date);
    let synthesized;
    let mc_config: &McConfig = match &spec.mc_config {
        Some(config) => config,
        None => {
            let (credit_spread_process, util_credit_corr) = match &facility.credit_curve_id {
                Some(hazard_id) => (
                    CreditSpreadProcessSpec::MarketAnchored {
                        credit_curve_id: hazard_id.clone(),
                        kappa: 0.1,
                        implied_vol: DEFAULT_CREDIT_SPREAD_IMPLIED_VOL,
                        tenor_years: None,
                    },
                    Some(DEFAULT_UTIL_CREDIT_CORR),
                ),
                None => (CreditSpreadProcessSpec::Constant(0.0), None),
            };
            synthesized = McConfig {
                correlation_matrix: None,
                credit_spread_process,
                interest_rate_process: None,
                util_credit_corr,
            };
            &synthesized
        }
    };
    let spread = match &mc_config.credit_spread_process {
        CreditSpreadProcessSpec::Constant(spread) => SpreadTerms::Constant(spread.max(0.0)),
        CreditSpreadProcessSpec::Cir { .. } | CreditSpreadProcessSpec::MarketAnchored { .. } => {
            let params = build_credit_spread_params(mc_config, facility, context, anchor)?;
            SpreadTerms::Cir {
                process: CirProcess::new(params.cir),
                initial: params.initial,
            }
        }
    };
    let util_credit_corr = mc_config
        .correlation_matrix
        .map(|m| m[0][2])
        .or(mc_config.util_credit_corr)
        .unwrap_or(0.0)
        .clamp(-1.0, 1.0);
    Ok(UtilizationTerms {
        kappa: *kappa,
        theta: *theta,
        sigma: *sigma,
        spread_sensitivity: *spread_sensitivity,
        util_credit_corr,
        spread,
    })
}

/// Capitalized amount of a PIK row: its explicit principal delta, never its
/// cash amount (record CORE-CONVENTIONS-015 #1, #4).
fn pik_delta(cf: &finstack_quant_core::cashflow::CashFlow) -> Result<f64> {
    cf.get_principal_delta()
        .map(|delta| delta.amount())
        .ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "PIK row on {} has no principal_delta",
                cf.get_date()
            ))
        })
}

/// Net principal change implied by flows dated on or before `as_of`.
///
/// Principal kinds carry lender-perspective signs: draws and initial funding
/// are negative (balance up), repayments positive (balance down); PIK
/// capitalization raises the balance. `include_funding` replays the initial
/// funding flow (term loans); bonds start from their notional instead.
fn past_principal_delta(
    schedule: &CashFlowSchedule,
    as_of: Date,
    include_funding: bool,
) -> Result<f64> {
    let mut delta = 0.0;
    for cf in schedule.get_flows() {
        if cf.get_date() > as_of {
            continue;
        }
        match cf.get_kind() {
            CFKind::Notional
            | CFKind::Amortization
            | CFKind::PrePayment
            | CFKind::RevolvingDraw
            | CFKind::RevolvingRepayment => {
                // The explicit par change, never the cash (record
                // CORE-CONVENTIONS-015 #4); a positive delta is funding.
                let change = principal_delta_of(cf)?;
                if change > 0.0 && !include_funding {
                    continue;
                }
                delta += change;
            }
            CFKind::Pik => delta += pik_delta(cf)?,
            _ => {}
        }
    }
    Ok(delta)
}

/// Explicit principal delta of a principal row.
fn principal_delta_of(cf: &finstack_quant_core::cashflow::CashFlow) -> Result<f64> {
    cf.get_principal_delta()
        .map(|delta| delta.amount())
        .ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "{} row on {} has no principal_delta",
                cf.get_kind(),
                cf.get_date()
            ))
        })
}

/// Balance a rate-based flow accrued on: `amount / (rate × accrual)`.
fn accrual_basis(cf: &CashFlow) -> f64 {
    match cf.get_rate() {
        Some(rate) if rate.abs() > 1e-12 && cf.get_accrual_factor() > 0.0 => {
            cf.get_amount().amount() / (rate * cf.get_accrual_factor())
        }
        _ => 0.0,
    }
}

/// Bucket a schedule's future flows by legal period `(pay_{k-1}, pay_k]`,
/// with flows after the last payment date landing in the last bucket.
fn bucket_flows(
    schedule: &CashFlowSchedule,
    as_of: Date,
    period_dates: &[Date],
) -> Result<Vec<PeriodBucket>> {
    let mut periods: Vec<PeriodBucket> = (0..period_dates.len())
        .map(|_| PeriodBucket::default())
        .collect();
    let fixings = &schedule.get_meta().projected_fixings;
    for cf in schedule.get_flows() {
        if cf.get_date() <= as_of {
            continue;
        }
        let k = period_dates
            .iter()
            .position(|pay| *pay >= cf.get_date())
            .unwrap_or(period_dates.len() - 1);
        let bucket = &mut periods[k];
        let amount = cf.get_amount().amount();
        match cf.get_kind() {
            CFKind::Fixed | CFKind::InflationCoupon | CFKind::Stub => {
                bucket.interest.push(InterestComponent {
                    projected: amount,
                    basis: accrual_basis(cf),
                    accrual: cf.get_accrual_factor(),
                    index_rate: None,
                    fixing_date: None,
                    rate: cf.get_rate(),
                });
            }
            CFKind::Floating => {
                let index_rate = cf.get_reset_date().and_then(|reset| {
                    fixings
                        .iter()
                        .find(|f| f.date == reset)
                        .and_then(|f| f.value)
                });
                bucket.interest.push(InterestComponent {
                    projected: amount,
                    basis: accrual_basis(cf),
                    accrual: cf.get_accrual_factor(),
                    index_rate,
                    fixing_date: cf.get_reset_date(),
                    rate: cf.get_rate(),
                });
            }
            CFKind::Fee
            | CFKind::CommitmentFee
            | CFKind::UsageFee
            | CFKind::FacilityFee
            | CFKind::LcFee
            | CFKind::FrontingFee => {
                let kind = match cf.get_kind() {
                    CFKind::CommitmentFee => FeeKind::Commitment,
                    CFKind::UsageFee => FeeKind::Usage,
                    _ => FeeKind::Fixed,
                };
                bucket.fees.push(FeeComponent {
                    projected: amount,
                    basis: accrual_basis(cf),
                    accrual: cf.get_accrual_factor(),
                    rate: cf.get_rate(),
                    kind,
                });
            }
            CFKind::Notional
            | CFKind::Amortization
            | CFKind::PrePayment
            | CFKind::RevolvingDraw
            | CFKind::RevolvingRepayment => {
                if amount < 0.0 {
                    bucket.draw += -amount;
                } else {
                    bucket.repayment += amount;
                }
            }
            CFKind::Pik => bucket.pik += pik_delta(cf)?,
            _ => {}
        }
    }
    Ok(periods)
}

fn fixed_coupon_of(spec: &CashflowSpec) -> Result<Option<f64>> {
    let decimal = match spec {
        CashflowSpec::Fixed(fixed) => Some(fixed.rate),
        CashflowSpec::StepUp(step_up) => Some(step_up.initial_rate),
        CashflowSpec::Floating(_) => None,
        CashflowSpec::Amortizing { base, .. } => return fixed_coupon_of(base),
    };
    decimal
        .map(|d| {
            d.to_f64().ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "coupon {d} cannot be represented as f64"
                ))
            })
        })
        .transpose()
}

fn floating_params_of(spec: &CashflowSpec) -> Result<Option<FloatingRateParams>> {
    match spec {
        CashflowSpec::Floating(floating) => {
            Ok(Some(FloatingRateParams::try_from(&floating.rate_spec)?))
        }
        CashflowSpec::Amortizing { base, .. } => floating_params_of(base),
        CashflowSpec::Fixed(_) | CashflowSpec::StepUp(_) => Ok(None),
    }
}
