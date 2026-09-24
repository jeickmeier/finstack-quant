//! Structural validation of a [`RevolvingCredit`] contract.
//!
//! `RevolvingCredit::validate` runs on every builder `build()` and on
//! `Instrument::validate_invariants`; it checks amounts, dates, dated step
//! schedules, fee tiers, the coupon specification and the draw/repay or
//! stochastic utilization specification.

use finstack_quant_core::dates::{calendar_by_id, Date};
use rust_decimal::Decimal;

use super::types::{
    BaseRateSpec, CreditSpreadProcessSpec, DrawRepaySpec, RevolvingCredit, UtilizationProcess,
};
use crate::cashflow::builder::FeeTier;
use crate::instruments::common_impl::validation;
use crate::instruments::fixed_income::loan_terms::UpfrontFee;

/// Validate a dated step schedule: strictly increasing, after the commitment
/// date and, depending on `allow_maturity`, on or strictly before maturity.
fn validate_step_dates(
    dates: impl Iterator<Item = Date>,
    context: &str,
    commitment_date: Date,
    maturity: Date,
    allow_maturity: bool,
) -> finstack_quant_core::Result<()> {
    let mut previous: Option<Date> = None;
    for (index, date) in dates.enumerate() {
        let inside = date > commitment_date
            && if allow_maturity {
                date <= maturity
            } else {
                date < maturity
            };
        validation::require_with(inside, || {
            format!(
                "RevolvingCredit {context}[{index}] dated {date} must lie after the commitment \
                 date {commitment_date} and {} maturity {maturity}",
                if allow_maturity {
                    "on or before"
                } else {
                    "before"
                }
            )
        })?;
        if let Some(prev) = previous {
            validation::require_with(date > prev, || {
                format!(
                    "RevolvingCredit {context} dates must be strictly increasing: [{index}] \
                     {date} <= [{}] {prev}",
                    index - 1
                )
            })?;
        }
        previous = Some(date);
    }
    Ok(())
}

/// Validate that fee tiers are sorted by threshold in strictly ascending order.
///
/// Fee tier evaluation picks the highest tier where utilization >= threshold,
/// so tiers must be strictly ascending for the algorithm to work correctly.
/// Duplicate thresholds are rejected because the first would be unreachable.
fn validate_fee_tier_ordering(tiers: &[FeeTier], context: &str) -> finstack_quant_core::Result<()> {
    for (index, tier) in tiers.iter().enumerate() {
        if !(Decimal::ZERO..=Decimal::ONE).contains(&tier.threshold) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "RevolvingCredit {context}[{index}].threshold must be in [0, 1], got {}",
                tier.threshold
            )));
        }
        if tier.bp < Decimal::ZERO {
            return Err(finstack_quant_core::Error::Validation(format!(
                "RevolvingCredit {context}[{index}].bp must be non-negative, got {}",
                tier.bp
            )));
        }
    }
    for i in 1..tiers.len() {
        if tiers[i].threshold <= tiers[i - 1].threshold {
            return Err(finstack_quant_core::Error::Validation(format!(
                "RevolvingCredit {} must be sorted by threshold strictly ascending: \
                 tier[{}].threshold ({}) <= tier[{}].threshold ({})",
                context,
                i,
                tiers[i].threshold,
                i - 1,
                tiers[i - 1].threshold
            )));
        }
    }
    Ok(())
}

impl RevolvingCredit {
    /// Validate all structural invariants of the revolving credit facility.
    ///
    /// Checks:
    /// - Commitment amount is positive
    /// - Drawn amount does not exceed commitment
    /// - Currency consistency between drawn and commitment amounts
    /// - Commitment date is before maturity date
    /// - Recovery rate is finite and in [0, 1]
    /// - Fee tiers are sorted by threshold ascending
    /// - Base rate fixed rate is finite
    ///
    /// # Errors
    ///
    /// Returns a validation error describing the first failed check.
    ///
    /// # Example
    ///
    /// ```text
    /// let facility = RevolvingCredit::builder()
    ///     .id("RCF-001".into())
    ///     // ... other fields ...
    ///     .recovery_rate(0.40)
    ///     .build()?;
    /// facility.validate()?; // Validates all parameters
    /// ```
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        use super::MAX_RECOVERY_RATE;

        self.pricing_model_override()?;

