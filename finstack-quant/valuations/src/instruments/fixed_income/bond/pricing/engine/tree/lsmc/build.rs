//! Template construction helpers: coupon, exercise, make-whole and grid utilities.

use super::*;

pub(super) fn required_date(value: Option<Date>, label: &str) -> Result<Date> {
    value.ok_or_else(|| Error::Validation(format!("floating coupon is missing {label} metadata")))
}

pub(super) fn floating_coupon_spec(bond: &Bond) -> Option<&FloatingCouponSpec> {
    match &bond.cashflow_spec {
        CashflowSpec::Floating(spec) => Some(spec),
        CashflowSpec::Amortizing { base, .. } => match base.as_ref() {
            CashflowSpec::Floating(spec) => Some(spec),
            _ => None,
        },
        _ => None,
    }
}

pub(super) fn contractual_reset_date(start: Date, spec: &FloatingCouponSpec) -> Result<Date> {
    let calendar_id = spec
        .rate_spec
        .fixing_calendar_id
        .as_deref()
        .unwrap_or(&spec.schedule.calendar_id);
    let calendar = resolve_calendar_strict(calendar_id)?;
    if spec.rate_spec.reset_lag_days == 0 {
        return Ok(start);
    }
    let reset = start.add_business_days(-spec.rate_spec.reset_lag_days, calendar)?;
    adjust(reset, spec.schedule.business_day_convention, calendar)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_overnight_coupon(
    tree: &RatesCreditTree,
    market: &MarketContext,
    discount: &dyn Discounting,
    as_of: Date,
    times: &[f64],
    start: Date,
    end: Date,
    spec: &FloatingCouponSpec,
) -> Result<(OvernightCoupon, FloatingRateObservation)> {
    let method = spec.rate_spec.overnight_compounding.ok_or_else(|| {
        Error::internal("bond hazard LSMC overnight replay has no compounding method")
    })?;
    let day_count = spec
        .rate_spec
        .overnight_basis
        .unwrap_or(spec.schedule.day_count);
    let day_count_basis = match day_count {
        DayCount::Act360 => 360.0,
        DayCount::Act365F => 365.0,
        other => {
            return Err(Error::Validation(format!(
                "bond hazard LSMC overnight basis must be Act360 or Act365F, got {other:?}"
            )))
        }
    };
    let calendar_id = spec
        .rate_spec
        .fixing_calendar_id
        .as_deref()
        .unwrap_or(&spec.schedule.calendar_id);
    let calendar = resolve_calendar_strict(calendar_id)?;
    let observations = OvernightObservationSchedule::compile(start, end, method, calendar)?;
    if observations.observations().is_empty() && start < end {
        return Err(Error::Validation(format!(
            "bond hazard LSMC overnight accrual [{start}, {end}) contains no observations on calendar '{calendar_id}'"
        )));
    }

    let constraints = OvernightRateConstraints {
        application: spec.rate_spec.overnight_index_constraints,
        index_floor_bp: decimal_option_to_f64(
            spec.rate_spec.index_floor_bp,
            "overnight index floor",
        )?,
        index_cap_bp: decimal_option_to_f64(spec.rate_spec.index_cap_bp, "overnight index cap")?,
    };
    let index_id = spec.rate_spec.index_id.as_str();
    let fixing_id = fixing_series_id(index_id);
    let fixings = market.get_series(&fixing_id).ok();
    let forward = market.get_forward(index_id).ok();
    let horizon = *times.last().ok_or_else(|| {
        Error::internal("bond hazard LSMC overnight replay has an empty tree grid")
    })?;
    let mut sources = BTreeMap::new();

    for observation in observations.observations() {
        let max_tenor = observation
            .rate_tenor_days
            .max(observation.weight_days)
            .max(1);
        for tenor_days in 1..=max_tenor {
            let key = (observation.observation_date, tenor_days);
            if sources.contains_key(&key) {
                continue;
            }
            let observation_date = observation.observation_date;
            let published_same_day = observation_date == as_of
                && fixings.is_some_and(|series| series.value_on_exact(observation_date).is_ok());
            let source = if observation_date < as_of || published_same_day {
                match require_fixing_value_exact(fixings, index_id, observation_date, as_of) {
                    Ok(rate) => OvernightRateSource::Fixed(rate),
                    Err(error) => OvernightRateSource::Fixed(fallback_index_rate(
                        &spec.rate_spec.fallback,
                        error,
                    )?),
                }
            } else if let Some(forward) = forward.as_ref() {
                if observation_date < forward.base_date() {
                    OvernightRateSource::Fixed(fallback_index_rate(
                        &spec.rate_spec.fallback,
                        Error::Validation(format!(
                            "overnight observation {observation_date} for index '{index_id}' precedes forward-curve base {}",
                            forward.base_date()
                        )),
                    )?)
                } else {
                    let observation_end = observation_date
                        .checked_add(Duration::days(i64::from(tenor_days)))
                        .ok_or_else(|| {
                            Error::Validation(
                                "bond hazard LSMC overnight observation end overflows the supported date range"
                                    .to_string(),
                            )
                        })?;
                    let curve_time = forward.day_count().year_fraction(
                        forward.base_date(),
                        observation_date,
                        DayCountContext::default(),
                    )?;
                    let accrual = f64::from(tenor_days) / day_count_basis;
                    let base_index_rate = forward.rate_period(curve_time, curve_time + accrual);
                    let base_df = discount.df_between_dates(observation_date, observation_end)?;
                    if !base_df.is_finite() || base_df <= 0.0 {
                        return Err(Error::Validation(format!(
                            "bond hazard LSMC has an invalid discount factor for overnight observation {observation_date}"
                        )));
                    }
                    let observation_step = exact_grid_step(times, as_of, observation_date)?;
                    let end_step = exact_grid_step(times, as_of, observation_end)?;
                    if observation_step >= end_step {
                        return Err(Error::Validation(format!(
                            "bond hazard LSMC overnight observation {observation_date} to {observation_end} maps to one tree step"
                        )));
                    }
                    OvernightRateSource::Conditional(ConditionalRate {
                        observation_step,
                        accrual,
                        base_index_rate,
                        base_discount_forward: (1.0 / base_df - 1.0) / accrual,
                        conditional_discount_factors: tree.conditional_discount_factors(
                            observation_step,
                            end_step,
                            horizon,
                        )?,
                    })
                }
            } else {
                OvernightRateSource::Fixed(fallback_index_rate(
                    &spec.rate_spec.fallback,
                    Error::Validation(format!(
                        "forward curve '{index_id}' is required for stochastic overnight observation {observation_date}"
                    )),
                )?)
            };
            sources.insert(key, source);
        }
    }

    let mut max_history_steps = 0_usize;
    for observation in observations.observations() {
        let Some(OvernightRateSource::Conditional(source)) =
            sources.get(&(observation.observation_date, observation.rate_tenor_days))
        else {
            continue;
        };
        let weight_end_step = exact_grid_step(times, as_of, observation.weight_end)?;
        max_history_steps =
            max_history_steps.max(weight_end_step.saturating_sub(source.observation_step));
    }

    Ok((
        OvernightCoupon {
            sources,
            max_history_steps,
        },
        FloatingRateObservation::Overnight {
            schedule: observations,
            day_count_basis,
            constraints,
        },
    ))
}

pub(super) fn decimal_option_to_f64(
    value: Option<rust_decimal::Decimal>,
    label: &str,
) -> Result<Option<f64>> {
    value
        .map(|value| {
            value.to_f64().ok_or_else(|| {
                Error::Validation(format!(
                    "bond hazard LSMC {label} cannot be represented as f64"
                ))
            })
        })
        .transpose()
}

pub(super) fn fallback_index_rate(fallback: &FloatingRateFallback, error: Error) -> Result<f64> {
    match fallback {
        FloatingRateFallback::Error => Err(error),
        FloatingRateFallback::SpreadOnly => Ok(0.0),
        FloatingRateFallback::FixedRate(rate) => rate.to_f64().ok_or_else(|| {
            Error::Validation(
                "bond hazard LSMC floating fallback rate cannot be represented as f64".to_string(),
            )
        }),
    }
}

pub(super) fn exact_grid_step(times: &[f64], as_of: Date, date: Date) -> Result<usize> {
    let time = DayCount::Act365F.year_fraction(as_of, date, DayCountContext::default())?;
    let step = nearest_step(times, time.max(0.0));
    let offset = times[step] - time.max(0.0);
    if offset.abs() > 1.0e-10 {
        return Err(Error::Validation(format!(
            "bond hazard LSMC date {date} is not an exact node on the daily ACT/365F grid (offset {offset})"
        )));
    }
    Ok(step)
}

pub(super) fn coupon_fractions(coupon_type: CouponType) -> Result<(f64, f64)> {
    let (cash, pik) = match coupon_type {
        CouponType::Cash => (1.0, 0.0),
        CouponType::Pik => (0.0, 1.0),
        CouponType::Split { cash_pct, pik_pct } => (
            cash_pct.to_f64().ok_or_else(|| {
                Error::Validation("floating cash fraction cannot be represented as f64".to_string())
            })?,
            pik_pct.to_f64().ok_or_else(|| {
                Error::Validation("floating PIK fraction cannot be represented as f64".to_string())
            })?,
        ),
    };
    if !cash.is_finite()
        || !pik.is_finite()
        || cash < 0.0
        || pik < 0.0
        || (cash + pik - 1.0).abs() > 1.0e-9
    {
        return Err(Error::Validation(format!(
            "floating coupon cash/PIK fractions must be non-negative and sum to one, got cash={cash}, pik={pik}"
        )));
    }
    Ok((cash, pik))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_make_whole_claim(
    call: &CallPut,
    exercise_date: Date,
    discount: &dyn Discounting,
    market: &MarketContext,
    as_of: Date,
    times: &[f64],
) -> Result<(usize, MakeWholeBasis)> {
    let spec = call.make_whole.as_ref().ok_or_else(|| {
        Error::internal("bond hazard LSMC attempted to build an absent make-whole claim")
    })?;
    let reference = market.get_discount(&spec.reference_curve_id)?;
    let base_grid = relative_curve_grid(discount, as_of, times, 0.0)?;
    let reference_grid =
        relative_curve_grid(reference.as_ref(), as_of, times, spec.spread_bp / 10_000.0)?;
    let mut interval_adjustments = Vec::with_capacity(times.len().saturating_sub(1));
    for step in 0..times.len().saturating_sub(1) {
        let base_interval = base_grid[step + 1] / base_grid[step];
        let reference_interval = reference_grid[step + 1] / reference_grid[step];
        let adjustment = reference_interval / base_interval;
        if !adjustment.is_finite() || adjustment <= 0.0 {
            return Err(Error::Validation(format!(
                "make-whole reference-basis adjustment at step {step} is not positive and finite"
            )));
        }
        interval_adjustments.push(adjustment);
    }
    Ok((
        exact_grid_step(times, as_of, exercise_date)?,
        MakeWholeBasis {
            interval_adjustments,
        },
    ))
}

pub(super) fn relative_curve_grid(
    curve: &dyn Discounting,
    origin: Date,
    times: &[f64],
    spread: f64,
) -> Result<Vec<f64>> {
    if !spread.is_finite() {
        return Err(Error::Validation(format!(
            "make-whole spread must be finite, got {spread}"
        )));
    }
    times
        .iter()
        .map(|time| {
            let day_position = time * 365.0;
            let lower_days = day_position.floor() as i64;
            let upper_days = day_position.ceil() as i64;
            let weight = day_position - lower_days as f64;
            let lower_date = origin
                .checked_add(Duration::days(lower_days))
                .ok_or_else(|| {
                    Error::Validation("make-whole grid date underflow or overflow".to_string())
                })?;
            let upper_date = origin
                .checked_add(Duration::days(upper_days))
                .ok_or_else(|| {
                    Error::Validation("make-whole grid date underflow or overflow".to_string())
                })?;
            let lower_df = curve.df_between_dates(origin, lower_date)?;
            let upper_df = curve.df_between_dates(origin, upper_date)?;
            if !lower_df.is_finite() || !upper_df.is_finite() || lower_df <= 0.0 || upper_df <= 0.0
            {
                return Err(Error::Validation(
                    "make-whole reference curve returned an invalid discount factor".to_string(),
                ));
            }
            let curve_df = ((1.0 - weight) * lower_df.ln() + weight * upper_df.ln()).exp();
            let lower_tau =
                curve
                    .day_count()
                    .year_fraction(origin, lower_date, DayCountContext::default())?;
            let upper_tau =
                curve
                    .day_count()
                    .year_fraction(origin, upper_date, DayCountContext::default())?;
            let tau = (1.0 - weight) * lower_tau + weight * upper_tau;
            let adjusted = curve_df * (-spread * tau).exp();
            if adjusted.is_finite() && adjusted > 0.0 {
                Ok(adjusted)
            } else {
                Err(Error::Validation(
                    "make-whole spread-adjusted reference discount is invalid".to_string(),
                ))
            }
        })
        .collect()
}

pub(super) fn make_whole_value(
    call: &CallPut,
    exercise_date: Date,
    flows: &[CashFlow],
    market: &MarketContext,
) -> Result<Option<f64>> {
    let Some(spec) = &call.make_whole else {
        return Ok(None);
    };
    let reference = market.get_discount(&spec.reference_curve_id)?;
    let spread = spec.spread_bp / 10_000.0;
    let mut value = 0.0;
    for flow in flows
        .iter()
        .filter(|flow| flow.date > exercise_date && is_cash_settlement_kind(flow.kind))
    {
        let tau = reference.day_count().year_fraction(
            exercise_date,
            flow.date,
            DayCountContext::default(),
        )?;
        value += flow.amount.amount()
            * reference.df_between_dates(exercise_date, flow.date)?
            * (-spread * tau).exp();
    }
    if value.is_finite() && value >= 0.0 {
        Ok(Some(value))
    } else {
        Err(Error::Validation(format!(
            "make-whole value at {exercise_date} must be non-negative and finite, got {value}"
        )))
    }
}

pub(super) fn is_holder_distribution(flow: &CashFlow) -> bool {
    flow.amount.amount() > 0.0
        && matches!(
            flow.kind,
            CFKind::Fixed | CFKind::FloatReset | CFKind::Stub | CFKind::Amortization
        )
}

pub(super) fn static_balance_delta(flow: &CashFlow, terminal_redemption: bool) -> Option<f64> {
    if terminal_redemption {
        return None;
    }
    match flow.kind {
        CFKind::Pik => Some(flow.amount.amount()),
        CFKind::Amortization
        | CFKind::PrePayment
        | CFKind::DefaultedNotional
        | CFKind::Notional
        | CFKind::RevolvingDraw
        | CFKind::RevolvingRepayment => Some(-flow.amount.amount()),
        _ => None,
    }
}

pub(super) fn scheduled_outstanding_after(
    bond: &Bond,
    flows: &[CashFlow],
    date: Date,
    final_redemption_date: Option<Date>,
) -> Result<f64> {
    let mut outstanding = bond.notional.amount();
    for flow in flows.iter().filter(|flow| flow.date <= date) {
        if flow.kind == CFKind::Notional
            && flow.date == bond.issue_date
            && flow.amount.amount() < 0.0
        {
            continue;
        }
        let terminal_redemption = flow.kind == CFKind::Notional
            && final_redemption_date.is_some_and(|redemption| flow.date == redemption);
        if let Some(delta) = static_balance_delta(flow, terminal_redemption) {
            outstanding += delta;
        }
    }
    if outstanding.is_finite() && outstanding >= -1.0e-8 {
        Ok(outstanding.max(0.0))
    } else {
        Err(Error::Validation(format!(
            "Bond '{}' has invalid scheduled outstanding {outstanding} after {date}",
            bond.id.as_str()
        )))
    }
}

pub(super) fn has_exercise_claim_on_date(bond: &Bond, date: Date) -> Result<bool> {
    if date > bond.maturity {
        return Ok(false);
    }
    if bond.call_put.as_ref().is_some_and(|rights| {
        rights
            .calls
            .iter()
            .chain(&rights.puts)
            .any(|right| right.start_date <= date && date <= right.end_date)
    }) {
        return Ok(true);
    }
    let Some(floor) = bond.return_floor.as_ref() else {
        return Ok(false);
    };
    Ok(
        return_floor_dates(floor.window, bond.issue_date, bond.maturity, date, &[])?
            .first()
            .is_some_and(|claim_date| *claim_date == date),
    )
}

pub(super) fn exercise_dates(
    option: &CallPut,
    as_of: Date,
    maturity: Date,
    event_dates: &[Date],
) -> Vec<Date> {
    BondValuator::exercise_candidates(
        option.start_date.max(as_of),
        option.end_date.min(maturity),
        event_dates,
    )
}

/// Return-floor exercise candidates: the protection window (clipped to the
/// bond's life after issue and before maturity, and to `as_of`) ends plus the
/// interior `event_dates` (distribution and balance dates).
pub(super) fn return_floor_dates(
    window: ProtectionWindow,
    issue_date: Date,
    maturity: Date,
    as_of: Date,
    event_dates: &[Date],
) -> Result<Vec<Date>> {
    let (window_start, window_end) =
        crate::instruments::fixed_income::bond::pricing::return_floor::protection_window_bounds(
            window, issue_date, maturity,
        )?;
    Ok(BondValuator::exercise_candidates(
        window_start.max(as_of),
        window_end,
        event_dates,
    ))
}

pub(super) fn nearest_step(times: &[f64], time: f64) -> usize {
    let last = times.len() - 1;
    if time <= times[0] {
        return 0;
    }
    if time >= times[last] {
        return last;
    }
    let upper = times.partition_point(|&candidate| candidate <= time);
    let lower = upper - 1;
    if time - times[lower] <= times[upper] - time {
        lower
    } else {
        upper
    }
}

pub(super) fn safe_accrual_ratio(elapsed: f64, total: f64) -> f64 {
    if total > 0.0 && elapsed.is_finite() && total.is_finite() {
        (elapsed / total).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

pub(super) fn path_state(
    path: &[RatesCreditPathState],
    step: usize,
) -> Result<&RatesCreditPathState> {
    let first = path
        .first()
        .map(|state| state.step)
        .ok_or_else(|| Error::internal("bond hazard LSMC sampled path is empty"))?;
    let offset = step.checked_sub(first).ok_or_else(|| {
        Error::internal(format!(
            "bond hazard LSMC step {step} precedes sampled segment start {first}"
        ))
    })?;
    path.get(offset)
        .filter(|state| state.step == step)
        .ok_or_else(|| {
            Error::internal(format!(
                "bond hazard LSMC step {step} is outside sampled segment"
            ))
        })
}

pub(super) fn validate_exercise_amount(label: &str, value: Option<f64>) -> Result<()> {
    if value.is_none_or(|amount| amount.is_finite() && amount >= 0.0) {
        Ok(())
    } else {
        Err(Error::Validation(format!(
            "bond hazard LSMC {label} amount must be non-negative and finite"
        )))
    }
}
