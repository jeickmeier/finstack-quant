//! Compiled floating-coupon accounting shared by projection and path replay.
//!
//! The compiled coupon separates the date on which a term rate is fixed from
//! the accrual start on which its notional is captured.  This matters for
//! reset-lagged coupons when amortization or PIK changes principal between the
//! two dates.  Overnight coupons reuse the canonical incremental observation
//! accumulator.

use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::{Error, Result};

use super::overnight::{
    OvernightObservationSchedule, OvernightObservationSlice, OvernightRateAccumulator,
    OvernightRateConstraints, OvernightRateReplay,
};
use super::rate_helpers::{calculate_floating_rate, FloatingRateParams};

/// Contractual dates and year fraction for one floating coupon.
#[derive(Debug, Clone, Copy)]
pub struct FloatingCouponPeriod {
    /// Date on which the coupon starts accruing and captures principal.
    pub accrual_start: Date,
    /// Contractual end of the coupon accrual period.
    pub accrual_end: Date,
    /// Date on which the cash portion is paid. PIK capitalizes at `accrual_end`.
    ///
    /// Business-day adjustment can place this date before an unadjusted
    /// contractual accrual end, such as a Sunday month-end paid on Friday.
    pub payment_date: Date,
    /// Day-count convention used for partial accrued-interest calculations.
    pub day_count: DayCount,
    /// Contractual calendar, coupon frequency, reference period, and termination
    /// convention retained for full and partial day-count calculations.
    pub day_count_context: DayCountContext<'static>,
    /// Full coupon or balance-segment accrual factor in years.
    ///
    /// For ACT/365L this factor owns the contractual 365/366 denominator,
    /// which can come from a larger original coupon period rather than this
    /// segment's end date. Partial accrued interest prorates the stored factor
    /// by actual days. Other conventions must match `day_count_context`.
    pub accrual_factor: f64,
}

/// Cash/PIK split and all-in rate adjustments for one floating coupon.
#[derive(Debug, Clone)]
pub struct FloatingCouponEconomics {
    /// Fraction of total coupon interest paid in cash, in `[0, 1]`.
    pub cash_fraction: f64,
    /// Fraction of total coupon interest capitalized as PIK, in `[0, 1]`.
    pub pik_fraction: f64,
    /// Spread, gearing, and index/all-in floor and cap rules.
    pub rate_params: FloatingRateParams,
}

/// Market-rate observation convention for a compiled floating coupon.
#[derive(Debug, Clone)]
pub enum FloatingRateObservation {
    /// A single term-index fixing whose quoted tenor comes from the resolved
    /// forward curve.
    Term {
        /// Business-day-adjusted fixing date, which can precede accrual start.
        reset_date: Date,
        /// Authoritative index tenor in years from the resolved forward curve.
        tenor_years: f64,
    },
    /// An overnight index aggregated over a compiled observation schedule.
    Overnight {
        /// Calendar-compiled observation dates and weights.
        schedule: OvernightObservationSchedule,
        /// Annual overnight day-count basis, normally 360 or 365.
        day_count_basis: f64,
        /// Daily or period-level index floor and cap rules.
        constraints: OvernightRateConstraints,
    },
}

/// Market-independent floating coupon ready for deterministic or path replay.
#[derive(Debug, Clone)]
pub struct CompiledFloatingCoupon {
    period: FloatingCouponPeriod,
    economics: FloatingCouponEconomics,
    observation: FloatingRateObservation,
}

/// Cloneable path state for one compiled floating coupon.
#[derive(Debug, Clone, Default)]
pub struct FloatingCouponReplayState {
    notional: Option<f64>,
    term_index_rate: Option<f64>,
    overnight: Option<OvernightRateAccumulator>,
}

/// Coupon amounts produced when a compiled floating coupon settles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloatingCouponSettlement {
    /// Index rate before coupon spread, gearing, and all-in bounds.
    pub projected_index_rate: f64,
    /// Coupon rate after all index and all-in adjustments, as a decimal.
    pub all_in_rate: f64,
    /// Total coupon interest before the cash/PIK split.
    pub total_amount: f64,
    /// Cash interest paid to the holder.
    pub cash_amount: f64,
    /// Interest capitalized into outstanding principal.
    pub pik_amount: f64,
}