        // Commitment amount must be positive
        validation::validate_money_gt(
            self.commitment_amount,
            0.0,
            "RevolvingCredit commitment_amount",
        )?;
        validation::validate_money_finite(self.drawn_amount, "RevolvingCredit drawn_amount")?;

        // Drawn amount must be non-negative (check before relationship check
        // so a negative drawn_amount is reported clearly rather than passing
        // the drawn <= commitment check vacuously)
        validation::require_with(self.drawn_amount.amount() >= 0.0, || {
            format!(
                "RevolvingCredit drawn_amount must be non-negative, got {}",
                self.drawn_amount
            )
        })?;

        // Drawn amount must not exceed commitment
        validation::require_with(
            self.drawn_amount.amount() <= self.commitment_amount.amount(),
            || {
                format!(
                    "RevolvingCredit drawn_amount ({}) must not exceed commitment_amount ({})",
                    self.drawn_amount, self.commitment_amount
                )
            },
        )?;

        // Currency consistency
        validation::validate_money_currency(
            self.drawn_amount,
            self.commitment_amount.currency(),
            "RevolvingCredit drawn_amount currency must match commitment_amount",
        )?;

        // Date ordering: commitment must be before maturity
        validation::validate_date_range_strict_with(
            self.commitment_date,
            self.maturity,
            |start, end| {
                format!(
                    "RevolvingCredit commitment_date ({}) must be before maturity ({})",
                    start, end
                )
            },
        )?;

        // Recovery is an explicit decimal fraction. Downstream hazard mappings
        // handle the full-recovery boundary without changing the stored input.
        validation::require_with(
            self.recovery_rate.is_finite()
                && self.recovery_rate >= 0.0
                && self.recovery_rate <= MAX_RECOVERY_RATE,
            || {
                format!(
                    "RevolvingCredit recovery_rate must be a finite decimal in [0, {}], got {}",
                    MAX_RECOVERY_RATE, self.recovery_rate
                )
            },
        )?;

        validation::require_with(
            self.leq.is_finite() && (0.0..=1.0).contains(&self.leq),
            || {
                format!(
                    "RevolvingCredit leq must be a finite decimal in [0, 1], got {}",
                    self.leq
                )
            },
        )?;

        if let Some(calendar_id) = self.calendar_id.as_deref() {
            validation::require_with(calendar_by_id(calendar_id).is_some(), || {
                format!(
                    "RevolvingCredit {} unknown calendar_id '{calendar_id}'",
                    self.id
                )
            })?;
        }

