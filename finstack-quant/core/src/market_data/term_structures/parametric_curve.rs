//! Nelson-Siegel and Nelson-Siegel-Svensson parametric yield curves.
//!
//! Provides an analytic parametric factor curve defined by validated
//! Nelson-Siegel (NS) or Nelson-Siegel-Svensson (NSS) parameters on an explicit
//! source clock and optional continuous-zero shocks / dated conditioning views.

use super::common::{
    CurveCalendar, PiecewiseLinearAdjustment, signed_curve_year_fraction,
    stable_positive_log_ratio, validate_curve_clock,
};
use crate::{
    dates::{Date, DayCount, DayCountContext},
    market_data::{
        bumps::{BumpSpec, Bumpable},
        traits::Discounting,
    },
    math::{
        Compounding,
        interp::exact_f64_eq,
        stats::{frexp_exp, ldexp},
        summation::NeumaierAccumulator,
    },
    types::CurveId,
};
use serde::{Deserialize, Serialize};

/// Nelson-Siegel model variant selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum NsVariant {
    /// Four-parameter Nelson-Siegel model.
    Ns,
    /// Six-parameter Nelson-Siegel-Svensson model.
    Nss,
}

/// Wire representation of [`NelsonSiegelModel`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "json-schema", schemars(rename = "NelsonSiegelModel"))]
#[serde(tag = "variant", rename_all = "snake_case", deny_unknown_fields)]
enum RawNelsonSiegelModel {
    /// Four-parameter Nelson-Siegel.
    Ns {
        beta0: f64,
        beta1: f64,
        beta2: f64,
        tau: f64,
    },
    /// Six-parameter Nelson-Siegel-Svensson.
    Nss {
        beta0: f64,
        beta1: f64,
        beta2: f64,
        beta3: f64,
        tau1: f64,
        tau2: f64,
    },
}

/// Validated Nelson-Siegel or Nelson-Siegel-Svensson model parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "json-schema", schemars(with = "RawNelsonSiegelModel"))]
#[serde(try_from = "RawNelsonSiegelModel", into = "RawNelsonSiegelModel")]
pub struct NelsonSiegelModel(RawNelsonSiegelModel);

impl From<NelsonSiegelModel> for RawNelsonSiegelModel {
    fn from(model: NelsonSiegelModel) -> Self {
        model.0
    }
}

impl TryFrom<RawNelsonSiegelModel> for NelsonSiegelModel {
    type Error = crate::Error;

    fn try_from(raw: RawNelsonSiegelModel) -> crate::Result<Self> {
        let model = Self(raw);
        model.validate()?;
        Ok(model)
    }
}

