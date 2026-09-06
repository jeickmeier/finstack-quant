//! Default Risk Charge (DRC) computation.
//!
//! DRC captures jump-to-default risk for credit and equity positions
//! that delta/vega/curvature cannot model. It is NOT subject to
//! correlation scenarios.

use super::types::{DrcAssetType, DrcPosition, DrcSector, DrcSeniority};
use finstack_quant_core::HashMap;
use std::collections::BTreeMap;
use std::sync::LazyLock;

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
    (DrcSeniority::CoveredBond, 0.25),
    (DrcSeniority::SeniorUnsecured, 0.75),
    (DrcSeniority::Subordinated, 1.00),
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
/// - BCBS FRTB Minimum Capital Requirements: `docs/REFERENCES.md#bcbs-frtb-minimum-capital-requirements`
///
pub fn drc_charge(positions: &[DrcPosition]) -> finstack_quant_core::Result<f64> {
    struct Obligor {
        sector: DrcSector,
        rating: u8,
        longs: [f64; 4],
        shorts: [f64; 4],
    }
    let mut obligors = BTreeMap::<&str, Obligor>::new();
    for pos in positions {
        let valid_sector = match pos.asset_type {
            DrcAssetType::Corporate | DrcAssetType::Equity => pos.sector == DrcSector::Corporate,
            DrcAssetType::Sovereign => pos.sector == DrcSector::Sovereign,
            DrcAssetType::LocalGovernment => pos.sector == DrcSector::LocalGovernment,
            DrcAssetType::Securitization => false,
        };
        if !valid_sector || pos.seniority == DrcSeniority::Securitization {
            return Err(finstack_quant_core::Error::Validation("DRC requires consistent non-securitisation asset and bucket classifications; securitisation DRC is unsupported".into()));
        }
        if pos.issuer.trim().is_empty()
            || !(1..=9).contains(&pos.rating_bucket)
            || !pos.maturity_years.is_finite()
            || pos.maturity_years < 0.0
            || !pos.jtd_amount.is_finite()
            || !pos.pnl_adjustment.is_finite()
            || (pos.asset_type == DrcAssetType::Equity) != (pos.seniority == DrcSeniority::Equity)
        {
            return Err(finstack_quant_core::Error::Validation(
                "invalid DRC amount, maturity, rating or equity seniority".into(),
            ));
        }
        let rank = match pos.seniority {
            DrcSeniority::CoveredBond => 0,
            DrcSeniority::SeniorUnsecured => 1,
            DrcSeniority::Subordinated => 2,
            DrcSeniority::Equity => 3,
            DrcSeniority::Securitization => {
                return Err(finstack_quant_core::Error::Validation(
                    "securitization DRC is unsupported".into(),
                ))
            }
        };
        let entry = obligors.entry(&pos.issuer).or_insert(Obligor {
            sector: pos.sector,
            rating: pos.rating_bucket,
            longs: [0.0; 4],
            shorts: [0.0; 4],
        });
        if entry.sector != pos.sector || entry.rating != pos.rating_bucket {
            return Err(finstack_quant_core::Error::Validation(
                "DRC obligor must have one bucket and rating assignment".into(),
            ));
        }
        let raw = drc_lgd(pos.seniority) * pos.jtd_amount + pos.pnl_adjustment;
        let maturity = pos.maturity_years.clamp(0.25, 1.0);
        if pos.jtd_amount > 0.0 {
            entry.longs[rank] += raw.max(0.0) * maturity;
        } else if pos.jtd_amount < 0.0 {
            entry.shorts[rank] += (-raw).max(0.0) * maturity;
        }
    }
    // Rank grows toward junior claims. Shorts can offset only equally senior
    // or more senior longs. Consume the most constrained longs first.
    let mut buckets = HashMap::<DrcSector, [f64; 4]>::default();
    for entry in obligors.values_mut() {
        for long_rank in (0..4).rev() {
            for short_rank in long_rank..4 {
                let offset = entry.longs[long_rank].min(entry.shorts[short_rank]);
                entry.longs[long_rank] -= offset;
                entry.shorts[short_rank] -= offset;
            }
        }
        let long: f64 = entry.longs.iter().sum();
        let short: f64 = entry.shorts.iter().sum();
        let rw = drc_risk_weight(entry.rating);
        let bucket = buckets.entry(entry.sector).or_default();
        bucket[0] += long;
        bucket[1] += short;
        bucket[2] += long * rw;
        bucket[3] += short * rw;
    }
    Ok(buckets
        .values()
        .map(|b| {
            let hbr = if b[0] + b[1] > 0.0 {
                b[0] / (b[0] + b[1])
            } else {
                0.0
            };
            (b[2] - hbr * b[3]).max(0.0)
        })
        .sum())
}

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
/// The fixed D457 table above is the canonical regulatory source.
fn drc_risk_weight(rating_bucket: u8) -> f64 {
    DRC_RW_BY_BUCKET
        .get(&rating_bucket)
        .copied()
        .unwrap_or(0.15) // Default: Unrated per MAR22.24
}

/// Look up LGD by seniority.
///
/// Defaults to 75% (senior unsecured) per Basel guidance for unmapped
/// seniorities.
fn drc_lgd(seniority: DrcSeniority) -> f64 {
    DRC_LGD_BY_SENIORITY
        .get(&seniority)
        .copied()
        .unwrap_or(0.75) // Default: senior unsecured
}
