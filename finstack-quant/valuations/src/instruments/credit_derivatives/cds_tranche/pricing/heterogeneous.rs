//! Heterogeneous capped loss by conditional convolution on a fixed loss grid.
use super::config::{
    CdsTranchePricer, PoolExposure, CDF_CLIP, GRID_STEP, HOMOGENEITY_TOLERANCE,
    MAX_CONVOLUTION_WORK, MAX_GRID_POINTS, NUMERICAL_TOLERANCE,
};
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::term_structures::CreditIndexData;
use finstack_quant_core::math::norm_cdf;
use finstack_quant_core::{Error, Result};
use std::cell::RefCell;

impl CdsTranchePricer {
    /// Integrate a complete constituent pool with per-issuer recovery rates
    /// by exact conditional convolution at every factor node.
    pub(super) fn calculate_equity_tranche_capped_hetero(
        &self,
        cap_pct: f64,
        correlation: f64,
        index_data: &CreditIndexData,
        maturity: Date,
        exposure: PoolExposure,
    ) -> Result<f64> {
        let curves = index_data.issuer_credit_curves.as_ref().ok_or_else(|| {
            Error::Validation("heterogeneous pricing requires issuer curves".to_owned())
        })?;
        let count = usize::from(index_data.num_constituents);
        if count == 0 || curves.len() != count {
            return Err(Error::Validation(
                "heterogeneous pricing requires complete issuer coverage".to_owned(),
            ));
        }
        let correlation = self.smooth_correlation_boundary(correlation);
        let cap = cap_pct / 100.0;
        let mut probabilities = Vec::with_capacity(count);
        let mut recoveries = Vec::with_capacity(count);
        let mut weights = Vec::with_capacity(count);
        let mut total_weight = 0.0;
        for (id, curve) in curves {
            let weight = index_data.get_issuer_weight(id);
            if !weight.is_finite() || weight < 0.0 {
                return Err(Error::Validation(
                    "issuer weights must be finite and non-negative".to_owned(),
                ));
            }
            total_weight += weight;
            // Names without exposure must not change the loss distribution or
            // the choice between the binomial and heterogeneous algorithms.
            if weight == 0.0 {
                continue;
            }
            let time = curve.day_count().year_fraction(
                curve.base_date(),
                maturity,
                DayCountContext::default(),
            )?;
            probabilities.push((1.0 - curve.sp(time)).clamp(0.0, 1.0));
            recoveries.push(index_data.get_issuer_recovery(id));
            weights.push(weight);
        }
        if (total_weight - 1.0).abs() > 1e-9 {
            return Err(Error::Validation(
                "issuer weights must preserve the full pool notional".to_owned(),
            ));
        }
        let count = weights.len();
        let uniform = |values: &[f64]| {
            values
                .iter()
                .all(|x| (x - values[0]).abs() <= HOMOGENEITY_TOLERANCE)
        };
        if uniform(&probabilities) && uniform(&recoveries) && uniform(&weights) {
            return self.homogeneous_capped_expectation(
                cap,
                count,
                probabilities[0],
                recoveries[0],
                correlation,
                exposure,
            );
        }
        let thresholds: Vec<_> = probabilities
            .iter()
            .map(|p| self.default_threshold_for_copula(*p))
            .collect();
        let conditional_p = |i: usize, factors: &[f64]| {
            if self.copula_spec.is_gaussian() {
                self.conditional_default_probability_enhanced(
                    thresholds[i],
                    correlation,
                    factors[0],
                )
            } else {
                self.conditional_default_prob_copula(
                    self.copula(),
                    thresholds[i],
                    factors,
                    correlation,
                )
            }
        };
        let exposures: Vec<f64> = recoveries
            .iter()
            .map(|&recovery| match exposure {
                PoolExposure::Loss => 1.0 - recovery,
                PoolExposure::Recovery => recovery,
            })
            .collect();
        // The PMF spans every reachable loss, including the mass above the cap.
        // Fractional-bin interpolation can round each name upward. Bound the
        // sum of those rounded supports, not just the rounded aggregate loss,
        // so no reachable probability mass falls off the convolution grid.
        let max_grid_index: f64 = (0..count)
            .map(|i| (weights[i] * exposures[i] / GRID_STEP).ceil())
            .sum();
        if !max_grid_index.is_finite() {
            return Err(Error::Validation(
                "non-finite heterogeneous pool exposure".to_owned(),
            ));
        }
        let max_points = checked_convolution_grid(count, max_grid_index)?;
        let buffers = RefCell::new((vec![0.0; max_points], vec![0.0; max_points]));
        self.integrate_factors(&|factors| {
            let mut buffers = buffers.borrow_mut();
            let (first, second) = &mut *buffers;
            first[0] = 1.0;
            let mut len = 1;
            let mut in_first = true;
            for i in 0..count {
                let p = conditional_p(i, factors);
                len = if in_first {
                    accumulate_issuer_pmf(
                        first,
                        len,
                        second,
                        max_points,
                        weights[i],
                        exposures[i],
                        p,
                    )
                } else {
                    accumulate_issuer_pmf(
                        second,
                        len,
                        first,
                        max_points,
                        weights[i],
                        exposures[i],
                        p,
                    )
                };
                in_first = !in_first;
            }
            Ok(expected_loss_capped(
                if in_first {
                    &first[..len]
                } else {
                    &second[..len]
                },
                cap,
            ))
        })
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
            // Perfect-correlation limit: the latent variable Aᵢ = √ρ·Z +
            // √(1−ρ)·εᵢ → Z, so default (Aᵢ ≤ Φ⁻¹(PD)) becomes the deterministic
            // step 1{Z ≤ Φ⁻¹(PD)}. The previous `Φ(Φ⁻¹(PD) − Z)` was the wrong
            // limit (it dropped the 1/√(1−ρ) divisor).
            //
            // Reachability: `smooth_correlation_boundary` caps ρ at
            // MAX_CORRELATION (0.99), so this branch is currently
            // unreachable; the exact limit is kept for correctness should the cap
            // ever be relaxed. The 0.99 cap is a deliberate numerical-stability
            // choice and means the exact ρ = 1 comonotonic limit is not priced.
            return if market_factor < default_threshold {
                1.0
            } else {
                0.0
            };
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

/// Check conservative PMF storage and work before allocation or convolution.
fn checked_convolution_grid(count: usize, max_grid_index: f64) -> Result<usize> {
    if !max_grid_index.is_finite() || max_grid_index < 0.0 {
        return Err(Error::Validation(
            "non-finite or negative heterogeneous grid extent".to_owned(),
        ));
    }
    if max_grid_index.ceil() >= MAX_GRID_POINTS as f64 {
        return Err(Error::Validation(format!(
            "heterogeneous exact convolution grid is exceeding the limit of {MAX_GRID_POINTS} points"
        )));
    }
    let points = (max_grid_index.ceil() as usize)
        .checked_add(1)
        .ok_or_else(|| Error::Validation("heterogeneous grid size overflow".to_owned()))?;
    let work = count.checked_mul(points).ok_or_else(|| {
        Error::Validation("heterogeneous convolution work estimate overflow".to_owned())
    })?;
    if work > MAX_CONVOLUTION_WORK {
        return Err(Error::Validation(format!(
            "heterogeneous exact convolution requires up to {work} issuer-grid visits per factor evaluation, exceeding the limit of {MAX_CONVOLUTION_WORK}; reduce the positive-weight constituent count"
        )));
    }
    Ok(points)
}

/// Convolve a single issuer's loss contribution into the destination PMF buffer.
///
/// Reads the active prefix `src[..src_len]`, writes the new active prefix into
/// `dst[..returned_len]`, and zeros only what it touches in `dst` so the buffer
/// can be reused without reallocating between issuers.
///
/// `loss_exact = weight * lgd / GRID_STEP` is split into floor + frac bins to
/// preserve fractional loss contributions when the issuer's loss does not align
/// with the grid. Mass conservation: each input mass `m` is distributed as
/// `m*(1-p)` to no-default bin, `m*p*(1-frac)` to floor bin, `m*p*frac` to ceil
/// bin (or floor if ceil is past the grid).
#[inline]
fn accumulate_issuer_pmf(
    src: &[f64],
    src_len: usize,
    dst: &mut [f64],
    max_points: usize,
    weight: f64,
    lgd: f64,
    p: f64,
) -> usize {
    let loss_exact = weight * lgd / GRID_STEP;
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

/// Compute `E[min(L, k)]` from a PMF where bin `i` represents loss `i * GRID_STEP`.
///
/// Uses Neumaier compensated summation to maintain accuracy when the PMF has
/// many bins (up to `MAX_GRID_POINTS`).
#[inline]
fn expected_loss_capped(pmf: &[f64], k: f64) -> f64 {
    finstack_quant_core::math::neumaier_sum(
        pmf.iter()
            .enumerate()
            .map(|(i, &mass)| mass * ((i as f64) * GRID_STEP).min(k)),
    )
}

#[cfg(test)]
mod work_limit_tests {
    use super::*;

    #[test]
    fn convolution_work_limit_checks_boundary_without_running_convolution() {
        // 100 * 50,000 visits is the configured budget; one extra grid point
        // must fail even though both grids fit the independent storage cap.
        assert_eq!(
            checked_convolution_grid(100, 49_998.0).expect("below work limit"),
            49_999
        );
        assert_eq!(
            checked_convolution_grid(100, 49_999.0).expect("at work limit"),
            50_000
        );
        assert!(checked_convolution_grid(100, 50_000.0).is_err());
        assert!(checked_convolution_grid(65_535, 65_535.0).is_err());
    }

    #[test]
    fn convolution_work_estimate_rejects_overflow_and_invalid_extent() {
        assert!(checked_convolution_grid(usize::MAX, 1.0).is_err());
        for extent in [f64::INFINITY, f64::NAN, -1.0, MAX_GRID_POINTS as f64] {
            assert!(checked_convolution_grid(1, extent).is_err());
        }
    }
}
