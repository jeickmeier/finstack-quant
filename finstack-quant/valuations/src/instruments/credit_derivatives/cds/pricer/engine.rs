use super::config::CDSPricerConfig;
use super::helpers::{
    date_from_hazard_time, df_asof_to, haz_t, isda_standard_model_boundaries, settlement_date,
    sp_cond_to, validate_recovery_consistency,
};
use crate::constants::{credit, numerical, BASIS_POINTS_PER_UNIT};
use crate::instruments::common_impl::helpers::year_fraction;
use crate::instruments::credit_derivatives::cds::{CdsValuationConvention, CreditDefaultSwap};
use finstack_quant_core::dates::{Date, HolidayCalendar};
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_core::{Error, Result};
use rust_decimal::prelude::ToPrimitive;

/// CDS pricing engine. Stateless wrapper carrying configuration.
#[derive(Debug)]
pub(crate) struct CDSPricer {
    pub(super) config: CDSPricerConfig,
}

#[derive(Clone, Copy)]
pub(super) struct AodInputs<'a> {
    pub(super) cds: &'a CreditDefaultSwap,
    pub(super) spread: f64,
    /// Date from which premium accrual is measured for the AoD integral.
    /// For spot CDS pricing this is the coupon period start. For forward
    /// CDS pricing this is clamped to the forward protection start, because
    /// defaults before the forward start cancel the forward CDS rather than
    /// accruing premium.
    pub(super) accrual_start_date: Date,
    /// Lower bound of the default-time integration interval. Always
    /// `>= as_of` (defaults strictly before `as_of` are not integrated).
    pub(super) start_date: Date,
    pub(super) end_date: Date,
    pub(super) settlement_delay: u16,
    pub(super) calendar: Option<&'a dyn HolidayCalendar>,
    pub(super) as_of: Date,
    pub(super) disc: &'a DiscountCurve,
    pub(super) surv: &'a HazardCurve,
}

#[derive(Clone, Copy)]
pub(super) struct CouponPeriod {
    pub(super) accrual_start: Date,
    pub(super) accrual_end: Date,
    pub(super) payment_date: Date,
    pub(super) is_final: bool,
}

