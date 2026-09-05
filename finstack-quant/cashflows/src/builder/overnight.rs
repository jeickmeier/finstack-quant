//! Reusable overnight observation schedules and rate replay.
//!
//! This module compiles the calendar part of an overnight coupon once. The
//! resulting schedule can then replay historical, projected, or simulated
//! observation rates for the full accrual period or for a partial accrual that
//! ends on an exercise date.

use finstack_quant_core::dates::{Date, DateExt, HolidayCalendar};
use finstack_quant_core::{Error, Result};

use super::specs::{OvernightCompoundingMethod, OvernightIndexConstraintApplication};

/// One dated observation and the calendar-day interval that it weights.
///
/// `observation_date` identifies the fixing supplied to the replay callback.
/// `weight_start..weight_end` identifies the half-open interval over which that
/// fixing accrues. These differ under lookback, and both weight dates move
/// under observation shift. `rate_tenor_days` preserves the tenor used when a
/// projected overnight rate is sampled; it is normally one day even when a
/// Friday fixing carries a three-day weekend weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OvernightObservationSlice {
    /// Business date whose overnight fixing or simulated rate is observed.
    pub observation_date: Date,
    /// Inclusive start of the interval weighted by this observation.
    pub weight_start: Date,
    /// Exclusive end of the interval weighted by this observation.
    pub weight_end: Date,
    /// Number of calendar days in `weight_start..weight_end`.
    pub weight_days: u32,
    /// Calendar-day tenor used to project the observation rate from a curve.
    /// This can differ from `weight_days` when a business-day fixing carries
    /// through a weekend or holiday.
    pub rate_tenor_days: u32,
}

/// Index-floor and index-cap policy applied during overnight replay.
///
/// Bounds are expressed in basis points and apply to the index component
/// before coupon spread, gearing, or all-in constraints. `Daily` constrains
/// every observed rate before aggregation; `Period` constrains the aggregated
/// period index rate once.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct OvernightRateConstraints {
    /// Whether index bounds apply to each daily observation or to the final
    /// period index rate.
    pub application: OvernightIndexConstraintApplication,
    /// Optional index floor in basis points; `Some(300.0)` means 3%.
    pub index_floor_bp: Option<f64>,
    /// Optional index cap in basis points; `Some(500.0)` means 5%.
    pub index_cap_bp: Option<f64>,
}

/// Rates and weight returned by an overnight observation replay.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OvernightRateReplay {
    /// Annualized period index rate before any daily or period index bounds,
    /// expressed as a decimal rate.
    pub projected_rate: f64,
    /// Annualized period index rate after the configured index bounds,
    /// expressed as a decimal rate.
    pub constrained_rate: f64,
    /// Calendar days in the replayed observation-weight window.
    pub weight_days: u32,
}

/// Cloneable state for incremental overnight-rate replay.
///
/// The accumulator lets path-dependent pricers checkpoint a partially accrued
/// coupon and resume it without replaying earlier observations. It preserves
/// the exact simple-versus-compounded and daily-versus-period constraint
/// semantics of [`OvernightObservationSchedule::replay`].
#[derive(Debug, Clone)]
pub struct OvernightRateAccumulator {
    accrual_start: Date,
    contractual_end: Date,
    observation_start: Date,
    accrued_through: Date,
    weight_end: Date,
    day_count_basis: f64,
    constraints: OvernightRateConstraints,
    is_simple: bool,
    projected_accumulator: f64,
    constrained_accumulator: f64,
    observed_any: bool,
    active_slice: Option<AccumulatedSlice>,
}

#[derive(Debug, Clone, Copy)]
struct AccumulatedSlice {
    index: usize,
    projected_rate: f64,
    constrained_rate: f64,
    weight_days: u32,
}

/// Calendar-compiled overnight observations for one contractual accrual period.
///
/// The schedule encodes in-arrears, lookback, lockout, and observation-shift
/// date semantics. It contains no market data, so callers can reuse it across
/// deterministic projections and stochastic paths.
#[derive(Debug, Clone)]
pub struct OvernightObservationSchedule {
    method: OvernightCompoundingMethod,
    accrual_start: Date,
    accrual_end: Date,
    observation_start: Date,
    observations: Vec<OvernightObservationSlice>,
    tenor_tracks_weight: Vec<bool>,
    observation_cutoffs: Vec<Date>,
}

