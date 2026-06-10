use super::config::{
    CDSTranchePricer, NUMERICAL_TOLERANCE, PAR_SPREAD_MAX_ITER, PAR_SPREAD_TOLERANCE,
};
use super::registry::JumpToDefaultResult;
use crate::cashflow::builder::build_dates;
use crate::cashflow::primitives::CFKind;
use crate::constants::BASIS_POINTS_PER_UNIT;
use crate::instruments::credit_derivatives::cds_tranche::CDSTranche;
use finstack_core::dates::{next_cds_date, Date};
use finstack_core::market_data::{context::MarketContext, term_structures::CreditIndexData};
use finstack_core::math::binomial_pmf_all;
use finstack_core::{Error, Result};

impl CDSTranchePricer {
    /// Apply smooth correlation boundary handling to avoid numerical discontinuities.
    ///
    /// Uses a smooth transition function near the boundaries to maintain numerical
    /// stability while preserving the underlying mathematical relationships.
    pub(super) fn smooth_correlation_boundary(&self, correlation: f64) -> f64 {
        let min_corr = self.params.min_correlation;
        let max_corr = self.params.max_correlation;
        let width = self.params.corr_boundary_width;

        if correlation <= min_corr + width {
            // Lower boundary: smooth transition using tanh
            let x = (correlation - min_corr) / width;
            min_corr + width * (1.0 + x.tanh()) / 2.0
        } else if correlation >= max_corr - width {
            // Upper boundary: smooth transition using tanh
            let x = (correlation - (max_corr - width)) / width;
            max_corr - width * (1.0 - x.tanh()) / 2.0
        } else {
            // Normal range: the branch conditions already guarantee
            // `min_corr + width < correlation < max_corr - width`, so no adjustment
            // (and no clamp) is needed here.
            correlation
        }
    }

    /// Conditional expected capped pool quantity `E[min(Σᵢ eᵢ·Bᵢ, cap) | Z]`
    /// for a homogeneous pool, where `eᵢ = exposure / n` per name.
    ///
    /// With `exposure = 1 − R` (loss given default) this is the conditional
    /// equity-tranche LOSS `E[min(L, K) | Z]`; with `exposure = R` it is the
    /// conditional capped RECOVERED notional `E[min(G, cap) | Z]` used for
    /// senior-side recovery amortization (see
    /// `calculate_equity_tranche_recovery`).
    ///
    /// Uses the binomial distribution to sum over all possible numbers of defaults.
    pub(super) fn conditional_equity_tranche_capped(
        &self,
        num_constituents: usize,
        cap_notional: f64,
        conditional_default_prob: f64,
        exposure: f64,
    ) -> f64 {
        let individual_notional = 1.0 / num_constituents as f64; // Normalized to 1.0 total

        // Evaluate the whole conditional binomial PMF once (O(n)) instead of
        // reconstructing a distribution per `k`.
        let pmf = binomial_pmf_all(num_constituents, conditional_default_prob);

        let mut expected = 0.0;

        // Sum over all possible numbers of defaults
        for (k, &prob_k_defaults) in pmf.iter().enumerate() {
            // Pool quantity (loss or recovered notional) given k defaults
            let pool_amount = k as f64 * individual_notional * exposure;

            // Capped at the strike (equity tranche [0, cap_notional])
            let capped = pool_amount.min(cap_notional);

            expected += prob_k_defaults * capped;
        }

        expected
    }

    /// Get default probability for the index at a given maturity.
    ///
    /// Clamps the result to `[0.0, 1.0]` to guard against floating-point rounding
    /// in the credit-curve interpolation producing a survival probability marginally
    /// outside `[0, 1]`.  Without the clamp a negative `default_prob` would reach
    /// `standard_normal_inv_cdf` (via the homogeneous Gaussian path), which panics
    /// for arguments strictly outside `[0, 1]` (see Task C2).
    pub(super) fn get_default_probability(
        &self,
        index_data: &CreditIndexData,
        maturity_years: f64,
    ) -> Result<f64> {
        let survival_prob = index_data.index_credit_curve.sp(maturity_years);
        Ok((1.0 - survival_prob).clamp(0.0, 1.0))
    }

    /// Calculate years from the credit curve base date.
    pub(super) fn years_from_base(&self, index_data: &CreditIndexData, date: Date) -> Result<f64> {
        let dc = index_data.index_credit_curve.day_count();
        dc.year_fraction(
            index_data.index_credit_curve.base_date(),
            date,
            finstack_core::dates::DayCountContext::default(),
        )
    }

