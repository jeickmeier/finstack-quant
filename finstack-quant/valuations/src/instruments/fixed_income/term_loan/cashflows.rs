//! Cashflow generation for Term Loans using the shared cashflow builder.
//!
//! Builds deterministic schedules (including DDTL draws, OID handling, PIK toggles,
//! amortization, and fees) via the unified `CashFlowBuilder` so date logic and
//! floating-rate conventions stay consistent across instruments.

use crate::cashflow::builder::schedule::{merge_cashflow_schedules, CashFlowSchedule};
use crate::cashflow::builder::specs::{
    CouponType, FeeBase, FeeSpec, FloatingCouponSpec, FloatingRateSpec, StepUpCouponSpec,
};
use crate::cashflow::builder::{CashFlowBuilder, PrincipalEvent, ScheduleParams};
use crate::cashflow::primitives::{CFKind, CashFlow};
use crate::instruments::fixed_income::term_loan::types::TermLoan;
use finstack_quant_core::cashflow::xirr_with_daycount;
use finstack_quant_core::dates::Date;
use finstack_quant_core::dates::DayCountContext;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use rust_decimal::Decimal;
use std::collections::BTreeMap;

fn loan_schedule_params(loan: &TermLoan) -> ScheduleParams {
    ScheduleParams {
        freq: loan.frequency,
        dc: loan.day_count,
        bdc: loan.bdc,
        calendar_id: loan
            .calendar_id
            .clone()
            .unwrap_or_else(|| crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID.to_string()),
        stub: loan.stub,
        end_of_month: false,
        payment_lag_days: 0,
        adjust_accrual_dates: false,
    }
}

