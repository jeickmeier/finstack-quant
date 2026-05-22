use super::config::{
    CDSTranchePricer, ADAPTIVE_INTEGRATION_HIGH, ADAPTIVE_INTEGRATION_LOW, NUMERICAL_TOLERANCE,
    PROBABILITY_CLIP,
};
use crate::correlation::recovery::RecoveryModel;
use crate::instruments::credit_derivatives::cds_tranche::CDSTranche;
use finstack_core::dates::Date;
use finstack_core::market_data::term_structures::CreditIndexData;
use finstack_core::math::standard_normal_inv_cdf;
use finstack_core::{Error, Result};

/// Magnitude below which a negative base-correlation tranchelet difference is
/// treated as benign numerical noise (quadrature / interpolation rounding)
/// rather than genuine base-correlation arbitrage. A senior equity
/// tranchelet `EL(0,D)` should never fall below the junior `EL(0,A)`; a gap
/// at or below this size is consistent with floating-point integration error
/// on EL values that are themselves `O(1e-2)`.
const BASE_CORR_ARBITRAGE_TOL: f64 = 1e-9;

/// Pre-computed invariants for EL fraction evaluation (hoisted out of the date loop).
struct ElInvariants {
    eff_attach: f64,
    eff_detach: f64,
    survival_factor: f64,
    corr_attach: f64,
    corr_detach: f64,
    orig_width: f64,
    prior_loss: f64,
    /// Original (contractual) attachment point, percent. Retained for
    /// base-correlation arbitrage diagnostics.
    attach_pct: f64,
    /// Original (contractual) detachment point, percent.
    detach_pct: f64,
}

impl CDSTranchePricer {
    /// Calculate expected tranche loss using the base correlation approach.
    ///
    /// Decomposes the tranche [A, D] as the difference between two equity
    /// tranches: EL(0, D) - EL(0, A), using correlations interpolated from
    /// the base correlation curve with enhanced numerical stability.
    pub(super) fn calculate_expected_tranche_loss(
        &self,
        tranche: &CDSTranche,
        index_data: &CreditIndexData,
        maturity: Date,
    ) -> Result<f64> {
        let (eff_attach, eff_detach, survival_factor) = self.calculate_effective_structure(tranche);

        // If effective width is zero, no loss
        if eff_detach <= eff_attach {
            return Ok(0.0);
        }

        // Get correlations for ORIGINAL attachment and detachment points
        // Base correlation is sticky to the original structure
        let corr_attach = index_data
            .base_correlation_curve
            .correlation(tranche.attach_pct);
        let corr_detach = index_data
            .base_correlation_curve
            .correlation(tranche.detach_pct);

        // Apply enhanced correlation boundary handling for numerical stability
        let corr_attach = self.smooth_correlation_boundary(corr_attach);
        let corr_detach = self.smooth_correlation_boundary(corr_detach);

        // Calculate expected losses for equity tranches [0, A_eff] and [0, D_eff]
        // Note: These inputs to calculate_equity_tranche_loss are now in "Effective %" terms
        // but correlations are from "Original %" terms.
        let el_to_attach = self.calculate_equity_tranche_loss(
            eff_attach * 100.0,
            corr_attach,
            index_data,
            maturity,
        )?;

        let el_to_detach = self.calculate_equity_tranche_loss(
            eff_detach * 100.0,
            corr_detach,
            index_data,
            maturity,
        )?;

        // The [A_eff, D_eff] tranche loss as a fraction of CURRENT portfolio.
        // EL(0,D) − EL(0,A) is the tranchelet EL; it must be non-negative
        // because a wider equity tranche cannot lose less than a narrower
        // one. A negative value signals genuine base-correlation arbitrage
        // (ρ(D) and ρ(A) inconsistent) — surfaced explicitly rather than
        // silently clamped to zero protection.
        let current_portfolio_loss_fraction = self.resolve_tranchelet_difference(
            el_to_detach,
            el_to_attach,
            tranche.attach_pct,
            tranche.detach_pct,
            maturity,
        )?;

        // Convert to currency amount:
        // Loss = CurrentPortFrac * CurrentPortNotional
        // CurrentPortNotional = OrigPortNotional * (1 - L)
        // OrigPortNotional = TrancheNotional / (D_orig - A_orig)

        let orig_width = (tranche.detach_pct - tranche.attach_pct) / 100.0;
        if orig_width <= 1e-9 {
            return Ok(0.0);
        }

        let orig_port_notional = tranche.notional.amount() / orig_width;
        let loss_amount = current_portfolio_loss_fraction * orig_port_notional * survival_factor;

        Ok(loss_amount)
    }

