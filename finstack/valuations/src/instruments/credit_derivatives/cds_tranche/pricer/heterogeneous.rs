use super::config::{
    CDSTranchePricer, HeteroMethod, ADAPTIVE_INTEGRATION_HIGH, ADAPTIVE_INTEGRATION_LOW, CDF_CLIP,
    GRID_STEP_MIN, LGD_FLOOR, MAX_GRID_POINTS, NUMERICAL_TOLERANCE, PROBABILITY_CLIP,
    SPA_VARIANCE_FLOOR,
};
use crate::constants::credit;
use crate::correlation::copula::Copula;
use crate::correlation::recovery::RecoveryModel;
use finstack_core::dates::Date;
use finstack_core::market_data::term_structures::CreditIndexData;
use finstack_core::math::{norm_cdf, norm_pdf};
use finstack_core::{Error, Result};
use tracing::warn;

impl CDSTranchePricer {
    /// Heterogeneous equity tranche loss via semi-analytical SPA or exact convolution fallback.
    ///
    /// Supports full bespoke portfolio heterogeneity:
    /// - Per-issuer hazard curves (default probability)
    /// - Per-issuer recovery rates (LGD)
    /// - Per-issuer weights (notional allocation)
    pub(super) fn calculate_equity_tranche_loss_hetero(
        &self,
        detachment_pct: f64,
        correlation: f64,
        index_data: &CreditIndexData,
        maturity: Date,
    ) -> Result<f64> {
        // Precompute unconditional PD_i(t)
        let t = self.years_from_base(index_data, maturity)?;
        let tranche_width = detachment_pct / 100.0;
        let correlation = self.smooth_correlation_boundary(correlation);

        // Quadrature setup
        let quad = self.select_quadrature()?;

        // Build heterogeneous vectors: PD, LGD, and Weight per issuer
        let mut pd_i: Vec<f64> = Vec::with_capacity(index_data.num_constituents as usize);
        let mut lgd_i: Vec<f64> = Vec::with_capacity(index_data.num_constituents as usize);
        let mut weight_i: Vec<f64> = Vec::with_capacity(index_data.num_constituents as usize);

        if let Some(curves) = &index_data.issuer_credit_curves {
            // Sort issuer IDs for determinism (HashMap iteration order is random)
            let mut sorted_ids: Vec<&str> = curves.keys().map(String::as_str).collect();
            sorted_ids.sort();

            for id in sorted_ids {
                let curve = index_data.get_issuer_curve(id);
                let sp = curve.sp(t);
                pd_i.push((1.0 - sp).clamp(0.0, 1.0));

                let rec = index_data.get_issuer_recovery(id);
                lgd_i.push((1.0 - rec).max(LGD_FLOOR));

                let w = index_data.get_issuer_weight(id);
                weight_i.push(w);
            }
        } else {
            // Fallback to homogeneous (should not happen if caller gates, but ensure safe)
            let sp = index_data.index_credit_curve.sp(t);
            let count = index_data.num_constituents as usize;
            pd_i = vec![(1.0 - sp).clamp(0.0, 1.0); count];
            lgd_i = vec![(1.0 - index_data.recovery_rate).max(LGD_FLOOR); count];
            weight_i = vec![1.0 / count as f64; count];
        }

        // Check if effectively homogeneous (optimization: use faster binomial path)
        let is_uniform_pd = pd_i
            .first()
            .map(|&first| pd_i.iter().all(|&p| (p - first).abs() <= PROBABILITY_CLIP))
            .unwrap_or(true);
        let is_uniform_lgd = lgd_i
            .first()
            .map(|&first| lgd_i.iter().all(|&l| (l - first).abs() <= 1e-9))
            .unwrap_or(true);
        let is_uniform_weight = weight_i
            .first()
            .map(|&first| weight_i.iter().all(|&w| (w - first).abs() <= 1e-9))
            .unwrap_or(true);

        if is_uniform_pd && is_uniform_lgd && is_uniform_weight {
            // Use homogeneous binomial path (faster)
            let num_constituents = index_data.num_constituents as usize;
            let detachment_notional = detachment_pct / 100.0;
            let base_recovery = 1.0 - lgd_i[0];

            // Build recovery model if configured (same as homogeneous path)
            let recovery_model: Option<Box<dyn RecoveryModel>> =
                self.params.recovery_spec.as_ref().map(|spec| spec.build());

            let default_prob = self.get_default_probability(index_data, t)?;
            let default_threshold = self.default_threshold_for_copula(default_prob);

            if self.params.copula_spec.is_gaussian() {
                let integrand = |z: f64| {
                    let p = self.conditional_default_probability_enhanced(
                        default_threshold,
                        correlation,
                        z,
                    );

                    // Use stochastic recovery if configured, otherwise constant
                    let recovery = match &recovery_model {
                        Some(model) => model.conditional_recovery(z),
                        None => base_recovery,
                    };

                    self.conditional_equity_tranche_loss(
                        num_constituents,
                        detachment_notional,
                        p,
                        recovery,
                    )
                };
                let expected_loss = if !(ADAPTIVE_INTEGRATION_LOW..=ADAPTIVE_INTEGRATION_HIGH)
                    .contains(&correlation)
                {
                    quad.integrate_adaptive(integrand, NUMERICAL_TOLERANCE)
                } else {
                    quad.integrate(integrand)
                };
                return Ok(expected_loss);
            }

            let copula_ref = self.copula();
            let expected_loss = copula_ref.integrate_fn(&|factors| {
                let p = self.conditional_default_prob_copula(
                    copula_ref,
                    default_threshold,
                    factors,
                    correlation,
                );
                let z = factors.first().copied().unwrap_or(0.0);
                let recovery = match &recovery_model {
                    Some(model) => model.conditional_recovery(z),
                    None => base_recovery,
                };
                self.conditional_equity_tranche_loss(
                    num_constituents,
                    detachment_notional,
                    p,
                    recovery,
                )
            });
            return Ok(expected_loss);
        }

        let use_gaussian = self.params.copula_spec.is_gaussian();
        let thresholds: Vec<f64> = pd_i
            .iter()
            .map(|&p| self.default_threshold_for_copula(p))
            .collect();

        // Prefer exact convolution for small pools to reduce SPA error
        let n_const = index_data.num_constituents as usize;

        if use_gaussian {
            // Integrand over common factor Z using heterogeneous LGD and weights
            let integrand = |factors: &[f64]| -> f64 {
                let z = factors.first().copied().unwrap_or(0.0);
                let sqrt_rho = correlation.sqrt();
                let sqrt_1mr = (1.0 - correlation).sqrt();
                let mut mean = 0.0;
                let mut var = 0.0;

                for i in 0..thresholds.len() {
                    let th = thresholds[i];
                    let cthr = (th - sqrt_rho * z) / sqrt_1mr;
                    let p = norm_cdf(cthr).clamp(0.0, 1.0);

                    let w = weight_i[i] * lgd_i[i];
                    mean += w * p;
                    var += w * w * p * (1.0 - p);
                }

                // SPA/normal approximation for E[min(L, K)] with K = detachment_notional
                // Formula: E[min(L,K)] = mu*Phi(a) - sigma*phi(a) + K*(1 - Phi(a))
                // Reference: O'Kane (2008) Eq. 9.12, Hull-White (2004)
                let k = tranche_width;
                if var <= SPA_VARIANCE_FLOOR {
                    return mean.min(k);
                }
                let s = var.sqrt();
                let a = (k - mean) / s;
                let phi_a = norm_cdf(a);
                mean * phi_a - s * norm_pdf(a) + k * (1.0 - phi_a)
            };

            let el = if n_const <= credit::SMALL_POOL_THRESHOLD {
                self.hetero_exact_convolution_full(
                    detachment_pct,
                    correlation,
                    &thresholds,
                    &lgd_i,
                    &weight_i,
                )?
            } else {
                match self.params.hetero_method {
                    HeteroMethod::Spa => {
                        warn!(
                            n_constituents = n_const,
                            threshold = credit::SMALL_POOL_THRESHOLD,
                            "CDS tranche using SPA approximation for heterogeneous pool \
                             (pool size {n_const} exceeds exact-convolution threshold {}). \
                             Results are approximate.",
                            credit::SMALL_POOL_THRESHOLD
                        );
                        if !(ADAPTIVE_INTEGRATION_LOW..=ADAPTIVE_INTEGRATION_HIGH)
                            .contains(&correlation)
                        {
                            quad.integrate_adaptive(|z| integrand(&[z]), NUMERICAL_TOLERANCE)
                        } else {
                            quad.integrate(|z| integrand(&[z]))
                        }
                    }
                    HeteroMethod::ExactConvolution => self.hetero_exact_convolution_full(
                        detachment_pct,
                        correlation,
                        &thresholds,
                        &lgd_i,
                        &weight_i,
                    )?,
                }
            };

            return Ok(el);
        }

        let copula_ref = self.copula();

        // Integrand over common factor Z using heterogeneous LGD and weights
        let integrand = |factors: &[f64]| -> f64 {
            let mut mean = 0.0;
            let mut var = 0.0;

            for i in 0..thresholds.len() {
                let p = self.conditional_default_prob_copula(
                    copula_ref,
                    thresholds[i],
                    factors,
                    correlation,
                );

                let w = weight_i[i] * lgd_i[i];
                mean += w * p;
                var += w * w * p * (1.0 - p);
            }

            // SPA/normal approximation for E[min(L, K)] with K = detachment_notional
            let k = tranche_width;
            if var <= SPA_VARIANCE_FLOOR {
                return mean.min(k);
            }
            let s = var.sqrt();
            let a = (k - mean) / s;
            let phi_a = norm_cdf(a);
            mean * phi_a - s * norm_pdf(a) + k * (1.0 - phi_a)
        };

        let el = if n_const <= credit::SMALL_POOL_THRESHOLD {
            self.hetero_exact_convolution_full(
                detachment_pct,
                correlation,
                &thresholds,
                &lgd_i,
                &weight_i,
            )?
        } else {
            match self.params.hetero_method {
                HeteroMethod::Spa => copula_ref.integrate_fn(&integrand),
                HeteroMethod::ExactConvolution => self.hetero_exact_convolution_full(
                    detachment_pct,
                    correlation,
                    &thresholds,
                    &lgd_i,
                    &weight_i,
                )?,
            }
        };

        Ok(el)
    }

