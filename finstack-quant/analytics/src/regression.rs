//! Full-rank, unweighted least squares with an optional holdings identity.
//!
//! Columns and targets are scaled independently for bounded SVD arithmetic.
//! Holdings constrain the aggregate fitted return; they do not weight the loss.

use finstack_quant_core::math::summation::neumaier_sum;
use finstack_quant_core::{Error, InputError, NonFiniteKind};
use nalgebra::DMatrix;
use num_rational::BigRational;
use num_traits::Zero;

const ACCEPTANCE_TOLERANCE: f64 = 1e-12;

fn calculation(message: &str) -> Error {
    Error::Internal(format!("least-squares calculation failed: {message}"))
}

/// A finite significand and unrestricted binary exponent for intermediate
/// products. This keeps cancellation and unit conversion out of the IEEE
/// overflow/underflow range until the requested result is materialized.
#[derive(Clone, Copy, Debug)]
struct Scaled {
    significand: f64,
    exponent: i32,
}

impl Scaled {
    fn new(value: f64) -> Self {
        if value.abs() <= 0.0 {
            return Self {
                significand: value,
                exponent: 0,
            };
        }
        let mut value = value;
        let mut adjustment = 0;
        if value.abs() < f64::MIN_POSITIVE {
            value *= 4_503_599_627_370_496.0;
            adjustment = -52;
        }
        let bits = value.to_bits();
        let exponent = ((bits >> 52) & 0x7ff) as i32 - 1022;
        let significand = f64::from_bits((bits & !(0x7ff_u64 << 52)) | (1022_u64 << 52));
        Self {
            significand,
            exponent: exponent + adjustment,
        }
    }

    fn normalize(significand: f64, exponent: i32) -> Self {
        let mut result = Self::new(significand);
        if result.significand.abs() > 0.0 {
            result.exponent += exponent;
        }
        result
    }

    fn mul(self, rhs: Self) -> Self {
        Self::normalize(
            self.significand * rhs.significand,
            self.exponent + rhs.exponent,
        )
    }

    fn div(self, rhs: Self) -> Self {
        Self::normalize(
            self.significand / rhs.significand,
            self.exponent - rhs.exponent,
        )
    }

    fn neg(self) -> Self {
        Self {
            significand: -self.significand,
            ..self
        }
    }

    fn abs(self) -> Self {
        Self {
            significand: self.significand.abs(),
            ..self
        }
    }

    fn at_exponent(self, exponent: i32) -> f64 {
        if self.significand.abs() <= 0.0 {
            return self.significand;
        }
        self.significand * 2.0_f64.powi(self.exponent - exponent)
    }

    fn value(self) -> finstack_quant_core::Result<f64> {
        let value = if self.exponent > 1023 {
            (self.significand * 2.0_f64.powi(1023)) * 2.0_f64.powi(self.exponent - 1023)
        } else if self.exponent < -1022 {
            (self.significand * 2.0_f64.powi(self.exponent + 1022)) * f64::MIN_POSITIVE
        } else {
            self.significand * 2.0_f64.powi(self.exponent)
        };
        if !value.is_finite() {
            return Err(calculation(
                "requested finite coefficient is not representable",
            ));
        }
        Ok(value)
    }
}

fn scaled_sum(values: impl IntoIterator<Item = Scaled>) -> Scaled {
    let values: Vec<Scaled> = values
        .into_iter()
        .filter(|v| v.significand.abs() > 0.0)
        .collect();
    let exponent = values.iter().map(|v| v.exponent).max().unwrap_or(0);
    let mut lower = Vec::new();
    let sum = neumaier_sum(values.iter().filter_map(|v| {
        let normalized = v.at_exponent(exponent);
        if normalized.abs() <= 0.0 {
            lower.push(*v);
            None
        } else {
            Some(normalized)
        }
    }));
    let result = Scaled::normalize(sum, exponent);
    // Preserve terms below the current floating-point range when larger
    // terms cancel. Rebase those terms after cancellation, rather than
    // silently discarding the newly representable requested result.
    if !lower.is_empty() && (result.significand.abs() <= 0.0 || result.exponent < exponent) {
        lower.push(result);
        return scaled_sum(lower);
    }
    result
}

fn scaled_dot(lhs: &[f64], rhs: &[Scaled]) -> Scaled {
    scaled_sum(lhs.iter().zip(rhs).map(|(&a, &b)| Scaled::new(a).mul(b)))
}

fn norm(values: &[f64]) -> f64 {
    let scale = values.iter().map(|v| v.abs()).fold(0.0, f64::max);
    if scale <= 0.0 {
        return 0.0;
    }
    scale * neumaier_sum(values.iter().map(|v| (v / scale).powi(2))).sqrt()
}

fn common_values(groups: &[&[Scaled]]) -> Vec<Vec<f64>> {
    let exponent = groups
        .iter()
        .flat_map(|g| g.iter())
        .filter(|v| v.significand.abs() > 0.0)
        .map(|v| v.exponent)
        .max()
        .unwrap_or(0);
    groups
        .iter()
        .map(|g| g.iter().map(|v| v.at_exponent(exponent)).collect())
        .collect()
}

struct ConstraintStep<'a> {
    weights: &'a [f64],
    unit: &'a [f64],
    h_norm: Scaled,
}

/// One column-normalized bounded decomposition, reusable for every target.
struct NormalizedSvd {
    original_design: DMatrix<f64>,
    design: DMatrix<f64>,
    scales: Vec<(f64, f64)>,
    u: DMatrix<f64>,
    vt: DMatrix<f64>,
    singular_values: Vec<f64>,
}

