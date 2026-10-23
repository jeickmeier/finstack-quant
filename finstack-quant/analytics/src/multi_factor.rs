//! Multi-factor OLS regression: [`multi_factor_greeks`] with its
//! [`ReturnKind`] input and [`MultiFactorResult`] output.
//!
//! [`ReturnKind`] and [`MultiFactorResult`] are re-exported at the crate root.
//! [`multi_factor_greeks`] is crate-internal and reached through
//! `Performance::multi_factor_greeks`; the fit solves through
//! `crate::regression::normalized_svd_least_squares`.

use crate::regression::normalized_svd_least_squares;
use finstack_quant_core::math::stats::OnlineStats;
use finstack_quant_core::wire::NonFiniteFields;
use nalgebra::DMatrix;

/// How the dependent return series is interpreted in a multi-factor regression.
///
/// Factor series are always treated as already-excess (Fama–French style).
/// Only the dependent series is adjusted when [`ReturnKind::Total`] is used.
/// The default, used by the bindings when the kind is omitted, is
/// [`ReturnKind::Excess`].
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ReturnKind {
    /// `returns` are already excess returns. Alpha is the annualized OLS
    /// intercept of excess `y` on the supplied (already-excess) factors.
    #[default]
    Excess,
    /// `returns` are total returns. The geometrically decompounded period
    /// risk-free rate is subtracted from `y` only, then OLS is run.
    /// Alpha is the annualized intercept and is Jensen-style for the
    /// dependent variable.
    Total {
        /// Annualized risk-free rate in decimal form (e.g. `0.02` for 2%).
        risk_free_rate: f64,
    },
}

impl ReturnKind {
    /// Build a return kind from its label and the caller's risk-free rate.
    ///
    /// # Arguments
    ///
    /// * `label` - `"excess"` or `"total"`; `None` selects the default,
    ///   [`ReturnKind::Excess`].
    /// * `risk_free_rate` - Annualized risk-free rate in decimal form
    ///   (`0.02` for 2%). [`ReturnKind::Total`] geometrically decompounds it
    ///   to the observation frequency and subtracts it from the dependent
    ///   series. [`ReturnKind::Excess`] has no use for a rate, so it must be
    ///   `0.0`.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] for an unknown label, or
    /// for a non-zero `risk_free_rate` with [`ReturnKind::Excess`].
    pub fn from_label(
        label: Option<&str>,
        risk_free_rate: f64,
    ) -> finstack_quant_core::Result<Self> {
        match label {
            None | Some("excess") if risk_free_rate == 0.0 => Ok(Self::Excess),
            None | Some("excess") => Err(finstack_quant_core::Error::Validation(format!(
                "risk_free_rate {risk_free_rate} has no effect on excess returns; \
                 pass return_kind \"total\" or leave risk_free_rate at 0.0"
            ))),
            Some("total") => Ok(Self::Total { risk_free_rate }),
            Some(other) => Err(finstack_quant_core::Error::Validation(format!(
                "unknown return_kind {other:?}; expected \"excess\" or \"total\""
            ))),
        }
    }
}

/// Result of a multi-factor regression.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MultiFactorResult {
    /// Annualized OLS intercept of the (possibly rf-adjusted) dependent series.
    pub alpha: f64,
    /// Regression coefficients, one per factor.
    pub betas: Vec<f64>,
    /// Fraction of variance explained; NaN for a constant dependent series.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::NonFiniteF64Wire")
    )]
    pub r_squared: f64,
    /// Adjusted R-squared; NaN when R-squared or residual degrees of freedom are undefined.
    ///
    /// ```text
    /// adj_R² = 1 − (1 − R²) × (n − 1) / (n − k − 1)
    /// ```
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::NonFiniteF64Wire")
    )]
    pub adjusted_r_squared: f64,
    /// Annualized residual volatility.
    pub residual_vol: f64,
    /// Number of observations the regression used: the common length of the
    /// dependent series and every factor series. Residual degrees of freedom
    /// are `n_obs − betas.len() − 1`. Required in the serialized result.
    pub n_obs: usize,
}

impl NonFiniteFields for MultiFactorResult {
    const NON_FINITE_FIELDS: &'static [&'static str] = &["r_squared", "adjusted_r_squared"];
}

