//! Expected tranche loss and the expected loss/write-down curve, with the
//! equity-tranche capped expectations they are built from.

#[cfg(test)]
use super::pool::HomogeneousPool;
use super::pool::PreparedPool;
use super::pricer::{
    CdsTranchePricer, ElWdPoint, INTEGRATION_TOLERANCE, PROBABILITY_CLIP, PoolExposure,
};
use crate::cds_tranche::CdsTranche;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::term_structures::{BaseCorrelationCurve, CreditIndexData};
use finstack_quant_core::{Error, Result};
use finstack_quant_models::correlation::CopulaSpec;
use std::sync::{Arc, Mutex};

/// Pre-computed invariants for EL fraction evaluation (hoisted out of the date loop).
#[derive(Clone, Copy)]
struct ElInvariants {
    eff_attach: f64,
    eff_detach: f64,
    pool_factor: f64,
    orig_width: f64,
    prior_loss: f64,
    /// Fraction of tranche notional already written down from the top by
    /// realized recoveries (senior-side amortization).
    prior_wd: f64,
    /// Original (contractual) attachment point, percent. Retained for
    /// base-correlation arbitrage diagnostics.
    attach_pct: f64,
    /// Original (contractual) detachment point, percent.
    detach_pct: f64,
    /// Tranche maturity: the horizon the base-correlation curve is quoted for
    /// and the only date at which a negative tranchelet loss is arbitrage.
    maturity: Date,
}

