//! Sensitivities configuration (user-facing defaults) resolved from `FinstackConfig` extensions.
//!
//! This module defines the versioned extension section:
//! - `valuations.sensitivities.v1`
//!
//! It is intended for settings that users commonly tweak and need to persist for reproducible
//! pipelines (Tier-3 configuration in the Rust code standards).

use finstack_quant_core::config::FinstackConfig;

/// Extension section key for sensitivities defaults.
pub(crate) const SENSITIVITIES_CONFIG_KEY_V1: &str = "valuations.sensitivities.v1";

/// Standard risk bucket grid in years used for IR DV01 and credit CS01.
pub const STANDARD_BUCKETS_YEARS: [f64; 11] =
    [0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 15.0, 20.0, 30.0];

/// Standard bucket labels corresponding to [`STANDARD_BUCKETS_YEARS`].
pub const STANDARD_BUCKET_LABELS: [&str; 11] = [
    "3m", "6m", "1y", "2y", "3y", "5y", "7y", "10y", "15y", "20y", "30y",
];

/// Format a bucket time (in years) as a human-readable label.
///
/// For standard buckets (0.25, 0.5, 1, 2, 3, 5, 7, 10, 15, 20, 30 years),
/// returns the canonical label ("3m", "6m", "1y", etc.).
/// For non-standard buckets, dynamically formats as "{N}m" or "{N}y".
///
/// # Examples
///
/// ```
/// use finstack_quant_valuations::metrics::format_bucket_label;
///
/// assert_eq!(format_bucket_label(0.25), "3m");
/// assert_eq!(format_bucket_label(1.0), "1y");
/// assert_eq!(format_bucket_label(10.0), "10y");
/// assert_eq!(format_bucket_label(0.75), "9m"); // non-standard
/// ```
#[inline]
pub fn format_bucket_label(years: f64) -> String {
    format_bucket_label_cow(years).into_owned()
}

/// Same as [`format_bucket_label`] but returns `Cow<'static, str>` to avoid
/// allocation for the 11 standard buckets (the common case in DV01/CS01 loops).
#[inline]
pub(crate) fn format_bucket_label_cow(years: f64) -> std::borrow::Cow<'static, str> {
    for (i, &bucket_time) in STANDARD_BUCKETS_YEARS.iter().enumerate() {
        if (years - bucket_time).abs() < 0.01 {
            return std::borrow::Cow::Borrowed(STANDARD_BUCKET_LABELS[i]);
        }
    }

    let s = if years < 1.0 {
        format!("{:.0}m", (years * 12.0).round())
    } else {
        format!("{:.0}y", years)
    };
    std::borrow::Cow::Owned(s)
}

/// Resolved (fully-populated) sensitivities configuration.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SensitivitiesConfig {
    /// Interest rate bump size in basis points (e.g., 1.0 = 1bp).
    pub(crate) rate_bump_bp: f64,
    /// Credit spread bump size in basis points (e.g., 1.0 = 1bp).
    pub(crate) credit_spread_bump_bp: f64,
    /// Spot bump size as a percentage (e.g., 0.01 = 1%).
    pub(crate) spot_bump_pct: f64,
    /// Vol bump size (absolute) as a percentage (e.g., 0.01 = 1% vol).
    pub(crate) vol_bump_pct: f64,
    /// Default DV01 key-rate buckets in years.
    pub(crate) dv01_buckets_years: Vec<f64>,
    /// Default CS01 key-rate buckets in years.
    pub(crate) cs01_buckets_years: Vec<f64>,
    /// When `true`, sensitivity calculators that rescale a bucketed
    /// decomposition to match a parallel total (currently `BucketedVega`)
    /// return an error if the rescaling factor deviates from `1.0` by more
    /// than the calculator's documented threshold, rather than silently
    /// applying the rescale and emitting a `tracing::warn!`. Defaults to
    /// `false` to preserve historical behaviour.
    pub(crate) bucketed_rescale_strict: bool,
}