/// Generate the full crate-internal cashflow schedule for a term loan.
pub(crate) fn generate_cashflows(
    loan: &TermLoan,
    market: &MarketContext,
    _as_of: Date,
) -> finstack_quant_core::Result<CashFlowSchedule> {
    let mut principal_events: Vec<PrincipalEvent> = Vec::new();
    let mut fees: Vec<FeeSpec> = Vec::new();

    // Draw stop date (if any)
    let draw_stop = effective_draw_stop(loan);

    // DDTL draws or upfront funding
    if let Some(ddtl) = &loan.ddtl {
        for ev in &ddtl.draws {
            if ev.date < ddtl.availability_start || ev.date > ddtl.availability_end {
                continue;
            }
            if let Some(ds) = draw_stop {
                if ev.date >= ds {
                    continue;
                }
            }

            // Apply OID policy to determine cash inflow
            let mut cash_inflow = ev.amount;
            if let Some(oid) = &ddtl.oid_policy {
                match oid {
                    super::spec::OidPolicy::WithheldPct(bp) => {
                        let pct = f64::from(*bp) * 1e-4;
                        cash_inflow =
                            Money::new(ev.amount.amount() * (1.0 - pct), ev.amount.currency());
                    }
                    super::spec::OidPolicy::WithheldAmount(m) => {
                        cash_inflow = ev.amount.checked_sub(*m)?;
                    }
                    super::spec::OidPolicy::SeparatePct(bp) => {
                        let pct = f64::from(*bp) * 1e-4;
                        let fee_amt = Money::new(ev.amount.amount() * pct, ev.amount.currency());
                        if fee_amt.amount() > 0.0 {
                            fees.push(FeeSpec::Fixed {
                                date: ev.date,
                                amount: fee_amt,
                            });
                        }
                    }
                    super::spec::OidPolicy::SeparateAmount(m) => {
                        if m.amount() > 0.0 {
                            fees.push(FeeSpec::Fixed {
                                date: ev.date,
                                amount: *m,
                            });
                        }
                    }
                }
            }

            principal_events.push(PrincipalEvent {
                date: ev.date,
                delta: ev.amount,
                cash: cash_inflow,
                kind: CFKind::Notional,
            });
        }
    } else if loan.notional_limit.amount() != 0.0 {
        principal_events.push(PrincipalEvent {
            date: loan.issue_date,
            delta: loan.notional_limit,
            cash: loan.notional_limit,
            kind: CFKind::Notional,
        });
    }

    // Upfront fee
    if let Some(fee) = loan.upfront_fee {
        if fee.amount() > 0.0 {
            fees.push(FeeSpec::Fixed {
                date: loan.issue_date,
                amount: fee,
            });
        }
    }

    // Cash sweeps
    if let Some(cov) = &loan.covenants {
        for sweep in &cov.cash_sweeps {
            if sweep.amount.amount() > 0.0 {
                principal_events.push(PrincipalEvent {
                    date: sweep.date,
                    delta: Money::new(-sweep.amount.amount(), sweep.amount.currency()),
                    cash: sweep.amount,
                    kind: CFKind::Amortization,
                });
            }
        }
    }
    if let Some(ov) = &loan.instrument_pricing_overrides.term_loan {
        for (dt, amt) in &ov.extra_cash_sweeps {
            if amt.amount() > 0.0 {
                principal_events.push(PrincipalEvent {
                    date: *dt,
                    delta: Money::new(-amt.amount(), amt.currency()),
                    cash: *amt,
                    kind: CFKind::Amortization,
                });
            }
        }
    }

    // Coupon dates for amortization conversion
    let coupon_dates: Vec<Date> = {
        use crate::cashflow::builder::periods::{build_periods, BuildPeriodsParams};

        let schedule = loan_schedule_params(loan);
        let periods = build_periods(BuildPeriodsParams::from_schedule(
            &schedule,
            loan.issue_date,
            loan.maturity,
            None,
        ))?;
        let adjust_dates = loan.calendar_id.is_some();
        std::iter::once(loan.issue_date)
            .chain(periods.into_iter().map(|period| {
                if adjust_dates {
                    period.payment_date
                } else {
                    period.accrual_end
                }
            }))
            .collect()
    };

    // Amortization → principal events
    match &loan.amortization {
        super::spec::AmortizationSpec::None => {}
        super::spec::AmortizationSpec::Custom(items) => {
            for (dt, amt) in items {
                principal_events.push(PrincipalEvent {
                    date: *dt,
                    delta: Money::new(-amt.amount(), amt.currency()),
                    cash: *amt,
                    kind: CFKind::Amortization,
                });
            }
        }
        super::spec::AmortizationSpec::PercentPerPeriod { bp } => {
            let pct = f64::from(*bp) * 1e-4;
            // Replay actual funding, sweep, and repayment events chronologically.
            // This is essential for delayed-draw facilities: undrawn commitment
            // is not principal and must never be amortized.
            let mut existing_events: Vec<(Date, f64)> = principal_events
                .iter()
                .map(|event| (event.date, event.delta.amount()))
                .collect();
            existing_events.sort_by_key(|(date, _)| *date);
            let mut next_event = 0usize;
            let mut running_balance = 0.0_f64;
            for d in coupon_dates.iter().copied().skip(1) {
                while next_event < existing_events.len() && existing_events[next_event].0 <= d {
                    running_balance = (running_balance + existing_events[next_event].1).max(0.0);
                    next_event += 1;
                }
                let amort_amount = (running_balance * pct).min(running_balance);
                let pay = Money::new(amort_amount, loan.currency);
                principal_events.push(PrincipalEvent {
                    date: d,
                    delta: Money::new(-pay.amount(), pay.currency()),
                    cash: pay,
                    kind: CFKind::Amortization,
                });
                running_balance -= amort_amount;
            }
        }
        super::spec::AmortizationSpec::PercentOfOriginalNotional { bp } => {
            // For DDTL loans, use the actual drawn (funded) amount as the original notional.
            // For regular loans, use notional_limit.
            let original_notional = if let Some(ddtl) = &loan.ddtl {
                let draw_stop = effective_draw_stop(loan);
                ddtl.draws
                    .iter()
                    .filter(|ev| {
                        ev.date >= ddtl.availability_start
                            && ev.date <= ddtl.availability_end
                            && draw_stop.is_none_or(|ds| ev.date < ds)
                    })
                    .map(|ev| ev.amount.amount())
                    .sum::<f64>()
                    .min(loan.notional_limit.amount())
            } else {
                loan.notional_limit.amount()
            };
            let pct = f64::from(*bp) * 1e-4;
            let flat_payment = original_notional * pct;
            for d in coupon_dates.iter().copied().skip(1) {
                let pay = Money::new(flat_payment, loan.currency);
                principal_events.push(PrincipalEvent {
                    date: d,
                    delta: Money::new(-pay.amount(), pay.currency()),
                    cash: pay,
                    kind: CFKind::Amortization,
                });
            }
        }
        super::spec::AmortizationSpec::Linear { start, end } => {
            // Amortization payments occur at period END dates strictly after the
            // start date and up to (and including) the end date.  Using `> *start`
            // prevents generating a spurious amortization event at the origination
            // date when `start == issue`.
            let steps: Vec<Date> = coupon_dates
                .iter()
                .copied()
                .filter(|d| *d > *start && *d <= *end)
                .collect();
            if !steps.is_empty() {
                // Divide notional evenly across the amortization steps.
                // Using `steps.len()` directly ensures the total amortization
                // equals the notional exactly, regardless of how many coupon
                // dates fall in the amortization window.
                let per_step = loan.notional_limit.amount() / (steps.len() as f64);
                for d in steps {
                    let pay = Money::new(per_step, loan.currency);
                    principal_events.push(PrincipalEvent {
                        date: d,
                        delta: Money::new(-pay.amount(), pay.currency()),
                        cash: pay,
                        kind: CFKind::Amortization,
                    });
                }
            }
        }
    }

    principal_events.sort_by_key(|e| e.date);

    // Cap amortization events to prevent negative outstanding balance.
    // Track running outstanding from funding events and cap each amort at
    // the remaining balance.  This guards against over-amortization when
    // PercentPerPeriod bp × num_periods > 10 000 or when cash sweeps
    // combine with scheduled amortization to exceed the notional.
    {
        let mut running = 0.0_f64;
        for event in &mut principal_events {
            match event.kind {
                CFKind::Notional => {
                    running += event.delta.amount();
                }
                CFKind::Amortization => {
                    let requested = (-event.delta.amount()).max(0.0);
                    let capped = requested.min(running.max(0.0));
                    if (capped - requested).abs() > 1e-10 {
                        event.delta = Money::new(-capped, event.delta.currency());
                        event.cash = Money::new(capped, event.cash.currency());
                    }
                    running -= capped;
                }
                _ => {}
            }
        }
    }

    // Build coupon program via unified builder
    let mut builder = CashFlowBuilder::default();
    let _ = builder
        .principal(
            Money::new(0.0, loan.currency),
            loan.issue_date,
            loan.maturity,
        )
        .amortization(crate::cashflow::builder::AmortizationSpec::None);
    for event in &principal_events {
        let _ = builder.add_principal_event(event.date, event.delta, Some(event.cash), event.kind);
    }

    match &loan.rate {
        super::types::RateSpec::Fixed { rate_bp } => {
            // Convert rate from basis points to decimal using exact Decimal arithmetic
            // to avoid f64 representation errors (e.g., 333 bp → 0.0333 exactly).
            let initial_rate = Decimal::from(*rate_bp) / Decimal::from(10_000);
            let mut dated_deltas = BTreeMap::<Date, i32>::new();
            if let Some(cov) = &loan.covenants {
                for step in &cov.margin_stepups {
                    *dated_deltas.entry(step.date).or_default() += step.delta_bp;
                }
            }
            if let Some(ov) = &loan.instrument_pricing_overrides.term_loan {
                for (date, delta_bp) in &ov.margin_add_bp_by_date {
                    *dated_deltas.entry(*date).or_default() += *delta_bp;
                }
            }

            let mut running_rate = initial_rate;
            let mut step_schedule = Vec::with_capacity(dated_deltas.len());
            for (date, delta_bp) in dated_deltas {
                running_rate += Decimal::from(delta_bp) / Decimal::from(10_000);
                step_schedule.push((date, running_rate));
            }

            let spec = StepUpCouponSpec {
                coupon_type: loan.coupon_type,
                initial_rate,
                step_schedule,
                schedule: loan_schedule_params(loan),
            };
            let _ = builder.step_up_cf(spec);
        }
        super::types::RateSpec::Floating(spec) => {
            // Build margin step-up schedule for `float_margin_stepup`.
            //
            // Convention: each entry `(date, margin_bp)` defines the END of a window
            // and the margin that applies from the PREVIOUS endpoint (or issue) up to
            // `date`.  So for a constant-spread loan the list is simply
            // `[(maturity, base_spread)]`, creating one window `[issue, maturity)`.
            //
            // Covenant step-ups and pricing overrides are deltas added at their
            // effective dates.  We push a breakpoint BEFORE applying the delta so
            // that the preceding window has the pre-step-up margin.
            let mut step_ups = BTreeMap::<Date, Decimal>::new();
            if let Some(cov) = &loan.covenants {
                for step in &cov.margin_stepups {
                    *step_ups.entry(step.date).or_default() += Decimal::from(step.delta_bp);
                }
            }
            if let Some(ov) = &loan.instrument_pricing_overrides.term_loan {
                for (dt, bp) in &ov.margin_add_bp_by_date {
                    *step_ups.entry(*dt).or_default() += Decimal::from(*bp);
                }
            }

            let mut steps: Vec<(Date, Decimal)> = Vec::new();
            let mut running = spec.spread_bp;
            for (date, delta) in step_ups {
                // Close the preceding window at the step-up date with the
                // current running margin (before the step-up takes effect).
                steps.push((date, running));
                running += delta;
            }
            // Final window extends to maturity with the final running margin.
            if steps
                .last()
                .map(|(d, _)| *d != loan.maturity)
                .unwrap_or(true)
            {
                steps.push((loan.maturity, running));
            }

            let base_spec = FloatingCouponSpec {
                coupon_type: loan.coupon_type,
                rate_spec: FloatingRateSpec {
                    index_id: spec.index_id.clone(),
                    spread_bp: Decimal::ZERO,
                    gearing: spec.gearing,
                    gearing_includes_spread: spec.gearing_includes_spread,
                    index_floor_bp: spec.index_floor_bp,
                    all_in_cap_bp: spec.all_in_cap_bp,
                    all_in_floor_bp: spec.all_in_floor_bp,
                    index_cap_bp: spec.index_cap_bp,
                    overnight_index_constraints: Default::default(),
                    reset_freq: loan.frequency,
                    index_tenor: None,
                    reset_lag_days: spec.reset_lag_days,
                    fixing_calendar_id: spec.fixing_calendar_id.clone(),
                    overnight_compounding: spec.overnight_compounding,
                    overnight_basis: spec.overnight_basis,
                    fallback: spec.fallback.clone(),
                },
                schedule: loan_schedule_params(loan),
            };
            let _ = builder.float_margin_stepup_decimal(&steps, base_spec);
        }
    }

    // Payment split windows for PIK toggles.
    // Handle both enable (→ PIK) and disable (→ Cash) events so that
    // a loan can transition back to cash interest after a PIK period.
    let mut toggle_events = BTreeMap::<Date, bool>::new();
    if let Some(cov) = &loan.covenants {
        for t in &cov.pik_toggles {
            toggle_events.insert(t.date, t.enable_pik);
        }
    }
    if let Some(ov) = &loan.instrument_pricing_overrides.term_loan {
        for (dt, en) in &ov.pik_toggle_by_date {
            // Instrument pricing overrides take precedence over covenant events
            // when both target the same effective date.
            toggle_events.insert(*dt, *en);
        }
    }

    if !toggle_events.is_empty() {
        // `payment_split_program` consumes end-dated windows, whereas PIK
        // toggles are effective-dated state transitions. Close each preceding
        // window with the state that applied before the transition, then carry
        // the final state to maturity.
        let mut payment_steps = Vec::with_capacity(toggle_events.len() + 1);
        let mut active_coupon_type = loan.coupon_type;
        for (date, enable_pik) in toggle_events {
            if date > loan.issue_date {
                payment_steps.push((date, active_coupon_type));
            }
            active_coupon_type = if enable_pik {
                CouponType::PIK
            } else {
                CouponType::Cash
            };
        }
        if payment_steps
            .last()
            .is_none_or(|(date, _)| *date < loan.maturity)
        {
            payment_steps.push((loan.maturity, active_coupon_type));
        }
        let _ = builder.payment_split_program(&payment_steps);
    }

    // Add upfront/OID fees
    for fee in fees {
        let _ = builder.fee(fee);
    }
    if let Some(ddtl) = &loan.ddtl {
        if ddtl.usage_fee_bp != 0 {
            let _ = builder.fee(FeeSpec::PeriodicBps {
                base: FeeBase::Drawn,
                bps: Decimal::from(ddtl.usage_fee_bp),
                freq: loan.frequency,
                dc: loan.day_count,
                bdc: loan.bdc,
                calendar_id: loan.calendar_id.clone().unwrap_or_else(|| {
                    crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID.to_string()
                }),
                stub: loan.stub,
                accrual_basis: Default::default(),
            });
        }
    }

    // Build via shared builder (use market for forwards)
    let mut schedule = builder.build(Some(market))?;

    if let Some(ddtl) = &loan.ddtl {
        if ddtl.commitment_fee_bp != 0 {
            let commitment_fees = build_commitment_fee_flows(loan, ddtl, draw_stop, &schedule)?;
            if !commitment_fees.is_empty() {
                let notional = schedule.get_notional().clone();
                let day_count = schedule.get_day_count();
                let fee_schedule = crate::cashflow::traits::schedule_from_classified_flows(
                    commitment_fees,
                    day_count,
                    crate::cashflow::traits::ScheduleBuildOpts {
                        notional_hint: Some(notional.initial),
                        meta: schedule.get_meta().clone(),
                    },
                )
                .with_notional(notional.clone());
                schedule = merge_cashflow_schedules([schedule, fee_schedule], notional, day_count);
            }
        }
    }

    // Keep the full engine schedule here; `TermLoan::cashflow_schedule()` applies
    // the public signed canonical schedule projection on top of this internal representation.
    Ok(schedule)
}