impl NormalizedSvd {
    fn new(mut design: DMatrix<f64>) -> finstack_quant_core::Result<Self> {
        let (n, p) = design.shape();
        if p == 0 || n < p {
            return Err(InputError::DimensionMismatch.into());
        }
        ensure_finite(design.as_slice())?;
        let original_design = design.clone();
        let mut scales = Vec::with_capacity(p);
        for mut column in design.column_iter_mut() {
            let maximum = column.iter().map(|v| v.abs()).fold(0.0, f64::max);
            if maximum <= 0.0 {
                return Err(Error::Validation(
                    "design matrix has an all-zero column".into(),
                ));
            }
            let binary_scale = f64::from_bits(maximum.to_bits() & (0x7ff_u64 << 52));
            let scale = if binary_scale > 0.0 {
                binary_scale
            } else {
                maximum
            };
            let column_norm = neumaier_sum(column.iter().map(|v| (v / scale).powi(2))).sqrt();
            // Retain both factors for subnormal norms too: rounding the
            // combined norm first can substantially change the unit column.
            scales.push((scale, column_norm));
            for value in column.iter_mut() {
                *value = *value / scale / column_norm;
            }
        }
        let svd = design
            .clone()
            .try_svd(true, true, 5.0 * f64::EPSILON, 1000)
            .ok_or_else(|| calculation("bounded SVD did not converge"))?;
        ensure_finite(svd.singular_values.as_slice())
            .map_err(|_| calculation("nonfinite singular values"))?;
        if svd.singular_values.iter().any(|&value| value < 0.0) {
            return Err(calculation("negative singular value"));
        }
        let maximum = svd.singular_values.iter().copied().fold(0.0, f64::max);
        let tolerance = f64::EPSILON * n.max(p) as f64 * maximum;
        let rank = svd
            .singular_values
            .iter()
            .filter(|&&value| value > tolerance)
            .count();
        if rank != p {
            return Err(Error::Validation(format!(
                "design matrix is rank deficient (rank {rank} < {p})"
            )));
        }
        let u = svd
            .u
            .ok_or_else(|| calculation("missing left singular vectors"))?;
        let vt = svd
            .v_t
            .ok_or_else(|| calculation("missing right singular vectors"))?;
        ensure_finite(u.as_slice()).map_err(|_| calculation("nonfinite left singular vectors"))?;
        ensure_finite(vt.as_slice())
            .map_err(|_| calculation("nonfinite right singular vectors"))?;
        Ok(Self {
            original_design,
            design,
            scales,
            u,
            vt,
            singular_values: svd.singular_values.iter().copied().collect(),
        })
    }

    fn projection_coordinates(&self, target: &[Scaled]) -> Vec<Scaled> {
        self.u
            .column_iter()
            .map(|column| scaled_dot(column.as_slice(), target))
            .collect()
    }

    fn resolved_projection_coordinates(&self, target: &[Scaled]) -> Vec<Scaled> {
        // Reconstruct U' target from Z and the same SVD. This retains a
        // small held-row projection that may have only absolute (rather
        // than relative) accuracy in the stored left singular vectors.
        let cross: Vec<Scaled> = self
            .original_design
            .column_iter()
            .enumerate()
            .map(|(j, column)| {
                scaled_dot(column.as_slice(), target)
                    .div(Scaled::new(self.scales[j].0))
                    .div(Scaled::new(self.scales[j].1))
            })
            .collect();
        (0..self.vt.nrows())
            .map(|i| {
                scaled_sum((0..self.vt.ncols()).map(|j| Scaled::new(self.vt[(i, j)]).mul(cross[j])))
                    .div(Scaled::new(self.singular_values[i]))
            })
            .collect()
    }

    fn projected_values(&self, coordinates: &[Scaled]) -> Vec<Scaled> {
        let normalized: Vec<Scaled> = (0..self.vt.ncols())
            .map(|i| {
                scaled_sum((0..self.vt.nrows()).map(|j| {
                    Scaled::new(self.vt[(j, i)])
                        .mul(coordinates[j])
                        .div(Scaled::new(self.singular_values[j]))
                }))
            })
            .collect();
        (0..self.design.nrows())
            .map(|i| {
                scaled_sum(
                    (0..self.design.ncols())
                        .map(|j| Scaled::new(self.design[(i, j)]).mul(normalized[j])),
                )
            })
            .collect()
    }

    fn refined_coordinates(&self, target: &[Scaled]) -> Vec<Scaled> {
        let mut coordinates = self.projection_coordinates(target);
        // Rebuild residuals from the design, then refine through the same
        // bounded SVD. This recovers small components obscured by absolute
        // roundoff in stored singular vectors without changing the model.
        for _ in 0..2 {
            let fitted = self.projected_values(&coordinates);
            let residual: Vec<Scaled> = target
                .iter()
                .zip(fitted)
                .map(|(&r, y)| scaled_sum([r, y.neg()]))
                .collect();
            let correction = self.projection_coordinates(&residual);
            coordinates = coordinates
                .into_iter()
                .zip(correction)
                .map(|(a, b)| scaled_sum([a, b]))
                .collect();
        }
        coordinates
    }

    fn coefficients(
        &self,
        coordinates: &[Scaled],
    ) -> finstack_quant_core::Result<(Vec<f64>, bool)> {
        let mut underflowed = false;
        let values = (0..self.vt.ncols())
            .map(|i| {
                let value = scaled_sum((0..self.vt.nrows()).map(|j| {
                    Scaled::new(self.vt[(j, i)])
                        .mul(coordinates[j])
                        .div(Scaled::new(self.singular_values[j]))
                }));
                let requested = value
                    .div(Scaled::new(self.scales[i].0))
                    .div(Scaled::new(self.scales[i].1));
                let coefficient = requested.value()?;
                underflowed |= coefficient.abs() <= 0.0 && requested.significand.abs() > 0.0;
                Ok(coefficient)
            })
            .collect::<finstack_quant_core::Result<_>>()?;
        Ok((values, underflowed))
    }

    fn refine_rounded_coefficients(
        &self,
        target: &[Scaled],
        mut coefficients: Vec<f64>,
        constraint: Option<&ConstraintStep<'_>>,
    ) -> finstack_quant_core::Result<Vec<f64>> {
        for _ in 0..2 {
            let fitted = self.fitted(&coefficients);
            let residual: Vec<Scaled> = target
                .iter()
                .zip(fitted)
                .map(|(&r, y)| scaled_sum([r, y.neg()]))
                .collect();
            if residual.iter().all(|v| v.significand.abs() <= 0.0) {
                break;
            }
            let mut coordinates = self.refined_coordinates(&residual);
            if let Some(step) = constraint {
                let step_y0 = self.projected_values(&coordinates);
                let (_, step_delta) = equality(step.weights, &residual, &step_y0);
                let step_mag = step_delta.div(step.h_norm);
                coordinates = coordinates
                    .iter()
                    .zip(step.unit)
                    .map(|(&a, &u)| scaled_sum([a, Scaled::new(u).mul(step_mag)]))
                    .collect();
            }
            let (correction, _) = self.coefficients(&coordinates)?;
            if correction.iter().all(|&c| c.abs() <= 0.0) {
                break;
            }
            for (value, change) in coefficients.iter_mut().zip(correction) {
                *value = scaled_sum([Scaled::new(*value), Scaled::new(change)]).value()?;
            }
        }
        Ok(coefficients)
    }

