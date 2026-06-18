use super::engine::{AodInputs, CDSPricer, CouponPeriod};
use super::helpers::{df_asof_to, sp_cond_to};
use crate::constants::{credit, numerical, BASIS_POINTS_PER_UNIT, ONE_BASIS_POINT};
use crate::instruments::common_impl::helpers::year_fraction;
use crate::instruments::credit_derivatives::cds::{
    CdsValuationConvention, CreditDefaultSwap, PayReceive,
};
use finstack_quant_core::dates::{adjust, next_cds_date, Date, DayCountContext};
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::{Error, Result};
use rust_decimal::prelude::ToPrimitive;

/// Day-count policy applied when measuring premium accrual from the most
/// recent coupon date to `as_of`.
///
/// The two variants share the same schedule walk but differ on `Act/360`
/// in-period accrual:
///
/// - [`Self::CdswInclusive`]: Bloomberg CDSW clean-settlement convention,
///   `Act/360` accrual is inclusive of the upper boundary (one extra day).
///   Other day-counts use plain `year_fraction`.
/// - [`Self::IsdaStandard`]: plain `year_fraction` in every day-count;
///   matches the ISDA default-payment convention used for jump-to-default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccrualDayCountPolicy {
    /// Bloomberg CDSW: `Act/360` inclusive of the last day; other day-counts
    /// use plain `year_fraction`.
    CdswInclusive,
    /// Standard ISDA: plain `year_fraction` in every day-count.
    IsdaStandard,
}

impl CDSPricer {
    /// Generate ISDA standard coupon dates (20th of Mar/Jun/Sep/Dec).
    ///
    /// Payment dates are adjusted using the CDS calendar and business day
    /// convention (Modified Following per ISDA 2014 standard). If no calendar
    /// is specified, dates are returned unadjusted.
    pub(crate) fn generate_isda_schedule(&self, cds: &CreditDefaultSwap) -> Result<Vec<Date>> {
        let mut schedule = vec![cds.premium.start];
        let mut current = cds.premium.start;

        // Resolve calendar for business day adjustment
        let calendar = cds
            .premium
            .calendar_id
            .as_deref()
            .and_then(finstack_quant_core::dates::calendar::calendar_by_id);

        while current < cds.premium.end {
            current = next_cds_date(current);
            if current <= cds.premium.end {
                // Apply business day adjustment if calendar is available.
                // Adjustment failure (e.g. unparseable holiday data) propagates
                // rather than silently using an unadjusted date, which would
                // produce wrong premium accrual periods.
                let adjusted = if let Some(cal) = calendar {
                    adjust(current, cds.premium.bdc, cal)?
                } else {
                    current
                };
                schedule.push(adjusted);
            }
        }

        // Handle maturity date - ensure it's in the schedule
        let maturity_adjusted = if let Some(cal) = calendar {
            adjust(cds.premium.end, cds.premium.bdc, cal)?
        } else {
            cds.premium.end
        };

        if schedule.last() != Some(&maturity_adjusted) {
            schedule.push(maturity_adjusted);
        }

        Ok(schedule)
    }

    pub(super) fn coupon_periods(
        &self,
        cds: &CreditDefaultSwap,
        as_of: Date,
    ) -> Result<Vec<CouponPeriod>> {
        self.generate_isda_coupon_periods(cds, as_of)
    }

