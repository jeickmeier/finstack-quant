//! Jump-to-Default metric for CDS Index.
//!
//! Calculates the instantaneous loss if a single constituent defaults immediately.
//!
//! ## Methodology
//!
//! ### When constituents data is available:
//! ```text
//! JTD_avg = (1 / N) × Σ(Weight_i × Notional × (1 - Recovery_i))
//! ```
//! Returns the **average** per-name default impact (single-name JTD).
//!
//! ### When using index-level curve only (simplified):
//! ```text
//! JTD = (1 / N) × Notional × (1 - Avg_Recovery)
//! ```
//! Where N = number of constituents in the index (e.g., 125 for CDX IG)
//!
//! ## Interpretation
//! - For protection **buyer**: JTD is positive (gain on default)
//! - For protection **seller**: JTD is negative (loss on default)
//!
//! ## Convention: gross LGD, no accrued netting
//!
//! This metric is the **gross** per-name protection payout
//! `LGD × Notional / N_orig`. Unlike the single-name CDS
//! `jump_to_default` (which nets the ISDA accrued-premium payable on
//! default — see `cds::metrics::jump_to_default`), the index variant does
//! **not** subtract per-name accrued premium, and there is no MTM-netted
//! "default exposure" variant for indices. The accrued adjustment is
//! bounded by `coupon × period_fraction / N_orig` (≈ $200 on a $48K
//! per-name JTD for a 100 bp quarterly coupon) and is deliberately
//! omitted to keep the index number a pure notional-at-risk screen.
//!
//! The per-name JTD is independent of `index_factor`: a surviving name
//! keeps its inception notional `Notional / N_orig` no matter how many
//! other names have defaulted (Markit index mechanics).
//!
//! ## Example
//! - CDX IG (125 names): $10M index with 40% recovery → JTD ≈ $48K per name default

use crate::instruments::credit_derivatives::cds::PayReceive;
use crate::instruments::credit_derivatives::cds_index::CDSIndex;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::{Error, Result};

/// Jump-to-default calculator for CDS Index.
pub(crate) struct JumpToDefaultCalculator;

/// Last-resort guess of an index's pool size from its name.
///
/// Index pool sizes drift with series (iTraxx Crossover has been 75 names
/// only since Series 9; CDX.NA.HY membership varies), so this is **only** a
/// fallback when no explicit count was supplied. Prefer
/// [`CDSIndex::with_num_constituents`] or a standard preset.
fn infer_constituent_count(index_name: &str) -> Option<f64> {
    let name = index_name.to_ascii_lowercase();
    if name.contains("cdx") && name.contains("na") && name.contains("ig") {
        Some(125.0)
    } else if name.contains("cdx") && name.contains("na") && name.contains("hy") {
        Some(100.0)
    } else if name.contains("itraxx") && name.contains("crossover") {
        Some(75.0)
    } else if name.contains("itraxx") {
        Some(125.0)
    } else if name.contains("cdx.em") || name.contains("cdx em") || name.contains("cdxem") {
        Some(40.0)
    } else {
        None
    }
}

/// Resolve the constituent count to use for a `SingleCurve`-mode JTD.
///
/// Prefers the explicit `num_constituents` supplied on the index. Falls back
/// to a name-substring guess only when no explicit count is available,
/// emitting a `tracing::warn!` because such guesses ignore per-series
/// membership drift. Errors when neither is available.
fn resolve_constituent_count(num_constituents: Option<u32>, index_name: &str) -> Result<f64> {
    if let Some(n) = num_constituents {
        if n == 0 {
            return Err(Error::Validation(format!(
                "CDS index '{index_name}' has num_constituents = 0; jump-to-default \
                 requires a positive pool size."
            )));
        }
        return Ok(f64::from(n));
    }

    match infer_constituent_count(index_name) {
        Some(n) => {
            tracing::warn!(
                index_name,
                inferred_count = n,
                "CDS index has no explicit num_constituents; jump-to-default is using a \
                 name-substring guess of the pool size, which ignores per-series membership \
                 drift. Set CDSIndex::with_num_constituents for an accurate JTD."
            );
            Ok(n)
        }
        None => Err(Error::Validation(format!(
            "Cannot determine constituent count for CDS index '{index_name}'. Set \
             CDSIndex::with_num_constituents (or supply constituents)."
        ))),
    }
}

