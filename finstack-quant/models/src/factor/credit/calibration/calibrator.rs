use std::collections::BTreeMap;

use finstack_quant_core::types::IssuerId;
use finstack_quant_core::Result;

use super::assemble::{
    anchor_levels, assemble_factor_model_config, build_diagnostics, build_factor_histories,
    build_factor_id_order, build_vol_state,
};
use super::config::CreditCalibrationConfig;
use super::inputs::CreditCalibrationInputs;
use super::inventory::{apply_fold_up, build_bucket_inventory};
use super::panel::{
    build_working_panel, classify_mode, convert_inputs_to_bp, issuer_bucket_weight_series,
};
use super::peel_fit::run_peel;
use super::statistics::{
    adder_vols_from_history, assign_adder_vol, build_peer_proxy_index, factor_variances,
};
use super::validation::{validate_calibration_config, validate_calibration_inputs, validation_err};
use crate::factor::credit::hierarchy::{
    CreditFactorModel, CreditFactorModelSchema, DateRange, IssuerBetaMode, IssuerBetaRow,
    IssuerBetas,
};

/// Deterministic calibrator that produces a [`CreditFactorModel`].
///
/// Construct with [`CreditCalibrator::new`], then run [`Self::calibrate`].
#[derive(Debug, Clone)]
pub struct CreditCalibrator {
    config: CreditCalibrationConfig,
}

impl CreditCalibrator {
    /// Wrap a configuration into a calibrator.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration object controlling validation, rounding, or solver behavior
    #[must_use]
    pub fn new(config: CreditCalibrationConfig) -> Self {
        Self { config }
    }

    /// The configuration this calibrator runs under.
    #[must_use]
    pub fn config(&self) -> &CreditCalibrationConfig {
        &self.config
    }