    fn generate_isda_coupon_periods(
        &self,
        cds: &CreditDefaultSwap,
        _as_of: Date,
    ) -> Result<Vec<CouponPeriod>> {
        if cds.uses_adjusted_premium_accrual_dates() {
            // Degenerate schedules (start >= end) have no future premium
            // cashflows. Returning an empty list mirrors the unadjusted
            // ISDA path and avoids spurious one-day phantom periods that
            // can appear when the maturity is on a holiday and gets
            // business-day-adjusted forward.
            if cds.premium.start >= cds.premium.end {
                return Ok(Vec::new());
            }
            let schedule = self.generate_isda_schedule(cds)?;
            return Ok(schedule
                .windows(2)
                .enumerate()
                .map(|window| CouponPeriod {
                    accrual_start: window.1[0],
                    accrual_end: window.1[1],
                    payment_date: window.1[1],
                    is_final: window.0 + 2 == schedule.len(),
                })
                .collect());
        }

        let mut accrual_dates = vec![cds.premium.start];
        let mut current = cds.premium.start;
        let calendar = cds
            .premium
            .calendar_id
            .as_deref()
            .and_then(finstack_quant_core::dates::calendar::calendar_by_id);

        while current < cds.premium.end {
            current = next_cds_date(current);
            if current <= cds.premium.end {
                accrual_dates.push(current);
            }
        }
        if accrual_dates.last() != Some(&cds.premium.end) {
            accrual_dates.push(cds.premium.end);
        }

        let mut periods = Vec::with_capacity(accrual_dates.len().saturating_sub(1));
        for (idx, window) in accrual_dates.windows(2).enumerate() {
            let payment_date = if let Some(cal) = calendar {
                adjust(window[1], cds.premium.bdc, cal)?
            } else {
                window[1]
            };
            periods.push(CouponPeriod {
                accrual_start: window[0],
                accrual_end: window[1],
                payment_date,
                is_final: idx + 2 == accrual_dates.len(),
            });
        }
        Ok(periods)
    }