fn effective_draw_stop(loan: &TermLoan) -> Option<Date> {
    let cov_stop = loan
        .covenants
        .as_ref()
        .and_then(|c| c.draw_stop_dates.iter().min().copied());
    let override_stop = loan
        .instrument_pricing_overrides
        .term_loan
        .as_ref()
        .and_then(|ov| ov.draw_stop_date);

    match (cov_stop, override_stop) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn build_commitment_fee_flows(
    loan: &TermLoan,
    ddtl: &super::spec::DdtlSpec,
    draw_stop: Option<Date>,
    schedule: &CashFlowSchedule,
) -> finstack_quant_core::Result<Vec<CashFlow>> {
    use finstack_quant_core::dates::DayCountContext;

    let fee_start = ddtl.availability_start;
    let mut fee_end = ddtl.availability_end;
    if let Some(ds) = draw_stop {
        if ds < fee_end {
            fee_end = ds;
        }
    }
    if fee_start >= fee_end {
        return Ok(Vec::new());
    }

    use crate::cashflow::builder::periods::{build_periods, BuildPeriodsParams};

    let schedule_params = loan_schedule_params(loan);
    let periods = build_periods(BuildPeriodsParams::from_schedule(
        &schedule_params,
        fee_start,
        fee_end,
        None,
    ))?;
    let mut dates: Vec<Date> = std::iter::once(fee_start)
        .chain(periods.into_iter().map(|period| period.accrual_end))
        .collect();
    for sd in &ddtl.commitment_step_downs {
        if sd.date > fee_start && sd.date < fee_end {
            dates.push(sd.date);
        }
    }
    // Add draw dates as breakpoints so the fee base is prorated correctly
    // when draws occur mid-period (the undrawn amount changes at each draw).
    for ev in &ddtl.draws {
        if ev.date > fee_start && ev.date < fee_end {
            if let Some(ds) = draw_stop {
                if ev.date >= ds {
                    continue;
                }
            }
            dates.push(ev.date);
        }
    }
    dates.sort();
    dates.dedup();

    let out_path = schedule.outstanding_by_date()?;
    let outstanding_at = |target: Date| -> Money {
        let mut last = Money::new(0.0, loan.currency);
        for (d, amt) in &out_path {
            if *d <= target {
                last = *amt;
            } else {
                break;
            }
        }
        last
    };

    let mut flows = Vec::new();
    let mut prev = dates[0];
    for &d in dates.iter().skip(1) {
        let yf = loan
            .day_count
            .year_fraction(prev, d, DayCountContext::default())?;
        // The sub-period is [prev, d), so a step-down effective on `d` must not
        // reduce the commitment base for the interval that ends on `d`.
        let limit = commitment_limit_at(ddtl, prev);
        if limit.currency() != loan.currency {
            return Err(finstack_quant_core::InputError::Invalid.into());
        }

        let base = match ddtl.fee_base {
            super::spec::CommitmentFeeBase::Undrawn => {
                // Use drawn amount at period start (prev) so that the fee
                // for a sub-period before a draw uses the pre-draw undrawn base.
                let drawn = cumulative_drawn_at(ddtl, draw_stop, prev);
                (limit.amount() - drawn).max(0.0)
            }
            super::spec::CommitmentFeeBase::CommitmentMinusOutstanding => {
                (limit.amount() - outstanding_at(prev).amount()).max(0.0)
            }
        };
        if base > 0.0 {
            let fee_rate = f64::from(ddtl.commitment_fee_bp) * 1e-4;
            let fee_amt = base * fee_rate * yf;
            if fee_amt > 0.0 {
                flows.push(CashFlow::new(
                    d,
                    None,
                    Money::new(fee_amt, loan.currency),
                    CFKind::CommitmentFee,
                    0.0,
                    Some(fee_rate),
                ));
            }
        }
        prev = d;
    }

    Ok(flows)
}

fn commitment_limit_at(ddtl: &super::spec::DdtlSpec, date: Date) -> Money {
    let mut limit = ddtl.commitment_limit;
    for sd in &ddtl.commitment_step_downs {
        if sd.date <= date {
            limit = sd.new_limit;
        }
    }
    limit
}

fn cumulative_drawn_at(ddtl: &super::spec::DdtlSpec, draw_stop: Option<Date>, date: Date) -> f64 {
    let mut total = 0.0;
    for ev in &ddtl.draws {
        if ev.date < ddtl.availability_start || ev.date > ddtl.availability_end {
            continue;
        }
        if let Some(ds) = draw_stop {
            if ev.date >= ds {
                continue;
            }
        }
        if ev.date <= date {
            total += ev.amount.amount();
        }
    }
    total
}

/// Period-level EIR amortization outputs for reporting.
#[derive(Debug, Clone)]
pub(crate) struct OidEirPeriod {
    /// Period end date.
    pub(crate) date: Date,
    /// OID amortization for the period.
    pub(crate) oid_amortization: Money,
    /// Closing balance for the period.
    pub(crate) closing_balance: Money,
}

/// EIR amortization schedule output.
#[derive(Debug, Clone)]
pub(crate) struct OidEirSchedule {
    /// Effective interest rate.
    pub(crate) effective_rate: f64,
    /// Period-by-period amortization details.
    pub(crate) periods: Vec<OidEirPeriod>,
}

/// Build an effective interest rate (EIR) amortization schedule from cashflows.
pub(crate) fn build_oid_eir_schedule(
    loan: &TermLoan,
    market: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<OidEirSchedule> {
    let schedule = generate_cashflows(loan, market, as_of)?;
    let spec = loan.oid_eir.clone().unwrap_or_default();

    let mut buckets: BTreeMap<Date, CashBuckets> = BTreeMap::new();
    for cf in schedule.get_flows() {
        match cf.kind {
            CFKind::Fixed | CFKind::FloatReset | CFKind::Stub => {
                buckets
                    .entry(cf.date)
                    .or_default()
                    .add_interest(cf.amount.amount());
            }
            CFKind::Fee | CFKind::CommitmentFee | CFKind::UsageFee | CFKind::FacilityFee
                if spec.include_fees =>
            {
                buckets
                    .entry(cf.date)
                    .or_default()
                    .add_interest(cf.amount.amount());
            }
            CFKind::Amortization => {
                buckets
                    .entry(cf.date)
                    .or_default()
                    .add_principal(cf.amount.amount());
            }
            CFKind::Notional => {
                buckets
                    .entry(cf.date)
                    .or_default()
                    .add_notional(cf.amount.amount());
            }
            _ => {}
        }
    }

    let flows: Vec<(Date, f64)> = buckets
        .iter()
        .map(|(d, b)| (*d, b.total))
        .filter(|(_, amt)| amt.abs() > 0.0)
        .collect();

    let effective_rate = xirr_with_daycount(flows.as_slice(), loan.day_count, None)?;

    let mut periods = Vec::new();
    let mut iter = buckets.iter();
    let (start_date, start_bucket) = iter
        .next()
        .ok_or(finstack_quant_core::InputError::TooFewPoints)?;
    // Initialize opening balance from notional (funding) flows only.
    // Using -total would incorrectly include fees or interest in the
    // first bucket, overstating the initial carrying amount.
    let mut opening_balance = -start_bucket.notional;
    let mut prev = *start_date;

    for (date, bucket) in iter {
        let yf = loan
            .day_count
            .year_fraction(prev, *date, DayCountContext::default())?;
        let interest_income = opening_balance * effective_rate * yf;
        let cash_interest = bucket.interest;
        let closing_balance = opening_balance + interest_income - bucket.total;
        let oid_amortization = interest_income - cash_interest;

        periods.push(OidEirPeriod {
            date: *date,
            oid_amortization: Money::new(oid_amortization, loan.currency),
            closing_balance: Money::new(closing_balance, loan.currency),
        });

        opening_balance = closing_balance;
        prev = *date;
    }

    Ok(OidEirSchedule {
        effective_rate,
        periods,
    })
}

#[derive(Default)]
struct CashBuckets {
    total: f64,
    interest: f64,
    principal: f64,
    /// Notional (funding) flows only, separated from amortization.
    notional: f64,
}

impl CashBuckets {
    fn add_interest(&mut self, amount: f64) {
        self.total += amount;
        self.interest += amount;
    }

    fn add_principal(&mut self, amount: f64) {
        self.total += amount;
        self.principal += amount;
    }

    fn add_notional(&mut self, amount: f64) {
        self.total += amount;
        self.principal += amount;
        self.notional += amount;
    }
}