    /// Compute the date-independent invariants needed for EL fraction evaluation.
    fn el_invariants(
        &self,
        tranche: &CDSTranche,
        index_data: &CreditIndexData,
    ) -> Result<ElInvariants> {
        let (eff_attach, eff_detach, survival_factor) = self.calculate_effective_structure(tranche);
        if eff_detach <= eff_attach {
            return Ok(ElInvariants {
                eff_attach: 0.0,
                eff_detach: 0.0,
                survival_factor: 0.0,
                corr_attach: 0.0,
                corr_detach: 0.0,
                orig_width: 0.0,
                prior_loss: 0.0,
                attach_pct: tranche.attach_pct,
                detach_pct: tranche.detach_pct,
            });
        }
        let corr_attach = self.smooth_correlation_boundary(
            index_data
                .base_correlation_curve
                .correlation(tranche.attach_pct),
        );
        let corr_detach = self.smooth_correlation_boundary(
            index_data
                .base_correlation_curve
                .correlation(tranche.detach_pct),
        );
        let orig_width = (tranche.detach_pct - tranche.attach_pct) / 100.0;
        let prior_loss = self.calculate_prior_tranche_loss(tranche);
        Ok(ElInvariants {
            eff_attach,
            eff_detach,
            survival_factor,
            corr_attach,
            corr_detach,
            orig_width,
            prior_loss,
            attach_pct: tranche.attach_pct,
            detach_pct: tranche.detach_pct,
        })
    }

    /// EL fraction at a date using pre-computed invariants (avoids redundant
    /// effective-structure and base-correlation lookups per date).
    fn el_fraction_at_date(
        &self,
        inv: &ElInvariants,
        index_data: &CreditIndexData,
        date: Date,
    ) -> Result<f64> {
        if inv.eff_detach <= inv.eff_attach || inv.orig_width <= 1e-9 {
            return Ok(0.0);
        }
        let el_to_attach = self.calculate_equity_tranche_loss(
            inv.eff_attach * 100.0,
            inv.corr_attach,
            index_data,
            date,
        )?;
        let el_to_detach = self.calculate_equity_tranche_loss(
            inv.eff_detach * 100.0,
            inv.corr_detach,
            index_data,
            date,
        )?;
        // Surface base-correlation arbitrage explicitly (see
        // `resolve_tranchelet_difference`) instead of silently clamping a
        // tranche's protection leg to zero.
        let current_portfolio_loss_fraction = self.resolve_tranchelet_difference(
            el_to_detach,
            el_to_attach,
            inv.attach_pct,
            inv.detach_pct,
            date,
        )?;
        let tranche_loss_fraction =
            (current_portfolio_loss_fraction * inv.survival_factor) / inv.orig_width;
        Ok((tranche_loss_fraction + inv.prior_loss).clamp(0.0, 1.0))
    }

