//! Credit factor covariance and idiosyncratic-volatility forecasts.
//!
//! Sample and EWMA variance models are supported. `OneStep` and
//! `Unconditional` map to the calibrated annualized variance unchanged;
//! `NSteps(n)` means `n` annualized model periods and multiplies variance by
//! `n`; fractional calendar horizons use `Years(y)` or parser input
//! `{"n_steps": N, "periods_per_year": P}`. `VolHorizon::Custom` is
//! not exposed, so PyO3 / WASM bindings do not need to serialize arbitrary
//! scaling callables.
//!
//! # Reuse
//!
//! - Σ(t, h) is the calibrated [`CreditFactorModel::config`] covariance
//!   multiplied by the horizon in years, preserving its ridge or shrinkage estimator.
//! - Per-issuer idiosyncratic vol is sourced from
//!   [`super::hierarchy::VolState::idiosyncratic`].
//! - The factor universe is taken straight from
//!   [`CreditFactorModel::config.factors`] in canonical order.

use crate::factor::credit::hierarchy::{CreditFactorModel, IdiosyncraticVolModel};
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
    /// Long-run / unconditional horizon. For both sample variance
    /// and EWMA (a martingale variance forecast) the
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
    /// # Arguments
    ///
    /// * `s` - Horizon keyword or JSON descriptor. `N` counts non-negative whole
    ///   periods; `Y` is a finite non-negative year fraction; `P` is the finite,
    ///   positive number of observation periods per year used to convert `N`.
    ///
    /// # Errors
    ///
    /// Returns a human-readable error message for an unrecognized descriptor
    /// or an invalid year fraction, period count, or periods-per-year value.
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

    /// Validated multiplier for annualized variances and covariances.
    fn variance_scale(self) -> finstack_quant_core::Result<f64> {
        let scale = match self {
            Self::OneStep | Self::Unconditional => 1.0,
            // `n as f64` is exact for the small `n` we expect here. Casting
            // is intentional and lossless within usize values that fit in
            // f64 mantissa precision (53 bits ≈ 9e15).
            #[allow(clippy::cast_precision_loss)]
            Self::NSteps(n) => n as f64,
            Self::Years(years) => years,
        };
        if !scale.is_finite() || scale < 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "Forecast horizon must be finite and non-negative".into(),
            ));
        }
        Ok(scale)
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
    ///
    /// # Arguments
    ///
    /// * `model` - Calibrated artifact whose systematic covariance estimator
    ///   and issuer variance estimates are preserved when scaling horizons.
    #[must_use]
    pub fn new(model: &'a CreditFactorModel) -> Self {
        Self { model }
    }

    /// Scale the calibrated factor covariance matrix to the requested horizon.
    ///
    /// Sample and EWMA forecasts have flat variance term structures. Every
    /// calibrated covariance entry is multiplied by the horizon in years,
    /// preserving ridge regularization and Ledoit-Wolf shrinkage. `OneStep`
    /// and `Unconditional` return the calibrated annualized covariance unchanged.
    ///
    /// # Errors
    ///
    /// Returns a validation error when:
    /// - the calibrated covariance axes do not match `config.factors`,
    /// - the horizon is negative or non-finite,
    /// - the resulting matrix fails PSD validation in
    ///   [`FactorCovarianceMatrix::new`].
    ///
    /// # Arguments
    ///
    /// * `horizon` - Non-negative variance horizon; `Years(y)` uses calendar
    ///   years and `NSteps(n)` uses whole annualized periods, not panel observations.
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
        let scale = horizon.variance_scale()?;
        let covariance = &self.model.config.covariance;
        if covariance.factor_ids() != factor_ids.as_slice() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FactorCovarianceForecast: calibrated covariance factor axes do not match \
                     config.factors (got {} covariance ids, {} config factors)",
                covariance.n_factors(),
                factor_ids.len()
            )));
        }
        let data = covariance
            .as_slice()
            .iter()
            .map(|entry| entry * scale)
            .collect();
        FactorCovarianceMatrix::new(factor_ids, data)
    }

    /// Idiosyncratic vol (std dev) for a specific issuer at the requested
    /// horizon.
    ///
    /// # Errors
    ///
    /// Returns a validation error when the issuer is not present in
    /// `VolState::idiosyncratic`, the horizon is negative or non-finite, or the
    /// scaled variance is negative or non-finite.
    ///
    /// # Arguments
    ///
    /// * `issuer_id` - Calibrated issuer identifier to look up; missing issuers
    ///   return a validation error rather than zero risk.
    /// * `horizon` - Non-negative horizon scaling annualized bp-squared
    ///   variance; the returned standard deviation is in spread basis points.
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
        let scale = horizon.variance_scale()?;
        let variance = match model {
            // Both Sample and Ewma use the same horizon scaling
            // (see covariance_at for rationale).
            IdiosyncraticVolModel::Sample { variance }
            | IdiosyncraticVolModel::Ewma { variance, .. } => scale * variance,
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
    /// # Arguments
    ///
    /// * `horizon` - Non-negative covariance horizon. `Years(y)` scales the
    ///   annualized covariance by `y`; `NSteps(n)` represents `n` whole years.
    /// * `risk_measure` - Variance, volatility, VaR, or expected-shortfall
    ///   measure to store in the returned configuration. The measure is applied
    ///   by downstream decomposition and does not change covariance scaling.
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
        FactorCorrelationMatrix, FactorVolModel, GenericFactorSpec, HierarchyDimension,
        IssuerBetaPolicy, LevelsAtAnchor, VolState,
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
            vec![0.04, 0.02, 0.02, 0.04],
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
                bump_config: None,
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
    fn covariance_forecast_preserves_calibrated_ridge_and_ledoit_wolf(
    ) -> finstack_quant_core::Result<()> {
        use crate::factor::credit::calibration::{
            CovarianceStrategy, CreditCalibrationConfig, CreditCalibrationInputs, CreditCalibrator,
            GenericFactorSeries, HistoryPanel, IssuerTagPanel,
        };
        use crate::factor::credit::hierarchy::IssuerTags;

        let dates = vec![
            create_date(2024, Month::January, 28)?,
            create_date(2024, Month::February, 28)?,
            create_date(2024, Month::March, 28)?,
            create_date(2024, Month::April, 28)?,
        ];
        let issuer = IssuerId::new("A");
        for (strategy, values, expected) in [
            (CovarianceStrategy::Ridge { alpha: 4.0 }, vec![0.01; 4], 4.0),
            (
                CovarianceStrategy::LedoitWolf,
                vec![0.01, 0.011, 0.013, 0.013],
                800.0,
            ),
        ] {
            let model = CreditCalibrator::new(CreditCalibrationConfig {
                covariance_strategy: strategy,
                bucket_weighting: BucketWeighting::Equal,
                ..Default::default()
            })
            .calibrate(CreditCalibrationInputs {
                history_panel: HistoryPanel {
                    dates: dates.clone(),
                    spreads: BTreeMap::from([(
                        issuer.clone(),
                        values.iter().copied().map(Some).collect(),
                    )]),
                },
                issuer_tags: IssuerTagPanel {
                    tags: BTreeMap::from([(issuer.clone(), IssuerTags(BTreeMap::new()))]),
                },
                generic_factor: GenericFactorSeries {
                    spec: GenericFactorSpec {
                        name: "PC".into(),
                        series_id: "PC".into(),
                    },
                    values: values.clone(),
                },
                as_of: dates[3],
                as_of_spreads: BTreeMap::from([(issuer.clone(), values[3])]),
                idiosyncratic_overrides: BTreeMap::new(),
                spread_durations: BTreeMap::new(),
            })?;
            let forecast = FactorCovarianceForecast::new(&model);
            assert!((model.config.covariance.variance_at(0) - expected).abs() < 1e-10);
            for (horizon, scale) in [
                (VolHorizon::OneStep, 1.0),
                (VolHorizon::Unconditional, 1.0),
                (VolHorizon::Years(0.25), 0.25),
                (VolHorizon::NSteps(2), 2.0),
            ] {
                assert!(
                    (forecast.covariance_at(horizon)?.variance_at(0) - scale * expected).abs()
                        < 1e-10
                );
            }
        }
        Ok(())
    }

    #[test]
    fn zero_covariance_does_not_mask_an_invalid_horizon() {
        let mut model = fixture_model();
        model.config.covariance = FactorCovarianceMatrix::new(
            model.config.covariance.factor_ids().to_vec(),
            vec![0.0; 4],
        )
        .unwrap();
        for years in [-1.0, f64::NAN, f64::INFINITY] {
            assert!(FactorCovarianceForecast::new(&model)
                .covariance_at(VolHorizon::Years(years))
                .is_err());
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