    /// Exact convolution with full heterogeneous LGD and weight vectors.
    ///
    /// This is the fully bespoke version that supports per-issuer:
    /// - Hazard rates (via probit thresholds)
    /// - Recovery rates (via lgd_i)
    /// - Notional weights (via weight_i)
    fn hetero_exact_convolution_full(
        &self,
        detachment_pct: f64,
        correlation: f64,
        thresholds: &[f64],
        lgd_i: &[f64],
        weight_i: &[f64],
    ) -> Result<f64> {
        let k = detachment_pct / 100.0;
        let grid_step = self.params.grid_step.max(GRID_STEP_MIN);
        // The convolved portfolio-loss PMF has support up to total LGD
        // (Σ wᵢ·lgdᵢ), which is far beyond the tranche detachment `k` for any
        // non-super-senior tranche. The buffer must span the full reachable
        // loss: `expected_loss_capped` computes `E[min(L,k)]`, whose dominant
        // term for an equity tranche is `k·P(L>k)` — sizing the buffer to `k`
        // alone silently drops that tail mass and biases tranche EL low.
        let total_lgd: f64 = weight_i
            .iter()
            .zip(lgd_i.iter())
            .map(|(&w, &l)| w * l)
            .sum();
        let max_points = (total_lgd / grid_step).ceil() as usize + 2;

        let use_gaussian = self.params.copula_spec.is_gaussian();
        let copula_ref: Option<&dyn Copula> = if use_gaussian {
            None
        } else {
            Some(self.copula())
        };

        if max_points > MAX_GRID_POINTS {
            // Performance guard: fall back to SPA approximation with heterogeneous vectors
            return self.hetero_spa_full(thresholds, correlation, k, lgd_i, weight_i, copula_ref);
        }

        let sqrt_rho = correlation.sqrt();
        let sqrt_1mr = (1.0 - correlation).sqrt();
        let quad = self.select_quadrature()?;

        // The convolution loop allocates two PMF buffers of `max_points` once per
        // integrand evaluation and ping-pongs between them, replacing the
        // previous per-issuer `vec![0.0f64; ...]` (was N×K allocations per
        // quadrature point; now 2). Each `accumulate_issuer_pmf` call zeros only
        // the active prefix of the destination buffer.
        if use_gaussian {
            let integrand = |factors: &[f64]| {
                let z = factors.first().copied().unwrap_or(0.0);
                let mut buf_a = vec![0.0f64; max_points];
                let mut buf_b = vec![0.0f64; max_points];
                buf_a[0] = 1.0;
                let mut pmf_len = 1usize;
                let mut pmf_in_a = true;

                for i in 0..thresholds.len() {
                    let th = thresholds[i];
                    let lgd = lgd_i[i];
                    let weight = weight_i[i];

                    let cthr = (th - sqrt_rho * z) / sqrt_1mr;
                    let p = norm_cdf(cthr).clamp(0.0, 1.0);

                    let new_len = if pmf_in_a {
                        accumulate_issuer_pmf(
                            &buf_a, pmf_len, &mut buf_b, max_points, weight, lgd, grid_step, p,
                        )
                    } else {
                        accumulate_issuer_pmf(
                            &buf_b, pmf_len, &mut buf_a, max_points, weight, lgd, grid_step, p,
                        )
                    };
                    pmf_len = new_len;
                    pmf_in_a = !pmf_in_a;
                }

                let active = if pmf_in_a { &buf_a } else { &buf_b };
                expected_loss_capped(&active[..pmf_len], grid_step, k)
            };

            let value =
                if !(ADAPTIVE_INTEGRATION_LOW..=ADAPTIVE_INTEGRATION_HIGH).contains(&correlation) {
                    quad.integrate_adaptive(|z| integrand(&[z]), NUMERICAL_TOLERANCE)
                } else {
                    quad.integrate(|z| integrand(&[z]))
                };

            return Ok(value);
        }

        let copula_ref = copula_ref.ok_or_else(|| {
            Error::Validation("Copula must be set for non-Gaussian convolution.".to_string())
        })?;
        let integrand = |factors: &[f64]| {
            let mut buf_a = vec![0.0f64; max_points];
            let mut buf_b = vec![0.0f64; max_points];
            buf_a[0] = 1.0;
            let mut pmf_len = 1usize;
            let mut pmf_in_a = true;

            for i in 0..thresholds.len() {
                let th = thresholds[i];
                let lgd = lgd_i[i];
                let weight = weight_i[i];

                let p = self.conditional_default_prob_copula(copula_ref, th, factors, correlation);

                let new_len = if pmf_in_a {
                    accumulate_issuer_pmf(
                        &buf_a, pmf_len, &mut buf_b, max_points, weight, lgd, grid_step, p,
                    )
                } else {
                    accumulate_issuer_pmf(
                        &buf_b, pmf_len, &mut buf_a, max_points, weight, lgd, grid_step, p,
                    )
                };
                pmf_len = new_len;
                pmf_in_a = !pmf_in_a;
            }

            let active = if pmf_in_a { &buf_a } else { &buf_b };
            expected_loss_capped(&active[..pmf_len], grid_step, k)
        };

        Ok(copula_ref.integrate_fn(&integrand))
    }

