/// Monte Carlo sampling settings for one Merton PIK simulation.
///
/// These are the per-instrument Monte Carlo controls, resolved from the bond's
/// `instrument_pricing_overrides.model_config` by [`MertonMcRun::resolve`]; the
/// wire [`MertonMcConfig`](super::MertonMcConfig) carries only the model and grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MertonMcRun {
    /// Number of independent estimators; antithetic sampling simulates two
    /// paths per estimator.
    pub num_paths: usize,
    /// Root RNG seed; every estimator draws from its own stream of it.
    pub seed: u64,
    /// Whether each estimator pairs its path with the sign-flipped mirror.
    pub antithetic: bool,
}

impl MertonMcRun {
    /// Resolve sampling settings from an instrument's model configuration.
    ///
    /// `mc_paths` and `mc_antithetic` fall back to the embedded registry's
    /// valuations `merton_pik_bond` defaults. The seed is
    /// `derive_seed(instrument_id, mc_seed_scenario)`, with `"base"` when no
    /// label is set.
    ///
    /// # Arguments
    ///
    /// * `instrument_id` - Instrument identifier hashed into the seed.
    /// * `model_config` - The instrument's
    ///   `instrument_pricing_overrides.model_config` (`mc_paths`,
    ///   `mc_antithetic`, `mc_seed_scenario`).
    ///
    /// # Errors
    ///
    /// Returns a validation error when the registry cannot be loaded or
    /// `mc_paths` exceeds the workspace path cap.
    pub fn resolve(
        instrument_id: &finstack_quant_core::types::InstrumentId,
        model_config: &crate::instruments::pricing_overrides::ModelConfig,
    ) -> finstack_quant_core::Result<Self> {
        let defaults = &crate::pricer::defaults::embedded_defaults()?.merton_pik_bond;
        Ok(Self {
            num_paths: crate::instruments::common_impl::helpers::resolve_mc_paths(
                model_config.mc_paths,
                defaults.num_paths,
            )?,
            seed: model_config.mc_seed(instrument_id),
            antithetic: model_config.mc_antithetic.unwrap_or(defaults.antithetic),
        })
    }
}

/// Result from Monte Carlo PIK pricing.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MertonMcResult {
    /// Clean price as percentage of par: `dirty_price_pct` minus the accrued
    /// interest at the valuation date.
    pub clean_price_pct: f64,
    /// Dirty price as percentage of par: the mean simulated present value at
    /// the valuation date, including the full next coupon.
    pub dirty_price_pct: f64,
    /// Expected loss as fraction of PIK-aware risk-free PV.
    ///
    /// Defined as `1 - mean_mc_pv / risk_free_pv` where the risk-free PV
    /// accounts for the PIK schedule (accreted notional in the no-default
    /// scenario). For Toggle periods, the risk-free scenario assumes cash
    /// (zero hazard implies no PIK trigger).
    pub expected_loss: f64,
    /// Unexpected loss (standard deviation of path PVs / notional).
    pub unexpected_loss: f64,
    /// Expected shortfall at the 95% confidence level.
    pub expected_shortfall_95: f64,
    /// Average PIK fraction across all coupon dates and paths.
    pub average_pik_fraction: f64,
    /// Effective spread in basis points implied by MC price vs risk-free.
    pub effective_spread_bp: f64,
    /// Path-level statistics.
    pub path_statistics: PathStatistics,
    /// Number of independent estimators used (antithetic pairs count once).
    pub num_paths: usize,
    /// Standard error of the clean price estimate (percentage of par).
    pub standard_error: f64,
}

/// Path-level statistics from the Monte Carlo simulation.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PathStatistics {
    /// Fraction of paths that defaulted.
    pub default_rate: f64,
    /// Average default time (in years) among defaulted paths.
    pub avg_default_time: f64,
    /// Average terminal notional (reflects PIK accrual).
    pub avg_terminal_notional: f64,
    /// Average recovery rate (decimal fraction) among defaulted paths.
    pub avg_recovery_rate: f64,
    /// Fraction of coupon dates where PIK was elected.
    pub pik_exercise_rate: f64,
}

impl MertonMcResult {
    /// Project the headline pricing and path statistics into one row, retaining decimal price percentages, loss fractions and spread basis points.
    ///
    /// The independent-estimator count is a signed 64-bit column; counts beyond
    /// that range return an error. Path statistics retain their original units.
    pub fn to_table(
        &self,
    ) -> finstack_quant_core::Result<finstack_quant_core::table::TableEnvelope> {
        use finstack_quant_core::table::{TableColumn, TableColumnData, TableEnvelope};
        let mut columns = vec![
            TableColumn::new(
                "clean_price_pct",
                TableColumnData::Float64(vec![self.clean_price_pct]),
            ),
            TableColumn::new(
                "dirty_price_pct",
                TableColumnData::Float64(vec![self.dirty_price_pct]),
            ),
            TableColumn::new(
                "expected_loss",
                TableColumnData::Float64(vec![self.expected_loss]),
            ),
            TableColumn::new(
                "unexpected_loss",
                TableColumnData::Float64(vec![self.unexpected_loss]),
            ),
            TableColumn::new(
                "expected_shortfall_95",
                TableColumnData::Float64(vec![self.expected_shortfall_95]),
            ),
            TableColumn::new(
                "average_pik_fraction",
                TableColumnData::Float64(vec![self.average_pik_fraction]),
            ),
            TableColumn::new(
                "effective_spread_bp",
                TableColumnData::Float64(vec![self.effective_spread_bp]),
            ),
        ];
        columns.push(TableColumn::new(
            "num_paths",
            TableColumnData::Int64(vec![i64::try_from(self.num_paths).map_err(|_| {
                finstack_quant_core::Error::Validation("estimator count exceeds i64".into())
            })?]),
        ));
        columns.push(TableColumn::new(
            "standard_error",
            TableColumnData::Float64(vec![self.standard_error]),
        ));
        columns.push(TableColumn::new(
            "default_rate",
            TableColumnData::Float64(vec![self.path_statistics.default_rate]),
        ));
        columns.push(TableColumn::new(
            "avg_default_time",
            TableColumnData::Float64(vec![self.path_statistics.avg_default_time]),
        ));
        columns.push(TableColumn::new(
            "avg_terminal_notional",
            TableColumnData::Float64(vec![self.path_statistics.avg_terminal_notional]),
        ));
        columns.push(TableColumn::new(
            "avg_recovery_rate",
            TableColumnData::Float64(vec![self.path_statistics.avg_recovery_rate]),
        ));
        columns.push(TableColumn::new(
            "pik_exercise_rate",
            TableColumnData::Float64(vec![self.path_statistics.pik_exercise_rate]),
        ));
        TableEnvelope::new(columns)
    }
}