impl OvernightObservationSchedule {
    /// Compile one accrual period into dated, weighted overnight observations.
    ///
    /// Lookback moves each fixing date while retaining accrual-date weights.
    /// Observation shift moves both the observation window and its weights.
    /// Lockout freezes the final observations at the preceding fixing. Its
    /// boundary belongs to the full contractual period, so partial replay
    /// does not introduce an earlier cut-off.
    ///
    /// # Arguments
    ///
    /// * `accrual_start` - Inclusive contractual accrual start date.
    /// * `accrual_end` - Exclusive contractual accrual end date; it must be on
    ///   or after `accrual_start`.
    /// * `method` - Overnight aggregation and observation-date convention.
    /// * `calendar` - Fixing calendar used for business-day lookback, lockout,
    ///   observation shift, and weekend or holiday carry.
    ///
    /// # Returns
    ///
    /// A market-data-independent schedule reusable for full and partial rate
    /// replay.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] for reversed dates or convention day
    /// counts larger than `i32::MAX`, and propagates calendar date errors.
    pub fn compile(
        accrual_start: Date,
        accrual_end: Date,
        method: OvernightCompoundingMethod,
        calendar: &dyn HolidayCalendar,
    ) -> Result<Self> {
        if accrual_end < accrual_start {
            return Err(Error::Validation(format!(
                "overnight accrual end {accrual_end} precedes start {accrual_start}"
            )));
        }

        let observation_shift = match method {
            OvernightCompoundingMethod::CompoundedWithObservationShift { shift_days } => {
                i32::try_from(shift_days).map_err(|_| {
                    Error::Validation(format!(
                        "observation shift_days = {shift_days} exceeds i32::MAX"
                    ))
                })?
            }
            _ => 0,
        };
        let observation_start = shift_back(accrual_start, observation_shift, calendar)?;
        let observation_end = shift_back(accrual_end, observation_shift, calendar)?;

        let lookback_days = match method {
            OvernightCompoundingMethod::CompoundedWithLookback { lookback_days } => lookback_days,
            _ => 0,
        };
        let (mut observations, mut tenor_tracks_weight) =
            compile_window(observation_start, observation_end, lookback_days, calendar)?;

        if let OvernightCompoundingMethod::CompoundedWithLockout { lockout_days } = method {
            apply_lockout(&mut observations, &mut tenor_tracks_weight, lockout_days)?;
        }

        let accrual_days = non_negative_days(accrual_start, accrual_end, "accrual period")?;
        let cutoff_capacity = usize::try_from(accrual_days)
            .unwrap_or(usize::MAX)
            .saturating_add(1);
        let mut observation_cutoffs = Vec::with_capacity(cutoff_capacity);
        let mut cutoff = accrual_start;
        loop {
            observation_cutoffs.push(shift_back(cutoff, observation_shift, calendar)?);
            if cutoff == accrual_end {
                break;
            }
            cutoff += time::Duration::days(1);
        }

        Ok(Self {
            method,
            accrual_start,
            accrual_end,
            observation_start,
            observations,
            tenor_tracks_weight,
            observation_cutoffs,
        })
    }

    /// Return the full-period dated observation slices in ascending weight order.
    ///
    /// # Returns
    ///
    /// The compiled observations. An accrual window containing no fixing
    /// business day returns an empty slice and will fail a non-zero replay.
    pub fn observations(&self) -> &[OvernightObservationSlice] {
        &self.observations
    }

    /// Start a cloneable incremental replay at the contractual accrual start.
    ///
    /// # Arguments
    ///
    /// * `day_count_basis` - Annual overnight basis in days, normally `360.0`
    ///   for SOFR or EURSTR and `365.0` for SONIA.
    /// * `constraints` - Index floor/cap bounds and their daily-versus-period
    ///   application convention; bound values are in basis points.
    ///
    /// # Returns
    ///
    /// An empty replay state that can be cloned at Monte Carlo checkpoints and
    /// advanced with [`Self::advance`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when the basis is not positive and finite
    /// or a supplied floor/cap is not finite.
    pub fn accumulator(
        &self,
        day_count_basis: f64,
        constraints: OvernightRateConstraints,
    ) -> Result<OvernightRateAccumulator> {
        if !day_count_basis.is_finite() || day_count_basis <= 0.0 {
            return Err(Error::Validation(format!(
                "overnight replay day_count_basis must be positive and finite, got {day_count_basis}"
            )));
        }
        validate_constraints(constraints)?;
        Ok(OvernightRateAccumulator {
            accrual_start: self.accrual_start,
            contractual_end: self.accrual_end,
            observation_start: self.observation_start,
            accrued_through: self.accrual_start,
            weight_end: self.observation_start,
            day_count_basis,
            constraints,
            is_simple: matches!(self.method, OvernightCompoundingMethod::SimpleAverage),
            projected_accumulator: if matches!(
                self.method,
                OvernightCompoundingMethod::SimpleAverage
            ) {
                0.0
            } else {
                1.0
            },
            constrained_accumulator: if matches!(
                self.method,
                OvernightCompoundingMethod::SimpleAverage
            ) {
                0.0
            } else {
                1.0
            },
            observed_any: false,
            active_slice: None,
        })
    }

