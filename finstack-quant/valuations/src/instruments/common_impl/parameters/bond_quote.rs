//! The quote a bond is marked from.
//!
//! [`BondQuoteInput`] names one market quote (a price, a yield or a spread)
//! that the bond quote conversions solve the others from, and that the
//! Merton calibration targets.

/// Quote input for the bond quote engine.
///
/// All spreads are expressed in **decimal** (`0.01 = 100bp`).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum BondQuoteInput {
    /// Clean price quoted as percentage of par (e.g., 99.5 = 99.5% of par).
    CleanPricePct(f64),
    /// Dirty price in currency units.
    DirtyPriceCurrency(f64),
    /// Yield to maturity (decimal).
    Ytm(f64),
    /// Yield to worst (decimal).
    ///
    /// For non-callable bonds this is equivalent to [`BondQuoteInput::Ytm`].
    /// For callable, puttable, or return-floor bonds, inversion prices every
    /// admissible workout path at this Street yield and selects the minimum
    /// dirty price.
    Ytw(f64),
    /// Z-spread over the discount curve (decimal).
    ZSpread(f64),
    /// Discount margin for FRNs (decimal).
    DiscountMargin(f64),
    /// Option-adjusted spread (decimal).
    Oas(f64),
    /// Asset swap market spread (decimal).
    AswMarket(f64),
    /// I-spread (decimal).
    ISpread(f64),
    /// Japanese simple yield (単利, decimal).
    ///
    /// Closed-form Tokyo quote: remaining ACT/365F life versus dirty price as
    /// a percent of par. This is **not** a discount-factor convention and does
    /// not alter Street [`BondQuoteInput::Ytm`].
    JapaneseSimpleYield(f64),
}