/// Multi-factor OLS regression of portfolio returns on factor returns.
///
/// Estimates the linear model
///
/// ```text
/// y = α + β₁f₁ + β₂f₂ + ... + βₖfₖ + ε
/// ```
///
/// by solving the least-squares system with an SVD of the (column-normalized)
/// design matrix. SVD avoids explicitly forming the normal equations and is
/// numerically robust to correlated factors and rank deficiency.
///
/// Factor series are treated as already-excess. [`ReturnKind::Total`]
/// subtracts the geometrically decompounded period risk-free rate from the
/// **dependent** series only. [`ReturnKind::Excess`] leaves `y` unchanged;
/// the annualized intercept is then the alpha of already-excess `y`, not a
/// Jensen adjustment of total returns.
///
/// # Arguments
///
/// * `returns`     - Portfolio simple-return series in decimal form (for
///   example, `0.01` for 1%). Interpreted as excess or total according to
///   `return_kind`.
/// * `factors`     - Slice of factor return series (each inner slice is one
///   factor's return series, all the same length as `returns`). Factors are
///   already-excess returns in the same decimal convention.
/// * `ann_factor`  - Number of observation periods per year used to
///   annualize the intercept and residual volatility (e.g. `252.0` daily).
/// * `return_kind` - Whether `returns` are already excess or total. Total
///   subtracts `(1 + rf)^{1/N} − 1` from `y` only.
///
/// # Returns
///
/// A [`MultiFactorResult`] containing:
///
/// - `alpha`: annualized OLS intercept of the (possibly rf-adjusted) `y`
/// - `betas`: one loading per factor
/// - `r_squared` and `adjusted_r_squared`: goodness-of-fit measures
/// - `residual_vol`: annualized residual volatility
///
/// Coefficients are estimated on the overlapping sample implied by the input
/// slices. The function does not truncate mismatched factor lengths; it rejects
/// them as invalid input instead.
///
/// # Errors
///
/// Returns an error when:
///
/// - [`ReturnKind::Total`] has a non-finite risk-free rate
/// - no factors are supplied
/// - there are too few observations for the requested number of factors
/// - any factor return is non-finite
/// - any factor length differs from `returns.len()`
/// - the factor matrix is singular or numerically rank deficient
/// - a coefficient, annualized intercept, or residual volatility is non-finite
///
/// # References
///
/// - Fama & French (1993): see docs/REFERENCES.md#fama-french-1993
/// - Higham: see docs/REFERENCES.md#higham-accuracy-and-stability `docs/REFERENCES.md#higham-accuracy-and-stability`
#[tracing::instrument(level = "debug", skip(returns, factors), fields(n = returns.len(), k = factors.len(), ann_factor = ann_factor))]
pub(crate) fn multi_factor_greeks(
    returns: &[f64],
    factors: &[&[f64]],
    ann_factor: f64,
    return_kind: ReturnKind,
) -> finstack_quant_core::Result<MultiFactorResult> {
    let adjusted_y;
    let y = match return_kind {
        ReturnKind::Excess => returns,
        ReturnKind::Total { risk_free_rate } => {
            let rf_period = crate::returns::periodic_risk_free_rate(risk_free_rate, ann_factor);
            if !rf_period.is_finite() {
                return Err(finstack_quant_core::Error::Validation(
                    "periodic risk-free rate must be finite".to_string(),
                ));
            }
            adjusted_y = returns.iter().map(|r| r - rf_period).collect::<Vec<_>>();
            adjusted_y.as_slice()
        }
    };

    let n = y.len();
    let k = factors.len();
    let p = k + 1; // intercept + k factors

    if n < p + 1 || k == 0 {
        tracing::debug!(
            n,
            k,
            min_observations = p + 1,
            reason = "insufficient_observations",
            "multi-factor greeks rejected input"
        );
        return Err(finstack_quant_core::Error::Validation(format!(
            "multi-factor greeks need at least {} observations, got {n}",
            p + 1
        )));
    }
    if factors
        .iter()
        .any(|factor| factor.iter().any(|v| !v.is_finite()))
    {
        tracing::debug!(
            n,
            k,
            reason = "non_finite_factors",
            "multi-factor greeks rejected input"
        );
        return Err(finstack_quant_core::Error::Validation(
            "factor returns must be finite".to_string(),
        ));
    }
    if factors.iter().any(|factor| factor.len() != n) {
        tracing::debug!(
            n,
            k,
            reason = "factor_length_mismatch",
            "multi-factor greeks rejected input"
        );
        return Err(finstack_quant_core::InputError::DimensionMismatch.into());
    }
    let design = DMatrix::from_fn(
        n,
        p,
        |row, col| {
            if col == 0 {
                1.0
            } else {
                factors[col - 1][row]
            }
        },
    );
    let targets = DMatrix::from_column_slice(n, 1, y);
    let solutions = normalized_svd_least_squares(design, &targets)?;
    let beta = solutions.column(0).iter().copied().collect::<Vec<_>>();

    let alpha_per_period = beta[0];
    let factor_betas: Vec<f64> = beta[1..].to_vec();

    // Both goodness-of-fit sums use the same response scale, which cancels
    // from their ratio. Squaring residuals in original units can overflow
    // even when the resulting standard deviation is representable.
    let response_scale = y.iter().map(|value| value.abs()).fold(0.0, f64::max);
    let response_scale = if response_scale > 0.0 {
        response_scale
    } else {
        1.0
    };
    let mut response_stats = OnlineStats::new();
    let mut ss_res = 0.0_f64;
    for (t, &r) in y.iter().enumerate().take(n) {
        let mut y_hat = alpha_per_period;
        for j in 0..k {
            let fj = factors[j][t];
            y_hat += factor_betas[j] * fj;
        }
        crate::regression::ensure_finite(&[y_hat])?;
        let scaled_response = r / response_scale;
        let residual = scaled_response - y_hat / response_scale;
        ss_res += residual * residual;
        response_stats.update(scaled_response);
    }

    // Online variance preserves exact zero for constant observations, even
    // when summing and dividing their level would round the mean.
    let ss_tot = response_stats.variance() * (n - 1) as f64;
    let r_sq = if ss_tot > 0.0 {
        1.0 - ss_res / ss_tot
    } else {
        f64::NAN
    };
    let dof = (n - k - 1) as f64;
    let residual_var = ss_res / dof;
    let residual_vol = residual_var.sqrt() * response_scale * ann_factor.sqrt();
    let alpha = alpha_per_period * ann_factor;
    crate::regression::ensure_finite(&[alpha, residual_vol])?;

    let adjusted_r_squared = if r_sq.is_finite() {
        1.0 - (1.0 - r_sq) * (n as f64 - 1.0) / dof
    } else {
        f64::NAN
    };
    if ss_tot > 0.0 {
        crate::regression::ensure_finite(&[r_sq, adjusted_r_squared])?;
    }

    Ok(MultiFactorResult {
        alpha,
        betas: factor_betas,
        r_squared: r_sq,
        adjusted_r_squared,
        residual_vol,
        n_obs: n,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::Date;

    #[test]
    fn return_kind_defaults_to_excess() {
        assert_eq!(ReturnKind::default(), ReturnKind::Excess);
        assert_eq!(
            ReturnKind::from_label(None, 0.0).expect("default"),
            ReturnKind::Excess
        );
        assert_eq!(
            ReturnKind::from_label(Some("total"), 0.02).expect("total"),
            ReturnKind::Total {
                risk_free_rate: 0.02
            }
        );
        assert!(ReturnKind::from_label(Some("excess"), 0.02).is_err());
        assert!(ReturnKind::from_label(Some("jensen"), 0.0).is_err());
    }

    #[test]
    fn multi_factor_single_factor() {
        // y = 2*x → alpha ≈ 0, beta ≈ 2, R² ≈ 1.
        let y = [0.02, 0.04, 0.06, 0.08, 0.10];
        let f1 = [0.01, 0.02, 0.03, 0.04, 0.05];
        let result = multi_factor_greeks(&y, &[&f1], 252.0, ReturnKind::Excess)
            .expect("single-factor regression");
        assert!((result.betas[0] - 2.0).abs() < 1e-8);
        assert!(result.r_squared > 0.999);
    }

    #[test]
    fn multi_factor_two_factors() {
        // y ≈ 1.5*f1 + 0.5*f2 (non-collinear factors).
        let f1 = [0.01, 0.02, 0.03, 0.04, 0.05];
        let f2 = [0.03, -0.01, 0.02, 0.01, -0.02];
        let y: Vec<f64> = (0..5).map(|i| 1.5 * f1[i] + 0.5 * f2[i]).collect();
        let result = multi_factor_greeks(&y, &[&f1, &f2], 252.0, ReturnKind::Excess)
            .expect("two-factor regression");
        assert!(result.r_squared > 0.99);
        assert_eq!(result.betas.len(), 2);
        assert!((result.betas[0] - 1.5).abs() < 1e-6);
        assert!((result.betas[1] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn multi_factor_empty_errors() {
        let result = multi_factor_greeks(&[], &[&[]], 252.0, ReturnKind::Excess);
        assert!(result.is_err());
    }

    #[test]
    fn multi_factor_mismatched_factor_lengths_error() {
        let y = [0.02, 0.04, 0.06, 0.08, 0.10];
        let f1 = [0.01, 0.02, 0.03];
        let result = multi_factor_greeks(&y, &[&f1], 252.0, ReturnKind::Excess);
        assert!(result.is_err());
    }

    #[test]
    fn multi_factor_adjusted_r_squared() {
        // y = 2*x → R²≈1, adj_R² should also be close to 1
        let y = [0.02, 0.04, 0.06, 0.08, 0.10];
        let f1 = [0.01, 0.02, 0.03, 0.04, 0.05];
        let result = multi_factor_greeks(&y, &[&f1], 252.0, ReturnKind::Excess)
            .expect("adjusted r-squared regression");
        assert!(result.adjusted_r_squared > 0.99);
        assert!(result.adjusted_r_squared <= result.r_squared);
    }

    #[test]
    fn multi_factor_total_zero_rf_matches_excess() {
        let y = [0.02, 0.04, 0.06, 0.08, 0.10];
        let f1 = [0.01, 0.02, 0.03, 0.04, 0.05];
        let excess =
            multi_factor_greeks(&y, &[&f1], 252.0, ReturnKind::Excess).expect("excess regression");
        let total_zero = multi_factor_greeks(
            &y,
            &[&f1],
            252.0,
            ReturnKind::Total {
                risk_free_rate: 0.0,
            },
        )
        .expect("total zero-rf regression");
        assert!((excess.alpha - total_zero.alpha).abs() < 1e-12);
        assert!((excess.betas[0] - total_zero.betas[0]).abs() < 1e-12);
        assert!((excess.r_squared - total_zero.r_squared).abs() < 1e-12);
        assert!((excess.residual_vol - total_zero.residual_vol).abs() < 1e-12);
    }

    #[test]
    fn multi_factor_total_matches_manual_excess_subtraction() {
        let y = [0.02, 0.03, 0.01, 0.04, 0.00];
        let f1 = [0.01, 0.02, -0.01, 0.03, 0.00];
        let rf_annual = 0.12;
        let rf_period = crate::returns::periodic_risk_free_rate(rf_annual, 252.0);
        let y_ex: Vec<f64> = y.iter().map(|r| r - rf_period).collect();
        let total = multi_factor_greeks(
            &y,
            &[&f1],
            252.0,
            ReturnKind::Total {
                risk_free_rate: rf_annual,
            },
        )
        .expect("total rf regression");
        let excess = multi_factor_greeks(&y_ex, &[&f1], 252.0, ReturnKind::Excess)
            .expect("manual excess regression");
        assert!((total.alpha - excess.alpha).abs() < 1e-12);
        assert!((total.betas[0] - excess.betas[0]).abs() < 1e-12);
        assert!((total.r_squared - excess.r_squared).abs() < 1e-12);
        assert!((total.residual_vol - excess.residual_vol).abs() < 1e-12);
    }

    #[test]
    fn standalone_multi_factor_greeks_errors_on_singular_factor_matrix() {
        let returns = [0.02, 0.04, 0.06, 0.08, 0.10];
        let factor_a = [0.01, 0.02, 0.03, 0.04, 0.05];
        let factor_b = [0.02, 0.04, 0.06, 0.08, 0.10];

        let result =
            multi_factor_greeks(&returns, &[&factor_a, &factor_b], 252.0, ReturnKind::Excess);
        assert!(result.is_err());
    }

    #[test]
    fn performance_multi_factor_greeks_errors_on_invalid_factor_input() {
        use crate::performance::Performance;
        use finstack_quant_core::dates::PeriodKind;
        use time::Month;

        let jan =
            |day: u8| Date::from_calendar_date(2024, Month::January, day).expect("valid date");
        let dates = vec![jan(1), jan(2), jan(3), jan(4), jan(5), jan(6)];
        let prices = vec![
            vec![100.0, 101.0, 102.0, 103.0, 104.0, 105.0],
            vec![100.0, 100.5, 101.0, 101.5, 102.0, 102.5],
        ];
        let perf = Performance::new(
            dates,
            prices,
            vec!["BENCH".to_string(), "PORT".to_string()],
            Some("BENCH"),
            PeriodKind::Daily,
        )
        .expect("performance should build");

        let invalid_factor = [0.01, 0.02];
        let result = perf.multi_factor_greeks(1, &[&invalid_factor], ReturnKind::Excess);
        assert!(result.is_err());
    }

    #[test]
    fn standalone_multi_factor_greeks_errors_on_near_singular_factor_matrix() {
        let returns = [0.02, 0.04, 0.06, 0.08, 0.10, 0.12];
        let factor_a = [0.01, 0.02, 0.03, 0.04, 0.05, 0.06];
        let factor_b = [
            0.010_000_000_001,
            0.020_000_000_002,
            0.029_999_999_999,
            0.040_000_000_001,
            0.050_000_000_003,
            0.060_000_000_000,
        ];

        let result =
            multi_factor_greeks(&returns, &[&factor_a, &factor_b], 252.0, ReturnKind::Excess);
        assert!(result.is_err());
    }

    #[test]
    fn standalone_multi_factor_greeks_errors_on_hidden_multicollinearity() {
        let returns = [0.04, 0.01, 0.03, 0.02, 0.05, 0.06];
        let factor_a = [0.01, -0.02, 0.03, -0.01, 0.02, 0.01];
        let factor_b = [0.02, 0.01, -0.01, 0.03, -0.02, 0.04];
        let factor_c: Vec<f64> = factor_a
            .iter()
            .zip(factor_b.iter())
            .map(|(a, b)| a + b)
            .collect();

        let result = multi_factor_greeks(
            &returns,
            &[&factor_a, &factor_b, &factor_c],
            252.0,
            ReturnKind::Excess,
        );
        assert!(result.is_err());
    }

    #[test]
    fn standalone_multi_factor_greeks_handles_full_rank_scaled_factors() {
        let factor_a = [1.0e8, 2.0e8, 3.0e8, 4.0e8, 5.0e8, 6.0e8];
        let factor_b = [1.0e-4, -2.0e-4, 3.0e-4, -4.0e-4, 5.0e-4, -6.0e-4];
        let returns: Vec<f64> = factor_a
            .iter()
            .zip(factor_b.iter())
            .map(|(a, b)| 2.0 * a - 3.0 * b)
            .collect();

        let result =
            multi_factor_greeks(&returns, &[&factor_a, &factor_b], 252.0, ReturnKind::Excess)
                .expect("scaled factors should solve successfully");
        assert!((result.betas[0] - 2.0).abs() < 1e-10);
        assert!((result.betas[1] + 3.0).abs() < 5e-5);
    }

    #[test]
    fn multi_factor_result_non_finite_fields_match_serde_sentinels() {
        use crate::test_support::assert_non_finite_fields;
        let multi = MultiFactorResult {
            alpha: f64::NAN,
            betas: vec![f64::NAN],
            r_squared: f64::NAN,
            adjusted_r_squared: f64::NAN,
            residual_vol: f64::NAN,
            n_obs: 0,
        };
        assert_non_finite_fields(&multi);
    }

    #[test]
    fn multi_factor_reports_the_regression_sample_size() {
        let y = [0.011, 0.019, 0.032, 0.038, 0.052, 0.049];
        let f1 = [0.010, 0.020, 0.030, 0.040, 0.050, 0.045];
        let f2 = [0.003, -0.002, 0.004, -0.001, 0.002, 0.006];
        let multi = multi_factor_greeks(&y, &[&f1, &f2], 252.0, ReturnKind::Excess)
            .expect("multi-factor fit");
        assert_eq!(multi.n_obs, y.len());
        let dof = (multi.n_obs - multi.betas.len() - 1) as f64;
        assert_eq!(
            multi.adjusted_r_squared,
            1.0 - (1.0 - multi.r_squared) * (multi.n_obs as f64 - 1.0) / dof
        );
    }
}
