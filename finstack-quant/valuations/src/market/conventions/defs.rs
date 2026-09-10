//! Market convention definitions for indices, options, and credit.
//!
//! This module defines the data structures for all market convention types. Conventions capture
//! market-standard parameters such as day count conventions, business day adjustments, payment
//! frequencies, and settlement lags that are required for accurate instrument construction
//! and pricing.

use crate::instruments::rates::irs::FloatingLegCompounding;
use crate::market::conventions::ids::CdsDocClause;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use finstack_quant_core::types::IndexId;
use serde::{Deserialize, Serialize};

/// Type of rate index for convention determination.
///
/// Distinguishes between overnight risk-free rate (RFR) indices and term indices, which have
/// different conventions for compounding, payment frequencies, and reset lags.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::market::conventions::RateIndexKind;
///
/// let overnight = RateIndexKind::OvernightRfr;
/// let term = RateIndexKind::Term;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateIndexKind {
    /// Overnight Risk-Free Rate index (e.g., SOFR, SONIA, ESTR).
    ///
    /// These indices require compounding conventions and typically use OIS-style swap conventions.
    OvernightRfr,
    /// Term index with a fixed period (e.g., 3M LIBOR, 6M EURIBOR).
    ///
    /// These indices have fixed reset periods and use standard swap conventions.
    Term,
}

/// Convention details for pricing instruments tied to a rate index.
///
/// This structure captures all market-standard parameters for instruments referencing a rate
/// index, including day count, business day conventions, payment frequencies, and settlement
/// lags. Used by builders to construct deposits, FRAs, swaps, and other rate instruments.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::market::conventions::{RateIndexConventions, RateIndexKind};
/// use finstack_quant_core::dates::{BusinessDayConvention, DayCount, Tenor};
/// use finstack_quant_core::currency::Currency;
///
/// // In practice, conventions are loaded from the registry
/// // let conv = registry.require_rate_index(&IndexId::new("USD-SOFR-OIS"))?;
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RateIndexConventions {
    /// Operating currency of the index.
    pub currency: Currency,
    /// Index category (Overnight vs Term).
    pub kind: RateIndexKind,
    /// Index tenor (None for overnight indices).
    pub tenor: Option<Tenor>,
    /// Market standard day count convention.
    pub day_count: DayCount,
    /// Typical payment frequency for swaps referencing this index.
    pub default_payment_frequency: Tenor,
    /// Business days between accrual end and payment.
    pub default_payment_lag_days: i32,
    /// Business days between fixing and accrual start.
    pub default_reset_lag_days: i32,
    /// Methodology for compounding overnight rates (OIS only).
    pub ois_compounding: Option<FloatingLegCompounding>,

    // Swap market defaults
    /// Market-standard calendar identifier.
    pub market_calendar_id: String,
    /// Market-standard spot settlement lag (business days).
    pub market_settlement_days: i32,
    /// Market-standard business day convention.
    pub market_business_day_convention: BusinessDayConvention,
    /// Market-standard fixed leg day count.
    pub default_fixed_leg_day_count: DayCount,
    /// Market-standard fixed leg frequency.
    pub default_fixed_leg_frequency: Tenor,
}

/// Regional ISDA CDS schedule family.
///
/// Exact documentation clauses stay on the instrument or quote. This enum
/// identifies the regional schedule family used to resolve calendars,
/// day-count, frequency, stub, and settlement from the convention registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum CdsConvention {
    /// Standard North American convention (quarterly, Act/360).
    #[default]
    IsdaNa,
    /// Standard European convention (quarterly, Act/360).
    IsdaEu,
    /// Standard Asian convention (quarterly, Act/360).
    IsdaAs,
    /// Custom convention.
    Custom,
}

impl CdsConvention {
    /// Documentation-clause tag that identifies this regional family in the registry.
    #[must_use]
    pub fn as_doc_clause(self) -> CdsDocClause {
        match self {
            Self::IsdaNa => CdsDocClause::IsdaNa,
            Self::IsdaEu => CdsDocClause::IsdaEu,
            Self::IsdaAs => CdsDocClause::IsdaAs,
            Self::Custom => CdsDocClause::Custom,
        }
    }

    /// Map a documentation clause to its regional schedule family.
    ///
    /// Meta `Au`/`Nz` clauses use the Asia family. Exact restructuring clauses
    /// have no family of their own; schedule family then comes from currency.
    ///
    /// # Arguments
    ///
    /// * `clause` - Quote or instrument documentation clause to classify
    #[must_use]
    pub fn family_from_doc_clause(clause: CdsDocClause) -> Option<Self> {
        match clause {
            CdsDocClause::IsdaNa => Some(Self::IsdaNa),
            CdsDocClause::IsdaEu => Some(Self::IsdaEu),
            CdsDocClause::IsdaAs | CdsDocClause::IsdaAu | CdsDocClause::IsdaNz => {
                Some(Self::IsdaAs)
            }
            CdsDocClause::Custom => Some(Self::Custom),
            CdsDocClause::Cr14 | CdsDocClause::Mr14 | CdsDocClause::Mm14 | CdsDocClause::Xr14 => {
                None
            }
        }
    }
}

impl std::fmt::Display for CdsConvention {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IsdaNa => write!(f, "isda_na"),
            Self::IsdaEu => write!(f, "isda_eu"),
            Self::IsdaAs => write!(f, "isda_as"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

impl std::str::FromStr for CdsConvention {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "isda_na" => Ok(Self::IsdaNa),
            "isda_eu" => Ok(Self::IsdaEu),
            "isda_as" => Ok(Self::IsdaAs),
            "custom" => Ok(Self::Custom),
            _ => Err(format!(
                "Unknown CDS convention: '{s}'. Expected one of: isda_na, isda_eu, isda_as, custom"
            )),
        }
    }
}

