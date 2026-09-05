//! Configuration, integration, and metric helpers for CDS pricing.
//!
use super::config::CDSPricerConfig;
use super::helpers::{
    date_from_hazard_time, haz_t, isda_standard_model_boundaries, settlement_date, sp_cond_to,
    validate_recovery_consistency,
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
pub(crate) struct CouponPeriod {
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
        Money::new(pv, cds.notional.currency())
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
        Money::new(pv, cds.notional.currency())
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
            let df = disc.df_between_dates(as_of, payment_date)?;

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
            .instrument_pricing_overrides
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
        )?;
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
            let df1 = inp.disc.df_between_dates(inp.as_of, settle1)?;
            let df2 = inp.disc.df_between_dates(inp.as_of, settle2)?;

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
                    .instrument_pricing_overrides
                    .model_config
                    .cds_act360_include_last_day
                {
                    bias_days += 0.5;
                }
                if inp
                    .cds
                    .instrument_pricing_overrides
                    .model_config
                    .cds_aod_half_day_bias
                {
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

/// Discount-side state reused across hazard bumps for bucketed CDS CS01.
///
/// Coupon dates, accruals, and payment discount factors do not change when
/// only the hazard curve is shocked. Protection and accrual-on-default still
/// re-integrate against the bumped survival curve.
pub(crate) struct CdsHazardRepriceCache {
    pricer: CDSPricer,
    cds: CreditDefaultSwap,
    disc: std::sync::Arc<DiscountCurve>,
    as_of: Date,
    periods: Vec<(CouponPeriod, f64, f64)>,
    spread: f64,
    upfront_pv: f64,
    upfront_adjustment: f64,
    clean_accrued: f64,
}

impl CdsHazardRepriceCache {
    /// Build the cache from the live discount curve and CDS schedule.
    ///
    /// # Arguments
    ///
    /// * `cds` - Live CDS whose premium schedule and valuation convention are cached.
    /// * `market` - Market supplying the premium-leg discount curve.
    /// * `as_of` - Valuation date used to drop expired coupons and compute DFs.
    pub(crate) fn try_new(
        cds: &CreditDefaultSwap,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> Result<Self> {
        use rust_decimal::prelude::ToPrimitive;

        let pricer = CDSPricer::with_config(CDSPricerConfig::from_cds(cds));
        let disc = market.get_discount(&cds.premium.discount_curve_id)?;
        let periods_raw = pricer.coupon_periods(cds, as_of)?;
        let mut periods = Vec::with_capacity(periods_raw.len());
        for period in periods_raw {
            if period.accrual_end <= as_of {
                continue;
            }
            let accrual = pricer.coupon_accrual(cds, &period)?;
            let df = disc.as_ref().df_between_dates(as_of, period.payment_date)?;
            periods.push((period, accrual, df));
        }
        let spread = cds.premium.spread_bp.to_f64().ok_or_else(|| {
            Error::Validation("premium spread_bp cannot be represented as f64".into())
        })? / BASIS_POINTS_PER_UNIT;
        let upfront_pv = match cds.upfront {
            Some((dt, amount)) if dt >= as_of => {
                amount.amount() * disc.as_ref().df_between_dates(as_of, dt)?
            }
            _ => 0.0,
        };
        let upfront_adjustment = cds
            .instrument_pricing_overrides
            .market_quotes
            .upfront_payment
            .map(|m| m.amount())
            .unwrap_or(0.0);
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
            upfront_adjustment,
            clean_accrued,
        })
    }

    /// Reprice after a hazard bump, reusing cached discount/premium structure.
    ///
    /// # Arguments
    ///
    /// * `surv` - Bumped or original survival/hazard curve used for protection
    ///   and accrual-on-default. Discount factors stay at the cached values.
    pub(crate) fn npv(&self, surv: &HazardCurve) -> Result<f64> {
        use super::helpers::sp_cond_to;
        use crate::instruments::credit_derivatives::cds::PayReceive;

        let protection_pv =
            self.pricer
                .pv_protection_leg_raw(&self.cds, self.disc.as_ref(), surv, self.as_of)?;
        let calendar = self
            .cds
            .premium
            .calendar_id
            .as_deref()
            .and_then(finstack_quant_core::dates::calendar::calendar_by_id);
        let mut premium_pv = 0.0;
        for &(period, accrual, df) in &self.periods {
            let sp = sp_cond_to(surv, self.as_of, period.accrual_end)?;
            premium_pv += self.cds.notional.amount() * self.spread * accrual * sp * df;
            if self.pricer.config.include_accrual {
                let spread_sign = self.spread.signum();
                premium_pv += spread_sign
                    * self.cds.notional.amount()
                    * self
                        .pricer
                        .accrual_on_default_isda_standard_model_cond(AodInputs {
                            cds: &self.cds,
                            spread: self.spread.abs(),
                            accrual_start_date: if matches!(
                                self.cds.valuation_convention,
                                crate::instruments::credit_derivatives::cds::CdsValuationConvention::BloombergCdswClean
                            ) {
                                period.accrual_start.max(self.as_of)
                            } else {
                                period.accrual_start
                            },
                            start_date: period.accrual_start.max(self.as_of),
                            end_date: period.accrual_end,
                            settlement_delay: self.cds.protection.settlement_delay,
                            calendar,
                            as_of: self.as_of,
                            disc: self.disc.as_ref(),
                            surv,
                        })?;
            }
        }

        let mut npv_amount = match self.cds.side {
            PayReceive::Pay => {
                protection_pv - premium_pv - self.upfront_pv - self.upfront_adjustment
            }
            PayReceive::Receive => {
                premium_pv - protection_pv + self.upfront_pv + self.upfront_adjustment
            }
        };
        if self.cds.uses_clean_price() {
            npv_amount = match self.cds.side {
                PayReceive::Pay => npv_amount + self.clean_accrued,
                PayReceive::Receive => npv_amount - self.clean_accrued,
            };
        }
        Ok(npv_amount)
    }
}

#[cfg(test)]
mod cds_hazard_reprice_cache_tests {
    use super::{CDSPricer, CDSPricerConfig, CdsHazardRepriceCache};
    use crate::instruments::common_impl::traits::Instrument;
    use crate::instruments::credit_derivatives::cds::{
        CdsConvention, CdsValuationConvention, CreditDefaultSwap, PayReceive,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use rust_decimal::Decimal;
    use time::macros::date;

    fn create_test_cds(
        valuation_convention: CdsValuationConvention,
    ) -> finstack_quant_core::Result<CreditDefaultSwap> {
        let mut cds = CreditDefaultSwap::new_isda(
            InstrumentId::new("CACHE-TEST-CDS"),
            Money::from((10_000_000_i64, Currency::USD)),
            PayReceive::Pay,
            CdsConvention::IsdaNa,
            Decimal::new(10_000, 2),
            date!(2024 - 12 - 20),
            date!(2030 - 03 - 20),
            0.40,
            CurveId::new("USD-OIS"),
            CurveId::new("TEST-CREDIT"),
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
    ) -> finstack_quant_core::Result<CdsHazardRepriceCache> {
        let cache = CdsHazardRepriceCache::try_new(cds, market, as_of)?;

        let base_cached = cache.npv(hazard)?;
        let base_full = cds.value(market, as_of)?.amount();
        assert_npv_matches(base_cached, base_full, "base hazard");

        let bumped_hazard = hazard.with_parallel_hazard_rate_bump_bp(1.0)?;
        let mut bumped_market = market.clone();
        bumped_market.insert_mut(bumped_hazard.clone());
        let bumped_cached = cache.npv(&bumped_hazard)?;
        let bumped_full = cds.value(&bumped_market, as_of)?.amount();
        assert_npv_matches(bumped_cached, bumped_full, "bumped hazard");
        assert!(
            (bumped_full - base_full).abs() > 1.0e-8,
            "parallel hazard bump should change the full CDS value"
        );

        Ok(cache)
    }

    #[test]
    fn cds_hazard_reprice_cache_matches_value_for_current_coupon_with_accrual_on_default(
    ) -> finstack_quant_core::Result<()> {
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
        assert!(
            cache.pricer.config.include_accrual,
            "CDSPricerConfig::from_cds should enable accrual-on-default"
        );

        let discount = market.get_discount(&cds.premium.discount_curve_id)?;
        let without_aod = CDSPricer::with_config(CDSPricerConfig {
            include_accrual: false,
            ..CDSPricerConfig::from_cds(&cds)
        })
        .npv_full(&cds, discount.as_ref(), &hazard, as_of)?;
        let with_aod = cds.value(&market, as_of)?.amount();
        assert!(
            (with_aod - without_aod).abs() > 1.0e-8,
            "fixture should produce a non-zero accrual-on-default contribution"
        );
        Ok(())
    }

    #[test]
    fn cds_hazard_reprice_cache_matches_value_for_bloomberg_clean_accrued(
    ) -> finstack_quant_core::Result<()> {
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
}