    /// SPA fallback with full heterogeneous vectors.
    fn hetero_spa_full(
        &self,
        thresholds: &[f64],
        correlation: f64,
        k: f64,
        lgd_i: &[f64],
        weight_i: &[f64],
        copula: Option<&dyn Copula>,
    ) -> Result<f64> {
        let quad = self.select_quadrature()?;
        let use_gaussian = copula.is_none();
        if use_gaussian {
            let integrand = |factors: &[f64]| -> f64 {
                let z = factors.first().copied().unwrap_or(0.0);
                let sqrt_rho = correlation.sqrt();
                let sqrt_1mr = (1.0 - correlation).sqrt();
                let mut mean = 0.0;
                let mut var = 0.0;

                for i in 0..thresholds.len() {
                    let th = thresholds[i];
                    let cthr = (th - sqrt_rho * z) / sqrt_1mr;
                    let p = norm_cdf(cthr).clamp(0.0, 1.0);
                    let w = weight_i[i] * lgd_i[i];
                    mean += w * p;
                    var += w * w * p * (1.0 - p);
                }

                if var <= SPA_VARIANCE_FLOOR {
                    return mean.min(k);
                }
                let s = var.sqrt();
                let a = (k - mean) / s;
                let phi_a = norm_cdf(a);
                mean * phi_a - s * norm_pdf(a) + k * (1.0 - phi_a)
            };

            let value =
                if !(ADAPTIVE_INTEGRATION_LOW..=ADAPTIVE_INTEGRATION_HIGH).contains(&correlation) {
                    quad.integrate_adaptive(|z| integrand(&[z]), NUMERICAL_TOLERANCE)
                } else {
                    quad.integrate(|z| integrand(&[z]))
                };

            return Ok(value);
        }

        let copula_ref = copula.ok_or_else(|| {
            Error::Validation("Copula must be set for non-Gaussian SPA.".to_string())
        })?;
        let integrand = |factors: &[f64]| -> f64 {
            let mut mean = 0.0;
            let mut var = 0.0;

            for i in 0..thresholds.len() {
                let p = self.conditional_default_prob_copula(
                    copula_ref,
                    thresholds[i],
                    factors,
                    correlation,
                );
                let w = weight_i[i] * lgd_i[i];
                mean += w * p;
                var += w * w * p * (1.0 - p);
            }

            if var <= SPA_VARIANCE_FLOOR {
                return mean.min(k);
            }
            let s = var.sqrt();
            let a = (k - mean) / s;
            let phi_a = norm_cdf(a);
            mean * phi_a - s * norm_pdf(a) + k * (1.0 - phi_a)
        };

        Ok(copula_ref.integrate_fn(&integrand))
    }