/// Resolved CDS schedule and settlement settings from the convention registry.
///
/// Exact documentation clauses remain on the instrument or quote. `family`
/// is the regional schedule family used to select these settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct CdsConventionSpec {
    /// Regional schedule family represented by this registry row.
    pub family: CdsConvention,
    /// The calendar used for business day adjustments.
    pub calendar_id: String,
    /// The day count convention for the premium leg.
    pub day_count: DayCount,
    /// The business day convention.
    pub business_day_convention: BusinessDayConvention,
    /// Stub convention used when constructing the premium schedule.
    pub stub: StubKind,
    /// The number of business days for settlement.
    pub settlement_days: u16,
    /// The payment frequency of the premium leg.
    pub frequency: Tenor,
}

/// Conventions for Swaptions (Volatility Surfaces).
///
/// Defines market-standard parameters for swaption instruments, including exercise calendars,
/// business day conventions, fixed leg conventions, and floating leg index references. Used
/// by swaption builders to construct instruments with correct market conventions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwaptionConventions {
    /// Calendar for exercise and settlement.
    pub calendar_id: String,
    /// Settlement lag in business days.
    pub settlement_days: i32,
    /// Business day convention for dates.
    pub business_day_convention: BusinessDayConvention,
    /// Fixed leg payment frequency.
    pub fixed_leg_frequency: Tenor,
    /// Fixed leg day count.
    pub fixed_leg_day_count: DayCount,
    /// Floating leg index (implies float leg conventions).
    pub float_leg_index: String,
}

/// Conventions for Inflation Swaps (ZCIS).
///
/// Defines market-standard parameters for inflation swap instruments, including payment
/// calendars, business day conventions, day count conventions, and inflation lag periods.
/// Used by inflation swap builders to construct instruments with correct market conventions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InflationSwapConventions {
    /// Calendar for payment/fixing.
    pub calendar_id: String,
    /// Settlement lag in business days.
    pub settlement_days: i32,
    /// Business day convention.
    pub business_day_convention: BusinessDayConvention,
    /// Day count for the fixed leg.
    pub day_count: DayCount,
    /// Inflation lag (observation lag) in months/period.
    pub inflation_lag: Tenor,
    /// Monthly reference CPI rule: interpolated daily for USD, monthly step for EUR/UK.
    pub interpolation: finstack_quant_core::market_data::scalars::InflationInterpolation,
}

/// Conventions for cross-currency basis swaps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XccyConventions {
    /// Base (foreign) currency of the pair.
    pub base_currency: Currency,
    /// Quote (domestic) currency of the pair.
    pub quote_currency: Currency,
    /// Rate index identifier for the base-currency floating leg.
    pub base_index_id: IndexId,
    /// Rate index identifier for the quote-currency floating leg.
    pub quote_index_id: IndexId,
    /// Standard spot settlement lag in business days.
    pub spot_lag_days: i32,
    /// Coupon payment frequency for both legs.
    pub payment_frequency: Tenor,
    /// Accrual day count convention.
    pub day_count: DayCount,
    /// Business day convention for schedule and settlement dates.
    pub business_day_convention: BusinessDayConvention,
    /// Base-currency calendar identifier for business day adjustments.
    pub base_calendar_id: String,
    /// Quote-currency calendar identifier for business day adjustments.
    pub quote_calendar_id: String,
    /// Notional-exchange behaviour for this currency pair.
    ///
    /// For G10 pairs against USD (where `quote_currency = USD`) the dealer convention
    /// is `MtmResetting { resetting_side: Leg1 }`: the base-currency leg (leg1) has its
    /// notional re-marked each period to match the constant quote-currency (leg2)
    /// notional in current FX. Pair conventions registered the other way around (USD
    /// as base) must invert this to `Leg2`. Registry entries must state it
    /// explicitly.
    pub notional_exchange: crate::instruments::rates::xccy_swap::NotionalExchange,
}

/// Rule for deriving an interest-rate future's reference period from its expiry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IrFutureReferencePeriod {
    /// The quoted expiry precedes the term-rate period; the period starts after the
    /// configured business-day settlement lag.
    ForwardStarting,
    /// The quoted expiry is the business day before the ending IMM Wednesday; the
    /// reference period runs between IMM Wednesdays and settles in arrears.
    ImmQuarterInArrears,
    /// The reference period is the complete calendar month containing the quoted
    /// expiry and settles in arrears.
    CalendarMonthInArrears,
    /// The reference period runs from the first business day in the expiry month
    /// to the first business day of the following month and settles in arrears.
    BusinessMonthInArrears,
}

/// Conventions for Interest Rate Futures.
///
/// Defines market-standard parameters for interest rate future contracts, including contract
/// specifications, reference-period construction, settlement lags, and optional convexity
/// adjustments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrFutureConventions {
    /// Underlying rate index identifier.
    pub index_id: IndexId,
    /// Exchange-defined averaging or fixing method for final settlement.
    pub rate_averaging: crate::instruments::RateAveragingMethod,
    /// Rule used to derive the rate reference period from the quoted expiry.
    pub reference_period: IrFutureReferencePeriod,
    /// Calendar for business day adjustments.
    pub calendar_id: String,
    /// Business-day lag from expiry to period start for forward-starting contracts.
    ///
    /// This must be zero for in-arrears reference-period rules.
    pub settlement_days: i32,
    /// Number of delivery months for the underlying rate period.
    pub delivery_months: u8,
    /// Face value of the contract.
    pub face_value: f64,
    /// Tick size in price points.
    pub tick_size: f64,
    /// Tick value in currency units.
    pub tick_value: f64,
    /// Optional convexity adjustment in rate terms.
    #[serde(default)]
    pub convexity_adjustment: Option<f64>,
}
