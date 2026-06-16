//! Inflation-linked bond specific parameters.

use finstack_quant_core::{
    dates::{Date, DayCount, Tenor},
    money::Money,
};
use rust_decimal::Decimal;

/// Inflation-linked bond specific parameters.
///
/// Groups parameters specific to inflation-linked bonds.
#[derive(Debug, Clone)]
pub struct InflationLinkedBondParams {
    /// Notional amount
    pub notional: Money,
    /// Real coupon rate
    pub real_coupon: Decimal,
    /// Issue date
    pub issue: Date,
    /// Maturity date
    pub maturity: Date,
    /// Base index value at issue
    pub base_index: f64,
    /// Payment frequency
    pub frequency: Tenor,
    /// Day count convention
    pub day_count: DayCount,
}

impl InflationLinkedBondParams {
    /// Create new inflation-linked bond parameters
    ///
    /// # Errors
    ///
    /// Returns an error if `real_coupon` is not representable as `Decimal` (e.g., NaN or Inf).
    pub fn new(
        notional: Money,
        real_coupon: f64,
        issue: Date,
        maturity: Date,
        base_index: f64,
        frequency: Tenor,
        day_count: DayCount,
    ) -> finstack_quant_core::Result<Self> {
        Ok(Self {
            notional,
            real_coupon: finstack_quant_core::decimal::f64_to_decimal(real_coupon)?,
            issue,
            maturity,
            base_index,
            frequency,
            day_count,
        })
    }

    /// Create US TIPS parameters (semi-annual, Act/Act)
    ///
    /// # Errors
    ///
    /// Returns an error if `real_coupon` is not representable as `Decimal` (e.g., NaN or Inf).
    pub fn tips(
        notional: Money,
        real_coupon: f64,
        issue: Date,
        maturity: Date,
        base_index: f64,
    ) -> finstack_quant_core::Result<Self> {
        Self::new(
            notional,
            real_coupon,
            issue,
            maturity,
            base_index,
            Tenor::semi_annual(),
            DayCount::ActActIsma,
        )
    }

    /// Create UK linker parameters (semi-annual, Act/Act)
    ///
    /// # Errors
    ///
    /// Returns an error if `real_coupon` is not representable as `Decimal` (e.g., NaN or Inf).
    pub fn uk_linker(
        notional: Money,
        real_coupon: f64,
        issue: Date,
        maturity: Date,
        base_index: f64,
    ) -> finstack_quant_core::Result<Self> {
        Self::new(
            notional,
            real_coupon,
            issue,
            maturity,
            base_index,
            Tenor::semi_annual(),
            DayCount::ActActIsma,
        )
    }
}
