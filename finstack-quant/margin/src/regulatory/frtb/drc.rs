//! Default Risk Charge (DRC) computation.
//!
//! DRC captures jump-to-default risk for credit and equity positions
//! that delta/vega/curvature cannot model. It is NOT subject to
//! correlation scenarios.

use super::types::{DrcPosition, DrcSector, DrcSeniority};
use finstack_quant_core::HashMap;

/// Prescribed DRC risk weights by rating bucket.
///
/// Source: Basel Framework MAR22.24 (FRTB Standardised Approach, DRC for
/// non-securitisations). Unrated exposures receive the BB-equivalent 15%
/// weight, and defaulted exposures receive 100%.
pub const DRC_RISK_WEIGHTS: &[(u8, f64)] = &[
    (1, 0.005), // AAA
    (2, 0.02),  // AA
    (3, 0.03),  // A
    (4, 0.06),  // BBB
    (5, 0.15),  // BB
    (6, 0.30),  // B
    (7, 0.50),  // CCC
    (8, 0.15),  // Unrated
    (9, 1.00),  // Defaulted
];

/// LGD assumptions by seniority.
pub const DRC_LGD: &[(DrcSeniority, f64)] = &[
    (DrcSeniority::SeniorUnsecured, 0.75),
    (DrcSeniority::Subordinated, 0.75),
    (DrcSeniority::Equity, 1.00),
    (DrcSeniority::Securitization, 1.00),
];

/// Compute the Default Risk Charge (MAR22.20-22.24).
///
/// Aggregates jump-to-default risk bucket-by-bucket (bucket = sector per
/// MAR22.5). Within each bucket:
///
/// 1. Gross JTD per position, with the MAR22.9 sign-preserving floor on
///    `LGD * notional + P&L`.
/// 2. Net JTD per obligor (long/short offset within issuer).
/// 3. Weighted long / short sums:
///    `WtS_long_b  = sum_k max(0, netJTD_k) * RW_k`
///    `WtS_short_b = sum_k min(0, netJTD_k) * RW_k`  (negative)
/// 4. Bucket hedge-benefit ratio, computed on **unweighted** net JTD per
///    MAR22.23. This captures how much of the short offset Basel allows
///    against the bucket's long exposure:
///    `HBR_b = sum_k max(0, netJTD_k) /
///             (sum_k max(0, netJTD_k) + sum_k |min(0, netJTD_k)|)`
/// 5. Per-bucket DRC:
///    `DRC_b = max(WtS_long_b - HBR_b * |WtS_short_b|, 0)`
///
/// Total DRC is the simple sum of per-bucket charges — there is no
/// further netting between buckets per MAR22.23.
///
/// # Arguments
///
/// * `positions` - Trading-book jump-to-default positions with signed JTD
///   notionals, rating buckets, sector buckets, seniority, and P&L adjustment.
///
/// # Returns
///
/// The total default risk charge. Returns `0.0` for an empty position set.
///
/// # References
///
/// - BCBS FRTB Minimum Capital Requirements:
///   `docs/REFERENCES.md#bcbs-frtb-minimum-capital-requirements`
pub fn drc_charge(positions: &[DrcPosition]) -> f64 {
    if positions.is_empty() {
        return 0.0;
    }

    // Step 1: per-position gross JTD, MAR22.9 floor.
    // Step 2: net per issuer inside the same sector bucket. We key by
    // (sector, issuer) so the same obligor in different buckets — which
    // shouldn't normally happen, but is well-defined here — is netted
    // within each bucket independently.
    struct NetEntry {
        net_jtd: f64,
        rating_bucket: u8,
    }
    let mut net_by_issuer: HashMap<(DrcSector, String), NetEntry> = HashMap::default();
    for pos in positions {
        let lgd = drc_lgd(pos.seniority);
        let raw = lgd * pos.jtd_amount + pos.pnl_adjustment;
        // MAR22.9 sign-preserving floor: longs clamp at 0 from below,
        // shorts clamp at 0 from above, using the *notional* sign.
        let gross_jtd = if pos.jtd_amount > 0.0 {
            raw.max(0.0)
        } else if pos.jtd_amount < 0.0 {
            raw.min(0.0)
        } else {
            0.0
        };
        let entry = net_by_issuer
            .entry((pos.sector, pos.issuer.clone()))
            .or_insert(NetEntry {
                net_jtd: 0.0,
                rating_bucket: pos.rating_bucket,
            });
        entry.net_jtd += gross_jtd;
    }

    // Step 3: for each sector bucket, accumulate BOTH weighted and
    // unweighted long/short sums. HBR uses unweighted net JTD per MAR22.23,
    // while the bucket DRC itself uses weighted sums.
    #[derive(Default, Clone, Copy)]
    struct BucketAcc {
        long_unweighted: f64,
        short_unweighted_abs: f64,
        long_weighted: f64,
        short_weighted_abs: f64,
    }
    let mut by_sector: HashMap<DrcSector, BucketAcc> = HashMap::default();
    for ((sector, _issuer), entry) in &net_by_issuer {
        let rw = drc_risk_weight(entry.rating_bucket);
        let weighted = entry.net_jtd * rw;
        let acc = by_sector.entry(*sector).or_default();
        if entry.net_jtd > 0.0 {
            acc.long_unweighted += entry.net_jtd;
            acc.long_weighted += weighted;
        } else if entry.net_jtd < 0.0 {
            acc.short_unweighted_abs += entry.net_jtd.abs();
            acc.short_weighted_abs += weighted.abs();
        }
    }

    // Steps 4 & 5: per-bucket HBR (unweighted) and DRC (weighted), summed
    // across buckets.
    let mut total = 0.0;
    for acc in by_sector.values() {
        let denom = acc.long_unweighted + acc.short_unweighted_abs;
        let hbr = if denom > 0.0 {
            acc.long_unweighted / denom
        } else {
            0.0
        };
        let bucket_drc = (acc.long_weighted - hbr * acc.short_weighted_abs).max(0.0);
        total += bucket_drc;
    }
    total
}

