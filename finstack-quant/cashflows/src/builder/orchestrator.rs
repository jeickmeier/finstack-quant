//! `CashFlowBuilder` orchestration: validation, schedule compilation, and
//! emission into a [`CashFlowSchedule`].
//!
//! Fluent coupon and fee methods live in `coupon_api`; principal and
//! amortization methods live in `principal`.

use super::schedule::{finalize_flows, CashFlowSchedule};
use crate::builder::{AmortizationSpec, Notional};
use crate::primitives::{CFKind, CashFlow};
use finstack_quant_core::dates::Date;
use finstack_quant_core::decimal::{decimal_to_f64, f64_to_decimal};
use finstack_quant_core::market_data::fixings::fixing_series_id;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::InputError;
use rust_decimal::Decimal;
use std::sync::Arc;

use super::compiler::{
    build_fee_schedules, collect_dates, compute_coupon_schedules, CompiledSchedules,
    CouponProgramPiece, FixedSchedule, FloatSchedule, PaymentProgramPiece, PeriodicFee,
};
use super::pipeline::{BuildContext, DateProcessor};
use super::specs::FeeSpec;
use smallvec::SmallVec;
use tracing::debug;

/// Internal state accumulated during schedule building.
#[derive(Debug, Clone)]
pub(super) struct BuildState {
    pub(super) flows: Vec<CashFlow>,
    pub(super) outstanding_after: finstack_quant_core::HashMap<Date, Decimal>,
    /// Outstanding balance tracked as `Decimal` for accounting-grade precision.
    ///
    /// Using `Decimal` eliminates f64 accumulation drift that can exceed 1 bp
    /// relative error on very long-dated instruments with many small cashflows
    /// (e.g., 600+ period amortizers). Converted to f64 only at API boundaries
    /// when passing to emission functions that operate in f64 space.
    pub(super) outstanding: Decimal,
}

/// Principal event applied during schedule build (draws/repays).
///
/// `delta` adjusts outstanding (positive increases, negative decreases).
/// `cash` represents the cash leg (e.g., net of OID/fees). If `delta` differs
/// from `cash`, the difference is interpreted as non-cash adjustments.
/// `kind` classifies the emitted cashflow. `CFKind::Amortization` emits a
/// positive principal-repayment cashflow; notional-like events emit as negative
/// borrower draw cashflows. In all cases, `delta` remains the source of truth
/// for outstanding balance movement.
#[derive(Debug, Clone)]
pub struct PrincipalEvent {
    /// Event date
    pub date: Date,
    /// Outstanding delta (positive = increases balance, negative = repays)
    pub delta: Money,
    /// Cash leg paid/received (may differ from delta for OID/fees)
    pub cash: Money,
    /// Classification for emitted cashflow
    pub kind: CFKind,
}

#[derive(Debug, Clone)]
pub(super) struct AmortizationSetup {
    pub(super) amort_dates: finstack_quant_core::HashSet<Date>,
    pub(super) step_remaining_map: Option<finstack_quant_core::HashMap<Date, Money>>, // for StepRemaining
    pub(super) custom_principal_map: Option<finstack_quant_core::HashMap<Date, Money>>,
    pub(super) linear_delta: Option<Decimal>, // for LinearTo
    pub(super) percent_per: Option<Decimal>,  // for PercentOfOriginalPerPeriod
}

/// Grouped inputs for collecting all relevant schedule dates.
#[derive(Clone, Copy)]
struct DateCollectionInputs<'a> {
    issue: Date,
    maturity: Date,
    fixed_schedules: &'a [FixedSchedule],
    float_schedules: &'a [FloatSchedule],
    periodic_fees: &'a [PeriodicFee],
    fixed_fees: &'a [(Date, Money)],
    notional: &'a Notional,
    principal_events: &'a [PrincipalEvent],
}

