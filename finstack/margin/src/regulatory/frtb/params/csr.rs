//! CSR (Credit Spread Risk) prescribed parameters.
//!
//! Risk weights and correlations for non-securitization, securitization CTP,
//! and securitization non-CTP per BCBS d457.

/// CSR non-securitization delta risk weights by bucket.
///
/// Values are expressed in **percent of notional** (e.g. `0.5` means
/// `0.5%` = `50 bp`); they multiply a CSR delta stated as P&L per
/// 1 percentage-point spread shift.
///
/// Buckets 1-18 per FRTB specification:
/// 1: Sovereigns (incl. central banks)
/// 2: Sovereigns (incl. central banks) - other
/// 3: Financials (incl. government-backed)
/// 4: Basic materials, energy, industrials
/// 5: Consumer goods and services
/// 6: Technology, media, telecommunications
/// 7: Health care, utilities, local government
/// 8-12: Higher-risk variants
/// 13-18: Additional sectors
pub const CSR_NONSEC_RISK_WEIGHTS: &[(u8, f64)] = &[
    (1, 0.5),
    (2, 1.0),
    (3, 5.0),
    (4, 3.0),
    (5, 3.0),
    (6, 2.0),
    (7, 1.5),
    (8, 1.0),
    (9, 2.5),
    (10, 4.0),
    (11, 12.0),
    (12, 7.0),
    (13, 8.5),
    (14, 5.5),
    (15, 5.0),
    (16, 12.0),
    (17, 1.5),
    (18, 5.0),
];

/// CSR non-sec intra-bucket name correlation.
pub const CSR_NONSEC_INTRA_BUCKET_NAME_CORRELATION: f64 = 0.35;

/// CSR non-sec intra-bucket tenor correlation.
pub const CSR_NONSEC_INTRA_BUCKET_TENOR_CORRELATION: f64 = 0.65;

/// CSR non-sec inter-bucket correlation (uniform).
pub const CSR_NONSEC_INTER_BUCKET_CORRELATION: f64 = 0.40;

/// CSR non-sec vega risk weight.
///
/// Stored as the liquidity-horizon scaled effective weight; for CSR non-sec
/// the Basel base weight scales to the 100% cap.
pub const CSR_NONSEC_VEGA_RISK_WEIGHT: f64 = 1.00;

/// CSR securitisation (CTP) vega risk weight.
///
/// Distinct from [`CSR_NONSEC_VEGA_RISK_WEIGHT`]. Per MAR21.92, securitised
/// CTP exposures carry a 100% vega risk weight (no further LH scaling
/// reduces it). Using the non-sec 55% base materially understates
/// securitisation vega capital.
pub const CSR_SEC_CTP_VEGA_RISK_WEIGHT: f64 = 1.00;

/// CSR securitisation (non-CTP) vega risk weight.
///
/// Also 100% per MAR21.92.
pub const CSR_SEC_NONCTP_VEGA_RISK_WEIGHT: f64 = 1.00;

/// CSR non-sec curvature risk weight scale.
pub const CSR_NONSEC_CURVATURE_RISK_WEIGHT: f64 = 0.5;

/// CSR securitization CTP risk weights by bucket.
pub const CSR_SEC_CTP_RISK_WEIGHTS: &[(u8, f64)] = &[
    (1, 4.0),
    (2, 4.0),
    (3, 8.0),
    (4, 5.0),
    (5, 4.0),
    (6, 3.0),
    (7, 2.0),
    (8, 6.0),
    (9, 13.0),
    (10, 13.0),
    (11, 16.0),
    (12, 10.0),
    (13, 12.0),
    (14, 12.0),
    (15, 12.0),
    (16, 13.0),
];

/// CSR securitization non-CTP risk weights by bucket.
pub const CSR_SEC_NONCTP_RISK_WEIGHTS: &[(u8, f64)] = &[
    (1, 0.9),
    (2, 1.5),
    (3, 2.0),
    (4, 2.5),
    (5, 0.8),
    (6, 1.2),
    (7, 3.5),
    (8, 5.5),
    (9, 5.0),
    (10, 3.5),
    (11, 4.0),
    (12, 6.0),
    (13, 5.0),
    (14, 5.0),
    (15, 5.0),
    (16, 3.5),
    (17, 5.5),
    (18, 5.0),
    (19, 5.0),
    (20, 5.0),
    (21, 5.0),
    (22, 5.0),
    (23, 5.0),
    (24, 5.0),
    (25, 12.5),
];

/// CSR sec CTP intra-bucket correlation.
pub const CSR_SEC_CTP_INTRA_BUCKET_CORRELATION: f64 = 0.30;

/// CSR sec CTP inter-bucket correlation.
pub const CSR_SEC_CTP_INTER_BUCKET_CORRELATION: f64 = 0.40;

/// CSR sec non-CTP intra-bucket correlation.
pub const CSR_SEC_NONCTP_INTRA_BUCKET_CORRELATION: f64 = 0.30;

/// CSR sec non-CTP inter-bucket correlation.
pub const CSR_SEC_NONCTP_INTER_BUCKET_CORRELATION: f64 = 0.20;

use std::sync::LazyLock;

use finstack_core::HashMap;

// Index the small `pub const` slices into hashmaps so per-trade lookups
// are O(1). Built once on first use; the registry-backed `FrtbParams`
// path remains the source of truth for revision-tagged parameters.
static CSR_NONSEC_BY_BUCKET: LazyLock<HashMap<u8, f64>> =
    LazyLock::new(|| CSR_NONSEC_RISK_WEIGHTS.iter().copied().collect());
static CSR_SEC_CTP_BY_BUCKET: LazyLock<HashMap<u8, f64>> =
    LazyLock::new(|| CSR_SEC_CTP_RISK_WEIGHTS.iter().copied().collect());
static CSR_SEC_NONCTP_BY_BUCKET: LazyLock<HashMap<u8, f64>> =
    LazyLock::new(|| CSR_SEC_NONCTP_RISK_WEIGHTS.iter().copied().collect());

/// Look up a CSR non-sec risk weight by bucket.
#[must_use]
pub fn csr_nonsec_risk_weight(bucket: u8) -> f64 {
    CSR_NONSEC_BY_BUCKET.get(&bucket).copied().unwrap_or(5.0)
}

/// Look up a CSR sec CTP risk weight by bucket.
#[must_use]
pub fn csr_sec_ctp_risk_weight(bucket: u8) -> f64 {
    CSR_SEC_CTP_BY_BUCKET.get(&bucket).copied().unwrap_or(8.0)
}

/// Look up a CSR sec non-CTP risk weight by bucket.
#[must_use]
pub fn csr_sec_nonctp_risk_weight(bucket: u8) -> f64 {
    CSR_SEC_NONCTP_BY_BUCKET
        .get(&bucket)
        .copied()
        .unwrap_or(5.0)
}