    /// Create a bumped base correlation curve for sensitivity analysis.
    ///
    /// Applies a **pure parallel shift** of `bump_abs` to every knot
    /// correlation. This is a genuine symmetric perturbation: the up-bump
    /// (`+h`) and down-bump (`−h`) are exact mirror images, which is required
    /// for the central-difference Correlation01
    /// `(PV(+h) − PV(−h)) / (2h)` to be unbiased.
    ///
    /// # Why no monotonicity repair
    ///
    /// A parallel shift of a monotone curve is *itself* monotone — shifting
    /// every point by the same `h` preserves the ordering exactly. The
    /// previous implementation ran a monotonicity-repair loop *after*
    /// bumping; that loop only ever fired because of the additional
    /// `[min_correlation, max_correlation]` clamp, and when it did fire it
    /// adjusted the up- and down-bumped curves *differently* — destroying the
    /// symmetry of the central difference and biasing Correlation01 near
    /// base-correlation-curve kinks. The repair loop has therefore been
    /// removed and the curve is built with `allow_non_monotonic()` so the
    /// pure shift is never silently re-shaped.
    ///
    /// # Bounds
    ///
    /// Correlations are clamped only to the curve type's hard `[0, 1]`
    /// domain (a `BaseCorrelationCurve` cannot hold values outside it). For
    /// realistic curves (`ρ ≈ 0.2–0.9`) and realistic bumps (`h ≈ 0.01`)
    /// this clamp never fires, so symmetry is preserved. The
    /// numerical-stability band `[min_correlation, max_correlation]` is *not*
    /// applied here — it is enforced downstream by
    /// [`Self::smooth_correlation_boundary`] inside the EL evaluation;
    /// re-applying it here would double-clamp and re-introduce the
    /// asymmetry this fix removes.
    pub(super) fn bump_base_correlation(
        &self,
        original_curve: &finstack_core::market_data::term_structures::BaseCorrelationCurve,
        bump_abs: f64,
    ) -> finstack_core::Result<finstack_core::market_data::term_structures::BaseCorrelationCurve>
    {
        use finstack_core::market_data::term_structures::BaseCorrelationCurve;

        // Pure parallel shift. Clamp only to the [0, 1] correlation domain
        // the curve type requires; no min/max-correlation band, no
        // monotonicity repair — both broke the up/down bump symmetry.
        let bumped_points: Vec<(f64, f64)> = original_curve
            .detachment_points()
            .iter()
            .zip(original_curve.correlations().iter())
            .map(|(&detach, &corr)| (detach, (corr + bump_abs).clamp(0.0, 1.0)))
            .collect();

        // `allow_non_monotonic`: a parallel shift of a monotone curve stays
        // monotone, but a non-monotone *input* curve must not be silently
        // rejected by the sensitivity path — the bump is a perturbation, not
        // an arbitrage-free pricing curve.
        BaseCorrelationCurve::builder("TEMP_BUMPED_CORR")
            .knots(bumped_points)
            .allow_non_monotonic()
            .build()
    }

    /// Create a bumped credit index with shifted hazard rates for CS01 calculation.
    ///
    /// Creates a new CreditIndexData with the index hazard curve shifted by delta_lambda.
    pub(super) fn rebuild_credit_index(
        &self,
        original_index: &CreditIndexData,
        recovery_rate: f64,
        index_credit_curve: std::sync::Arc<
            finstack_core::market_data::term_structures::HazardCurve,
        >,
        base_correlation_curve: std::sync::Arc<
            finstack_core::market_data::term_structures::BaseCorrelationCurve,
        >,
    ) -> Result<CreditIndexData> {
        let mut builder = CreditIndexData::builder()
            .num_constituents(original_index.num_constituents)
            .recovery_rate(recovery_rate)
            .index_credit_curve(index_credit_curve)
            .base_correlation_curve(base_correlation_curve);

        if let Some(curves) = &original_index.issuer_credit_curves {
            builder = builder.issuer_curves(curves.clone());
        }
        if let Some(rates) = &original_index.issuer_recovery_rates {
            builder = builder.issuer_recovery_rates(rates.clone());
        }
        if let Some(weights) = &original_index.issuer_weights {
            builder = builder.issuer_weights(weights.clone());
        }

        builder.build()
    }