fn validate_core_inputs(
    b: &CashFlowBuilder,
) -> finstack_quant_core::Result<(Notional, Date, Date)> {
    let notional = b.notional.clone().ok_or_else(|| InputError::NotFound {
        id: "notional (call principal() first)".into(),
    })?;
    let issue = b.issue.ok_or_else(|| InputError::NotFound {
        id: "issue date (call principal() first)".into(),
    })?;
    let maturity = b.maturity.ok_or_else(|| InputError::NotFound {
        id: "maturity date (call principal() first)".into(),
    })?;

    if issue >= maturity {
        return Err(finstack_quant_core::Error::Validation(format!(
            "issue date {} must be strictly before maturity date {}",
            issue, maturity
        )));
    }

    // Validate notional and amortization spec (e.g., total amortization <= notional)
    notional.validate()?;

    let out_of_range = match &notional.amort {
        AmortizationSpec::StepRemaining { schedule } => schedule
            .iter()
            .find(|(date, _)| *date < issue || *date > maturity)
            .map(|(date, _)| *date),
        AmortizationSpec::CustomPrincipal { items } => items
            .iter()
            .find(|(date, _)| *date < issue || *date > maturity)
            .map(|(date, _)| *date),
        _ => None,
    };
    if let Some(date) = out_of_range {
        return Err(InputError::DateOutOfRange {
            date,
            range: (issue, maturity),
        }
        .into());
    }

    Ok((notional, issue, maturity))
}

fn derive_amortization_setup(
    notional: &Notional,
    fixed_schedules: &[FixedSchedule],
    float_schedules: &[FloatSchedule],
) -> finstack_quant_core::Result<AmortizationSetup> {
    // Determine base cadence schedule for linear/percent amortization by
    // borrowing the first available coupon leg. For multi-leg instruments with
    // differing frequencies, amortization follows this first leg's cadence.
    let amort_base: Option<&[Date]> = match notional.amort {
        AmortizationSpec::LinearTo { .. } | AmortizationSpec::PercentOfOriginalPerPeriod { .. } => {
            if let Some(schedule) = fixed_schedules.first() {
                Some(schedule.dates.as_slice())
            } else if let Some(schedule) = float_schedules.first() {
                Some(schedule.dates.as_slice())
            } else {
                None
            }
        }
        _ => None,
    };

    if amort_base.is_none()
        && matches!(
            notional.amort,
            AmortizationSpec::LinearTo { .. } | AmortizationSpec::PercentOfOriginalPerPeriod { .. }
        )
    {
        return Err(InputError::Invalid.into());
    }

    // Precompute helpers depending on amort spec
    let step_remaining_map: Option<finstack_quant_core::HashMap<Date, Money>> =
        match &notional.amort {
            AmortizationSpec::StepRemaining { schedule } => {
                let mut m = finstack_quant_core::HashMap::default();
                m.reserve(schedule.len());
                for (d, mny) in schedule {
                    m.insert(*d, *mny);
                }
                Some(m)
            }
            _ => None,
        };

    let custom_principal_map: Option<finstack_quant_core::HashMap<Date, Money>> =
        match &notional.amort {
            AmortizationSpec::CustomPrincipal { items } => {
                let mut m = finstack_quant_core::HashMap::default();
                m.reserve(items.len());
                for (d, mny) in items {
                    if mny.amount() > 0.0 {
                        m.entry(*d)
                            .and_modify(|existing| *existing += *mny)
                            .or_insert(*mny);
                    }
                }
                Some(m)
            }
            _ => None,
        };

    let (linear_delta, percent_per) = match &notional.amort {
        AmortizationSpec::LinearTo { final_notional } => {
            let base = amort_base.ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "amortization_base_schedule".to_string(),
                })
            })?;
            let steps = Decimal::from(base.len() as u64);
            let initial = f64_to_decimal(notional.initial.amount())?;
            let final_notional = f64_to_decimal(final_notional.amount())?;
            let delta = (initial - final_notional) / steps;
            (
                Some(if delta > Decimal::ZERO {
                    delta
                } else {
                    Decimal::ZERO
                }),
                None,
            )
        }
        AmortizationSpec::PercentOfOriginalPerPeriod { pct } => {
            let initial = f64_to_decimal(notional.initial.amount())?;
            let pct = f64_to_decimal(*pct)?;
            let per = initial * pct;
            (
                None,
                Some(if per > Decimal::ZERO {
                    per
                } else {
                    Decimal::ZERO
                }),
            )
        }
        _ => (None, None),
    };

    let amort_dates: finstack_quant_core::HashSet<Date> = amort_base
        .map(|v| v.iter().copied().collect())
        .unwrap_or_default();

    Ok(AmortizationSetup {
        amort_dates,
        step_remaining_map,
        custom_principal_map,
        linear_delta,
        percent_per,
    })
}

