//! FRTB MAR21 intra-bucket and inter-bucket correlation lookups.

use super::super::types::FrtbRiskClass;

/// Name/tranche correlation before tenor, basis and scenario adjustments.
pub(in super::super) fn name_correlation(class: FrtbRiskClass, bucket: u8) -> f64 {
    match class {
        FrtbRiskClass::Equity => match bucket {
            1..=4 => 0.15,
            5..=8 => 0.25,
            9 => 0.075,
            10 => 0.125,
            12..=13 => 0.8,
            _ => 0.0,
        },
        FrtbRiskClass::Commodity => [0.55, 0.95, 0.4, 0.8, 0.6, 0.65, 0.55, 0.45, 0.15, 0.4, 0.15]
            .get(usize::from(bucket.wrapping_sub(1)))
            .copied()
            .unwrap_or(f64::NAN),
        FrtbRiskClass::CsrNonSec | FrtbRiskClass::CsrSecCtp => {
            if bucket >= 17 {
                0.8
            } else {
                0.35
            }
        }
        FrtbRiskClass::CsrSecNonCtp => 0.4,
        _ => 1.0,
    }
}

/// Buckets whose constituents receive no intra-bucket diversification.
pub(in super::super) fn other_bucket(class: FrtbRiskClass, bucket: u8) -> bool {
    matches!(
        (class, bucket),
        (FrtbRiskClass::Equity, 11)
            | (FrtbRiskClass::CsrNonSec | FrtbRiskClass::CsrSecCtp, 16)
            | (FrtbRiskClass::CsrSecNonCtp, 25)
    )
}

/// Basel MAR21.57 sector matrix, with rating adjustment for IG versus HY.
fn credit_inter_correlation(b: u8, c: u8) -> f64 {
    const MATRIX: [[f64; 11]; 11] = [
        [1., 0.75, 0.10, 0.20, 0.25, 0.20, 0.15, 0.10, 0., 0.45, 0.45],
        [0.75, 1., 0.05, 0.15, 0.20, 0.15, 0.10, 0.10, 0., 0.45, 0.45],
        [0.10, 0.05, 1., 0.05, 0.15, 0.20, 0.05, 0.20, 0., 0.45, 0.45],
        [0.20, 0.15, 0.05, 1., 0.20, 0.25, 0.05, 0.05, 0., 0.45, 0.45],
        [0.25, 0.20, 0.15, 0.20, 1., 0.25, 0.05, 0.15, 0., 0.45, 0.45],
        [0.20, 0.15, 0.20, 0.25, 0.25, 1., 0.05, 0.20, 0., 0.45, 0.45],
        [0.15, 0.10, 0.05, 0.05, 0.05, 0.05, 1., 0.05, 0., 0.45, 0.45],
        [0.10, 0.10, 0.20, 0.05, 0.15, 0.20, 0.05, 1., 0., 0.45, 0.45],
        [0., 0., 0., 0., 0., 0., 0., 0., 1., 0., 0.],
        [0.45, 0.45, 0.45, 0.45, 0.45, 0.45, 0.45, 0.45, 0., 1., 0.75],
        [0.45, 0.45, 0.45, 0.45, 0.45, 0.45, 0.45, 0.45, 0., 0.75, 1.],
    ];
    let sector = |x: u8| {
        Some(usize::from(match x {
            1..=8 => x - 1,
            9..=15 => x - 9,
            16..=18 => x - 8,
            _ => return None,
        }))
    };
    let rating = if b <= 15 && c <= 15 && (b <= 8) != (c <= 8) {
        0.5
    } else {
        1.0
    };
    match (sector(b), sector(c)) {
        (Some(i), Some(j)) => rating * MATRIX[i][j],
        _ => f64::NAN,
    }
}

/// Correlation between distinct Basel buckets before scenario scaling.
pub(in super::super) fn bucket_correlation(class: FrtbRiskClass, b: u8, c: u8) -> f64 {
    match class {
        FrtbRiskClass::Equity => {
            if b == 11 || c == 11 {
                0.0
            } else if b <= 10 && c <= 10 {
                0.15
            } else if b >= 12 && c >= 12 {
                0.75
            } else {
                0.45
            }
        }
        FrtbRiskClass::Commodity => {
            if b == 11 || c == 11 {
                0.0
            } else {
                0.2
            }
        }
        FrtbRiskClass::CsrNonSec | FrtbRiskClass::CsrSecCtp => credit_inter_correlation(b, c),
        _ => 0.0,
    }
}