impl NelsonSiegelModel {
    /// Construct and validate a 4-parameter Nelson-Siegel model.
    ///
    /// # Arguments
    ///
    /// * `beta0` - Long-term rate level (decimal continuous rate).
    /// * `beta1` - Short-term slope component (`beta0 + beta1` is the origin short rate).
    /// * `beta2` - Medium-term hump component.
    /// * `tau` - Positive decay horizon in source-clock years (`tau > 0`).
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] if any parameter is non-finite or `tau <= 0.0`.
    pub fn ns(beta0: f64, beta1: f64, beta2: f64, tau: f64) -> crate::Result<Self> {
        let model = Self(RawNelsonSiegelModel::Ns {
            beta0,
            beta1,
            beta2,
            tau,
        });
        model.validate()?;
        Ok(model)
    }

    /// Construct and validate a 6-parameter Nelson-Siegel-Svensson model.
    ///
    /// Equal or nearly equal `(tau1, tau2)` horizons are permitted.
    ///
    /// # Arguments
    ///
    /// * `beta0` - Long-term rate level (decimal continuous rate).
    /// * `beta1` - Short-term slope component.
    /// * `beta2` - First medium-term hump component associated with `tau1`.
    /// * `beta3` - Second medium-term hump component associated with `tau2`.
    /// * `tau1` - First positive decay horizon in source-clock years (`tau1 > 0`).
    /// * `tau2` - Second positive decay horizon in source-clock years (`tau2 > 0`).
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] if any parameter is non-finite or
    /// `tau1 <= 0.0` or `tau2 <= 0.0`.
    pub fn nss(
        beta0: f64,
        beta1: f64,
        beta2: f64,
        beta3: f64,
        tau1: f64,
        tau2: f64,
    ) -> crate::Result<Self> {
        let model = Self(RawNelsonSiegelModel::Nss {
            beta0,
            beta1,
            beta2,
            beta3,
            tau1,
            tau2,
        });
        model.validate()?;
        Ok(model)
    }

    /// Model variant selector (`Ns` or `Nss`).
    #[must_use]
    pub const fn variant(&self) -> NsVariant {
        match self.0 {
            RawNelsonSiegelModel::Ns { .. } => NsVariant::Ns,
            RawNelsonSiegelModel::Nss { .. } => NsVariant::Nss,
        }
    }

    /// Asymptotic continuous rate `beta0`.
    #[must_use]
    pub const fn beta0(&self) -> f64 {
        match self.0 {
            RawNelsonSiegelModel::Ns { beta0, .. } | RawNelsonSiegelModel::Nss { beta0, .. } => {
                beta0
            }
        }
    }

    /// Short-end slope loading `beta1`.
    #[must_use]
    pub const fn beta1(&self) -> f64 {
        match self.0 {
            RawNelsonSiegelModel::Ns { beta1, .. } | RawNelsonSiegelModel::Nss { beta1, .. } => {
                beta1
            }
        }
    }

    /// Primary curvature loading `beta2`.
    #[must_use]
    pub const fn beta2(&self) -> f64 {
        match self.0 {
            RawNelsonSiegelModel::Ns { beta2, .. } | RawNelsonSiegelModel::Nss { beta2, .. } => {
                beta2
            }
        }
    }

    /// Secondary curvature loading `beta3` (`None` for NS).
    #[must_use]
    pub const fn beta3(&self) -> Option<f64> {
        match self.0 {
            RawNelsonSiegelModel::Ns { .. } => None,
            RawNelsonSiegelModel::Nss { beta3, .. } => Some(beta3),
        }
    }

    /// Primary decay horizon (`tau` for NS, `tau1` for NSS).
    #[must_use]
    pub const fn tau1(&self) -> f64 {
        match self.0 {
            RawNelsonSiegelModel::Ns { tau, .. } | RawNelsonSiegelModel::Nss { tau1: tau, .. } => {
                tau
            }
        }
    }

    /// Secondary decay horizon `tau2` (`None` for NS).
    #[must_use]
    pub const fn tau2(&self) -> Option<f64> {
        match self.0 {
            RawNelsonSiegelModel::Ns { .. } => None,
            RawNelsonSiegelModel::Nss { tau2, .. } => Some(tau2),
        }
    }

    /// Convert parameters to a flat vector for optimizer consumption.
    #[must_use]
    pub fn to_params_vec(&self) -> Vec<f64> {
        match self.0 {
            RawNelsonSiegelModel::Ns {
                beta0,
                beta1,
                beta2,
                tau,
            } => vec![beta0, beta1, beta2, tau],
            RawNelsonSiegelModel::Nss {
                beta0,
                beta1,
                beta2,
                beta3,
                tau1,
                tau2,
            } => vec![beta0, beta1, beta2, beta3, tau1, tau2],
        }
    }

    /// Construct from a flat parameter vector.
    ///
    /// # Arguments
    ///
    /// * `variant` - `Ns` (expects length 4) or `Nss` (expects length 6).
    /// * `params` - Parameter slice matching `(beta0, beta1, beta2, tau)` or
    ///   `(beta0, beta1, beta2, beta3, tau1, tau2)`.
    ///
    /// # Errors
    ///
    /// Returns an error if the vector length doesn't match the variant, any
    /// parameter is non-finite, or any decay horizon is non-positive.
    pub fn from_params_vec(variant: NsVariant, params: &[f64]) -> crate::Result<Self> {
        match variant {
            NsVariant::Ns => {
                if params.len() != 4 {
                    return Err(crate::Error::Validation(format!(
                        "NS requires 4 parameters, got {}",
                        params.len()
                    )));
                }
                Self::ns(params[0], params[1], params[2], params[3])
            }
            NsVariant::Nss => {
                if params.len() != 6 {
                    return Err(crate::Error::Validation(format!(
                        "NSS requires 6 parameters, got {}",
                        params.len()
                    )));
                }
                Self::nss(
                    params[0], params[1], params[2], params[3], params[4], params[5],
                )
            }
        }
    }

    /// Validate finite rate loadings and strictly positive decay horizons.
    pub fn validate(&self) -> crate::Result<()> {
        let finite = match self.0 {
            RawNelsonSiegelModel::Ns {
                beta0,
                beta1,
                beta2,
                tau,
            } => [beta0, beta1, beta2, tau].iter().all(|v| v.is_finite()),
            RawNelsonSiegelModel::Nss {
                beta0,
                beta1,
                beta2,
                beta3,
                tau1,
                tau2,
            } => [beta0, beta1, beta2, beta3, tau1, tau2]
                .iter()
                .all(|v| v.is_finite()),
        };
        if !finite {
            return Err(crate::Error::Validation(
                "Nelson-Siegel coefficients and decay horizons must be finite".to_string(),
            ));
        }
        match self.0 {
            RawNelsonSiegelModel::Ns { tau, .. } => {
                if tau <= 0.0 {
                    return Err(crate::Error::Validation(format!(
                        "NS tau must be positive, got {tau}"
                    )));
                }
            }
            RawNelsonSiegelModel::Nss { tau1, tau2, .. } => {
                if tau1 <= 0.0 || tau2 <= 0.0 {
                    return Err(crate::Error::Validation(format!(
                        "NSS tau values must be positive, got tau1={tau1}, tau2={tau2}"
                    )));
                }
            }
        }
        Ok(())
    }
}

/// Evaluate `t * scale` with exponent decomposition when direct product overflows/underflows.
fn checked_scaled_product(a: f64, b: f64) -> crate::Result<f64> {
    if exact_f64_eq(a, 0.0) || exact_f64_eq(b, 0.0) {
        return Ok(0.0);
    }
    let direct = a * b;
    if direct.is_finite() && !exact_f64_eq(direct, 0.0) {
        return Ok(direct);
    }
    let ea = frexp_exp(a);
    let eb = frexp_exp(b);
    let ma = ldexp(a, -ea);
    let mb = ldexp(b, -eb);
    let prod_m = ma * mb;
    let total_e = ea.saturating_add(eb);
    let res = if total_e > i32::from(i16::MAX) {
        return Err(crate::Error::Validation(format!(
            "parametric product overflowed f64: {a} * {b}"
        )));
    } else if total_e < -1100 {
        0.0
    } else {
        ldexp(prod_m, total_e)
    };
    if !res.is_finite() {
        return Err(crate::Error::Validation(format!(
            "parametric product overflowed f64: {a} * {b}"
        )));
    }
    Ok(res)
}

