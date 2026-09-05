//! Credit factor covariance and idiosyncratic-volatility forecasts.
//!
//! The [`FactorVolModel::Sample`] and [`FactorVolModel::Ewma`] variants are supported. `OneStep` and
//! `Unconditional` map to the calibrated annualized variance unchanged;
//! `NSteps(n)` means `n` annualized model periods and multiplies variance by
//! `n`; fractional calendar horizons use `Years(y)` or parser input
//! `{"n_steps": N, "periods_per_year": P}`. `VolHorizon::Custom` is
//! not exposed, so PyO3 / WASM bindings do not need to serialize arbitrary
//! scaling callables.
//!
//! # Reuse
//!
//! - Σ(t, h) = D · ρ_static · D, with D = diag(σ_factor) and ρ_static taken
//!   straight from [`CreditFactorModel::static_correlation`].
//! - Per-issuer idiosyncratic vol is sourced from
//!   [`VolState::idiosyncratic`].
//! - The factor universe is taken straight from
//!   [`CreditFactorModel::config.factors`] in canonical order.

use crate::factor::credit::hierarchy::{CreditFactorModel, FactorVolModel, IdiosyncraticVolModel};
use crate::factor::{FactorCovarianceMatrix, FactorModelConfig, RiskMeasure};
use finstack_quant_core::types::IssuerId;

/// Forecast horizon used to scale a calibrated `Sample` vol estimate.
///
/// Supports annualized period counts and explicit fractional-year
/// horizons. The `Custom` variant from the design spec is not
/// exposed, so PyO3 / WASM bindings do not need to serialize arbitrary
/// scaling callables.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VolHorizon {
    /// One-period horizon. Returns the calibrated annualized variance
    /// unchanged (Sample model).
    OneStep,
    /// `n` annualized model periods. Variance scales linearly with `n`; vol
    /// therefore scales as `sqrt(n)` after the variance → vol conversion.
    /// `n = 0` returns zero variance.
    NSteps(usize),
    /// Fractional-year horizon. For example, 10 trading days from annualized
    /// variances should use `Years(10.0 / 252.0)` rather than `NSteps(10)`.
    Years(f64),
    /// Long-run / unconditional horizon. For both [`FactorVolModel::Sample`]
    /// and [`FactorVolModel::Ewma`] (a martingale variance forecast) the
    /// long-run variance equals the calibrated variance, so this is
    /// numerically identical to [`Self::OneStep`]. The variant is kept
    /// distinct so future mean-reverting estimators can override the
    /// behaviour without breaking existing call sites.
    Unconditional,
}

impl VolHorizon {
    /// Parse a horizon descriptor string into a [`VolHorizon`].
    ///
    /// Shared by the PyO3 and WASM binding crates so the accepted vocabulary
    /// stays in lockstep.
    ///
    /// Accepted forms (leading/trailing whitespace is trimmed):
    /// - `"one_step"` → [`VolHorizon::OneStep`]
    /// - `"unconditional"` → [`VolHorizon::Unconditional`]
    /// - a JSON object string `'{"n_steps": N}'` → [`VolHorizon::NSteps`]
    /// - a JSON object string `'{"years": Y}'` → [`VolHorizon::Years`]
    /// - a JSON object string `'{"n_steps": N, "periods_per_year": P}'`
    ///   → [`VolHorizon::Years`] with `Y = N / P` (MO-20)
    ///
    /// # Errors
    ///
    /// Returns a human-readable error message string if `s` is neither a
    /// recognized keyword nor a valid `{"n_steps": N}` JSON object.
    pub fn parse(s: &str) -> Result<VolHorizon, String> {
        match s.trim() {
            "one_step" => Ok(VolHorizon::OneStep),
            "unconditional" => Ok(VolHorizon::Unconditional),
            other => {
                // Try JSON object {"years": Y} or {"n_steps": N}.
                let v: serde_json::Value = serde_json::from_str(other).map_err(|_| {
                    format!(
                        "invalid horizon {other:?}: expected \"one_step\", \"unconditional\", \
                         {{\"years\": Y}}, or {{\"n_steps\": N}}"
                    )
                })?;
                if let Some(years) = v.get("years").and_then(serde_json::Value::as_f64) {
                    if !(years.is_finite() && years >= 0.0) {
                        return Err(format!(
                            "invalid horizon object {other:?}: years must be finite and non-negative"
                        ));
                    }
                    return Ok(VolHorizon::Years(years));
                }
                let n = v
                    .get("n_steps")
                    .and_then(serde_json::Value::as_u64)
                    .ok_or_else(|| {
                        format!(
                            "invalid horizon object {other:?}: expected {{\"years\": Y}} or \
                             {{\"n_steps\": N}}"
                        )
                    })?;
                if let Some(periods_per_year) = v
                    .get("periods_per_year")
                    .and_then(serde_json::Value::as_f64)
                {
                    if !(periods_per_year.is_finite() && periods_per_year > 0.0) {
                        return Err(format!(
                            "invalid horizon object {other:?}: periods_per_year must be finite and positive"
                        ));
                    }
                    return Ok(VolHorizon::Years(n as f64 / periods_per_year));
                }
                Ok(VolHorizon::NSteps(n as usize))
            }
        }
    }

