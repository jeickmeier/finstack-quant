//! `CdsPricer`: premium and protection legs, protection-leg integration over the
//! ISDA Standard Model knot grid, NPV and the prepared CDS workspace.

use super::grid_cache::IsdaWindowCache;
use super::helpers::{
    date_from_hazard_time, haz_t, isda_standard_model_boundaries, settlement_date, sp_cond_to,
    validate_recovery_consistency,
};
use crate::cds::CreditDefaultSwap;
use finstack_quant_core::dates::{Date, DayCountContext, HolidayCalendar};
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::math::ZERO_TOLERANCE;
use finstack_quant_core::math::units::BASIS_POINTS_PER_UNIT;
use finstack_quant_core::money::Money;
use finstack_quant_core::{Error, Result};
use finstack_quant_valuations_core::constants::credit;
use rust_decimal::prelude::ToPrimitive;

/// CDS pricing engine.
///
/// Stateless: every valuation choice (par-spread denominator, step-in,
/// accrual day count) comes from the instrument's
/// [`CdsValuationConvention`](finstack_quant_valuations_core::market::conventions::CdsValuationConvention).
/// Accrual-on-default is always included in the premium leg.
#[derive(Debug, Default, Clone, Copy)]
#[doc(hidden)]
pub struct CdsPricer;

#[derive(Clone, Copy)]
pub(super) struct AodInputs<'a> {
    pub(super) cds: &'a CreditDefaultSwap,
    pub(super) spread: f64,
    /// Date from which premium accrual is measured for the AoD integral.
    /// Premium accrues from the contractual coupon start, including when
    /// protection begins later. Protection effective dates do not alter the
    /// premium obligation represented by `premium_leg`. Clean presentation
    /// subtracts accrued separately; it never resets this cashflow origin.
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
    pub(super) grid_cache: Option<&'a IsdaWindowCache>,
}

#[derive(Clone, Copy)]
pub(crate) struct CouponPeriod {
    pub(super) accrual_start: Date,
    pub(super) accrual_end: Date,
    pub(super) payment_date: Date,
    pub(super) is_final: bool,
}

impl CdsPricer {
    fn add_premium_period(
        &self,
        total: &mut f64,
        inputs: AodInputs<'_>,
        accrual: f64,
        df: f64,
    ) -> Result<()> {
        let sp = if inputs.end_date <= inputs.as_of {
            1.0
        } else {
            sp_cond_to(inputs.surv, inputs.as_of, inputs.end_date)?
        };
        let scheduled_coupon = inputs.cds.notional.amount() * inputs.spread * accrual;
        *total += scheduled_coupon * sp * df;
        *total += inputs.spread.signum()
            * inputs.cds.notional.amount()
            * self.accrual_on_default_isda_standard_model_cond(AodInputs {
                spread: inputs.spread.abs(),
                ..inputs
            })?;
        Ok(())
    }

    /// Create the (stateless) CDS pricer.
    #[must_use]
    #[doc(hidden)]
    pub fn new() -> Self {
        Self
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
        Money::from_model_value(pv, cds.notional.currency())
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
        self.pv_protection_leg_prepared(cds, disc, surv, as_of, None)
    }