    fn check_underflow(
        &self,
        underflowed: bool,
        returns: &[f64],
        coefficients: &[f64],
        fitted: &[Scaled],
        active_weights: Option<&[f64]>,
    ) -> finstack_quant_core::Result<()> {
        // A coefficient that is mathematically zero can acquire a tiny SVD
        // rounding residue even on noisy overdetermined or active equality
        // fits. Full column rank plus per-column KKT stationarity in the
        // original design certifies that no nonzero loading collapsed.
        if !underflowed {
            return Ok(());
        }
        let residual: Vec<Scaled> = returns
            .iter()
            .zip(fitted)
            .map(|(&r, &y)| scaled_sum([Scaled::new(r), y.neg()]))
            .collect();
        if residual.iter().all(|v| v.significand.abs() <= 0.0) {
            return Ok(());
        }
        let p = self.original_design.ncols();
        let mut grad = Vec::with_capacity(p);
        let mut fit_scale = Vec::with_capacity(p);
        for j in 0..p {
            let column = self.original_design.column(j);
            let s = Scaled::new(self.scales[j].0);
            let v = Scaled::new(self.scales[j].1);
            grad.push(scaled_dot(column.as_slice(), &residual).div(s).div(v));
            let gross = scaled_sum(
                column
                    .iter()
                    .zip(fitted)
                    .map(|(&x, &y)| Scaled::new(x.abs()).mul(y.abs())),
            );
            fit_scale.push(gross.div(s).div(v));
        }
        let stationarity: Vec<(Scaled, Scaled)> = if let Some(weights) = active_weights {
            let holdings: Vec<Scaled> = weights.iter().copied().map(Scaled::new).collect();
            let b: Vec<Scaled> = (0..p)
                .map(|j| {
                    let column = self.original_design.column(j);
                    scaled_dot(column.as_slice(), &holdings)
                        .div(Scaled::new(self.scales[j].0))
                        .div(Scaled::new(self.scales[j].1))
                })
                .collect();
            let b_norm_sq = scaled_sum(b.iter().map(|&v| v.mul(v)));
            if b_norm_sq.significand.abs() <= 0.0 {
                return Err(calculation("nonzero holdings projection is unresolved"));
            }
            let b_dot_grad = scaled_sum(b.iter().zip(&grad).map(|(&bj, &gj)| bj.mul(gj)));
            let lambda = b_dot_grad.div(b_norm_sq);
            grad.into_iter()
                .zip(fit_scale)
                .zip(b)
                .map(|((gj, sj), bj)| {
                    let lagrange = lambda.mul(bj);
                    (
                        scaled_sum([gj, lagrange.neg()]),
                        scaled_sum([sj, lagrange.abs()]),
                    )
                })
                .collect()
        } else {
            grad.into_iter().zip(fit_scale).collect()
        };
        for (j, (k_j, scale_j)) in stationarity.into_iter().enumerate() {
            if scale_j.significand.abs() <= 0.0 {
                if k_j.significand.abs() > 0.0
                    && !self.exact_column_stationary(j, returns, coefficients, active_weights)?
                {
                    return Err(calculation("requested finite coefficient underflowed"));
                }
            } else {
                let pair = common_values(&[&[k_j.abs()], &[scale_j]]);
                if pair[0][0] > ACCEPTANCE_TOLERANCE * pair[1][0] {
                    return Err(calculation("requested finite coefficient underflowed"));
                }
            }
        }
        Ok(())
    }