fn initialize_build_state(
    issue: Date,
    notional: &Notional,
    estimated_dates: usize,
    principal_events: &[PrincipalEvent],
) -> finstack_quant_core::Result<BuildState> {
    let estimated_flows = estimated_dates * 3;
    let mut flows: Vec<CashFlow> = Vec::with_capacity(estimated_flows);

    if notional.initial.amount() != 0.0 {
        flows.push(CashFlow::new(
            issue,
            None,
            notional.initial * -1.0,
            CFKind::Notional,
            0.0,
            None,
        ));
    }

    let mut outstanding = f64_to_decimal(notional.initial.amount())?;

    for ev in principal_events.iter().filter(|ev| ev.date <= issue) {
        if ev.delta.amount() != 0.0 || ev.cash.amount() != 0.0 {
            // Sign convention depends on flow kind:
            // - Notional (draws): cash is inflow to borrower, flow is negative (funding outflow from lender)
            // - Amortization: cash is repayment, flow is positive (inflow to lender)
            let flow_amount = match ev.kind {
                CFKind::Amortization => ev.cash.amount(),
                _ => -ev.cash.amount(),
            };
            flows.push(CashFlow::new(
                ev.date,
                None,
                Money::new(flow_amount, ev.cash.currency()),
                ev.kind,
                0.0,
                None,
            ));
            outstanding += f64_to_decimal(ev.delta.amount())?;
        }
    }

    let mut outstanding_after: finstack_quant_core::HashMap<Date, Decimal> =
        finstack_quant_core::HashMap::default();
    outstanding_after.reserve(estimated_dates);
    outstanding_after.insert(issue, outstanding);

    Ok(BuildState {
        flows,
        outstanding_after,
        outstanding,
    })
}

fn collect_all_dates(inputs: &DateCollectionInputs<'_>) -> finstack_quant_core::Result<Vec<Date>> {
    let periodic_date_slices: Vec<&[Date]> = inputs
        .periodic_fees
        .iter()
        .map(|pf| pf.dates.as_slice())
        .collect();
    let mut dates: Vec<Date> = collect_dates(
        inputs.issue,
        inputs.maturity,
        inputs.fixed_schedules,
        inputs.float_schedules,
        &periodic_date_slices,
        inputs.fixed_fees,
        inputs.notional,
    );
    for pf in inputs.periodic_fees {
        for period in pf.prev.values() {
            dates.push(period.accrual_start);
            dates.push(period.accrual_end);
        }
    }
    for ev in inputs.principal_events {
        dates.push(ev.date);
    }
    dates.sort_unstable();
    dates.dedup();
    if dates.len() < 2 {
        return Err(InputError::TooFewPoints.into());
    }
    Ok(dates)
}

/// Builder for constructing cashflow schedules with validation.
///
/// Fluent methods are split across `principal` (notional and amortization) and
/// `coupon_api` (coupons, fees, payment splits). Build orchestration lives in
/// this module.
#[derive(Debug, Clone)]
pub struct CashFlowBuilder {
    pub(super) notional: Option<Notional>,
    pub(super) amortization: Option<AmortizationSpec>,
    pub(super) issue: Option<Date>,
    pub(super) maturity: Option<Date>,
    /// Fee specifications. SmallVec<4> avoids heap allocation for typical instruments
    /// with ≤4 fee specs (commitment fee, facility fee, usage fee, admin fee).
    pub(super) fees: SmallVec<[FeeSpec; 4]>,
    pub(super) principal_events: Vec<PrincipalEvent>,
    // Segmented programs (optional): coupon program and payment/PIK program
    pub(super) coupon_program: Vec<CouponProgramPiece>,
    pub(super) payment_program: Vec<PaymentProgramPiece>,
    // Sticky builder error for fluent APIs that cannot return Result.
    pub(super) pending_error: Option<finstack_quant_core::Error>,
}

impl Default for CashFlowBuilder {
    fn default() -> Self {
        Self {
            notional: None,
            amortization: None,
            issue: None,
            maturity: None,
            fees: SmallVec::new(),
            principal_events: Vec::new(),
            coupon_program: Vec::new(),
            payment_program: Vec::new(),
            pending_error: None,
        }
    }
}

#[derive(Clone)]
struct CompiledCashFlowPlan {
    notional: Notional,
    issue: Date,
    maturity: Date,
    fixed_schedules: Vec<FixedSchedule>,
    float_schedules: Vec<FloatSchedule>,
    periodic_fees: Vec<PeriodicFee>,
    fixed_fees: Vec<(Date, Money)>,
    principal_events: Vec<PrincipalEvent>,
    dates: Vec<Date>,
    amort_setup: AmortizationSetup,
}

