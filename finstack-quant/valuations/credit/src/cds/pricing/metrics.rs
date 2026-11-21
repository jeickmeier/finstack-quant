//! Premium schedules, coupon accruals, par spread, risky annuity and PV01, and
//! full NPV on `CdsPricer`.

use super::engine::{AodInputs, CdsPricer, CouponPeriod};
use crate::cds::CreditDefaultSwap;
use crate::constants::{credit, numerical};
use finstack_quant_core::dates::{Date, DayCountContext, Schedule, StubKind, Tenor};
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::math::units::{BASIS_POINTS_PER_UNIT, ONE_BASIS_POINT};
use finstack_quant_core::{Error, Result};
use finstack_quant_valuations_core::instruments::CdsDateRegime;
use rust_decimal::prelude::ToPrimitive;

impl CdsPricer {
    /// Generate contractual accrual and payment dates from the premium terms.
    fn premium_schedule(&self, cds: &CreditDefaultSwap) -> Result<Schedule> {
        let builder = match cds.premium_leg.date_regime {
            CdsDateRegime::Standard => {
                if cds.premium_leg.frequency != Tenor::quarterly() {
                    return Err(Error::Validation(
                        "premium.date_regime = standard requires quarterly frequency; set \
                         premium.date_regime = bespoke for bespoke premium terms"
                            .into(),
                    ));
                }
                // The standard regime runs quarterly on CDS roll dates from the
                // roll on or before the premium start, with a short final period
                // for an off-grid maturity; it owns that stub (record
                // CALIBRATION-FRAMEWORK-007, O7).
                let anchor = if finstack_quant_core::dates::is_cds_date(cds.premium_leg.start) {
                    cds.premium_leg.start
                } else {
                    finstack_quant_core::dates::prev_cds_date(cds.premium_leg.start)?
                };
                Schedule::builder(anchor, cds.premium_leg.end)?
                    .frequency(Tenor::quarterly())
                    .stub_rule(StubKind::ShortBack)
            }
            CdsDateRegime::Bespoke => {
                Schedule::builder(cds.premium_leg.start, cds.premium_leg.end)?
                    .frequency(cds.premium_leg.frequency)
                    .stub_rule(cds.premium_leg.stub)
            }
        };
        let mut builder = builder.grid(finstack_quant_core::dates::GridMode::Natural);
        // The CDS product adjusts premium dates only when the leg names a
        // calendar; without one its dates stay unadjusted, stated explicitly
        // here. Whether a convention without a calendar should be accepted is
        // the CDS date regime's decision (record CORE-CONVENTIONS-008 O7).
        builder = match cds.premium_leg.calendar_id.as_deref() {
            Some(calendar_id) => {
                builder.adjust_with(cds.premium_leg.business_day_convention, calendar_id)
            }
            None => builder.unadjusted(),
        };
        let schedule = builder.build()?;
        // The standard grid includes the previous roll date. The explicit
        // premium start remains the contractual first accrual boundary; the
        // replaced grid is revalidated rather than mutated in place.
        let mut dates = schedule.get_dates().to_vec();
        dates[0] = cds.premium_leg.start;
        Schedule::try_new(
            dates,
            schedule.get_payment_dates().to_vec(),
            schedule.get_fixing_dates().to_vec(),
        )
    }

    pub(crate) fn generate_isda_schedule(&self, cds: &CreditDefaultSwap) -> Result<Vec<Date>> {
        let schedule = self.premium_schedule(cds)?;
        let mut dates = vec![cds.premium_leg.start];
        dates.extend(schedule.get_payment_dates());
        Ok(dates)
    }