    fn exact_column_stationary(
        &self,
        column_idx: usize,
        returns: &[f64],
        coefficients: &[f64],
        active_weights: Option<&[f64]>,
    ) -> finstack_quant_core::Result<bool> {
        let nrows = self.original_design.nrows();
        let ncols = self.original_design.ncols();
        let coeff_q: Vec<BigRational> = coefficients
            .iter()
            .map(|&c| crate::exact::rational(c))
            .collect::<finstack_quant_core::Result<_>>()?;
        let mut residual_q = Vec::with_capacity(nrows);
        for (i, &ret) in returns.iter().enumerate().take(nrows) {
            let mut err = crate::exact::rational(ret)?;
            for (m, cm) in coeff_q.iter().enumerate() {
                let x_q = crate::exact::rational(self.original_design[(i, m)])?;
                err -= &x_q * cm;
            }
            residual_q.push(err);
        }
        let col_grad = |col: usize| -> finstack_quant_core::Result<BigRational> {
            let mut sum = BigRational::zero();
            for (i, err) in residual_q.iter().enumerate() {
                let x_q = crate::exact::rational(self.original_design[(i, col)])?;
                sum += &x_q * err;
            }
            Ok(sum)
        };
        let grad_j = col_grad(column_idx)?;
        let Some(weights) = active_weights else {
            return Ok(grad_j.is_zero());
        };
        let weights_q: Vec<BigRational> = weights
            .iter()
            .map(|&w| crate::exact::rational(w))
            .collect::<finstack_quant_core::Result<_>>()?;
        if ncols == 1 {
            let mut hold_err = BigRational::zero();
            for (w, err) in weights_q.iter().zip(&residual_q) {
                hold_err += w * err;
            }
            return Ok(hold_err.is_zero());
        }
        let col_hold = |col: usize| -> finstack_quant_core::Result<BigRational> {
            let mut sum = BigRational::zero();
            for (i, w) in weights_q.iter().enumerate() {
                let x_q = crate::exact::rational(self.original_design[(i, col)])?;
                sum += &x_q * w;
            }
            Ok(sum)
        };
        let hold_j = col_hold(column_idx)?;
        for m in 0..ncols {
            if m == column_idx {
                continue;
            }
            let hold_m = col_hold(m)?;
            let grad_m = col_grad(m)?;
            if &grad_j * &hold_m != &grad_m * &hold_j {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn fitted(&self, coefficients: &[f64]) -> Vec<Scaled> {
        (0..self.original_design.nrows())
            .map(|i| {
                scaled_sum((0..self.original_design.ncols()).map(|j| {
                    Scaled::new(self.original_design[(i, j)]).mul(Scaled::new(coefficients[j]))
                }))
            })
            .collect()
    }

    fn check_ols(&self, target: &[Scaled], fitted: &[Scaled]) -> finstack_quant_core::Result<()> {
        let values = common_values(&[target, fitted]);
        let residual: Vec<Scaled> = values[0]
            .iter()
            .zip(&values[1])
            .map(|(&r, &y)| Scaled::new(r - y))
            .collect();
        let projected: Vec<f64> = self
            .projection_coordinates(&residual)
            .into_iter()
            .map(Scaled::value)
            .collect::<finstack_quant_core::Result<_>>()?;
        if norm(&projected) > ACCEPTANCE_TOLERANCE * (norm(&values[0]) + norm(&values[1])) {
            return Err(calculation(
                "final OLS coefficients fail projection optimality",
            ));
        }
        Ok(())
    }
}

/// Coefficients and fitted values from the same normalized full-rank solve.
pub(crate) struct LeastSquaresFit {
    /// Coefficients in the original design/target units.
    pub coefficients: DMatrix<f64>,
    /// Fitted responses divided by their corresponding target scale.
    pub fitted: DMatrix<f64>,
    /// Maximum absolute target values, or one for a zero target.
    pub target_scales: Vec<f64>,
}

/// Solve equal-weight full-rank OLS and retain normalized fitted responses.
pub(crate) fn normalized_svd_fit(
    design: DMatrix<f64>,
    targets: &DMatrix<f64>,
) -> finstack_quant_core::Result<LeastSquaresFit> {
    if targets.nrows() != design.nrows() || targets.ncols() == 0 {
        return Err(InputError::DimensionMismatch.into());
    }
    ensure_finite(targets.as_slice())?;
    let model = NormalizedSvd::new(design)?;
    let mut coefficients = DMatrix::zeros(model.design.ncols(), targets.ncols());
    let mut fitted = DMatrix::zeros(targets.nrows(), targets.ncols());
    let mut target_scales = Vec::with_capacity(targets.ncols());
    for j in 0..targets.ncols() {
        let column = targets.column(j);
        let target: Vec<Scaled> = column.iter().copied().map(Scaled::new).collect();
        let coordinates = model.refined_coordinates(&target);
        let (solution, underflowed) = model.coefficients(&coordinates)?;
        let solution = if underflowed {
            model.refine_rounded_coefficients(&target, solution, None)?
        } else {
            solution
        };
        let actual_fitted = model.fitted(&solution);
        model.check_underflow(
            underflowed,
            column.as_slice(),
            &solution,
            &actual_fitted,
            None,
        )?;
        model.check_ols(&target, &actual_fitted)?;
        let scale = column.iter().map(|v| v.abs()).fold(0.0, f64::max);
        let scale = if scale <= 0.0 { 1.0 } else { scale };
        target_scales.push(scale);
        for (i, value) in solution.into_iter().enumerate() {
            coefficients[(i, j)] = value;
        }
        for (i, value) in actual_fitted.into_iter().enumerate() {
            fitted[(i, j)] = value.div(Scaled::new(scale)).value()?;
        }
    }
    Ok(LeastSquaresFit {
        coefficients,
        fitted,
        target_scales,
    })
}

fn equality(weights: &[f64], target: &[Scaled], fitted: &[Scaled]) -> (bool, Scaled) {
    let realized: Vec<Scaled> = weights
        .iter()
        .zip(target)
        .map(|(&w, &r)| Scaled::new(w).mul(r))
        .collect();
    let predicted: Vec<Scaled> = weights
        .iter()
        .zip(fitted)
        .map(|(&w, &y)| Scaled::new(w).mul(y))
        .collect();
    let common = common_values(&[&realized, &predicted]);
    let mismatch = neumaier_sum(common[0].iter().zip(&common[1]).map(|(&r, &y)| r - y));
    let gross = neumaier_sum(
        common[0]
            .iter()
            .zip(&common[1])
            .map(|(&r, &y)| r.abs() + y.abs()),
    );
    let delta = scaled_sum(
        realized
            .into_iter()
            .chain(predicted.into_iter().map(Scaled::neg)),
    );
    (mismatch.abs() <= ACCEPTANCE_TOLERANCE * gross, delta)
}

/// Fit unweighted residual least squares subject to `weights' X f = weights' returns`.
///
/// Signed, unnormalized and zero holdings are supported. No intercept is
/// inserted; zero-holding assets still participate in the unweighted loss.
///
/// # Arguments
///
/// * `exposures` - Finite row-major `returns.len() × n_factors` design matrix;
///   callers supply any intercept explicitly and own exposure units/order.
/// * `n_factors` - Positive independent factor-column count, at most the asset count.
/// * `returns` - Finite aligned observation-period response per asset, in common units.
/// * `weights` - Finite signed holdings per asset, defining the one aggregate identity;
///   these are not residual-loss weights and need not sum to one.
///
/// # Errors
///
/// Invalid dimensions, nonfinite observations, deficient design or a genuinely
/// infeasible equality are input/model errors. Bounded decomposition exhaustion,
/// unresolved arithmetic, unrepresentable coefficients or failed final weighted
/// equality/projection-optimality checks are calculation errors.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_analytics::regression::constrained_least_squares;
/// let fit = constrained_least_squares(
///     &[0.0,1.0,1.0,0.0,0.0,1.0], 2,
///     &[0.05,0.02,0.01], &[0.6,0.3,0.1],
/// )?;
/// assert!((0.3*fit[0]+0.7*fit[1]-0.037).abs() < 1e-12);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn constrained_least_squares(
    exposures: &[f64],
    n_factors: usize,
    returns: &[f64],
    weights: &[f64],
) -> finstack_quant_core::Result<Vec<f64>> {
    let n = returns.len();
    if n == 0 || n_factors == 0 {
        return Err(Error::Validation(
            "constrained least squares needs at least one asset and one factor".into(),
        ));
    }
    if n.checked_mul(n_factors) != Some(exposures.len()) || weights.len() != n {
        return Err(InputError::DimensionMismatch.into());
    }
    ensure_finite(exposures)?;
    ensure_finite(returns)?;
    ensure_finite(weights)?;
    let model = NormalizedSvd::new(DMatrix::from_row_slice(n, n_factors, exposures))?;
    let target: Vec<Scaled> = returns.iter().copied().map(Scaled::new).collect();
    let coordinates = model.refined_coordinates(&target);
    let y0 = model.projected_values(&coordinates);
    model.check_ols(&target, &y0)?;
    let (satisfied, delta) = equality(weights, &target, &y0);
    let holdings: Vec<Scaled> = weights.iter().copied().map(Scaled::new).collect();
    let direction = model.resolved_projection_coordinates(&holdings);
    let normalized = common_values(&[&direction]);
    let direction_norm = norm(&normalized[0]);
    let mut corrected = coordinates.clone();
    let mut active_constraint: Option<(Vec<f64>, Scaled)> = None;
    if !satisfied {
        // Exact compensated X'w detects a genuinely orthogonal holdings
        // vector without turning a merely small projection into infeasibility.
        let orthogonal = model
            .original_design
            .column_iter()
            .all(|column| scaled_dot(column.as_slice(), &holdings).significand.abs() <= 0.0);
        if orthogonal {
            return Err(InputError::ConstraintUnreachable { denominator: 0.0 }.into());
        }
        if direction_norm <= 0.0 {
            return Err(calculation("nonzero holdings projection is unresolved"));
        }
        let largest_exponent = direction
            .iter()
            .filter(|v| v.significand.abs() > 0.0)
            .map(|v| v.exponent)
            .max()
            .unwrap_or(0);
        let h_norm = Scaled::normalize(direction_norm, largest_exponent);
        let magnitude = delta.div(h_norm);
        let unit: Vec<f64> = normalized[0].iter().map(|v| v / direction_norm).collect();
        corrected = coordinates
            .iter()
            .zip(&unit)
            .map(|(&a, &u)| scaled_sum([a, Scaled::new(u).mul(magnitude)]))
            .collect();
        active_constraint = Some((unit, h_norm));
    }
    let (coefficients, underflowed) = model.coefficients(&corrected)?;
    let constraint_step = active_constraint
        .as_ref()
        .map(|(unit, h_norm)| ConstraintStep {
            weights,
            unit,
            h_norm: *h_norm,
        });
    let coefficients = if underflowed {
        model.refine_rounded_coefficients(&target, coefficients, constraint_step.as_ref())?
    } else {
        coefficients
    };
    let fitted = model.fitted(&coefficients);
    let active_weights = active_constraint.as_ref().map(|_| weights);
    model.check_underflow(underflowed, returns, &coefficients, &fitted, active_weights)?;
    if !equality(weights, &target, &fitted).0 {
        return Err(calculation(
            "final coefficients fail weighted-gross equality",
        ));
    }
    let common = common_values(&[&target, &fitted, &y0]);
    let change: Vec<f64> = common[1]
        .iter()
        .zip(&common[2])
        .map(|(&y, &base)| y - base)
        .collect();
    let check = if let Some((unit, _)) = active_constraint {
        let change_scaled: Vec<Scaled> = change.iter().copied().map(Scaled::new).collect();
        let coordinates = model.projection_coordinates(&change_scaled);
        let coordinates: Vec<f64> = coordinates
            .into_iter()
            .map(Scaled::value)
            .collect::<finstack_quant_core::Result<_>>()?;
        let along = neumaier_sum(unit.iter().zip(&coordinates).map(|(&a, &b)| a * b));
        let projected: Vec<f64> = model
            .projected_values(
                &unit
                    .iter()
                    .map(|&u| Scaled::new(u * along))
                    .collect::<Vec<_>>(),
            )
            .into_iter()
            .map(Scaled::value)
            .collect::<finstack_quant_core::Result<_>>()?;
        norm(
            &change
                .iter()
                .zip(&projected)
                .map(|(&v, &p)| v - p)
                .collect::<Vec<_>>(),
        )
    } else {
        model.check_ols(&target, &fitted)?;
        norm(&change)
    };
    let bound = ACCEPTANCE_TOLERANCE * (norm(&common[0]) + norm(&common[1]) + norm(&common[2]));
    if check > bound {
        return Err(calculation(
            "final constrained coefficients fail projection optimality",
        ));
    }
    Ok(coefficients)
}

/// Returns an input error if any supplied value is NaN or infinite.
///
/// # Arguments
///
/// * `values` - Input floating-point observations required to be finite.
pub(crate) fn ensure_finite(values: &[f64]) -> finstack_quant_core::Result<()> {
    for &value in values {
        if !value.is_finite() {
            let kind = if value.is_nan() {
                NonFiniteKind::NaN
            } else if value.is_sign_positive() {
                NonFiniteKind::PosInfinity
            } else {
                NonFiniteKind::NegInfinity
            };
            return Err(InputError::NonFiniteValue { kind }.into());
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::constrained_least_squares;

    #[test]
    fn subnormal_identity_retains_exact_zero_intercept() {
        use nalgebra::DMatrix;
        let x = [1e-320, 2e-320, 3e-320, 4e-320];
        let design = DMatrix::from_fn(4, 2, |i, j| if j == 0 { 1.0 } else { x[i] });
        let target = DMatrix::from_column_slice(4, 1, &x);
        let fit = super::normalized_svd_fit(design, &target).unwrap();
        assert_eq!(fit.coefficients[(0, 0)], 0.0);
        assert_eq!(fit.coefficients[(1, 0)], 1.0);
        let exposures: Vec<f64> = x.into_iter().flat_map(|value| [1.0, value]).collect();
        for weights in [[0.0; 4], [1.0; 4]] {
            let constrained = constrained_least_squares(&exposures, 2, &x, &weights).unwrap();
            assert_eq!(constrained, vec![0.0, 1.0]);
        }
    }

    #[test]
    fn genuinely_unrepresentable_coefficient_is_not_silently_zero() {
        use nalgebra::DMatrix;
        assert!(
            super::normalized_svd_fit(
                DMatrix::from_element(1, 1, 1e308),
                &DMatrix::from_element(1, 1, 1e-320)
            )
            .is_err()
        );
    }

    #[test]
    fn overdetermined_noisy_ols_retains_zero_loading_across_extreme_scales() {
        use nalgebra::DMatrix;
        let x = [1e-320, 2e-320, 3e-320, 4e-320];
        let design = DMatrix::from_fn(4, 2, |i, j| if j == 0 { 1.0 } else { x[i] });
        let target = DMatrix::from_column_slice(4, 1, &[2e-320, 1e-320, 2e-320, 5e-320]);
        let fit = super::normalized_svd_fit(design, &target).unwrap();
        assert_eq!(fit.coefficients[(0, 0)], 0.0);
        assert!((fit.coefficients[(1, 0)] - 1.0).abs() < 1e-12);

        let large_design = DMatrix::from_row_slice(3, 2, &[1.0, 1e308, 1.0, -1e308, 1.0, 0.0]);
        let ordinary_target = DMatrix::from_column_slice(3, 1, &[1.1, 1.1, 0.8]);
        let large_fit = super::normalized_svd_fit(large_design, &ordinary_target).unwrap();
        assert!((large_fit.coefficients[(0, 0)] - 1.0).abs() < 1e-12);
        assert_eq!(large_fit.coefficients[(1, 0)], 0.0);
    }

    #[test]
    fn active_constraint_with_zero_loading_preserves_equality() {
        let f_subnormal = constrained_least_squares(
            &[1.0, 1e-320, 1.0, 2e-320, 1.0, 3e-320],
            2,
            &[1e-320, 4e-320, 2e-320],
            &[1.0, 0.0, 0.0],
        )
        .unwrap();
        assert_eq!(f_subnormal[0], 0.0);
        assert!((f_subnormal[1] - 1.0).abs() < 1e-12);

        let f_large = constrained_least_squares(
            &[1e308, 1.0, 1e308, 2.0, 1e308, 3.0],
            2,
            &[0.01, 0.04, 0.02],
            &[1.0, 0.0, 0.0],
        )
        .unwrap();
        assert_eq!(f_large[0], 0.0);
        assert!((f_large[1] - 0.01).abs() < 1e-14);
        assert!((1e308 * f_large[0] + f_large[1] - 0.01).abs() < 1e-14);

        let f_unit = constrained_least_squares(
            &[1e308, 1.0, 1e308, 2.0, 1e308, 3.0],
            2,
            &[1.0, 4.0, 2.0],
            &[1.0, 0.0, 0.0],
        )
        .unwrap();
        assert!((1e308 * f_unit[0]).abs() < 1e-12);
        assert!((f_unit[1] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn decoupled_and_cancelled_genuine_underflow_still_fail_closed() {
        use nalgebra::DMatrix;
        let err_decoupled = super::normalized_svd_fit(
            DMatrix::from_row_slice(2, 2, &[1.0, 0.0, 0.0, 1e308]),
            &DMatrix::from_column_slice(2, 1, &[1.0, 1e-320]),
        )
        .err()
        .expect("decoupled subnormal loading on 1e308 column must fail closed");
        assert!(
            err_decoupled.to_string().contains("underflowed"),
            "{err_decoupled}"
        );

        let err_cancelled = super::normalized_svd_fit(
            DMatrix::from_element(3, 1, 1e308),
            &DMatrix::from_column_slice(3, 1, &[1.0, -1.0, 1e-320]),
        )
        .err()
        .expect("residual surviving 1e308 cancellation must fail closed");
        assert!(
            err_cancelled.to_string().contains("underflowed"),
            "{err_cancelled}"
        );

        let err_cls = constrained_least_squares(
            &[1e-305, 0.0, 1e-305, 0.0, 0.0, 1e308],
            2,
            &[1e-305, 3e-305, 1e-320],
            &[1.0, 0.0, 0.0],
        )
        .unwrap_err();
        assert!(err_cls.to_string().contains("underflowed"), "{err_cls}");
    }

    #[test]
    fn lower_exponent_terms_survive_large_cancellation() {
        let value = super::scaled_sum([1e308, -1e308, 1e-320].into_iter().map(super::Scaled::new))
            .value()
            .unwrap();
        assert_eq!(value, 1e-320);
    }

    #[test]
    fn original_design_retains_tiny_held_exposure_after_column_scaling() {
        let fit = constrained_least_squares(&[1e308, 1e-308], 1, &[0.0, 1.0], &[0.0, 1.0]).unwrap();
        assert!((fit[0] / 1e308 - 1.0).abs() < 1e-12);
    }

    #[test]
    fn unheld_asset_cannot_loosen_weighted_gross_equality() {
        let coefficients = constrained_least_squares(&[1e-14, 1.0], 1, &[2e-14, 1.0], &[1.0, 0.0])
            .expect("tiny but resolved held exposure defines a feasible equality");
        assert!((coefficients[0] - 2.0).abs() < 1e-12);
    }

    #[test]
    fn zero_return_avoids_unrequested_auxiliary_overflow() {
        assert_eq!(
            constrained_least_squares(&[1e-320], 1, &[0.0], &[1.0]).unwrap(),
            vec![0.0]
        );
    }

    #[test]
    fn subnormal_design_and_response_recover_finite_coefficients() {
        let fit = constrained_least_squares(&[1e-320], 1, &[1e-320], &[1.0]).unwrap();
        assert!((fit[0] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn roundoff_rank_gate_keeps_resolvable_near_collinearity() {
        use nalgebra::DMatrix;
        let design = DMatrix::from_row_slice(3, 2, &[1.0, 1.0, 0.0, 1e-12, 0.0, 0.0]);
        let target = DMatrix::from_column_slice(3, 1, &[2.0, 1e-12, 0.0]);
        let fit = super::normalized_svd_fit(design, &target)
            .unwrap()
            .coefficients;
        assert!((fit[(0, 0)] - 1.0).abs() < 1e-3, "{}", fit[(0, 0)]);
        assert!((fit[(1, 0)] - 1.0).abs() < 1e-3);
    }

    #[test]
    fn shared_solve_rejects_nonfinite_entries_and_underdetermined_models() {
        use nalgebra::DMatrix;
        let finite = DMatrix::from_element(1, 1, 1.0);
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(super::normalized_svd_fit(DMatrix::from_element(1, 1, bad), &finite).is_err());
            assert!(
                super::normalized_svd_fit(finite.clone(), &DMatrix::from_element(1, 1, bad))
                    .is_err()
            );
        }
        assert!(super::normalized_svd_fit(DMatrix::from_element(1, 2, 1.0), &finite).is_err());
    }

    #[test]
    fn binary_factor_case_matches_hand_closed_form() {
        // Paper Exhibit 1 data. X binary: A=(0,1), B=(1,0), C=(0,1); r=(0.05,0.02,0.01);
        // w = benchmark holdings (0.6,0.3,0.1).
        // f̂=(0.02,0.03); g=(0.3,0.35)/… hand: g=(0.003,0.0035)·100 — in decimals g=(0.3,0.35)
        // scaled: X'y=(0.3,0.7), X'X=diag(1,2) ⇒ g=(0.3,0.35); λ=(w'ε̂)/(w'Xg)=0.01/0.335·…
        // In decimals: w'ε̂=0.010, w'Xg=0.335 ⇒ λ=0.0298507463.
        // f = (0.02 + 0.3λ, 0.03 + 0.35λ) = (0.0289552239, 0.0404477612).
        let x = [0.0, 1.0, 1.0, 0.0, 0.0, 1.0];
        let f = constrained_least_squares(&x, 2, &[0.05, 0.02, 0.01], &[0.6, 0.3, 0.1]).unwrap();
        assert!((f[0] - 0.0289552239).abs() < 1e-9);
        assert!((f[1] - 0.0404477612).abs() < 1e-9);
    }

    #[test]
    fn continuous_case_satisfies_constraint_and_matches_hand_derivation() {
        // Paper Exhibit 6 exposures; returns/holdings in decimals.
        // Hand-derived (this plan, NOT the paper's unreproducible Exhibit 8):
        // f ≈ (0.04695653, 0.01130477).
        let x = [1.2, -0.8, 0.5, 1.2, -0.7, 0.7];
        let r = [0.05, 0.02, 0.01];
        let w = [0.6, 0.3, 0.1];
        let f = constrained_least_squares(&x, 2, &r, &w).unwrap();
        assert!((f[0] - 0.04695653).abs() < 1e-6);
        assert!((f[1] - 0.01130477).abs() < 1e-6);
        // Exact identity: w'Xf == w'r
        let wxf = w[0] * (x[0] * f[0] + x[1] * f[1])
            + w[1] * (x[2] * f[0] + x[3] * f[1])
            + w[2] * (x[4] * f[0] + x[5] * f[1]);
        let wr = 0.6 * 0.05 + 0.3 * 0.02 + 0.1 * 0.01;
        assert!((wxf - wr).abs() < 1e-12);
    }

    #[test]
    fn orthogonal_weights_fail_closed() {
        // X = [1, −1] (2 assets × 1 factor), w = (0.5, 0.5) ⇒ w'Xg = 0 ⇒ error.
        let err =
            constrained_least_squares(&[1.0, -1.0], 1, &[0.02, 0.01], &[0.5, 0.5]).unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("constraint"),
            "{err}"
        );
    }

    #[test]
    fn orthogonal_weights_return_ols_when_constraint_is_already_met() {
        let f = constrained_least_squares(&[1.0, -1.0], 1, &[0.02, -0.02], &[0.5, 0.5]).unwrap();
        assert!((f[0] - 0.02).abs() < 1e-15);
    }

    #[test]
    fn extreme_returns_do_not_overflow_satisfaction_tolerance() {
        let err = constrained_least_squares(&[1.0, -1.0], 1, &[1.0e308, 1.0e308], &[0.5, 0.5])
            .unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("constraint"),
            "{err}"
        );
    }

    #[test]
    fn coefficient_rescaling_overflow_fails_closed() {
        let err =
            constrained_least_squares(&[1.0e-154, 2.0e-154], 1, &[1.0e308, 1.0e308], &[1.0, 1.0])
                .expect_err("finite inputs must not produce an infinite rescaled coefficient");
        assert!(err.to_string().to_lowercase().contains("finite"), "{err}");
    }

    #[test]
    fn normalized_weights_avoid_spurious_lagrange_multiplier_overflow() {
        let coefficients =
            constrained_least_squares(&[1.0, 0.0], 1, &[0.0, 1.0e200], &[1.0e-150, 1.0e-150])
                .expect("the constrained coefficient is finite despite tiny holdings");
        assert!((coefficients[0] / 1.0e200 - 1.0).abs() < 1.0e-12);
    }

    #[test]
    fn uniform_weight_scaling_preserves_reachable_and_unreachable_constraints() {
        for scale in [
            -1.0e308, -1.0, -1.0e-320, 1.0e-320, 1.0e-200, 1.0, 100.0, 1.0e200, 1.0e308,
        ] {
            let weights = [scale, scale];
            let coefficients = constrained_least_squares(&[1.0, 0.0], 1, &[0.0, 1.0], &weights)
                .unwrap_or_else(|e| panic!("uniform scale {scale}: {e}"));
            assert!((coefficients[0] - 1.0).abs() < 1.0e-12);
            assert!(constrained_least_squares(&[1.0, -1.0], 1, &[0.02, 0.01], &weights).is_err());
        }
    }

    #[test]
    fn exposure_normalization_supports_extreme_finite_column_scales() {
        for scale in [1.0e-200, 1.0e200] {
            let coefficients =
                constrained_least_squares(&[scale, 0.0], 1, &[0.0, 1.0], &[1.0, 1.0]).expect(
                    "a representable coefficient must not fail because its column norm overflows",
                );
            assert!((coefficients[0] * scale - 1.0).abs() < 1.0e-12);
        }
    }

    #[test]
    fn final_coefficient_overflow_fails_closed() {
        let err = constrained_least_squares(&[1.0e-100, 0.0], 1, &[0.0, 1.0e308], &[1.0, 1.0])
            .expect_err("finite inputs must not produce an infinite corrected coefficient");
        assert!(err.to_string().to_lowercase().contains("finite"), "{err}");
    }

    #[test]
    fn valid_extreme_scale_coefficients_remain_supported() {
        let coefficients = constrained_least_squares(&[1.0e-100, 0.0], 1, &[0.0, 1.0], &[1.0, 1.0])
            .expect("a large but finite constrained coefficient remains valid");
        assert!(coefficients[0].is_finite());
        assert!((coefficients[0] - 1.0e100).abs() / 1.0e100 < 1.0e-12);
    }

    #[test]
    fn overflowing_exposure_dimensions_fail_closed() {
        let err =
            constrained_least_squares(&[], usize::MAX, &[0.02, 0.01], &[0.5, 0.5]).unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("dimension"),
            "{err}"
        );
    }

    #[test]
    fn dimension_mismatch_on_exposures_fails_closed() {
        // exposures has 5 entries but n_assets(3) * n_factors(2) = 6.
        let err = constrained_least_squares(
            &[0.0, 1.0, 1.0, 0.0, 0.0],
            2,
            &[0.05, 0.02, 0.01],
            &[0.6, 0.3, 0.1],
        )
        .unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("dimension"),
            "{err}"
        );
    }

    #[test]
    fn dimension_mismatch_on_weights_fails_closed() {
        let x = [0.0, 1.0, 1.0, 0.0, 0.0, 1.0];
        let err = constrained_least_squares(&x, 2, &[0.05, 0.02, 0.01], &[0.6, 0.4]).unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("dimension"),
            "{err}"
        );
    }

    #[test]
    fn empty_assets_fails_closed() {
        // n_assets = returns.len() = 0 with n_factors = 2: exposures.len()
        // (0) and weights.len() (0) both match n_assets * n_factors / n_assets
        // trivially, so this trips the dedicated `n_assets == 0` guard, not a
        // dimension-mismatch check.
        let err = constrained_least_squares(&[], 2, &[], &[]).unwrap_err();
        assert!(
            err.to_string()
                .contains("at least one asset and one factor"),
            "{err}"
        );
    }

    #[test]
    fn zero_factors_fail_closed() {
        let err = constrained_least_squares(&[], 0, &[0.02], &[1.0]).unwrap_err();
        assert!(
            err.to_string()
                .contains("at least one asset and one factor"),
            "{err}"
        );
    }

    /// Two factor columns in an exact `2x` proportion (perfectly collinear)
    /// make the 3-asset x 2-factor design matrix rank-deficient: after
    /// column normalization both columns are numerically identical, so the
    /// underlying normalized SVD solve sees only one
    /// non-negligible singular value for two factor columns and must fail
    /// closed rather than silently returning an arbitrary least-norm split
    /// between the two collinear factors.
    #[test]
    fn rank_deficient_exposures_fail_closed() {
        let x = [1.0, 2.0, 2.0, 4.0, 3.0, 6.0]; // factor 2 = 2 * factor 1
        let r = [0.05, 0.02, 0.01];
        let w = [0.6, 0.3, 0.1];
        let err = constrained_least_squares(&x, 2, &r, &w).unwrap_err();
        assert!(err.to_string().contains("rank deficient"), "{err}");
    }

    #[test]
    fn non_finite_return_fails_closed() {
        let x = [0.0, 1.0, 1.0, 0.0, 0.0, 1.0];
        let err = constrained_least_squares(&x, 2, &[f64::NAN, 0.02, 0.01], &[0.6, 0.3, 0.1])
            .unwrap_err();
        assert!(err.to_string().to_lowercase().contains("finite"), "{err}");
    }

    #[test]
    fn non_finite_exposures_and_weights_fail_closed_with_finite_value_error() {
        // Parametrized over {exposures, weights} x {NaN, +Inf, -Inf}. Each
        // combination must be rejected by `ensure_finite` itself (message
        // mentions "finite"), not merely by some downstream guard that
        // happens to also reject it under a different, less specific error
        // (e.g. "Invalid input data" or "constraint ... degenerate").
        let good_x = [0.0, 1.0, 1.0, 0.0, 0.0, 1.0];
        let good_r = [0.05, 0.02, 0.01];
        let good_w = [0.6, 0.3, 0.1];

        for &bad in &[f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut x = good_x;
            x[0] = bad;
            let err = constrained_least_squares(&x, 2, &good_r, &good_w).unwrap_err();
            assert!(
                err.to_string().to_lowercase().contains("finite"),
                "exposures[0] = {bad}: {err}"
            );

            let mut w = good_w;
            w[0] = bad;
            let err = constrained_least_squares(&good_x, 2, &good_r, &w).unwrap_err();
            assert!(
                err.to_string().to_lowercase().contains("finite"),
                "weights[0] = {bad}: {err}"
            );
        }
    }

    #[test]
    fn near_degenerate_boundary_is_scale_invariant() {
        // X = [1, -1] (2 assets x 1 factor); w = c * (0.5, 0.5 + delta).
        // The Cauchy-Schwarz ratio |w'Xg| / (||w|| ||Xg||) that the
        // degeneracy guard tests depends only on `delta`, not on the
        // overall scale `c` (both the numerator and denominator scale as
        // c^2 under w -> c*w, so c cancels exactly). A caller supplying
        // weights as percent (c = 100) rather than decimal (c = 1) must
        // therefore see the same accept/reject outcome for the same delta.
        let x = [1.0, -1.0];
        let r = [0.02, 0.01];

        let classify = |delta: f64, c: f64| -> bool {
            let w = [c * 0.5, c * (0.5 + delta)];
            constrained_least_squares(&x, 1, &r, &w).is_err()
        };

        // Both directions are nonzero and resolved. There is no separate
        // magnitude/cosine floor on feasible constraints.
        assert!(!classify(1e-12, 1.0));
        assert!(!classify(1e-12, 100.0));
        assert!(!classify(1e-8, 1.0), "delta=1e-8, c=1 should NOT error");
        assert!(!classify(1e-8, 100.0), "delta=1e-8, c=100 should NOT error");
    }
}