    fn bump_index_hazard(
        &self,
        original_index: &CreditIndexData,
        delta_lambda: f64,
    ) -> Result<CreditIndexData> {
        // Create bumped hazard curve
        let bumped_hazard_curve = original_index
            .index_credit_curve
            .with_parallel_bump(delta_lambda)?;

        self.rebuild_credit_index(
            original_index,
            original_index.recovery_rate,
            std::sync::Arc::new(bumped_hazard_curve),
            std::sync::Arc::clone(&original_index.base_correlation_curve),
        )
    }

    /// Calculate prior realized loss on the tranche as a fraction of original tranche notional.
    pub(super) fn calculate_prior_tranche_loss(&self, tranche: &CDSTranche) -> f64 {
        let l = tranche.accumulated_loss;
        let attach = tranche.attach_pct / 100.0;
        let detach = tranche.detach_pct / 100.0;
        let width = detach - attach;

        if width <= 1e-9 {
            return 0.0;
        }

        // Fraction of tranche already wiped out
        let loss_in_tranche = (l - attach).clamp(0.0, width);
        loss_in_tranche / width
    }

    /// Realized pool default state implied by the accumulated loss.
    ///
    /// `accumulated_loss` records the realized pool LOSS fraction `L`
    /// (original-pool units). With recovery `R`, the defaulted NOTIONAL
    /// fraction is `X = L / (1 − R)` and the recovered notional `G = X·R`
    /// is amortized from the top of the capital structure (senior-side
    /// writedown). Returns `(defaulted_fraction, recovered_fraction)`, both
    /// clamped so `X ≤ 1`.
    pub(super) fn realized_default_state(
        &self,
        tranche: &CDSTranche,
        recovery_rate: f64,
    ) -> (f64, f64) {
        let l = tranche.accumulated_loss.clamp(0.0, 1.0);
        let lgd = (1.0 - recovery_rate).max(1e-9);
        let defaulted = (l / lgd).min(1.0);
        let recovered = defaulted * recovery_rate.clamp(0.0, 1.0);
        (defaulted, recovered)
    }

    /// Fraction of the tranche notional already WRITTEN DOWN from the top by
    /// realized recoveries (senior-side amortization).
    ///
    /// Recovered notional `G` erodes the pool from `1.0` downward, so the
    /// tranche `[A, D]` loses `(G − (1−D))⁺ − (G − (1−A))⁺` of notional from
    /// the top. Returned as a fraction of tranche width, clamped so the sum
    /// with `calculate_prior_tranche_loss` never exceeds `1`.
    pub(super) fn calculate_prior_tranche_writedown(
        &self,
        tranche: &CDSTranche,
        recovery_rate: f64,
    ) -> f64 {
        let attach = tranche.attach_pct / 100.0;
        let detach = tranche.detach_pct / 100.0;
        let width = detach - attach;

        if width <= 1e-9 {
            return 0.0;
        }

        let (_, recovered) = self.realized_default_state(tranche, recovery_rate);
        let wd_top = (recovered - (1.0 - detach)).max(0.0) - (recovered - (1.0 - attach)).max(0.0);
        let wd_fraction = (wd_top / width).clamp(0.0, 1.0);

        // Joint cap: realized loss (bottom-up) + writedown (top-down) cannot
        // exceed the whole tranche.
        let loss_fraction = self.calculate_prior_tranche_loss(tranche);
        wd_fraction.min(1.0 - loss_fraction)
    }

    /// Generate payment schedule for the tranche using canonical schedule builder.
    ///
    /// Uses the robust date scheduling utilities with proper business day
    /// conventions and calendar support.
    pub(super) fn generate_payment_schedule(
        &self,
        tranche: &CDSTranche,
        as_of: Date,
    ) -> Result<Vec<Date>> {
        let start_date = tranche.contractual_effective_date(as_of).unwrap_or(as_of);

        let dates = if self.params.use_isda_coupon_dates || tranche.standard_imm_dates {
            let mut out = vec![start_date];
            let mut current = start_date;
            while current < tranche.maturity {
                current = next_cds_date(current);
                // Ensure we don't go past maturity (next_cds_date can go past if close)
                if current > tranche.maturity {
                    out.push(tranche.maturity);
                    break;
                }
                out.push(current);
            }
            // If precise maturity match is needed, we might need to adjust the last date
            // But standard CDS rolls on 20th.
            out
        } else {
            build_dates(
                start_date,
                tranche.maturity,
                tranche.frequency,
                self.params.schedule_stub,
                tranche.bdc,
                false,
                0,
                tranche
                    .calendar_id
                    .as_deref()
                    .unwrap_or(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID),
            )?
            .dates
        };

        // Filter out dates before as_of (in case effective_date < as_of)
        let payment_dates: Vec<Date> = dates.into_iter().filter(|&date| date > as_of).collect();

        Ok(payment_dates)
    }