impl CompiledFloatingCoupon {
    /// Validate and compile one floating coupon.
    ///
    /// # Arguments
    ///
    /// * `period` - Contractual accrual/payment dates, day count, and coupon
    ///   or balance-segment accrual factor in years.
    /// * `economics` - Cash/PIK split plus decimal-rate adjustment rules.
    /// * `observation` - Term fixing or compiled overnight observation rule.
    ///
    /// # Returns
    ///
    /// A reusable coupon descriptor with no market or simulated rate values.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] for invalid dates, accrual, split, rate
    /// parameters, term tenor, or overnight accumulator configuration.
    /// Propagates day-count errors when the supplied contractual context is
    /// incomplete; rejects an accrual factor inconsistent with that context.
    /// ACT/365L instead validates the stored factor's implied actual-day
    /// denominator as 365 or 366, preserving a larger coupon's convention
    /// when this descriptor represents only one balance segment.
    pub fn compile(
        period: FloatingCouponPeriod,
        mut economics: FloatingCouponEconomics,
        observation: FloatingRateObservation,
    ) -> Result<Self> {
        if period.accrual_end <= period.accrual_start {
            return Err(Error::Validation(format!(
                "floating coupon accrual end {} must follow start {}",
                period.accrual_end, period.accrual_start
            )));
        }
        if !period.accrual_factor.is_finite() || period.accrual_factor <= 0.0 {
            return Err(Error::Validation(format!(
                "floating coupon accrual factor must be positive and finite, got {}",
                period.accrual_factor
            )));
        }
        validate_fraction(economics.cash_fraction, "cash_fraction")?;
        validate_fraction(economics.pik_fraction, "pik_fraction")?;
        let split = economics.cash_fraction + economics.pik_fraction;
        if (split - 1.0).abs() > 1.0e-12 {
            return Err(Error::Validation(format!(
                "floating coupon cash_fraction + pik_fraction must equal 1, got {split}"
            )));
        }
        economics.rate_params.validate()?;
        let contractual_accrual = if period.day_count == DayCount::Act365L {
            let actual_days = (period.accrual_end - period.accrual_start).whole_days() as f64;
            let denominator = actual_days / period.accrual_factor;
            if (denominator - 365.0).abs() > 1e-9 && (denominator - 366.0).abs() > 1e-9 {
                return Err(Error::Validation(format!(
                    "ACT/365L coupon accrual factor must imply a 365 or 366 day denominator, got {denominator}",
                )));
            }
            period.accrual_factor
        } else {
            period.day_count.year_fraction(
                period.accrual_start,
                period.accrual_end,
                period.day_count_context,
            )?
        };
        if (contractual_accrual - period.accrual_factor).abs()
            > 1e-12 * contractual_accrual.abs().max(1.0)
        {
            return Err(Error::Validation(format!(
                "floating coupon accrual factor {} differs from contractual day count {contractual_accrual}",
                period.accrual_factor,
            )));
        }
        match &observation {
            FloatingRateObservation::Term { tenor_years, .. }
                if !tenor_years.is_finite() || *tenor_years <= 0.0 =>
            {
                return Err(Error::Validation(format!(
                    "floating term-index tenor must be positive and finite, got {tenor_years}"
                )));
            }
            FloatingRateObservation::Overnight {
                schedule,
                day_count_basis,
                constraints,
            } => {
                if schedule.accrual_period() != (period.accrual_start, period.accrual_end) {
                    return Err(Error::Validation(
                        "overnight observations must match the contractual coupon period".into(),
                    ));
                }
                let _ = schedule.accumulator(*day_count_basis, *constraints)?;
                if constraints.application
                    == super::specs::OvernightIndexConstraintApplication::Daily
                {
                    for (name, economic_bound, observation_bound) in [
                        (
                            "index_floor_bp",
                            economics.rate_params.index_floor_bp,
                            constraints.index_floor_bp,
                        ),
                        (
                            "index_cap_bp",
                            economics.rate_params.index_cap_bp,
                            constraints.index_cap_bp,
                        ),
                    ] {
                        if economic_bound.is_some() && economic_bound != observation_bound {
                            return Err(Error::Validation(format!(
                                "overnight daily {name} must match the observation constraint"
                            )));
                        }
                    }
                    // Daily index bounds belong to observation replay, not the
                    // annualized compound rate produced by that replay.
                    economics.rate_params.index_floor_bp = None;
                    economics.rate_params.index_cap_bp = None;
                }
            }
            FloatingRateObservation::Term { .. } => {}
        }
        Ok(Self {
            period,
            economics,
            observation,
        })
    }

    /// Return the contractual period metadata.
    #[must_use]
    pub const fn period(&self) -> FloatingCouponPeriod {
        self.period
    }

    /// Return the market-rate observation rule.
    #[must_use]
    pub fn observation(&self) -> &FloatingRateObservation {
        &self.observation
    }

    /// Return the coupon cash/PIK and all-in rate rules.
    #[must_use]
    pub fn economics(&self) -> &FloatingCouponEconomics {
        &self.economics
    }

    /// Create empty replay state for this coupon.
    ///
    /// The state can lock a term fixing before principal is known. Overnight
    /// accumulation begins only when [`Self::capture_notional`] is called at
    /// accrual start, keeping future coupon checkpoints compact.
    #[must_use]
    pub fn replay_state(&self) -> FloatingCouponReplayState {
        FloatingCouponReplayState::default()
    }