/// Evaluate `beta * exp(-x)` safely without `inf * 0 = NaN` or premature `exp(-x)` underflow.
fn weighted_exp_decay(beta: f64, x: f64) -> f64 {
    if exact_f64_eq(beta, 0.0) {
        return 0.0;
    }
    if x < 700.0 {
        return beta * (-x).exp();
    }
    if x > 2000.0 {
        return 0.0;
    }
    let log_mag = beta.abs().ln() - x;
    if log_mag < -750.0 {
        0.0
    } else {
        beta.signum() * log_mag.exp()
    }
}

/// Evaluate `beta * x * exp(-x)` safely without `inf * 0 = NaN` or premature underflow.
fn weighted_x_exp_decay(beta: f64, x: f64) -> f64 {
    if exact_f64_eq(beta, 0.0) || exact_f64_eq(x, 0.0) {
        return 0.0;
    }
    if x < 700.0 {
        return beta * x * (-x).exp();
    }
    if x > 2000.0 {
        return 0.0;
    }
    let log_mag = beta.abs().ln() + x.ln() - x;
    if log_mag < -750.0 {
        0.0
    } else {
        beta.signum() * log_mag.exp()
    }
}

/// Classify `t / tau` cleanly using log difference / `frexp` so neither overflow nor
/// premature underflow corrupts small-`x` or large-`x` regimes.
fn scaled_ratio(t: f64, tau: f64) -> (f64, i32) {
    let et = frexp_exp(t);
    let e_tau = frexp_exp(tau);
    let mt = ldexp(t, -et);
    let m_tau = ldexp(tau, -e_tau);
    (mt / m_tau, et - e_tau)
}

/// Accumulate degree-16 Taylor series for a single decay scale `(m_x, e_x)` with `x <= 0.5`.
///
/// Combines primary `(b1, b2)` into `(b1 - n * b2)` (or secondary `(0, b3)` into `-n * b3`
/// by setting `b1 = 0, b2 = b3`).
fn accumulate_small_x_terms(
    t: f64,
    m_x: f64,
    e_x: i32,
    b1: f64,
    b2: f64,
    k_acc: &mut NeumaierAccumulator,
    f_acc: &mut NeumaierAccumulator,
) -> crate::Result<()> {
    let mut fact_n = 1.0_f64;
    let mut pow_m = 1.0_f64;
    let mut pow_e: i32 = 0;

    for n in 1_usize..=16_usize {
        let nf = n as f64;
        fact_n *= nf;
        // (-x)^n power tracking via mantissa * 2^pow_e
        pow_m *= -m_x;
        pow_e = pow_e.saturating_add(e_x);
        // Renormalize mantissa when needed to stay well within normal float range.
        if !exact_f64_eq(pow_m, 0.0) {
            let em = frexp_exp(pow_m);
            pow_m = ldexp(pow_m, -em);
            pow_e = pow_e.saturating_add(em);
        }

        let combo = b1 - nf * b2;
        if exact_f64_eq(combo, 0.0) {
            continue;
        }
        // f_n = (combo / n!) * (-x)^n
        let coeff_f = (combo / fact_n) * pow_m;
        let term_f = if pow_e < -1100 {
            0.0
        } else if pow_e > 1020 {
            return Err(crate::Error::Validation(
                "parametric Taylor term overflowed f64".to_string(),
            ));
        } else {
            ldexp(coeff_f, pow_e)
        };
        if !term_f.is_finite() {
            return Err(crate::Error::Validation(
                "parametric forward Taylor term overflowed f64".to_string(),
            ));
        }
        f_acc.add(term_f)?;

        // K_n = t * (combo / (n+1)!) * (-x)^n = (t / (n + 1)) * f_n (or scaled directly)
        let term_k_raw = checked_scaled_product(t / (nf + 1.0), term_f)?;
        k_acc.add(term_k_raw)?;
    }
    Ok(())
}

/// Add a single scale contribution to `(K(t), f(t))` for `t > 0`.
fn add_scale_contribution(
    t: f64,
    tau: f64,
    b1: f64,
    b2: f64,
    k_acc: &mut NeumaierAccumulator,
    f_acc: &mut NeumaierAccumulator,
) -> crate::Result<()> {
    let (m_x, e_x) = scaled_ratio(t, tau);
    // Check whether x = m_x * 2^e_x <= 0.5 without overflowing t/tau:
    // If e_x <= -2: x < 2 * 0.25 = 0.5. If e_x in -1..=0: check ldexp(m_x, e_x) <= 0.5.
    let in_small = e_x <= -2 || (e_x <= 0 && ldexp(m_x, e_x) <= 0.5);
    if in_small {
        f_acc.add(b1)?;
        k_acc.add(checked_scaled_product(t, b1)?)?;
        accumulate_small_x_terms(t, m_x, e_x, b1, b2, k_acc, f_acc)?;
        return Ok(());
    }

    // x > 0.5: compare against x >= 750 or asymptotic regimes via log or bounded ratio.
    let log_x = m_x.ln() + f64::from(e_x) * std::f64::consts::LN_2;
    if log_x >= 2000.0_f64.ln() {
        // x >= 2000: e^(-x) and x*e^(-x) are strictly below subnormals even times f64::MAX.
        // K contribution = (b1 + b2) * tau; f contribution = 0.
        k_acc.add(checked_scaled_product(b1 + b2, tau)?)?;
        return Ok(());
    }

    let x = ldexp(m_x, e_x);
    if !x.is_finite() {
        k_acc.add(checked_scaled_product(b1 + b2, tau)?)?;
        return Ok(());
    }

    // Standard / scaled evaluation for 0.5 < x < 2000.
    let em1 = if x < 750.0 { -(-x).exp_m1() } else { 1.0 };
    k_acc.add(checked_scaled_product((b1 + b2) * em1, tau)?)?;
    k_acc.add(weighted_exp_decay(-b2 * t, x))?;

    f_acc.add(weighted_exp_decay(b1, x))?;
    f_acc.add(weighted_x_exp_decay(b2, x))?;
    Ok(())
}