    pub(crate) fn coupon_periods(
        &self,
        cds: &CreditDefaultSwap,
        _as_of: Date,
    ) -> Result<Vec<CouponPeriod>> {
        if cds.premium_leg.start >= cds.premium_leg.end {
            return Ok(Vec::new());
        }
        let schedule = self.premium_schedule(cds)?;
        let adjusted = cds.uses_adjusted_premium_accrual_dates();
        Ok(schedule
            .get_dates()
            .windows(2)
            .enumerate()
            .map(|(idx, dates)| {
                let payment_date = schedule.get_payment_dates()[idx];
                CouponPeriod {
                    accrual_start: if adjusted && idx > 0 {
                        schedule.get_payment_dates()[idx - 1]
                    } else {
                        dates[0]
                    },
                    // Contractual final accrual is unadjusted; Following
                    // determines payment only. Preserve the explicit QuantLib
                    // parity schedule separately.
                    accrual_end: if adjusted
                        && (idx + 1 < schedule.get_payment_dates().len()
                            || cds.valuation_convention.act360_includes_last_day())
                    {
                        payment_date
                    } else {
                        dates[1]
                    },
                    payment_date,
                    is_final: idx + 1 == schedule.get_payment_dates().len(),
                }
            })
            .collect())
    }

    pub(crate) fn coupon_accrual(
        &self,
        cds: &CreditDefaultSwap,
        period: &CouponPeriod,
    ) -> Result<f64> {
        // QuantLib `Actual360(true)` parity: under the `quant_lib_isda_parity`
        // valuation convention every Act/360 accrual period is inclusive of
        // its end date. Production CDS pricing uses the standard Bloomberg
        // CDSW rule below.
        if cds.valuation_convention.act360_includes_last_day()
            && cds.premium_leg.day_count == finstack_quant_core::dates::DayCount::Act360
            && period.accrual_end > period.accrual_start
        {
            let days = (period.accrual_end - period.accrual_start).whole_days() + 1;
            return Ok((days.max(0) as f64) / 360.0);
        }
        // The contractual final Act/360 coupon includes the unadjusted
        // maturity date. Rolling payment by multiple days never extends it.
        if cds.uses_adjusted_premium_accrual_dates()
            && cds.premium_leg.day_count == finstack_quant_core::dates::DayCount::Act360
            && period.is_final
            && period.accrual_end > period.accrual_start
        {
            let days = (period.accrual_end - period.accrual_start).whole_days() + 1;
            return Ok((days.max(0) as f64) / 360.0);
        }

        cds.premium_leg.day_count.year_fraction(
            period.accrual_start,
            period.accrual_end,
            DayCountContext::default(),
        )
    }

    pub(crate) fn premium_cashflow_accruals(
        &self,
        cds: &CreditDefaultSwap,
        as_of: Date,
    ) -> Result<Vec<(Date, f64)>> {
        self.coupon_periods(cds, as_of)?
            .into_iter()
            .map(|period| Ok((period.payment_date, self.coupon_accrual(cds, &period)?)))
            .collect()
    }

    /// Premium accrued up to the default date, using the jump-to-default
    /// convention's exclusive cutoff and rolled payment dates.
    pub(crate) fn default_accrued_fraction(
        &self,
        cds: &CreditDefaultSwap,
        as_of: Date,
    ) -> Result<f64> {
        if as_of <= cds.premium_leg.start || as_of >= cds.premium_leg.end {
            return Ok(0.0);
        }
        let schedule = self.generate_isda_schedule(cds)?;
        let last_coupon = schedule
            .into_iter()
            .take_while(|&coupon_date| coupon_date <= as_of)
            .last()
            .unwrap_or(cds.premium_leg.start);
        cds.premium_leg
            .day_count
            .year_fraction(last_coupon, as_of, DayCountContext::default())
    }

    pub(crate) fn clean_quote_accrual_fraction(
        &self,
        cds: &CreditDefaultSwap,
        as_of: Date,
    ) -> Result<f64> {
        if as_of <= cds.premium_leg.start || as_of >= cds.premium_leg.end {
            return Ok(0.0);
        }
        let periods = self.coupon_periods(cds, as_of)?;
        let Some(period) = periods
            .iter()
            .find(|period| period.accrual_start <= as_of && as_of < period.accrual_end)
        else {
            return Ok(0.0);
        };
        if period.accrual_start == as_of {
            return Ok(0.0);
        }
        let close = as_of.next_day().ok_or_else(|| {
            Error::Validation("CDS quote valuation close is outside the date range".into())
        })?;
        cds.premium_leg.day_count.year_fraction(
            period.accrual_start,
            close,
            DayCountContext::default(),
        )
    }