    fn pv_protection_leg_prepared(
        &self,
        cds: &CreditDefaultSwap,
        disc: &DiscountCurve,
        surv: &HazardCurve,
        as_of: Date,
        grid_cache: Option<&IsdaWindowCache>,
    ) -> Result<f64> {
        // Note: Recovery rate validation is performed at CDS construction time.
        // All public constructors (builder, new_isda) call validate().
        //
        // Additionally enforce that the trade-spec recovery agrees with the
        // recovery used to bootstrap the hazard curve. The ISDA Standard Model
        // requires the same R in both legs; mismatched recoveries silently
        // mis-scale the protection leg (1 − R) factor.
        validate_recovery_consistency(cds.protection_leg.recovery_rate, surv)?;

        // Protection leg covers the period from protection start to premium end.
        // For forward-starting CDS, protection begins at protection_effective_date
        // (which may be later than the premium leg start).
        // We only value protection from as_of onwards (can't protect against past defaults).
        //
        // **Audit P3c**: standard CDS (single-name, CDX, iTraxx) terminate
        // protection and premium on the same date, which is why we read
        // `cds.premium_leg.end` here. `ProtectionLegSpec`
        // (see `instruments::ProtectionLegSpec`) carries no
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
        let step_in =
            as_of + time::Duration::days(cds.valuation_convention.protection_step_in_days());
        let protection_start = step_in.max(cds.protection_start());
        let protection_end = cds.premium_leg.end;

        // Protection can expire before the last rolled premium payment.
        // An exhausted protection interval contributes zero, leaving the
        // pending premium receivable available to the rest of the pricer.
        if protection_end <= as_of {
            return Ok(0.0);
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

        let recovery = cds.protection_leg.recovery_rate;
        let calendar = cds
            .premium_leg
            .calendar_id
            .as_deref()
            .map(finstack_quant_core::dates::calendar_by_id_strict)
            .transpose()?;

        // Compute survival at as_of for conditioning
        let sp_asof = surv.sp(t_asof);

        let grid = IsdaGrid {
            t_start,
            t_end,
            settlement_delay: cds.protection_leg.settlement_delay,
            calendar,
            sp_asof,
            as_of,
            disc,
            surv,
            grid_cache,
        };
        let protection_pv = protection_leg_isda_standard_model_cond(&grid, recovery)?;

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
        Money::from_model_value(pv, cds.notional.currency())
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
            .premium_leg
            .calendar_id
            .as_deref()
            .map(finstack_quant_core::dates::calendar_by_id_strict)
            .transpose()?;
        let periods = self.coupon_periods(cds, as_of)?;
        let spread = cds.premium_leg.coupon_bp.to_f64().ok_or_else(|| {
            Error::Validation("premium.coupon_bp cannot be represented as f64".into())
        })? / BASIS_POINTS_PER_UNIT;

        let mut premium_pv = 0.0;
        for period in periods {
            let start_date = period.accrual_start;
            let end_date = period.accrual_end;
            let payment_date = period.payment_date;

            // Payments on as_of are treated as settled throughout CDS pricing.
            if payment_date <= as_of {
                continue;
            }

            // Discounting uses discount curve's day-count and relative DF from as_of
            let df = disc.df_between_dates(as_of, payment_date)?;

            let accrual = self.coupon_accrual(cds, &period)?;
            self.add_premium_period(
                &mut premium_pv,
                AodInputs {
                    cds,
                    spread,
                    accrual_start_date: start_date,
                    start_date: start_date.max(as_of),
                    end_date,
                    settlement_delay: cds.protection_leg.settlement_delay,
                    calendar,
                    as_of,
                    disc,
                    surv,
                    grid_cache: None,
                },
                accrual,
                df,
            )?;
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
        // behaviour for the AoD integral under the QuantLib parity convention
        // so the linear `tau` interpolation matches QuantLib's
        // `IsdaCdsEngine`.
        let tau_remaining = if inp.cds.valuation_convention.act360_includes_last_day()
            && inp.cds.premium_leg.day_count == finstack_quant_core::dates::DayCount::Act360
            && inp.end_date > inp.accrual_start_date
        {
            let days = (inp.end_date - inp.accrual_start_date).whole_days() + 1;
            (days.max(0) as f64) / 360.0
        } else {
            inp.cds.premium_leg.day_count.year_fraction(
                inp.accrual_start_date,
                inp.end_date,
                DayCountContext::default(),
            )?
        };
        let accrual_period_length_haz = t_end - t_accrual_start;
        if accrual_period_length_haz <= 0.0 || tau_remaining <= 0.0 {
            return Ok(0.0);
        }
        // Linear scale from hazard-time position to instrument-day-count accrual.
        let tau_per_haz = tau_remaining / accrual_period_length_haz;

        // Accrued fraction at interval start, expressed in instrument-DC units.
        //
        // QuantLib's `Actual360(true)` shifts discrete coupon accrual by
        // one inclusive day. In the continuous default-accrual integral,
        // that convention contributes half a day at the interval boundary.
        // `IsdaCdsEngine` can also add its explicit `HalfDayBias`; when
        // both QuantLib knobs are enabled the starting accrual is shifted
        // by one full day.
        let mut bias_days = 0.0;
        if inp.cds.premium_leg.day_count == finstack_quant_core::dates::DayCount::Act360 {
            if inp.cds.valuation_convention.act360_includes_last_day() {
                bias_days += 0.5;
            }
            if inp.cds.valuation_convention.aod_half_day_bias() {
                bias_days += 0.5;
            }
        }
        let accrual_bias = bias_days / 360.0;

        let grid = IsdaGrid {
            t_start,
            t_end,
            settlement_delay: inp.settlement_delay,
            calendar: inp.calendar,
            sp_asof,
            as_of: inp.as_of,
            disc: inp.disc,
            surv: inp.surv,
            grid_cache: inp.grid_cache,
        };
        let mut accrual_pv = 0.0;
        grid.for_each_window(|w| {
            let tau_at_t1 = (w.t1 - t_accrual_start) * tau_per_haz + accrual_bias;

            // Analytical integration for
            //   ∫ spread * (τ_at_t1 + (t - t1) * tau_per_haz) * λ * S(t1) * D(t1)
            //     * exp(-(λ + r)(t - t1)) dt
            // Let k = λ + r. Then
            //   I0 = (1 - e^{-kΔ})/k
            //   I1 = (1 - e^{-kΔ}(1 + kΔ))/k²
            let k = w.hazard_rate + w.interest_rate;
            let contribution = if k.abs() > ZERO_TOLERANCE {
                let exp_term = (-k * w.dt).exp();
                let i0 = (1.0 - exp_term) / k;
                let i1 = (1.0 - exp_term * (1.0 + k * w.dt)) / (k * k);
                inp.spread * w.df1 * w.sp1 * w.hazard_rate * (tau_at_t1 * i0 + tau_per_haz * i1)
            } else {
                // Small-k fallback: midpoint approximation keeps AoD well-behaved
                // for near-zero hazard or near-zero (r+λ).
                let t_mid = (w.t1 + w.t2) * 0.5;
                let position =
                    ((t_mid - t_accrual_start) / accrual_period_length_haz).clamp(0.0, 1.0);
                let accrued_tau = tau_remaining * position + accrual_bias;
                inp.spread * accrued_tau * (w.sp1 - w.sp2) * w.df1
            };
            accrual_pv += contribution;
        })?;
        Ok(accrual_pv)
    }
}

/// Immutable CDS contract and discount snapshot for repeated hazard-curve pricing.
///
/// Coupon dates, accruals, and payment discount factors do not change when
/// only the hazard curve is shocked. Protection and accrual-on-default still
/// re-integrate against the bumped survival curve. Discount windows are cached
/// for the last hazard knot topology; changing bases, day count or knot times
/// replaces that cache. Storage is bounded by this contract’s integration windows.
pub struct PreparedCds {
    pricer: CdsPricer,
    cds: CreditDefaultSwap,
    disc: std::sync::Arc<DiscountCurve>,
    as_of: Date,
    periods: Vec<(CouponPeriod, f64, f64)>,
    spread: f64,
    upfront_pv: f64,
    clean_accrued: f64,
    grids: IsdaWindowCache,
}

impl PreparedCds {
    /// Build the cache from the live discount curve and CDS schedule.
    ///
    /// # Arguments
    ///
    /// * `cds` - Valid CDS contract cloned into the workspace, including notional
    ///   currency, premium coupon in basis points, upfront cash, recovery,
    ///   settlement/calendar rules and clean/dirty valuation convention.
    /// * `market` - Supplies the discount curve named by the premium leg. Its
    ///   immutable Arc snapshot is retained; subsequent market changes do not
    ///   affect this workspace. No hazard curve is required at preparation.
    /// * `as_of` - Valuation date used to drop expired coupons and upfronts,
    ///   condition survival, calculate accrued premium and rebase discounting.
    ///
    /// # Errors
    /// Returns validation, missing-market-data or date/schedule errors for an
    /// invalid contract, unavailable discount curve or invalid discount/accrual
    /// inputs. Hazard/recovery compatibility is checked by [`Self::price`].
    pub fn new(
        cds: &CreditDefaultSwap,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> Result<Self> {
        use rust_decimal::prelude::ToPrimitive;

        cds.validate()?;
        let pricer = CdsPricer::new();
        let disc = market.get_discount(&cds.premium_leg.discount_curve_id)?;
        let periods_raw = pricer.coupon_periods(cds, as_of)?;
        let mut periods = Vec::with_capacity(periods_raw.len());
        for period in periods_raw {
            if period.payment_date <= as_of {
                continue;
            }
            let accrual = pricer.coupon_accrual(cds, &period)?;
            let df = disc.as_ref().df_between_dates(as_of, period.payment_date)?;
            periods.push((period, accrual, df));
        }
        let spread = cds.premium_leg.coupon_bp.to_f64().ok_or_else(|| {
            Error::Validation("premium.coupon_bp cannot be represented as f64".into())
        })? / BASIS_POINTS_PER_UNIT;
        let upfront_pv = match cds.upfront {
            Some((dt, amount)) if dt >= as_of => {
                amount.amount() * disc.as_ref().df_between_dates(as_of, dt)?
            }
            _ => 0.0,
        };
        let clean_accrued = if cds.uses_clean_price() {
            let accrual_fraction = pricer.coupon_accrued_fraction(
                cds,
                as_of,
                super::metrics::AccrualDayCountPolicy::CdswInclusive,
            )?;
            cds.notional.amount() * spread * accrual_fraction
        } else {
            0.0
        };
        Ok(Self {
            pricer,
            cds: cds.clone(),
            disc,
            as_of,
            periods,
            spread,
            upfront_pv,
            clean_accrued,
            grids: IsdaWindowCache::default(),
        })
    }

    /// Reprice after a hazard bump, reusing cached discount/premium structure.
    ///
    /// # Arguments
    ///
    /// * `surv` - Trial hazard curve, with decimal annual hazard rates and its
    ///   own base date/day-count clock. Its recovery must match the CDS; its ID
    ///   need not match the contract's original credit curve ID. Protection,
    ///   premium survival and accrual-on-default use this curve. Discounting
    ///   remains fixed at the snapshot captured by [`Self::new`].
    ///
    /// # Returns
    /// Unrounded signed present value in the CDS notional currency, including
    /// dated upfront and the contract's clean/dirty accrued treatment.
    ///
    /// # Errors
    /// Returns recovery-validation, survival, discounting or integration/date
    /// errors from the shared CDS pricing kernel.
    pub fn price(&self, surv: &HazardCurve) -> Result<f64> {
        let protection_pv = self.pricer.pv_protection_leg_prepared(
            &self.cds,
            self.disc.as_ref(),
            surv,
            self.as_of,
            Some(&self.grids),
        )?;
        let calendar = self
            .cds
            .premium_leg
            .calendar_id
            .as_deref()
            .map(finstack_quant_core::dates::calendar_by_id_strict)
            .transpose()?;
        let mut premium_pv = 0.0;
        for &(period, accrual, df) in &self.periods {
            self.pricer.add_premium_period(
                &mut premium_pv,
                AodInputs {
                    cds: &self.cds,
                    spread: self.spread,
                    accrual_start_date: period.accrual_start,
                    start_date: period.accrual_start.max(self.as_of),
                    end_date: period.accrual_end,
                    settlement_delay: self.cds.protection_leg.settlement_delay,
                    calendar,
                    as_of: self.as_of,
                    disc: self.disc.as_ref(),
                    surv,
                    grid_cache: Some(&self.grids),
                },
                accrual,
                df,
            )?;
        }

        Ok(signed_npv(
            &self.cds,
            protection_pv,
            premium_pv,
            self.upfront_pv,
            self.clean_accrued,
        ))
    }
}

pub(super) fn signed_npv(
    cds: &CreditDefaultSwap,
    protection_pv: f64,
    premium_pv: f64,
    upfront_pv: f64,
    clean_accrued: f64,
) -> f64 {
    use finstack_quant_valuations_core::instruments::PayReceive;
    let mut value = match cds.side {
        PayReceive::Pay => protection_pv - premium_pv - upfront_pv,
        PayReceive::Receive => premium_pv - protection_pv + upfront_pv,
    };
    if cds.uses_clean_price() {
        value = match cds.side {
            PayReceive::Pay => value + clean_accrued,
            PayReceive::Receive => value - clean_accrued,
        };
    }
    value
}

// Protection-leg and accrual-on-default integration over the ISDA Standard
// Model knot grid.

/// Per-integral constants of the ISDA Standard Model knot grid shared by the
/// protection-leg and accrual-on-default integrators.
#[derive(Clone, Copy)]
pub(super) struct IsdaGrid<'a> {
    /// Hazard-curve time (years) of the integration window start.
    pub(super) t_start: f64,
    /// Hazard-curve time (years) of the integration window end.
    pub(super) t_end: f64,
    /// Settlement delay in business days applied to the default date.
    pub(super) settlement_delay: u16,
    /// Optional business-day calendar for the settlement-date shift.
    pub(super) calendar: Option<&'a dyn HolidayCalendar>,
    /// Survival probability at `as_of` used to condition later hazard times.
    pub(super) sp_asof: f64,
    /// Pricing valuation date (discount factors are relative to it).
    pub(super) as_of: Date,
    /// Discount curve valuing the default-settlement cashflows.
    pub(super) disc: &'a DiscountCurve,
    /// Hazard / survival curve producing default densities.
    pub(super) surv: &'a HazardCurve,
    pub(super) grid_cache: Option<&'a IsdaWindowCache>,
}

/// One grid window with piecewise-constant hazard and interest rates.
#[derive(Clone, Copy)]
pub(super) struct IsdaWindow {
    /// Window start (hazard-curve years).
    pub(super) t1: f64,
    /// Window end (hazard-curve years).
    pub(super) t2: f64,
    /// Window length `t2 - t1`.
    pub(super) dt: f64,
    /// Conditional survival at `t1`.
    pub(super) sp1: f64,
    /// Conditional survival at `t2`.
    pub(super) sp2: f64,
    /// Piecewise-constant hazard rate `λ` over the window.
    pub(super) hazard_rate: f64,
    /// Relative discount factor from `as_of` to the settlement of a default at `t1`.
    pub(super) df1: f64,
    /// Piecewise-constant interest rate `r` over the window (may be negative).
    pub(super) interest_rate: f64,
}

#[derive(Clone, Copy)]
pub(super) struct DiscountWindow {
    pub(super) df1: f64,
    pub(super) interest_rate: f64,
}

impl IsdaGrid<'_> {
    pub(super) fn discount_window(&self, t1: f64, t2: f64, dt: f64) -> Result<DiscountWindow> {
        // Relative DF anchored at as_of, via settled default date per knot.
        let settle1 = settlement_date(
            date_from_hazard_time(self.surv, t1),
            self.settlement_delay,
            self.calendar,
        )?;
        let settle2 = settlement_date(
            date_from_hazard_time(self.surv, t2),
            self.settlement_delay,
            self.calendar,
        )?;
        let df1 = self.disc.df_between_dates(self.as_of, settle1)?;
        let df2 = self.disc.df_between_dates(self.as_of, settle2)?;

        // Piecewise-constant interest rate (may be negative if df2 > df1).
        let interest_rate = if df1 > 0.0 && df2 > 0.0 {
            -(df2 / df1).ln() / dt
        } else {
            0.0
        };

        Ok(DiscountWindow { df1, interest_rate })
    }

    /// Visit every window of the knot grid that carries default probability
    /// (`sp1 > sp2` and `sp1 > 0`).
    pub(super) fn for_each_window(&self, mut visit: impl FnMut(IsdaWindow)) -> Result<()> {
        let prepared = self.grid_cache.map(|cache| cache.get(self)).transpose()?;
        let owned;
        let boundaries = if let Some(grid) = &prepared {
            grid.boundaries.as_slice()
        } else {
            owned = isda_standard_model_boundaries(self.t_start, self.t_end, self.surv, self.disc)?;
            owned.as_slice()
        };
        for (index, window) in boundaries.windows(2).enumerate() {
            let t1 = window[0];
            let t2 = window[1];
            let dt = t2 - t1;
            if dt <= ZERO_TOLERANCE {
                continue;
            }

            let sp1 = self.surv.sp(t1) / self.sp_asof;
            let sp2 = self.surv.sp(t2) / self.sp_asof;
            if !(sp1 > sp2 && sp1 > 0.0) {
                continue;
            }

            // Piecewise-constant hazard rate over [t1, t2].
            let hazard_rate = -(sp2 / sp1).ln() / dt;

            let DiscountWindow { df1, interest_rate } = match prepared.as_ref() {
                Some(grid) => grid.discounts[index].clone()?,
                None => self.discount_window(t1, t2, dt)?,
            };

            visit(IsdaWindow {
                t1,
                t2,
                dt,
                sp1,
                sp2,
                hazard_rate,
                df1,
                interest_rate,
            });
        }
        Ok(())
    }
}

/// ISDA Standard Model protection leg per unit notional, with conditional
/// survival and relative discounting; `recovery` is the decimal recovery rate.
fn protection_leg_isda_standard_model_cond(grid: &IsdaGrid<'_>, recovery: f64) -> Result<f64> {
    if grid.t_start >= grid.t_end {
        return Err(Error::Validation(format!(
            "Protection leg start time ({}) must be before end time ({})",
            grid.t_start, grid.t_end
        )));
    }
    if grid.sp_asof <= credit::SURVIVAL_PROBABILITY_FLOOR {
        return Ok(0.0);
    }

    let lgd = 1.0 - recovery;
    let mut integral = 0.0;
    grid.for_each_window(|w| {
        // ISDA Standard Model analytical integration
        let lambda_plus_r = w.hazard_rate + w.interest_rate;
        if lambda_plus_r.abs() > ZERO_TOLERANCE {
            let exp_term = (-lambda_plus_r * w.dt).exp();
            integral += w.df1 * w.sp1 * (w.hazard_rate / lambda_plus_r) * (1.0 - exp_term);
        } else {
            integral += w.df1 * w.sp1 * w.hazard_rate * w.dt;
        }
    })?;

    Ok(integral * lgd)
}

#[cfg(test)]
mod prepared_cds_tests {
    use super::{CdsPricer, PreparedCds};
    use crate::cds::CreditDefaultSwap;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use finstack_quant_valuations_core::instruments::Instrument;
    use finstack_quant_valuations_core::instruments::PayReceive;
    use finstack_quant_valuations_core::market::conventions::{
        CdsConvention, CdsValuationConvention,
    };
    use rust_decimal::Decimal;
    use time::macros::date;

