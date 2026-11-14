//! Multi-factor OLS regression: [`multi_factor_greeks`] with its
//! [`ReturnKind`] input and [`MultiFactorResult`] output.
//!
//! [`ReturnKind`] and [`MultiFactorResult`] are re-exported at the crate root.
//! The raw statistical function also accepts already-excess observations;
//! the investment-panel adapter always supplies total returns and explicit cash.

use crate::regression::normalized_svd_fit;
use finstack_quant_core::math::summation::neumaier_sum;
use finstack_quant_core::wire::NonFiniteFields;
use nalgebra::DMatrix;

/// How the dependent return series is interpreted in a multi-factor regression.
///
/// Factors are explanatory observations in caller-supplied units. Only the
/// dependent series is adjusted when [`ReturnKind::Total`] is used. The caller
/// must select the interpretation explicitly.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ReturnKind {
    /// `returns` are already excess returns. Alpha is the annualized OLS
    /// intercept of excess `y` on the supplied explanatory observations.
    Excess,
    /// `returns` are total returns. The geometrically decompounded period
    /// risk-free rate is subtracted from `y` only, then OLS is run.
    /// Alpha is the annualized intercept; its Jensen interpretation requires
    /// the caller's intended excess-return factor model.
    Total {
        /// Finite annual-effective decimal cash rate above `-1`, including
        /// explicit zero (for example, `0.02` for 2% effective per year).
        risk_free_rate: f64,
    },
}

impl ReturnKind {
    /// Build a return kind from its label and the caller's risk-free rate.
    ///
    /// # Arguments
    ///
    /// * `label` - Explicit `"excess"` or `"total"` response interpretation.
    /// * `risk_free_rate` - Annual-effective risk-free rate in decimal form
    ///   (`0.02` for 2%). [`ReturnKind::Total`] geometrically decompounds it
    ///   to the observation frequency and subtracts it from the dependent
    ///   series. Required and finite above `-1` for total returns, and absent
    ///   for excess returns, including when the intended cash rate is zero.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] for an unknown label,
    /// missing total-return cash, invalid cash, or any cash with excess returns.
    pub fn from_label(
        label: &str,
        risk_free_rate: Option<f64>,
    ) -> finstack_quant_core::Result<Self> {
        match (label, risk_free_rate) {
            ("excess", None) => Ok(Self::Excess),
            ("excess", Some(_)) => Err(finstack_quant_core::Error::Validation(
                "excess responses do not accept a risk_free_rate".into(),
            )),
            ("total", Some(rate)) if rate.is_finite() && rate > -1.0 => {
                Ok(Self::Total { risk_free_rate: rate })
            }
            ("total", _) => Err(finstack_quant_core::Error::Validation(
                "total responses require an explicit finite annual-effective risk_free_rate above -1".into(),
            )),
            (other, _) => Err(finstack_quant_core::Error::Validation(format!(
                "unknown return_kind {other:?}; expected \"excess\" or \"total\""
            ))),
        }
    }
}

/// Result of a multi-factor regression.
#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MultiFactorResult {
    /// Annualized OLS intercept of the (possibly rf-adjusted) dependent series.
    pub alpha: f64,
    /// Regression coefficients, one per factor.
    #[cfg_attr(feature = "json-schema", schemars(length(min = 1)))]
    pub betas: Vec<f64>,
    /// Fraction of variance explained; NaN for a constant dependent series.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(
            with = "finstack_quant_core::wire::FiniteOrNanWire<finstack_quant_core::wire::ClosedUnitIntervalF64Wire>"
        )
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
        schemars(with = "finstack_quant_core::wire::FiniteOrNanWire")
    )]
    pub adjusted_r_squared: f64,
    /// Annualized residual-df-adjusted volatility; NaN for a saturated fit.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(
            with = "finstack_quant_core::wire::FiniteOrNanWire<finstack_quant_core::wire::NonNegativeF64Wire>"
        )
    )]
    pub residual_vol: f64,
    /// Number of observations the regression used: the common length of the
    /// dependent series and every factor series. Residual degrees of freedom
    /// are `n_obs − betas.len() − 1`. Required in the serialized result.
    #[cfg_attr(feature = "json-schema", schemars(range(min = 2)))]
    pub n_obs: usize,
}