    /// Calculate conditional default probability given market factor Z.
    ///
    /// Standard implementation kept for compatibility and testing.
    /// The enhanced version `conditional_default_probability_enhanced` is used
    /// in production calculations for superior numerical stability.
    ///
    /// P(default | Z) = Φ((Φ⁻¹(PD) - √ρ * Z) / √(1-ρ))
    #[cfg(test)]
    pub(super) fn conditional_default_probability(
        &self,
        default_threshold: f64,
        correlation: f64,
        market_factor: f64,
    ) -> f64 {
        let sqrt_rho = correlation.sqrt();
        let one_minus_rho: f64 = 1.0 - correlation;
        let sqrt_one_minus_rho = one_minus_rho.sqrt();

        let conditional_threshold =
            (default_threshold - sqrt_rho * market_factor) / sqrt_one_minus_rho;
        norm_cdf(conditional_threshold)
    }

    /// Enhanced conditional default probability with improved numerical stability.
    ///
    /// Provides superior handling of boundary cases and extreme correlation values
    /// through sophisticated boundary transition functions and overflow protection.
    ///
    /// P(default | Z) = Φ((Φ⁻¹(PD) - √ρ * Z) / √(1-ρ))
    pub(super) fn conditional_default_probability_enhanced(
        &self,
        default_threshold: f64,
        correlation: f64,
        market_factor: f64,
    ) -> f64 {
        // Apply smooth correlation boundaries to avoid numerical discontinuities
        let correlation = self.smooth_correlation_boundary(correlation);

        // Handle extreme correlation cases with special care
        if correlation < NUMERICAL_TOLERANCE {
            // Near-zero correlation: independent case
            return norm_cdf(default_threshold);
        }
        if correlation > 1.0 - NUMERICAL_TOLERANCE {
            // Near-perfect correlation: deterministic case
            let threshold_adj = default_threshold - market_factor;
            return norm_cdf(threshold_adj);
        }

        // Enhanced calculation with overflow protection
        let sqrt_rho = correlation.sqrt();
        let one_minus_rho = 1.0 - correlation;

        // Protect against numerical issues when correlation approaches 1
        let sqrt_one_minus_rho = if one_minus_rho < NUMERICAL_TOLERANCE {
            NUMERICAL_TOLERANCE.sqrt() // Minimum practical value to avoid division by zero
        } else {
            let one_minus_rho: f64 = 1.0 - correlation;
            one_minus_rho.sqrt()
        };

        // Calculate conditional threshold with overflow protection
        let numerator = default_threshold - sqrt_rho * market_factor;
        let conditional_threshold = numerator / sqrt_one_minus_rho;

        // Clamp to reasonable range to prevent CDF overflow
        let conditional_threshold = conditional_threshold.clamp(-CDF_CLIP, CDF_CLIP);

        norm_cdf(conditional_threshold)
    }
}