    /// Calculate upfront amount for the tranche.
    ///
    /// This is the net present value at inception, representing the
    /// payment required to enter the position at the standard coupon.
    pub fn calculate_upfront(
        &self,
        tranche: &CDSTranche,
        market_ctx: &MarketContext,
        as_of: Date,
    ) -> Result<f64> {
        let pv = self.price_tranche(tranche, market_ctx, as_of)?;
        Ok(pv.amount())
    }

    /// Calculate Spread DV01 (sensitivity to 1bp change in running coupon).
    ///
    /// Uses central difference for O(h²) accuracy, consistent with CS01 and Correlation01.
    pub fn calculate_spread_dv01(
        &self,
        tranche: &CDSTranche,
        market_ctx: &MarketContext,
        as_of: Date,
    ) -> Result<f64> {
        // Central difference: (PV(c+1bp) - PV(c-1bp)) / 2
        let mut tranche_up = tranche.clone();
        tranche_up.running_coupon_bp += 1.0;

        let mut tranche_down = tranche.clone();
        tranche_down.running_coupon_bp -= 1.0;

        let pv_up = self.price_tranche(&tranche_up, market_ctx, as_of)?.amount();
        let pv_down = self
            .price_tranche(&tranche_down, market_ctx, as_of)?
            .amount();

        Ok((pv_up - pv_down) / 2.0)
    }