    /// Advance a partial replay without revisiting completed observations.
    ///
    /// The update is transactional: if the callback or validation fails,
    /// `accumulator` retains its prior state. The callback may be invoked again
    /// for the currently clipped leading slice when its rate tenor grows with
    /// the clipped weight; this is required for exact equivalence with a fresh
    /// prefix replay.
    ///
    /// # Arguments
    ///
    /// * `accumulator` - State returned by [`Self::accumulator`] for this same
    ///   compiled schedule.
    /// * `accrual_end` - New inclusive replay endpoint. It must not precede the
    ///   endpoint already stored in `accumulator` or exceed the contractual end.
    /// * `observation_rate` - Callback returning the decimal annual rate for a
    ///   supplied dated observation slice.
    ///
    /// # Returns
    ///
    /// The projected and constrained annualized rates through `accrual_end`,
    /// plus the accumulated observation-weight days.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] for a mismatched accumulator, a reversed
    /// or out-of-period endpoint, or a non-empty replay with no observations.
    /// Callback errors propagate unchanged.
    pub fn advance<F>(
        &self,
        accumulator: &mut OvernightRateAccumulator,
        accrual_end: Date,
        mut observation_rate: F,
    ) -> Result<OvernightRateReplay>
    where
        F: FnMut(&OvernightObservationSlice) -> Result<f64>,
    {
        let mut next = accumulator.clone();
        let result = self.advance_inner(&mut next, accrual_end, &mut observation_rate)?;
        *accumulator = next;
        Ok(result)
    }

    /// Replay observation rates through a full or partial accrual end date.
    ///
    /// The callback is invoked once for every included slice and may source a
    /// historical fixing, a forward projection, or a simulated path rate. For
    /// a partial ending inside a weekend or holiday carry interval, the
    /// callback receives a clipped `weight_end` and `weight_days`; the fixing
    /// date remains unchanged.
    ///
    /// # Arguments
    ///
    /// * `accrual_end` - Exercise or payment date ending the replayed accrual;
    ///   it must lie in the inclusive range from the compiled accrual start to
    ///   the compiled contractual end.
    /// * `day_count_basis` - Annual overnight basis in days, normally `360.0`
    ///   for SOFR or €STR and `365.0` for SONIA.
    /// * `constraints` - Index floor/cap bounds and their daily-versus-period
    ///   application convention; values are in basis points.
    /// * `observation_rate` - Callback returning the decimal annual rate for a
    ///   supplied dated observation slice.
    ///
    /// # Returns
    ///
    /// The annualized projected and constrained index rates plus the calendar
    /// weight used to annualize them. A zero-length replay returns zero rates
    /// and zero weight without invoking `observation_rate`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when the partial end lies outside the
    /// compiled period, the basis or bounds are non-finite, or a non-empty
    /// replay has no compiled observations. Callback errors propagate unchanged.
    pub fn replay<F>(
        &self,
        accrual_end: Date,
        day_count_basis: f64,
        constraints: OvernightRateConstraints,
        mut observation_rate: F,
    ) -> Result<OvernightRateReplay>
    where
        F: FnMut(&OvernightObservationSlice) -> Result<f64>,
    {
        let mut accumulator = self.accumulator(day_count_basis, constraints)?;
        self.advance(&mut accumulator, accrual_end, &mut observation_rate)
    }