/// Convolve a single issuer's loss contribution into the destination PMF buffer.
///
/// Reads the active prefix `src[..src_len]`, writes the new active prefix into
/// `dst[..returned_len]`, and zeros only what it touches in `dst` so the buffer
/// can be reused without reallocating between issuers.
///
/// `loss_exact = weight * lgd / grid_step` is split into floor + frac bins to
/// preserve fractional loss contributions when the issuer's loss does not align
/// with the grid. Mass conservation: each input mass `m` is distributed as
/// `m*(1-p)` to no-default bin, `m*p*(1-frac)` to floor bin, `m*p*frac` to ceil
/// bin (or floor if ceil is past the grid).
#[inline]
#[allow(clippy::too_many_arguments)] // hot-path numerical helper; grouping into a struct would add allocation
fn accumulate_issuer_pmf(
    src: &[f64],
    src_len: usize,
    dst: &mut [f64],
    max_points: usize,
    weight: f64,
    lgd: f64,
    grid_step: f64,
    p: f64,
) -> usize {
    let loss_exact = weight * lgd / grid_step;
    let loss_floor = loss_exact.floor() as usize;
    let frac = loss_exact - loss_floor as f64;

    let new_len = (src_len + loss_floor + 2).min(max_points).min(dst.len());

    // Zero only the active prefix that we're about to write.
    for slot in dst[..new_len].iter_mut() {
        *slot = 0.0;
    }

    for j in 0..src_len {
        let mass = src[j];
        if mass == 0.0 {
            continue;
        }

        if j < new_len {
            dst[j] += mass * (1.0 - p);
        }

        let j_floor = j + loss_floor;
        let j_ceil = j_floor + 1;

        if j_floor < new_len {
            dst[j_floor] += mass * p * (1.0 - frac);
        }
        if j_ceil < new_len && frac > 0.0 {
            dst[j_ceil] += mass * p * frac;
        } else if j_floor < new_len && frac > 0.0 {
            // Ceil falls off the grid; collapse the fractional piece into floor
            // to preserve total mass.
            dst[j_floor] += mass * p * frac;
        }
    }

    new_len
}

/// Compute `E[min(L, k)]` from a PMF where bin `i` represents loss `i * grid_step`.
///
/// Uses Neumaier compensated summation to maintain accuracy when the PMF has
/// many bins (up to `max_grid_points`, which can be 200K).
#[inline]
fn expected_loss_capped(pmf: &[f64], grid_step: f64, k: f64) -> f64 {
    finstack_core::math::neumaier_sum(
        pmf.iter()
            .enumerate()
            .map(|(i, &mass)| mass * ((i as f64) * grid_step).min(k)),
    )
}