    fn clean_par_spread_denominator(
        &self,
        cds: &CreditDefaultSwap,
        disc: &DiscountCurve,
        surv: &HazardCurve,
        as_of: Date,
    ) -> Result<f64> {
        let full_premium_per_bp = self.premium_leg_pv_per_bp(cds, disc, surv, as_of)?;
        let accrued_per_unit_spread = self.clean_quote_accrual_fraction(cds, as_of)?;
        Ok(full_premium_per_bp / ONE_BASIS_POINT - accrued_per_unit_spread)
    }

    /// Calculate par spread (bp) that sets NPV to zero.
    ///
    /// # Par spread definition
    ///
    /// ```text
    /// Par Spread = Protection_PV / Denominator
    /// ```
    ///
    /// The denominator is selected by the instrument's `valuation_convention`
    /// (see [`Self::par_spread_denominator`]). The default,
    /// `BloombergCdswClean`, uses the premium-leg annuity **including**
    /// accrual-on-default minus the accrued fraction.
    /// The dirty quote uses the full premium-leg annuity. Accrual-on-default matters most for distressed
    /// credits (typically 2-5 bp for hazard > 3%).
    ///
    /// # Returns
    ///
    /// Par spread in basis points (bp).
    /// Per-unit-notional, per-unit-spread denominator used by [`Self::par_spread`].
    ///
    /// Selects the convention from the instrument's `valuation_convention`:
    ///
    /// * full-premium denominator (`BloombergCdswCleanFullPremium`,
    ///   `QuantLibIsdaParity`) → full premium leg PV per unit
    ///   spread including accrual-on-default, with no accrued subtraction.
    /// * `cds.uses_clean_price()` → clean-price denominator (premium leg per unit
    ///   spread including accrual-on-default, minus the accrued fraction;
    ///   `BloombergCdswClean`, the default).
    /// * Otherwise (`IsdaDirty`) → full premium-leg annuity.
    ///
    /// All branches return values in the same units (per unit notional, per
    /// unit spread), so callers can reuse the result for index aggregation
    /// or other consistent par-spread calculations.
    #[must_use = "denominator computation is pure"]
    pub(crate) fn par_spread_denominator(
        &self,
        cds: &CreditDefaultSwap,
        disc: &DiscountCurve,
        surv: &HazardCurve,
        as_of: Date,
    ) -> Result<f64> {
        if cds.uses_full_premium_par_spread_denominator() {
            self.premium_leg_annuity(cds, disc, surv, as_of, 1.0, true)
        } else if cds.uses_clean_price() {
            self.clean_par_spread_denominator(cds, disc, surv, as_of)
        } else {
            self.premium_leg_annuity(cds, disc, surv, as_of, 1.0, true)
        }
    }

    #[must_use = "par spread calculation is pure computation"]
    pub(crate) fn par_spread(
        &self,
        cds: &CreditDefaultSwap,
        disc: &DiscountCurve,
        surv: &HazardCurve,
        as_of: Date,
    ) -> Result<f64> {
        let protection_pv = self.pv_protection_leg(cds, disc, surv, as_of)?;
        let denom = self.par_spread_denominator(cds, disc, surv, as_of)?;

        // Guard `denom < tolerance` (not `denom.abs() < tolerance`): for the
        // clean (Bloomberg) convention the denominator is `risky_annuity -
        // accrued` and can go negative for a seasoned CDS near a coupon date.
        // An `abs()` guard would let a negative denominator through and produce
        // a negative par spread, violating the par-spread positivity invariant.
        if denom < numerical::RATE_COMPARISON_TOLERANCE {
            return Err(Error::Validation(
                "Par spread denominator is non-positive or too small (risky \
                 annuity ≈ 0, or clean denominator went negative for a seasoned \
                 CDS). This may indicate zero survival probability, an expired \
                 CDS, or accrued exceeding the remaining risky annuity."
                    .to_string(),
            ));
        }

        Ok(protection_pv.amount() / (denom * cds.notional.amount()) * BASIS_POINTS_PER_UNIT)
    }