    fn create_test_cds(
        valuation_convention: CdsValuationConvention,
    ) -> finstack_quant_core::Result<CreditDefaultSwap> {
        let mut cds = CreditDefaultSwap::new_isda(
            CdsConvention::IsdaNa,
            Decimal::new(10_000, 2),
            crate::cds::CdsTradeParams {
                id: InstrumentId::new("CACHE-TEST-CDS"),
                notional: Money::from((10_000_000_i64, Currency::USD)),
                side: PayReceive::Pay,
                start: date!(2024 - 12 - 20),
                end: date!(2030 - 03 - 20),
                recovery_rate: 0.40,
                discount_curve_id: CurveId::new("USD-OIS"),
                credit_curve_id: CurveId::new("TEST-CREDIT"),
            },
        )?;
        cds.valuation_convention = valuation_convention;
        Ok(cds)
    }

    fn create_test_market() -> finstack_quant_core::Result<(MarketContext, HazardCurve)> {
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(date!(2025 - 01 - 01))
            .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.80), (10.0, 0.65)])
            .build()?;
        let hazard = HazardCurve::builder("TEST-CREDIT")
            .base_date(date!(2025 - 01 - 01))
            .recovery_rate(0.40)
            .knots([(1.0, 0.02), (3.0, 0.03), (5.0, 0.04), (10.0, 0.05)])
            .par_spreads([(1.0, 100.0), (3.0, 150.0), (5.0, 200.0), (10.0, 250.0)])
            .build()?;
        let market = MarketContext::new().insert(discount).insert(hazard.clone());
        Ok((market, hazard))
    }

    fn assert_npv_matches(cached: f64, full: f64, case: &str) {
        let tolerance = 1.0e-8_f64.max(full.abs() * 1.0e-12);
        assert!(
            (cached - full).abs() <= tolerance,
            "{case}: cached NPV {cached} differs from full value {full} by more than {tolerance}"
        );
    }

    fn assert_base_and_bumped_values_match(
        cds: &CreditDefaultSwap,
        market: &MarketContext,
        hazard: &HazardCurve,
        as_of: Date,
    ) -> finstack_quant_core::Result<PreparedCds> {
        let cache = PreparedCds::new(cds, market, as_of)?;

        let base_cached = cache.price(hazard)?;
        let base_full = cds.value(market, as_of)?.amount();
        assert_npv_matches(base_cached, base_full, "base hazard");

        let bumped_hazard = hazard.with_parallel_hazard_rate_bump_bp(1.0)?;
        let mut bumped_market = market.clone();
        bumped_market.insert_mut(bumped_hazard.clone());
        let bumped_cached = cache.price(&bumped_hazard)?;
        let bumped_full = cds.value(&bumped_market, as_of)?.amount();
        assert_npv_matches(bumped_cached, bumped_full, "bumped hazard");
        assert!(
            (bumped_full - base_full).abs() > 1.0e-8,
            "parallel hazard bump should change the full CDS value"
        );

        Ok(cache)
    }

    #[test]
    fn prepared_cds_matches_value_for_current_coupon_with_accrual_on_default()
    -> finstack_quant_core::Result<()> {
        let as_of = date!(2025 - 02 - 15);
        let cds = create_test_cds(CdsValuationConvention::IsdaDirty)?;
        let (market, hazard) = create_test_market()?;
        let cache = assert_base_and_bumped_values_match(&cds, &market, &hazard, as_of)?;

        assert!(
            cache.periods.iter().any(|(period, _, _)| {
                period.accrual_start < as_of && as_of < period.accrual_end
            }),
            "valuation date should fall inside a live coupon accrual period"
        );
        let discount = market.get_discount(&cds.premium_leg.discount_curve_id)?;
        let pricer = CdsPricer::new();
        let with_aod =
            pricer.premium_leg_annuity(&cds, discount.as_ref(), &hazard, as_of, 1.0, true)?;
        let without_aod =
            pricer.premium_leg_annuity(&cds, discount.as_ref(), &hazard, as_of, 1.0, false)?;
        assert!(
            (with_aod - without_aod).abs() > 1.0e-8,
            "fixture should produce a non-zero accrual-on-default contribution"
        );
        Ok(())
    }

    #[test]
    fn prepared_cds_matches_value_for_bloomberg_clean_accrued() -> finstack_quant_core::Result<()> {
        let as_of = date!(2025 - 02 - 15);
        let cds = create_test_cds(CdsValuationConvention::BloombergCdswClean)?;
        let (market, hazard) = create_test_market()?;
        let cache = assert_base_and_bumped_values_match(&cds, &market, &hazard, as_of)?;

        assert!(
            cache.clean_accrued.abs() > 1.0e-8,
            "current-coupon Bloomberg clean fixture should have non-zero accrued premium"
        );
        Ok(())
    }
    #[test]
    fn prepared_cds_matches_raw_pricing_bitwise_and_snapshots_discount()
    -> finstack_quant_core::Result<()> {
        let (market, hazard) = create_test_market()?;
        let discount = market.get_discount("USD-OIS")?;
        let pricer = CdsPricer::new();
        for convention in [
            CdsValuationConvention::IsdaDirty,
            CdsValuationConvention::BloombergCdswClean,
            CdsValuationConvention::BloombergCdswCleanFullPremium,
            CdsValuationConvention::QuantLibIsdaParity,
        ] {
            for as_of in [
                date!(2025 - 01 - 01),
                date!(2025 - 02 - 15),
                date!(2025 - 03 - 20),
            ] {
                let mut cds = create_test_cds(convention)?;
                cds.upfront = Some((
                    as_of + time::Duration::days(3),
                    Money::new(12345.67, Currency::USD)?,
                ));
                let prepared = PreparedCds::new(&cds, &market, as_of)?;
                for curve in [&hazard, &hazard.with_parallel_hazard_rate_bump_bp(1.0)?] {
                    let expected = pricer.npv_full(&cds, discount.as_ref(), curve, as_of)?;
                    assert_eq!(
                        prepared.price(curve)?.to_bits(),
                        expected.to_bits(),
                        "{convention:?} {as_of}"
                    );
                }
                let original = prepared.price(&hazard)?;
                let replacement = DiscountCurve::builder("USD-OIS")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (10.0, 0.5)])
                    .build()?;
                let changed = market.clone().insert(replacement);
                assert_eq!(prepared.price(&hazard)?.to_bits(), original.to_bits());
                assert!(
                    (PreparedCds::new(&cds, &changed, as_of)?.price(&hazard)? - original).abs()
                        > 1.0
                );
            }
        }
        Ok(())
    }

    #[test]
    fn prepared_cds_requires_valid_contract_and_discount_but_no_hazard()
    -> finstack_quant_core::Result<()> {
        let (market, hazard) = create_test_market()?;
        let cds = create_test_cds(CdsValuationConvention::IsdaDirty)?;
        let discount = market.get_discount("USD-OIS")?;
        let discount_only = MarketContext::new().insert(discount.as_ref().clone());
        let prepared = PreparedCds::new(&cds, &discount_only, date!(2025 - 02 - 15))?;
        assert!(prepared.price(&hazard)?.is_finite());
        assert!(PreparedCds::new(&cds, &MarketContext::new(), date!(2025 - 02 - 15)).is_err());
        let mismatched = hazard
            .to_builder_with_id("other")
            .recovery_rate(0.2)
            .build()?;
        assert!(prepared.price(&mismatched).is_err());
        let mut invalid = cds;
        invalid.protection_leg.recovery_rate = f64::NAN;
        assert!(PreparedCds::new(&invalid, &discount_only, date!(2025 - 02 - 15)).is_err());
        Ok(())
    }
    #[test]
    fn prepared_cds_grids_reuse_rates_and_track_actual_topology() -> finstack_quant_core::Result<()>
    {
        use finstack_quant_core::dates::DayCount;
        let (market, hazard) = create_test_market()?;
        let cds = create_test_cds(CdsValuationConvention::IsdaDirty)?;
        let as_of = date!(2025 - 02 - 15);
        let discount = market.get_discount("USD-OIS")?;
        let prepared = PreparedCds::new(&cds, &market, as_of)?;
        let ordinary = CdsPricer::new();
        let verify = |curve: &HazardCurve| -> finstack_quant_core::Result<()> {
            assert_eq!(
                prepared.price(curve)?.to_bits(),
                ordinary.npv_full(&cds, &discount, curve, as_of)?.to_bits()
            );
            Ok(())
        };
        verify(&hazard)?;
        let initial = prepared.grids.build_count();
        assert!(initial > 1);
        verify(&hazard)?;
        verify(&hazard.with_parallel_hazard_rate_bump_bp(1.0)?)?;
        assert_eq!(
            prepared.grids.build_count(),
            initial,
            "rate changes must not rebuild grids"
        );
        let retained = prepared.grids.retained_windows();
        for (base, day_count, knots) in [
            (date!(2025 - 01 - 01), DayCount::Act365F, vec![(1.0, 0.02)]),
            (
                date!(2025 - 01 - 01),
                DayCount::Act365F,
                vec![(1.0, 0.02), (3.0, 0.03)],
            ),
            (
                date!(2024 - 12 - 20),
                DayCount::Act365F,
                vec![(1.0, 0.02), (3.0, 0.03)],
            ),
            (
                date!(2024 - 12 - 20),
                DayCount::Act360,
                vec![(1.0, 0.02), (3.0, 0.03)],
            ),
            (
                date!(2024 - 12 - 20),
                DayCount::Act360,
                vec![(1.0, 0.02), (3.000000001, 0.03)],
            ),
        ] {
            let curve = HazardCurve::builder("trial")
                .base_date(base)
                .day_count(day_count)
                .recovery_rate(0.4)
                .knots(knots)
                .build()?;
            let before = prepared.grids.build_count();
            verify(&curve)?;
            assert!(
                prepared.grids.build_count() > before,
                "topology must invalidate"
            );
            let warm = prepared.grids.build_count();
            verify(&curve)?;
            assert_eq!(prepared.grids.build_count(), warm);
        }
        verify(&hazard)?;
        assert_eq!(
            prepared.grids.retained_windows(),
            retained,
            "only the last topology remains"
        );
        eprintln!(
            "CDS grid builds for first trial: {initial}; warm rate-only trial: 0; retained windows: {retained}; bytes per window: {}",
            std::mem::size_of::<finstack_quant_core::Result<super::DiscountWindow>>()
                + std::mem::size_of::<f64>()
        );
        Ok(())
    }

    #[test]
    fn prepared_grid_defers_discount_errors_until_positive_default_mass()
    -> finstack_quant_core::Result<()> {
        use finstack_quant_core::math::interp::ExtrapolationPolicy;
        let base = date!(2025 - 01 - 01);
        let disc = DiscountCurve::builder("short")
            .base_date(base)
            .knots([(0.0, 1.0), (0.5, 0.98)])
            .extrapolation(ExtrapolationPolicy::None)
            .build()?;
        let zero = HazardCurve::builder("trial")
            .base_date(base)
            .recovery_rate(0.4)
            .knots([(1.0, 0.0), (3.0, 0.0)])
            .build()?;
        let positive = zero.with_parallel_hazard_rate_bump_bp(100.0)?;
        let cache = super::IsdaWindowCache::default();
        let grid = super::IsdaGrid {
            t_start: 1.0,
            t_end: 2.0,
            settlement_delay: 0,
            calendar: None,
            sp_asof: 1.0,
            as_of: base,
            disc: &disc,
            surv: &zero,
            grid_cache: Some(&cache),
        };
        grid.for_each_window(|_| panic!("zero hazard must skip all windows"))?;
        let builds = cache.build_count();
        let positive_grid = super::IsdaGrid {
            surv: &positive,
            ..grid
        };
        let cached_error = positive_grid
            .for_each_window(|_| {})
            .expect_err("discount range");
        let ordinary_error = super::IsdaGrid {
            grid_cache: None,
            ..positive_grid
        }
        .for_each_window(|_| {})
        .expect_err("same range");
        assert_eq!(cached_error.to_string(), ordinary_error.to_string());
        grid.for_each_window(|_| panic!("zero mass ignores retained errors"))?;
        assert_eq!(cache.build_count(), builds);
        let cold = super::IsdaWindowCache::default();
        let exhausted = super::IsdaGrid {
            sp_asof: 0.0,
            grid_cache: Some(&cold),
            ..positive_grid
        };
        assert_eq!(
            super::protection_leg_isda_standard_model_cond(&exhausted, 0.4)?.to_bits(),
            0.0_f64.to_bits()
        );
        assert_eq!(
            cold.build_count(),
            0,
            "survival floor precedes grid work/errors"
        );
        Ok(())
    }

    #[test]
    fn prepared_cds_grid_matches_negative_rates_and_settlement_clocks()
    -> finstack_quant_core::Result<()> {
        use finstack_quant_core::dates::DayCount;
        let (_, hazard) = create_test_market()?;
        for as_of in [date!(2025 - 01 - 01), date!(2025 - 02 - 15)] {
            let discount=DiscountCurve::builder("USD-OIS").base_date(date!(2024-12-20)).day_count(DayCount::Act360)
                .knots([(0.0,1.0),(0.7,1.02),(4.1,1.1),(10.0,1.2)])
                .validation(finstack_quant_core::market_data::term_structures::ValidationMode::NegativeRateFriendly {forward_floor:-0.1}).build()?;
            let market = MarketContext::new().insert(discount.clone());
            for delay in [0, 3, 7] {
                let mut cds = create_test_cds(CdsValuationConvention::BloombergCdswClean)?;
                cds.protection_leg.settlement_delay = delay;
                let prepared = PreparedCds::new(&cds, &market, as_of)?;
                for curve in [&hazard, &hazard.with_parallel_hazard_rate_bump_bp(1.0)?] {
                    let expected = CdsPricer::new().npv_full(&cds, &discount, curve, as_of)?;
                    assert_eq!(prepared.price(curve)?.to_bits(), expected.to_bits());
                    assert_eq!(prepared.price(curve)?.to_bits(), expected.to_bits());
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod cds_review_regression_tests {
    use super::{CdsPricer, PreparedCds};
    use crate::cds::CreditDefaultSwap;
    use crate::cds::pricing::AccrualDayCountPolicy;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use finstack_quant_valuations_core::instruments::PayReceive;
    use finstack_quant_valuations_core::market::conventions::{
        CdsConvention, CdsValuationConvention,
    };
    use rust_decimal::Decimal;
    use time::macros::date;

    fn contract(
        start: Date,
        end: Date,
        convention: CdsValuationConvention,
    ) -> finstack_quant_core::Result<CreditDefaultSwap> {
        let mut cds = CreditDefaultSwap::new_isda(
            CdsConvention::IsdaNa,
            Decimal::from(100),
            crate::cds::CdsTradeParams {
                id: InstrumentId::new("CDS-REVIEW"),
                notional: Money::from((10_000_000_i64, Currency::USD)),
                side: PayReceive::Pay,
                start,
                end,
                recovery_rate: 0.4,
                discount_curve_id: CurveId::new("USD-OIS"),
                credit_curve_id: CurveId::new("CREDIT"),
            },
        )?;
        cds.valuation_convention = convention;
        cds.protection_leg.settlement_delay = 0;
        Ok(cds)
    }

    fn flat_market(
        as_of: Date,
        hazard_rate: f64,
    ) -> finstack_quant_core::Result<(DiscountCurve, HazardCurve)> {
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (10.0, 1.0)])
            .build()?;
        let hazard = HazardCurve::builder("CREDIT")
            .base_date(as_of)
            .recovery_rate(0.4)
            .knots([(0.0, hazard_rate), (10.0, hazard_rate)])
            .build()?;
        Ok((discount, hazard))
    }

    fn close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-7,
            "actual {actual}, expected {expected}, difference {}",
            actual - expected
        );
    }

    #[test]
    fn cds_final_coupon_uses_unadjusted_inclusive_maturity() -> finstack_quant_core::Result<()> {
        // Weekday, Sunday, Saturday and a holiday: payment may roll by
        // different amounts but contractual accrual always adds exactly one day.
        for (start, end) in [
            (date!(2025 - 03 - 20), date!(2025 - 06 - 20)),
            (date!(2026 - 06 - 20), date!(2026 - 09 - 20)),
            (date!(2025 - 06 - 20), date!(2025 - 09 - 20)),
            (date!(2025 - 12 - 20), date!(2026 - 06 - 19)),
        ] {
            let cds = contract(start, end, CdsValuationConvention::BloombergCdswClean)?;
            let pricer = CdsPricer::new();
            let periods = pricer.coupon_periods(&cds, start)?;
            let final_period = periods.last().expect("coupon period");
            assert_eq!(final_period.accrual_end, end);
            assert!(final_period.payment_date >= end);
            let expected = ((end - final_period.accrual_start).whole_days() + 1) as f64 / 360.0;
            close(pricer.coupon_accrual(&cds, final_period)?, expected);
        }
        Ok(())
    }

    #[test]
    fn cds_unpaid_final_coupon_survives_protection_expiry_and_cache()
    -> finstack_quant_core::Result<()> {
        let start = date!(2025 - 06 - 20);
        let end = date!(2025 - 09 - 20); // Saturday; payment is Monday September 22.
        for convention in [
            CdsValuationConvention::IsdaDirty,
            CdsValuationConvention::BloombergCdswClean,
        ] {
            let cds = contract(start, end, convention)?;
            let pricer = CdsPricer::new();
            let expected_days =
                (end - start).whole_days() + i64::from(convention.uses_clean_price());
            let expected_coupon = 10_000_000.0 * 0.01 * expected_days as f64 / 360.0;
            for as_of in [
                end,
                date!(2025 - 09 - 21),
                date!(2025 - 09 - 22),
                date!(2025 - 09 - 23),
            ] {
                for hazard_rate in [0.0, 0.2] {
                    let (disc, surv) = flat_market(as_of, hazard_rate)?;
                    let expected = if as_of < date!(2025 - 09 - 22) {
                        expected_coupon
                    } else {
                        0.0
                    };
                    close(
                        pricer.pv_protection_leg_raw(&cds, &disc, &surv, as_of)?,
                        0.0,
                    );
                    close(
                        pricer.pv_premium_leg_raw(&cds, &disc, &surv, as_of)?,
                        expected,
                    );
                    close(
                        pricer.premium_leg_pv_per_bp(&cds, &disc, &surv, as_of)?
                            * 100.0
                            * 10_000_000.0,
                        expected,
                    );
                    close(
                        pricer.risky_annuity(&cds, &disc, &surv, as_of)? * 0.01 * 10_000_000.0,
                        expected,
                    );
                    let market = MarketContext::new()
                        .insert(disc.clone())
                        .insert(surv.clone());
                    let cached = PreparedCds::new(&cds, &market, as_of)?.price(&surv)?;
                    close(cached, -expected);
                    close(cached, pricer.npv_full(&cds, &disc, &surv, as_of)?);
                }
            }
        }
        Ok(())
    }

    #[test]
    fn cds_completed_coupon_is_certain_below_historical_survival_floor()
    -> finstack_quant_core::Result<()> {
        let start = date!(2025 - 06 - 20);
        let end = date!(2025 - 09 - 20);
        let hazard_base = date!(2020 - 09 - 20);
        let surv = HazardCurve::builder("CREDIT")
            .base_date(hazard_base)
            .recovery_rate(0.4)
            .knots([(0.0, 10.0), (10.0, 10.0)])
            .build()?;
        assert!(
            surv.sp(5.0)
                < finstack_quant_valuations_core::constants::credit::SURVIVAL_PROBABILITY_FLOOR
        );
        for convention in [
            CdsValuationConvention::IsdaDirty,
            CdsValuationConvention::BloombergCdswClean,
            CdsValuationConvention::BloombergCdswCleanFullPremium,
        ] {
            let cds = contract(start, end, convention)?;
            let pricer = CdsPricer::new();
            let accrual = ((end - start).whole_days() + i64::from(convention.uses_clean_price()))
                as f64
                / 360.0;
            for as_of in [end, date!(2025 - 09 - 21), date!(2025 - 09 - 22)] {
                let (disc, _) = flat_market(as_of, 0.0)?;
                let expected_annuity = if as_of < date!(2025 - 09 - 22) {
                    accrual
                } else {
                    0.0
                };
                let expected_premium = expected_annuity * 10_000_000.0 * 0.01;
                close(
                    pricer.pv_premium_leg_raw(&cds, &disc, &surv, as_of)?,
                    expected_premium,
                );
                close(
                    pricer.premium_leg_pv_per_bp(&cds, &disc, &surv, as_of)? * 10_000_000.0 * 100.0,
                    expected_premium,
                );
                close(
                    pricer.risky_annuity(&cds, &disc, &surv, as_of)?,
                    expected_annuity,
                );
                close(
                    pricer.par_spread_denominator(&cds, &disc, &surv, as_of)?,
                    expected_annuity,
                );
                close(
                    pricer.npv_full(&cds, &disc, &surv, as_of)?,
                    -expected_premium,
                );
                let market = MarketContext::new().insert(disc).insert(surv.clone());
                close(
                    PreparedCds::new(&cds, &market, as_of)?.price(&surv)?,
                    -expected_premium,
                );
            }
        }
        Ok(())
    }

    #[test]
    fn cds_aod_cashflows_match_independent_integral_for_both_clean_denominators()
    -> finstack_quant_core::Result<()> {
        let as_of = date!(2025 - 01 - 15);
        let end = date!(2025 - 03 - 20);
        let lambda: f64 = 0.2;
        let (disc, surv) = flat_market(as_of, lambda)?;
        // Include a seasoned standard coupon, a front stub and a new coupon.
        for start in [date!(2024 - 12 - 20), date!(2025 - 01 - 01), as_of] {
            let mut values = Vec::new();
            let mut denominators = Vec::new();
            for convention in [
                CdsValuationConvention::BloombergCdswClean,
                CdsValuationConvention::BloombergCdswCleanFullPremium,
            ] {
                let cds = contract(start, end, convention)?;
                let pricer = CdsPricer::new();
                let t = (end - as_of).whole_days() as f64 / 365.0;
                let elapsed = (as_of - start).whole_days() as f64 / 360.0;
                let coupon = ((end - start).whole_days() + 1) as f64 / 360.0;
                // Independent integral of [elapsed + 365/360*t] times the
                // conditional exponential default density lambda*exp(-lambda*t).
                // Clean accrued settlement is a separate deterministic cashflow.
                let i0 = -(-lambda * t).exp_m1() / lambda;
                let i1 = (1.0 - (-lambda * t).exp() * (1.0 + lambda * t)) / lambda.powi(2);
                let default_accrual = lambda * (elapsed * i0 + 365.0 / 360.0 * i1);
                let expected_premium =
                    10_000_000.0 * 0.01 * (coupon * (-lambda * t).exp() + default_accrual);
                let premium = pricer.pv_premium_leg_raw(&cds, &disc, &surv, as_of)?;
                close(premium, expected_premium);
                close(
                    pricer.premium_leg_pv_per_bp(&cds, &disc, &surv, as_of)? * 100.0 * 10_000_000.0,
                    expected_premium,
                );
                let accrued = pricer.coupon_accrued_fraction(
                    &cds,
                    as_of,
                    AccrualDayCountPolicy::CdswInclusive,
                )? * 10_000_000.0
                    * 0.01;
                let expected_protection = 10_000_000.0 * 0.6 * (1.0 - (-lambda * t).exp());
                let npv = pricer.npv_full(&cds, &disc, &surv, as_of)?;
                close(npv, expected_protection - expected_premium + accrued);
                let market = MarketContext::new()
                    .insert(disc.clone())
                    .insert(surv.clone());
                close(PreparedCds::new(&cds, &market, as_of)?.price(&surv)?, npv);
                values.push(npv);
                denominators.push(pricer.par_spread_denominator(&cds, &disc, &surv, as_of)?);
            }
            close(values[0], values[1]);
            if start < as_of {
                assert!(denominators[1] > denominators[0]);
            }
        }
        Ok(())
    }
}