    /// Calculate the par spread (running coupon in bp that sets PV = 0).
    ///
    /// The par spread is always a positive basis-point number regardless of protection side.
    ///
    /// # Algorithm
    ///
    /// Uses Newton-Raphson iteration to find the spread that makes NPV = 0:
    /// 1. Seed with `|protection_pv| / |premium_per_bp|` — both legs are signed by
    ///    `project_discountable_rows` (opposite polarities per side), so unsigned magnitudes
    ///    give a correct positive starting point for both `BuyProtection` and `SellProtection`.
    /// 2. Iterate: `spread_new = spread - NPV(spread) / Spread_DV01`
    /// 3. Converge when `|NPV| < tolerance` or max iterations reached.
    ///
    /// This is more accurate than a plain ratio method because it accounts for the non-linear
    /// relationship between spread and premium leg PV due to accrual-on-default and notional
    /// write-down effects.
    #[must_use = "par spread result should be used"]
    pub fn calculate_par_spread(
        &self,
        tranche: &CDSTranche,
        market_ctx: &MarketContext,
        as_of: Date,
    ) -> Result<f64> {
        let discount_curve = market_ctx.get_discount(&tranche.discount_curve_id)?;

        // Initial guess: unsigned magnitude of protection PV divided by premium per bp.
        // Both quantities are signed by project_discountable_rows (opposite polarities for
        // BuyProtection vs SellProtection), so we use their absolute values to guarantee a
        // positive seed for both protection sides.
        let mut unit_tranche = tranche.clone();
        unit_tranche.running_coupon_bp = 1.0;
        let premium_per_bp_rows =
            self.project_discountable_rows(&unit_tranche, market_ctx, as_of)?;
        let premium_per_bp = self.discount_projected_rows(
            &premium_per_bp_rows
                .iter()
                .filter(|row| row.cashflow.kind == CFKind::Fixed)
                .cloned()
                .collect::<Vec<_>>(),
            discount_curve.as_ref(),
            as_of,
        )?;

        if premium_per_bp.abs() < NUMERICAL_TOLERANCE {
            return Ok(0.0);
        }

        let protection_rows = self.project_discountable_rows(tranche, market_ctx, as_of)?;
        let protection_pv = self.discount_projected_rows(
            &protection_rows
                .iter()
                .filter(|row| row.cashflow.kind == CFKind::DefaultedNotional)
                .cloned()
                .collect::<Vec<_>>(),
            discount_curve.as_ref(),
            as_of,
        )?;

        // Initial guess: unsigned ratio of protection PV magnitude to premium per bp magnitude.
        //
        // `project_discountable_rows` applies a side-dependent sign to every cashflow
        // (`premium_sign = -1 / +1` and `protection_sign = +1 / -1` for
        // `BuyProtection` / `SellProtection`).  Consequently both `protection_pv` and
        // `premium_per_bp` are signed with opposite polarities, making their raw ratio
        // always negative.  Taking unsigned magnitudes produces the correct positive
        // initial guess for both sides.
        let mut spread = protection_pv.abs() / premium_per_bp.abs().max(NUMERICAL_TOLERANCE);

        // Newton-Raphson iteration to refine the par spread.
        //
        // On non-convergence the solver returns an explicit error rather than
        // the last (un-converged) iterate: a silently-returned non-par spread
        // would feed wrong numbers into upfront / breakeven calculations with
        // no signal that the root-find failed.
        let mut last_npv = f64::INFINITY;
        for _iter in 0..PAR_SPREAD_MAX_ITER {
            // Create test tranche with current spread guess
            let mut test_tranche = tranche.clone();
            test_tranche.running_coupon_bp = spread;

            // Calculate NPV at current spread
            let npv = self
                .price_tranche(&test_tranche, market_ctx, as_of)?
                .amount();
            last_npv = npv;

            // Check convergence (NPV close to zero)
            if npv.abs() < PAR_SPREAD_TOLERANCE * tranche.notional.amount() {
                return Ok(spread);
            }

            // Calculate Spread DV01 for Newton step
            let spread_dv01 = self.calculate_spread_dv01(&test_tranche, market_ctx, as_of)?;

            if spread_dv01.abs() < NUMERICAL_TOLERANCE {
                // Degenerate Jacobian: the Newton step is undefined, so the
                // par spread cannot be resolved. Fail loudly.
                return Err(finstack_core::Error::Validation(format!(
                    "CDS tranche par-spread solve failed: Spread DV01 collapsed to \
                     {spread_dv01:.3e} at spread {spread:.4} bp (NPV {npv:.3e}); the \
                     premium leg is insensitive to the coupon so no par spread exists."
                )));
            }

            // Newton step: spread_new = spread - NPV / DV01.
            // The seed is always positive, so the clamp keeps subsequent iterates
            // non-negative for both protection sides.
            let adjustment = npv / spread_dv01;
            spread -= adjustment;

            // Ensure spread stays reasonable (non-negative, bounded)
            spread = spread.clamp(0.0, 100000.0); // Max 10000% = 100000bp
        }

        // Exhausted the iteration budget without meeting the NPV tolerance.
        Err(finstack_core::Error::Validation(format!(
            "CDS tranche par-spread solve did not converge within {PAR_SPREAD_MAX_ITER} \
             Newton iterations: last spread {spread:.4} bp leaves NPV {last_npv:.3e} \
             (tolerance {:.3e}). Inspect the credit/discount curves or widen the \
             iteration budget.",
            PAR_SPREAD_TOLERANCE * tranche.notional.amount()
        )))
    }

    /// Calculate expected loss metric (the total expected loss at maturity).
    pub fn calculate_expected_loss(
        &self,
        tranche: &CDSTranche,
        market_ctx: &MarketContext,
    ) -> Result<f64> {
        let index_data_arc = market_ctx.get_credit_index(&tranche.credit_index_id)?;
        self.calculate_expected_tranche_loss(tranche, index_data_arc.as_ref(), tranche.maturity)
    }

    /// Calculate CS01 (sensitivity to 1bp parallel shift in credit spreads) using central difference.
    #[must_use = "CS01 result should be used for hedging"]
    pub fn calculate_cs01(
        &self,
        tranche: &CDSTranche,
        market_ctx: &MarketContext,
        as_of: Date,
    ) -> Result<f64> {
        if self.params.cs01_bump_size <= 0.0 {
            return Err(finstack_core::Error::Validation(
                "CS01 bump size must be positive".to_string(),
            ));
        }

        let original_index_arc = market_ctx.get_credit_index(&tranche.credit_index_id)?;
        let delta_lambda = self.params.cs01_bump_size * 1e-4;

        // Central difference: (PV_up - PV_down) / 2 for O(h²) accuracy
        let bumped_index_up = self.bump_index_hazard(original_index_arc.as_ref(), delta_lambda)?;
        let bumped_index_down =
            self.bump_index_hazard(original_index_arc.as_ref(), -delta_lambda)?;

        let ctx_up = market_ctx
            .clone()
            .insert_credit_index(&tranche.credit_index_id, bumped_index_up);
        let ctx_down = market_ctx
            .clone()
            .insert_credit_index(&tranche.credit_index_id, bumped_index_down);

        let pv_up = self.price_tranche(tranche, &ctx_up, as_of)?.amount();
        let pv_down = self.price_tranche(tranche, &ctx_down, as_of)?.amount();

        // Return sensitivity normalized to a 1bp configured bump.
        Ok((pv_up - pv_down) / (2.0 * self.params.cs01_bump_size))
    }