impl CashFlowBuilder {
    /// Build the cashflow schedule with optional market curves for floating rate projection.
    ///
    /// When curves are provided, floating rate coupons use forward rates:
    /// `coupon = outstanding * (forward_rate * gearing + margin_bp * 1e-4) * year_fraction`
    ///
    /// Without curves, the fallback policy on each floating spec controls behavior
    /// (default: error; `SpreadOnly` uses just margin; `FixedRate(r)` uses a fixed index).
    ///
    /// The builder compiles dates, coupon programs, fees, and principal events
    /// into a canonical ordered [`CashFlowSchedule`]. `curves` is used only for
    /// floating-rate projection; fixed coupons and deterministic fees do not
    /// require a market context.
    ///
    /// # Errors
    ///
    /// Returns any deferred builder-configuration error, invalid core input or
    /// schedule/coupon/fee/principal-event error, or a floating-rate projection
    /// failure. In particular, a floating spec with the default error fallback
    /// fails when its required market data is unavailable.
    ///
    pub fn build(
        &self,
        curves: Option<&finstack_quant_core::market_data::context::MarketContext>,
    ) -> finstack_quant_core::Result<CashFlowSchedule> {
        self.compile_plan()?.project(curves)
    }

    fn compile_plan(&self) -> finstack_quant_core::Result<CompiledCashFlowPlan> {
        if let Some(err) = &self.pending_error {
            return Err(err.clone());
        }
        let (notional, issue, maturity) = validate_core_inputs(self)?;

        let (
            CompiledSchedules {
                fixed_schedules,
                float_schedules,
            },
            periodic_fees,
            fixed_fees,
        ) = {
            let compiled = compute_coupon_schedules(self, issue, maturity)?;
            let (periodic_fees, fixed_fees) = build_fee_schedules(issue, maturity, &self.fees)?;
            (compiled, periodic_fees, fixed_fees)
        };

        let mut principal_events = self.principal_events.clone();
        principal_events.sort_by_key(|ev| ev.date);

        // Reject principal events with currency different from notional.
        let expected_ccy = notional.initial.currency();
        if let Some(ev) = principal_events
            .iter()
            .find(|ev| ev.delta.currency() != expected_ccy)
        {
            return Err(finstack_quant_core::Error::CurrencyMismatch {
                expected: expected_ccy,
                actual: ev.delta.currency(),
            });
        }

        if let Some(ev) = principal_events.iter().find(|ev| ev.date > maturity) {
            return Err(InputError::DateOutOfRange {
                date: ev.date,
                range: (issue, maturity),
            }
            .into());
        }

        let date_inputs = DateCollectionInputs {
            issue,
            maturity,
            fixed_schedules: &fixed_schedules,
            float_schedules: &float_schedules,
            periodic_fees: &periodic_fees,
            fixed_fees: &fixed_fees,
            notional: &notional,
            principal_events: &principal_events,
        };
        let dates = collect_all_dates(&date_inputs)?;
        debug!(dates = dates.len(), %issue, %maturity, "cashflow schedule: dates collected");

        let amort_setup = derive_amortization_setup(&notional, &fixed_schedules, &float_schedules)?;

        Ok(CompiledCashFlowPlan {
            notional,
            issue,
            maturity,
            fixed_schedules,
            float_schedules,
            periodic_fees,
            fixed_fees,
            principal_events,
            dates,
            amort_setup,
        })
    }
}