impl Default for SensitivitiesConfig {
    fn default() -> Self {
        Self {
            rate_bump_bp: 1.0,
            credit_spread_bump_bp: 1.0,
            spot_bump_pct: crate::metrics::bump_sizes::SPOT,
            vol_bump_pct: crate::metrics::bump_sizes::VOLATILITY,
            dv01_buckets_years: STANDARD_BUCKETS_YEARS.to_vec(),
            cs01_buckets_years: STANDARD_BUCKETS_YEARS.to_vec(),
            bucketed_rescale_strict: false,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SensitivitiesConfigV1 {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) rate_bump_bp: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) credit_spread_bump_bp: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) spot_bump_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) vol_bump_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) dv01_buckets_years: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) cs01_buckets_years: Option<Vec<f64>>,
    /// See `SensitivitiesConfig::bucketed_rescale_strict`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) bucketed_rescale_strict: Option<bool>,
}

fn ensure_finite_positive(name: &str, v: f64) -> finstack_quant_core::Result<()> {
    if !v.is_finite() || v <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Invalid sensitivities config: '{name}' must be finite and > 0, got {v}"
        )));
    }
    Ok(())
}

fn ensure_bucket_grid(name: &str, buckets: &[f64]) -> finstack_quant_core::Result<()> {
    if buckets.is_empty() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Invalid sensitivities config: '{name}' must be non-empty"
        )));
    }
    for (i, &t) in buckets.iter().enumerate() {
        if !t.is_finite() || t <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Invalid sensitivities config: '{name}[{i}]' must be finite and > 0, got {t}"
            )));
        }
        if i > 0 && t <= buckets[i - 1] {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Invalid sensitivities config: '{name}' must be strictly increasing (got {prev} then {t})",
                prev = buckets[i - 1]
            )));
        }
    }
    Ok(())
}

/// Resolve sensitivities defaults from a `FinstackConfig`.
///
/// If the extension section `valuations.sensitivities.v1` is present, its fields override
/// the defaults; otherwise defaults are used.
pub(crate) fn from_finstack_config_or_default(
    cfg: &FinstackConfig,
) -> finstack_quant_core::Result<SensitivitiesConfig> {
    let mut base = SensitivitiesConfig::default();

    if let Some(raw) = cfg.extensions.get(SENSITIVITIES_CONFIG_KEY_V1) {
        let overrides: SensitivitiesConfigV1 =
            serde_json::from_value(raw.clone()).map_err(|e| {
                finstack_quant_core::Error::Calibration {
                    message: format!(
                        "Failed to parse extension '{}': {}",
                        SENSITIVITIES_CONFIG_KEY_V1, e
                    ),
                    category: "config".to_string(),
                }
            })?;

        if let Some(v) = overrides.rate_bump_bp {
            ensure_finite_positive("rate_bump_bp", v)?;
            base.rate_bump_bp = v;
        }
        if let Some(v) = overrides.credit_spread_bump_bp {
            ensure_finite_positive("credit_spread_bump_bp", v)?;
            base.credit_spread_bump_bp = v;
        }
        if let Some(v) = overrides.spot_bump_pct {
            ensure_finite_positive("spot_bump_pct", v)?;
            base.spot_bump_pct = v;
        }
        if let Some(v) = overrides.vol_bump_pct {
            ensure_finite_positive("vol_bump_pct", v)?;
            base.vol_bump_pct = v;
        }
        if let Some(v) = overrides.dv01_buckets_years {
            ensure_bucket_grid("dv01_buckets_years", &v)?;
            base.dv01_buckets_years = v;
        }
        if let Some(v) = overrides.cs01_buckets_years {
            ensure_bucket_grid("cs01_buckets_years", &v)?;
            base.cs01_buckets_years = v;
        }
        if let Some(v) = overrides.bucketed_rescale_strict {
            base.bucketed_rescale_strict = v;
        }
    }

    Ok(base)
}