impl MetricCalculator for JumpToDefaultCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let index: &CDSIndex = context.instrument_as()?;

        // Check if we have constituent data for more accurate calculation
        if !index.constituents.is_empty() {
            // Average per-name JTD using surviving constituents only.
            // Defaulted names have already been settled and are not exposed
            // to a future jump-to-default event.
            let active: Vec<_> = index.constituents.iter().filter(|c| !c.defaulted).collect();
            if active.is_empty() {
                return Ok(0.0);
            }
            let n = active.len() as f64;
            let sum_w: f64 = active.iter().map(|c| c.weight).sum();
            let norm = if sum_w > 0.0 { sum_w } else { 1.0 };

            let mut weighted_lgd = 0.0;
            for constituent in active {
                let lgd = 1.0 - constituent.credit.recovery_rate;
                weighted_lgd += (constituent.weight / norm) * lgd;
            }

            let scale = index.index_factor;
            let avg_jtd = index.notional.amount() * scale * weighted_lgd / n;

            // Apply sign based on position
            let signed_jtd = match index.side {
                PayReceive::Pay => avg_jtd,
                PayReceive::Receive => -avg_jtd,
            };

            Ok(signed_jtd)
        } else {
            // Simplified calculation using index-level parameters
            // Assume equal-weighted constituents
            //
            // `num_constituents` is the ORIGINAL series membership; each
            // surviving name keeps its inception notional `Notional / N_orig`
            // regardless of how many names have since defaulted, so the
            // per-name JTD is independent of `index_factor` (Markit index
            // mechanics). Equivalent formulation:
            // `factor·Notional / (factor·N_orig) · LGD`. The Constituents
            // branch above computes the same quantity via the surviving count.
            let num_constituents =
                resolve_constituent_count(index.num_constituents, &index.index_name)?;
            let avg_weight = 1.0 / num_constituents;
            let lgd = 1.0 - index.protection.recovery_rate;

            // Single name default impact
            let single_name_jtd = avg_weight * index.notional.amount() * lgd;

            // Apply sign based on position
            let signed_jtd = match index.side {
                PayReceive::Pay => single_name_jtd,
                PayReceive::Receive => -single_name_jtd,
            };

            Ok(signed_jtd)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_count_overrides_name_inference() {
        // Index named "CDX.NA.HY" maps to a hardcoded 100 by name inference,
        // but the actual series here has 97 names. The explicit count must win.
        let resolved =
            resolve_constituent_count(Some(97), "CDX.NA.HY").expect("explicit count must resolve");
        assert!(
            (resolved - 97.0).abs() < 1e-12,
            "expected supplied count 97, got {resolved}"
        );
        // Confirm the name on its own would have produced the wrong 100.
        assert_eq!(infer_constituent_count("CDX.NA.HY"), Some(100.0));
    }

    #[test]
    fn explicit_count_used_for_off_series_crossover() {
        // iTraxx Crossover pre-Series-9 had 50 names, not the inferred 75.
        let resolved = resolve_constituent_count(Some(50), "iTraxx Crossover")
            .expect("explicit count must resolve");
        assert!((resolved - 50.0).abs() < 1e-12, "got {resolved}");
        assert_eq!(infer_constituent_count("iTraxx Crossover"), Some(75.0));
    }

    #[test]
    fn falls_back_to_name_inference_when_count_absent() {
        let resolved = resolve_constituent_count(None, "CDX.NA.IG")
            .expect("known index name must resolve via fallback");
        assert!((resolved - 125.0).abs() < 1e-12, "got {resolved}");
    }

    #[test]
    fn errors_when_no_count_and_unknown_name() {
        assert!(resolve_constituent_count(None, "BESPOKE-INDEX-XYZ").is_err());
    }

    #[test]
    fn errors_on_zero_explicit_count() {
        assert!(resolve_constituent_count(Some(0), "CDX.NA.IG").is_err());
    }
}
