use super::MetricId;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Logical grouping of standard metrics for discovery and display.
///
/// Each standard metric belongs to exactly one group. Use
/// [`MetricGroup::metrics()`] to list members and
/// [`MetricGroup::ALL`] to iterate all groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum MetricGroup {
    /// Static pricing outputs: prices, yields, spreads, durations, implied
    /// levels, convexity, embedded option value.
    Pricing,
    /// Time-driven P&L: theta decomposition, carry components, financing,
    /// dollar-roll carry.
    Carry,
    /// First-order bump sensitivities to market curves: DV01, PV01,
    /// bucketed DV01, rho, and other rates-focused "01" metrics.
    Sensitivity,
    /// Options-style Greeks and all second-order / higher-order
    /// sensitivities: delta, gamma, vega, cross-gammas, variance vega.
    Greeks,
    /// CDS/credit analytics and credit-specific sensitivities: CS01,
    /// bucketed CS01, par spread, risky PV01/annuity, spread DV01,
    /// correlation01, default metrics, recovery.
    Credit,
    /// Rates instrument decomposition: IRS legs, annuities, par rates,
    /// basis swap, TRS, deposit/calibration intermediates.
    Rates,
    /// FX instrument pricing and analytics: spot rates, amounts, FX
    /// sensitivities (FX01, FX delta, FX vega).
    Fx,
    /// Equity/basket/ETF pricing, equity-derivative analytics, and
    /// variance swap pricing outputs.
    Equity,
    /// Securitization pool and tranche analytics: WAL, WAM, CPR, CDR,
    /// prepayment/severity sensitivities, ABS/CLO/CMBS/facility specifics.
    StructuredCredit,
    /// PE fund metrics, DCF valuation, repo analytics,
    /// inflation-linked bond metrics, VaR.
    Alternatives,
}