    /// Lock the term-index fixing without capturing the accrual notional.
    ///
    /// # Arguments
    ///
    /// * `state` - Mutable replay state for this compiled coupon.
    /// * `index_rate` - Observed term-index fixing as a decimal rate.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] for an overnight coupon, a non-finite
    /// rate, or a duplicate fixing.
    pub fn observe_term(
        &self,
        state: &mut FloatingCouponReplayState,
        index_rate: f64,
    ) -> Result<()> {
        if !matches!(self.observation, FloatingRateObservation::Term { .. }) {
            return Err(Error::Validation(
                "cannot apply a term fixing to an overnight coupon".to_string(),
            ));
        }
        if !index_rate.is_finite() {
            return Err(Error::Validation(format!(
                "floating term-index fixing must be finite, got {index_rate}"
            )));
        }
        if state.term_index_rate.is_some() {
            return Err(Error::Validation(
                "floating term-index fixing was supplied more than once".to_string(),
            ));
        }
        state.term_index_rate = Some(index_rate);
        Ok(())
    }

    /// Capture the principal on which this coupon accrues.
    ///
    /// # Arguments
    ///
    /// * `state` - Mutable replay state for this compiled coupon.
    /// * `notional` - Outstanding principal after all same-day accrual-start
    ///   balance events, in currency units.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] for a negative/non-finite notional or a
    /// duplicate capture, and propagates overnight accumulator validation.
    pub fn capture_notional(
        &self,
        state: &mut FloatingCouponReplayState,
        notional: f64,
    ) -> Result<()> {
        if !notional.is_finite() || notional < 0.0 {
            return Err(Error::Validation(format!(
                "floating coupon notional must be non-negative and finite, got {notional}"
            )));
        }
        if state.notional.is_some() {
            return Err(Error::Validation(
                "floating coupon notional was captured more than once".to_string(),
            ));
        }
        if let FloatingRateObservation::Overnight {
            schedule,
            day_count_basis,
            constraints,
        } = &self.observation
        {
            state.overnight = Some(schedule.accumulator(*day_count_basis, *constraints)?);
        }
        state.notional = Some(notional);
        Ok(())
    }

    /// Advance an overnight coupon through a contractual accrual date.
    ///
    /// # Arguments
    ///
    /// * `state` - Mutable replay state after notional capture.
    /// * `end` - Contractual accrual cutoff, which may lie inside the period.
    /// * `rate` - Callback returning each historical, projected, or simulated
    ///   overnight fixing as a decimal rate.
    ///
    /// # Returns
    ///
    /// The accumulated annualized index rate through `end`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] for a term coupon or an overnight coupon
    /// advanced before notional capture, and propagates observation replay
    /// validation and callback errors.
    pub fn advance_overnight<F>(
        &self,
        state: &mut FloatingCouponReplayState,
        end: Date,
        rate: F,
    ) -> Result<OvernightRateReplay>
    where
        F: FnMut(&OvernightObservationSlice) -> Result<f64>,
    {
        let FloatingRateObservation::Overnight { schedule, .. } = &self.observation else {
            return Err(Error::Validation(
                "cannot advance overnight observations for a term coupon".to_string(),
            ));
        };
        let accumulator = state.overnight.as_mut().ok_or_else(|| {
            Error::Validation(
                "overnight coupon must capture accrual-start notional before replay".to_string(),
            )
        })?;
        schedule.advance(accumulator, end, rate)
    }

    /// Settle a coupon directly from an observed index rate and notional.
    ///
    /// Deterministic schedule emission and replay state both use this method,
    /// so floor/cap, gearing, spread, and cash/PIK arithmetic cannot diverge.
    ///
    /// # Arguments
    ///
    /// * `notional` - Accrual-start outstanding principal in currency units.
    /// * `constrained_index_rate` - Decimal index rate after any overnight
    ///   daily/period index constraints; for term coupons this is the fixing.
    /// * `projected_index_rate` - Decimal index rate recorded as cashflow
    ///   metadata before overnight period constraints.
    ///
    /// # Returns
    ///
    /// All-in coupon rate and total, cash, and PIK amounts.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when an input or calculated amount is
    /// non-finite or when `notional` is negative.
    pub fn settle_index_rate(
        &self,
        notional: f64,
        constrained_index_rate: f64,
        projected_index_rate: f64,
    ) -> Result<FloatingCouponSettlement> {
        self.settle_index_rate_for_accrual(
            notional,
            constrained_index_rate,
            projected_index_rate,
            self.period.accrual_factor,
        )
    }

    /// Settle a constant-balance segment without cloning the observation schedule.
    pub(crate) fn settle_index_rate_for_accrual(
        &self,
        notional: f64,
        constrained_index_rate: f64,
        projected_index_rate: f64,
        accrual_factor: f64,
    ) -> Result<FloatingCouponSettlement> {
        if !accrual_factor.is_finite() || accrual_factor < 0.0 {
            return Err(Error::Validation(
                "floating coupon segment accrual factor must be non-negative and finite"
                    .to_string(),
            ));
        }
        if !notional.is_finite() || notional < 0.0 {
            return Err(Error::Validation(format!(
                "floating coupon notional must be non-negative and finite, got {notional}"
            )));
        }
        if !constrained_index_rate.is_finite() || !projected_index_rate.is_finite() {
            return Err(Error::Validation(
                "floating coupon index rates must be finite".to_string(),
            ));
        }
        let all_in_rate =
            calculate_floating_rate(constrained_index_rate, self.settlement_rate_params());
        let total_amount = notional * accrual_factor * all_in_rate;
        let cash_amount = total_amount * self.economics.cash_fraction;
        let pik_amount = total_amount * self.economics.pik_fraction;
        if !all_in_rate.is_finite()
            || !total_amount.is_finite()
            || !cash_amount.is_finite()
            || !pik_amount.is_finite()
        {
            return Err(Error::Validation(
                "floating coupon settlement produced a non-finite value".to_string(),
            ));
        }
        Ok(FloatingCouponSettlement {
            projected_index_rate,
            all_in_rate,
            total_amount,
            cash_amount,
            pik_amount,
        })
    }