impl CdsTranchePricer {
    /// Calculate expected tranche loss using the base correlation approach.
    ///
    /// Decomposes the tranche [A, D] as the difference between two equity
    /// tranches: EL(0, D) - EL(0, A), using correlations interpolated from
    /// the base correlation curve with enhanced numerical stability.
    pub(super) fn calculate_expected_tranche_loss(
        &self,
        tranche: &CdsTranche,
        index_data: &CreditIndexData,
        maturity: Date,
    ) -> Result<f64> {
        let eff = self.calculate_effective_structure(tranche, index_data.recovery_rate());
        let (eff_attach, eff_detach, pool_factor) =
            (eff.eff_attach, eff.eff_detach, eff.pool_factor);

        // If effective width is zero, no loss
        if eff_detach <= eff_attach {
            return Ok(0.0);
        }

        // Get correlations for ORIGINAL attachment and detachment points
        // Base correlation is sticky to the original structure
        let corr_attach = index_data
            .base_correlation_curve()
            .correlation(tranche.attach_pct);
        let corr_detach = index_data
            .base_correlation_curve()
            .correlation(tranche.detach_pct);

        // Apply enhanced correlation boundary handling for numerical stability
        let corr_attach = self.smooth_correlation_boundary(corr_attach);
        let corr_detach = self.smooth_correlation_boundary(corr_detach);

        // Calculate expected losses for equity tranches [0, A_eff] and [0, D_eff]
        // Note: These inputs to calculate_equity_tranche_capped are now in "Effective %" terms
        // but correlations are from "Original %" terms.
        let el_to_attach = self.calculate_equity_tranche_capped(
            eff_attach * 100.0,
            corr_attach,
            index_data,
            maturity,
            PoolExposure::Loss,
        )?;

        let el_to_detach = self.calculate_equity_tranche_capped(
            eff_detach * 100.0,
            corr_detach,
            index_data,
            maturity,
            PoolExposure::Loss,
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
        let loss_amount = current_portfolio_loss_fraction * orig_port_notional * pool_factor;

        Ok(loss_amount)
    }

    /// Compute the date-independent invariants needed for EL fraction evaluation.
    fn el_invariants(
        &self,
        tranche: &CdsTranche,
        index_data: &CreditIndexData,
    ) -> Result<ElInvariants> {
        let eff = self.calculate_effective_structure(tranche, index_data.recovery_rate());
        if eff.eff_detach <= eff.eff_attach {
            return Ok(ElInvariants {
                eff_attach: 0.0,
                eff_detach: 0.0,
                pool_factor: 0.0,
                orig_width: 0.0,
                prior_loss: 0.0,
                prior_wd: 0.0,
                attach_pct: tranche.attach_pct,
                detach_pct: tranche.detach_pct,
                maturity: tranche.maturity,
            });
        }
        let orig_width = (tranche.detach_pct - tranche.attach_pct) / 100.0;
        let prior_loss = self.calculate_prior_tranche_loss(tranche);
        let prior_wd = self.calculate_prior_tranche_writedown(tranche, index_data.recovery_rate());
        Ok(ElInvariants {
            eff_attach: eff.eff_attach,
            eff_detach: eff.eff_detach,
            pool_factor: eff.pool_factor,
            orig_width,
            prior_loss,
            prior_wd,
            attach_pct: tranche.attach_pct,
            detach_pct: tranche.detach_pct,
            maturity: tranche.maturity,
        })
    }

    /// EL and recovery-writedown fractions at a date using pre-computed
    /// invariants (avoids redundant effective-structure and base-correlation
    /// lookups per date). Returns `(el_fraction, wd_fraction)` with
    /// `el + wd ≤ 1`.
    fn el_wd_fraction_from_caps(
        &self,
        inv: &ElInvariants,
        date: Date,
        [
            (el_to_attach, rec_to_attach_cap),
            (el_to_detach, rec_to_detach_cap),
        ]: [(f64, f64); 2],
    ) -> Result<(f64, f64)> {
        // Base correlations are quoted for the tranche maturity, so the
        // tranchelet loss is only guaranteed non-negative there. Applying the
        // same pair of correlations at an earlier date is a model
        // interpolation in time, and a steep skew dips slightly below zero at
        // short horizons (order 1e-5 of pool notional) without any arbitrage
        // in the quotes. Those interim losses are floored at zero and the
        // curve is then made monotone in time; arbitrage at maturity is still
        // an error (see `resolve_tranchelet_difference`).
        let current_portfolio_loss_fraction = if date < inv.maturity {
            let diff = el_to_detach - el_to_attach;
            if !diff.is_finite() {
                return Err(Error::Validation(
                    "non-finite base-correlation loss difference".to_owned(),
                ));
            }
            if diff < 0.0 {
                tracing::debug!(
                    attach_pct = inv.attach_pct,
                    detach_pct = inv.detach_pct,
                    ?date,
                    gap = diff,
                    "interim base-correlation tranchelet loss floored at zero"
                );
            }
            diff.max(0.0)
        } else {
            self.resolve_tranchelet_difference(
                el_to_detach,
                el_to_attach,
                inv.attach_pct,
                inv.detach_pct,
                date,
            )?
        };
        let tranche_loss_fraction =
            (current_portfolio_loss_fraction * inv.pool_factor) / inv.orig_width;
        let el_fraction = (tranche_loss_fraction + inv.prior_loss).clamp(0.0, 1.0);

        // Senior-side recovery writedown: recovered pool notional G erodes
        // the structure from the top, so the tranche [A, D] (current-pool
        // strikes) loses E[(G − (1−D))⁺] − E[(G − (1−A))⁺] of notional.
        // Using E[(G−s)⁺] = E[G] − E[min(G,s)] this is
        //     E[min(G, 1−A)] − E[min(G, 1−D)],
        // a difference of two capped recovered-notional expectations
        // evaluated with the SAME copula machinery as the loss side. The
        // base correlations are sticky to the original strikes, mirroring
        // the loss decomposition: the (1−A) cap pairs with ρ(A) and the
        // (1−D) cap with ρ(D). Any small negative residual from that
        // correlation mismatch is benign and clamped to zero (the recovery
        // leg has no arbitrage-validation contract).
        let current_portfolio_wd_fraction = (rec_to_attach_cap - rec_to_detach_cap).max(0.0);
        let tranche_wd_fraction =
            (current_portfolio_wd_fraction * inv.pool_factor) / inv.orig_width;
        let wd_fraction = (tranche_wd_fraction + inv.prior_wd).clamp(0.0, 1.0 - el_fraction);

        Ok((el_fraction, wd_fraction))
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
    /// - A negative gap within the sum of both converged integration budgets
    ///   and floating-point subtraction error is clamped to zero.
    /// - A negative gap *beyond* the tolerance is genuine base-correlation
    ///   arbitrage and returns an explicit [`Error::Validation`] naming the
    ///   strikes and the magnitude, so the caller cannot unknowingly price a
    ///   senior tranche with zero protection.
    fn resolve_tranchelet_difference(
        &self,
        el_to_detach: f64,
        el_to_attach: f64,
        attach_pct: f64,
        detach_pct: f64,
        date: Date,
    ) -> Result<f64> {
        let diff = el_to_detach - el_to_attach;
        let error_budget = 2.0 * INTEGRATION_TOLERANCE
            + 16.0 * f64::EPSILON * (el_to_detach.abs() + el_to_attach.abs());
        if !diff.is_finite() {
            return Err(Error::Validation(
                "non-finite base-correlation loss difference".to_owned(),
            ));
        }
        if diff >= -error_budget {
            // Non-negative (up to numerical noise) — clamp the tiny residual.
            return Ok(diff.max(0.0));
        }

        // Genuine base-correlation arbitrage: EL(0,D) < EL(0,A).
        Err(Error::Validation(format!(
            "base-correlation arbitrage at strikes [{attach_pct:.4}%, {detach_pct:.4}%] \
                 on {date:?}: equity EL(0,{detach_pct:.4}%)={el_to_detach:.8} is below \
                 EL(0,{attach_pct:.4}%)={el_to_attach:.8} (gap {diff:.2e}, integration budget {error_budget:.2e}). The base-correlation \
                 curve is not arbitrage-free at these detachment points; pricing this tranche \
                 would assign it negative protection. Recalibrate with nonnegative tranche-loss constraints."
        )))
    }

    /// Build the expected loss AND senior-side recovery-writedown curve for
    /// all payment dates.
    ///
    /// Each point carries the cumulative expected loss fraction (erodes the
    /// tranche from the bottom) and the cumulative expected recovery
    /// writedown fraction (amortizes the detachment from the top); their sum
    /// never exceeds 1. The premium leg accrues on
    /// `1 − el_fraction − wd_fraction`, while only the loss side triggers
    /// protection payments.
    ///
    /// Any computed EL or WD value that is less than the previous date's is
    /// clamped to maintain monotonicity. This prevents small arbitrage
    /// opportunities that can arise from base correlation model
    /// inconsistencies.
    #[cfg(test)]
    pub(super) fn build_el_wd_curve(
        &self,
        tranche: &CdsTranche,
        index_data: &CreditIndexData,
        dates: &[Date],
    ) -> Result<Vec<ElWdPoint>> {
        self.prepare_el_wd_curve(tranche, index_data, dates)?
            .evaluate(self, index_data.base_correlation_curve())
    }

    pub(super) fn prepare_el_wd_curve(
        &self,
        tranche: &CdsTranche,
        index_data: &CreditIndexData,
        dates: &[Date],
    ) -> Result<PreparedExpectedLoss> {
        let inv = self.el_invariants(tranche, index_data)?;
        let pools = if inv.eff_detach <= inv.eff_attach || inv.orig_width <= 1e-9 {
            Vec::new()
        } else {
            dates
                .iter()
                .map(|&date| PreparedPool::new(self, index_data, date))
                .collect::<Result<Vec<_>>>()?
        };
        Ok(PreparedExpectedLoss {
            inv,
            dates: dates.to_vec(),
            pools,
            attachment: None,
        })
    }

    /// Shared engine for capped pool expectations `E[min(Σᵢ wᵢ·eᵢ·Bᵢ, cap)]`
    /// with `eᵢ` selected by `exposure` (loss given default or recovery).
    ///
    /// With [`PoolExposure::Loss`] this is the expected loss of an equity
    /// tranche `[0, K]` under the Gaussian copula, using adaptive integration
    /// for numerical stability near the correlation boundaries (0 and 1),
    /// where the conditional default probability has sharp transitions.
    ///
    /// With [`PoolExposure::Recovery`] it is the expected CAPPED RECOVERED
    /// notional `E[min(G, cap)]`, where `G = Σᵢ wᵢ·Rᵢ·Bᵢ` — same conditional
    /// default distribution, with per-name exposure `Rᵢ` instead of LGD
    /// `1 − Rᵢ`. Used to amortize the detachment point from the top: the
    /// senior writedown of tranche `[A, D]` is `E[min(G, 1−A)] − E[min(G, 1−D)]`.
    ///
    /// # Arguments
    /// * `cap_pct` - Cap (detachment point K for losses) in percent
    /// * `correlation` - Asset correlation parameter ρ
    /// * `index_data` - Credit index market data
    /// * `maturity` - Maturity date for the loss calculation
    /// * `exposure` - Per-name exposure: loss given default or recovery
    pub(super) fn calculate_equity_tranche_capped(
        &self,
        cap_pct: f64,
        correlation: f64,
        index_data: &CreditIndexData,
        maturity: Date,
        exposure: PoolExposure,
    ) -> Result<f64> {
        PreparedPool::new(self, index_data, maturity)?.capped(
            self,
            cap_pct / 100.0,
            self.smooth_correlation_boundary(correlation),
            exposure,
        )
    }

    /// Gaussian marginal identity, applicable only to valid finite pools.
    /// Invalid inputs still reach the checked conditional/integration path, so
    /// eliminating an integral cannot conceal its validation failures.
    pub(super) fn gaussian_cap_identity(
        &self,
        cap: f64,
        count: usize,
        default_prob: f64,
        exposure: f64,
        correlation: f64,
    ) -> Option<f64> {
        if self.copula_spec != CopulaSpec::Gaussian
            || count == 0
            || !default_prob.is_finite()
            || !cap.is_finite()
            || !(0.0..=1.0).contains(&exposure)
            || !(0.0..=1.0).contains(&correlation)
        {
            return None;
        }
        if cap == 0.0 {
            Some(0.0)
        } else if cap >= exposure {
            // The same clipping used to construct the Gaussian threshold.
            // E[Binomial(N, P(default|Z))/N] = effective marginal PD.
            Some(exposure * default_prob.clamp(PROBABILITY_CLIP, 1.0 - PROBABILITY_CLIP))
        } else {
            None
        }
    }

    /// Shared uniform-pool engine with a constant recovery rate.
    #[cfg(test)]
    pub(super) fn homogeneous_capped_expectation(
        &self,
        cap: f64,
        count: usize,
        default_prob: f64,
        base_recovery: f64,
        correlation: f64,
        exposure: PoolExposure,
    ) -> Result<f64> {
        HomogeneousPool::new(self, count, default_prob, base_recovery).capped(
            self,
            cap,
            correlation,
            exposure,
        )
    }
    #[cfg(test)]
    fn homogeneous_capped_pair(
        &self,
        loss_cap: f64,
        recovery_cap: f64,
        count: usize,
        default_prob: f64,
        base_recovery: f64,
        correlation: f64,
    ) -> Result<(f64, f64)> {
        HomogeneousPool::new(self, count, default_prob, base_recovery).pair(
            self,
            loss_cap,
            recovery_cap,
            correlation,
        )
    }
}

pub(super) struct PreparedExpectedLoss {
    inv: ElInvariants,
    dates: Vec<Date>,
    pools: Vec<PreparedPool>,
    // One raw attachment state only. Caps, pool data and copula are immutable;
    // the actual twice-smoothed interpolation result is the remaining key.
    attachment: Option<Mutex<Option<AttachmentExpectations>>>,
}
struct AttachmentExpectations {
    correlation_bits: u64,
    caps: Arc<Vec<(f64, f64)>>,
}
impl PreparedExpectedLoss {
    pub(super) fn enable_attachment_cache(&mut self) {
        self.attachment = Some(Mutex::new(None));
    }

    pub(super) fn evaluate(
        &self,
        pricer: &CdsTranchePricer,
        correlation: &BaseCorrelationCurve,
    ) -> Result<Vec<ElWdPoint>> {
        let inv = self.inv;
        if inv.eff_detach <= inv.eff_attach || inv.orig_width <= 1e-9 {
            return Ok(vec![
                ElWdPoint {
                    el_fraction: 0.0,
                    wd_fraction: 0.0
                };
                self.dates.len()
            ]);
        }
        // Preserve both established smoothing passes, including their rounding
        // near boundaries. Preparation does not change the correlation model.
        let corr_attach = pricer.smooth_correlation_boundary(
            pricer.smooth_correlation_boundary(correlation.correlation(inv.attach_pct)),
        );
        let corr_detach = pricer.smooth_correlation_boundary(
            pricer.smooth_correlation_boundary(correlation.correlation(inv.detach_pct)),
        );
        let correlation_bits = corr_attach.to_bits();
        let cached_attachment = self.attachment.as_ref().and_then(|cache| {
            cache
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_ref()
                .filter(|entry| entry.correlation_bits == correlation_bits)
                .map(|entry| Arc::clone(&entry.caps))
        });
        let mut new_attachment = Vec::with_capacity(
            if self.attachment.is_some() && cached_attachment.is_none() {
                self.dates.len()
            } else {
                0
            },
        );
        let mut curve = Vec::with_capacity(self.dates.len());
        let mut prev_el = 0.0;
        let mut prev_wd = 0.0;
        for (i, (&date, pool)) in self.dates.iter().zip(&self.pools).enumerate() {
            let attachment = if let Some(cached) = &cached_attachment {
                cached[i]
            } else {
                let caps = pool.pair(
                    pricer,
                    (inv.eff_attach * 100.0) / 100.0,
                    ((1.0 - inv.eff_attach) * 100.0) / 100.0,
                    corr_attach,
                )?;
                if self.attachment.is_some() {
                    new_attachment.push(caps);
                }
                caps
            };
            let detachment = pool.pair(
                pricer,
                (inv.eff_detach * 100.0) / 100.0,
                ((1.0 - inv.eff_detach) * 100.0) / 100.0,
                corr_detach,
            )?;
            let (mut el_fraction, mut wd_fraction) =
                pricer.el_wd_fraction_from_caps(&inv, date, [attachment, detachment])?;
            if el_fraction < prev_el - 1e-6 {
                tracing::debug!(
                    "EL decreased from {:.6} to {:.6} at {:?} (delta={:.6}) - enforcing monotonicity",
                    prev_el,
                    el_fraction,
                    date,
                    el_fraction - prev_el
                );
            }
            el_fraction = el_fraction.max(prev_el);
            wd_fraction = wd_fraction.max(prev_wd);
            wd_fraction = wd_fraction.min(1.0 - el_fraction);
            curve.push(ElWdPoint {
                el_fraction,
                wd_fraction,
            });
            prev_el = el_fraction;
            prev_wd = wd_fraction;
        }
        if cached_attachment.is_none()
            && let Some(cache) = &self.attachment
        {
            *cache
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some(AttachmentExpectations {
                    correlation_bits,
                    caps: Arc::new(new_attachment),
                });
        }
        Ok(curve)
    }
}

#[cfg(test)]
mod production_credit_audit {
    use super::*;
    use time::Month;
    #[test]
    fn base_correlation_clamps_only_numerical_budget() {
        let date = Date::from_calendar_date(2030, Month::January, 1).expect("date");
        let tolerance = INTEGRATION_TOLERANCE;
        let pricer = CdsTranchePricer::new();
        assert_eq!(
            pricer
                .resolve_tranchelet_difference(0.01 - tolerance, 0.01, 3.0, 7.0, date)
                .expect("noise"),
            0.0
        );
        let error = pricer
            .resolve_tranchelet_difference(0.01 - 4.0 * tolerance, 0.01, 3.0, 7.0, date)
            .expect_err("materially negative loss");
        assert!(error.to_string().contains("integration budget"));
    }

    #[test]
    fn gaussian_nonbinding_caps_use_clipped_marginal_probability() {
        let pricer = CdsTranchePricer::new();
        for count in [1, 125, 10_000] {
            for pd in [0.0_f64, 1e-8, 0.05, 0.9, 1.0] {
                for recovery in [0.0_f64, 0.4, 1.0] {
                    for rho in [0.01, 0.3, 0.99] {
                        for exposure in [PoolExposure::Loss, PoolExposure::Recovery] {
                            let amount = match exposure {
                                PoolExposure::Loss => 1.0 - recovery,
                                PoolExposure::Recovery => recovery,
                            };
                            let expected =
                                amount * pd.clamp(PROBABILITY_CLIP, 1.0 - PROBABILITY_CLIP);
                            for cap in [amount, amount.next_up(), 1.0] {
                                let actual = pricer
                                    .homogeneous_capped_expectation(
                                        cap, count, pd, recovery, rho, exposure,
                                    )
                                    .expect("nonbinding cap");
                                assert_eq!(actual.to_bits(), expected.to_bits());
                            }
                            assert_eq!(
                                pricer
                                    .homogeneous_capped_expectation(
                                        0.0, count, pd, recovery, rho, exposure
                                    )
                                    .expect("zero cap"),
                                0.0
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn gaussian_cap_seam_preserves_finite_pool_value() {
        let pricer = CdsTranchePricer::new();
        for recovery in [0.0_f64, 0.4, 1.0] {
            for exposure in [PoolExposure::Loss, PoolExposure::Recovery] {
                let amount = match exposure {
                    PoolExposure::Loss => 1.0 - recovery,
                    PoolExposure::Recovery => recovery,
                };
                if amount > 0.0 {
                    let below = pricer
                        .homogeneous_capped_expectation(
                            amount.next_down(),
                            125,
                            0.12,
                            recovery,
                            0.8,
                            exposure,
                        )
                        .expect("binding cap at seam");
                    assert!((below - amount * 0.12).abs() < INTEGRATION_TOLERANCE);
                }
            }
        }
    }

    #[test]
    fn cap_identity_never_bypasses_invalid_correlation_or_non_gaussian_model() {
        let pricer = CdsTranchePricer::new();
        for cap in [0.0, 1.0] {
            assert!(
                pricer
                    .homogeneous_capped_expectation(
                        cap,
                        125,
                        0.1,
                        0.4,
                        f64::NAN,
                        PoolExposure::Loss
                    )
                    .is_err()
            );
            assert_eq!(pricer.gaussian_cap_identity(cap, 0, 0.1, 0.6, 0.3), None);
            assert_eq!(
                pricer.gaussian_cap_identity(cap, 125, 0.1, f64::NAN, 0.3),
                None
            );
            let student = CdsTranchePricer::with_copula(CopulaSpec::StudentT {
                degrees_of_freedom: 6.0,
            })
            .expect("copula");
            assert_eq!(student.gaussian_cap_identity(cap, 125, 0.1, 0.6, 0.3), None);
        }
    }

    #[test]
    fn paired_caps_resolve_each_component_independently() {
        let pricer = CdsTranchePricer::new();
        for (loss_cap, recovery_cap) in [(0.0, 1.0), (0.03, 1.0), (1.0, 0.03), (0.03, 0.03)] {
            let pair = pricer
                .homogeneous_capped_pair(loss_cap, recovery_cap, 125, 0.12, 0.4, 0.3)
                .expect("pair");
            let loss = pricer
                .homogeneous_capped_expectation(loss_cap, 125, 0.12, 0.4, 0.3, PoolExposure::Loss)
                .expect("loss");
            let recovery = pricer
                .homogeneous_capped_expectation(
                    recovery_cap,
                    125,
                    0.12,
                    0.4,
                    0.3,
                    PoolExposure::Recovery,
                )
                .expect("recovery");
            assert_eq!(pair.0.to_bits(), loss.to_bits());
            assert_eq!(pair.1.to_bits(), recovery.to_bits());
        }
    }
}