        validate_step_dates(
            self.commitment_schedule.iter().map(|step| step.date),
            "commitment_schedule",
            self.commitment_date,
            self.maturity,
            true,
        )?;
        for (index, step) in self.commitment_schedule.iter().enumerate() {
            validation::validate_money_finite(
                step.amount,
                &format!("RevolvingCredit commitment_schedule[{index}].amount"),
            )?;
            validation::require_with(step.amount.amount() >= 0.0, || {
                format!(
                    "RevolvingCredit commitment_schedule[{index}].amount must be non-negative \
                     (zero ends availability), got {}",
                    step.amount
                )
            })?;
            validation::validate_money_currency(
                step.amount,
                self.commitment_amount.currency(),
                "RevolvingCredit commitment_schedule amount currency",
            )?;
            validation::validate_f64_non_negative(
                step.fee_bp,
                &format!("RevolvingCredit commitment_schedule[{index}].fee_bp"),
            )?;
        }
        if let Some(lc) = &self.lc {
            validation::validate_money_gt(lc.sublimit, 0.0, "RevolvingCredit lc.sublimit")?;
            validation::validate_money_currency(
                lc.sublimit,
                self.commitment_amount.currency(),
                "RevolvingCredit lc.sublimit currency",
            )?;
            validation::validate_money_finite(lc.outstanding, "RevolvingCredit lc.outstanding")?;
            validation::validate_money_currency(
                lc.outstanding,
                self.commitment_amount.currency(),
                "RevolvingCredit lc.outstanding currency",
            )?;
            validation::require_with(
                lc.outstanding.amount() >= 0.0 && lc.outstanding.amount() <= lc.sublimit.amount(),
                || {
                    format!(
                        "RevolvingCredit lc.outstanding ({}) must lie in [0, sublimit {}]",
                        lc.outstanding, lc.sublimit
                    )
                },
            )?;
            validation::require_with(
                self.drawn_amount.amount() + lc.outstanding.amount()
                    <= self.commitment_amount.amount() + 1e-9,
                || {
                    format!(
                        "RevolvingCredit drawn_amount ({}) plus lc.outstanding ({}) must not \
                         exceed commitment_amount ({})",
                        self.drawn_amount, lc.outstanding, self.commitment_amount
                    )
                },
            )?;
            validation::require_with(lc.leq.is_finite() && (0.0..=1.0).contains(&lc.leq), || {
                format!(
                    "RevolvingCredit lc.leq must be a finite decimal in [0, 1], got {}",
                    lc.leq
                )
            })?;
            validation::validate_f64_non_negative(
                lc.fronting_fee_bp,
                "RevolvingCredit lc.fronting_fee_bp",
            )?;
            match (lc.fee_bp, &self.base_rate_spec) {
                (Some(fee_bp), _) => {
                    validation::validate_f64_non_negative(fee_bp, "RevolvingCredit lc.fee_bp")?;
                }
                (None, BaseRateSpec::Fixed { .. }) => {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "RevolvingCredit {}: lc.fee_bp is required on a fixed-rate facility (no \
                         floating margin to default to)",
                        self.id
                    )));
                }
                (None, BaseRateSpec::Floating(_)) => {}
            }
            let mut outstanding = lc.outstanding.amount();
            let mut previous: Option<Date> = None;
            for (index, event) in lc.events.iter().enumerate() {
                validation::require_with(
                    event.date > self.commitment_date && event.date <= self.maturity,
                    || {
                        format!(
                            "RevolvingCredit lc.events[{index}] dated {} must be after the \
                             commitment date and on or before maturity",
                            event.date
                        )
                    },
                )?;
                if let Some(prev) = previous {
                    validation::require_with(event.date >= prev, || {
                        format!("RevolvingCredit lc.events must be sorted by date: [{index}] {} < {prev}", event.date)
                    })?;
                }
                previous = Some(event.date);
                validation::validate_money_gt(
                    event.amount,
                    0.0,
                    "RevolvingCredit lc.events amount",
                )?;
                validation::validate_money_currency(
                    event.amount,
                    self.commitment_amount.currency(),
                    "RevolvingCredit lc.events amount currency",
                )?;
                outstanding += if event.is_issue {
                    event.amount.amount()
                } else {
                    -event.amount.amount()
                };
                validation::require_with(
                    outstanding >= -1e-9 && outstanding <= lc.sublimit.amount() + 1e-9,
                    || {
                        format!(
                            "RevolvingCredit lc.events[{index}] on {} takes the LC outstanding to {}, \
                             outside [0, sublimit {}]",
                            event.date, outstanding, lc.sublimit
                        )
                    },
                )?;
            }
        }

        validate_step_dates(
            self.margin_steps.iter().map(|step| step.date),
            "margin_steps",
            self.commitment_date,
            self.maturity,
            false,
        )?;
        validate_step_dates(
            self.fees.steps.iter().map(|step| step.date),
            "fees.steps",
            self.commitment_date,
            self.maturity,
            false,
        )?;
        for (index, step) in self.fees.steps.iter().enumerate() {
            for (name, value) in [
                ("commitment_delta_bp", step.commitment_delta_bp),
                ("usage_delta_bp", step.usage_delta_bp),
                ("facility_delta_bp", step.facility_delta_bp),
            ] {
                validation::validate_f64_finite(
                    value,
                    &format!("RevolvingCredit fees.steps[{index}].{name}"),
                )?;
            }
        }

        // Validate fee tier ordering: thresholds must be strictly ascending
        validate_fee_tier_ordering(&self.fees.commitment_fee_tiers, "commitment_fee_tiers")?;
        validate_fee_tier_ordering(&self.fees.usage_fee_tiers, "usage_fee_tiers")?;

        match &self.fees.upfront_fee {
            Some(UpfrontFee::Amount(upfront_fee)) => {
                validation::validate_money_finite(*upfront_fee, "RevolvingCredit upfront_fee")?;
                validation::validate_money_currency(
                    *upfront_fee,
                    self.commitment_amount.currency(),
                    "RevolvingCredit upfront_fee currency",
                )?;
                validation::require_with(upfront_fee.amount() >= 0.0, || {
                    format!(
                        "RevolvingCredit upfront_fee must be non-negative, got {}",
                        upfront_fee
                    )
                })?;
            }
            Some(UpfrontFee::PctOfCommitment(pct)) => {
                validation::require_with(pct.is_finite() && (0.0..=1.0).contains(pct), || {
                    format!(
                        "RevolvingCredit upfront_fee percentage must be a finite decimal in \
                         [0, 1], got {pct}"
                    )
                })?;
            }
            None => {}
        }

        for (index, fee) in self.scheduled_fees.iter().enumerate() {
            validation::require_with(
                fee.date > self.commitment_date && fee.date <= self.maturity,
                || {
                    format!(
                        "RevolvingCredit scheduled_fees[{index}] dated {} must lie after the \
                         commitment date and on or before maturity",
                        fee.date
                    )
                },
            )?;
            validation::validate_money_finite(fee.amount, "RevolvingCredit scheduled_fees amount")?;
            validation::validate_money_currency(
                fee.amount,
                self.commitment_amount.currency(),
                "RevolvingCredit scheduled_fees amount currency",
            )?;
            validation::require_with(fee.amount.amount() >= 0.0, || {
                format!(
                    "RevolvingCredit scheduled_fees[{index}] must be non-negative, got {}",
                    fee.amount
                )
            })?;
        }

        // Validate facility fee is non-negative
        validation::validate_f64_non_negative(
            self.fees.facility_fee_bp,
            "RevolvingCredit facility_fee_bp",
        )?;

        // Validate the complete coupon specification through the same canonical
        // conversion used by the cashflow engine. This keeps the public
        // validation/JSON boundary aligned with pricing for gearing, decimal
        // conversion, and index/all-in floor-cap ordering.
        match &self.base_rate_spec {
            BaseRateSpec::Fixed { rate } => {
                validation::validate_f64_finite(*rate, "RevolvingCredit fixed base rate")?;
            }
            BaseRateSpec::Floating(spec) => {
                validation::require_with(spec.gearing > Decimal::ZERO, || {
                    format!(
                        "RevolvingCredit floating gearing must be positive, got {}",
                        spec.gearing
                    )
                })?;
                if let (Some(floor), Some(cap)) = (spec.index_floor_bp, spec.index_cap_bp) {
                    validation::require_with(floor <= cap, || {
                        format!(
                            "RevolvingCredit index_floor_bp ({floor}) must not exceed index_cap_bp ({cap})"
                        )
                    })?;
                }
                if let (Some(floor), Some(cap)) = (spec.all_in_floor_bp, spec.all_in_cap_bp) {
                    validation::require_with(floor <= cap, || {
                        format!(
                            "RevolvingCredit all_in_floor_bp ({floor}) must not exceed all_in_cap_bp ({cap})"
                        )
                    })?;
                }
                // The revolver projects every reset from the forward curve
                // and has no fallback path; reject fields it would ignore.
                validation::require_with(spec.index_tenor.is_none(), || {
                    "RevolvingCredit does not support index_tenor: resets use the \
                     forward curve's own tenor"
                        .to_string()
                })?;
                validation::require_with(spec.fallback.is_default(), || {
                    "RevolvingCredit does not support a floating-rate fallback: a missing \
                     curve or fixing is an error"
                        .to_string()
                })?;
                let _ = crate::cashflow::builder::FloatingRateParams::try_from(spec)?;
                let _ = crate::instruments::common_impl::pricing::overnight_conventions::resolved_overnight_compounding(
                    spec.index_id.as_str(),
                    spec.overnight_compounding.as_ref(),
                )?;
            }
        }

        match &self.draw_repay_spec {
            DrawRepaySpec::Deterministic(events) => {
                let mut events = events.iter().collect::<Vec<_>>();
                events.sort_by_key(|event| event.date);
                let mut balance = self.drawn_amount.amount();
                for event in &events {
                    validation::require_with(
                        event.date > self.commitment_date && event.date <= self.maturity,
                        || {
                            format!(
                                "RevolvingCredit draw/repay event dated {} must be after \
                                 commitment ({}) and on or before maturity ({})",
                                event.date, self.commitment_date, self.maturity
                            )
                        },
                    )?;
                    validation::validate_money_finite(
                        event.amount,
                        "RevolvingCredit draw/repay amount",
                    )?;
                    validation::validate_money_gt(
                        event.amount,
                        0.0,
                        "RevolvingCredit draw/repay amount",
                    )?;
                    validation::validate_money_currency(
                        event.amount,
                        self.commitment_amount.currency(),
                        "RevolvingCredit draw/repay amount currency",
                    )?;
                    if event.is_draw {
                        balance += event.amount.amount();
                        let commitment = self.commitment_at(event.date);
                        validation::require_with(balance <= commitment.amount(), || {
                            format!(
                                "RevolvingCredit draw on {} would increase balance to {}, \
                                 above commitment {}",
                                event.date, balance, commitment
                            )
                        })?;
                    } else {
                        validation::require_with(event.amount.amount() <= balance, || {
                            format!(
                                "RevolvingCredit repayment on {} of {} exceeds current balance {}",
                                event.date, event.amount, balance
                            )
                        })?;
                        balance -= event.amount.amount();
                    }
                }
                // A step down must not leave the balance above the new
                // commitment: the analyst dates the repayment.
                for step in &self.commitment_schedule {
                    let balance_at_step = self
                        .drawn_balance_at(events.iter().copied(), self.commitment_date, step.date)?
                        .amount();
                    validation::require_with(balance_at_step <= step.amount.amount(), || {
                        format!(
                            "RevolvingCredit commitment step on {} to {} is below the drawn \
                             balance {} on that date; schedule a repayment on or before it",
                            step.date, step.amount, balance_at_step
                        )
                    })?;
                }
            }
            DrawRepaySpec::Stochastic(spec) => {
                validation::require_with(spec.num_paths >= 2, || {
                    format!(
                        "RevolvingCredit stochastic num_paths must be at least 2, got {}",
                        spec.num_paths
                    )
                })?;
                // Antithetic pairing is defined for pseudorandom draws only;
                // negating Sobol points destroys the low-discrepancy
                // structure. Reject the combination rather than silently
                // dropping the antithetic flag.
                validation::require_with(!(spec.antithetic && spec.use_sobol_qmc), || {
                    "RevolvingCredit stochastic spec cannot combine antithetic \
                     variance reduction with Sobol QMC; disable one of the two flags"
                        .to_string()
                })?;
                match &spec.utilization_process {
                    UtilizationProcess::MeanReverting {
                        target_rate,
                        speed,
                        volatility,
                        spread_sensitivity,
                    } => {
                        validation::require_with(
                            target_rate.is_finite() && (0.0..=1.0).contains(target_rate),
                            || {
                                format!(
                                    "RevolvingCredit utilization target_rate must be finite and \
                                     in [0, 1], got {target_rate}"
                                )
                            },
                        )?;
                        validation::validate_f64_positive(
                            *speed,
                            "RevolvingCredit utilization speed",
                        )?;
                        validation::validate_f64_non_negative(
                            *volatility,
                            "RevolvingCredit utilization volatility",
                        )?;
                        validation::require_with(spread_sensitivity.is_finite(), || {
                            format!(
                                "RevolvingCredit utilization spread_sensitivity must be finite, \
                                 got {spread_sensitivity}"
                            )
                        })?;
                    }
                }
                if let Some(mc_config) = &spec.mc_config {
                    mc_config.validate()?;
                    // A hazard curve on the facility is both the CS01 bump
                    // target and the anchor for pathwise survival. Any other
                    // spread process ignores the curve, so a hazard bump would
                    // reprice to the same PV and report a silent zero CS01.
                    if let Some(curve) = &self.credit_curve_id {
                        match &mc_config.credit_spread_process {
                            CreditSpreadProcessSpec::MarketAnchored {
                                credit_curve_id, ..
                            } if credit_curve_id == curve => {}
                            CreditSpreadProcessSpec::MarketAnchored {
                                credit_curve_id, ..
                            } => {
                                return Err(finstack_quant_core::Error::Validation(format!(
                                    "RevolvingCredit {}: McConfig market-anchored credit curve \
                                     '{}' must equal the facility credit_curve_id '{}'",
                                    self.id, credit_curve_id, curve
                                )));
                            }
                            other => {
                                return Err(finstack_quant_core::Error::Validation(format!(
                                    "RevolvingCredit {}: credit_curve_id '{}' requires \
                                     CreditSpreadProcessSpec::MarketAnchored on that curve; the \
                                     supplied '{}' process ignores the curve, so hazard CS01 \
                                     would silently report zero. Drop credit_curve_id to price \
                                     on an explicit spread process",
                                    self.id,
                                    curve,
                                    other.kind()
                                )));
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
