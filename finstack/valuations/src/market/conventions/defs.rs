//! Market convention definitions for indices, options, and credit.
//!
//! This module defines the data structures for all market convention types. Conventions capture
//! market-standard parameters such as day count conventions, business day adjustments, payment
//! frequencies, and settlement lags that are required for accurate instrument construction
//! and pricing.

use crate::instruments::rates::irs::FloatingLegCompounding;
use crate::instruments::{BondConvention, ExerciseStyle, SettlementType};
use crate::market::conventions::ids::FxConventionId;
use finstack_core::currency::Currency;
use finstack_core::dates::{BusinessDayConvention, DayCount, Tenor};
use finstack_core::types::IndexId;
use serde::{Deserialize, Serialize};

/// Type of rate index for convention determination.
///
/// Distinguishes between overnight risk-free rate (RFR) indices and term indices, which have
/// different conventions for compounding, payment frequencies, and reset lags.
///
/// # Examples
///
/// ```rust
/// use finstack_valuations::market::conventions::RateIndexKind;
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
/// use finstack_valuations::market::conventions::{RateIndexConventions, RateIndexKind};
/// use finstack_core::dates::{BusinessDayConvention, DayCount, Tenor};
/// use finstack_core::currency::Currency;
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

/// Conventions for Credit Default Swaps.
///
/// Defines market-standard parameters for CDS instruments, including payment frequencies,
/// day count conventions, business day adjustments, and settlement lags. Used by CDS builders
/// to construct instruments with correct market conventions.
///
/// # Examples
///
/// ```rust
/// use finstack_valuations::market::conventions::CdsConventions;
/// use finstack_core::dates::{BusinessDayConvention, DayCount, Tenor};
///
/// // In practice, conventions are loaded from the registry
/// // let conv = registry.require_cds(&CdsConventionKey { ... })?;
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CdsConventions {
    /// The calendar used for business day adjustments.
    pub calendar_id: String,
    /// The day count convention for the premium leg.
    pub day_count: DayCount,
    /// The business day convention.
    pub bdc: BusinessDayConvention,
    /// The number of business days for settlement.
    pub settlement_days: i32,
    /// The payment frequency of the premium leg.
    pub frequency: Tenor,
}

/// Conventions for Options (Equity/Commodity/FX).
///
/// Defines market-standard parameters for option instruments, including settlement calendars,
/// business day conventions, and settlement lags. Used by option builders to construct
/// instruments with correct market conventions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptionConventions {
    /// Calendar for exercise and settlement.
    pub calendar_id: String,
    /// Settlement lag in business days.
    pub settlement_days: i32,
    /// Business day convention.
    pub business_day_convention: BusinessDayConvention,
}

/// Conventions for FX spot and forward settlement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FxConventions {
    /// Base currency of the pair (numerator).
    pub base_currency: Currency,
    /// Quote currency of the pair (denominator / domestic currency).
    pub quote_currency: Currency,
    /// Standard spot settlement lag in business days.
    pub spot_lag_days: i32,
    /// Business day convention for spot and maturity adjustment.
    pub business_day_convention: BusinessDayConvention,
    /// Base currency calendar identifier used in the joint calendar.
    pub base_calendar_id: String,
    /// Quote currency calendar identifier used in the joint calendar.
    pub quote_calendar_id: String,
}

/// Conventions for fixed-rate bullet bonds in the market layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BondConventions {
    /// Currency of the bond.
    pub currency: Currency,
    /// Canonical in-code bond convention used by the bond instrument.
    pub market_convention: BondConvention,
    /// Default discount curve identifier when the builder context does not override it.
    pub default_discount_curve_id: String,
}

/// Conventions for vanilla FX options.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FxOptionConventions {
    /// Underlying FX pair convention.
    pub fx_convention_id: FxConventionId,
    /// Exercise style used by the market quote.
    pub exercise_style: ExerciseStyle,
    /// Settlement type used by the market quote.
    pub settlement: SettlementType,
    /// Day count convention used for option time to expiry.
    pub day_count: DayCount,
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
    /// as base) must invert this to `Leg2`. Defaults to `InitialAndFinal` when not
    /// specified, preserving legacy fixed-notional behaviour for entries that omit
    /// the field.
    #[serde(default)]
    pub notional_exchange: crate::instruments::rates::xccy_swap::NotionalExchange,
}

/// Conventions for Interest Rate Futures.
///
/// Defines market-standard parameters for interest rate future contracts, including contract
/// specifications (face value, tick size, tick value), delivery months, settlement lags, and
/// optional convexity adjustments. Used by futures builders to construct instruments with
/// correct market conventions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrFutureConventions {
    /// Underlying rate index identifier.
    pub index_id: IndexId,
    /// Calendar for business day adjustments.
    pub calendar_id: String,
    /// Settlement lag in business days between expiry and period start.
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