impl NonFiniteFields for MultiFactorResult {
    const NON_FINITE_FIELDS: &'static [&'static str] =
        &["r_squared", "adjusted_r_squared", "residual_vol"];
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct MultiFactorWire {
    alpha: f64,
    betas: Vec<f64>,
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    r_squared: f64,
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    adjusted_r_squared: f64,
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    residual_vol: f64,
    n_obs: usize,
}

impl<'de> serde::Deserialize<'de> for MultiFactorResult {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = <MultiFactorWire as serde::Deserialize>::deserialize(deserializer)?;
        wire.try_into().map_err(serde::de::Error::custom)
    }
}

impl TryFrom<MultiFactorWire> for MultiFactorResult {
    type Error = String;

    fn try_from(wire: MultiFactorWire) -> Result<Self, Self::Error> {
        let result = Self {
            alpha: wire.alpha,
            betas: wire.betas,
            r_squared: wire.r_squared,
            adjusted_r_squared: wire.adjusted_r_squared,
            residual_vol: wire.residual_vol,
            n_obs: wire.n_obs,
        };
        result.validate()?;
        Ok(result)
    }
}

impl MultiFactorResult {
    fn validate(&self) -> Result<(), String> {
        let p = self
            .betas
            .len()
            .checked_add(1)
            .ok_or("factor count overflow")?;
        if self.betas.is_empty()
            || !self.alpha.is_finite()
            || self.betas.iter().any(|v| !v.is_finite())
            || self.n_obs < p
        {
            return Err(
                "multifactor result requires finite coefficients and n_obs >= factors + 1".into(),
            );
        }
        let r = self.r_squared;
        if !(r.is_nan() || r.is_finite() && (0.0..=1.0).contains(&r)) {
            return Err("R-squared must be in [0,1] or nan".into());
        }
        let dof = self.n_obs - p;
        if dof == 0 {
            if !self.adjusted_r_squared.is_nan()
                || !self.residual_vol.is_nan()
                || r.is_finite() && (r - 1.0).abs() > 64.0 * f64::EPSILON * self.n_obs as f64
            {
                return Err("saturated fits require undefined adjusted fit and residual risk, and available R-squared one".into());
            }
        } else {
            if !self.residual_vol.is_finite() || self.residual_vol < 0.0 {
                return Err(
                    "positive residual degrees of freedom require finite nonnegative residual risk"
                        .into(),
                );
            }
            if r.is_nan() {
                if !self.adjusted_r_squared.is_nan() {
                    return Err("undefined R-squared requires undefined adjusted R-squared".into());
                }
            } else {
                let expected = 1.0 - (1.0 - r) * (self.n_obs - 1) as f64 / dof as f64;
                let tolerance = 64.0 * f64::EPSILON * expected.abs().max(1.0);
                if !self.adjusted_r_squared.is_finite()
                    || (self.adjusted_r_squared - expected).abs() > tolerance
                {
                    return Err(
                        "adjusted R-squared is inconsistent with R-squared and counts".into(),
                    );
                }
            }
        }
        Ok(())
    }
}