impl MetricGroup {
    /// All groups in display order.
    pub const ALL: &'static [MetricGroup] = &[
        MetricGroup::Pricing,
        MetricGroup::Carry,
        MetricGroup::Sensitivity,
        MetricGroup::Greeks,
        MetricGroup::Credit,
        MetricGroup::Rates,
        MetricGroup::Fx,
        MetricGroup::Equity,
        MetricGroup::StructuredCredit,
        MetricGroup::Alternatives,
    ];

    /// Human-readable group name.
    pub const fn display_name(&self) -> &'static str {
        match self {
            MetricGroup::Pricing => "Pricing",
            MetricGroup::Carry => "Carry",
            MetricGroup::Sensitivity => "Sensitivity",
            MetricGroup::Greeks => "Greeks",
            MetricGroup::Credit => "Credit",
            MetricGroup::Rates => "Rates",
            MetricGroup::Fx => "FX",
            MetricGroup::Equity => "Equity",
            MetricGroup::StructuredCredit => "Structured Credit",
            MetricGroup::Alternatives => "Alternatives",
        }
    }

    /// Standard metrics belonging to this group.
    pub fn metrics(&self) -> &'static [MetricId] {
        match self {
            Self::Pricing => &[
                MetricId::DirtyPrice,
                MetricId::CleanPrice,
                MetricId::ConversionFactor,
                MetricId::Accrued,
                MetricId::Ytm,
                MetricId::Ytw,
                MetricId::JapaneseSimpleYield,
                MetricId::MoosmullerYtm,
                MetricId::Moic,
                MetricId::MoicToWorst,
                MetricId::Xirr,
                MetricId::XirrToWorst,
                MetricId::ZSpread,
                MetricId::Oas,
                MetricId::ISpread,
                MetricId::ASWPar,
                MetricId::ASWMarket,
                MetricId::DiscountMargin,
                MetricId::EmbeddedOptionValue,
                MetricId::DurationMac,
                MetricId::DurationMod,
                MetricId::RealDuration,
                MetricId::YieldDv01,
                MetricId::Convexity,
                MetricId::ImpliedVol,
                MetricId::TimeToMaturity,
                MetricId::FuturesPrice,
                MetricId::Basis,
            ],
            Self::Carry => &[
                MetricId::Theta,
                MetricId::ThetaCarry,
                MetricId::ThetaRollDown,
                MetricId::CarryTotal,
                MetricId::CouponIncome,
                MetricId::PullToPar,
                MetricId::RollDown,
                MetricId::FundingCost,
                MetricId::ImpliedFinancingRate,
                MetricId::RollSpecialness,
                MetricId::Breakeven,
                MetricId::ThetaPeriodDays,
                MetricId::CarryDecompositionDegenerate,
            ],
            Self::Sensitivity => &[
                MetricId::Dv01,
                MetricId::BucketedDv01,
                MetricId::DurationDv01,
                MetricId::Pv01,
                MetricId::ForwardPv01,
                MetricId::Rho,
                MetricId::ForeignRho,
                MetricId::Dv01Domestic,
                MetricId::Dv01Foreign,
                MetricId::Dividend01,
                MetricId::Inflation01,
                MetricId::Conversion01,
                MetricId::CollateralHaircut01,
                MetricId::CollateralPrice01,
                MetricId::ConvexityAdjustmentRisk,
            ],
            Self::Greeks => &[
                MetricId::Delta,
                MetricId::DeltaForward,
                MetricId::DeltaPremiumAdjustedSpot,
                MetricId::DeltaPremiumAdjustedForward,
                MetricId::Gamma,
                MetricId::Vega,
                MetricId::HwSigmaVega,
                MetricId::BucketedVega,
                MetricId::BucketedVegaResidual,
                MetricId::Vanna,
                MetricId::Volga,
                MetricId::Charm,
                MetricId::Color,
                MetricId::Speed,
                MetricId::IrConvexity,
                MetricId::IrCrossGamma,
                MetricId::InflationConvexity,
                MetricId::CsGamma,
                MetricId::CrossGammaRatesCredit,
                MetricId::CrossGammaRatesVol,
                MetricId::CrossGammaSpotVol,
                MetricId::CrossGammaSpotCredit,
                MetricId::CrossGammaFxVol,
                MetricId::CrossGammaFxRates,
                MetricId::CrossGammaCreditVol,
                MetricId::VarianceVega,
            ],
            Self::Credit => &[
                MetricId::Cs01,
                MetricId::BucketedCs01,
                MetricId::ParSpread,
                MetricId::RiskyPv01,
                MetricId::RiskyAnnuity,
                MetricId::SpreadDv01,
                MetricId::Spread01,
                MetricId::Correlation01,
                MetricId::Default01,
                MetricId::ProtectionLegPv,
                MetricId::PremiumLegPv,
                MetricId::JumpToDefault,
                MetricId::DefaultExposure,
                MetricId::ExpectedLoss,
                MetricId::Recovery01,
            ],
            Self::Rates => &[
                MetricId::Annuity,
                MetricId::ParRate,
                MetricId::PvFixed,
                MetricId::PvFloat,
                MetricId::PvPrimary,
                MetricId::PvReference,
                MetricId::AnnuityPrimary,
                MetricId::AnnuityReference,
                MetricId::BasisParSpread,
                MetricId::IncrementalParSpread,
                MetricId::FinancingAnnuity,
                MetricId::IndexDelta,
                MetricId::Yf,
                MetricId::DfStart,
                MetricId::DfEnd,
                MetricId::DepositParRate,
                MetricId::DfEndFromQuote,
                MetricId::QuoteRate,
                MetricId::ImpliedForward,
                MetricId::ConvexityAdjustment,
                MetricId::ExpectedExerciseTime,
            ],
            Self::Fx => &[
                MetricId::SpotRate,
                MetricId::BaseAmount,
                MetricId::InverseRate,
                MetricId::Fx01,
                MetricId::FxDelta,
                MetricId::FxVega,
            ],
            Self::Equity => &[
                MetricId::EquityPricePerShare,
                MetricId::EquityShares,
                MetricId::EquityDividendYield,
                MetricId::EquityForwardPrice,
                MetricId::WeightRisk,
                MetricId::DeltaVol,
                MetricId::ConstituentDelta,
                MetricId::ConstituentCount,
                MetricId::ExpenseRatio,
                MetricId::ExpectedVariance,
                MetricId::RealizedVariance,
                MetricId::VarianceNotional,
                MetricId::VarianceStrikeVol,
                MetricId::VarianceTimeToMaturity,
            ],
            Self::StructuredCredit => &[
                MetricId::WAL,
                MetricId::WAM,
                MetricId::CPR,
                MetricId::CDR,
                MetricId::SpreadDuration,
                MetricId::Prepayment01,
                MetricId::Severity01,
                MetricId::AbsDelinquency,
                MetricId::AbsChargeOff,
                MetricId::AbsExcessSpread,
                MetricId::AbsCreditEnhancement,
                MetricId::AbsPaymentRate,
                MetricId::CloWarf,
                MetricId::CloWas,
                MetricId::CmbsDscr,
                MetricId::AbfBorrowingBase,
                MetricId::AbfBorrowingBaseCushion,
                MetricId::AbfAdvanceRateUtilization,
                MetricId::AbfFacilityIrr,
                MetricId::AbfResidualIrr,
            ],
            Self::Alternatives => &[
                MetricId::RealYield,
                MetricId::IndexRatio,
                MetricId::BreakevenInflation,
                MetricId::LpIrr,
                MetricId::GpCarryTotal,
                MetricId::MoicLp,
                MetricId::DpiLp,
                MetricId::TvpiLp,
                MetricId::CarryAccrued,
                MetricId::Nav01,
                MetricId::Carry01,
                MetricId::Hurdle01,
                MetricId::EnterpriseValue,
                MetricId::EquityValue,
                MetricId::TerminalValuePV,
                MetricId::CollateralValue,
                MetricId::RequiredCollateral,
                MetricId::CollateralCoverage,
                MetricId::RepoInterest,
                MetricId::FundingRisk,
                MetricId::EffectiveRate,
                MetricId::ImpliedCollateralReturn,
                MetricId::HVar,
                MetricId::ExpectedShortfall,
            ],
        }
    }
}

impl fmt::Display for MetricGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}
