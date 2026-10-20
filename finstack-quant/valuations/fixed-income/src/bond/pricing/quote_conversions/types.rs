use finstack_quant_core::math::Compounding;

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
/// Plain rate bases reuse the canonical core [`Compounding`]; the remaining
/// variants are schedule-aware bond conventions core cannot express.
///
/// # Market Standard Conventions
///
/// | Convention | Use Case | Formula |
/// |------------|----------|---------|
/// | `Street` | Most secondary market trading | `(1 + y/f)^(-f*t)` |
/// | `Rate(Simple)` | Money market instruments | `1/(1 + y*t)` |
/// | `Rate(Annual)` | Annual bond yields | `(1 + y)^(-t)` |
/// | `Rate(Periodic(m))` | Explicit periods per year | `(1 + y/m)^(-m*t)` |
/// | `Rate(Continuous)` | Theoretical/academic | `exp(-y*t)` |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YieldCompounding {
    /// A plain rate basis from the canonical core [`Compounding`].
    Rate(Compounding),

    /// Street convention: periodic compounding aligned with bond's coupon frequency.
    ///
    /// This is the standard convention for secondary market bond trading.
    /// Formula: `DF = (1 + y/f)^(-f*t)` where `f` is coupon frequency.
    Street,

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