/// Evaluate integrated forward `K(t)` and analytic instantaneous forward `f(t)` for `t >= 0`.
fn eval_source_kernel(model: &NelsonSiegelModel, t: f64) -> crate::Result<(f64, f64)> {
    if exact_f64_eq(t, 0.0) {
        let short_rate = model.beta0() + model.beta1();
        if !short_rate.is_finite() {
            return Err(crate::Error::Validation(
                "parametric origin short rate overflowed f64".to_string(),
            ));
        }
        return Ok((0.0, short_rate));
    }

    let mut k_acc = NeumaierAccumulator::new();
    let mut f_acc = NeumaierAccumulator::new();

    match model.0 {
        RawNelsonSiegelModel::Ns {
            beta0,
            beta1,
            beta2,
            tau,
        } => {
            f_acc.add(beta0)?;
            k_acc.add(checked_scaled_product(beta0, t)?)?;
            add_scale_contribution(t, tau, beta1, beta2, &mut k_acc, &mut f_acc)?;
        }
        RawNelsonSiegelModel::Nss {
            beta0,
            beta1,
            beta2,
            beta3,
            tau1,
            tau2,
        } => {
            f_acc.add(beta0)?;
            k_acc.add(checked_scaled_product(beta0, t)?)?;
            if tau1.to_bits() == tau2.to_bits() {
                add_scale_contribution(t, tau1, beta1, beta2 + beta3, &mut k_acc, &mut f_acc)?;
            } else {
                add_scale_contribution(t, tau1, beta1, beta2, &mut k_acc, &mut f_acc)?;
                add_scale_contribution(t, tau2, 0.0, beta3, &mut k_acc, &mut f_acc)?;
            }
        }
    }

    let k = k_acc.get_total()?;
    let f = f_acc.get_total()?;
    if !k.is_finite() || !f.is_finite() {
        return Err(crate::Error::Validation(format!(
            "parametric curve evaluation overflowed f64 at t={t}"
        )));
    }
    Ok((
        if exact_f64_eq(k, 0.0) { 0.0 } else { k },
        if exact_f64_eq(f, 0.0) { 0.0 } else { f },
    ))
}

/// Parametric yield curve based on Nelson-Siegel or Nelson-Siegel-Svensson models.
///
/// Retains original model parameters, source clock, and optional accumulated
/// continuous-zero adjustments cleanly through conditioning and shocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "RawParametricCurve", into = "RawParametricCurve")]
pub struct ParametricCurve {
    id: CurveId,
    base_date: Date,
    day_count: DayCount,
    calendar: Option<CurveCalendar>,
    model: NelsonSiegelModel,
    source_base_date: Date,
    adjustment: PiecewiseLinearAdjustment,
    offset: f64,
}

/// Raw serializable state of ParametricCurve.
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct RawParametricCurve {
    /// Curve identifier.
    id: String,
    /// Base date.
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    base_date: Date,
    /// Day count convention.
    day_count: DayCount,
    /// Registered holiday calendar id for Bus/252.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    calendar_id: Option<String>,
    /// Validated Nelson-Siegel model parameters.
    model: NelsonSiegelModel,
    /// Original anchor date when the curve has been conditioned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<crate::wire::DateWire>")
    )]
    source_base_date: Option<Date>,
    /// Accumulated continuous-zero shock adjustment on the source clock.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    adjustment: Option<PiecewiseLinearAdjustment>,
}

impl From<ParametricCurve> for RawParametricCurve {
    fn from(curve: ParametricCurve) -> Self {
        let has_adj = !curve.adjustment.is_zero();
        RawParametricCurve {
            id: curve.id.to_string(),
            base_date: curve.base_date,
            day_count: curve.day_count,
            calendar_id: curve.calendar.map(|c| c.id),
            model: curve.model,
            source_base_date: (curve.source_base_date != curve.base_date)
                .then_some(curve.source_base_date),
            adjustment: has_adj.then_some(curve.adjustment),
        }
    }
}

impl TryFrom<RawParametricCurve> for ParametricCurve {
    type Error = crate::Error;

    fn try_from(raw: RawParametricCurve) -> crate::Result<Self> {
        let mut builder = ParametricCurve::builder(raw.id)
            .base_date(raw.base_date)
            .day_count(raw.day_count)
            .calendar_id_opt(raw.calendar_id)
            .model(raw.model)
            .source_base_date_opt(raw.source_base_date);
        builder.adjustment = raw.adjustment;
        builder.build()
    }
}

impl ParametricCurve {
    /// Start building a parametric curve with explicit required clock and model.
    #[must_use]
    pub fn builder(id: impl Into<CurveId>) -> ParametricCurveBuilder {
        ParametricCurveBuilder {
            id: id.into(),
            base_date: None,
            day_count: None,
            calendar_id: None,
            model: None,
            source_base_date: None,
            adjustment: None,
        }
    }

    /// Original untransformed anchor date for conditioned curves, or `base_date()` otherwise.
    #[must_use]
    #[inline]
    pub fn source_base_date(&self) -> Date {
        self.source_base_date
    }

    /// Access the validated model parameters.
    #[must_use]
    pub fn params(&self) -> &NelsonSiegelModel {
        &self.model
    }