/// Resolve sensitivities defaults and then layer instrument-level pricing overrides.
pub(crate) fn from_context_or_default(
    cfg: &FinstackConfig,
    pricing_overrides: Option<&crate::instruments::MetricPricingOverrides>,
) -> finstack_quant_core::Result<SensitivitiesConfig> {
    let base = from_finstack_config_or_default(cfg)?;
    apply_pricing_overrides(base, pricing_overrides)
}

/// Apply per-instrument pricing overrides to a resolved sensitivities config.
pub(crate) fn apply_pricing_overrides(
    mut base: SensitivitiesConfig,
    pricing_overrides: Option<&crate::instruments::MetricPricingOverrides>,
) -> finstack_quant_core::Result<SensitivitiesConfig> {
    let Some(po) = pricing_overrides else {
        return Ok(base);
    };

    if let Some(v) = po
        .bump_config
        .rate_bump_bp
        .or_else(|| po.bump_config.rho_bump_decimal.map(|x| x * 10_000.0))
    {
        ensure_finite_positive("pricing_overrides.rate_bump_bp", v)?;
        base.rate_bump_bp = v;
    }
    if let Some(v) = po.bump_config.credit_spread_bump_bp {
        ensure_finite_positive("pricing_overrides.credit_spread_bump_bp", v)?;
        base.credit_spread_bump_bp = v;
    }
    if let Some(v) = po.bump_config.spot_bump_pct {
        ensure_finite_positive("pricing_overrides.spot_bump_pct", v)?;
        base.spot_bump_pct = v;
    }
    if let Some(v) = po
        .bump_config
        .vol_bump_pct
        .or(po.bump_config.vega_bump_decimal)
    {
        ensure_finite_positive("pricing_overrides.vol_bump_pct", v)?;
        base.vol_bump_pct = v;
    }

    Ok(base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn apply_pricing_overrides_prefers_explicit_fields() {
        let base = SensitivitiesConfig::default();
        let po = crate::instruments::MetricPricingOverrides {
            bump_config: crate::instruments::BumpConfig::default(),
            mc_seed_scenario: None,
            theta_period: None,
            breakeven_config: None,
            bond_risk_basis: None,
            var_config: None,
        }
        .with_rate_bump(2.0)
        .with_credit_spread_bump(3.0)
        .with_spot_bump(0.02)
        .with_vol_bump(0.03);

        let resolved = apply_pricing_overrides(base, Some(&po)).expect("valid overrides");
        assert_eq!(resolved.rate_bump_bp, 2.0);
        assert_eq!(resolved.credit_spread_bump_bp, 3.0);
        assert_eq!(resolved.spot_bump_pct, 0.02);
        assert_eq!(resolved.vol_bump_pct, 0.03);
    }

    #[test]
    fn apply_pricing_overrides_uses_fallback_units() {
        let base = SensitivitiesConfig::default();
        let mut po = crate::instruments::MetricPricingOverrides::default();
        po.bump_config.rho_bump_decimal = Some(0.0002);
        po.bump_config.vega_bump_decimal = Some(0.015);

        let resolved = apply_pricing_overrides(base, Some(&po)).expect("valid overrides");
        assert_eq!(resolved.rate_bump_bp, 2.0);
        assert_eq!(resolved.vol_bump_pct, 0.015);
    }

    #[test]
    fn apply_pricing_overrides_rejects_non_positive_values() {
        let base = SensitivitiesConfig::default();
        let po = crate::instruments::MetricPricingOverrides::default().with_rate_bump(0.0);

        let err = apply_pricing_overrides(base, Some(&po)).expect_err("must fail");
        assert!(
            err.to_string().contains("must be finite and > 0"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn format_bucket_label_covers_standard_and_dynamic_values() {
        assert_eq!(format_bucket_label(0.25), "3m");
        assert_eq!(format_bucket_label(10.0), "10y");
        assert_eq!(format_bucket_label(0.75), "9m");
        assert_eq!(format_bucket_label(1.4), "1y");
        assert_eq!(
            format_bucket_label_cow(5.0),
            std::borrow::Cow::Borrowed("5y")
        );
        assert_eq!(format_bucket_label_cow(1.25).into_owned(), "1y");
    }

    #[test]
    fn from_finstack_config_or_default_uses_defaults_without_extension() {
        let cfg = FinstackConfig::default();
        let resolved = from_finstack_config_or_default(&cfg).expect("default config should parse");

        assert_eq!(resolved, SensitivitiesConfig::default());
    }

    #[test]
    fn from_finstack_config_or_default_applies_valid_overrides() {
        let mut cfg = FinstackConfig::default();
        cfg.extensions.insert(
            SENSITIVITIES_CONFIG_KEY_V1,
            json!({
                "rate_bump_bp": 2.5,
                "credit_spread_bump_bp": 3.0,
                "spot_bump_pct": 0.02,
                "vol_bump_pct": 0.03,
                "dv01_buckets_years": [0.5, 1.0, 5.0],
                "cs01_buckets_years": [1.0, 3.0, 7.0]
            }),
        );

        let resolved = from_finstack_config_or_default(&cfg).expect("overrides should parse");
        assert_eq!(resolved.rate_bump_bp, 2.5);
        assert_eq!(resolved.credit_spread_bump_bp, 3.0);
        assert_eq!(resolved.spot_bump_pct, 0.02);
        assert_eq!(resolved.vol_bump_pct, 0.03);
        assert_eq!(resolved.dv01_buckets_years, vec![0.5, 1.0, 5.0]);
        assert_eq!(resolved.cs01_buckets_years, vec![1.0, 3.0, 7.0]);
    }

    #[test]
    fn from_finstack_config_or_default_rejects_parse_and_grid_errors() {
        let mut bad_shape = FinstackConfig::default();
        bad_shape
            .extensions
            .insert(SENSITIVITIES_CONFIG_KEY_V1, json!({"unexpected": 1}));
        let parse_err =
            from_finstack_config_or_default(&bad_shape).expect_err("must reject unknown fields");
        assert!(
            parse_err.to_string().contains("Failed to parse extension"),
            "unexpected error: {parse_err}"
        );

        let mut empty_grid = FinstackConfig::default();
        empty_grid.extensions.insert(
            SENSITIVITIES_CONFIG_KEY_V1,
            json!({"dv01_buckets_years": []}),
        );
        let empty_err = from_finstack_config_or_default(&empty_grid)
            .expect_err("must reject empty bucket grid");
        assert!(
            empty_err.to_string().contains("must be non-empty"),
            "unexpected error: {empty_err}"
        );

        let mut unsorted_grid = FinstackConfig::default();
        unsorted_grid.extensions.insert(
            SENSITIVITIES_CONFIG_KEY_V1,
            json!({"cs01_buckets_years": [1.0, 1.0]}),
        );
        let unsorted_err =
            from_finstack_config_or_default(&unsorted_grid).expect_err("must reject unsorted grid");
        assert!(
            unsorted_err
                .to_string()
                .contains("must be strictly increasing"),
            "unexpected error: {unsorted_err}"
        );
    }

    #[test]
    fn from_context_or_default_layers_metric_overrides_on_top_of_config() {
        let mut cfg = FinstackConfig::default();
        cfg.extensions.insert(
            SENSITIVITIES_CONFIG_KEY_V1,
            json!({
                "rate_bump_bp": 2.5,
                "vol_bump_pct": 0.03
            }),
        );
        let pricing_overrides = crate::instruments::MetricPricingOverrides::default()
            .with_rate_bump(4.0)
            .with_vol_bump(0.05);

        let resolved = from_context_or_default(&cfg, Some(&pricing_overrides))
            .expect("layered config should parse");
        assert_eq!(resolved.rate_bump_bp, 4.0);
        assert_eq!(resolved.vol_bump_pct, 0.05);
        assert_eq!(resolved.credit_spread_bump_bp, 1.0);
    }

    #[test]
    fn apply_pricing_overrides_returns_base_when_missing() {
        let base = SensitivitiesConfig::default();
        let resolved =
            apply_pricing_overrides(base.clone(), None).expect("missing overrides should be ok");
        assert_eq!(resolved, base);
    }
}