    /// Calculate correlation delta (sensitivity to correlation changes) using central difference.
    #[must_use = "Correlation01 result should be used for hedging"]
    pub fn calculate_correlation_delta(
        &self,
        tranche: &CDSTranche,
        market_ctx: &MarketContext,
        as_of: Date,
    ) -> Result<f64> {
        let bump_abs = self.params.corr_bump_abs;
        let original_index_arc = market_ctx.get_credit_index(&tranche.credit_index_id)?;

        // Central difference: (PV_up - PV_down) / (2 * bump) for O(h²) accuracy
        let bumped_corr_curve_up =
            self.bump_base_correlation(&original_index_arc.base_correlation_curve, bump_abs)?;
        let bumped_corr_curve_down =
            self.bump_base_correlation(&original_index_arc.base_correlation_curve, -bump_abs)?;

        let bumped_index_up = self.rebuild_credit_index(
            original_index_arc.as_ref(),
            original_index_arc.recovery_rate,
            std::sync::Arc::clone(&original_index_arc.index_credit_curve),
            std::sync::Arc::new(bumped_corr_curve_up),
        )?;

        let bumped_index_down = self.rebuild_credit_index(
            original_index_arc.as_ref(),
            original_index_arc.recovery_rate,
            std::sync::Arc::clone(&original_index_arc.index_credit_curve),
            std::sync::Arc::new(bumped_corr_curve_down),
        )?;

        let ctx_up = market_ctx
            .clone()
            .insert_credit_index(&tranche.credit_index_id, bumped_index_up);
        let ctx_down = market_ctx
            .clone()
            .insert_credit_index(&tranche.credit_index_id, bumped_index_down);

        let pv_up = self.price_tranche(tranche, &ctx_up, as_of)?.amount();
        let pv_down = self.price_tranche(tranche, &ctx_down, as_of)?.amount();

        // Return sensitivity per unit correlation change (central difference)
        Ok((pv_up - pv_down) / (2.0 * bump_abs))
    }

    /// Calculate jump-to-default (immediate loss from specific entity default).
    ///
    /// For a homogeneous portfolio, estimates the immediate impact if one average
    /// entity defaults instantly. This is distinct from correlation sensitivity.
    ///
    /// Returns the average JTD across all constituents. For detailed min/max/avg,
    /// use `calculate_jump_to_default_detail`.
    #[must_use = "JTD result should be used for risk management"]
    pub fn calculate_jump_to_default(
        &self,
        tranche: &CDSTranche,
        market_ctx: &MarketContext,
        _as_of: Date,
    ) -> Result<f64> {
        let detail = self.calculate_jump_to_default_detail(tranche, market_ctx)?;
        Ok(detail.average)
    }