impl Default for CDSPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl CDSPricer {
    /// Create new pricer with default ISDA-compliant config.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            config: CDSPricerConfig::default(),
        }
    }

    /// Create pricer with custom config.
    #[must_use]
    pub(crate) fn with_config(config: CDSPricerConfig) -> Self {
        Self { config }
    }

    /// Calculate PV of protection leg using ISDA Standard Model integration.
    ///
    /// The protection leg represents the contingent payment made by the
    /// protection seller upon a credit event. Its present value is:
    ///
    /// ```text
    /// PV_prot = (1 - R) × ∫ DF(t + delay) × (-dS(t)) dt
    /// ```
    ///
    /// where R is the recovery rate, DF is the discount factor, S is the
    /// survival probability, and delay is the settlement delay in years.
    /// Calculate PV of protection leg (Money)
    pub(crate) fn pv_protection_leg(
        &self,
        cds: &CreditDefaultSwap,
        disc: &DiscountCurve,
        surv: &HazardCurve,
        as_of: Date,
    ) -> Result<Money> {
        let pv = self.pv_protection_leg_raw(cds, disc, surv, as_of)?;
        Ok(Money::new(pv, cds.notional.currency()))
    }

    /// Calculate PV of protection leg (raw f64)
    ///
    /// Uses proper time-axis conventions:
    /// - Times are computed using the hazard curve's day-count convention
    /// - Survival probabilities are conditional on no default before `as_of`
    /// - Discounting uses the discount curve (times mapped from hazard curve axis)
    ///
    /// # Panics
    ///
    /// This method assumes the CDS has been validated at construction time.
    /// Recovery rate is expected to be in [0, 1]. Invalid recovery rates will
    /// produce incorrect results without error.
    pub(crate) fn pv_protection_leg_raw(
        &self,
        cds: &CreditDefaultSwap,
        disc: &DiscountCurve,
        surv: &HazardCurve,
        as_of: Date,
    ) -> Result<f64> {
        // Note: Recovery rate validation is performed at CDS construction time.
        // All public constructors (builder, new_isda) call validate().
        //
        // Additionally enforce that the trade-spec recovery agrees with the
        // recovery used to bootstrap the hazard curve. The ISDA Standard Model
        // requires the same R in both legs; mismatched recoveries silently
        // mis-scale the protection leg (1 − R) factor.
        validate_recovery_consistency(cds.protection.recovery_rate, surv)?;

        // Protection leg covers the period from protection start to premium end.
        // For forward-starting CDS, protection begins at protection_effective_date
        // (which may be later than the premium leg start).
        // We only value protection from as_of onwards (can't protect against past defaults).
        //
        // **Audit P3c**: standard CDS (single-name, CDX, iTraxx) terminate
        // protection and premium on the same date, which is why we read
        // `cds.premium.end` here. `ProtectionLegSpec`
        // (see `common_impl::parameters::legs`) carries no
        // `end_date` field today. Bespoke contracts that need a separate
        // protection termination (contingent CDS, amortising structures
        // with shrinking notional schedules) would have to extend
        // `ProtectionLegSpec` with an explicit `end_date: Option<Date>`
        // and prefer it here when present.
        // Protection step-in: ISDA-standard conventions step protection in at
        // T+1 calendar (a default on the valuation date itself is not
        // covered); Bloomberg CDSW/CDSO conventions integrate from the
        // valuation date ("protection starts immediately"). See
        // `CdsValuationConvention::protection_step_in_days`.
        let step_in = as_of
            + finstack_quant_core::dates::Duration::days(
                cds.valuation_convention.protection_step_in_days(),
            );
        let protection_start = step_in.max(cds.protection_start());
        let protection_end = cds.premium.end;

        // Expired contract: protection ended on or before the valuation date.
        if protection_end <= as_of {
            return Err(Error::Validation(format!(
                "CDS '{}' is expired: protection end {} is on or before valuation date {}",
                cds.id, protection_end, as_of
            )));
        }

        // Step-in at/after the protection end (e.g. valuing a 1-day CDS where
        // T+1 step-in lands on maturity): empty protection interval, zero
        // protection value.
        if protection_start >= protection_end {
            return Ok(0.0);
        }

        // Use hazard curve's day-count for time axis (survival is the dominant factor)
        let t_asof = haz_t(surv, as_of)?;
        let t_start = haz_t(surv, protection_start)?;
        let t_end = haz_t(surv, protection_end)?;

        let recovery = cds.protection.recovery_rate;
        let calendar = cds
            .premium
            .calendar_id
            .as_deref()
            .and_then(finstack_quant_core::dates::calendar::calendar_by_id);

        // Compute survival at as_of for conditioning
        let sp_asof = surv.sp(t_asof);

        let inputs = super::integration::ProtectionLegInputs {
            t_start,
            t_end,
            recovery,
            settlement_delay: cds.protection.settlement_delay,
            calendar,
            sp_asof,
            as_of,
            disc,
            surv,
        };
        let protection_pv = self.protection_leg_isda_standard_model_cond(&inputs)?;

        Ok(protection_pv * cds.notional.amount())
    }

    /// Calculate PV of premium leg with optional accrual-on-default
    /// Calculate PV of premium leg (Money)
    pub(crate) fn pv_premium_leg(
        &self,
        cds: &CreditDefaultSwap,
        disc: &DiscountCurve,
        surv: &HazardCurve,
        as_of: Date,
    ) -> Result<Money> {
        let pv = self.pv_premium_leg_raw(cds, disc, surv, as_of)?;
        Ok(Money::new(pv, cds.notional.currency()))
    }

    /// Calculate PV of premium leg (raw f64)
    ///
    /// Uses proper time-axis conventions:
    /// - Discounting: relative DF from `as_of` using discount curve's day-count
    /// - Survival: conditional survival given no default before `as_of` using hazard curve's day-count
    /// - Accrual: instrument's premium leg day-count convention (Act/360 for NA, etc.)
    /// - Accrual-on-default: analytical piecewise-constant integration over hazard/disc knots
    pub(crate) fn pv_premium_leg_raw(
        &self,
        cds: &CreditDefaultSwap,
        disc: &DiscountCurve,
        surv: &HazardCurve,
        as_of: Date,
    ) -> Result<f64> {
        let calendar = cds
            .premium
            .calendar_id
            .as_deref()
            .and_then(finstack_quant_core::dates::calendar::calendar_by_id);
        let periods = self.coupon_periods(cds, as_of)?;
        let spread = cds.premium.spread_bp.to_f64().ok_or_else(|| {
            Error::Validation("premium spread_bp cannot be represented as f64".into())
        })? / BASIS_POINTS_PER_UNIT;

        let mut premium_pv = 0.0;
        for period in periods {
            let start_date = period.accrual_start;
            let end_date = period.accrual_end;
            let payment_date = period.payment_date;

            // Skip periods that have already ended before as_of
            if end_date <= as_of {
                continue;
            }

            // Discounting uses discount curve's day-count and relative DF from as_of
            let df = df_asof_to(disc, as_of, payment_date)?;

            // Survival uses hazard curve's day-count and conditional probability
            let sp = sp_cond_to(surv, as_of, end_date)?;

            let accrual = self.coupon_accrual(cds, &period)?;
            let scheduled_coupon = cds.notional.amount() * spread * accrual;
            premium_pv += scheduled_coupon * sp * df;

            if self.config.include_accrual {
                let spread_sign = spread.signum();
                // Keep AoD on the same dollar basis as the scheduled coupon leg.
                premium_pv += spread_sign
                    * cds.notional.amount()
                    * self.accrual_on_default_isda_standard_model_cond(AodInputs {
                        cds,
                        spread: spread.abs(),
                        accrual_start_date: if matches!(
                            cds.valuation_convention,
                            CdsValuationConvention::BloombergCdswClean
                        ) {
                            start_date.max(as_of)
                        } else {
                            start_date
                        },
                        start_date: start_date.max(as_of),
                        end_date,
                        settlement_delay: cds.protection.settlement_delay,
                        calendar,
                        as_of,
                        disc,
                        surv,
                    })?;
            }
        }

        Ok(premium_pv)
    }

    // ─── Accrual-on-default integration ───────────────────────────────────

    /// ISDA Standard Model AoD: analytical integration over piecewise-constant
    /// hazard and interest rate intervals (knot-aligned), using conditional
    /// survival and relative discount factors from `as_of`.
    pub(super) fn accrual_on_default_isda_standard_model_cond(
        &self,
        inp: AodInputs<'_>,
    ) -> Result<f64> {
        if inp.end_date <= inp.start_date {
            return Ok(0.0);
        }
        // `accrual_start_date` may pre-date the hazard curve base for the
        // in-progress coupon period of an ISDA-dirty CDS valued mid-period.
        // Compute a signed year fraction so the accrual origin extends back
        // across the base date instead of clamping to zero.
        let t_accrual_start = if inp.accrual_start_date < inp.surv.base_date() {
            -inp.surv.day_count().year_fraction(
                inp.accrual_start_date,
                inp.surv.base_date(),
                finstack_quant_core::dates::DayCountContext::default(),
            )?
        } else {
            haz_t(inp.surv, inp.accrual_start_date)?
        };
        let t_start = haz_t(inp.surv, inp.start_date)?;
        let t_end = haz_t(inp.surv, inp.end_date)?;
        let t_asof = haz_t(inp.surv, inp.as_of)?;
        let sp_asof = inp.surv.sp(t_asof);
        if sp_asof <= credit::SURVIVAL_PROBABILITY_FLOOR {
            return Ok(0.0);
        }

        // QuantLib parity: with `Actual360(true)` the within-period
        // accrual fraction is inclusive of the upper boundary. Mirror that
        // behaviour for the AoD integral when the override is requested
        // so the linear `tau` interpolation matches QuantLib's
        // `IsdaCdsEngine`.
        let tau_remaining = if inp
            .cds
            .pricing_overrides
            .model_config
            .cds_act360_include_last_day
            && inp.cds.premium.day_count == finstack_quant_core::dates::DayCount::Act360
            && inp.end_date > inp.accrual_start_date
        {
            let days = finstack_quant_core::dates::DayCount::calendar_days(
                inp.accrual_start_date,
                inp.end_date,
            ) + 1;
            (days.max(0) as f64) / 360.0
        } else {
            year_fraction(
                inp.cds.premium.day_count,
                inp.accrual_start_date,
                inp.end_date,
            )?
        };
        let accrual_period_length_haz = t_end - t_accrual_start;
        if accrual_period_length_haz <= 0.0 || tau_remaining <= 0.0 {
            return Ok(0.0);
        }
        // Linear scale from hazard-time position to instrument-day-count accrual.
        let tau_per_haz = tau_remaining / accrual_period_length_haz;

        let boundaries = isda_standard_model_boundaries(
            t_start,
            t_end,
            inp.surv,
            inp.disc,
            self.config.protection_leg_substeps_per_year,
        );
        let mut accrual_pv = 0.0;

        for window in boundaries.windows(2) {
            let t1 = window[0];
            let t2 = window[1];
            let dt = t2 - t1;
            if dt <= numerical::ZERO_TOLERANCE {
                continue;
            }

            let sp1 = inp.surv.sp(t1) / sp_asof;
            let sp2 = inp.surv.sp(t2) / sp_asof;
            if !(sp1 > sp2 && sp1 > 0.0) {
                continue;
            }

            // Piecewise-constant hazard rate over [t1, t2].
            let hazard_rate = -(sp2 / sp1).ln() / dt;

            // Relative DF anchored at as_of, via settled default date per knot.
            let settle1 = settlement_date(
                date_from_hazard_time(inp.surv, t1),
                inp.settlement_delay,
                inp.calendar,
                self.config.business_days_per_year,
            )?;
            let settle2 = settlement_date(
                date_from_hazard_time(inp.surv, t2),
                inp.settlement_delay,
                inp.calendar,
                self.config.business_days_per_year,
            )?;
            let df1 = df_asof_to(inp.disc, inp.as_of, settle1)?;
            let df2 = df_asof_to(inp.disc, inp.as_of, settle2)?;

            // Piecewise-constant interest rate (may be negative if df2 > df1).
            let interest_rate = if df1 > 0.0 && df2 > 0.0 {
                -(df2 / df1).ln() / dt
            } else {
                0.0
            };

            // Accrued fraction at interval start, expressed in instrument-DC units.
            //
            // QuantLib's `Actual360(true)` shifts discrete coupon accrual by
            // one inclusive day. In the continuous default-accrual integral,
            // that convention contributes half a day at the interval boundary.
            // `IsdaCdsEngine` can also add its explicit `HalfDayBias`; when
            // both QuantLib knobs are enabled the starting accrual is shifted
            // by one full day.
            let mut bias_days = 0.0;
            if inp.cds.premium.day_count == finstack_quant_core::dates::DayCount::Act360 {
                if inp
                    .cds
                    .pricing_overrides
                    .model_config
                    .cds_act360_include_last_day
                {
                    bias_days += 0.5;
                }
                if inp.cds.pricing_overrides.model_config.cds_aod_half_day_bias {
                    bias_days += 0.5;
                }
            }
            let accrual_bias = bias_days / 360.0;
            let tau_at_t1 = (t1 - t_accrual_start) * tau_per_haz + accrual_bias;

            // Analytical integration for
            //   ∫ spread * (τ_at_t1 + (t - t1) * tau_per_haz) * λ * S(t1) * D(t1)
            //     * exp(-(λ + r)(t - t1)) dt
            // Let k = λ + r. Then
            //   I0 = (1 - e^{-kΔ})/k
            //   I1 = (1 - e^{-kΔ}(1 + kΔ))/k²
            let k = hazard_rate + interest_rate;
            let contribution = if k.abs() > numerical::ZERO_TOLERANCE {
                let exp_term = (-k * dt).exp();
                let i0 = (1.0 - exp_term) / k;
                let i1 = (1.0 - exp_term * (1.0 + k * dt)) / (k * k);
                inp.spread * df1 * sp1 * hazard_rate * (tau_at_t1 * i0 + tau_per_haz * i1)
            } else {
                // Small-k fallback: midpoint approximation keeps AoD well-behaved
                // for near-zero hazard or near-zero (r+λ).
                let t_mid = (t1 + t2) * 0.5;
                let position =
                    ((t_mid - t_accrual_start) / accrual_period_length_haz).clamp(0.0, 1.0);
                let accrued_tau = tau_remaining * position + accrual_bias;
                inp.spread * accrued_tau * (sp1 - sp2) * df1
            };
            accrual_pv += contribution;
        }
        Ok(accrual_pv)
    }
}