    /// Canonical id of the curve's time-axis holiday calendar, if any.
    #[must_use]
    pub fn calendar_id(&self) -> Option<&str> {
        self.calendar.as_ref().map(|cal| cal.id.as_str())
    }

    /// Day-count context carrying the resolved calendar, if any.
    #[must_use]
    pub fn day_count_context(&self) -> DayCountContext<'static> {
        DayCountContext {
            calendar: self.calendar.as_ref().map(|cal| cal.calendar),
            ..DayCountContext::default()
        }
    }

    fn eval_inst_forward_at_source_time(&self, source_time: f64) -> crate::Result<f64> {
        if !source_time.is_finite() || source_time < 0.0 {
            return Err(crate::Error::Validation(format!(
                "ParametricCurve query time must lie on original source domain t >= 0, got source_time={source_time}"
            )));
        }
        let (_, f_src) = eval_source_kernel(&self.model, source_time)?;
        let adj_deriv = self.adjustment.value_with_left_interior(source_time, 0.0);
        let inst_fwd = f_src - adj_deriv;
        if !inst_fwd.is_finite() {
            return Err(crate::Error::Validation(format!(
                "ParametricCurve instantaneous forward overflowed at source_t={source_time}"
            )));
        }
        Ok(if exact_f64_eq(inst_fwd, 0.0) {
            0.0
        } else {
            inst_fwd
        })
    }

    fn eval_at_source_time(&self, source_time: f64) -> crate::Result<(f64, f64)> {
        if !source_time.is_finite() || source_time < 0.0 {
            return Err(crate::Error::Validation(format!(
                "ParametricCurve query time must lie on original source domain t >= 0, got source_time={source_time}"
            )));
        }
        let (k_src, f_src) = eval_source_kernel(&self.model, source_time)?;
        let (k_orig, _) = if exact_f64_eq(self.offset, 0.0) {
            (0.0, 0.0)
        } else {
            eval_source_kernel(&self.model, self.offset)?
        };
        let adj_int = self.adjustment.checked_integral(self.offset, source_time)?;
        let net_k = (k_src - k_orig) - adj_int;
        let df = (-net_k).exp();
        if !df.is_finite() || df <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "ParametricCurve discount factor overflowed or underflowed at source_t={source_time}"
            )));
        }
        let adj_deriv = self.adjustment.value_with_left_interior(source_time, 0.0);
        let inst_fwd = f_src - adj_deriv;
        if !inst_fwd.is_finite() {
            return Err(crate::Error::Validation(format!(
                "ParametricCurve instantaneous forward overflowed at source_t={source_time}"
            )));
        }
        Ok((
            if exact_f64_eq(source_time, self.offset) {
                1.0
            } else {
                df
            },
            if exact_f64_eq(inst_fwd, 0.0) {
                0.0
            } else {
                inst_fwd
            },
        ))
    }

    /// Discount factor at non-negative time `t` (years from `base_date`).
    ///
    /// # Arguments
    ///
    /// * `t` - Finite non-negative year fraction from [`base_date`](Self::base_date).
    ///
    /// # Errors
    ///
    /// Returns an error if `t` is non-finite or negative, or if the evaluated
    /// factor underflows to zero or overflows `f64`.
    #[inline]
    #[must_use = "computed discount factor should not be discarded"]
    pub fn df(&self, t: f64) -> crate::Result<f64> {
        if !t.is_finite() || t < 0.0 {
            return Err(crate::Error::Validation(format!(
                "ParametricCurve::df requires finite non-negative time t >= 0, got {t}"
            )));
        }
        Ok(self.eval_at_source_time(self.offset + t)?.0)
    }

    /// Discount factor on calendar date `date >= source_base_date()`.
    ///
    /// # Arguments
    ///
    /// * `date` - Target calendar date on or after [`source_base_date`](Self::source_base_date).
    ///
    /// # Errors
    ///
    /// Returns an error if `date` precedes [`source_base_date`](Self::source_base_date),
    /// day-count mapping fails, or factor evaluation fails.
    #[inline]
    #[must_use = "computed discount factor should not be discarded"]
    pub fn df_on_date(&self, date: Date) -> crate::Result<f64> {
        if date < self.source_base_date {
            return Err(crate::Error::Validation(format!(
                "ParametricCurve::df_on_date date ({date}) precedes source_base_date ({})",
                self.source_base_date
            )));
        }
        let source_t = signed_curve_year_fraction(
            self.source_base_date,
            date,
            self.day_count,
            self.calendar.as_ref(),
        )?;
        Ok(self.eval_at_source_time(source_t)?.0)
    }

    /// Zero rate at strictly positive horizon `t > 0` under `compounding`.
    ///
    /// # Arguments
    ///
    /// * `t` - Finite strictly positive year fraction from [`base_date`](Self::base_date).
    /// * `compounding` - Explicit compounding convention (`Continuous`,
    ///   `Periodic(m)`, or `Simple`).
    ///
    /// # Errors
    ///
    /// Returns an error if `t <= 0` or non-finite, discount-factor evaluation
    /// fails, or scalar rate conversion fails.
    #[inline]
    #[must_use = "computed zero rate should not be discarded"]
    pub fn zero_rate(&self, t: f64, compounding: Compounding) -> crate::Result<f64> {
        if !t.is_finite() || t <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "ParametricCurve::zero_rate requires finite strictly positive time t > 0, got {t}"
            )));
        }
        let df = self.df(t)?;
        compounding.rate_from_df(df, t)
    }

    /// Zero rate on calendar date `date > base_date()` under `compounding`.
    ///
    /// The horizon is the difference between the original source clock's
    /// coordinates for `date` and `base_date()`.
    ///
    /// # Arguments
    ///
    /// * `date` - Calendar date strictly after [`base_date`](Self::base_date).
    /// * `compounding` - Explicit compounding convention.
    ///
    /// # Errors
    ///
    /// Returns an error if mapped `t <= 0`, day-count mapping fails, or rate
    /// inversion fails.
    #[inline]
    #[must_use = "computed zero rate should not be discarded"]
    pub fn zero_rate_on_date(&self, date: Date, compounding: Compounding) -> crate::Result<f64> {
        let source_t = signed_curve_year_fraction(
            self.source_base_date,
            date,
            self.day_count,
            self.calendar.as_ref(),
        )?;
        let t = source_t - self.offset;
        if !t.is_finite() || t <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "ParametricCurve::zero_rate_on_date requires mapped horizon t > 0, got {t} at {date}"
            )));
        }
        let (df, _) = self.eval_at_source_time(source_t)?;
        compounding.rate_from_df(df, t)
    }

    /// Continuously compounded interval forward rate `-ln(DF(t2)/DF(t1)) / (t2 - t1)`.
    ///
    /// # Arguments
    ///
    /// * `t1` - Finite non-negative interval start time in years.
    /// * `t2` - Finite interval end time strictly greater than `t1`.
    ///
    /// # Errors
    ///
    /// Returns an error if `t1` or `t2` is non-finite, `t1 < 0.0`, `t2 <= t1`,
    /// or endpoint factor evaluation or log extraction fails.
    #[inline]
    #[must_use = "computed forward rate should not be discarded"]
    pub fn forward(&self, t1: f64, t2: f64) -> crate::Result<f64> {
        if !t1.is_finite() || !t2.is_finite() || t1 < 0.0 || t2 <= t1 {
            return Err(crate::Error::Validation(format!(
                "ParametricCurve::forward requires finite 0 <= t1 < t2, got [{t1}, {t2}]"
            )));
        }
        let df1 = self.df(t1)?;
        let df2 = self.df(t2)?;
        let dt = t2 - t1;
        let neg_log = stable_positive_log_ratio(df1, df2)?;
        let rate = neg_log / dt;
        if !rate.is_finite() || (!exact_f64_eq(neg_log, 0.0) && exact_f64_eq(rate, 0.0)) {
            return Err(crate::Error::Validation(format!(
                "ParametricCurve::forward overflowed or underflowed over [{t1}, {t2}]"
            )));
        }
        Ok(if exact_f64_eq(rate, 0.0) { 0.0 } else { rate })
    }

    /// Analytic instantaneous forward rate `-D'(t) / D(t)` at `t >= 0`.
    ///
    /// On an unshocked unconditioned curve at `t = 0.0`, evaluates `beta0 + beta1`.
    ///
    /// # Arguments
    ///
    /// * `t` - Finite non-negative year fraction from [`base_date`](Self::base_date).
    ///
    /// # Errors
    ///
    /// Returns an error if `t` is non-finite or negative, or if factor or
    /// derivative evaluation fails.
    #[inline]
    #[must_use = "computed instantaneous forward rate should not be discarded"]
    pub fn instantaneous_forward(&self, t: f64) -> crate::Result<f64> {
        if !t.is_finite() || t < 0.0 {
            return Err(crate::Error::Validation(format!(
                "ParametricCurve::instantaneous_forward requires finite t >= 0, got {t}"
            )));
        }
        self.eval_inst_forward_at_source_time(self.offset + t)
    }

    /// Apply a continuous-zero shock overlay in-place on the retained analytic source.
    pub(crate) fn bump_in_place(&mut self, spec: &BumpSpec) -> crate::Result<()> {
        let rate = spec.resolve_additive_or_error("ParametricCurve")?;
        if exact_f64_eq(rate, 0.0) {
            return Ok(());
        }
        let mut next = self.clone();
        next.adjustment
            .add_bump(self.offset, rate, spec.bump_type)?;
        next.adjustment.validate()?;
        if let crate::market_data::bumps::BumpType::TriangularKeyRate {
            prev_bucket,
            target_bucket,
            next_bucket,
        } = spec.bump_type
        {
            for rel_t in prev_bucket
                .into_iter()
                .chain([target_bucket])
                .chain(next_bucket)
            {
                let src_t = self.offset + rel_t;
                if src_t >= 0.0 {
                    let _ = next.eval_at_source_time(src_t)?;
                }
            }
        } else {
            let _ = next.df(1.0)?;
        }
        *self = next;
        Ok(())
    }

    /// Curve identifier.
    #[must_use]
    #[inline]
    pub fn id(&self) -> &CurveId {
        &self.id
    }

    /// Base date of the curve.
    #[must_use]
    #[inline]
    pub fn base_date(&self) -> Date {
        self.base_date
    }

    /// Day count convention.
    #[must_use]
    #[inline]
    pub fn day_count(&self) -> DayCount {
        self.day_count
    }

    /// Condition the curve to `reference_date >= source_base_date()`, retaining its
    /// original model, source clock, and continuous-zero shocks.
    ///
    /// # Arguments
    ///
    /// * `reference_date` - Target valuation anchor date on or after [`source_base_date`](Self::source_base_date).
    ///
    /// # Errors
    ///
    /// Returns an error if `reference_date < source_base_date()`, day-count
    /// calculation fails, or the discount factor at `reference_date` cannot be admitted.
    pub fn condition(&self, reference_date: Date) -> crate::Result<Self> {
        if reference_date < self.source_base_date {
            return Err(crate::Error::Validation(format!(
                "ParametricCurve::condition requires reference_date ({reference_date}) on or after source_base_date ({})",
                self.source_base_date
            )));
        }
        let new_offset = signed_curve_year_fraction(
            self.source_base_date,
            reference_date,
            self.day_count,
            self.calendar.as_ref(),
        )?;
        if !new_offset.is_finite() || new_offset < 0.0 {
            return Err(crate::Error::Validation(
                "conditioned offset must be finite and non-negative".to_string(),
            ));
        }
        let _ = self.eval_at_source_time(new_offset)?;
        if reference_date == self.base_date {
            return Ok(self.clone());
        }
        let mut conditioned = self.clone();
        conditioned.base_date = reference_date;
        conditioned.offset = new_offset;
        Ok(conditioned)
    }
}

