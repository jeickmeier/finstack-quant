//! Copula selection, projected premium and protection schedules, discounting and
//! settlement for `CdsTranchePricer`.

use super::prepared::{PeriodTiming, PreparedProjection};
use super::pricer::{
    BESPOKE_SETTLEMENT_DAYS, CdsTranchePricer, EffectiveStructure, INDEX_SETTLEMENT_DAYS,
    PROBABILITY_CLIP, ProjectedDiscountedRow,
};
use crate::cds_tranche::CdsTranche;
use finstack_quant_cashflows::builder::{CashFlowMeta, CashFlowSchedule};
use finstack_quant_core::dates::{Date, DateExt, HolidayCalendar, calendar_by_id_strict};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::math::units::BASIS_POINTS_PER_UNIT;
use finstack_quant_core::money::Money;
use finstack_quant_core::{Error, Result};
use finstack_quant_models::correlation::{Copula, CopulaSpec, GaussianCopula};
use std::sync::Arc;

impl CdsTranchePricer {
    /// Return the copula instance constructed for this pricer.
    pub(super) fn copula(&self) -> &dyn Copula {
        self.copula.as_ref()
    }

    pub(super) fn default_threshold_for_copula(&self, default_prob: f64) -> f64 {
        let eps = PROBABILITY_CLIP;
        let p = default_prob.max(eps).min(1.0 - eps);
        self.copula_spec.default_threshold(p).unwrap_or(f64::NAN)
    }

    pub(super) fn conditional_default_prob_copula(
        &self,
        copula: &dyn Copula,
        default_threshold: f64,
        factor_realization: &[f64],
        correlation: f64,
    ) -> f64 {
        copula
            .conditional_default_prob(default_threshold, factor_realization, correlation)
            .clamp(0.0, 1.0)
    }
    /// Create a new one-factor Gaussian copula pricer.
    pub fn new() -> Self {
        Self {
            copula_spec: CopulaSpec::Gaussian,
            copula: Arc::new(GaussianCopula::new()),
        }
    }

    /// Create a pricer with a validated copula specification.
    ///
    /// # Arguments
    ///
    /// * `copula_spec` - Copula model used for every valuation performed by
    ///   the pricer: Gaussian, Student-t (degrees of freedom above 2; typical
    ///   4-10 for CDX), random factor loading (loading volatility in
    ///   `[0, 0.5]`) or the global-plus-sector multi-factor copula.
    ///
    /// # Returns
    ///
    /// A pricer with an immutable validated copula.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] when the Student-t
    /// degrees of freedom or the loading volatility are outside their
    /// documented ranges.
    pub fn with_copula(copula_spec: CopulaSpec) -> Result<Self> {
        let copula = copula_spec
            .build()
            .map_err(|error| Error::Validation(error.to_string()))?;
        Ok(Self {
            copula_spec,
            copula: Arc::from(copula),
        })
    }

    /// Price a CDS tranche using the Gaussian Copula model.
    ///
    /// # Arguments
    /// * `tranche` - The CDS tranche to price
    /// * `market` - Market data context containing curves and credit index data
    /// * `as_of` - Valuation date
    ///
    /// # Returns
    /// The present value of the tranche
    ///
    /// # Settlement Convention
    ///
    /// Uses ISDA standard settlement:
    /// - Index CDS tranches (CDX, iTraxx): T+1 business days (Big Bang 2009)
    /// - Bespoke tranches: T+3 business days
    #[must_use = "pricing result should be used"]
    pub fn price_tranche(
        &self,
        tranche: &CdsTranche,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        tranche.validate()?;
        self.price_validated_tranche(tranche, market, as_of)
    }

    /// [`Self::price_tranche`] for a tranche the caller already validated.
    pub(super) fn price_validated_tranche(
        &self,
        tranche: &CdsTranche,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        self.prepare_validated_tranche(tranche, market, as_of)?
            .price_at_snapshot()
    }