/// Multi-factor OLS regression of portfolio returns on factor returns.
///
/// Estimates the linear model
///
/// ```text
/// y = α + β₁f₁ + β₂f₂ + ... + βₖfₖ + ε
/// ```
///
/// by normalized full-rank SVD, without normal equations, regularization,
/// dropped factors, or a rank-truncated minimum-norm fallback.
///
/// Factor observations retain their supplied interpretation. [`ReturnKind::Total`]
/// subtracts the geometrically decompounded period risk-free rate from the
/// **dependent** series only. [`ReturnKind::Excess`] leaves `y` unchanged;
/// the annualized intercept is then the alpha of already-excess `y`, not a
/// Jensen adjustment of total returns.
///
/// # Arguments
///
/// * `returns` - Finite response observations in decimal-return units, paired
///   by position with every factor. Total histories must exceed `-1`, except
///   for one optional terminal `-1`; already-excess observations may be below
///   `-1` and are never compounded as wealth.
/// * `factors` - Factor-major finite explanatory observations, each exactly
///   `returns.len()` long. Factor order determines beta order. Callers own
///   dates, intervals, units and factor construction; no cash is subtracted
///   from predictors. For a financial model use intended excess-return or
///   long-short factors, or explicitly interpret other numeric predictor units.
/// * `ann_factor` - Explicit finite positive observations per year, such as
///   `252.0`. Alpha scales arithmetically by this factor and residual volatility
///   by its square root; observation dates do not infer frequency.
/// * `return_kind` - Explicit response interpretation: `Excess` uses the response
///   unchanged; `Total` requires annual-effective decimal cash above `-1` and
///   subtracts `expm1(log1p(cash)/ann_factor)` only from the fitted intercept.
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
/// A full-rank model requires at least `factors.len() + 1` observations. In a
/// saturated fit coefficients and nonconstant-response R-squared are available,
/// while adjusted R-squared and residual volatility are NaN. Constant-response
/// fit statistics are NaN, with zero residual volatility only for positive
/// residual degrees of freedom. No alignment, truncation or missing-value fill
/// is performed. Residual risk is in-sample, not a predictive guarantee.
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] or an input error when:
///
/// - frequency, cash, response domain or any numeric input is invalid
/// - no factors are supplied
/// - there are too few observations for the requested number of factors
/// - any factor return is non-finite
/// - any factor length differs from `returns.len()`
/// - the factor matrix is singular or numerically rank deficient
///
/// Returns [`finstack_quant_core::Error::Internal`] when numerical evaluation
/// cannot represent or validate a requested coefficient or fit statistic.
/// Successful results preserve expected field-wise NaN availability.
///
/// # References
///
/// - Fama & French (1993): see docs/REFERENCES.md#fama-french-1993
/// - Higham: see docs/REFERENCES.md#higham-accuracy-and-stability `docs/REFERENCES.md#higham-accuracy-and-stability`
#[tracing::instrument(level = "debug", skip(returns, factors), fields(n = returns.len(), k = factors.len(), ann_factor = ann_factor))]
pub fn multi_factor_greeks(
    returns: &[f64],
    factors: &[&[f64]],
    ann_factor: f64,
    return_kind: ReturnKind,
) -> finstack_quant_core::Result<MultiFactorResult> {
    use finstack_quant_core::{Error, InputError};
    crate::returns::validate_frequency(ann_factor)?;
    if let ReturnKind::Total { risk_free_rate } = return_kind {
        crate::returns::validate_cash(risk_free_rate)?;
    }
    let n = returns.len();
    let k = factors.len();
    let p = k.checked_add(1).ok_or(InputError::DimensionMismatch)?;
    if factors.iter().any(|factor| factor.len() != n) {
        return Err(InputError::DimensionMismatch.into());
    }
    crate::regression::ensure_finite(returns)?;
    for factor in factors {
        crate::regression::ensure_finite(factor)?;
    }
    if matches!(return_kind, ReturnKind::Total { .. }) {
        crate::returns::validate_history(returns)?;
    }
    let cash = match return_kind {
        ReturnKind::Excess => 0.0,
        ReturnKind::Total { risk_free_rate } => {
            crate::returns::periodic_risk_free_rate(risk_free_rate, ann_factor)?
        }
    };
    if k == 0 || n < p {
        return Err(Error::Validation(format!(
            "multi-factor regression requires at least one factor and factors + 1 observations; got {k} factors and {n} observations"
        )));
    }
    let design = DMatrix::from_fn(
        n,
        p,
        |row, col| {
            if col == 0 { 1.0 } else { factors[col - 1][row] }
        },
    );
    // Center around an observed value, retaining high-offset response variation.
    // Cash is translated only in the fitted intercept, so large cash does not
    // round distinct total-return observations into one repeated excess value.
    let first = returns[0];
    let first_abs = first.abs();
    let origin = if returns.iter().all(|v| {
        v.is_sign_negative() == first.is_sign_negative()
            && v.abs() >= first_abs * 0.5
            && v.abs() * 0.5 <= first_abs
    }) {
        first
    } else {
        0.0
    };
    let shifted: Vec<f64> = returns.iter().map(|value| value - origin).collect();
    let targets = DMatrix::from_column_slice(n, 1, &shifted);
    let fit = normalized_svd_fit(design, &targets)?;
    let response_scale = fit.target_scales[0];
    let coefficients = fit.coefficients.column(0);
    let factor_betas: Vec<f64> = coefficients.iter().skip(1).copied().collect();
    let alpha = scaled_sum_product(&[origin, coefficients[0], -cash], ann_factor)?;
    let constant = returns.iter().all(|value| (*value - first).abs() <= 0.0);
    let dof = n - p;
    let (r_squared, adjusted_r_squared, residual_vol) = if constant {
        (f64::NAN, f64::NAN, if dof > 0 { 0.0 } else { f64::NAN })
    } else {
        let normalized: Vec<f64> = shifted.iter().map(|value| value / response_scale).collect();
        let mean = neumaier_sum(normalized.iter().map(|value| value / n as f64));
        let centered: Vec<f64> = normalized.iter().map(|value| value - mean).collect();
        let residuals: Vec<f64> = normalized
            .iter()
            .enumerate()
            .map(|(i, value)| value - fit.fitted[(i, 0)])
            .collect();
        let total_norm = stable_norm(&centered)?;
        let residual_norm = stable_norm(&residuals)?;
        if total_norm == 0.0 {
            return Err(Error::Internal(
                "nonconstant response dispersion collapsed numerically".into(),
            ));
        }
        let relative_residual = residual_norm / total_norm;
        // Near zero fit, subtracting SSE/SST from one loses all relative
        // accuracy and can manufacture a negative value. Re-evaluate the same
        // intercept-OLS projection through explained dispersion instead.
        let r_squared = if relative_residual > std::f64::consts::FRAC_1_SQRT_2 {
            let explained: Vec<f64> = (0..n).map(|i| fit.fitted[(i, 0)] - mean).collect();
            let fraction = stable_norm(&explained)? / total_norm;
            fraction * fraction
        } else {
            1.0 - relative_residual * relative_residual
        };
        if !r_squared.is_finite() || !(0.0..=1.0).contains(&r_squared) {
            return Err(Error::Internal(
                "OLS fit statistics are outside their finite domain".into(),
            ));
        }
        let adjusted = if dof > 0 {
            1.0 - (1.0 - r_squared) * (n - 1) as f64 / dof as f64
        } else {
            f64::NAN
        };
        let volatility = if dof > 0 {
            finite_product3(
                residual_norm,
                response_scale,
                ann_factor.sqrt() / (dof as f64).sqrt(),
            )?
        } else {
            f64::NAN
        };
        if dof > 0 && !adjusted.is_finite() {
            return Err(Error::Internal(
                "adjusted R-squared is not representable".into(),
            ));
        }
        (r_squared, adjusted, volatility)
    };
    let result = MultiFactorResult {
        alpha,
        betas: if constant { vec![0.0; k] } else { factor_betas },
        r_squared,
        adjusted_r_squared,
        residual_vol,
        n_obs: n,
    };
    result.validate().map_err(Error::Internal)?;
    Ok(result)
}