    /// Calculate detailed jump-to-default metrics including min, max, and average.
    ///
    /// For heterogeneous portfolios with issuer-specific recovery rates or weights,
    /// this provides the full distribution of JTD impacts.
    ///
    /// # Returns
    ///
    /// `JumpToDefaultResult` containing:
    /// - `min`: JTD for the smallest impact name
    /// - `max`: JTD for the largest impact name (worst case for risk)
    /// - `average`: Average JTD across all names
    /// - `count`: Number of names that would impact this tranche
    pub fn calculate_jump_to_default_detail(
        &self,
        tranche: &CDSTranche,
        market_ctx: &MarketContext,
    ) -> Result<JumpToDefaultResult> {
        let index_data = market_ctx.get_credit_index(&tranche.credit_index_id)?;

        let attach_frac = tranche.attach_pct / 100.0;
        let detach_frac = tranche.detach_pct / 100.0;
        let tranche_width = detach_frac - attach_frac;
        let tranche_notional = tranche.notional.amount();

        // Handle zero-width tranche edge case
        if tranche_width <= NUMERICAL_TOLERANCE {
            return Ok(JumpToDefaultResult {
                min: 0.0,
                max: 0.0,
                average: 0.0,
                count: 0,
            });
        }

        let num_constituents = index_data.num_constituents as usize;
        let base_weight = 1.0 / (num_constituents as f64);
        let base_recovery = index_data.recovery_rate;
        let width = detach_frac - attach_frac;
        let current_loss = tranche.accumulated_loss;

        // Collect JTD impacts for all names
        let mut impacts: Vec<f64> = Vec::with_capacity(num_constituents);
        let mut impacting_count = 0;

        let loss_in_tranche_before = (current_loss - attach_frac).clamp(0.0, width);

        if index_data.has_issuer_curves() {
            if let Some(curves) = &index_data.issuer_credit_curves {
                let mut sorted_ids: Vec<&str> = curves.keys().map(String::as_str).collect();
                sorted_ids.sort();
                for id in sorted_ids {
                    let individual_weight = index_data.get_issuer_weight(id);
                    let recovery = index_data.get_issuer_recovery(id);
                    let individual_loss = individual_weight * (1.0 - recovery);

                    let loss_in_tranche_after =
                        (current_loss + individual_loss - attach_frac).clamp(0.0, width);
                    let incremental = (loss_in_tranche_after - loss_in_tranche_before).max(0.0);
                    let impact_amount = if incremental > 0.0 {
                        impacting_count += 1;
                        tranche_notional * (incremental / width)
                    } else {
                        0.0
                    };
                    impacts.push(impact_amount);
                }
            }
        } else {
            for _i in 0..num_constituents {
                let individual_loss = base_weight * (1.0 - base_recovery);

                let loss_in_tranche_after =
                    (current_loss + individual_loss - attach_frac).clamp(0.0, width);
                let incremental = (loss_in_tranche_after - loss_in_tranche_before).max(0.0);
                let impact_amount = if incremental > 0.0 {
                    impacting_count += 1;
                    tranche_notional * (incremental / width)
                } else {
                    0.0
                };
                impacts.push(impact_amount);
            }
        }

        // Calculate min, max, average
        let (min, max, sum) = if impacts.is_empty() {
            (0.0, 0.0, 0.0)
        } else {
            impacts.iter().fold(
                (f64::INFINITY, f64::NEG_INFINITY, 0.0),
                |(min, max, sum), &impact| (min.min(impact), max.max(impact), sum + impact),
            )
        };

        let average = if !impacts.is_empty() {
            sum / (impacts.len() as f64)
        } else {
            0.0
        };

        Ok(JumpToDefaultResult {
            min,
            max,
            average,
            count: impacting_count,
        })
    }

    /// Calculate accrued premium on the tranche.
    ///
    /// Returns the premium accrued since the last payment date, calculated on
    /// the outstanding notional (after accounting for any realized losses).
    ///
    /// # Calculation
    ///
    /// ```text
    /// Accrued = Coupon × Accrual_Fraction × Outstanding_Notional
    /// ```
    ///
    /// Where:
    /// - Coupon is the running coupon rate (running_coupon_bp / 10000)
    /// - Accrual_Fraction is the day count fraction from last payment to as_of
    /// - Outstanding_Notional accounts for any realized losses
    ///
    /// # Use Cases
    ///
    /// - Dirty vs clean price: `dirty_price = clean_price + accrued`
    /// - Settlement amount calculation
    /// - Mark-to-market accounting
    #[must_use = "accrued premium result should be used"]
    pub fn calculate_accrued_premium(
        &self,
        tranche: &CDSTranche,
        market_ctx: &MarketContext,
        as_of: Date,
    ) -> Result<f64> {
        let start_date = tranche.contractual_effective_date(as_of).ok_or_else(|| {
            Error::Validation(
                "CDS tranche accrued premium requires an explicit effective_date for non-standard schedules"
                    .to_string(),
            )
        })?;

        // Get credit index data for loss calculations
        let index_data = match market_ctx.get_credit_index(&tranche.credit_index_id) {
            Ok(data) => data,
            Err(_) => return Ok(0.0), // No credit data, no accrued
        };

        // Generate the payment schedule
        let payment_dates = self.generate_payment_schedule(tranche, start_date)?;

        // Find the last payment date on or before as_of
        let last_payment = payment_dates
            .iter()
            .filter(|&&d| d <= as_of)
            .max()
            .copied()
            .unwrap_or(start_date);

        // Find the next payment date after as_of
        let next_payment = payment_dates.iter().filter(|&&d| d > as_of).min().copied();

        // If no next payment, we're past maturity
        let _next_payment = match next_payment {
            Some(d) => d,
            None => return Ok(0.0),
        };

        // Calculate the accrual fraction from last payment to as_of
        let accrual_fraction = tranche
            .day_count
            .year_fraction(
                last_payment,
                as_of,
                finstack_core::dates::DayCountContext::default(),
            )
            .unwrap_or(0.0);

        if accrual_fraction <= 0.0 {
            return Ok(0.0);
        }

        // Calculate outstanding notional (accounting for realized losses)
        let prior_loss = self.calculate_prior_tranche_loss(tranche);
        let outstanding_notional = tranche.notional.amount() * (1.0 - prior_loss);

        // Also factor in expected loss if we want to be more precise
        // For simplicity, use outstanding based on realized loss only
        let _ = index_data; // Mark as used (could compute expected loss here)

        // Calculate accrued premium
        let coupon = tranche.running_coupon_bp / BASIS_POINTS_PER_UNIT;
        let accrued = coupon * accrual_fraction * outstanding_notional;

        Ok(accrued)
    }