    fn advance_inner<F>(
        &self,
        accumulator: &mut OvernightRateAccumulator,
        accrual_end: Date,
        observation_rate: &mut F,
    ) -> Result<OvernightRateReplay>
    where
        F: FnMut(&OvernightObservationSlice) -> Result<f64>,
    {
        if accumulator.accrual_start != self.accrual_start
            || accumulator.contractual_end != self.accrual_end
        {
            return Err(Error::Validation(
                "overnight accumulator belongs to a different observation schedule".to_string(),
            ));
        }
        if accrual_end < accumulator.accrued_through || accrual_end > self.accrual_end {
            return Err(Error::Validation(format!(
                "overnight incremental replay end {accrual_end} must lie in [{}, {}]",
                accumulator.accrued_through, self.accrual_end
            )));
        }
        if accrual_end == accumulator.accrued_through {
            return accumulator.result();
        }
        let offset_days = non_negative_days(self.accrual_start, accrual_end, "partial accrual")?;
        let offset = usize::try_from(offset_days).map_err(|_| {
            Error::Validation(format!(
                "overnight partial accrual spans too many days: {offset_days}"
            ))
        })?;
        let new_weight_end = self
            .observation_cutoffs
            .get(offset)
            .copied()
            .ok_or_else(|| {
                Error::Validation(format!(
                    "overnight replay end {accrual_end} is outside the compiled cutoff grid"
                ))
            })?;
        let old_weight_end = accumulator.weight_end;
        let daily_constraints = accumulator.constraints.application
            == OvernightIndexConstraintApplication::Daily
            && (accumulator.constraints.index_floor_bp.is_some()
                || accumulator.constraints.index_cap_bp.is_some());

        for (index, full_slice) in self.observations.iter().enumerate() {
            if full_slice.weight_start >= new_weight_end {
                break;
            }
            let clipped_end = full_slice.weight_end.min(new_weight_end);
            if clipped_end <= old_weight_end.max(full_slice.weight_start) {
                continue;
            }
            let tracks_weight_tenor = self
                .tenor_tracks_weight
                .get(index)
                .copied()
                .unwrap_or(false);
            let continuing = accumulator
                .active_slice
                .filter(|active| active.index == index);
            // One fixing contributes one simple-interest factor, even when
            // a checkpoint falls inside its weekend/holiday weight interval.
            if let Some(active) = continuing {
                accumulator.remove(active);
            }
            let weight_start = full_slice.weight_start;
            let weight_days = positive_days(weight_start, clipped_end, "observation slice")?;
            let (rate, constrained) = if !tracks_weight_tenor {
                continuing
                    .map(|active| (active.projected_rate, active.constrained_rate))
                    .unwrap_or_else(|| (f64::NAN, f64::NAN))
            } else {
                (f64::NAN, f64::NAN)
            };
            let (rate, constrained) = if rate.is_nan() {
                let mut slice = *full_slice;
                slice.weight_start = weight_start;
                slice.weight_end = clipped_end;
                slice.weight_days = weight_days;
                if tracks_weight_tenor {
                    slice.rate_tenor_days = weight_days;
                }
                let rate = observation_rate(&slice)?;
                let constrained = if daily_constraints {
                    constrain_rate(rate, accumulator.constraints)
                } else {
                    rate
                };
                if !rate.is_finite() {
                    return Err(Error::Validation(format!(
                        "overnight observation {} returned a non-finite rate",
                        slice.observation_date
                    )));
                }
                (rate, constrained)
            } else {
                (rate, constrained)
            };
            accumulator.add(rate, constrained, weight_days)?;
            accumulator.active_slice =
                (clipped_end < full_slice.weight_end).then_some(AccumulatedSlice {
                    index,
                    projected_rate: rate,
                    constrained_rate: constrained,
                    weight_days,
                });
        }

        accumulator.accrued_through = accrual_end;
        accumulator.weight_end = new_weight_end;
        accumulator.result()
    }
}

impl OvernightRateAccumulator {
    fn add(&mut self, projected_rate: f64, constrained_rate: f64, weight_days: u32) -> Result<()> {
        let weight = f64::from(weight_days);
        if self.is_simple {
            self.projected_accumulator += projected_rate * weight;
            self.constrained_accumulator += constrained_rate * weight;
        } else {
            let projected_factor = 1.0 + projected_rate * weight / self.day_count_basis;
            let constrained_factor = 1.0 + constrained_rate * weight / self.day_count_basis;
            if !projected_factor.is_finite()
                || !constrained_factor.is_finite()
                || projected_factor <= 0.0
                || constrained_factor <= 0.0
            {
                return Err(Error::Validation(format!(
                    "overnight compounded observation factor must be positive and finite, got projected={projected_factor}, constrained={constrained_factor}"
                )));
            }
            self.projected_accumulator *= projected_factor;
            self.constrained_accumulator *= constrained_factor;
        }
        self.observed_any = true;
        Ok(())
    }