impl Discounting for ParametricCurve {
    fn id(&self) -> &CurveId {
        &self.id
    }

    fn base_date(&self) -> Date {
        self.base_date
    }

    fn day_count(&self) -> DayCount {
        self.day_count
    }

    fn day_count_context(&self) -> DayCountContext<'_> {
        ParametricCurve::day_count_context(self)
    }

    fn df(&self, t: f64) -> crate::Result<f64> {
        ParametricCurve::df(self, t)
    }

    fn df_on_date(&self, date: Date) -> crate::Result<f64> {
        ParametricCurve::df_on_date(self, date)
    }

    fn instantaneous_forward(&self, t: f64) -> crate::Result<f64> {
        ParametricCurve::instantaneous_forward(self, t)
    }
}

impl Bumpable for ParametricCurve {
    /// Apply an additive continuous-zero shock on the retained analytic source.
    fn apply_bump(&self, spec: BumpSpec) -> crate::Result<Self> {
        let mut bumped = self.clone();
        bumped.id = spec.standard_bump_id(self.id());
        bumped.bump_in_place(&spec)?;
        Ok(bumped)
    }
}

/// Builder for [`ParametricCurve`].
pub struct ParametricCurveBuilder {
    id: CurveId,
    base_date: Option<Date>,
    day_count: Option<DayCount>,
    calendar_id: Option<String>,
    model: Option<NelsonSiegelModel>,
    source_base_date: Option<Date>,
    adjustment: Option<PiecewiseLinearAdjustment>,
}

