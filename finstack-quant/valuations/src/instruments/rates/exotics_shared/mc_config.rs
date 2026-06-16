//! Monte Carlo configuration for rate exotic products.
//!
//! Resolves numeric MC settings (num_paths, seed, antithetic, time-step density)
//! from a set of pricing overrides, falling back to production defaults.

use finstack_quant_core::Result;
use serde::{Deserialize, Serialize};

/// Runtime Monte Carlo configuration shared across rate exotic pricers.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RateExoticMcConfig {
    /// Total number of Monte Carlo paths (before antithetic doubling).
    pub num_paths: usize,
    /// Random seed for reproducibility.
    pub seed: u64,
    /// Whether to use antithetic variates (doubles effective paths).
    pub antithetic: bool,
    /// Minimum number of simulation sub-steps between two consecutive
    /// observation/coupon dates. Ensures accurate short-rate dynamics.
    pub min_steps_between_events: usize,
    /// Polynomial basis degree for LSMC regression (only used by
    /// [`crate::instruments::rates::exotics_shared::hw1f_lsmc`]).
    pub basis_degree: usize,
    /// Split-sample (out-of-sample) LSMC pricing.
    ///
    /// When `true`, paths are partitioned by stream parity: even-indexed
    /// streams are used to fit the continuation-value regression, odd-indexed
    /// streams are used to price under that fitted policy. This removes the
    /// well-known positive in-sample bias of plain Longstaff-Schwartz at the
    /// cost of roughly √2× more standard error (half the paths drive the
    /// estimate). Aggregation reports stats on the pricing half only.
    ///
    /// Default is `false` (in-sample / Longstaff-Schwartz baseline) to keep
    /// existing pricing reproducible; enable for conservative bracketing of
    /// the true callable value.
    pub oos_lsmc: bool,
}

impl Default for RateExoticMcConfig {
    fn default() -> Self {
        let defaults = &finstack_quant_monte_carlo::registry::embedded_defaults_or_panic()
            .rust
            .rate_exotics;
        Self {
            num_paths: defaults.num_paths,
            seed: defaults.seed,
            antithetic: defaults.antithetic,
            min_steps_between_events: defaults.min_steps_between_events,
            basis_degree: defaults.basis_degree,
            oos_lsmc: false,
        }
    }
}

impl RateExoticMcConfig {
    /// Parse an `RateExoticMcConfig` from a `serde_json::Value` blob, falling
    /// back to the default for any missing field.
    ///
    /// Recognized keys (any may be omitted):
    /// - `mc_num_paths`: `usize`
    /// - `mc_seed`: `u64`
    /// - `mc_antithetic`: `bool`
    /// - `mc_min_steps_between_events`: `usize`
    /// - `mc_basis_degree`: `usize`
    ///
    /// Currently infallible; the `Result` return is reserved for future validation
    /// (e.g. range or parse errors).
    pub fn from_overrides(overrides: Option<&serde_json::Value>) -> Result<Self> {
        let mut cfg = Self::default();
        let Some(obj) = overrides.and_then(|v| v.as_object()) else {
            return Ok(cfg);
        };
        if let Some(v) = obj.get("mc_num_paths").and_then(|x| x.as_u64()) {
            cfg.num_paths = v as usize;
        }
        if let Some(v) = obj.get("mc_seed").and_then(|x| x.as_u64()) {
            cfg.seed = v;
        }
        if let Some(v) = obj.get("mc_antithetic").and_then(|x| x.as_bool()) {
            cfg.antithetic = v;
        }
        if let Some(v) = obj
            .get("mc_min_steps_between_events")
            .and_then(|x| x.as_u64())
        {
            cfg.min_steps_between_events = (v as usize).max(1);
        }
        if let Some(v) = obj.get("mc_basis_degree").and_then(|x| x.as_u64()) {
            cfg.basis_degree = (v as usize).clamp(1, 4);
        }
        if let Some(v) = obj.get("mc_oos_lsmc").and_then(|x| x.as_bool()) {
            cfg.oos_lsmc = v;
        }
        Ok(cfg)
    }

    /// Total effective Monte Carlo paths generated. With `antithetic = true`,
    /// returns `num_paths` rounded **down to the nearest even number** (antithetic
    /// paths come in pairs); with `antithetic = false`, returns `num_paths`
    /// unchanged.
    pub fn effective_path_count(&self) -> usize {
        if self.antithetic {
            self.num_paths / 2 * 2
        } else {
            self.num_paths
        }
    }

    /// Number of distinct RNG shock streams required. With `antithetic = true`
    /// each stream is replayed twice (once with negated shocks), so this is
    /// `num_paths / 2`. With `antithetic = false` it equals `num_paths`.
    pub fn raw_stream_count(&self) -> usize {
        if self.antithetic {
            self.num_paths / 2
        } else {
            self.num_paths
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn default_values() {
        let cfg = RateExoticMcConfig::default();
        assert_eq!(cfg.num_paths, 20_000);
        assert_eq!(cfg.seed, 42);
        assert!(cfg.antithetic);
        assert_eq!(cfg.min_steps_between_events, 4);
        assert_eq!(cfg.basis_degree, 2);
    }

    #[test]
    fn from_overrides_partial() {
        let overrides = json!({ "mc_num_paths": 5000, "mc_seed": 7 });
        let cfg = RateExoticMcConfig::from_overrides(Some(&overrides)).expect("ok");
        assert_eq!(cfg.num_paths, 5000);
        assert_eq!(cfg.seed, 7);
        assert!(cfg.antithetic);
        assert_eq!(cfg.basis_degree, 2);
    }

    #[test]
    fn from_overrides_none_returns_default() {
        let cfg = RateExoticMcConfig::from_overrides(None).expect("ok");
        assert_eq!(cfg, RateExoticMcConfig::default());
    }

    #[test]
    fn basis_degree_clamped() {
        let overrides = json!({ "mc_basis_degree": 99 });
        let cfg = RateExoticMcConfig::from_overrides(Some(&overrides)).expect("ok");
        assert_eq!(cfg.basis_degree, 4);
    }

    #[test]
    fn effective_path_count_antithetic() {
        let cfg = RateExoticMcConfig {
            num_paths: 101,
            antithetic: true,
            ..Default::default()
        };
        // 101/2 = 50 streams * 2 = 100 (odd half path dropped)
        assert_eq!(cfg.effective_path_count(), 100);
    }

    #[test]
    fn raw_stream_count_antithetic_and_non() {
        let a = RateExoticMcConfig {
            num_paths: 100,
            antithetic: true,
            ..Default::default()
        };
        assert_eq!(a.raw_stream_count(), 50);
        let b = RateExoticMcConfig {
            num_paths: 100,
            antithetic: false,
            ..Default::default()
        };
        assert_eq!(b.raw_stream_count(), 100);
    }

    #[test]
    fn basis_degree_clamped_low_side() {
        let overrides = serde_json::json!({ "mc_basis_degree": 0 });
        let cfg = RateExoticMcConfig::from_overrides(Some(&overrides)).expect("ok");
        assert_eq!(cfg.basis_degree, 1);
    }

    #[test]
    fn min_steps_between_events_floored_at_one() {
        let overrides = serde_json::json!({ "mc_min_steps_between_events": 0 });
        let cfg = RateExoticMcConfig::from_overrides(Some(&overrides)).expect("ok");
        assert_eq!(cfg.min_steps_between_events, 1);
    }
}
