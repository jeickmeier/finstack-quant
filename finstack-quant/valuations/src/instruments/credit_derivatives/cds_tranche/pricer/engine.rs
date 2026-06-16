use super::config::{
    CDSTranchePricer, CDSTranchePricerConfig, DiscountAt, EffectiveStructure,
    ProjectedDiscountedRow, ProjectionInputs, PROBABILITY_CLIP,
};
use crate::cashflow::builder::{CashFlowMeta, CashFlowSchedule};
use crate::cashflow::primitives::{CFKind, CashFlow};
use crate::constants::BASIS_POINTS_PER_UNIT;
use crate::correlation::copula::{
    Copula, CopulaSpec, GaussianCopula, MultiFactorCopula, RandomFactorLoadingCopula,
    StudentTCopula,
};
use crate::instruments::credit_derivatives::cds_tranche::{CDSTranche, TrancheSide};
use finstack_quant_core::dates::{CalendarRegistry, Date, DateExt, HolidayCalendar};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::math::{
    standard_normal_inv_cdf, student_t_inv_cdf, GaussHermiteQuadrature,
};
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

impl CDSTranchePricer {
    #[inline]
    pub(super) fn select_quadrature(&self) -> Result<&GaussHermiteQuadrature> {
        Ok(self.quadrature_cache.get_or_init(|| {
            crate::correlation::copula::select_quadrature(self.params.quadrature_order)
        }))
    }

    /// Return the cached copula instance, building it on first call.
    ///
    /// The copula is determined entirely by `self.params.copula_spec` at
    /// pricer-construction time, so a single instance can be reused across
    /// every EL/integrand evaluation for the lifetime of this pricer.
    pub(super) fn copula(&self) -> &dyn Copula {
        self.copula_cache
            .get_or_init(|| match &self.params.copula_spec {
                CopulaSpec::Gaussian => Box::new(GaussianCopula::with_quadrature_order(
                    self.params.quadrature_order,
                )),
                CopulaSpec::StudentT { degrees_of_freedom } => {
                    Box::new(StudentTCopula::with_quadrature_order(
                        *degrees_of_freedom,
                        self.params.quadrature_order,
                    ))
                }
                CopulaSpec::RandomFactorLoading { loading_volatility } => {
                    Box::new(RandomFactorLoadingCopula::with_quadrature_order(
                        *loading_volatility,
                        self.params.quadrature_order,
                    ))
                }
                CopulaSpec::MultiFactor { num_factors } => {
                    // Honor the configured quadrature order like every other
                    // copula variant. Note the multi-factor cost is
                    // `order^{num_factors}` — tune `quadrature_order`
                    // accordingly when selecting this copula.
                    Box::new(MultiFactorCopula::with_quadrature_order(
                        *num_factors,
                        self.params.quadrature_order,
                    ))
                }
            })
            .as_ref()
    }

    pub(super) fn default_threshold_for_copula(&self, default_prob: f64) -> f64 {
        let eps = PROBABILITY_CLIP;
        let p = default_prob.max(eps).min(1.0 - eps);
        match &self.params.copula_spec {
            CopulaSpec::StudentT { degrees_of_freedom } => {
                student_t_inv_cdf(p, *degrees_of_freedom)
            }
            _ => standard_normal_inv_cdf(p),
        }
    }

    /// Factor supplied to the stochastic recovery model.
    ///
    /// Gaussian-family recovery models are calibrated to the same systematic
    /// market factor that drives conditional default probabilities. For the
    /// Student-t copula, `factors[0]` is the normal numerator and `factors[1]`
    /// is the chi-square scale mixture, so the actual market factor is
    /// `Z / sqrt(W)`.
    pub(super) fn recovery_driver_for_factors(&self, factors: &[f64]) -> f64 {
        match self.params.copula_spec {
            CopulaSpec::StudentT { .. } if factors.len() >= 2 => {
                let z = factors[0];
                let w = factors[1];
                if z.is_finite() && w.is_finite() && w > 0.0 {
                    z / w.sqrt()
                } else {
                    z
                }
            }
            _ => factors.first().copied().unwrap_or(0.0),
        }
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
    /// Create a new Gaussian Copula model with default parameters.
    pub fn new() -> Self {
        Self {
            params: CDSTranchePricerConfig::default(),
            copula_cache: std::sync::OnceLock::new(),
            quadrature_cache: std::sync::OnceLock::new(),
        }
    }

    /// Create a new model with custom parameters.
    pub fn with_params(params: CDSTranchePricerConfig) -> Self {
        Self {
            params,
            copula_cache: std::sync::OnceLock::new(),
            quadrature_cache: std::sync::OnceLock::new(),
        }
    }

    /// Price a CDS tranche using the Gaussian Copula model.
    ///
    /// Falls back to zero PV when credit index data is not available as default behavior.
    ///
    /// # Arguments
    /// * `tranche` - The CDS tranche to price
    /// * `market_ctx` - Market data context containing curves and credit index data
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
        tranche: &CDSTranche,
        market_ctx: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        // Check if tranche is already wiped out
        if tranche.accumulated_loss >= tranche.detach_pct / 100.0 {
            return Ok(Money::new(0.0, tranche.notional.currency()));
        }

        let discount_curve = market_ctx.get_discount(tranche.discount_curve_id.as_ref())?;
        let rows = self.project_discountable_rows(tranche, market_ctx, as_of)?;

        if rows.is_empty() {
            return Ok(Money::new(0.0, tranche.notional.currency()));
        }

        let net_pv = self.discount_projected_rows(&rows, discount_curve.as_ref(), as_of)?;

        Ok(Money::new(net_pv, tranche.notional.currency()))
    }