    /// Settle this coupon from its locked replay state.
    ///
    /// # Arguments
    ///
    /// * `state` - Replay state containing the term fixing or completed
    ///   overnight accumulator and the accrual-start notional.
    ///
    /// # Returns
    ///
    /// All-in coupon rate and total, cash, and PIK amounts.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when required state is missing or overnight
    /// replay has not reached the full contractual accrual end, and
    /// propagates settlement validation.
    pub fn settle(&self, state: &FloatingCouponReplayState) -> Result<FloatingCouponSettlement> {
        let notional = state.notional.ok_or_else(|| {
            Error::Validation("floating coupon settled before notional capture".to_string())
        })?;
        match &self.observation {
            FloatingRateObservation::Term { .. } => {
                let index_rate = state.term_index_rate.ok_or_else(|| {
                    Error::Validation("floating term coupon settled before fixing".to_string())
                })?;
                self.settle_index_rate(notional, index_rate, index_rate)
            }
            FloatingRateObservation::Overnight { .. } => {
                let accumulator = state.overnight.as_ref().ok_or_else(|| {
                    Error::Validation("floating overnight coupon settled before replay".to_string())
                })?;
                if !accumulator.is_complete() {
                    return Err(Error::Validation(
                        "floating overnight coupon settled before full contractual replay".into(),
                    ));
                }
                let replay = accumulator.result()?;
                self.settle_index_rate(notional, replay.constrained_rate, replay.projected_rate)
            }
        }
    }

    /// Compute total accrued interest through an interior date.
    ///
    /// # Arguments
    ///
    /// * `state` - Replay state containing accrual-start notional and the rate
    ///   observed through the requested date.
    /// * `date` - Exercise or reporting date; dates before accrual start or on
    ///   or after both cash payment and accrual end return zero. The caller
    ///   separates cash entitlement at payment from PIK entitlement at accrual end.
    ///
    /// # Returns
    ///
    /// Total accrued coupon amount before the cash/PIK split.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] for missing active state or overnight
    /// replay through a different date, and propagates
    /// day-count and overnight replay errors.
    pub fn accrued_amount(&self, state: &FloatingCouponReplayState, date: Date) -> Result<f64> {
        if date <= self.period.accrual_start
            || date >= self.period.payment_date.max(self.period.accrual_end)
        {
            return Ok(0.0);
        }
        let notional = state.notional.ok_or_else(|| {
            Error::Validation("active floating coupon has no captured notional".to_string())
        })?;
        let elapsed_end = date.min(self.period.accrual_end);
        let mut context = self.period.day_count_context;
        context.end_is_termination_date &= elapsed_end == self.period.accrual_end;
        let elapsed = if self.period.day_count == DayCount::Act365L {
            // ACT/365L chooses its 365/366 denominator from the full coupon
            // period, including a future leap day or the contractual end year.
            // Truncating the period at a reporting date must not change it.
            super::periods::prorated_actual_accrual(
                (self.period.accrual_start, self.period.accrual_end),
                self.period.accrual_start,
                elapsed_end,
                self.period.accrual_factor,
            )?
        } else {
            self.period
                .day_count
                .year_fraction(self.period.accrual_start, elapsed_end, context)?
        };
        let index_rate = match &self.observation {
            FloatingRateObservation::Term { .. } => state.term_index_rate.ok_or_else(|| {
                Error::Validation("active floating term coupon has no fixing".to_string())
            })?,
            FloatingRateObservation::Overnight { .. } => {
                let accumulator = state.overnight.as_ref().ok_or_else(|| {
                    Error::Validation("active overnight coupon has no replay state".to_string())
                })?;
                if accumulator.accrued_through() != elapsed_end {
                    return Err(Error::Validation(
                        "overnight accrued interest requires replay through the requested accrual date".into(),
                    ));
                }
                accumulator.result()?.constrained_rate
            }
        };
        let amount =
            notional * elapsed * calculate_floating_rate(index_rate, self.settlement_rate_params());
        if amount.is_finite() {
            Ok(amount)
        } else {
            Err(Error::Validation(
                "floating coupon accrued amount is non-finite".to_string(),
            ))
        }
    }

    /// Return whether replay state currently carries a locked fixing,
    /// captured notional, or overnight accumulator.
    #[must_use]
    pub fn is_live(&self, state: &FloatingCouponReplayState) -> bool {
        state.notional.is_some() || state.term_index_rate.is_some() || state.overnight.is_some()
    }