    /// Apply this horizon's scaling rule to an annualized variance under the
    /// `Sample` vol model.
    fn scale_sample_variance(self, variance: f64) -> f64 {
        match self {
            Self::OneStep | Self::Unconditional => variance,
            // `n as f64` is exact for the small `n` we expect here. Casting
            // is intentional and lossless within usize values that fit in
            // f64 mantissa precision (53 bits ≈ 9e15).
            #[allow(clippy::cast_precision_loss)]
            Self::NSteps(n) => variance * (n as f64),
            Self::Years(years) => variance * years,
        }
    }
}

/// Vol-forecast view over a calibrated [`CreditFactorModel`].
///
/// The forecaster is a thin borrow over the model — it does no allocation
/// beyond what the requested horizon demands and does not mutate the
/// underlying artifact.
pub struct FactorCovarianceForecast<'a> {
    model: &'a CreditFactorModel,
}

impl<'a> FactorCovarianceForecast<'a> {
    /// Wrap a calibrated credit factor model for vol forecasting.
    #[must_use]
    pub fn new(model: &'a CreditFactorModel) -> Self {
        Self { model }
    }

    /// Build the factor covariance matrix `Σ(t, h) = D · ρ_static · D`.
    ///
    /// `D = diag(σ_factor)` where `σ_factor` is the square root of the
    /// horizon-scaled variance for each factor, in the same order as
    /// `CreditFactorModel::config::factors`.
    ///
    /// # Errors
    ///
    /// Returns a validation error when:
    /// - a factor in `config.factors` has no entry in `vol_state.factors`,
    /// - the static correlation matrix axes do not match `config.factors`,
    /// - any computed σ² is negative (data error in the artifact),
    /// - the resulting matrix fails PSD validation in
    ///   [`FactorCovarianceMatrix::new`].
    ///
    /// # Arguments
    ///
    /// * `horizon` - Horizon used by the algorithm, subject to the enclosing type invariants and documented units.
    pub fn covariance_at(
        &self,
        horizon: VolHorizon,
    ) -> finstack_quant_core::Result<FactorCovarianceMatrix> {
        let factor_ids: Vec<_> = self
            .model
            .config
            .factors
            .iter()
            .map(|f| f.id.clone())
            .collect();
        let n = factor_ids.len();

        let rho_ids = &self.model.static_correlation.factor_ids;
        if rho_ids.as_slice() != factor_ids.as_slice() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FactorCovarianceForecast: static_correlation factor axes do not match \
                     config.factors (got {} ρ ids, {} config factors)",
                rho_ids.len(),
                n
            )));
        }

        let mut sigma = Vec::with_capacity(n);
        for fid in &factor_ids {
            let vol_model = self.model.vol_state.factors.get(fid).ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "FactorCovarianceForecast: vol_state.factors is missing factor {fid}"
                ))
            })?;
            let variance = match vol_model {
                // Both Sample and Ewma (martingale variance forecast with flat
                // horizon term structure) use the same horizon scaling
                // (Longerstaey & Spencer 1996, §5.3).
                FactorVolModel::Sample { variance } | FactorVolModel::Ewma { variance, .. } => {
                    horizon.scale_sample_variance(*variance)
                }
            };
            if !variance.is_finite() || variance < 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "FactorCovarianceForecast: invalid variance {variance} for factor {fid}"
                )));
            }
            sigma.push(variance.sqrt());
        }

        // Σ[i][j] = σ_i · ρ[i][j] · σ_j (row-major flat).
        let mut data = vec![0.0_f64; n * n];
        for i in 0..n {
            // Hoist the correlation row out of the inner loop so the nested
            // `Vec<Vec<f64>>` is dereferenced once per row, not once per element.
            let rho_row = &self.model.static_correlation.data[i];
            let sigma_i = sigma[i];
            for j in 0..n {
                data[i * n + j] = sigma_i * rho_row[j] * sigma[j];
            }
        }

        FactorCovarianceMatrix::new(factor_ids, data)
    }

    /// Idiosyncratic vol (std dev) for a specific issuer at the requested
    /// horizon.
    ///
    /// # Errors
    ///
    /// Returns a validation error when the issuer is not present in
    /// `VolState::idiosyncratic` or the calibrated variance is negative.
    pub fn idiosyncratic_vol(
        &self,
        issuer_id: &IssuerId,
        horizon: VolHorizon,
    ) -> finstack_quant_core::Result<f64> {
        let model = self
            .model
            .vol_state
            .idiosyncratic
            .get(issuer_id)
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "FactorCovarianceForecast: no idiosyncratic vol model for issuer {}",
                    issuer_id.as_str()
                ))
            })?;
        let variance = match model {
            // Both Sample and Ewma use the same horizon scaling
            // (see covariance_at for rationale).
            IdiosyncraticVolModel::Sample { variance }
            | IdiosyncraticVolModel::Ewma { variance, .. } => {
                horizon.scale_sample_variance(*variance)
            }
        };
        if !variance.is_finite() || variance < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FactorCovarianceForecast: invalid idiosyncratic variance {variance} for \
                     issuer {}",
                issuer_id.as_str()
            )));
        }
        Ok(variance.sqrt())
    }

    /// Build a factor-model config using `Σ(t, h)` at the given
    /// horizon and requested risk measure.
    ///
    /// # Errors
    ///
    /// Returns a validation error when [`Self::covariance_at`] fails.
    pub fn factor_model_config_at(
        &self,
        horizon: VolHorizon,
        risk_measure: RiskMeasure,
    ) -> finstack_quant_core::Result<FactorModelConfig> {
        let covariance = self.covariance_at(horizon)?;
        let mut config = self.model.config.clone();
        config.covariance = covariance;
        config.risk_measure = risk_measure;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::factor::credit::calibration::{BucketWeighting, PanelFrequency, PanelSpace};
    use crate::factor::credit::hierarchy::{
        CalibrationDiagnostics, CreditFactorModelSchema, CreditHierarchySpec, DateRange,
        FactorCorrelationMatrix, GenericFactorSpec, HierarchyDimension, IssuerBetaPolicy,
        LevelsAtAnchor, VolState,
    };
    use crate::factor::{
        FactorDefinition, FactorId, FactorType, MarketMapping, MatchingConfig, PricingMode,
    };
    use finstack_quant_core::dates::create_date;
    use finstack_quant_core::market_data::bumps::BumpUnits;
    use finstack_quant_core::types::CurveId;
    use std::collections::BTreeMap;
    use time::Month;

    fn fixture_model() -> CreditFactorModel {
        let rates = FactorId::new("Rates");
        let credit = FactorId::new("Credit");
        let factors = vec![
            FactorDefinition {
                id: rates.clone(),
                factor_type: FactorType::Rates,
                market_mapping: MarketMapping::CurveParallel {
                    curve_ids: vec![CurveId::new("USD-OIS")],
                    units: BumpUnits::RateBp,
                },
                description: None,
            },
            FactorDefinition {
                id: credit.clone(),
                factor_type: FactorType::Credit,
                market_mapping: MarketMapping::CurveParallel {
                    curve_ids: vec![CurveId::new("CDX-IG")],
                    units: BumpUnits::RateBp,
                },
                description: None,
            },
        ];
        let static_correlation = FactorCorrelationMatrix::new(
            vec![rates.clone(), credit.clone()],
            vec![vec![1.0, 0.5], vec![0.5, 1.0]],
        )
        .expect("valid correlation fixture");
        let covariance = FactorCovarianceMatrix::new(
            vec![rates.clone(), credit.clone()],
            vec![1.0, 0.0, 0.0, 1.0],
        )
        .expect("valid covariance fixture");
        let mut factor_vols = BTreeMap::new();
        factor_vols.insert(rates, FactorVolModel::Sample { variance: 0.04 });
        factor_vols.insert(credit, FactorVolModel::Sample { variance: 0.04 });

        CreditFactorModel {
            schema: CreditFactorModelSchema::CURRENT,
            as_of: create_date(2024, Month::March, 29).expect("valid date"),
            calibration_window: DateRange {
                start: create_date(2022, Month::March, 29).expect("valid date"),
                end: create_date(2024, Month::March, 29).expect("valid date"),
            },
            policy: IssuerBetaPolicy::GloballyOff,
            generic_factor: GenericFactorSpec {
                name: "CDX IG 5Y".to_owned(),
                series_id: "cdx.ig.5y".to_owned(),
            },
            hierarchy: CreditHierarchySpec {
                levels: vec![HierarchyDimension::Rating, HierarchyDimension::Sector],
            },
            panel_frequency: PanelFrequency::Monthly,
            use_returns_or_levels: PanelSpace::Returns,
            bucket_weighting: BucketWeighting::Equal,
            config: FactorModelConfig {
                factors,
                covariance,
                matching: MatchingConfig::MappingTable(vec![]),
                pricing_mode: PricingMode::DeltaBased,
                risk_measure: RiskMeasure::Variance,
                bump_size: None,
                unmatched_policy: None,
            },
            issuer_betas: vec![],
            anchor_state: LevelsAtAnchor {
                pc: 0.0,
                by_level: vec![],
            },
            static_correlation,
            vol_state: VolState {
                factors: factor_vols,
                idiosyncratic: BTreeMap::new(),
            },
            factor_histories: None,
            diagnostics: CalibrationDiagnostics {
                mode_counts: BTreeMap::new(),
                bucket_sizes_per_level: vec![],
                fold_ups: vec![],
                r_squared_histogram: None,
                tag_taxonomy: BTreeMap::new(),
            },
        }
    }

    #[test]
    fn covariance_forecast_is_psd_and_scales_by_horizon() {
        let model = fixture_model();
        let forecast = FactorCovarianceForecast::new(&model);
        let one = forecast
            .covariance_at(VolHorizon::OneStep)
            .expect("one-step covariance");
        let four = forecast
            .covariance_at(VolHorizon::NSteps(4))
            .expect("four-step covariance");

        for (actual, expected) in one.as_slice().iter().zip([0.04, 0.02, 0.02, 0.04]) {
            assert!((actual - expected).abs() < 1e-12);
        }
        assert!(one.as_slice()[0] * one.as_slice()[3] - one.as_slice()[1].powi(2) >= 0.0);
        for (one_value, four_value) in one.as_slice().iter().zip(four.as_slice()) {
            assert!((four_value - 4.0 * one_value).abs() < 1e-12);
        }
    }

    #[test]
    fn idiosyncratic_forecast_uses_issuer_state() {
        let mut model = fixture_model();
        let issuer = IssuerId::new("ACME");
        model.vol_state.idiosyncratic.insert(
            issuer.clone(),
            IdiosyncraticVolModel::Sample { variance: 0.09 },
        );
        let forecast = FactorCovarianceForecast::new(&model);

        let vol = forecast
            .idiosyncratic_vol(&issuer, VolHorizon::NSteps(4))
            .expect("issuer forecast");
        assert!((vol - 0.6).abs() < 1e-12);
        assert!(forecast
            .idiosyncratic_vol(&IssuerId::new("MISSING"), VolHorizon::OneStep)
            .is_err());
    }

    #[test]
    fn horizon_parser_accepts_fractional_years_and_rejects_invalid_input() {
        assert_eq!(
            VolHorizon::parse(r#"{"n_steps": 10, "periods_per_year": 252}"#)
                .expect("valid fractional horizon"),
            VolHorizon::Years(10.0 / 252.0)
        );
        assert!(VolHorizon::parse(r#"{"years": -0.1}"#).is_err());
        assert!(VolHorizon::parse("unknown").is_err());
    }
}