    fn remove(&mut self, slice: AccumulatedSlice) {
        let weight = f64::from(slice.weight_days);
        if self.is_simple {
            self.projected_accumulator -= slice.projected_rate * weight;
            self.constrained_accumulator -= slice.constrained_rate * weight;
        } else {
            self.projected_accumulator /=
                1.0 + slice.projected_rate * weight / self.day_count_basis;
            self.constrained_accumulator /=
                1.0 + slice.constrained_rate * weight / self.day_count_basis;
        }
    }

    /// Return the replay result through the accumulator's current endpoint.
    ///
    /// # Returns
    ///
    /// The projected and constrained annualized rates plus accumulated
    /// observation-weight days. A newly created accumulator returns zero rates
    /// and zero weight.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if a non-empty accrued interval contains
    /// no compiled business-day observation.
    pub fn result(&self) -> Result<OvernightRateReplay> {
        let weight_days = non_negative_days(
            self.observation_start,
            self.weight_end,
            "observation-weight period",
        )?;
        if weight_days == 0 {
            return Ok(OvernightRateReplay {
                projected_rate: 0.0,
                constrained_rate: 0.0,
                weight_days: 0,
            });
        }
        if !self.observed_any {
            return Err(Error::Validation(format!(
                "overnight replay through {} contains no business-day observations",
                self.accrued_through
            )));
        }
        let projected_rate = annualize(
            self.projected_accumulator,
            self.is_simple,
            weight_days,
            self.day_count_basis,
        );
        let daily_constraints = self.constraints.application
            == OvernightIndexConstraintApplication::Daily
            && (self.constraints.index_floor_bp.is_some()
                || self.constraints.index_cap_bp.is_some());
        let constrained_rate = match self.constraints.application {
            OvernightIndexConstraintApplication::Daily if daily_constraints => annualize(
                self.constrained_accumulator,
                self.is_simple,
                weight_days,
                self.day_count_basis,
            ),
            OvernightIndexConstraintApplication::Daily => projected_rate,
            OvernightIndexConstraintApplication::Period => {
                constrain_rate(projected_rate, self.constraints)
            }
        };
        Ok(OvernightRateReplay {
            projected_rate,
            constrained_rate,
            weight_days,
        })
    }
}

#[derive(Debug)]
struct OvernightBusinessDays {
    days: Vec<Date>,
}

impl OvernightBusinessDays {
    fn build(
        window_start: Date,
        window_end: Date,
        lead_business_days: u32,
        calendar: &dyn HolidayCalendar,
    ) -> Result<Self> {
        let lead = i32::try_from(lead_business_days).map_err(|_| {
            Error::Validation(format!(
                "lookback lead = {lead_business_days} exceeds i32::MAX"
            ))
        })?;
        let earliest = if lead == 0 {
            window_start
        } else {
            window_start.add_business_days(-lead, calendar)?
        };
        let span = non_negative_days(earliest, window_end, "overnight observation window")?;
        let capacity = usize::try_from(span).unwrap_or(usize::MAX) / 5 * 2 + 8;
        let mut days = Vec::with_capacity(capacity);
        let mut cursor = earliest;
        while cursor < window_end {
            if cursor.is_business_day(calendar) {
                days.push(cursor);
            }
            cursor += time::Duration::days(1);
        }
        Ok(Self { days })
    }

    fn cursor_at_or_after(&self, date: Date) -> usize {
        self.days.partition_point(|day| *day < date)
    }

    fn step_back(&self, ordinal: usize, back: usize) -> Option<Date> {
        self.days.get(ordinal.checked_sub(back)?).copied()
    }
}