    /// Reconcile the base-correlation tranchelet difference `EL(0,D) − EL(0,A)`.
    ///
    /// The two equity expected losses are computed at *different* correlations
    /// (`ρ(D)` and `ρ(A)` from the base-correlation curve). A well-formed,
    /// arbitrage-free base-correlation curve guarantees `EL(0,D) ≥ EL(0,A)` —
    /// a wider equity tranche must have at least as much expected loss. When
    /// the difference is negative the curve is *not* arbitrage-free at the
    /// `[A, D]` strikes.
    ///
    /// Behaviour:
    /// - A negative gap within `BASE_CORR_ARBITRAGE_TOL` is benign quadrature
    ///   / interpolation noise and is clamped to zero silently.
    /// - A negative gap *beyond* the tolerance is genuine base-correlation
    ///   arbitrage. With `validate_arbitrage_free = true` (the default) this
    ///   returns an explicit [`Error::Validation`] naming the strikes and the
    ///   magnitude, so the caller cannot unknowingly price a senior tranche
    ///   with zero protection. With `validate_arbitrage_free = false` it is
    ///   clamped to zero but logged at `warn` level (not the previous silent
    ///   `debug`), so the degradation is at least visible in logs.
    fn resolve_tranchelet_difference(
        &self,
        el_to_detach: f64,
        el_to_attach: f64,
        attach_pct: f64,
        detach_pct: f64,
        date: Date,
    ) -> Result<f64> {
        let diff = el_to_detach - el_to_attach;
        if diff >= -BASE_CORR_ARBITRAGE_TOL {
            // Non-negative (up to numerical noise) — clamp the tiny residual.
            return Ok(diff.max(0.0));
        }

        // Genuine base-correlation arbitrage: EL(0,D) < EL(0,A).
        if self.params.validate_arbitrage_free {
            return Err(Error::Validation(format!(
                "base-correlation arbitrage at strikes [{attach_pct:.4}%, {detach_pct:.4}%] \
                 on {date:?}: equity EL(0,{detach_pct:.4}%)={el_to_detach:.8} is below \
                 EL(0,{attach_pct:.4}%)={el_to_attach:.8} (gap {diff:.2e}). The base-correlation \
                 curve is not arbitrage-free at these detachment points; pricing this tranche \
                 would assign it negative protection. Re-fit the base-correlation curve (e.g. \
                 isotonic / PAVA smoothing) or disable validation via \
                 `CDSTranchePricerConfig::with_arbitrage_validation(false)` to clamp instead."
            )));
        }

        // Validation disabled: clamp but make the degradation visible.
        tracing::warn!(
            attach_pct,
            detach_pct,
            el_to_detach,
            el_to_attach,
            gap = diff,
            "base-correlation arbitrage on {date:?}: equity EL(0,D) < EL(0,A); \
             clamping tranchelet protection to zero (arbitrage validation disabled)"
        );
        Ok(0.0)
    }

    /// Build the expected loss curve for all payment dates.
    ///
    /// Returns a vector of (Date, EL_fraction) pairs where EL_fraction
    /// is the cumulative expected loss as a fraction of tranche notional.
    ///
    /// When `enforce_el_monotonicity` is enabled (default), any computed EL
    /// value that is less than the previous date's EL will be clamped to
    /// maintain monotonicity. This prevents small arbitrage opportunities
    /// that can arise from base correlation model inconsistencies.
    pub(super) fn build_el_curve(
        &self,
        tranche: &CDSTranche,
        index_data: &CreditIndexData,
        dates: &[Date],
    ) -> Result<Vec<(Date, f64)>> {
        let inv = self.el_invariants(tranche, index_data)?;
        let mut el_curve = Vec::with_capacity(dates.len());
        let mut prev_el = 0.0;

        for &date in dates {
            let mut el_fraction = self.el_fraction_at_date(&inv, index_data, date)?;

            // Check for non-monotonic EL (indicates numerical issue or model limitation)
            // This can happen due to base correlation model inconsistencies
            if el_fraction < prev_el - 1e-6 {
                tracing::debug!(
                    "EL decreased from {:.6} to {:.6} at {:?} (Δ={:.6}){}",
                    prev_el,
                    el_fraction,
                    date,
                    prev_el - el_fraction,
                    if self.params.enforce_el_monotonicity {
                        " - enforcing monotonicity"
                    } else {
                        ""
                    }
                );

                // Enforce monotonicity if configured (default: true)
                if self.params.enforce_el_monotonicity {
                    el_fraction = prev_el;
                }
            }

            el_curve.push((date, el_fraction));
            prev_el = el_fraction;
        }

        Ok(el_curve)
    }

