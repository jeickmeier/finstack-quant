//! Deterministic fixtures shared by factor-model Criterion targets.
//!
//! No RNG crate and no clock. Inputs use the same decimal-spread convention
//! and monthly grid the calibrator validates.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(dead_code)]

use std::collections::BTreeMap;

use finstack_quant_core::dates::Date;
use finstack_quant_core::types::{Attributes, CurveId, IssuerId};
use finstack_quant_factor::credit::calibration::{
    BetaShrinkage, BucketSizeThresholds, BucketWeighting, CovarianceStrategy,
    CreditCalibrationConfig, CreditCalibrationInputs, CreditCalibrator, GenericFactorSeries,
    HistoryPanel, IssuerTagPanel, PanelFrequency, PanelSpace, VolModelChoice,
};
use finstack_quant_factor::credit::hierarchy::{
    AdderVolSource, CreditFactorModel, CreditHierarchySpec, GenericFactorSpec, HierarchyDimension,
    IssuerBetaMode, IssuerBetaPolicy, IssuerBetaRow, IssuerBetas, IssuerTags,
};
use finstack_quant_factor::matching::{
    AttributeFilter, CreditHierarchicalConfig, DependencyFilter, MappingRule, ISSUER_ID_META_KEY,
};
use finstack_quant_factor::{
    CurveType, DependencyType, FactorId, MarketDependency, SensitivityMatrix,
};
use time::Month;

/// Splitmix-style `u64` mapped into `[0, 1)`.
pub fn det_unit(seed_a: usize, seed_b: usize) -> f64 {
    let x = (seed_a.wrapping_mul(1_664_525).wrapping_add(1_013_904_223))
        ^ (seed_b
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407));
    (x & 0xFFFF) as f64 / 65535.0
}

/// `F0`, `F1`, … factor identifiers.
pub fn factor_ids(n: usize) -> Vec<FactorId> {
    (0..n).map(|i| FactorId::new(format!("F{i}"))).collect()
}

/// Symmetric PSD fixture: unit diagonal, off-diagonal `0.3 / (|i−j| + 1)`.
pub fn psd_matrix(n: usize) -> Vec<f64> {
    let mut data = vec![0.0; n * n];
    for i in 0..n {
        data[i * n + i] = 1.0;
        for j in (i + 1)..n {
            let cov = 0.3 / ((j - i) as f64 + 1.0);
            data[i * n + j] = cov;
            data[j * n + i] = cov;
        }
    }
    data
}

/// First-match-wins mapping-table rules, one curve id per rule.
pub fn mapping_rules(n: usize) -> Vec<MappingRule> {
    (0..n)
        .map(|i| MappingRule {
            dependency_filter: DependencyFilter {
                dependency_type: Some(DependencyType::Discount),
                curve_type: Some(CurveType::Discount),
                id: Some(format!("CURVE-{i}")),
            },
            attribute_filter: AttributeFilter::default(),
            factor_id: FactorId::new(format!("Factor-{i}")),
        })
        .collect()
}

/// Regular monthly grid starting 2020-01-28, matching [`PanelFrequency::Monthly`].
pub fn monthly_dates(n: usize) -> Vec<Date> {
    let origin = Date::from_calendar_date(2020, Month::January, 28).expect("valid origin");
    (0..n)
        .map(|i| {
            PanelFrequency::Monthly
                .date_after(origin, i32::try_from(i).expect("panel fits i32"))
                .expect("date in range")
        })
        .collect()
}

pub fn rating_label(idx: usize) -> &'static str {
    ["IG", "HY", "EM"][idx % 3]
}

pub fn region_label(idx: usize) -> &'static str {
    ["NA", "EU", "APAC"][idx % 3]
}

pub fn sector_label(idx: usize) -> &'static str {
    ["FIN", "UTIL", "TECH", "ENERGY", "HEALTH", "CONS"][idx % 6]
}

/// Three-level rating → region → sector spec.
pub fn three_level_spec() -> CreditHierarchySpec {
    CreditHierarchySpec {
        levels: vec![
            HierarchyDimension::Rating,
            HierarchyDimension::Region,
            HierarchyDimension::Sector,
        ],
    }
}