    /// Premium leg PV per 1 bp of spread per unit notional, including
    /// accrual-on-default.
    ///
    /// Uses proper time-axis conventions for discounting and survival.
    #[must_use = "premium leg calculation is pure computation"]
    pub(crate) fn premium_leg_pv_per_bp(
        &self,
        cds: &CreditDefaultSwap,
        disc: &DiscountCurve,
        surv: &HazardCurve,
        as_of: Date,
    ) -> Result<f64> {
        self.premium_leg_annuity(cds, disc, surv, as_of, ONE_BASIS_POINT, true)
    }

    /// Risky annuity: survival-weighted duration of premium payments.
    ///
    /// This is the sum of `DF(t) × SP(t) × YearFrac` across all coupon periods.
    /// Used as the denominator in ISDA standard par spread calculation.
    ///
    /// Uses proper time-axis conventions for discounting and survival.
    #[must_use = "risky annuity calculation is pure computation"]
    pub(crate) fn risky_annuity(
        &self,
        cds: &CreditDefaultSwap,
        disc: &DiscountCurve,
        surv: &HazardCurve,
        as_of: Date,
    ) -> Result<f64> {
        self.premium_leg_annuity(cds, disc, surv, as_of, 1.0, false)
    }

    /// Premium leg PV per unit notional at a running `spread` (decimal),
    /// `Σ spread · τ · DF · SP` over unsettled coupons, plus the
    /// accrual-on-default integral when `include_aod` is set.
    ///
    /// Discounting is `df(t_pay) / df(as_of)` on the discount curve's day
    /// count; survival is conditional on no default before `as_of`. Completed
    /// coupons are certain receivables even below the survival floor.
    pub(super) fn premium_leg_annuity(
        &self,
        cds: &CreditDefaultSwap,
        disc: &DiscountCurve,
        surv: &HazardCurve,
        as_of: Date,
        spread: f64,
        include_aod: bool,
    ) -> Result<f64> {
        let periods = self.coupon_periods(cds, as_of)?;
        let calendar = if include_aod {
            cds.premium_leg
                .calendar_id
                .as_deref()
                .map(finstack_quant_core::dates::calendar_by_id_strict)
                .transpose()?
        } else {
            None
        };

        // Pre-compute year fractions once to avoid repeated day-count calls.
        let disc_day_count = disc.day_count();
        let disc_base = disc.base_date();
        let haz_day_count = surv.day_count();
        let haz_base = surv.base_date();
        let t_asof_disc =
            disc_day_count.year_fraction(disc_base, as_of, DayCountContext::default())?;
        let t_asof_haz =
            haz_day_count.year_fraction(haz_base, as_of, DayCountContext::default())?;
        let df_asof = disc.df(t_asof_disc)?;
        let sp_asof = surv.sp(t_asof_haz)?;
        let sp_floor = credit::SURVIVAL_PROBABILITY_FLOOR;
        let sp_cond_denom = (sp_asof > sp_floor).then_some(sp_asof);

        let mut annuity = 0.0;
        for period in periods {
            let start_date = period.accrual_start;
            let end_date = period.accrual_end;
            let payment_date = period.payment_date;

            if !crate::cds::cash_is_eligible(
                payment_date,
                as_of,
                cds.include_valuation_date_cashflows,
            ) {
                continue;
            }

            // Accrual uses instrument day-count
            let accrual = self.coupon_accrual(cds, &period)?;

            // Discount factor: df(payment) / df(as_of)
            let t_pay = disc_day_count.year_fraction(
                disc_base,
                payment_date,
                DayCountContext::default(),
            )?;
            let df = disc.df(t_pay)? / df_asof;

            // Completed coupons are fixed receivables even if historical
            // absolute survival lies below the conditional-ratio floor.
            let sp = if end_date <= as_of {
                1.0
            } else if let Some(denom) = sp_cond_denom {
                let t_end =
                    haz_day_count.year_fraction(haz_base, end_date, DayCountContext::default())?;
                surv.sp(t_end)? / denom
            } else {
                0.0
            };

            annuity += spread * accrual * sp * df;

            if include_aod {
                annuity += self.accrual_on_default_isda_standard_model_cond(AodInputs {
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
                })?;
            }
        }
        Ok(annuity)
    }