    /// Calculate expected loss for an equity tranche [0, K] using Gaussian Copula.
    ///
    /// Enhanced with adaptive integration for superior numerical stability,
    /// particularly critical near correlation boundaries (0 and 1) where
    /// the conditional default probability function exhibits sharp transitions.
    ///
    /// # Arguments
    /// * `detachment_pct` - Detachment point K in percent
    /// * `correlation` - Asset correlation parameter ρ
    /// * `index_data` - Credit index market data
    /// * `maturity` - Maturity date for loss calculation
    pub(super) fn calculate_equity_tranche_loss(
        &self,
        detachment_pct: f64,
        correlation: f64,
        index_data: &CreditIndexData,
        maturity: Date,
    ) -> Result<f64> {
        // Heterogeneous path if enabled and issuer curves present
        if self.params.use_issuer_curves && index_data.has_issuer_curves() {
            self.calculate_equity_tranche_loss_hetero(
                detachment_pct,
                correlation,
                index_data,
                maturity,
            )
        } else {
            // Homogeneous: use index marginals
            let num_constituents = index_data.num_constituents as usize;
            let base_recovery = index_data.recovery_rate;

            // Build recovery model if configured, otherwise use constant
            let recovery_model: Option<Box<dyn RecoveryModel>> =
                self.params.recovery_spec.as_ref().map(|spec| spec.build());

            let detachment_notional = detachment_pct / 100.0;
            let maturity_years = self.years_from_base(index_data, maturity)?;
            let default_prob = self.get_default_probability(index_data, maturity_years)?;
            let correlation = self.smooth_correlation_boundary(correlation);

            if self.params.copula_spec.is_gaussian() {
                let quad = self.select_quadrature()?;
                // Clamp to the same open-interval guard used by the heterogeneous
                // path (`default_threshold_for_copula`).  `get_default_probability`
                // already clamps to `[0, 1]`, but extreme values at the boundary
                // (0 → −∞, 1 → +∞) still produce non-finite thresholds and
                // incorrect EL integrals.  Clamping to `[PROBABILITY_CLIP, 1−PROBABILITY_CLIP]`
                // keeps the probit finite and matches the heterogeneous branch.
                let default_prob_clamped =
                    default_prob.clamp(PROBABILITY_CLIP, 1.0 - PROBABILITY_CLIP);
                let default_threshold = standard_normal_inv_cdf(default_prob_clamped);
                let integrand = |z: f64| {
                    let p = self.conditional_default_probability_enhanced(
                        default_threshold,
                        correlation,
                        z,
                    );

                    // Use stochastic recovery if configured, otherwise constant
                    let recovery_rate = match &recovery_model {
                        Some(model) => model.conditional_recovery(z),
                        None => base_recovery,
                    };

                    self.conditional_equity_tranche_loss(
                        num_constituents,
                        detachment_notional,
                        p,
                        recovery_rate,
                    )
                };
                let expected_loss = if !(ADAPTIVE_INTEGRATION_LOW..=ADAPTIVE_INTEGRATION_HIGH)
                    .contains(&correlation)
                {
                    quad.integrate_adaptive(integrand, NUMERICAL_TOLERANCE)
                } else {
                    quad.integrate(integrand)
                };
                Ok(expected_loss)
            } else {
                let copula_ref = self.copula();
                let default_threshold = self.default_threshold_for_copula(default_prob);
                let expected_loss = copula_ref.integrate_fn(&|factors| {
                    let p = self.conditional_default_prob_copula(
                        copula_ref,
                        default_threshold,
                        factors,
                        correlation,
                    );

                    let z = factors.first().copied().unwrap_or(0.0);
                    let recovery_rate = match &recovery_model {
                        Some(model) => model.conditional_recovery(z),
                        None => base_recovery,
                    };

                    self.conditional_equity_tranche_loss(
                        num_constituents,
                        detachment_notional,
                        p,
                        recovery_rate,
                    )
                });
                Ok(expected_loss)
            }
        }
    }
}