fn hierarchy_for_levels(n_levels: usize) -> CreditHierarchySpec {
    let mut levels = Vec::new();
    if n_levels >= 1 {
        levels.push(HierarchyDimension::Rating);
    }
    if n_levels >= 2 {
        levels.push(HierarchyDimension::Region);
    }
    if n_levels >= 3 {
        levels.push(HierarchyDimension::Sector);
    }
    CreditHierarchySpec { levels }
}

/// Synthetic credit panel plus the calibrator config that consumes it.
///
/// Spreads and the generic series are **decimal** (`0.01` = 100 bp).
#[derive(Clone)]
pub struct CreditBook {
    /// Number of issuers in [`Self::inputs`].
    pub n_issuers: usize,
    /// Observation count (monthly).
    pub n_months: usize,
    /// Hierarchy depth used to tag issuers.
    pub n_levels: usize,
    /// Calibrator inputs (decimal spreads, regular monthly grid).
    pub inputs: CreditCalibrationInputs,
    /// Calibrator configuration.
    pub config: CreditCalibrationConfig,
}

impl CreditBook {
    /// Build a complete regular panel.
    ///
    /// # Arguments
    ///
    /// * `n_issuers` - Issuer count; each gets a unique `ISSUER-{idx:04}` id.
    /// * `n_months` - Length of the monthly date grid and every series.
    /// * `n_levels` - Hierarchy depth in `1..=3` (rating / region / sector).
    pub fn new(n_issuers: usize, n_months: usize, n_levels: usize) -> Self {
        assert!(
            (1..=3).contains(&n_levels),
            "fixture hierarchy supports 1..=3 levels"
        );
        let dates = monthly_dates(n_months);
        let as_of = dates[n_months - 1];
        let generic: Vec<f64> = (0..n_months)
            .map(|i| 0.010 + 0.0005 * (i as f64 * 0.3).sin())
            .collect();

        let mut spreads = BTreeMap::new();
        let mut tags = BTreeMap::new();
        let mut as_of_spreads = BTreeMap::new();
        let mut spread_durations = BTreeMap::new();

        for idx in 0..n_issuers {
            let issuer = IssuerId::new(format!("ISSUER-{idx:04}"));
            let base = 0.008 + (idx % 20) as f64 * 0.001;
            let beta_pc = 0.4 + (idx % 10) as f64 * 0.06;
            let series: Vec<Option<f64>> = (0..n_months)
                .map(|t| {
                    let noise = 0.0002 * det_unit(idx, t) - 0.0001;
                    Some(base + beta_pc * (generic[t] - 0.010) + noise)
                })
                .collect();
            as_of_spreads.insert(issuer.clone(), series[n_months - 1].expect("dense panel"));
            spreads.insert(issuer.clone(), series);

            let mut tag_row = BTreeMap::new();
            if n_levels >= 1 {
                tag_row.insert("rating".to_owned(), rating_label(idx).to_owned());
            }
            if n_levels >= 2 {
                tag_row.insert("region".to_owned(), region_label(idx).to_owned());
            }
            if n_levels >= 3 {
                tag_row.insert("sector".to_owned(), sector_label(idx).to_owned());
            }
            tags.insert(issuer.clone(), IssuerTags(tag_row));
            spread_durations.insert(issuer, 4.0 + (idx % 5) as f64 * 0.5);
        }

        let inputs = CreditCalibrationInputs {
            history_panel: HistoryPanel { dates, spreads },
            issuer_tags: IssuerTagPanel { tags },
            generic_factor: GenericFactorSeries {
                spec: GenericFactorSpec {
                    name: "CDX IG 5Y".into(),
                    series_id: "cdx.ig.5y".into(),
                },
                values: generic,
            },
            as_of,
            as_of_spreads,
            idiosyncratic_overrides: BTreeMap::new(),
            spread_durations,
        };

        let config = CreditCalibrationConfig {
            policy: IssuerBetaPolicy::GloballyOff,
            hierarchy: hierarchy_for_levels(n_levels),
            min_bucket_size_per_level: BucketSizeThresholds {
                per_level: vec![2; n_levels],
            },
            vol_model: VolModelChoice::Sample,
            covariance_strategy: CovarianceStrategy::FullSampleRepaired,
            beta_shrinkage: BetaShrinkage::None,
            use_returns_or_levels: PanelSpace::Returns,
            panel_frequency: PanelFrequency::Monthly,
            bucket_weighting: BucketWeighting::Equal,
        };

        Self {
            n_issuers,
            n_months,
            n_levels,
            inputs,
            config,
        }
    }