    /// Run the full calibration pipeline.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] when:
    /// - the calibration config fails validation (e.g. an out-of-range
    ///   [`VolModelChoice::Ewma`][super::super::calibration::VolModelChoice] `lambda`),
    /// - the inputs are structurally malformed (length mismatches, missing
    ///   `as_of` in the date grid, missing tags),
    /// - the assembled [`CreditFactorModel::validate`] check fails.
    ///
    /// # Arguments
    ///
    /// * `inputs` - History panel, tags, generic series, `as_of` cross-section,
    ///   and optional idiosyncratic-vol overrides. Spreads and the generic
    ///   series are **decimal** (`0.01` = 100 bp); they are converted to bp
    ///   immediately after validation. The panel must be a complete regular
    ///   grid of `config.panel_frequency` with no `None` issuer observations.
    pub fn calibrate(&self, inputs: CreditCalibrationInputs) -> Result<CreditFactorModel> {
        validate_calibration_config(&self.config)?;
        validate_calibration_inputs(&inputs, &self.config)?;
        let mut inputs = inputs;
        convert_inputs_to_bp(&mut inputs);

        // As-of cross-section weights, used only for the anchor peel (step 7).
        // The historical peel uses per-date weights built below so DTS
        // weighting stays contemporaneous instead of leaking the as-of
        // cross-section into every historical bucket mean.
        let anchor_weights = crate::factor::credit::peel::issuer_bucket_weights(
            self.config.bucket_weighting,
            inputs.history_panel.spreads.keys(),
            &inputs.spread_durations,
            &inputs.as_of_spreads,
        )
        .map_err(validation_err)?;

        // -- Structural validation of inputs. -------------------------------
        let dates = &inputs.history_panel.dates;
        if dates.is_empty() {
            return Err(validation_err(
                "CreditCalibrator: history_panel.dates is empty",
            ));
        }
        if inputs.generic_factor.values.len() != dates.len() {
            return Err(validation_err(format!(
                "CreditCalibrator: generic_factor.values length {} != dates length {}",
                inputs.generic_factor.values.len(),
                dates.len()
            )));
        }
        for (issuer, series) in &inputs.history_panel.spreads {
            if series.len() != dates.len() {
                return Err(validation_err(format!(
                    "CreditCalibrator: spread series for issuer {:?} has length {}, expected {}",
                    issuer.as_str(),
                    series.len(),
                    dates.len()
                )));
            }
        }

        let asof_idx = dates
            .iter()
            .position(|d| *d == inputs.as_of)
            .ok_or_else(|| {
                validation_err(format!(
                    "CreditCalibrator: as_of {:?} not present in history_panel.dates",
                    inputs.as_of
                ))
            })?;

        // -- 1. Mode classification. ----------------------------------------
        let mut modes: BTreeMap<IssuerId, IssuerBetaMode> = BTreeMap::new();
        for issuer in inputs.history_panel.spreads.keys() {
            let mode = classify_mode(
                &self.config.policy,
                issuer,
                &inputs.history_panel.spreads,
                &self.config.use_returns_or_levels,
            );
            modes.insert(issuer.clone(), mode);
        }

        // -- 2. Returns or levels. ------------------------------------------
        let panel = build_working_panel(
            &self.config.use_returns_or_levels,
            dates,
            &inputs.history_panel.spreads,
            &inputs.generic_factor.values,
        );

        // Per-date peel weights aligned to the working panel: equal `1.0`
        // everywhere, or contemporaneous DTS (`SD × begin-of-period spread`
        // under Returns, `SD × same-date spread` under Levels).
        let peel_weights = issuer_bucket_weight_series(
            self.config.bucket_weighting,
            &self.config.use_returns_or_levels,
            &inputs.history_panel.spreads,
            &inputs.spread_durations,
        )
        .map_err(validation_err)?;

        // -- 3. Bucket inventory + fold-up. ---------------------------------
        let inventory =
            build_bucket_inventory(&self.config.hierarchy, &inputs.issuer_tags.tags, &modes)?;
        let (folded, fold_ups) = apply_fold_up(&inventory, &self.config.min_bucket_size_per_level);
        let bucket_sizes_per_level = inventory.bucket_sizes_per_level.clone();
        let tag_taxonomy = inventory.tag_taxonomy.clone();

        // -- 4 + 5. PC peel + per-level peel. -------------------------------
        let peel_outcome = run_peel(
            &self.config,
            &panel,
            &modes,
            &inventory.bucket_paths,
            &folded,
            &peel_weights,
        );

        // All variance/correlation estimation operates on factor and adder
        // *moves*. In `Returns` space the peeled series already are moves; a
        // `Levels` panel is first-differenced here. Estimating dispersion on
        // raw levels and multiplying by the annualization factor is
        // dimensionally meaningless (a flat 150bp spread would report a huge
        // "variance" with zero actual change-vol), and it is the differencing
        // that keeps the covariance in the canonical
        // annualized-variance-of-moves units documented on
        // [`crate::factor::FactorCovarianceMatrix`].
        let (stat_factor_returns, stat_adder_series) = match self.config.use_returns_or_levels {
            super::config::PanelSpace::Returns => (
                peel_outcome.factor_returns.clone(),
                peel_outcome.adder_series.clone(),
            ),
            super::config::PanelSpace::Levels => (
                peel_outcome
                    .factor_returns
                    .iter()
                    .map(|(fid, series)| (fid.clone(), super::panel::diff_sparse(series)))
                    .collect(),
                peel_outcome
                    .adder_series
                    .iter()
                    .map(|(issuer, series)| (issuer.clone(), super::panel::diff_sparse(series)))
                    .collect(),
            ),
        };

        // -- 6. Adder series → idiosyncratic vol. ---------------------------
        // Compute from-history vols for every issuer with enough residual
        // observations, regardless of mode: under `GloballyOff` (the default
        // policy) all issuers are `BucketOnly`, and restricting this step to
        // `IssuerBeta` issuers would leave every idiosyncratic vol at the
        // hard-coded 0.0 fallback — silently zeroing issuer-specific risk.
        let from_history_vols = adder_vols_from_history(
            &stat_adder_series,
            self.config.vol_model,
            self.config.panel_frequency.annualization_factor(),
        );
        let peer_proxy_index = build_peer_proxy_index(
            &from_history_vols,
            &inventory.bucket_paths,
            self.config.hierarchy.levels.len(),
        );

        // -- 7. Anchor levels at as_of. -------------------------------------
        let generic_at_asof = inputs.generic_factor.values[asof_idx];
        let anchor = anchor_levels(
            &self.config.hierarchy,
            &inputs.as_of_spreads,
            &inputs.issuer_tags.tags,
            generic_at_asof,
            &peel_outcome.betas,
            &folded,
            &anchor_weights,
        )?;

        // -- 8. Per-factor variance forecast (sample or EWMA), over moves. --
        let factor_variances = factor_variances(
            &stat_factor_returns,
            self.config.vol_model,
            self.config.panel_frequency.annualization_factor(),
        );

        // -- Build issuer beta rows. ----------------------------------------
        let mut issuer_betas: Vec<IssuerBetaRow> = Vec::new();
        for issuer_id in inputs.history_panel.spreads.keys() {
            // Every issuer in `spreads` was classified in step 1 above, so this
            // lookup is by-construction `Some(_)`. Fall back to BucketOnly to
            // avoid `.expect()` (clippy::expect_used is `#[deny]` in this crate).
            let mode = modes
                .get(issuer_id)
                .copied()
                .unwrap_or(IssuerBetaMode::BucketOnly);
            let tags = inputs
                .issuer_tags
                .tags
                .get(issuer_id)
                .cloned()
                .unwrap_or_default();
            let betas = peel_outcome
                .betas
                .get(issuer_id)
                .cloned()
                .unwrap_or_else(|| IssuerBetas::unit(self.config.hierarchy.levels.len()));
            let adder_at_anchor = anchor.adder.get(issuer_id).copied().unwrap_or(0.0);
            let (adder_vol, adder_vol_source) = assign_adder_vol(
                issuer_id,
                &from_history_vols,
                &peer_proxy_index,
                &inventory.bucket_paths,
                &inputs.idiosyncratic_overrides,
                self.config.hierarchy.levels.len(),
            );
            let fit_quality = peel_outcome.fit_quality.get(issuer_id).cloned();
            let level_fit_quality = peel_outcome
                .level_fit_quality
                .get(issuer_id)
                .cloned()
                .unwrap_or_default();
            let spread_duration = inputs
                .spread_durations
                .get(issuer_id)
                .copied()
                .unwrap_or(1.0);
            issuer_betas.push(IssuerBetaRow {
                issuer_id: issuer_id.clone(),
                tags,
                mode,
                betas,
                adder_at_anchor,
                adder_vol_annualized: adder_vol,
                adder_vol_source,
                fit_quality,
                level_fit_quality,
                spread_duration,
            });
        }
        // BTreeMap iteration is already sorted by issuer_id, but be defensive.
        issuer_betas.sort_by(|a, b| a.issuer_id.as_str().cmp(b.issuer_id.as_str()));

        // -- 9. Correlation matrix and covariance assembly. -----------------
        let factor_id_order = build_factor_id_order(&peel_outcome.factor_returns);

        // -- 10. Assemble FactorModelConfig (correlations over moves). ------
        let (static_correlation, config) = assemble_factor_model_config(
            &factor_id_order,
            &factor_variances,
            &stat_factor_returns,
            &self.config.hierarchy,
            &issuer_betas,
            self.config.covariance_strategy,
            self.config.panel_frequency.annualization_factor(),
        )?;

        // -- 11. Diagnostics. -----------------------------------------------
        let diagnostics = build_diagnostics(
            &modes,
            bucket_sizes_per_level,
            fold_ups,
            &peel_outcome.fit_quality,
            tag_taxonomy,
        );

        // -- 12. Bundle artifact + final validate(). ------------------------
        let calibration_window = DateRange {
            start: *dates
                .first()
                .ok_or_else(|| validation_err("dates non-empty checked above"))?,
            end: *dates
                .last()
                .ok_or_else(|| validation_err("dates non-empty checked above"))?,
        };

        let factor_histories = Some(build_factor_histories(
            dates,
            &self.config.use_returns_or_levels,
            &peel_outcome.factor_returns,
        )?);
        let vol_state = build_vol_state(&factor_variances, &issuer_betas, self.config.vol_model);

        let model = CreditFactorModel {
            schema: CreditFactorModelSchema::CURRENT,
            as_of: inputs.as_of,
            calibration_window,
            policy: self.config.policy.clone(),
            generic_factor: inputs.generic_factor.spec.clone(),
            hierarchy: self.config.hierarchy.clone(),
            panel_frequency: self.config.panel_frequency,
            use_returns_or_levels: self.config.use_returns_or_levels,
            bucket_weighting: self.config.bucket_weighting,
            config,
            issuer_betas,
            anchor_state: anchor.levels,
            static_correlation,
            vol_state,
            factor_histories,
            diagnostics,
        };

        model.validate()?;
        Ok(model)
    }
}
