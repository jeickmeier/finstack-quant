//! The pairs of risk factors a cross-gamma metric crosses.

use crate::metrics::MetricId;

/// Identifies which two risk factors are crossed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CrossFactorPair {
    /// Parallel rates against parallel credit spreads.
    RatesCredit,
    /// Parallel rates against parallel volatility shifts.
    RatesVol,
    /// Spot/underlying against parallel volatility shifts.
    SpotVol,
    /// Spot/underlying against parallel credit spreads.
    SpotCredit,
    /// FX spot against parallel volatility shifts.
    FxVol,
    /// FX spot against parallel interest-rate shifts.
    FxRates,
    /// Parallel credit spreads against parallel volatility shifts.
    CreditVol,
}

impl CrossFactorPair {
    /// All currently supported cross-factor pairs.
    pub const ALL: &'static [Self] = &[
        Self::RatesCredit,
        Self::RatesVol,
        Self::SpotVol,
        Self::SpotCredit,
        Self::FxVol,
        Self::FxRates,
        Self::CreditVol,
    ];

    /// Returns the standard `MetricId` associated with this pair.
    pub fn metric_id(self) -> MetricId {
        match self {
            Self::RatesCredit => MetricId::CrossGammaRatesCredit,
            Self::RatesVol => MetricId::CrossGammaRatesVol,
            Self::SpotVol => MetricId::CrossGammaSpotVol,
            Self::SpotCredit => MetricId::CrossGammaSpotCredit,
            Self::FxVol => MetricId::CrossGammaFxVol,
            Self::FxRates => MetricId::CrossGammaFxRates,
            Self::CreditVol => MetricId::CrossGammaCreditVol,
        }
    }
}