impl ParametricCurveBuilder {
    /// Set the base date.
    pub fn base_date(mut self, date: Date) -> Self {
        self.base_date = Some(date);
        self
    }

    /// Set the explicit supported curve day-count convention.
    pub fn day_count(mut self, day_count: DayCount) -> Self {
        self.day_count = Some(day_count);
        self
    }

    /// Set the registered holiday calendar for `Bus/252`.
    pub fn calendar_id(mut self, calendar_id: impl Into<String>) -> Self {
        self.calendar_id = Some(calendar_id.into());
        self
    }

    /// Optionally set the holiday calendar id.
    pub fn calendar_id_opt(mut self, calendar_id: Option<String>) -> Self {
        self.calendar_id = calendar_id;
        self
    }

    /// Set the validated Nelson-Siegel model parameters.
    pub fn model(mut self, model: NelsonSiegelModel) -> Self {
        self.model = Some(model);
        self
    }

    fn source_base_date_opt(mut self, source_base_date: Option<Date>) -> Self {
        self.source_base_date = source_base_date;
        self
    }

    /// Build the parametric curve.
    ///
    /// # Errors
    ///
    /// Returns an error if `base_date`, `day_count`, or `model` was not
    /// supplied, if the day-count convention is unsupported or lacks a `Bus/252`
    /// calendar, if `base_date` precedes `source_base_date`, or if evaluating
    /// the normalization factor fails.
    pub fn build(self) -> crate::Result<ParametricCurve> {
        let base_date = self
            .base_date
            .ok_or(crate::Error::Validation("base_date is required".into()))?;
        let day_count = self
            .day_count
            .ok_or(crate::Error::Validation("day_count is required".into()))?;
        let model = self
            .model
            .ok_or(crate::Error::Validation("model is required".into()))?;
        model.validate()?;
        let calendar = validate_curve_clock(
            "ParametricCurve",
            self.id.as_str(),
            day_count,
            self.calendar_id.as_deref(),
        )?;

        let source_base_date = self.source_base_date.unwrap_or(base_date);
        if base_date < source_base_date {
            return Err(crate::Error::Validation(format!(
                "ParametricCurve base_date ({base_date}) cannot precede source_base_date ({source_base_date})"
            )));
        }
        let offset =
            signed_curve_year_fraction(source_base_date, base_date, day_count, calendar.as_ref())?;
        if !offset.is_finite() || offset < 0.0 {
            return Err(crate::Error::Validation(
                "ParametricCurve condition offset must be finite and non-negative".to_string(),
            ));
        }
        let adjustment = self.adjustment.unwrap_or_default();
        adjustment.validate()?;

        let curve = ParametricCurve {
            id: self.id,
            base_date,
            day_count,
            calendar,
            model,
            source_base_date,
            adjustment,
            offset,
        };
        let _ = curve.eval_at_source_time(offset)?;
        Ok(curve)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    fn base_date() -> Date {
        Date::from_calendar_date(2025, Month::January, 1).expect("valid date")
    }

    #[test]
    fn parametric_curve_discount_and_origin_short_rate() {
        let curve = ParametricCurve::builder("USD-NS")
            .base_date(base_date())
            .day_count(DayCount::Act365F)
            .model(NelsonSiegelModel::ns(0.03, -0.01, 0.005, 2.0).expect("valid model"))
            .build()
            .expect("valid curve");

        assert_eq!(curve.df(0.0).expect("origin df"), 1.0);
        assert!(curve.df(1.0).expect("1y df") < 1.0);
        assert!(curve.df(5.0).expect("5y df") < curve.df(1.0).expect("1y df"));
        assert!((curve.instantaneous_forward(0.0).expect("f(0)") - 0.02).abs() < 1e-15);
        assert!(curve.zero_rate(0.0, Compounding::Continuous).is_err());
        assert!(curve.df(-0.1).is_err());
        assert!(
            curve
                .df_on_date(base_date() - time::Duration::days(1))
                .is_err()
        );
    }

    #[test]
    fn nss_equal_taus_are_allowed() {
        let model = NelsonSiegelModel::nss(0.04, -0.02, 0.01, 0.005, 2.0, 2.0).expect("equal tau");
        let curve = ParametricCurve::builder("NSS-EQ")
            .base_date(base_date())
            .day_count(DayCount::Act365F)
            .model(model)
            .build()
            .expect("equal taus valid");
        let combined = ParametricCurve::builder("NS-COMB")
            .base_date(base_date())
            .day_count(DayCount::Act365F)
            .model(NelsonSiegelModel::ns(0.04, -0.02, 0.015, 2.0).expect("ns"))
            .build()
            .expect("combined ns");
        assert!((curve.df(3.0).expect("df") - combined.df(3.0).expect("df")).abs() < 1e-15);
    }

    #[test]
    fn extreme_x_ratio_avoids_inf_times_zero_nan() {
        let curve = ParametricCurve::builder("EXTREME-X")
            .base_date(base_date())
            .day_count(DayCount::Act365F)
            .model(NelsonSiegelModel::ns(0.03, -0.01, 0.02, 1e-305).expect("model"))
            .build()
            .expect("curve");
        // t / tau = 1e5 / 1e-305 -> 1e310 (overflows direct f64 quotient)
        let f = curve
            .instantaneous_forward(1e5)
            .expect("asymptotic forward");
        assert!((f - 0.03).abs() < 1e-15);
    }

    #[test]
    fn ns_and_nss_rolls_realize_discount_factors_and_survive_serde() {
        for model in [
            NelsonSiegelModel::ns(0.05, -0.03, 0.01, 1.2).expect("ns"),
            NelsonSiegelModel::nss(0.05, -0.03, 0.01, 0.004, 1.2, 5.0).expect("nss"),
        ] {
            let source = ParametricCurve::builder("NS-ROLL")
                .base_date(base_date())
                .day_count(DayCount::Act365F)
                .model(model)
                .build()
                .expect("source");
            let rolled = source
                .condition(source.base_date() + time::Duration::days(365))
                .expect("condition");
            assert!(
                (rolled.df(0.5).expect("rolled df")
                    - source.df(1.5).expect("df1.5") / source.df(1.0).expect("df1"))
                .abs()
                    < 1e-14
            );
            assert!(
                (rolled.instantaneous_forward(0.5).expect("fwd")
                    - source.instantaneous_forward(1.5).expect("src fwd"))
                .abs()
                    < 1e-14
            );

            let wire = serde_json::to_value(&rolled).expect("json");
            assert!(wire.get("time_offset").is_none());
            let restored: ParametricCurve = serde_json::from_value(wire).expect("restore");
            assert!((restored.df(0.5).expect("df") - rolled.df(0.5).expect("df")).abs() < 1e-14);
        }
    }

    #[test]
    fn conditioned_zero_rate_on_date_uses_source_clock_horizon() {
        use time::macros::date;

        let curve = ParametricCurve::builder("30U-360-NS-ROLL")
            .base_date(date!(2025 - 01 - 15))
            .day_count(DayCount::Thirty360)
            .model(NelsonSiegelModel::ns(0.04, 0.0, 0.0, 1.0).expect("flat curve model"))
            .build()
            .expect("curve")
            .condition(date!(2025 - 02 - 28))
            .expect("condition");
        let rate = curve
            .zero_rate_on_date(date!(2025 - 03 - 31), Compounding::Continuous)
            .expect("dated zero rate");
        assert!((rate - 0.04).abs() < 1e-13);
        assert!(
            curve
                .zero_rate_on_date(date!(2025 - 02 - 28), Compounding::Continuous)
                .is_err()
        );
    }

    #[test]
    fn parametric_curve_bumps_support_parallel_and_triangular() {
        let curve = ParametricCurve::builder("USD-NS")
            .base_date(base_date())
            .day_count(DayCount::Act365F)
            .model(NelsonSiegelModel::nss(0.03, -0.01, 0.005, 0.002, 1.5, 5.0).expect("nss"))
            .build()
            .expect("valid curve");

        let bumped = curve
            .apply_bump(BumpSpec::parallel_bp(25.0))
            .expect("parallel bump");
        assert_eq!(bumped.params().beta0(), 0.03);
        for t in [0.25, 1.0, 5.0, 10.0] {
            assert!(
                (bumped.zero_rate(t, Compounding::Continuous).expect("zr")
                    - (curve.zero_rate(t, Compounding::Continuous).expect("zr") + 0.0025))
                    .abs()
                    < 1e-14
            );
        }

        let kr_bumped = curve
            .apply_bump(BumpSpec::triangular_key_rate_bp(0.5, 1.0, 2.0, 10.0))
            .expect("key-rate bump");
        assert!(kr_bumped.df(1.0).expect("df") < curve.df(1.0).expect("df"));
    }
}