impl CompiledCashFlowPlan {
    fn project(
        &self,
        curves: Option<&finstack_quant_core::market_data::context::MarketContext>,
    ) -> finstack_quant_core::Result<CashFlowSchedule> {
        let mut state = initialize_build_state(
            self.issue,
            &self.notional,
            self.dates.len(),
            &self.principal_events,
        )?;
        let ccy = self.notional.initial.currency();
        // Fixed fees dated at or before issue are emitted during initialization.
        // The date loop below only processes dates strictly after issue, so this
        // is the single emission point for such fees: at-issue fees are not
        // duplicated and pre-issue fees (e.g., upfront/commitment fees on
        // delayed-funding structures) are not dropped.
        for (fee_date, amount) in &self.fixed_fees {
            if *fee_date <= self.issue && amount.amount() != 0.0 {
                state.flows.push(CashFlow::new(
                    *fee_date,
                    None,
                    *amount,
                    CFKind::Fee,
                    0.0,
                    None,
                ));
            }
        }
        // Principal redemption pays on the business-day-adjusted maturity using
        // the calendar/BDC of the principal-paying leg (first fixed schedule,
        // else first floating schedule). Without any coupon leg there is no
        // resolved calendar, so the raw maturity date is kept.
        let redemption_date = if let Some(schedule) = self.fixed_schedules.first() {
            finstack_quant_core::dates::adjust(
                self.maturity,
                schedule.spec.schedule.bdc,
                schedule.calendar,
            )?
        } else if let Some(schedule) = self.float_schedules.first() {
            finstack_quant_core::dates::adjust(
                self.maturity,
                schedule.spec.schedule.bdc,
                schedule.calendar,
            )?
        } else {
            self.maturity
        };

        let ctx = BuildContext {
            ccy,
            issue: self.issue,
            maturity: self.maturity,
            redemption_date,
            notional: &self.notional,
            fixed_schedules: &self.fixed_schedules,
            float_schedules: &self.float_schedules,
            periodic_fees: &self.periodic_fees,
            fixed_fees: &self.fixed_fees,
            principal_events: &self.principal_events,
        };

        // Resolve curves and historical-fixing series upfront and reuse across
        // all payment dates. Fixings follow the canonical core convention: a
        // `ScalarTimeSeries` stored under `FIXING:{index_id}` carries realized
        // index observations for dates before the curve base (seasoned
        // instruments).
        let (resolved_curves, resolved_fixings): (
            Vec<Option<Arc<ForwardCurve>>>,
            Vec<Option<ScalarTimeSeries>>,
        ) = if let Some(mkt) = curves {
            self.float_schedules
                .iter()
                .map(|schedule| {
                    let index_id = schedule.spec.rate_spec.index_id.as_str();
                    (
                        mkt.get_forward(index_id).ok(),
                        mkt.get_series(fixing_series_id(index_id)).ok().cloned(),
                    )
                })
                .unzip()
        } else {
            (
                vec![None; self.float_schedules.len()],
                vec![None; self.float_schedules.len()],
            )
        };

        // Initialization above consumes everything dated at or before issue
        // (initial funding, pre-/at-issue principal events, pre-/at-issue fixed
        // fees); the loop processes only dates strictly after issue. The two
        // emission paths are mutually exclusive so pre-issue dates in
        // `self.dates` can never cause issue-dated flows to be emitted twice.
        let processor =
            DateProcessor::new(&ctx, &self.amort_setup, &resolved_curves, &resolved_fixings);
        processor.process_issue_amortization(&mut state)?;
        for &d in self.dates.iter().filter(|&&d| d > self.issue) {
            state = processor.process(d, state)?;
        }

        // Warn on material residual principal without rejecting flexible structures.
        let threshold = Decimal::new(1, 4); // 1e-4 = 1 bp relative
        let initial_amount = self.notional.initial.amount();
        if initial_amount.abs() > 0.0 {
            let initial_dec = f64_to_decimal(initial_amount)?;
            if initial_dec != Decimal::ZERO {
                let abs_outstanding = if state.outstanding < Decimal::ZERO {
                    -state.outstanding
                } else {
                    state.outstanding
                };
                let abs_initial = if initial_dec < Decimal::ZERO {
                    -initial_dec
                } else {
                    initial_dec
                };
                let relative_residual = abs_outstanding / abs_initial;
                if relative_residual > threshold {
                    let final_outstanding = decimal_to_f64(state.outstanding)?;
                    let relative_residual_f64 = decimal_to_f64(relative_residual)?;
                    tracing::warn!(
                        initial = initial_amount,
                        final_outstanding,
                        relative_residual = relative_residual_f64,
                        threshold_bps = 1.0,
                        "cashflow schedule: final outstanding balance deviates from zero; \
                         check amortization schedule or instrument terminal flow"
                    );
                }
            }
        }

        let (flows, meta, out_dc) = finalize_flows(
            state.flows,
            &self.fixed_schedules,
            &self.float_schedules,
            Some(self.issue),
            Some(self.maturity),
        );
        debug!(flows = flows.len(), "cashflow schedule: project complete");
        Ok(CashFlowSchedule::from_parts(
            flows,
            self.notional.clone(),
            out_dc,
            meta,
        ))
    }
}