use std::sync::LazyLock;

static DRC_RW_BY_BUCKET: LazyLock<finstack_quant_core::HashMap<u8, f64>> =
    LazyLock::new(|| DRC_RISK_WEIGHTS.iter().copied().collect());
static DRC_LGD_BY_SENIORITY: LazyLock<finstack_quant_core::HashMap<DrcSeniority, f64>> =
    LazyLock::new(|| DRC_LGD.iter().copied().collect());

/// Look up DRC risk weight by rating bucket.
///
/// Unknown buckets fall back to the Unrated weight (15% per MAR22.24),
/// matching how the Basel text treats exposures that lack an external
/// rating. Callers who want a stricter policy should validate rating
/// assignment upstream and not rely on this fallback.
///
/// The defaults are mirrored in
/// [`crate::regulatory::frtb::params::DrcParams::d457`] so a
/// [`super::params::FrtbParams`] bundle carries the same values for
/// audit-trail tagging and JSON-overlay substitution.
fn drc_risk_weight(rating_bucket: u8) -> f64 {
    DRC_RW_BY_BUCKET
        .get(&rating_bucket)
        .copied()
        .unwrap_or(0.15) // Default: Unrated per MAR22.24
}

/// Look up LGD by seniority.
///
/// Defaults to 75% (senior unsecured) per Basel guidance for unmapped
/// seniorities. Mirrored in
/// [`crate::regulatory::frtb::params::DrcParams::d457`].
fn drc_lgd(seniority: DrcSeniority) -> f64 {
    DRC_LGD_BY_SENIORITY
        .get(&seniority)
        .copied()
        .unwrap_or(0.75) // Default: senior unsecured
}