    /// Representative production-shaped book: 50 issuers × 36 months × 3 levels.
    pub fn representative() -> Self {
        Self::new(50, 36, 3)
    }

    /// Override the covariance assembly strategy.
    #[must_use]
    pub fn with_strategy(mut self, strategy: CovarianceStrategy) -> Self {
        self.config.covariance_strategy = strategy;
        self
    }

    /// Override the issuer-beta policy.
    #[must_use]
    pub fn with_policy(mut self, policy: IssuerBetaPolicy) -> Self {
        self.config.policy = policy;
        self
    }

    /// Use DTS bucket weights (durations are already on the panel).
    #[must_use]
    pub fn with_dts(mut self) -> Self {
        self.config.bucket_weighting = BucketWeighting::Dts;
        self
    }

    /// Fit every issuer with enough history (`min_history` = 24 returns).
    #[must_use]
    pub fn with_issuer_beta(self) -> Self {
        self.with_policy(IssuerBetaPolicy::Dynamic {
            min_history: 24,
            overrides: BTreeMap::new(),
        })
    }

    /// Run [`CreditCalibrator::calibrate`] on a clone of the panel.
    pub fn calibrate(&self) -> CreditFactorModel {
        CreditCalibrator::new(self.config.clone())
            .calibrate(self.inputs.clone())
            .expect("calibration fixture must succeed")
    }
}

/// Hand-built credit-hierarchy matcher config with `n_issuers` sorted rows.
pub fn credit_hierarchical_config(n_issuers: usize) -> CreditHierarchicalConfig {
    let hierarchy = three_level_spec();
    let issuer_betas = (0..n_issuers)
        .map(|idx| {
            let mut tags = BTreeMap::new();
            tags.insert("rating".to_owned(), rating_label(idx).to_owned());
            tags.insert("region".to_owned(), region_label(idx).to_owned());
            tags.insert("sector".to_owned(), sector_label(idx).to_owned());
            IssuerBetaRow {
                issuer_id: IssuerId::new(format!("ISSUER-{idx:04}")),
                tags: IssuerTags(tags),
                mode: IssuerBetaMode::IssuerBeta,
                betas: IssuerBetas {
                    pc: 0.9,
                    levels: vec![0.85, 0.8, 0.75],
                },
                adder_at_anchor: 0.0,
                adder_vol_annualized: 0.01,
                adder_vol_source: AdderVolSource::Default,
                fit_quality: None,
                level_fit_quality: vec![],
                spread_duration: 5.0,
            }
        })
        .collect();
    CreditHierarchicalConfig {
        dependency_filter: DependencyFilter::default(),
        hierarchy,
        issuer_betas,
        require_issuer_id: true,
    }
}

/// Credit-curve dependency used by matcher benches.
pub fn credit_dependency(id: &str) -> MarketDependency {
    MarketDependency::CreditCurve {
        id: CurveId::new(id),
    }
}

/// Attributes carrying `credit::issuer_id` for a known calibrated issuer.
pub fn known_issuer_attrs(idx: usize) -> Attributes {
    Attributes::default().with_meta(ISSUER_ID_META_KEY, format!("ISSUER-{idx:04}"))
}

/// Unknown issuer with a complete namespaced tag set (bucket-only path).
pub fn unknown_issuer_attrs() -> Attributes {
    Attributes::default()
        .with_meta(ISSUER_ID_META_KEY, "NEWCO")
        .with_meta("credit::rating", "HY")
        .with_meta("credit::region", "NA")
        .with_meta("credit::sector", "TECH")
}

/// Zero matrix with `n_positions` string ids and `n_factors` `F{i}` columns.
pub fn zero_sensitivity_matrix(n_positions: usize, n_factors: usize) -> SensitivityMatrix {
    SensitivityMatrix::zeros(
        (0..n_positions).map(|i| format!("P{i}")).collect(),
        factor_ids(n_factors),
    )
}