    /// Expose the expected loss curve for diagnostic and debugging purposes.
    ///
    /// Returns a vector of (Date, EL_fraction) pairs where EL_fraction
    /// is the cumulative expected loss as a fraction of tranche notional [0, 1].
    ///
    /// This is useful for:
    /// - Visualizing the expected loss profile over time
    /// - Debugging pricing discrepancies
    /// - Validating model behavior
    pub fn get_expected_loss_curve(
        &self,
        tranche: &CDSTranche,
        market_ctx: &MarketContext,
        as_of: Date,
    ) -> Result<Vec<(Date, f64)>> {
        let index_data = market_ctx.get_credit_index(&tranche.credit_index_id)?;
        let payment_dates = self.generate_payment_schedule(tranche, as_of)?;
        self.build_el_curve(tranche, index_data.as_ref(), &payment_dates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// W-20: `conditional_equity_tranche_loss` evaluates the conditional binomial
    /// PMF for every `k` in `0..=N` via [`binomial_pmf_all`]. With `N = 125` a
    /// naive factorial-based binomial would overflow `C(125, 62)` and yield
    /// `inf`/`0`. Confirm the recurrence stays numerically stable:
    /// `C(125, 62) * 0.5^125` is finite and matches `0.07094031336820422`.
    #[test]
    fn binomial_pmf_all_finite_for_large_n() {
        let pmf = binomial_pmf_all(125, 0.5);
        let p = pmf[62];
        assert!(
            p.is_finite(),
            "binomial_pmf_all(125,0.5)[62] must be finite, got {p}"
        );
        assert!(p > 0.0, "probability must be strictly positive, got {p}");
        // Exact: comb(125,62) * 0.5^125.
        assert!(
            (p - 0.070_940_313_368_204_22).abs() < 1e-12,
            "binomial_pmf_all(125,0.5)[62] = {p}, expected 0.07094031336820422"
        );
    }

    /// W-20: the conditional equity-tranche loss must be a finite, sane number
    /// for a full 125-name index. A binomial overflow inside the `0..=N` sum
    /// would propagate `inf`/`NaN` into the expected loss.
    #[test]
    fn conditional_equity_tranche_loss_finite_for_full_index() {
        let pricer = CDSTranchePricer::new();
        let el = pricer.conditional_equity_tranche_capped(
            125,        // num_constituents
            0.03,       // cap_notional (3% equity tranche)
            0.10,       // conditional default probability
            1.0 - 0.40, // exposure = LGD at 40% recovery
        );
        assert!(
            el.is_finite(),
            "conditional equity tranche loss must be finite, got {el}"
        );
        // Expected loss of a [0, 3%] tranche is bounded by the detachment.
        assert!(
            (0.0..=0.03 + 1e-12).contains(&el),
            "equity tranche EL {el} must lie in [0, detachment]"
        );

        // The binomial pmf over 0..=N must form a valid probability mass.
        let total: f64 = binomial_pmf_all(125, 0.10).iter().sum();
        assert!(
            (total - 1.0).abs() < 1e-9,
            "binomial pmf over 0..=125 must sum to 1, got {total}"
        );
    }
}