    /// Return the captured accrual-start notional, when available.
    #[must_use]
    pub fn captured_notional(&self, state: &FloatingCouponReplayState) -> Option<f64> {
        state.notional
    }

    /// Return the locked term-index fixing, when available.
    #[must_use]
    pub fn locked_term_index_rate(&self, state: &FloatingCouponReplayState) -> Option<f64> {
        state.term_index_rate
    }

    fn settlement_rate_params(&self) -> &FloatingRateParams {
        &self.economics.rate_params
    }
}

fn validate_fraction(value: f64, label: &str) -> Result<()> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(Error::Validation(format!(
            "floating coupon {label} must be finite and in [0, 1], got {value}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::specs::{FloatingLegCompounding, OvernightIndexConstraintApplication};
    use finstack_quant_core::dates::WEEKENDS_ONLY;
    use time::macros::date;

    fn economics() -> FloatingCouponEconomics {
        FloatingCouponEconomics {
            cash_fraction: 0.4,
            pik_fraction: 0.6,
            rate_params: FloatingRateParams::with_spread(100.0),
        }
    }

    #[test]
    fn term_fixing_precedes_and_does_not_capture_accrual_notional() {
        let coupon = CompiledFloatingCoupon::compile(
            FloatingCouponPeriod {
                accrual_start: date!(2025 - 01 - 15),
                accrual_end: date!(2025 - 04 - 15),
                payment_date: date!(2025 - 04 - 15),
                day_count: DayCount::Act360,
                day_count_context: DayCountContext::default(),
                accrual_factor: 0.25,
            },
            economics(),
            FloatingRateObservation::Term {
                reset_date: date!(2025 - 01 - 13),
                tenor_years: 0.25,
            },
        )
        .expect("term coupon compiles");
        let mut state = coupon.replay_state();
        coupon
            .observe_term(&mut state, 0.03)
            .expect("reset fixes the index");
        assert_eq!(coupon.captured_notional(&state), None);

        // Principal has changed from 100 to 108 between reset and accrual
        // start. The locked rate remains 3%, while the new balance is used.
        coupon
            .capture_notional(&mut state, 108.0)
            .expect("accrual start captures balance");
        let replayed = coupon.settle(&state).expect("coupon settles");
        let projected = coupon
            .settle_index_rate(108.0, 0.03, 0.03)
            .expect("deterministic projection settles");
        assert_eq!(replayed, projected);
        assert!((replayed.all_in_rate - 0.04).abs() < 1.0e-12);
        assert!((replayed.total_amount - 1.08).abs() < 1.0e-12);
        assert!((replayed.cash_amount - 0.432).abs() < 1.0e-12);
        assert!((replayed.pik_amount - 0.648).abs() < 1.0e-12);
    }

    #[test]
    fn adjusted_payment_may_precede_unadjusted_accrual_end() {
        let coupon = CompiledFloatingCoupon::compile(
            FloatingCouponPeriod {
                accrual_start: date!(2024 - 01 - 31),
                accrual_end: date!(2024 - 03 - 31),
                payment_date: date!(2024 - 03 - 29),
                day_count: DayCount::Act360,
                day_count_context: DayCountContext::default(),
                accrual_factor: 60.0 / 360.0,
            },
            economics(),
            FloatingRateObservation::Term {
                reset_date: date!(2024 - 01 - 29),
                tenor_years: 2.0 / 12.0,
            },
        )
        .expect("business-day-adjusted payment compiles");

        let mut state = coupon.replay_state();
        coupon
            .observe_term(&mut state, 0.03)
            .expect("term rate fixes");
        coupon
            .capture_notional(&mut state, 100.0)
            .expect("notional is captured");
        let settled = coupon.settle(&state).expect("coupon settles");
        assert!((settled.total_amount - (100.0 * 60.0 / 360.0 * 0.04)).abs() < 1.0e-12);
        let after_cash_payment = coupon
            .accrued_amount(&state, date!(2024 - 03 - 30))
            .expect("PIK remains accrued until its contractual end");
        assert!((after_cash_payment - 100.0 * 59.0 / 360.0 * 0.04).abs() < 1e-12);
        assert_eq!(
            coupon
                .accrued_amount(&state, date!(2024 - 03 - 31))
                .expect("both entitlements settled"),
            0.0,
        );
    }

    #[test]
    fn overnight_incremental_state_matches_one_shot_coupon_settlement() {
        let start = date!(2025 - 01 - 06);
        let middle = date!(2025 - 01 - 09);
        let end = date!(2025 - 01 - 13);
        let observations = OvernightObservationSchedule::compile(
            start,
            end,
            FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days: 2 },
            &WEEKENDS_ONLY,
        )
        .expect("overnight observations compile");
        let constraints = OvernightRateConstraints {
            application: OvernightIndexConstraintApplication::Daily,
            index_floor_bp: Some(100.0),
            index_cap_bp: Some(600.0),
        };
        let mut overnight_economics = economics();
        overnight_economics.rate_params.index_floor_bp = constraints.index_floor_bp;
        overnight_economics.rate_params.index_cap_bp = constraints.index_cap_bp;
        let coupon = CompiledFloatingCoupon::compile(
            FloatingCouponPeriod {
                accrual_start: start,
                accrual_end: end,
                payment_date: end,
                day_count: DayCount::Act360,
                day_count_context: DayCountContext::default(),
                accrual_factor: 7.0 / 360.0,
            },
            overnight_economics,
            FloatingRateObservation::Overnight {
                schedule: observations.clone(),
                day_count_basis: 360.0,
                constraints,
            },
        )
        .expect("overnight coupon compiles");
        let observed = |slice: &OvernightObservationSlice| {
            Ok(0.02 + f64::from(slice.observation_date.day()) * 0.001)
        };
        let one_shot = observations
            .replay(end, 360.0, constraints, observed)
            .expect("one-shot replay");
        let mut state = coupon.replay_state();
        coupon
            .capture_notional(&mut state, 108.0)
            .expect("capture notional");
        coupon
            .advance_overnight(&mut state, middle, observed)
            .expect("first replay segment");
        coupon
            .advance_overnight(&mut state, end, observed)
            .expect("second replay segment");
        let replayed = coupon.settle(&state).expect("coupon settles");
        let projected = coupon
            .settle_index_rate(108.0, one_shot.constrained_rate, one_shot.projected_rate)
            .expect("one-shot coupon settles");
        assert!((replayed.projected_index_rate - projected.projected_index_rate).abs() < 1e-12);
        assert!((replayed.total_amount - projected.total_amount).abs() < 1e-12);
    }

    fn weekly_coupon(
        params: FloatingRateParams,
        application: OvernightIndexConstraintApplication,
    ) -> CompiledFloatingCoupon {
        CompiledFloatingCoupon::compile(
            FloatingCouponPeriod {
                accrual_start: date!(2025 - 01 - 06),
                accrual_end: date!(2025 - 01 - 13),
                payment_date: date!(2025 - 01 - 13),
                day_count: DayCount::Act360,
                day_count_context: DayCountContext::default(),
                accrual_factor: 7.0 / 360.0,
            },
            FloatingCouponEconomics {
                cash_fraction: 1.0,
                pik_fraction: 0.0,
                rate_params: params.clone(),
            },
            FloatingRateObservation::Overnight {
                schedule: OvernightObservationSchedule::compile(
                    date!(2025 - 01 - 06),
                    date!(2025 - 01 - 13),
                    FloatingLegCompounding::sofr(),
                    &WEEKENDS_ONLY,
                )
                .expect("observations"),
                day_count_basis: 360.0,
                constraints: OvernightRateConstraints {
                    application,
                    index_floor_bp: params.index_floor_bp,
                    index_cap_bp: params.index_cap_bp,
                },
            },
        )
        .expect("coupon")
    }

    #[test]
    fn daily_index_caps_preserve_compound_growth_and_all_in_bounds() {
        let params = FloatingRateParams {
            index_cap_bp: Some(500.0),
            ..Default::default()
        };
        let coupon = weekly_coupon(params.clone(), OvernightIndexConstraintApplication::Daily);
        let mut state = coupon.replay_state();
        coupon
            .capture_notional(&mut state, 100_000_000.0)
            .expect("notional");
        coupon
            .advance_overnight(&mut state, coupon.period().accrual_end, |_| Ok(0.06))
            .expect("replay");
        let settlement = coupon.settle(&state).expect("settlement");
        let growth = (1.0_f64 + 0.05 / 360.0).powi(4) * (1.0 + 0.05 * 3.0 / 360.0) - 1.0;
        assert!((settlement.total_amount - 100_000_000.0 * growth).abs() < 1e-7);
        assert!(settlement.all_in_rate > 0.05);

        for (application, all_in_cap, expected_rate) in [
            (OvernightIndexConstraintApplication::Period, None, 0.05),
            (
                OvernightIndexConstraintApplication::Daily,
                Some(500.0),
                0.05,
            ),
        ] {
            let coupon = weekly_coupon(
                FloatingRateParams {
                    all_in_cap_bp: all_in_cap,
                    ..params.clone()
                },
                application,
            );
            let mut state = coupon.replay_state();
            coupon
                .capture_notional(&mut state, 100.0)
                .expect("notional");
            coupon
                .advance_overnight(&mut state, coupon.period().accrual_end, |_| Ok(0.06))
                .expect("replay");
            assert!(
                (coupon.settle(&state).expect("settlement").all_in_rate - expected_rate).abs()
                    < 1e-12
            );
        }
    }

    #[test]
    fn negative_daily_index_floor_and_all_in_floor_remain_distinct() {
        let coupon = weekly_coupon(
            FloatingRateParams {
                index_floor_bp: Some(-100.0),
                ..Default::default()
            },
            OvernightIndexConstraintApplication::Daily,
        );
        let mut state = coupon.replay_state();
        coupon
            .capture_notional(&mut state, 100.0)
            .expect("notional");
        coupon
            .advance_overnight(&mut state, coupon.period().accrual_end, |_| Ok(-0.02))
            .expect("replay");
        let settlement = coupon.settle(&state).expect("settlement");
        let growth = (1.0_f64 - 0.01 / 360.0).powi(4) * (1.0 - 0.01 * 3.0 / 360.0) - 1.0;
        assert!((settlement.total_amount - 100.0 * growth).abs() < 1e-12);
        let coupon = weekly_coupon(
            FloatingRateParams {
                index_floor_bp: Some(-100.0),
                all_in_floor_bp: Some(0.0),
                ..Default::default()
            },
            OvernightIndexConstraintApplication::Daily,
        );
        let mut state = coupon.replay_state();
        coupon
            .capture_notional(&mut state, 100.0)
            .expect("notional");
        coupon
            .advance_overnight(&mut state, coupon.period().accrual_end, |_| Ok(-0.02))
            .expect("replay");
        assert_eq!(coupon.settle(&state).expect("settlement").total_amount, 0.0);
    }

    #[test]
    fn unfinished_overnight_coupons_cannot_settle() {
        let coupon = weekly_coupon(
            FloatingRateParams::with_spread(100.0),
            OvernightIndexConstraintApplication::Daily,
        );
        let mut state = coupon.replay_state();
        coupon
            .capture_notional(&mut state, 100.0)
            .expect("notional");
        assert!(coupon.settle(&state).is_err());
        coupon
            .advance_overnight(&mut state, date!(2025 - 01 - 07), |_| Ok(0.03))
            .expect("partial replay");
        assert!(coupon.settle(&state).is_err());
        coupon
            .advance_overnight(&mut state, coupon.period().accrual_end, |_| Ok(0.03))
            .expect("full replay");
        assert!(coupon.settle(&state).is_ok());
    }

    #[test]
    fn overnight_accrued_interest_rejects_stale_and_future_replay_state() {
        let coupon = weekly_coupon(
            Default::default(),
            OvernightIndexConstraintApplication::Daily,
        );
        let mut state = coupon.replay_state();
        coupon
            .capture_notional(&mut state, 100.0)
            .expect("notional");
        coupon
            .advance_overnight(&mut state, date!(2025 - 01 - 08), |_| Ok(0.03))
            .expect("partial replay");
        assert!(coupon
            .accrued_amount(&state, date!(2025 - 01 - 07))
            .is_err());
        assert!(coupon
            .accrued_amount(&state, date!(2025 - 01 - 09))
            .is_err());
        assert!(coupon.accrued_amount(&state, date!(2025 - 01 - 08)).is_ok());
    }

    #[test]
    fn contradictory_daily_bounds_and_inconsistent_accrual_are_rejected() {
        let coupon = weekly_coupon(
            FloatingRateParams {
                index_cap_bp: Some(500.0),
                ..Default::default()
            },
            OvernightIndexConstraintApplication::Daily,
        );
        let mut economics = coupon.economics().clone();
        economics.rate_params.index_cap_bp = Some(400.0);
        assert!(CompiledFloatingCoupon::compile(
            coupon.period(),
            economics,
            coupon.observation().clone()
        )
        .is_err());
        let mut period = coupon.period();
        period.accrual_factor *= 2.0;
        assert!(CompiledFloatingCoupon::compile(
            period,
            coupon.economics().clone(),
            coupon.observation().clone()
        )
        .is_err());
    }

    #[test]
    fn duplicate_fixing_and_notional_errors_preserve_locked_state() {
        let coupon = CompiledFloatingCoupon::compile(
            FloatingCouponPeriod {
                accrual_start: date!(2025 - 01 - 06),
                accrual_end: date!(2025 - 01 - 13),
                payment_date: date!(2025 - 01 - 13),
                day_count: DayCount::Act360,
                day_count_context: DayCountContext::default(),
                accrual_factor: 7.0 / 360.0,
            },
            economics(),
            FloatingRateObservation::Term {
                reset_date: date!(2025 - 01 - 06),
                tenor_years: 0.25,
            },
        )
        .expect("coupon");
        let mut state = coupon.replay_state();
        coupon.observe_term(&mut state, 0.03).expect("fixing");
        coupon
            .capture_notional(&mut state, 100.0)
            .expect("notional");
        let before = coupon.settle(&state).expect("settlement");
        assert!(coupon.observe_term(&mut state, 0.09).is_err());
        assert!(coupon.capture_notional(&mut state, 200.0).is_err());
        assert_eq!(coupon.settle(&state).expect("settlement"), before);
        assert_eq!(coupon.locked_term_index_rate(&state), Some(0.03));
        assert_eq!(coupon.captured_notional(&state), Some(100.0));
    }

    #[test]
    fn act365l_balance_segment_uses_its_stored_contractual_denominator() {
        let period = FloatingCouponPeriod {
            accrual_start: date!(2023 - 12 - 01),
            accrual_end: date!(2023 - 12 - 15),
            payment_date: date!(2024 - 03 - 01),
            day_count: DayCount::Act365L,
            day_count_context: DayCountContext {
                frequency: Some(finstack_quant_core::dates::Tenor::quarterly()),
                ..Default::default()
            },
            // The original coupon ends in leap year 2024, although this
            // balance segment ends before the year boundary.
            accrual_factor: 14.0 / 366.0,
        };
        let economics = FloatingCouponEconomics {
            cash_fraction: 1.0,
            pik_fraction: 0.0,
            rate_params: Default::default(),
        };
        let observation = FloatingRateObservation::Term {
            reset_date: period.accrual_start,
            tenor_years: 0.25,
        };
        let coupon =
            CompiledFloatingCoupon::compile(period, economics.clone(), observation.clone())
                .expect("stored contractual leap denominator");
        let mut state = coupon.replay_state();
        coupon.observe_term(&mut state, 0.05).expect("fixing");
        coupon
            .capture_notional(&mut state, 100.0)
            .expect("notional");
        assert!(
            (coupon
                .accrued_amount(&state, date!(2023 - 12 - 08))
                .expect("partial accrual")
                - 100.0 * 0.05 * 7.0 / 366.0)
                .abs()
                < 1e-12
        );
        let mut invalid_period = period;
        invalid_period.accrual_factor = 14.0 / 364.0;
        assert!(CompiledFloatingCoupon::compile(invalid_period, economics, observation).is_err());
    }

    #[test]
    fn act365l_accrued_interest_keeps_the_full_coupon_leap_denominator() {
        use finstack_quant_core::dates::Tenor;
        for (start, end, cutoff, frequency, elapsed_days) in [
            (
                date!(2023 - 03 - 01),
                date!(2024 - 03 - 01),
                date!(2023 - 04 - 01),
                Tenor::annual(),
                31.0,
            ),
            (
                date!(2023 - 12 - 01),
                date!(2024 - 03 - 01),
                date!(2023 - 12 - 15),
                Tenor::quarterly(),
                14.0,
            ),
        ] {
            let context = DayCountContext {
                frequency: Some(frequency),
                coupon_period: Some((start, end)),
                ..Default::default()
            };
            let coupon = CompiledFloatingCoupon::compile(
                FloatingCouponPeriod {
                    accrual_start: start,
                    accrual_end: end,
                    payment_date: end,
                    day_count: DayCount::Act365L,
                    day_count_context: context,
                    accrual_factor: DayCount::Act365L
                        .year_fraction(start, end, context)
                        .expect("full accrual"),
                },
                FloatingCouponEconomics {
                    cash_fraction: 1.0,
                    pik_fraction: 0.0,
                    rate_params: Default::default(),
                },
                FloatingRateObservation::Term {
                    reset_date: start,
                    tenor_years: frequency.to_years(),
                },
            )
            .expect("coupon");
            let mut state = coupon.replay_state();
            coupon.observe_term(&mut state, 0.05).expect("fixing");
            coupon
                .capture_notional(&mut state, 100_000_000.0)
                .expect("notional");
            let expected = 100_000_000.0 * 0.05 * elapsed_days / 366.0;
            assert!(
                (coupon
                    .accrued_amount(&state, cutoff)
                    .expect("partial accrual")
                    - expected)
                    .abs()
                    < 1e-7
            );
        }
    }

    #[test]
    fn accrued_interest_retains_frequency_reference_period_and_calendar() {
        use finstack_quant_core::dates::Tenor;
        let start = date!(2024 - 03 - 01);
        let end = date!(2024 - 06 - 01);
        let cutoff = date!(2024 - 04 - 01);
        let context = DayCountContext {
            calendar: Some(&WEEKENDS_ONLY),
            frequency: Some(Tenor::quarterly()),
            coupon_period: Some((start, end)),
            ..Default::default()
        };
        for day_count in [DayCount::Act365L, DayCount::ActActIsma, DayCount::Bus252] {
            let coupon = CompiledFloatingCoupon::compile(
                FloatingCouponPeriod {
                    accrual_start: start,
                    accrual_end: end,
                    payment_date: end,
                    day_count,
                    day_count_context: context,
                    accrual_factor: day_count
                        .year_fraction(start, end, context)
                        .expect("full accrual"),
                },
                FloatingCouponEconomics {
                    cash_fraction: 1.0,
                    pik_fraction: 0.0,
                    rate_params: Default::default(),
                },
                FloatingRateObservation::Term {
                    reset_date: start,
                    tenor_years: 0.25,
                },
            )
            .expect("coupon");
            let mut state = coupon.replay_state();
            coupon.observe_term(&mut state, 0.05).expect("fixing");
            coupon
                .capture_notional(&mut state, 100_000_000.0)
                .expect("notional");
            let expected = match day_count {
                DayCount::Act365L => 100_000_000.0 * 0.05 * 31.0 / 366.0,
                DayCount::ActActIsma => 100_000_000.0 * 0.05 * 31.0 / (92.0 * 4.0),
                DayCount::Bus252 => 100_000_000.0 * 0.05 * 21.0 / 252.0,
                _ => unreachable!("test conventions"),
            };
            assert!(
                (coupon
                    .accrued_amount(&state, cutoff)
                    .expect("partial accrual")
                    - expected)
                    .abs()
                    < 1e-7
            );
        }
    }
}