    /// Build the projected premium/default schedule for the tranche.
    pub fn build_projected_schedule(
        &self,
        tranche: &CDSTranche,
        market_ctx: &MarketContext,
        as_of: Date,
    ) -> Result<CashFlowSchedule> {
        if as_of >= tranche.maturity {
            return Ok(crate::cashflow::traits::schedule_from_classified_flows(
                Vec::new(),
                tranche.day_count,
                crate::cashflow::traits::ScheduleBuildOpts {
                    notional_hint: Some(tranche.notional),
                    meta: Some(CashFlowMeta {
                        representation: crate::cashflow::builder::CashflowRepresentation::Projected,
                        calendar_ids: tranche.calendar_id.clone().into_iter().collect(),
                        facility_limit: None,
                        issue_date: tranche.contractual_effective_date(as_of),
                    }),
                    ..Default::default()
                },
            ));
        }
        let (_, valuation_date, _, _) =
            self.prepare_projection_inputs(tranche, market_ctx, as_of)?;
        let flows = self
            .project_discountable_rows(tranche, market_ctx, as_of)?
            .into_iter()
            .map(|row| row.cashflow)
            .collect();

        Ok(crate::cashflow::traits::schedule_from_classified_flows(
            flows,
            tranche.day_count,
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: Some(tranche.notional),
                meta: Some(CashFlowMeta {
                    representation: crate::cashflow::builder::CashflowRepresentation::Projected,
                    calendar_ids: tranche.calendar_id.clone().into_iter().collect(),
                    facility_limit: None,
                    issue_date: tranche.contractual_effective_date(valuation_date),
                }),
                ..Default::default()
            },
        ))
    }

    pub(super) fn project_discountable_rows(
        &self,
        tranche: &CDSTranche,
        market_ctx: &MarketContext,
        as_of: Date,
    ) -> Result<Vec<ProjectedDiscountedRow>> {
        if as_of >= tranche.maturity {
            return Ok(Vec::new());
        }
        let (_index_data_arc, valuation_date, payment_dates, el_curve) =
            self.prepare_projection_inputs(tranche, market_ctx, as_of)?;
        if valuation_date >= tranche.maturity {
            return Ok(Vec::new());
        }

        let coupon = tranche.running_coupon_bp / BASIS_POINTS_PER_UNIT;
        let tranche_notional = tranche.notional.amount();
        let premium_sign = match tranche.side {
            TrancheSide::BuyProtection => -1.0,
            TrancheSide::SellProtection => 1.0,
        };
        let protection_sign = -premium_sign;

        let mut rows =
            Vec::with_capacity(payment_dates.len() * 2 + usize::from(tranche.upfront.is_some()));
        let mut prev_el_fraction = self.calculate_prior_tranche_loss(tranche);
        let mut prev_wd_fraction =
            self.calculate_prior_tranche_writedown(tranche, _index_data_arc.recovery_rate);

        // Hazard-axis payment times, used ONLY for survival quantities
        // (within-period default timing). Discounting uses the discount
        // curve's own axis via `DiscountAt` (see `discount_projected_rows`).
        let payment_times: Vec<f64> = payment_dates
            .iter()
            .map(|&d| self.years_from_base(_index_data_arc.as_ref(), d))
            .collect::<Result<Vec<_>>>()?;

        for (i, &payment_date) in payment_dates.iter().enumerate() {
            let el_fraction = el_curve[i].el_fraction;
            let wd_fraction = el_curve[i].wd_fraction;
            let delta_el_fraction = (el_fraction - prev_el_fraction).max(0.0);
            let delta_wd_fraction = (wd_fraction - prev_wd_fraction).max(0.0);
            // Premium accrues on the notional surviving BOTH bottom-up loss
            // erosion and top-down recovery writedown (senior amortization).
            let outstanding_notional =
                tranche_notional * (1.0 - prev_el_fraction - prev_wd_fraction).max(0.0);
            let period_start = if i == 0 {
                tranche
                    .contractual_effective_date(valuation_date)
                    .unwrap_or(valuation_date)
            } else {
                payment_dates[i - 1]
            };
            let accrual_period = tranche.day_count.year_fraction(
                period_start,
                payment_date,
                finstack_quant_core::dates::DayCountContext::default(),
            )?;
            let payment_time = payment_times[i];
            let prior_time = if i == 0 {
                if period_start <= _index_data_arc.index_credit_curve.base_date() {
                    0.0
                } else {
                    self.years_from_base(_index_data_arc.as_ref(), period_start)?
                }
            } else {
                payment_times[i - 1]
            };

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
                _index_data_arc.as_ref(),
                prior_time,
                payment_time,
            );

            // Premium reduction on the slice that defaults within the period.
            //
            // A name defaulting at the survival-weighted fraction `f` of the
            // period pays accrued premium over `f·Δ` only, so the premium
            // notional lost on the defaulted slice is `(1−f)·Δerosion`. With
            // accrual-on-default disabled, a defaulted name pays nothing for
            // the period — the full erosion drops out of the premium
            // notional. Both the loss increment (bottom-up) and the recovery
            // writedown increment (top-down) occur at default time, so they
            // receive the same treatment.
            let delta_erosion = delta_el_fraction + delta_wd_fraction;
            let aod_adjustment = if self.params.accrual_on_default_enabled {
                (1.0 - default_fraction) * tranche_notional * delta_erosion
            } else {
                tranche_notional * delta_erosion
            };
            let premium_amount =
                coupon * accrual_period * (outstanding_notional - aod_adjustment).max(0.0);

            if premium_amount.abs() > f64::EPSILON {
                rows.push(ProjectedDiscountedRow {
                    cashflow: CashFlow {
                        date: payment_date,
                        reset_date: None,
                        amount: Money::new(
                            premium_amount * premium_sign,
                            tranche.notional.currency(),
                        ),
                        kind: CFKind::Fixed,
                        accrual_factor: accrual_period,
                        rate: Some(coupon),
                    },
                    discount_at: DiscountAt::PaymentDate,
                });
            }

            let default_amount = tranche_notional * delta_el_fraction;
            if default_amount.abs() > f64::EPSILON {
                rows.push(ProjectedDiscountedRow {
                    cashflow: CashFlow {
                        date: payment_date,
                        reset_date: None,
                        amount: Money::new(
                            default_amount * protection_sign,
                            tranche.notional.currency(),
                        ),
                        kind: CFKind::DefaultedNotional,
                        accrual_factor: 0.0,
                        rate: None,
                    },
                    // Discount the loss increment at the time the underlying
                    // defaults actually occur, not the period end. With
                    // `mid_period_protection`, the within-period loss is
                    // discounted at the SURVIVAL-WEIGHTED mean default time
                    // (Item 6) — `fraction` of the way through the period —
                    // rather than the flat period midpoint, which
                    // over-discounted the increment by assuming all loss
                    // lands at the midpoint. The default fraction is < 0.5
                    // for a positive hazard, so the loss is correctly
                    // discounted slightly earlier than mid-period. The
                    // fraction is measured on the hazard axis (a survival
                    // quantity); the DF lookup itself happens on the
                    // discount curve's axis in `discount_projected_rows`.
                    discount_at: if self.params.mid_period_protection {
                        DiscountAt::WithinPeriod {
                            start: period_start,
                            fraction: default_fraction,
                        }
                    } else {
                        DiscountAt::PaymentDate
                    },
                });
            }

            prev_el_fraction = el_fraction;
            prev_wd_fraction = wd_fraction;
        }

        if let Some((date, amount)) = tranche.upfront.filter(|(date, _)| *date >= as_of) {
            rows.push(ProjectedDiscountedRow {
                cashflow: CashFlow {
                    date,
                    reset_date: None,
                    amount: Money::new(amount.amount() * premium_sign, amount.currency()),
                    kind: CFKind::Fee,
                    accrual_factor: 0.0,
                    rate: None,
                },
                discount_at: DiscountAt::PaymentDate,
            });
        }

        Ok(rows)
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
            let df = match row.discount_at {
                DiscountAt::PaymentDate => {
                    discount_curve.df_between_dates(as_of, row.cashflow.date)?
                }
                DiscountAt::WithinPeriod { start, fraction } => {
                    // Interpolate the survival-weighted default date inside
                    // the period in calendar days, then take the relative DF
                    // from as_of on the discount curve's axis. The date is
                    // clamped to [as_of, payment_date]: a stub period can
                    // start before as_of, but the EL increment only covers
                    // defaults occurring on or after the valuation date.
                    let end = row.cashflow.date;
                    let period_days = (end - start).whole_days().max(0);
                    let offset_days = (fraction * period_days as f64).round() as i64;
                    let mid = start
                        .checked_add(time::Duration::days(offset_days))
                        .unwrap_or(end)
                        .clamp(as_of, end);
                    discount_curve.df_between_dates(as_of, mid)?
                }
            };
            pv += row.cashflow.amount.amount() * df;
        }
        Ok(pv)
    }

    pub(super) fn prepare_projection_inputs(
        &self,
        tranche: &CDSTranche,
        market_ctx: &MarketContext,
        as_of: Date,
    ) -> Result<ProjectionInputs> {
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
        let valuation_date = self.calculate_settlement_date(tranche, market_ctx, as_of)?;
        let payment_dates = self.generate_payment_schedule(tranche, valuation_date)?;
        let el_curve = if payment_dates.is_empty() || valuation_date >= tranche.maturity {
            Vec::new()
        } else {
            self.build_el_wd_curve(tranche, index_data_arc.as_ref(), &payment_dates)?
        };
        Ok((index_data_arc, valuation_date, payment_dates, el_curve))
    }

    /// Calculate the settlement date based on ISDA conventions.
    ///
    /// - If effective_date is set, uses as_of directly (explicit settlement)
    /// - For index tranches (CDX, iTraxx): T+1 business days
    /// - For bespoke tranches: T+3 business days
    ///
    /// Uses business day calendars when available via the tranche's `calendar_id`.
    /// Falls back to weekend-only logic when no calendar is specified.
    pub(super) fn calculate_settlement_date(
        &self,
        tranche: &CDSTranche,
        _market_ctx: &MarketContext,
        as_of: Date,
    ) -> Result<Date> {
        // If effective date is explicitly set, use as_of directly
        if tranche.effective_date.is_some() {
            return Ok(as_of);
        }

        // Determine settlement lag based on index type
        let is_standard_index = tranche.index_name.starts_with("CDX")
            || tranche.index_name.starts_with("iTraxx")
            || tranche.index_name.starts_with("ITRAXX");
        let settlement_lag = if is_standard_index {
            self.params.index_settlement_lag
        } else {
            self.params.bespoke_settlement_lag
        };

        // Use calendar if available, otherwise fall back to weekday-only adjustment
        let calendar: Option<&dyn HolidayCalendar> = tranche
            .calendar_id
            .as_deref()
            .and_then(|id| CalendarRegistry::global().resolve_str(id));

        if let Some(cal) = calendar {
            as_of.add_business_days(settlement_lag, cal)
        } else {
            Ok(as_of.add_weekdays(settlement_lag))
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
        let curve = &index_data.index_credit_curve;
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
    /// `accumulated_loss` is the realized pool LOSS fraction `L`
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
    /// - Accumulated loss is in [0, 1]
    /// - Attachment <= Detachment (after percentage conversion)
    /// - Results are always in [0, 1]
    pub(super) fn calculate_effective_structure(
        &self,
        tranche: &CDSTranche,
        recovery_rate: f64,
    ) -> EffectiveStructure {
        let l = tranche.accumulated_loss;
        let attach = tranche.attach_pct / 100.0;
        let detach = tranche.detach_pct / 100.0;

        // Debug assertions for invariants
        debug_assert!(
            (0.0..=1.0).contains(&l),
            "accumulated_loss {} must be in [0, 1]",
            l
        );
        debug_assert!(
            attach <= detach,
            "attach {} must be <= detach {}",
            attach,
            detach
        );
        debug_assert!(
            (0.0..=1.0).contains(&attach),
            "attach {} must be in [0, 1]",
            attach
        );
        debug_assert!(
            (0.0..=1.0).contains(&detach),
            "detach {} must be in [0, 1]",
            detach
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