fn stable_norm(values: &[f64]) -> finstack_quant_core::Result<f64> {
    let scale = values.iter().map(|v| v.abs()).fold(0.0, f64::max);
    if scale == 0.0 {
        return Ok(0.0);
    }
    finite_product3(
        neumaier_sum(values.iter().map(|v| (v / scale).powi(2))).sqrt(),
        scale,
        1.0,
    )
}

fn scaled_sum_product(values: &[f64], multiplier: f64) -> finstack_quant_core::Result<f64> {
    let direct = neumaier_sum(values.iter().copied());
    if direct.is_finite() {
        return finite_product3(direct, 1.0, multiplier);
    }
    let scale = values.iter().map(|v| v.abs()).fold(0.0, f64::max);
    if scale == 0.0 {
        return Ok(0.0);
    }
    finite_product3(
        neumaier_sum(values.iter().map(|v| v / scale)),
        scale,
        multiplier,
    )
}

fn finite_product3(a: f64, b: f64, c: f64) -> finstack_quant_core::Result<f64> {
    if a == 0.0 || b == 0.0 || c == 0.0 {
        return Ok(0.0);
    }
    // Refine the same multiplication in a safe order before rejecting range.
    let candidates = [(a * b, c), (a * c, b), (b * c, a)];
    for &(partial, remaining) in &candidates {
        let value = partial * remaining;
        if partial.is_normal() && value.is_finite() && value != 0.0 {
            return Ok(value);
        }
    }
    for &(partial, remaining) in &candidates {
        let value = partial * remaining;
        if value.is_finite() && value != 0.0 {
            return Ok(value);
        }
    }
    Err(finstack_quant_core::Error::Internal(
        "multifactor requested estimate is not representable as a finite nonzero value".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::Date;

    #[test]
    fn return_kind_requires_explicit_kind_and_cash() {
        assert_eq!(
            ReturnKind::from_label("excess", None).expect("excess"),
            ReturnKind::Excess
        );
        assert_eq!(
            ReturnKind::from_label("total", Some(0.02)).expect("total"),
            ReturnKind::Total {
                risk_free_rate: 0.02
            }
        );
        assert!(ReturnKind::from_label("total", None).is_err());
        assert!(ReturnKind::from_label("excess", Some(0.0)).is_err());
        assert!(ReturnKind::from_label("total", Some(-1.0)).is_err());
        assert!(ReturnKind::from_label("jensen", None).is_err());
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
        let rf_period = (f64::ln_1p(rf_annual) / 252.0).exp_m1();
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
        let perf = Performance::new(&crate::performance::prices_panel(
            dates,
            prices,
            vec!["BENCH".to_string(), "PORT".to_string()],
            Some("BENCH"),
            PeriodKind::Daily,
        ))
        .expect("performance should build");

        let invalid_factor = [0.01, 0.02];
        let result = perf.multi_factor_greeks(1, &[&invalid_factor], 0.0);
        assert!(result.is_err());
    }

    #[test]
    fn standalone_multi_factor_greeks_accepts_resolved_near_collinearity() {
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
        assert!(result.is_ok());
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
        // Decimal80 OLS over the actual binary-f64 response observations,
        // not the unrounded generating expression, gives this second loading.
        assert!((result.betas[1] + 3.000_027_858_293_973_5).abs() < 5e-5);
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

    #[test]
    fn saturated_fit_has_identified_loadings_without_residual_risk() {
        // Three independently specified equations uniquely determine a=1%,
        // b1=2, b2=-0.5; no residual degrees of freedom remain.
        let result = multi_factor_greeks(
            &[-0.015, 0.020, 0.025],
            &[&[-0.01, 0.0, 0.01], &[0.01, -0.02, 0.01]],
            12.0,
            ReturnKind::Excess,
        )
        .expect("identified saturated fit");
        assert!((result.alpha - 0.12).abs() < 1e-14);
        assert!((result.betas[0] - 2.0).abs() < 1e-14);
        assert!((result.betas[1] + 0.5).abs() < 1e-14);
        assert_eq!(result.r_squared, 1.0);
        assert!(result.adjusted_r_squared.is_nan());
        assert!(result.residual_vol.is_nan());
        let json = serde_json::to_value(&result).expect("wire");
        assert_eq!(json["residual_vol"], "nan");
        let restored: MultiFactorResult = serde_json::from_value(json).expect("restore");
        assert!(restored.residual_vol.is_nan());
    }

    #[test]
    fn constant_response_and_invalid_model_requests_remain_distinct() {
        let fit = multi_factor_greeks(&[0.03, 0.03], &[&[-1.0, 1.0]], 12.0, ReturnKind::Excess)
            .expect("constant saturated fit");
        assert_eq!(fit.betas, [0.0]);
        assert!((fit.alpha - 0.36).abs() < 1e-15);
        assert!(fit.r_squared.is_nan());
        assert!(fit.residual_vol.is_nan());
        assert!(multi_factor_greeks(&[0.0], &[&[1.0]], 0.0, ReturnKind::Excess).is_err());
        assert!(
            multi_factor_greeks(&[0.0, 0.1], &[&[1.0, 1.0]], 12.0, ReturnKind::Excess).is_err()
        );
        assert!(
            multi_factor_greeks(
                &[-1.0, 0.0],
                &[&[0.0, 1.0]],
                12.0,
                ReturnKind::Total {
                    risk_free_rate: 0.0
                }
            )
            .is_err()
        );
        assert!(
            multi_factor_greeks(&[-1.5, -2.0], &[&[0.0, 1.0]], 12.0, ReturnKind::Excess).is_ok()
        );
    }

    #[test]
    fn total_cash_translation_preserves_tiny_response_variation() {
        let y = [1e-200, 2e-200, 3e-200, 4e-200];
        let factors = [&[-1.0, 0.0, 1.0, 2.0][..]];
        let excess = multi_factor_greeks(&y, &factors, 1.0, ReturnKind::Excess).expect("excess");
        let total = multi_factor_greeks(
            &y,
            &factors,
            1.0,
            ReturnKind::Total {
                risk_free_rate: 1e100,
            },
        )
        .expect("total");
        assert_eq!(total.betas, excess.betas);
        assert_eq!(total.r_squared, excess.r_squared);
        assert_eq!(total.residual_vol, excess.residual_vol);
        assert!((total.alpha / -1e100 - 1.0).abs() < 1e-13);
    }

    #[test]
    fn subnormal_response_and_factor_units_preserve_finite_loading() {
        let x = [1e-320, 2e-320, 3e-320, 4e-320];
        let fit = multi_factor_greeks(&x, &[&x], 1.0, ReturnKind::Excess)
            .expect("finite loading despite subnormal observation units");
        assert!((fit.betas[0] - 1.0).abs() < 1e-3);
        assert_eq!(fit.r_squared, 1.0);
    }

    #[test]
    fn noisy_subnormal_observations_retain_zero_alpha_and_unit_beta() {
        let x = [1e-320, 2e-320, 3e-320, 4e-320];
        let y = [2e-320, 1e-320, 2e-320, 5e-320];
        let fit = multi_factor_greeks(&y, &[&x], 1.0, ReturnKind::Excess)
            .expect("overdetermined noisy subnormal regression should preserve exact zero alpha");
        assert_eq!(fit.alpha, 0.0);
        assert!((fit.betas[0] - 1.0).abs() < 1e-12);
        assert!((fit.r_squared - 5.0 / 9.0).abs() < 1e-12);
        assert!(fit.residual_vol.is_finite());
    }

    #[test]
    fn large_residual_variance_preserves_zero_fit_and_finite_risk() {
        let fit = multi_factor_greeks(
            &[3e200, 1e200, 3e200, 1e200, 3e200],
            &[&[-0.2, -0.1, 0.0, 0.1, 0.2]],
            252.0,
            ReturnKind::Total {
                risk_free_rate: 0.0,
            },
        )
        .expect("finite annual risk despite overflowing conceptual squared residuals");
        assert!((fit.alpha / (2.2e200 * 252.0) - 1.0).abs() < 1e-14);
        assert!(fit.r_squared.abs() < 1e-28);
        assert!((fit.adjusted_r_squared + 1.0 / 3.0).abs() < 1e-14);
        assert!((fit.residual_vol / ((4.8_f64 / 3.0 * 252.0).sqrt() * 1e200) - 1.0).abs() < 1e-14);
    }

    #[test]
    fn result_restoration_rejects_inconsistent_availability_and_fit() {
        let valid = serde_json::json!({"alpha":0.1,"betas":[2.0],"r_squared":0.5,"adjusted_r_squared":0.0,"residual_vol":0.2,"n_obs":3});
        assert!(serde_json::from_value::<MultiFactorResult>(valid.clone()).is_ok());
        for (field, value) in [
            ("betas", serde_json::json!([])),
            ("r_squared", serde_json::json!(1.1)),
            ("adjusted_r_squared", serde_json::json!(0.5)),
            ("residual_vol", serde_json::json!("nan")),
            ("residual_vol", serde_json::json!("inf")),
            ("residual_vol", serde_json::json!(-0.1)),
            ("residual_vol", serde_json::Value::Null),
            ("n_obs", serde_json::json!(2)),
        ] {
            let mut malformed = valid.clone();
            malformed[field] = value;
            assert!(
                serde_json::from_value::<MultiFactorResult>(malformed).is_err(),
                "{field}"
            );
        }
    }
}
