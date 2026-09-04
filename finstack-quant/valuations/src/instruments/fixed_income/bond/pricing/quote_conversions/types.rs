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

/// Full quote set produced by the quote engine.
///
/// - Prices are returned both in currency and as % of par.
/// - All spreads are decimal (`0.01 = 100bp`).
#[derive(Debug, Clone)]
pub struct BondQuoteSet {
    /// Clean price in currency.
    pub clean_price_currency: f64,
    /// Clean price as percentage of par (quote convention).
    pub clean_price_pct: f64,
    /// Dirty price in currency.
    pub dirty_price_currency: f64,
    /// Yield to maturity (decimal), if applicable.
    pub ytm: Option<f64>,
    /// Yield to worst (decimal), if applicable.
    pub ytw: Option<f64>,
    /// Z-spread over discount curve (decimal), if applicable.
    pub z_spread: Option<f64>,
    /// Discount margin for FRNs (decimal), if applicable.
    pub discount_margin: Option<f64>,
    /// Option-adjusted spread (decimal), if applicable.
    pub oas: Option<f64>,
    /// Asset swap par spread (decimal), if applicable.
    pub asw_par: Option<f64>,
    /// Asset swap market spread (decimal), if applicable.
    pub asw_market: Option<f64>,
    /// I-spread (decimal), if applicable.
    pub i_spread: Option<f64>,
    /// Japanese simple yield (単利, decimal), if applicable.
    pub japanese_simple_yield: Option<f64>,
    /// Moosmüller yield to maturity (decimal), if applicable.
    pub moosmuller_ytm: Option<f64>,
}

/// Yield Compounding enumeration.
///
/// Defines how yield-to-maturity is compounded when calculating present values.
/// Different markets and instrument types use different conventions.
///
/// # Market Standard Conventions
///
/// | Convention | Use Case | Formula |
/// |------------|----------|---------|
/// | `Street` | Most secondary market trading | `(1 + y/f)^(-f*t)` |
/// | `TreasuryActual` | US Treasury new issues with stubs | Simple interest for first period |
/// | `Simple` | Money market instruments | `1/(1 + y*t)` |
/// | `Continuous` | Theoretical/academic | `exp(-y*t)` |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YieldCompounding {
    /// Simple interest: `DF = 1 / (1 + y * t)`
    ///
    /// Used for money market instruments and short-dated securities.
    Simple,

    /// Annual compounding: `DF = (1 + y)^(-t)`
    Annual,

    /// Periodic compounding with explicit periods per year: `DF = (1 + y/m)^(-m*t)`
    Periodic(u32),

    /// Continuous compounding: `DF = exp(-y * t)`
    ///
    /// Used in theoretical models and some derivative pricing.
    Continuous,

    /// Street convention: periodic compounding aligned with bond's coupon frequency.
    ///
    /// This is the standard convention for secondary market bond trading.
    /// Formula: `DF = (1 + y/f)^(-f*t)` where `f` is coupon frequency.
    Street,

    /// U.S. Treasury actual convention for regular and irregular first periods.
    ///
    /// The schedule-aware pricing path decomposes the first horizon into an
    /// initial simple-interest fractional quasi-coupon and zero or more full
    /// periodically compounded coupon periods, following 31 CFR Part 356,
    /// Appendix B, section II.
    ///
    /// # When to Use
    ///
    /// - U.S. Treasury auction/new-issue price conversion
    /// - Regular, short-first, and long-first payment periods
    /// - Source-backed Treasury price/yield reconciliation
    ///
    /// The standalone [`super::df_from_yield`] helper has no schedule and can
    /// only infer the initial fraction from `t`. Use
    /// [`super::price_from_ytm_compounded_params`] for irregular schedules.
    TreasuryActual,

    /// Moosmüller: simple interest to the next coupon, then periodic compounding.
    ///
    /// ```text
    /// PV = 1/(1 + y*w) * [CF_1 + Σ_{k≥2} CF_k / (1 + y/f)^{k-1}]
    /// ```
    ///
    /// `w` is the year fraction from settlement to the next coupon (bond day
    /// count) and `f` is coupon frequency. Used by the `moosmuller_ytm` metric.
    /// Street `ytm` is unchanged.
    ///
    /// On a coupon date `w = 1/f`, so the simple factor is absorbed into
    /// periodic compounding and the discount factors match [`Self::Street`].
    Moosmuller,
}