    /// Risky PV01: PV of 1bp of running premium on the notional, on the
    /// instrument's valuation convention.
    ///
    /// Computed as `annuity × Notional / 10000`:
    ///
    /// * Bloomberg CDSW clean conventions use the clean annuity, the premium
    ///   leg per unit spread including accrual-on-default less accrued
    ///   premium, so that `clean NPV = (par spread − coupon) × Risky PV01`
    ///   for a protection buyer on [`CdsValuationConvention::BloombergCdswClean`].
    /// * Dirty conventions use [`Self::risky_annuity`], the coupon-only
    ///   annuity (QuantLib `couponLegBPS`).
    ///
    /// It is an annuity, not a bump-and-reprice sensitivity: the PV change for
    /// a 1bp move in the quoted spread is the `cs01` metric (Bloomberg CDSW
    /// "Spread DV01"), which also carries the annuity's own spread sensitivity
    /// when the coupon differs from the par spread.
    ///
    /// [`CdsValuationConvention::BloombergCdswClean`]: finstack_quant_valuations_core::market::conventions::CdsValuationConvention::BloombergCdswClean
    #[must_use = "risky PV01 calculation is pure computation"]
    #[doc(hidden)]
    pub fn risky_pv01(
        &self,
        cds: &CreditDefaultSwap,
        disc: &DiscountCurve,
        surv: &HazardCurve,
        as_of: Date,
    ) -> Result<f64> {
        let annuity = if cds.uses_clean_price() {
            self.clean_par_spread_denominator(cds, disc, surv, as_of)?
        } else {
            self.risky_annuity(cds, disc, surv, as_of)?
        };
        Ok(annuity * cds.notional.amount() / BASIS_POINTS_PER_UNIT)
    }

    /// Canonical CDS instrument NPV from the holder's side, including dated
    /// upfront cash at its discount factor and the Bloomberg CDSW clean-price
    /// accrued add-back when `cds.uses_clean_price()` is `true`.
    pub(crate) fn npv_full(
        &self,
        cds: &CreditDefaultSwap,
        disc: &DiscountCurve,
        surv: &HazardCurve,
        as_of: Date,
    ) -> Result<f64> {
        let protection_pv = self.pv_protection_leg_raw(cds, disc, surv, as_of)?;
        let premium_pv = self.pv_premium_leg_raw(cds, disc, surv, as_of)?;

        // Positive upfront is paid by the buyer. The coupon cash boundary also applies here.
        let upfront_pv = match cds.upfront {
            Some((dt, amount))
                if crate::cds::cash_is_eligible(
                    dt,
                    as_of,
                    cds.include_valuation_date_cashflows,
                ) =>
            {
                amount.amount() * disc.df_between_dates(as_of, dt)?
            }
            _ => 0.0,
        };

        let clean_accrued = if cds.uses_clean_price() {
            let accrual_fraction = self.clean_quote_accrual_fraction(cds, as_of)?;
            let spread = cds.premium_leg.coupon_bp.to_f64().ok_or_else(|| {
                Error::Validation("premium.coupon_bp cannot be represented as f64".into())
            })? / BASIS_POINTS_PER_UNIT;
            cds.notional.amount() * spread * accrual_fraction
        } else {
            0.0
        };
        Ok(super::engine::signed_npv(
            cds,
            protection_pv,
            premium_pv,
            upfront_pv,
            clean_accrued,
        ))
    }
}