    pub(super) fn coupon_accrual(
        &self,
        cds: &CreditDefaultSwap,
        period: &CouponPeriod,
    ) -> Result<f64> {
        // QuantLib `Actual360(true)` parity: when explicitly requested,
        // every Act/360 accrual period is inclusive of its end date. This
        // is opt-in via `cds_act360_include_last_day` for QuantLib parity
        // tests; production CDS pricing uses the standard Bloomberg CDSW
        // rule below.
        if cds
            .pricing_overrides
            .model_config
            .cds_act360_include_last_day
            && cds.premium.day_count == finstack_quant_core::dates::DayCount::Act360
            && period.accrual_end > period.accrual_start
        {
            let days = finstack_quant_core::dates::DayCount::calendar_days(
                period.accrual_start,
                period.accrual_end,
            ) + 1;
            return Ok((days.max(0) as f64) / 360.0);
        }
        // CDSW final-coupon convention: the final Act/360 premium period is
        // inclusive of the maturity date (one extra calendar day). The rule
        // is the canonical Bloomberg CDSW behaviour and is shared by every
        // pricer/convention that uses business-day-adjusted accrual periods
        // (`uses_adjusted_premium_accrual_dates()` — currently
        // `BloombergCdswClean` plus the explicit override flag). The CDS
        // option synthetic underlying and CDS tranche index legs both type
        // their underlying CDS as Bloomberg-clean so they pick up this rule
        // automatically.
        //
        // The `payment_date == cds.premium.end` guard avoids double-counting
        // when business-day adjustment has already pushed the final accrual
        // boundary past the unadjusted maturity (e.g. a Sunday IMM rolling
        // forward to Monday). In that case the BDA shift already accounts
        // for the extra calendar day(s) and the +1-day rule must not apply
        // on top of it.
        if cds.uses_adjusted_premium_accrual_dates()
            && cds.premium.day_count == finstack_quant_core::dates::DayCount::Act360
            && period.is_final
            && period.payment_date == cds.premium.end
            && period.accrual_end > period.accrual_start
        {
            let days = finstack_quant_core::dates::DayCount::calendar_days(
                period.accrual_start,
                period.accrual_end,
            ) + 1;
            return Ok((days.max(0) as f64) / 360.0);
        }

        year_fraction(
            cds.premium.day_count,
            period.accrual_start,
            period.accrual_end,
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

    /// Year fraction of premium accrued from the most recent coupon date to
    /// `as_of`, expressed in the instrument's premium-leg day-count.
    ///
    /// CDS desks use two slightly different conventions for the in-period
    /// accrual:
    ///
    /// - [`AccrualDayCountPolicy::CdswInclusive`] — Bloomberg CDSW clean
    ///   settlement adds one extra calendar day for `Act/360` (so 2026-03-20
    ///   to 2026-05-02 reads as 44 days). Other day-counts use plain
    ///   `year_fraction`.
    /// - [`AccrualDayCountPolicy::IsdaStandard`] — plain `year_fraction` in
    ///   every day-count. Used for the jump-to-default accrued-premium
    ///   calculation.
    ///
    /// Returns `0.0` when `as_of` is at or outside the premium-leg window
    /// (`<= start` or `>= end`).
    pub(crate) fn coupon_accrued_fraction(
        &self,
        cds: &CreditDefaultSwap,
        as_of: Date,
        policy: AccrualDayCountPolicy,
    ) -> Result<f64> {
        if as_of <= cds.premium.start || as_of >= cds.premium.end {
            return Ok(0.0);
        }

        let schedule = self.generate_isda_schedule(cds)?;
        let mut last_coupon = cds.premium.start;
        for &coupon_date in &schedule {
            if coupon_date <= as_of {
                last_coupon = coupon_date;
            } else {
                break;
            }
        }

        if matches!(policy, AccrualDayCountPolicy::CdswInclusive)
            && cds.premium.day_count == finstack_quant_core::dates::DayCount::Act360
        {
            let days = finstack_quant_core::dates::DayCount::calendar_days(last_coupon, as_of) + 1;
            return Ok((days.max(0) as f64) / 360.0);
        }

        year_fraction(cds.premium.day_count, last_coupon, as_of)
    }

    fn clean_par_spread_denominator(
        &self,
        cds: &CreditDefaultSwap,
        disc: &DiscountCurve,
        surv: &HazardCurve,
        as_of: Date,
    ) -> Result<f64> {
        let full_premium_per_bp = self.premium_leg_pv_per_bp(cds, disc, surv, as_of)?;
        let accrued_per_unit_spread =
            self.coupon_accrued_fraction(cds, as_of, AccrualDayCountPolicy::CdswInclusive)?;
        Ok(full_premium_per_bp / ONE_BASIS_POINT - accrued_per_unit_spread)
    }

    /// Calculate par spread (bps) that sets NPV to zero.
    ///
    /// # ISDA Standard Par Spread Definition
    ///
    /// By default (when `par_spread_uses_full_premium = false`), this implements
    /// the **ISDA standard definition**:
    ///
    /// ```text
    /// Par Spread = Protection_PV / Risky_Annuity
    /// ```
    ///
    /// where `Risky_Annuity` is the sum of `DF(t) * SP(t) * YearFrac` across
    /// coupon periods, **excluding** accrual-on-default from the denominator.
    ///
    /// # Par Spread with Full Premium Leg
    ///
    /// When `par_spread_uses_full_premium = true`, the denominator includes the
    /// full premium leg PV (with accrual-on-default) per basis point. This matches
    /// Bloomberg CDSW-style calculations and is appropriate for distressed credits
    /// where accrual-on-default has material impact (typically 2-5 bps for hazard > 3%).
    ///
    /// # Returns
    ///
    /// Par spread in basis points (bps).
    /// Per-unit-notional, per-unit-spread denominator used by [`Self::par_spread`].
    ///
    /// Selects the right convention based on `config.par_spread_uses_full_premium`
    /// and the instrument's `valuation_convention`:
    ///
    /// * `par_spread_uses_full_premium = true` → full premium leg PV per unit
    ///   spread including accrual-on-default (Bloomberg CDSW
    ///   `BloombergCdswCleanFullPremium` and QuantLib parity behaviour).
    /// * `cds.uses_clean_price()` → clean-price denominator (premium leg per unit
    ///   spread minus accrued fraction; Bloomberg CDSW default).
    /// * Otherwise → ISDA standard risky annuity (`Σ DF·SP·τ`).
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
        if self.config.par_spread_uses_full_premium {
            let periods = self.coupon_periods(cds, as_of)?;
            let calendar = cds
                .premium
                .calendar_id
                .as_deref()
                .and_then(finstack_quant_core::dates::calendar::calendar_by_id);
            let mut ann = 0.0;
            for period in periods {
                let start_date = period.accrual_start;
                let end_date = period.accrual_end;
                let payment_date = period.payment_date;
                if end_date <= as_of {
                    continue;
                }
                let accrual = self.coupon_accrual(cds, &period)?;
                let df = df_asof_to(disc, as_of, payment_date)?;
                let sp = sp_cond_to(surv, as_of, end_date)?;
                let unit_spread = 1.0;
                ann += unit_spread * accrual * sp * df;
                ann += self.accrual_on_default_isda_standard_model_cond(AodInputs {
                    cds,
                    spread: unit_spread,
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
            Ok(ann)
        } else if cds.uses_clean_price() {
            self.clean_par_spread_denominator(cds, disc, surv, as_of)
        } else {
            self.risky_annuity(cds, disc, surv, as_of)
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

    /// Premium leg PV per 1 bp of spread, including accrual-on-default if configured.
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
        let periods = self.coupon_periods(cds, as_of)?;
        let calendar = cds
            .premium
            .calendar_id
            .as_deref()
            .and_then(finstack_quant_core::dates::calendar::calendar_by_id);

        // Pre-compute year fractions once to avoid repeated day-count calls.
        let disc_dc = disc.day_count();
        let disc_base = disc.base_date();
        let haz_dc = surv.day_count();
        let haz_base = surv.base_date();
        let t_asof_disc = disc_dc.year_fraction(disc_base, as_of, DayCountContext::default())?;
        let t_asof_haz = haz_dc.year_fraction(haz_base, as_of, DayCountContext::default())?;
        let df_asof = disc.df(t_asof_disc);
        let sp_asof = surv.sp(t_asof_haz);
        let sp_floor = credit::SURVIVAL_PROBABILITY_FLOOR;
        let sp_cond_denom = if sp_asof > sp_floor {
            sp_asof
        } else {
            return Ok(0.0);
        };

        let mut per_bp_pv = 0.0;
        for period in periods {
            let start_date = period.accrual_start;
            let end_date = period.accrual_end;
            let payment_date = period.payment_date;

            // Skip periods that have already ended before as_of
            if end_date <= as_of {
                continue;
            }

            // Accrual uses instrument day-count
            let accrual = self.coupon_accrual(cds, &period)?;

            // Discount factor: df(payment) / df(as_of)
            let t_pay =
                disc_dc.year_fraction(disc_base, payment_date, DayCountContext::default())?;
            let df = disc.df(t_pay) / df_asof;

            // Conditional survival: sp(end) / sp(as_of)
            let t_end = haz_dc.year_fraction(haz_base, end_date, DayCountContext::default())?;
            let sp = surv.sp(t_end) / sp_cond_denom;

            per_bp_pv += ONE_BASIS_POINT * accrual * sp * df;

            if self.config.include_accrual {
                per_bp_pv += self.accrual_on_default_isda_standard_model_cond(AodInputs {
                    cds,
                    spread: ONE_BASIS_POINT,
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
        Ok(per_bp_pv)
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
        let periods = self.coupon_periods(cds, as_of)?;

        // Pre-compute year fractions once to avoid repeated day-count calls.
        let disc_dc = disc.day_count();
        let disc_base = disc.base_date();
        let haz_dc = surv.day_count();
        let haz_base = surv.base_date();
        let t_asof_disc = disc_dc.year_fraction(disc_base, as_of, DayCountContext::default())?;
        let t_asof_haz = haz_dc.year_fraction(haz_base, as_of, DayCountContext::default())?;
        let df_asof = disc.df(t_asof_disc);
        let sp_asof = surv.sp(t_asof_haz);
        let sp_floor = credit::SURVIVAL_PROBABILITY_FLOOR;
        let sp_cond_denom = if sp_asof > sp_floor {
            sp_asof
        } else {
            return Ok(0.0);
        };

        let mut annuity = 0.0;
        for period in periods {
            let end_date = period.accrual_end;
            let payment_date = period.payment_date;

            // Skip periods that have already ended before as_of
            if end_date <= as_of {
                continue;
            }

            // Accrual uses instrument day-count
            let accrual = self.coupon_accrual(cds, &period)?;

            // Discount factor: df(payment) / df(as_of)
            let t_pay =
                disc_dc.year_fraction(disc_base, payment_date, DayCountContext::default())?;
            let df = disc.df(t_pay) / df_asof;

            // Conditional survival: sp(end) / sp(as_of)
            let t_end = haz_dc.year_fraction(haz_base, end_date, DayCountContext::default())?;
            let sp = surv.sp(t_end) / sp_cond_denom;

            annuity += accrual * sp * df;
        }
        Ok(annuity)
    }

    /// Risky PV01: change in NPV for a 1bp increase in the CDS spread.
    ///
    /// Computed as `Risky Annuity × Notional / 10000`.
    #[must_use = "risky PV01 calculation is pure computation"]
    pub(crate) fn risky_pv01(
        &self,
        cds: &CreditDefaultSwap,
        disc: &DiscountCurve,
        surv: &HazardCurve,
        as_of: Date,
    ) -> Result<f64> {
        let risky_annuity = self.risky_annuity(cds, disc, surv, as_of)?;
        Ok(risky_annuity * cds.notional.amount() / BASIS_POINTS_PER_UNIT)
    }

    /// Canonical CDS instrument NPV from the perspective of the `PayReceive`
    /// side, including both upfront-payment forms and the clean-price accrued
    /// add-back.
    ///
    /// Layered components (raw `f64`):
    ///
    /// 1. **Leg PV with sign:** Protection PV − Premium PV (for `Pay`),
    ///    Premium PV − Protection PV (for `Receive`).
    /// 2. **Dated upfront** (`cds.upfront: Option<(Date, Money)>`): a specific
    ///    payment on a specific date, discounted from `as_of`. Positive
    ///    amount = paid by Buyer (reduces Buyer NPV).
    /// 3. **PV-adjustment upfront**
    ///    (`cds.pricing_overrides.market_quotes.upfront_payment: Option<Money>`):
    ///    already-discounted PV adjustment at `as_of`. Positive = paid by
    ///    Buyer.
    /// 4. **Clean-price accrued add-back**: when [`CreditDefaultSwap::uses_clean_price`]
    ///    is `true`, add (Buyer view) or subtract (Seller view) the
    ///    Bloomberg CDSW-style accrued premium so the reported NPV matches the
    ///    "Principal" line. Cash settlement is `Principal + Accrued`.
    ///
    /// Both upfront forms can be set simultaneously without double-counting;
    /// each is applied exactly once.
    pub(crate) fn npv_full(
        &self,
        cds: &CreditDefaultSwap,
        disc: &DiscountCurve,
        surv: &HazardCurve,
        as_of: Date,
    ) -> Result<f64> {
        let protection_pv = self.pv_protection_leg_raw(cds, disc, surv, as_of)?;
        let premium_pv = self.pv_premium_leg_raw(cds, disc, surv, as_of)?;

        // Dated upfront PV: positive = paid by Buyer. Past upfronts (dt < as_of)
        // are dropped — they are not part of the forward-looking NPV.
        let upfront_pv = match cds.upfront {
            Some((dt, amount)) if dt >= as_of => amount.amount() * df_asof_to(disc, as_of, dt)?,
            _ => 0.0,
        };

        // PV-adjustment upfront: already-discounted; positive = paid by Buyer.
        let upfront_adjustment = cds
            .pricing_overrides
            .market_quotes
            .upfront_payment
            .map(|m| m.amount())
            .unwrap_or(0.0);

        let mut npv_amount = match cds.side {
            PayReceive::Pay => protection_pv - premium_pv - upfront_pv - upfront_adjustment,
            PayReceive::Receive => premium_pv - protection_pv + upfront_pv + upfront_adjustment,
        };

        if cds.uses_clean_price() {
            let accrual_fraction =
                self.coupon_accrued_fraction(cds, as_of, AccrualDayCountPolicy::CdswInclusive)?;
            let spread = cds.premium.spread_bp.to_f64().ok_or_else(|| {
                Error::Validation("premium spread_bp cannot be represented as f64".into())
            })? / BASIS_POINTS_PER_UNIT;
            let accrued = cds.notional.amount() * spread * accrual_fraction;
            npv_amount = match cds.side {
                PayReceive::Pay => npv_amount + accrued,
                PayReceive::Receive => npv_amount - accrued,
            };
        }

        Ok(npv_amount)
    }
}
