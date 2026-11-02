use finstack_quant_core::math::Compounding;
use serde::{Deserialize, Serialize};

/// Full quote set produced by the quote engine; the return value of
/// [`compute_quotes`](super::compute_quotes).
///
/// - Prices are returned both in currency and as % of par.
/// - All yields and spreads are decimal (`0.01 = 100bp`).
/// - Every optional field is `None` when it does not apply to the bond or its
///   metric has no finite value, so an infinite or NaN value never crosses the
///   wire; JSON writes `None` as `null`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BondQuoteSet {
    /// Clean price in the bond's currency, for its whole notional.
    pub clean_price_currency: f64,
    /// Clean price as percentage of par (`99.5` = 99.5%, the quote convention).
    pub clean_price_pct: f64,
    /// Dirty price at the quote date in the bond's currency, for its whole
    /// notional: clean price plus accrued interest to the quote date.
    pub dirty_price_currency: f64,
    /// Street yield to maturity (decimal), compounded at the coupon frequency.
    pub ytm: Option<f64>,
    /// Yield to worst across every call, put and maturity exit (decimal).
    pub ytw: Option<f64>,
    /// Z-spread over the discount curve (decimal). On a bond with exercise
    /// rights it discounts the worst exit's flows.
    pub z_spread: Option<f64>,
    /// Discount margin (decimal). `None` unless the bond has floating coupons.
    pub discount_margin: Option<f64>,
    /// Option-adjusted spread (decimal), from the bond's tree model.
    pub oas: Option<f64>,
    /// Asset swap par spread (decimal). `None` unless the bond has a fixed coupon
    /// and no custom cashflows, or custom cashflows with a floating specification.
    pub asw_par: Option<f64>,
    /// Asset swap market spread (decimal). `None` under the same rule as
    /// `asw_par`, including on a plain floating-rate note, which the `asw_market`
    /// metric alone would still value.
    pub asw_market: Option<f64>,
    /// I-spread (decimal): the yield less the swap par rate. On a bond with
    /// exercise rights the yield and horizon are the worst exit's.
    pub i_spread: Option<f64>,
    /// Japanese simple yield (単利, decimal). `None` unless the coupon is fixed.
    pub japanese_simple_yield: Option<f64>,
    /// Moosmüller yield to maturity (decimal): simple interest to the next
    /// coupon, then periodic compounding.
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
