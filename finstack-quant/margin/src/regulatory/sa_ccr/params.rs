//! SA-CCR supervisory parameters per BCBS 279 Table 2.

use super::types::SaCcrAssetClass;

/// Supervisory factors by asset class per BCBS 279 Table 2.
pub const SUPERVISORY_FACTORS: &[(SaCcrAssetClass, f64)] = &[
    (SaCcrAssetClass::InterestRate, 0.005),
    (SaCcrAssetClass::ForeignExchange, 0.04),
    (SaCcrAssetClass::Credit, 0.05),
    (SaCcrAssetClass::Equity, 0.32),
    (SaCcrAssetClass::Commodity, 0.18),
];

/// Supervisory correlation by asset class per BCBS 279 Table 2.
pub const SUPERVISORY_CORRELATIONS: &[(SaCcrAssetClass, f64)] = &[
    (SaCcrAssetClass::InterestRate, 1.0),
    (SaCcrAssetClass::ForeignExchange, 1.0),
    (SaCcrAssetClass::Credit, 0.50),
    (SaCcrAssetClass::Equity, 0.80),
    (SaCcrAssetClass::Commodity, 0.40),
];

/// Supervisory option volatilities per BCBS 279 Table 2.
pub const SUPERVISORY_OPTION_VOLS: &[(SaCcrAssetClass, f64)] = &[
    (SaCcrAssetClass::InterestRate, 0.50),
    (SaCcrAssetClass::ForeignExchange, 0.15),
    (SaCcrAssetClass::Credit, 1.00),
    (SaCcrAssetClass::Equity, 1.20),
    (SaCcrAssetClass::Commodity, 1.50),
];

/// Look up supervisory factor for an asset class.
#[must_use]
pub fn supervisory_factor(asset_class: SaCcrAssetClass) -> f64 {
    SUPERVISORY_FACTORS
        .iter()
        .find(|(ac, _)| *ac == asset_class)
        .map(|(_, f)| *f)
        .unwrap_or(0.05)
}

/// Look up supervisory correlation for an asset class.
#[must_use]
pub fn supervisory_correlation(asset_class: SaCcrAssetClass) -> f64 {
    SUPERVISORY_CORRELATIONS
        .iter()
        .find(|(ac, _)| *ac == asset_class)
        .map(|(_, c)| *c)
        .unwrap_or(0.50)
}