    /// Build the projected premium/default schedule for the tranche.
    pub fn build_projected_schedule(
        &self,
        tranche: &CdsTranche,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<CashFlowSchedule> {
        tranche.validate()?;
        if as_of >= tranche.maturity {
            return Ok(finstack_quant_cashflows::schedule_from_classified_flows(
                Vec::new(),
                tranche.day_count,
                finstack_quant_cashflows::ScheduleBuildOpts {
                    notional_hint: Some(tranche.notional),
                    meta: CashFlowMeta {
                        projected_fixings: Vec::new(),
                        representation:
                            finstack_quant_cashflows::builder::CashflowRepresentation::Projected,
                        calendar_ids: tranche
                            .calendar_id
                            .iter()
                            .map(ToString::to_string)
                            .collect(),
                        commitment: None,
                        issue_date: tranche.contractual_start_date(as_of),
                        maturity: Some(tranche.maturity),
                    },
                },
            ));
        }
        let inputs = self.prepare_projection_inputs(tranche, market, as_of)?;
        let valuation_date = inputs.valuation_date;
        let flows = self
            .project_prepared_rows(
                &inputs,
                inputs.index.base_correlation_curve(),
                tranche.coupon_bp / BASIS_POINTS_PER_UNIT,
                false,
            )?
            .into_iter()
            .map(|row| row.cashflow)
            .collect();

        Ok(finstack_quant_cashflows::schedule_from_classified_flows(
            flows,
            tranche.day_count,
            finstack_quant_cashflows::ScheduleBuildOpts {
                notional_hint: Some(tranche.notional),
                meta: CashFlowMeta {
                    projected_fixings: Vec::new(),
                    representation:
                        finstack_quant_cashflows::builder::CashflowRepresentation::Projected,
                    calendar_ids: tranche
                        .calendar_id
                        .iter()
                        .map(ToString::to_string)
                        .collect(),
                    commitment: None,
                    issue_date: tranche.contractual_start_date(valuation_date),
                    maturity: Some(tranche.maturity),
                },
            },
        ))
    }

    #[cfg(test)]
    pub(super) fn project_discountable_rows(
        &self,
        tranche: &CdsTranche,
        market_ctx: &MarketContext,
        as_of: Date,
    ) -> Result<Vec<ProjectedDiscountedRow>> {
        self.project_coupon_rows(
            tranche,
            market_ctx,
            as_of,
            tranche.coupon_bp / BASIS_POINTS_PER_UNIT,
            false,
        )
    }

    pub(super) fn project_coupon_rows(
        &self,
        tranche: &CdsTranche,
        market_ctx: &MarketContext,
        as_of: Date,
        coupon: f64,
        include_tiny_premiums: bool,
    ) -> Result<Vec<ProjectedDiscountedRow>> {
        let inputs = self.prepare_projection_inputs(tranche, market_ctx, as_of)?;
        self.project_prepared_rows(
            &inputs,
            inputs.index.base_correlation_curve(),
            coupon,
            include_tiny_premiums,
        )
    }

    /// Discount projected rows on the DISCOUNT curve's own time axis,
    /// relative to `as_of` (`DF(as_of → t) = DF(0 → t) / DF(0 → as_of)`),
    /// mirroring the single-name CDS `df_asof_to` convention. This is exact
    /// regardless of the discount curve's base date or any day-count
    /// mismatch with the hazard curve.
    pub(super) fn discount_projected_rows(
        &self,
        rows: &[ProjectedDiscountedRow],
        discount_curve: &dyn Discounting,
        as_of: Date,
    ) -> Result<f64> {
        let mut pv = 0.0;
        for row in rows {
            let df = discount_curve.df_between_dates(as_of, row.discount_date(as_of))?;
            pv += row.cashflow.get_amount().amount() * df;
        }
        Ok(pv)
    }

    pub(super) fn prepare_projection_inputs(
        &self,
        tranche: &CdsTranche,
        market_ctx: &MarketContext,
        as_of: Date,
    ) -> Result<PreparedProjection> {
        let index_data_arc = market_ctx
            .get_credit_index(&tranche.credit_index_id)
            .map_err(|_| {
                finstack_quant_core::Error::Input(finstack_quant_core::InputError::NotFound {
                    id: format!(
                        "Credit index '{}' required for tranche '{}' pricing",
                        tranche.credit_index_id, tranche.id
                    ),
                })
            })?;
        if as_of < tranche.maturity && index_data_arc.index_credit_curve().base_date() != as_of {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CDS tranche '{}' requires credit curve '{}' to be based on valuation date {}; got {}. Rebase the curve or provide a conditionally rebased credit index.",
                tranche.id,
                tranche.credit_index_id,
                as_of,
                index_data_arc.index_credit_curve().base_date()
            )));
        }
        if as_of < tranche.maturity
            && let Some(issuer_curves) = index_data_arc.issuer_credit_curves()
        {
            for (issuer_id, curve) in issuer_curves {
                if curve.base_date() != as_of {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "CDS tranche '{}' requires issuer credit curve '{}' to be based on valuation date {}; got {}. Rebase the curve or provide a conditionally rebased credit index.",
                        tranche.id,
                        issuer_id,
                        as_of,
                        curve.base_date()
                    )));
                }
            }
        }
        let valuation_date = self.calculate_settlement_date(tranche, market_ctx, as_of)?;
        let periods: Vec<_> = self
            .generate_coupon_periods(tranche, valuation_date)?
            .into_iter()
            .filter(|period| period.payment_date >= as_of)
            .collect();
        let future_dates: Vec<_> = periods
            .iter()
            .map(|p| p.accrual_end)
            .filter(|date| *date > as_of)
            .collect();
        let losses = self.prepare_el_wd_curve(tranche, index_data_arc.as_ref(), &future_dates)?;
        let known_count = periods.iter().filter(|p| p.accrual_end <= as_of).count();
        let mut timing = Vec::with_capacity(periods.len());
        for period in &periods {
            let period_start = period.accrual_start;
            let protection_start = period_start.max(as_of).min(period.accrual_end);
            let prior_time = if protection_start <= as_of {
                0.0
            } else {
                self.years_from_base(index_data_arc.as_ref(), protection_start)?
            };
            let payment_time = if period.accrual_end <= as_of {
                0.0
            } else {
                self.years_from_base(index_data_arc.as_ref(), period.accrual_end)?
            };
            let remaining_accrual = tranche.day_count.year_fraction(
                protection_start,
                period.accrual_end,
                finstack_quant_core::dates::DayCountContext::default(),
            )?;

            // Survival-weighted mean within-period default fraction.
            //
            // A name that defaults at τ ∈ [t_{i-1}, t_i] pays accrued premium
            // only over [t_{i-1}, τ] and triggers the loss at τ. Both the
            // accrual-on-default adjustment (Item 5) and the mid-period
            // protection timing (Item 6) need E[(τ − t_{i-1}) / period | τ in
            // period]. Default timing is NOT uniform — it follows the index
            // hazard, so the survival-weighted mean is < 0.5 for a positive
            // hazard. `within_period_default_fraction` integrates the
            // piecewise-constant-hazard default density over the period,
            // which is the discrete-model analogue of the single-name CDS
            // pricer's analytic accrual-on-default integral. It degrades
            // gracefully to 0.5 (the uniform limit) when λΔ → 0.
            let default_fraction = self.within_period_default_fraction(
                index_data_arc.as_ref(),
                prior_time,
                payment_time,
            );

            timing.push(PeriodTiming {
                protection_start,
                remaining_accrual,
                default_fraction,
            });
        }
        Ok(PreparedProjection {
            tranche: tranche.clone(),
            index: index_data_arc,
            as_of,
            valuation_date,
            periods,
            timing,
            losses,
            known_count,
        })
    }

    /// Calculate the settlement date based on ISDA conventions.
    ///
    /// - If start_date is set, uses as_of directly (explicit settlement)
    /// - For index tranches (CDX, iTraxx): T+1 business days
    /// - For bespoke tranches: T+3 business days
    ///
    /// Uses business day calendars when available via the tranche's `calendar_id`.
    /// Falls back to weekend-only logic when no calendar is specified.
    pub(super) fn calculate_settlement_date(
        &self,
        tranche: &CdsTranche,
        _market_ctx: &MarketContext,
        as_of: Date,
    ) -> Result<Date> {
        // If effective date is explicitly set, use as_of directly
        if tranche.start_date.is_some() {
            return Ok(as_of);
        }

        // Determine settlement lag based on index type
        let is_standard_index = tranche.index_name.starts_with("CDX")
            || tranche.index_name.starts_with("iTraxx")
            || tranche.index_name.starts_with("ITRAXX");
        let settlement_lag = if is_standard_index {
            INDEX_SETTLEMENT_DAYS
        } else {
            BESPOKE_SETTLEMENT_DAYS
        };

        // Use calendar if available, otherwise fall back to weekday-only adjustment
        let calendar: Option<&dyn HolidayCalendar> = tranche
            .calendar_id
            .as_deref()
            .map(calendar_by_id_strict)
            .transpose()?;

        if let Some(cal) = calendar {
            as_of.add_business_days(settlement_lag, cal)
        } else {
            as_of.add_weekdays(settlement_lag)
        }
    }

    /// Survival-weighted mean within-period default fraction.
    ///
    /// Returns `E[(τ − t₀) / (t₁ − t₀) | τ ∈ [t₀, t₁]]` where the default time
    /// `τ` is distributed according to the index credit curve. This is the
    /// fraction of the coupon period that a name defaulting inside the period
    /// is expected to have survived — used both to size the accrual-on-default
    /// premium reduction and to time the within-period loss for discounting.
    ///
    /// # Method
    ///
    /// Over `[t₀, t₁]` the index survival is approximated by a piecewise-
    /// constant hazard `λ = −ln(S(t₁)/S(t₀)) / (t₁ − t₀)`, exact for the
    /// hazard curves used here (piecewise-constant or log-linear survival).
    /// The default-time density is `f(t) ∝ −S'(t) = λ·S(t₀)·e^{−λ(t−t₀)}`, so
    ///
    /// ```text
    /// E[(τ−t₀)/Δ | τ∈period] = ( 1/λ − Δ·e^{−λΔ}/(1−e^{−λΔ}) ) / Δ
    /// ```
    ///
    /// which is the standard analytic accrual-on-default factor (consistent
    /// with the single-name CDS pricer's `accrual_on_default_isda_standard_model_cond`).
    /// It is bounded in `(0, 0.5]` for `λ ≥ 0` and tends to `0.5` — the
    /// uniform-default limit — as `λΔ → 0`.
    ///
    /// Degenerate inputs (non-positive period, non-finite or non-positive
    /// survival, `λ ≤ 0`) fall back to the `0.5` midpoint so the caller never
    /// sees a NaN or an out-of-range fraction.
    pub(super) fn within_period_default_fraction(
        &self,
        index_data: &finstack_quant_core::market_data::term_structures::CreditIndexData,
        t_start: f64,
        t_end: f64,
    ) -> f64 {
        const UNIFORM_MIDPOINT: f64 = 0.5;
        // `x` is a finite, strictly-positive number (NaN/∞/≤0 are rejected).
        // Used in place of negated `>` comparisons so the NaN handling is
        // explicit (and clippy-clean).
        let is_finite_positive = |x: f64| x.is_finite() && x > 0.0;

        let dt = t_end - t_start;
        if !is_finite_positive(dt) {
            return UNIFORM_MIDPOINT;
        }
        let curve = index_data.index_credit_curve();
        let s0 = curve.sp(t_start.max(0.0));
        let s1 = curve.sp(t_end);
        // Need a genuine, ordered survival drop to define a hazard:
        // `s1 > s0` or NaN inputs all fall back to the uniform midpoint.
        let ordered_drop = is_finite_positive(s0) && is_finite_positive(s1) && s1 < s0;
        if !ordered_drop {
            return UNIFORM_MIDPOINT;
        }
        // Piecewise-constant hazard over the period.
        let lambda = -(s1 / s0).ln() / dt;
        if !is_finite_positive(lambda) {
            return UNIFORM_MIDPOINT;
        }
        let lambda_dt = lambda * dt;
        // Small-λΔ guard: the closed form is 0/0 as λΔ→0; the Taylor limit
        // is exactly 0.5, and for tiny λΔ the direct formula loses precision.
        if lambda_dt < 1e-8 {
            return UNIFORM_MIDPOINT;
        }
        let exp_neg = (-lambda_dt).exp();
        // E[τ−t₀] = 1/λ − Δ·e^{−λΔ}/(1−e^{−λΔ}); fraction = that / Δ.
        let mean_offset = (1.0 / lambda) - dt * exp_neg / (1.0 - exp_neg);
        let fraction = mean_offset / dt;
        // Analytically `fraction ∈ (0, 0.5)`; clamp to absorb fp residue.
        fraction.clamp(0.0, UNIFORM_MIDPOINT)
    }

    /// Calculate effective attachment/detachment points given realized
    /// defaults (losses AND recoveries).
    ///
    /// `realized_loss` is the realized pool LOSS fraction `L`
    /// (original-pool units). With recovery `R` the realized DEFAULTED
    /// notional fraction is `X = L / (1 − R)`:
    ///
    /// - The loss `L` erodes the structure from the BOTTOM (attachment up).
    /// - The recovered notional `G = X·R` amortizes the structure from the
    ///   TOP (detachment down) — senior-side recovery writedown.
    /// - The surviving pool is `1 − X` (defaulted names leave the pool
    ///   entirely), NOT `1 − L` — using the loss conflates loss with
    ///   defaulted notional and is exact only at `R = 0`.
    ///
    /// Strikes are re-normalized to the surviving pool.
    ///
    /// # Invariants
    ///
    /// - Realized loss is in [0, 1]
    /// - Attachment <= Detachment (after percentage conversion)
    /// - Results are always in [0, 1]
    pub(super) fn calculate_effective_structure(
        &self,
        tranche: &CdsTranche,
        recovery_rate: f64,
    ) -> EffectiveStructure {
        let l = tranche.realized_loss;
        let attach = tranche.attach_pct / 100.0;
        let detach = tranche.detach_pct / 100.0;

        // Debug assertions for invariants
        debug_assert!((0.0..=1.0).contains(&l), "realized_loss must be in [0, 1]");
        debug_assert!(attach <= detach, "attachment must not exceed detachment");
        debug_assert!(
            (0.0..=1.0).contains(&attach),
            "attachment must be in [0, 1]"
        );
        debug_assert!(
            (0.0..=1.0).contains(&detach),
            "detachment must be in [0, 1]"
        );

        let (defaulted, recovered) = self.realized_default_state(tranche, recovery_rate);

        if defaulted >= 1.0 - 1e-9 {
            return EffectiveStructure {
                eff_attach: 0.0,
                eff_detach: 0.0,
                pool_factor: 0.0,
            };
        }

        let pool_factor = 1.0 - defaulted;

        // Bottom erosion by realized loss, top erosion by realized recovery.
        let eroded_top = 1.0 - recovered;
        let eff_attach = (attach.min(eroded_top) - l).max(0.0) / pool_factor;
        let eff_detach = (detach.min(eroded_top) - l).max(0.0) / pool_factor;

        let result = EffectiveStructure {
            eff_attach: eff_attach.clamp(0.0, 1.0),
            eff_detach: eff_detach.clamp(0.0, 1.0),
            pool_factor,
        };

        // Post-condition assertions
        debug_assert!(
            result.eff_attach <= result.eff_detach,
            "effective attach {} > effective detach {}",
            result.eff_attach,
            result.eff_detach
        );

        result
    }
}