fn compile_window(
    window_start: Date,
    window_end: Date,
    lookback_days: u32,
    calendar: &dyn HolidayCalendar,
) -> Result<(Vec<OvernightObservationSlice>, Vec<bool>)> {
    let lead_days = lookback_days.saturating_add(1);
    let business_days =
        OvernightBusinessDays::build(window_start, window_end, lead_days, calendar)?;
    let mut ordinal = business_days.cursor_at_or_after(window_start);
    let lookback = usize::try_from(lookback_days).unwrap_or(usize::MAX);
    let mut observations = Vec::new();
    let mut tenor_tracks_weight = Vec::new();
    let mut current = window_start;

    while current < window_end {
        let next = (current + time::Duration::days(1)).min(window_end);
        if business_days.days.get(ordinal).copied() == Some(current) {
            if observations.is_empty() && current > window_start {
                let observation_date = business_days
                    .step_back(ordinal, lookback.saturating_add(1))
                    .ok_or_else(|| {
                        insufficient_lookback_window(lookback.saturating_add(1), current)
                    })?;
                let weight_days =
                    positive_days(window_start, current, "leading overnight observation slice")?;
                observations.push(OvernightObservationSlice {
                    observation_date,
                    weight_start: window_start,
                    weight_end: current,
                    weight_days,
                    rate_tenor_days: weight_days,
                });
                tenor_tracks_weight.push(true);
            }

            let observation_date = business_days
                .step_back(ordinal, lookback)
                .ok_or_else(|| insufficient_lookback_window(lookback, current))?;
            let weight_days = positive_days(current, next, "overnight observation slice")?;
            observations.push(OvernightObservationSlice {
                observation_date,
                weight_start: current,
                weight_end: next,
                weight_days,
                rate_tenor_days: weight_days,
            });
            tenor_tracks_weight.push(false);
            ordinal += 1;
        } else if let Some(last) = observations.last_mut() {
            last.weight_end = next;
            last.weight_days =
                positive_days(last.weight_start, next, "overnight observation carry slice")?;
        }
        current = next;
    }

    Ok((observations, tenor_tracks_weight))
}

fn apply_lockout(
    observations: &mut [OvernightObservationSlice],
    tenor_tracks_weight: &mut [bool],
    lockout_days: u32,
) -> Result<()> {
    if lockout_days == 0 {
        return Ok(());
    }
    let lockout = usize::try_from(lockout_days).unwrap_or(usize::MAX);
    if lockout >= observations.len() {
        return Err(Error::Validation(
            "lockout must leave at least one preceding fixing".into(),
        ));
    }
    let lockout_start = observations.len() - lockout;
    let source = observations[lockout_start - 1];
    for (offset, observation) in observations[lockout_start..].iter_mut().enumerate() {
        observation.observation_date = source.observation_date;
        observation.rate_tenor_days = source.rate_tenor_days;
        tenor_tracks_weight[lockout_start + offset] = false;
    }
    Ok(())
}

fn shift_back(date: Date, shift_days: i32, calendar: &dyn HolidayCalendar) -> Result<Date> {
    if shift_days == 0 {
        Ok(date)
    } else {
        date.add_business_days(-shift_days, calendar)
    }
}

fn non_negative_days(start: Date, end: Date, label: &str) -> Result<u32> {
    u32::try_from((end - start).whole_days()).map_err(|_| {
        Error::Validation(format!(
            "{label} end {end} must be on or after start {start} and fit in u32 days"
        ))
    })
}

fn positive_days(start: Date, end: Date, label: &str) -> Result<u32> {
    let days = non_negative_days(start, end, label)?;
    if days == 0 {
        return Err(Error::Validation(format!(
            "{label} [{start}, {end}) must have positive length"
        )));
    }
    Ok(days)
}

fn insufficient_lookback_window(back: usize, date: Date) -> Error {
    Error::Validation(format!(
        "overnight observation window lacks {back} business days of history before {date}"
    ))
}

fn validate_constraints(constraints: OvernightRateConstraints) -> Result<()> {
    for (name, bound) in [
        ("index_floor_bp", constraints.index_floor_bp),
        ("index_cap_bp", constraints.index_cap_bp),
    ] {
        if bound.is_some_and(|value| !value.is_finite()) {
            return Err(Error::Validation(format!(
                "overnight replay {name} must be finite when supplied"
            )));
        }
    }
    if let (Some(floor), Some(cap)) = (constraints.index_floor_bp, constraints.index_cap_bp) {
        if floor > cap {
            return Err(Error::Validation(format!(
                "overnight replay index floor {floor} bp exceeds index cap {cap} bp"
            )));
        }
    }
    Ok(())
}

fn constrain_rate(mut rate: f64, constraints: OvernightRateConstraints) -> f64 {
    if let Some(floor_bp) = constraints.index_floor_bp {
        rate = rate.max(floor_bp * 1e-4);
    }
    if let Some(cap_bp) = constraints.index_cap_bp {
        rate = rate.min(cap_bp * 1e-4);
    }
    rate
}

fn annualize(accumulator: f64, is_simple: bool, total_days: u32, basis: f64) -> f64 {
    if is_simple {
        accumulator / f64::from(total_days)
    } else {
        (accumulator - 1.0) * basis / f64::from(total_days)
    }
}
