//! Bond quote engine for mapping between price, yields, and spreads.
//!
//! This module provides a small, opinionated API that takes **one**
//! quote input (price, yield, or spread) and produces a consistent
//! set of derived bond quotes using the existing pricing and metric
//! infrastructure.
//!
//! All spread-style quantities exposed here use **decimal units**:
//! `0.01` corresponds to **100 basis points**.
use crate::constants::numerical::ZERO_TOLERANCE;
use crate::instruments::common_impl::pricing::time::{rate_between_on_dates, rate_period_on_dates};
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::bond::metrics::price_yield_spread::z_spread::{
    bond_z_spread_compounding_frequency, z_spread_discount_factor,
};
use crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext;
use crate::instruments::fixed_income::bond::Bond;
use crate::metrics::{standard_registry, MetricRegistry};
use crate::metrics::{MetricContext, MetricId};
use finstack_quant_core::dates::Date;
use finstack_quant_core::dates::DayCountContext;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use rust_decimal::prelude::ToPrimitive;
use std::sync::Arc;

fn resolved_asw_forward_curve_id(bond: &Bond) -> Option<CurveId> {
    bond.pricing_overrides
        .model_config
        .asw_forward_curve_id
        .clone()
        .or_else(|| bond.forward_curve_id.clone())
}

/// Convert payment frequency to approximate periods per year.
///
/// **Important:** This function is for **frequency conversion only**, NOT day count conventions.
///
/// # Purpose
///
/// This helper determines how many payment periods occur in a year based on the
/// payment frequency. For example, semi-annual payments occur 2 times per year,
/// monthly payments occur 12 times per year.
///
/// # Day Count Conventions
///
/// Actual day count calculations (Actual/360, Actual/365, Actual/Actual, 30/360, etc.)
/// are handled separately via the `DayCount` enum and `year_fraction()` methods in
/// finstack-quant-core. Those methods properly account for:
/// - Leap years (Actual/Actual)
/// - Different day count bases (360 vs 365)
/// - Month length variations (30/360)
///
/// # Arguments
///
/// * `freq` - Payment frequency (e.g., `Tenor::semi_annual()`)
///
/// # Returns
///
/// Number of periods per year as `f64`.
///
/// # Errors
///
/// Returns `Err` when:
/// - Tenor is zero (invalid)
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::periods_per_year;
/// use finstack_quant_core::dates::Tenor;
///
/// assert_eq!(periods_per_year(Tenor::semi_annual())?, 2.0);
/// assert_eq!(periods_per_year(Tenor::quarterly())?, 4.0);
/// assert_eq!(periods_per_year(Tenor::annual())?, 1.0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Note on Daily Tenor
///
/// For daily frequencies, this uses 365 as an approximation of annual periods.
/// This is appropriate for frequency calculations but should NOT be confused with
/// the Actual/365 day count convention used in accrual and discount factor calculations.
#[inline]
pub fn periods_per_year(
    freq: finstack_quant_core::dates::Tenor,
) -> finstack_quant_core::Result<f64> {
    match freq.unit() {
        finstack_quant_core::dates::TenorUnit::Months => {
            if freq.count() == 0 {
                return Err(finstack_quant_core::InputError::Invalid.into());
            }
            Ok(12.0 / (freq.count() as f64))
        }
        finstack_quant_core::dates::TenorUnit::Days => {
            if freq.count() == 0 {
                return Err(finstack_quant_core::InputError::Invalid.into());
            }
            // Use 365 as approximate annual basis for frequency calculations
            // Note: This is NOT a day count convention - actual day count is handled
            // via the DayCount enum (Actual/360, Actual/365, Actual/Actual, etc.)
            Ok(365.0 / (freq.count() as f64))
        }
        finstack_quant_core::dates::TenorUnit::Years => {
            if freq.count() == 0 {
                return Err(finstack_quant_core::InputError::Invalid.into());
            }
            Ok(1.0 / (freq.count() as f64))
        }
        finstack_quant_core::dates::TenorUnit::Weeks => {
            if freq.count() == 0 {
                return Err(finstack_quant_core::InputError::Invalid.into());
            }
            Ok(52.0 / (freq.count() as f64))
        }
    }
}

/// Fixed-leg annuity for a bond-style schedule using discount-curve discount factors.
///
/// This computes the standard swap-style annuity:
/// ```text
/// Annuity = Σ (α_i · P(as_of, T_i))
/// ```
/// where `α_i` is the year fraction between consecutive schedule dates under `dc`,
/// and `P(as_of, T_i)` is the discount factor from `as_of` to date `T_i`.
///
/// The `schedule` is expected to start at the valuation date (`as_of`) and
/// contain strictly increasing dates.
///
/// # Arguments
///
/// * `disc` - Discount curve for discount factor calculations
/// * `dc` - Day count convention for year fraction calculations
/// * `schedule` - Schedule of coupon payment dates (must start at `as_of`)
///
/// # Returns
///
/// The fixed-leg annuity value.
///
/// # Errors
///
/// Returns an error if any year_fraction calculation fails (e.g., invalid dates).
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::fixed_leg_annuity;
/// use finstack_quant_core::market_data::term_structures::DiscountCurve;
/// use finstack_quant_core::dates::{DayCount, Date};
///
/// # let disc = DiscountCurve::builder("USD-OIS").base_date(Date::from_calendar_date(2024, time::Month::January, 1).unwrap()).knots([(0.0, 1.0)]).build().unwrap();
/// # let schedule = vec![Date::from_calendar_date(2024, time::Month::January, 1).unwrap(), Date::from_calendar_date(2025, time::Month::January, 1).unwrap()];
/// let annuity = fixed_leg_annuity(&disc, DayCount::Act365F, None, &schedule)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn fixed_leg_annuity(
    disc: &finstack_quant_core::market_data::term_structures::DiscountCurve,
    dc: finstack_quant_core::dates::DayCount,
    frequency: Option<finstack_quant_core::dates::Tenor>,
    schedule: &[Date],
) -> finstack_quant_core::Result<f64> {
    use finstack_quant_core::dates::DayCountContext;

    if schedule.len() < 2 {
        return Ok(0.0);
    }

    let dc_ctx = DayCountContext {
        frequency,
        ..DayCountContext::default()
    };
    let mut ann = 0.0;
    let mut prev = schedule[0];
    for &d in &schedule[1..] {
        let alpha = dc.year_fraction(prev, d, dc_ctx)?;
        let p = disc.df_on_date_curve(d)?;
        ann += alpha * p;
        prev = d;
    }
    Ok(ann)
}

/// Par swap rate from discount-curve discount ratios and a fixed-leg annuity.
///
/// Uses the standard discount-ratio formula:
/// ```text
/// par_rate = (P(as_of, T₀) - P(as_of, Tₙ)) / Annuity
/// ```
/// where the denominator is the fixed-leg annuity computed with `dc`.
///
/// Returns both the par rate and the annuity so callers can reuse the latter
/// in asset-swap formulas and related analytics.
///
/// # Arguments
///
/// * `disc` - Discount curve for discount factor calculations
/// * `dc` - Day count convention for year fraction calculations
/// * `schedule` - Schedule of coupon payment dates
///
/// # Returns
///
/// Tuple of `(par_rate, annuity)` where:
/// - `par_rate` is the par swap rate (decimal, e.g., 0.05 for 5%)
/// - `annuity` is the fixed-leg annuity value
///
/// # Errors
///
/// Returns an error if the annuity calculation fails (invalid dates/day-count).
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::par_rate_and_annuity_from_discount;
/// use finstack_quant_core::market_data::term_structures::DiscountCurve;
/// use finstack_quant_core::dates::{DayCount, Date};
///
/// # let disc = DiscountCurve::builder("USD-OIS").base_date(Date::from_calendar_date(2024, time::Month::January, 1).unwrap()).knots([(0.0, 1.0)]).build().unwrap();
/// # let schedule = vec![Date::from_calendar_date(2024, time::Month::January, 1).unwrap(), Date::from_calendar_date(2025, time::Month::January, 1).unwrap()];
/// let (par_rate, annuity) = par_rate_and_annuity_from_discount(&disc, DayCount::Act365F, None, &schedule)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn par_rate_and_annuity_from_discount(
    disc: &finstack_quant_core::market_data::term_structures::DiscountCurve,
    dc: finstack_quant_core::dates::DayCount,
    frequency: Option<finstack_quant_core::dates::Tenor>,
    schedule: &[Date],
) -> finstack_quant_core::Result<(f64, f64)> {
    if schedule.len() < 2 {
        return Ok((0.0, 0.0));
    }

    let ann = fixed_leg_annuity(disc, dc, frequency, schedule)?;
    // Use epsilon check to avoid division by near-zero values that could amplify numerical noise
    if ann.abs() < 1e-12 {
        return Ok((0.0, 0.0));
    }

    let p0 = disc.df_on_date_curve(schedule[0])?;
    // `schedule.len() >= 2` by the guard above, so `schedule[0]` and `schedule[last]` are safe.
    let pn_date = schedule[schedule.len() - 1];
    let pn = disc.df_on_date_curve(pn_date)?;
    let num = p0 - pn;
    Ok((num / ann, ann))
}

/// Forward-projected par rate and fixed-leg annuity for an asset-swap schedule.
pub fn par_rate_and_annuity_from_forward(
    disc: &finstack_quant_core::market_data::term_structures::DiscountCurve,
    fwd: &finstack_quant_core::market_data::term_structures::ForwardCurve,
    fixed_dc: finstack_quant_core::dates::DayCount,
    fixed_frequency: Option<finstack_quant_core::dates::Tenor>,
    schedule: &[Date],
    float_spread_bp: f64,
) -> finstack_quant_core::Result<(f64, f64)> {
    let ann = fixed_leg_annuity(disc, fixed_dc, fixed_frequency, schedule)?;
    if ann.abs() < 1e-12 {
        return Ok((0.0, 0.0));
    }

    let f_dc = fwd.day_count();
    let spread = float_spread_bp * 1e-4;
    let mut pv_float = finstack_quant_core::math::summation::NeumaierAccumulator::new();
    let mut prev = schedule[0];
    for &d in &schedule[1..] {
        let yf = f_dc.year_fraction(prev, d, DayCountContext::default())?;
        let rate = asset_swap_projection_rate(fwd, prev, d)? + spread;
        let df = disc.df_on_date_curve(d)?;
        pv_float.add(rate * yf * df);
        prev = d;
    }

    Ok((pv_float.total() / ann, ann))
}

/// Asset-swap forward leg PV and fixed/floating annuities per unit notional.
pub fn asset_swap_forward_components(
    disc: &finstack_quant_core::market_data::term_structures::DiscountCurve,
    fwd: &finstack_quant_core::market_data::term_structures::ForwardCurve,
    fixed_dc: finstack_quant_core::dates::DayCount,
    fixed_frequency: Option<finstack_quant_core::dates::Tenor>,
    schedule: &[Date],
    float_spread_bp: f64,
) -> finstack_quant_core::Result<(f64, f64, f64)> {
    let fixed_ann = fixed_leg_annuity(disc, fixed_dc, fixed_frequency, schedule)?;
    if schedule.len() < 2 {
        return Ok((0.0, fixed_ann, 0.0));
    }

    let f_dc = fwd.day_count();
    let spread = float_spread_bp * 1e-4;
    let mut float_pv = finstack_quant_core::math::summation::NeumaierAccumulator::new();
    let mut float_ann = finstack_quant_core::math::summation::NeumaierAccumulator::new();
    let mut prev = schedule[0];
    for &d in &schedule[1..] {
        let yf = f_dc.year_fraction(prev, d, DayCountContext::default())?;
        let df = disc.df_on_date_curve(d)?;
        float_pv.add((asset_swap_projection_rate(fwd, prev, d)? + spread) * yf * df);
        float_ann.add(yf * df);
        prev = d;
    }

    Ok((float_pv.total(), fixed_ann, float_ann.total()))
}

/// Project an asset-swap floating coupon from the curve's index convention.
///
/// Overnight indices represent observation rates that are averaged over the
/// coupon window. Term indices instead use the discount-factor-implied simple
/// forward for the whole accrual period.
pub(crate) fn asset_swap_projection_rate(
    fwd: &finstack_quant_core::market_data::term_structures::ForwardCurve,
    start: Date,
    end: Date,
) -> finstack_quant_core::Result<f64> {
    const MAX_OVERNIGHT_TENOR_YEARS: f64 = 1.0 / 52.0;

    if fwd.tenor() <= MAX_OVERNIGHT_TENOR_YEARS {
        rate_period_on_dates(fwd, start, end)
    } else {
        rate_between_on_dates(fwd, start, end)
    }
}

/// Quote input for the bond quote engine.
///
/// All spreads are expressed in **decimal** (`0.01 = 100bp`).
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BondQuoteInput {
    /// Clean price quoted as percentage of par (e.g., 99.5 = 99.5% of par).
    CleanPricePct(f64),
    /// Dirty price in currency units.
    DirtyPriceCcy(f64),
    /// Yield to maturity (decimal).
    Ytm(f64),
    /// Yield to worst (decimal).
    ///
    /// For non-callable bonds this is equivalent to [`BondQuoteInput::Ytm`].
    /// For callable bonds, prefer [`BondQuoteInput::Oas`] when exercise-aware
    /// pricing is required — YTW inversion via this variant uses maturity flows
    /// (consistent with `Bond::base_value`'s `quoted_ytw` path).
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
}

/// Full quote set produced by the quote engine.
///
/// - Prices are returned both in currency and as % of par.
/// - All spreads are decimal (`0.01 = 100bp`).
#[derive(Debug, Clone)]
pub struct BondQuoteSet {
    /// Clean price in currency.
    pub clean_price_ccy: f64,
    /// Clean price as percentage of par (quote convention).
    pub clean_price_pct: f64,
    /// Dirty price in currency.
    pub dirty_price_ccy: f64,
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
}

// ============================================================================
// Price-from-Metric Functions
// ============================================================================

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

    /// ISDA/Treasury actual convention with simple interest for odd first period.
    ///
    /// Uses simple interest `1/(1 + y*t)` for the first (potentially irregular) period,
    /// then switches to periodic compounding for subsequent periods. This matches
    /// the official SEC/Treasury methodology for new issue pricing with stub periods.
    ///
    /// # When to Use
    ///
    /// - US Treasury new issues with short first coupons
    /// - Regulatory yield calculations requiring ISDA compliance
    /// - Benchmarking against official Bloomberg/Reuters Treasury yields
    ///
    /// # Typical Difference
    ///
    /// The difference vs `Street` convention is typically < 0.5 basis points for
    /// seasoned bonds, but can be 1-2 basis points for new issues with significant stubs.
    ///
    /// # Limitation
    ///
    /// Stub period detection is **time-based**, using `t < 1/frequency` as the criterion.
    /// This works correctly for standard bonds but may misclassify stubs on bonds with
    /// irregular first coupons that don't align with the standard frequency (e.g., a
    /// long-first stub spanning 8 months on a semi-annual bond).
    TreasuryActual,
}

/// Discount factor from yield.
///
/// Computes the discount factor for a given yield, time, and compounding convention.
///
/// # Arguments
///
/// * `ytm` - Yield to maturity as decimal (e.g., 0.05 for 5%)
/// * `t` - Time in years from valuation date to cashflow date
/// * `comp` - Compounding convention (see [`YieldCompounding`])
/// * `bond_freq` - Bond's coupon frequency (used for `Street` and `TreasuryActual`)
///
/// # Compounding Formulas
///
/// | Convention | Formula |
/// |------------|---------|
/// | Simple | `1 / (1 + y * t)` |
/// | Annual | `(1 + y)^(-t)` |
/// | Periodic(m) | `(1 + y/m)^(-m*t)` |
/// | Continuous | `exp(-y * t)` |
/// | Street | `(1 + y/f)^(-f*t)` where f = frequency |
/// | TreasuryActual | Simple for t < 1/f, then periodic |
///
/// # Errors
///
/// Returns `Err` if the bond frequency is invalid (zero periods).
///
/// # Negative Yields
///
/// Negative yields are supported for all compounding conventions. However:
/// - **Extreme negative yields** (< -50%) will log a warning as they often indicate
///   data or input errors.
/// - For periodic/annual compounding, yields more negative than `-m` (where `m` is
///   compounding frequency) would make `(1 + y/m)` negative, leading to `NaN` from
///   `powf`. Such cases return `Err`.
/// - Discount factors > 1.0 are mathematically valid for negative rates but unusual
///   in practice.
#[inline]
pub fn df_from_yield(
    ytm: f64,
    t: f64,
    comp: YieldCompounding,
    bond_freq: finstack_quant_core::dates::Tenor,
) -> finstack_quant_core::Result<f64> {
    if t <= 0.0 {
        return Ok(1.0);
    }

    // Warn on extreme negative yields which often indicate data errors
    if ytm < -0.5 {
        tracing::warn!(
            ytm = ytm,
            "Extreme negative yield detected (< -50%). This may indicate a data error."
        );
    }

    Ok(match comp {
        YieldCompounding::Simple => {
            let denom = 1.0 + ytm * t;
            // Check for non-positive denominator which would give invalid discount factor
            if denom <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Simple interest denominator (1 + y*t) = {} is non-positive for ytm={}, t={}",
                    denom, ytm, t
                )));
            }
            1.0 / denom
        }
        YieldCompounding::Annual => {
            let base = 1.0 + ytm;
            if base <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Annual compounding base (1 + y) = {} is non-positive for ytm={}",
                    base, ytm
                )));
            }
            base.powf(-t)
        }
        YieldCompounding::Periodic(m) => {
            let m = m as f64;
            let base = 1.0 + ytm / m;
            if base <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Periodic compounding base (1 + y/m) = {} is non-positive for ytm={}, m={}",
                    base, ytm, m
                )));
            }
            base.powf(-m * t)
        }
        YieldCompounding::Continuous => (-ytm * t).exp(),
        YieldCompounding::Street => {
            let m = periods_per_year(bond_freq)?.max(1.0);
            let base = 1.0 + ytm / m;
            if base <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Street compounding base (1 + y/m) = {} is non-positive for ytm={}, m={}",
                    base, ytm, m
                )));
            }
            base.powf(-m * t)
        }
        YieldCompounding::TreasuryActual => {
            // ISDA/Treasury actual convention:
            // - Use simple interest for the first (potentially irregular) period
            // - Use periodic compounding for subsequent full periods
            //
            // LIMITATION: Stub period detection is TIME-BASED, not SCHEDULE-AWARE.
            // We identify the first period as t < 1/frequency (i.e., less than
            // one full coupon period). This is a reasonable approximation that
            // captures the essence of the convention for standard bonds.
            //
            // For bonds with irregular first coupons that don't align with the
            // standard frequency (e.g., a long-first stub spanning 8 months on
            // a semi-annual bond), this heuristic may misclassify the stub.
            // For exact ISDA compliance with non-standard structures, consider
            // passing actual stub information from the cashflow schedule.
            let m = periods_per_year(bond_freq)?.max(1.0);
            let period_length = 1.0 / m;

            // Validate periodic compounding base for extreme negative yields
            let periodic_base = 1.0 + ytm / m;
            if periodic_base <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "TreasuryActual periodic base (1 + y/m) = {} is non-positive for ytm={}, m={}",
                    periodic_base, ytm, m
                )));
            }

            if t <= period_length {
                // First (potentially stub) period: simple interest
                let denom = 1.0 + ytm * t;
                if denom <= 0.0 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "TreasuryActual simple interest denom (1 + y*t) = {} is non-positive for ytm={}, t={}",
                        denom, ytm, t
                    )));
                }
                1.0 / denom
            } else {
                // For subsequent periods, we need to compound:
                // - Simple interest for the first period portion
                // - Periodic compounding for the remaining full periods
                //
                // Total time t = stub_time + n_full_periods / m
                // where stub_time <= period_length
                //
                // DF = DF_stub * DF_periodic
                //    = 1/(1 + y*stub) * (1 + y/m)^(-n_full_periods)
                let n_full_periods = (t * m).floor();
                let stub_time = t - n_full_periods / m;

                if stub_time > 1e-10 {
                    // Has a stub period
                    let stub_denom = 1.0 + ytm * stub_time;
                    if stub_denom <= 0.0 {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "TreasuryActual stub denom (1 + y*stub) = {} is non-positive for ytm={}, stub_time={}",
                            stub_denom, ytm, stub_time
                        )));
                    }
                    let df_stub = 1.0 / stub_denom;
                    let df_periodic = periodic_base.powf(-n_full_periods);
                    df_stub * df_periodic
                } else {
                    // No stub, pure periodic
                    periodic_base.powf(-m * t)
                }
            }
        }
    })
}

/// `TreasuryActual` discount factor with a schedule-flagged first-period length.
///
/// Unlike [`df_from_yield`], which infers the first (stub) period purely from time
/// (`t <= 1/m`), this variant takes the **actual** first-coupon period length
/// `first_period_len` derived from the bond's cashflow schedule. Simple interest
/// is applied over the whole first period — long, short, or regular — and periodic
/// compounding over the remaining full periods. This avoids the 1-2bp
/// misclassification on new issues with irregular (notably long) first coupons.
fn df_treasury_actual_with_first_period(
    ytm: f64,
    t: f64,
    m: f64,
    first_period_len: f64,
) -> finstack_quant_core::Result<f64> {
    // First-period simple-interest leg over `min(t, first_period_len)`.
    let stub_t = t.min(first_period_len);
    let stub_denom = 1.0 + ytm * stub_t;
    if stub_denom <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "TreasuryActual simple interest denom (1 + y*t) = {stub_denom} is non-positive for ytm={ytm}, t={stub_t}"
        )));
    }
    let df_stub = 1.0 / stub_denom;

    if t <= first_period_len {
        return Ok(df_stub);
    }

    // Periodic compounding over the remaining time after the first period.
    let periodic_base = 1.0 + ytm / m;
    if periodic_base <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "TreasuryActual periodic base (1 + y/m) = {periodic_base} is non-positive for ytm={ytm}, m={m}"
        )));
    }
    let remaining = t - first_period_len;
    Ok(df_stub * periodic_base.powf(-m * remaining))
}

/// Price from yield using explicit day count and frequency (no `Bond` borrow required).
///
/// For the [`YieldCompounding::TreasuryActual`] convention the first (potentially
/// irregular) coupon period is flagged from the **cashflow schedule** — the
/// year-fraction to the first post-`as_of` flow — rather than inferred from time.
/// This keeps the YTM↔price conversion correct for new issues with long first
/// coupons, where the time-based `t <= 1/m` heuristic in [`df_from_yield`] would
/// misapply simple interest to the wrong horizon.
#[inline]
pub fn price_from_ytm_compounded_params(
    day_count: finstack_quant_core::dates::DayCount,
    freq: finstack_quant_core::dates::Tenor,
    flows: &[(
        finstack_quant_core::dates::Date,
        finstack_quant_core::money::Money,
    )],
    as_of: finstack_quant_core::dates::Date,
    ytm: f64,
    comp: YieldCompounding,
) -> finstack_quant_core::Result<f64> {
    use finstack_quant_core::math::summation::NeumaierAccumulator;

    // ACT/ACT (ICMA) requires the coupon frequency in the day-count context;
    // the default context hard-errors for that convention.
    let dc_ctx = DayCountContext {
        frequency: Some(freq),
        ..DayCountContext::default()
    };

    // Schedule-aware first-period length for the TreasuryActual stub: the
    // year-fraction from `as_of` to the first cashflow strictly after `as_of`.
    let treasury_first_period = if matches!(comp, YieldCompounding::TreasuryActual) {
        let mut first: Option<f64> = None;
        for &(date, _) in flows {
            if date <= as_of {
                continue;
            }
            let yf = day_count.year_fraction(as_of, date, dc_ctx)?;
            if yf > 0.0 {
                first = Some(yf);
                break;
            }
        }
        first
    } else {
        None
    };

    let mut pv = NeumaierAccumulator::new();
    for &(date, amount) in flows {
        if date <= as_of {
            continue;
        }
        let t = day_count.year_fraction(as_of, date, dc_ctx)?;
        if t > 0.0 {
            let df = match (comp, treasury_first_period) {
                (YieldCompounding::TreasuryActual, Some(first_period_len)) => {
                    let m = periods_per_year(freq)?.max(1.0);
                    df_treasury_actual_with_first_period(ytm, t, m, first_period_len)?
                }
                _ => df_from_yield(ytm, t, comp, freq)?,
            };
            pv.add(amount.amount() * df);
        }
    }
    Ok(pv.total())
}

/// Price from ytm compounded.
pub fn price_from_ytm_compounded(
    bond: &Bond,
    flows: &[(
        finstack_quant_core::dates::Date,
        finstack_quant_core::money::Money,
    )],
    as_of: finstack_quant_core::dates::Date,
    ytm: f64,
    comp: YieldCompounding,
) -> finstack_quant_core::Result<f64> {
    price_from_ytm_compounded_params(
        bond.cashflow_spec.day_count(),
        bond.cashflow_spec.frequency(),
        flows,
        as_of,
        ytm,
        comp,
    )
}

/// Price from ytm (using Street convention).
pub fn price_from_ytm(
    bond: &Bond,
    flows: &[(
        finstack_quant_core::dates::Date,
        finstack_quant_core::money::Money,
    )],
    as_of: finstack_quant_core::dates::Date,
    ytm: f64,
) -> finstack_quant_core::Result<f64> {
    price_from_ytm_compounded(bond, flows, as_of, ytm, YieldCompounding::Street)
}

/// Compute outstanding principal at a given date from the cashflow schedule.
///
/// This is used by YTW and other yield calculations to determine the
/// redemption amount for amortizing callable/putable bonds.
pub(crate) fn outstanding_principal_at_date(
    schedule: &crate::cashflow::builder::CashFlowSchedule,
    target_date: Date,
) -> f64 {
    use crate::cashflow::primitives::CFKind;

    let initial = schedule.notional.initial.amount();
    let mut outstanding = initial;

    // Sum all amortization and principal payments up to (and including) target_date
    for cf in &schedule.flows {
        if cf.date > target_date {
            break;
        }
        if matches!(cf.kind, CFKind::Amortization | CFKind::Notional) && cf.amount.amount() > 0.0 {
            outstanding -= cf.amount.amount();
        }
    }

    outstanding.max(0.0)
}

/// One candidate early-exit for yield-to-worst enumeration.
///
/// Represents a single admissible exercise date and the corresponding clean
/// redemption price expressed as a percentage of par (e.g. `103.0` for 103%).
#[derive(Debug, Clone, Copy)]
pub(crate) struct ExitCandidate {
    /// The admissible exercise date (call or put date, aligned to a flow date
    /// where possible, and clipped to `[as_of, bond.maturity]`).
    pub(crate) date: Date,
    /// Clean redemption price as percent of par (e.g. `103.0` for 103%).
    pub(crate) price_pct_of_par: f64,
}

/// Enumerate call/put exit candidates for yield-to-worst analysis.
///
/// For each call or put window `[start_date, end_date]` in `bond.call_put`,
/// this function produces one `ExitCandidate` per admissible exercise date:
///
/// 1. Seed with `start_date` and `end_date`.
/// 2. Extend with any flow dates that fall within `[start_date, end_date]`.
/// 3. Sort and de-duplicate the resulting dates.
/// 4. Retain only dates in `[as_of, bond.maturity]`.
///
/// Returns an empty `Vec` when the bond has no `call_put` schedule.
///
/// # Arguments
///
/// * `bond`  – The bond whose `call_put` schedule is enumerated.
/// * `flows` – Holder-view cashflows used to align candidates to payment dates.
/// * `as_of` – Earliest admissible exercise date (valuation/quote date).
pub(crate) fn enumerate_exit_paths(
    bond: &Bond,
    flows: &[(Date, Money)],
    as_of: Date,
) -> Vec<ExitCandidate> {
    let Some(cp) = &bond.call_put else {
        return Vec::new();
    };

    let mut call_candidates: Vec<ExitCandidate> = Vec::new();
    let mut put_candidates: Vec<ExitCandidate> = Vec::new();

    let push_period_candidates = |candidates: &mut Vec<ExitCandidate>,
                                  start_date: Date,
                                  end_date: Date,
                                  price_pct_of_par: f64| {
        let align_to_flow_date = |boundary: Date| {
            flows
                .iter()
                .map(|(date, _)| *date)
                .filter_map(|date| {
                    let distance = (date - boundary).whole_days().unsigned_abs();
                    (distance <= 7).then_some((distance, date))
                })
                .min()
                .map_or(boundary, |(_, date)| date)
        };
        let aligned_start = align_to_flow_date(start_date);
        let aligned_end = align_to_flow_date(end_date);
        let mut exercise_dates = vec![aligned_start, aligned_end];
        exercise_dates.extend(
            flows
                .iter()
                .map(|(date, _)| *date)
                .filter(|date| *date >= aligned_start && *date <= aligned_end),
        );
        exercise_dates.sort_unstable();
        exercise_dates.dedup();

        for exercise_date in exercise_dates {
            if exercise_date >= as_of && exercise_date <= bond.maturity {
                candidates.push(ExitCandidate {
                    date: exercise_date,
                    price_pct_of_par,
                });
            }
        }
    };

    for c in &cp.calls {
        push_period_candidates(
            &mut call_candidates,
            c.start_date,
            c.end_date,
            c.price_pct_of_par,
        );
    }
    for p in &cp.puts {
        push_period_candidates(
            &mut put_candidates,
            p.start_date,
            p.end_date,
            p.price_pct_of_par,
        );
    }

    // Adjacent step-down windows share boundary dates. At such a boundary the
    // issuer exercises the cheapest call, while the holder exercises the most
    // valuable put. Retaining both stale and current strikes creates
    // economically impossible YTW paths.
    call_candidates.sort_by(|left, right| {
        left.date
            .cmp(&right.date)
            .then_with(|| left.price_pct_of_par.total_cmp(&right.price_pct_of_par))
    });
    call_candidates.dedup_by_key(|candidate| candidate.date);
    put_candidates.sort_by(|left, right| {
        left.date
            .cmp(&right.date)
            .then_with(|| right.price_pct_of_par.total_cmp(&left.price_pct_of_par))
    });
    put_candidates.dedup_by_key(|candidate| candidate.date);
    call_candidates.extend(put_candidates);

    call_candidates
}

/// Solve yield-to-worst over all call/put/maturity candidates for a given flow set.
///
/// Returns the worst (minimum) yield and the corresponding truncated cashflow path.
///
/// # Call/Put Redemption Convention
///
/// Call/put redemption prices are dirty street redemption amounts:
/// `outstanding_principal × (price_pct_of_par / 100) + accrued_interest(exercise_date)`,
/// where `outstanding_principal` is the remaining principal at the exercise date after
/// any amortization. This correctly handles amortizing callable bonds and is consistent
/// with the tree-based OAS pricing.
///
/// # Arguments
///
/// * `bond` - The bond to calculate YTW for
/// * `flows` - Holder-view cashflows (coupons + principal)
/// * `as_of` - Valuation/quote date
/// * `dirty_price_target` - Target dirty price to match
/// * `schedule` - Optional full cashflow schedule for accurate outstanding principal
///   computation on amortizing bonds. When `None`, falls back to original notional.
pub(crate) fn solve_ytw_from_flows(
    bond: &Bond,
    flows: &[(Date, Money)],
    as_of: Date,
    dirty_price_target: Money,
    schedule: Option<&crate::cashflow::builder::CashFlowSchedule>,
) -> finstack_quant_core::Result<(f64, Vec<(Date, Money)>)> {
    // Generate call/put candidates + maturity.
    // Call/put paths come from enumerate_exit_paths; maturity is appended separately.
    let exit_paths = enumerate_exit_paths(bond, flows, as_of);
    let mut candidates: Vec<(Date, Money)> = exit_paths
        .into_iter()
        .map(|ec| {
            (
                ec.date,
                Money::new(ec.price_pct_of_par, bond.notional.currency()),
            )
        })
        .collect();

    // At maturity, principal redemption is already present in the cashflow schedule,
    // so use a zero additional redemption here to avoid double-counting.
    //
    // The redemption Notional flow is dated on the BDC-adjusted maturity, which can
    // roll past the unadjusted `bond.maturity` (e.g. maturity falling on a holiday),
    // so truncate the maturity candidate at the final projected flow date instead of
    // dropping the redemption.
    let maturity_candidate = flows
        .iter()
        .map(|(d, _)| *d)
        .max()
        .map_or(bond.maturity, |last| last.max(bond.maturity));
    candidates.push((
        maturity_candidate,
        Money::new(0.0, bond.notional.currency()),
    ));

    let mut best_yield = f64::INFINITY;
    let mut best_flows: Vec<(Date, Money)> = Vec::new();

    for (exercise_date, pct_or_zero) in candidates {
        // Truncate flows to exercise and add redemption
        let mut ex_flows: Vec<(Date, Money)> = Vec::with_capacity(flows.len());
        for &(d, a) in flows {
            if d > as_of && d <= exercise_date {
                ex_flows.push((d, a));
            }
        }

        // Compute redemption amount:
        // - For maturity: pct is 0, so redemption is 0 (already in flows)
        // - For call/put: use dirty street redemption at exercise date
        let redemption = if pct_or_zero.amount() > 0.0 {
            // This is a call/put candidate, pct_or_zero holds the price_pct_of_par
            let pct = pct_or_zero.amount();
            // Use full schedule for accurate outstanding principal when available;
            // otherwise fall back to original notional (valid for bullet bonds).
            let outstanding = if let Some(sched) = schedule {
                outstanding_principal_at_date(sched, exercise_date)
            } else {
                bond.notional.amount()
            };
            let accrued = if let Some(sched) = schedule {
                crate::cashflow::accrual::accrued_interest_amount(
                    sched,
                    exercise_date,
                    &bond.accrual_config(),
                )?
            } else {
                0.0
            };
            Money::new(
                outstanding * (pct / 100.0) + accrued,
                bond.notional.currency(),
            )
        } else {
            Money::new(0.0, bond.notional.currency())
        };
        ex_flows.push((exercise_date, redemption));

        // Solve yield that matches target dirty price
        let coupon_rate = match &bond.cashflow_spec {
            crate::instruments::fixed_income::bond::CashflowSpec::Fixed(spec) => {
                spec.rate.to_f64().unwrap_or(0.0)
            }
            _ => 0.0,
        };
        let y = crate::instruments::fixed_income::bond::pricing::ytm_solver::solve_ytm(
            &ex_flows,
            as_of,
            dirty_price_target,
            crate::instruments::fixed_income::bond::pricing::ytm_solver::YtmPricingSpec {
                day_count: bond.cashflow_spec.day_count(),
                notional: bond.notional,
                coupon_rate,
                compounding: YieldCompounding::Street,
                frequency: bond.cashflow_spec.frequency(),
            },
        )?;
        if y < best_yield {
            best_yield = y;
            best_flows = ex_flows;
        }
    }

    Ok((best_yield, best_flows))
}

/// Price from Yield-To-Worst by scanning call/put candidates and selecting the lowest yield path.
pub fn price_from_ytw(
    bond: &Bond,
    curves: &MarketContext,
    as_of: Date,
    dirty_price_target: Money,
) -> finstack_quant_core::Result<f64> {
    // Build signed canonical schedule flows and full schedule for accurate amortizing bond handling
    let flows = bond.pricing_dated_cashflows(curves, as_of)?;
    let schedule = bond.full_cashflow_schedule(curves)?;
    let (best_yield, best_flows) =
        solve_ytw_from_flows(bond, &flows, as_of, dirty_price_target, Some(&schedule))?;

    // Re-price along the worst-yield path for a consistent price result
    let best_price = price_from_ytm_compounded(
        bond,
        &best_flows,
        as_of,
        best_yield,
        YieldCompounding::Street,
    )?;

    Ok(best_price)
}

/// Price from Z-spread added to zero rates in the bond's compounding convention.
///
/// # Settlement origin
///
/// `as_of` is the **valuation/trade date**. The Z-spread is, by market
/// convention, a settlement-anchored quantity: [`ZSpreadCalculator`] solves it
/// with discounting and year-fractions measured from the bond's settlement
/// (`quote_date`), not from `as_of`. This inverter mirrors that exactly — it
/// derives the same `quote_date` internally via `QuoteDateContext` and
/// anchors all discounting there. As a result the documented round-trip
///
/// ```text
/// price_from_z_spread(bond, market, as_of, ZSpreadCalculator.solve(...)) == dirty
/// ```
///
/// holds for **any** bond, including ones with a non-zero `settlement_days`
/// lag (`quote_date != as_of`). Callers must pass the valuation date as
/// `as_of`; the settlement offset is handled here.
///
/// [`ZSpreadCalculator`]: crate::instruments::fixed_income::bond::ZSpreadCalculator
pub fn price_from_z_spread(
    bond: &Bond,
    curves: &MarketContext,
    as_of: Date,
    z: f64,
) -> finstack_quant_core::Result<f64> {
    use finstack_quant_core::math::summation::NeumaierAccumulator;

    // Cashflows are generated at the valuation date — identical to the
    // ZSpreadCalculator, which builds them via `pricing_dated_cashflows(as_of)`.
    let flows = bond.pricing_dated_cashflows(curves, as_of)?;
    let disc = curves.get_discount(&bond.discount_curve_id)?;
    let compounds_per_year = bond_z_spread_compounding_frequency(bond);

    // Settlement-anchored time origin: the same `quote_date` the Z-spread was
    // calibrated on. When `settlement_days == 0` this equals `as_of`.
    let quote_date = QuoteDateContext::new(bond, curves, as_of)?.quote_date;

    let mut pv = NeumaierAccumulator::new();
    for (d, a) in &flows {
        // Mirror the ZSpreadCalculator convention: strictly-future cashflows
        // relative to the settlement date. A flow dated exactly on `quote_date`
        // has t = 0 and is excluded by the solver's `d > quote_date` filter, so
        // it is excluded here too — keeping both paths on the same cashflow set.
        if *d <= quote_date {
            continue;
        }
        // Time and base discount factor are both measured from `quote_date`
        // (the settlement origin the Z-spread was solved on), so the
        // periodically-compounded z-spread term (see `z_spread_discount_factor`)
        // is applied on the same axis. The spread shifts the compounded zero
        // rate at frequency `m`; it is not a continuous `exp(-z·t)` shift.
        let t_from_quote =
            disc.day_count()
                .year_fraction(quote_date, *d, DayCountContext::default())?;

        let df = disc.df_between_dates(quote_date, *d)?;
        // Propagate Err from `z_spread_discount_factor` so callers receive a
        // clear curve-data or spread-domain error rather than Ok(INFINITY) or
        // Ok(NaN) when the base DF or compounding denominator is non-positive.
        let df_z = z_spread_discount_factor(df, t_from_quote, z, compounds_per_year)?;
        pv.add(a.amount() * df_z);
    }
    Ok(pv.total())
}

/// Price from Option-Adjusted Spread using the short-rate tree pricer.
///
/// The public API takes **decimal spread units** (`oas_decimal`), where
/// `0.01` corresponds to **100 basis points**. Internally, the tree
/// pricer continues to work in basis points for compatibility, so we
/// convert:
///
/// - `oas_bp = oas_decimal * 10_000.0`
///
/// This keeps all bond spread-style metrics on a consistent decimal
/// convention at the API surface while preserving existing internal
/// tree semantics.
pub fn price_from_oas(
    bond: &Bond,
    curves: &MarketContext,
    as_of: Date,
    oas_decimal: f64,
) -> finstack_quant_core::Result<f64> {
    // Convert decimal spread (0.01 = 100bp) to basis points for the tree.
    let oas_bp = oas_decimal * 10_000.0;
    let pricer =
        crate::instruments::fixed_income::bond::pricing::engine::tree::TreePricer::with_config(
            crate::instruments::fixed_income::bond::pricing::engine::tree::bond_tree_config(bond)?,
        );
    pricer.price_at_oas(bond, curves, as_of, oas_bp)
}

/// Price from Discount Margin for FRNs by adding DM (decimal) to float margin and delegating to pricer.
///
/// This helper prices against the model PV, independent of any price-from-quote
/// override on the bond. It is used by the DM metric solver that seeks a DM
/// reproducing a quoted price, so it must not short-circuit via the quote.
pub fn price_from_dm(
    bond: &Bond,
    curves: &MarketContext,
    as_of: Date,
    dm: f64,
) -> finstack_quant_core::Result<f64> {
    let mut b = bond.clone();
    clear_price_driving_overrides(&mut b);

    // Check if it's a floating rate bond
    let is_floating = matches!(
        &b.cashflow_spec,
        crate::instruments::fixed_income::bond::CashflowSpec::Floating(_)
    );
    if !is_floating {
        return Ok(b.value(curves, as_of)?.amount());
    }
    if let crate::instruments::fixed_income::bond::CashflowSpec::Floating(spec) =
        &mut b.cashflow_spec
    {
        // Convert dm (in decimal) to basis points and add to spread_bp (Decimal)
        let dm_bp = finstack_quant_core::decimal::f64_to_decimal(dm * 1e4)?;
        spec.rate_spec.spread_bp += dm_bp;
    }
    Ok(b.value(curves, as_of)?.amount())
}

/// Clear all price-driving market-quote overrides on a bond so downstream
/// pricing calls evaluate the model PV. Used by inversion helpers that need
/// the raw model response even when the bond carries a quoted price override.
pub(crate) fn clear_price_driving_overrides(bond: &mut Bond) {
    let quotes = &mut bond.pricing_overrides.market_quotes;
    quotes.quoted_clean_price = None;
    quotes.quoted_dirty_price_ccy = None;
    quotes.quoted_ytm = None;
    quotes.quoted_ytw = None;
    quotes.quoted_z_spread = None;
    quotes.quoted_oas = None;
    quotes.quoted_discount_margin = None;
    quotes.quoted_i_spread = None;
    quotes.quoted_asw_market = None;
}

// ============================================================================
// Main Quote Engine
// ============================================================================

/// Convert between price, yield, and spread metrics for a bond.
///
/// The engine:
/// - Normalizes the chosen `quote_input` into a **canonical dirty price in currency**.
/// - Derives the corresponding clean price (% of par) and stamps it into
///   `pricing_overrides.quoted_clean_price` on an internal bond clone.
/// - Uses the standard metrics registry to compute the remaining metrics.
///
/// # Arguments
///
/// * `bond` - The bond to compute quotes for
/// * `curves` - Market context with discount and forward curves
/// * `as_of` - Valuation date
/// * `quote_input` - One quote input (price, yield, or spread) to normalize from
///
/// # Returns
///
/// A `BondQuoteSet` containing all computed price, yield, and spread metrics.
///
/// # Errors
///
/// Returns `Err` when:
/// - Market curves are missing
/// - Cashflow schedule building fails
/// - Metric calculations fail
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
/// use finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::{compute_quotes, BondQuoteInput};
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_core::dates::Date;
///
/// # let bond = Bond::example().unwrap();
/// # let curves = MarketContext::new();
/// # let as_of = Date::from_calendar_date(2024, time::Month::January, 15).unwrap();
/// let quotes = compute_quotes(&bond, &curves, as_of, BondQuoteInput::CleanPricePct(98.5))?;
/// // quotes contains YTM, Z-spread, OAS, etc.
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn compute_quotes(
    bond: &Bond,
    curves: &MarketContext,
    as_of: Date,
    quote_input: BondQuoteInput,
) -> Result<BondQuoteSet> {
    // Work on a local clone so we never mutate the caller's bond instance.
    let mut bond_for_metrics = bond.clone();

    // Quote normalization (clean/dirty conversion) must use accrued at quote/settlement date.
    let quote_ctx = QuoteDateContext::new(&bond_for_metrics, curves, as_of)?;
    let accrued_ccy = quote_ctx.accrued_at_quote_date;

    let notional = bond_for_metrics.notional.amount();
    if notional.abs() < ZERO_TOLERANCE {
        return Ok(BondQuoteSet {
            clean_price_ccy: 0.0,
            clean_price_pct: 0.0,
            dirty_price_ccy: 0.0,
            ytm: None,
            ytw: None,
            z_spread: None,
            discount_margin: None,
            oas: None,
            asw_par: None,
            asw_market: None,
            i_spread: None,
        });
    }

    // 1) Stamp the quote input into the corresponding price-driving override
    //    on the bond clone, then delegate to `base_value` (which runs the same
    //    precedence chain used by the pricing pipeline). This keeps
    //    `compute_quotes` and `Bond::base_value` in lock-step and eliminates
    //    the per-variant inversion logic that used to live here.
    clear_price_driving_overrides(&mut bond_for_metrics);
    {
        let quotes = &mut bond_for_metrics.pricing_overrides.market_quotes;
        match quote_input {
            BondQuoteInput::CleanPricePct(v) => quotes.quoted_clean_price = Some(v),
            BondQuoteInput::DirtyPriceCcy(v) => quotes.quoted_dirty_price_ccy = Some(v),
            BondQuoteInput::Ytm(v) => quotes.quoted_ytm = Some(v),
            BondQuoteInput::Ytw(v) => quotes.quoted_ytw = Some(v),
            BondQuoteInput::ZSpread(v) => quotes.quoted_z_spread = Some(v),
            BondQuoteInput::DiscountMargin(v) => quotes.quoted_discount_margin = Some(v),
            BondQuoteInput::Oas(v) => quotes.quoted_oas = Some(v),
            BondQuoteInput::AswMarket(v) => quotes.quoted_asw_market = Some(v),
            BondQuoteInput::ISpread(v) => quotes.quoted_i_spread = Some(v),
        }
    }

    let base_value = bond_for_metrics.value(curves, as_of)?;
    let dirty_price_ccy = base_value.amount();
    let clean_price_ccy = dirty_price_ccy - accrued_ccy;
    let clean_price_pct = clean_price_ccy / notional * 100.0;

    // Stamp the canonical clean price quote into pricing_overrides so that all
    // existing metric calculators interpret this as the market price.
    // (Replaces the specific quote field with the clean-price normalization
    // expected by the downstream metric calculators.)
    clear_price_driving_overrides(&mut bond_for_metrics);
    bond_for_metrics
        .pricing_overrides
        .market_quotes
        .quoted_clean_price = Some(clean_price_pct);

    // 2) Build metric context and use the standard registry for the rest.
    let base_value = bond_for_metrics.value(curves, as_of)?;
    let registry: MetricRegistry = standard_registry().clone();

    let instrument_arc: Arc<dyn Instrument> = Arc::new(bond_for_metrics.clone());
    let curves_arc = Arc::new(curves.clone());
    let mut ctx = MetricContext::new(
        instrument_arc,
        curves_arc,
        as_of,
        base_value,
        MetricContext::default_config(),
    );
    ctx.notional = Some(bond_for_metrics.notional);

    // Pre-populate accrued since we've already computed it.
    ctx.computed.insert(MetricId::Accrued, accrued_ccy);

    // Request the core price/yield/spread metrics.
    let metric_ids = [
        MetricId::Ytm,
        MetricId::Ytw,
        MetricId::ZSpread,
        MetricId::DiscountMargin,
        MetricId::Oas,
        MetricId::ASWPar,
        MetricId::ASWMarket,
        MetricId::ISpread,
    ];

    // Some quote metrics are not applicable to all bond types (e.g. FRN vs fixed),
    // and we want `compute_quotes` to return whatever is available rather than
    // failing the entire quote set.
    for metric_id in metric_ids.iter() {
        if let Err(err) = registry.compute(std::slice::from_ref(metric_id), &mut ctx) {
            tracing::debug!(
                metric_id = metric_id.as_str(),
                error = %err,
                "Bond quote engine metric computation failed; leaving unset"
            );
        }
    }

    // Read back the metrics we care about.
    let ytm = ctx.computed.get(&MetricId::Ytm).copied();
    let ytw = ctx.computed.get(&MetricId::Ytw).copied();
    let z_spread = ctx.computed.get(&MetricId::ZSpread).copied();
    let discount_margin = ctx.computed.get(&MetricId::DiscountMargin).copied();
    let oas = ctx.computed.get(&MetricId::Oas).copied();
    let asw_par = ctx.computed.get(&MetricId::ASWPar).copied();
    let asw_market = ctx.computed.get(&MetricId::ASWMarket).copied();
    let i_spread = ctx.computed.get(&MetricId::ISpread).copied();

    Ok(BondQuoteSet {
        clean_price_ccy,
        clean_price_pct,
        dirty_price_ccy,
        ytm,
        ytw,
        z_spread,
        discount_margin,
        oas,
        asw_par,
        asw_market,
        i_spread,
    })
}

/// Resolve any bond price-quote override into a dirty price in currency units.
///
/// Follows the precedence chain documented on [`MarketQuoteOverrides`]:
///
/// 1. `quoted_dirty_price_ccy` → return directly
/// 2. `quoted_clean_price` → convert to dirty using quote-date accrued
/// 3. `quoted_ytm` → [`price_from_ytm`]
/// 4. `quoted_ytw` → [`price_from_ytw`]
/// 5. `quoted_z_spread` → [`price_from_z_spread`]
/// 6. `quoted_oas` → [`price_from_oas`]
/// 7. `quoted_discount_margin` → [`price_from_dm`]
/// 8. `quoted_i_spread` → par-swap-rate inversion + [`price_from_ytm`]
/// 9. `quoted_asw_market` → ASW market-convention inversion
///
/// Returns `Ok(None)` when no price-driving override is set so the caller can
/// fall through to model pricing.
///
/// [`MarketQuoteOverrides`]: crate::instruments::pricing_overrides::MarketQuoteOverrides
pub(crate) fn price_from_quote_overrides(
    bond: &Bond,
    curves: &MarketContext,
    as_of: Date,
) -> Result<Option<f64>> {
    let quotes = &bond.pricing_overrides.market_quotes;

    // Fast path: no price-driving override is set.
    if quotes.quoted_dirty_price_ccy.is_none()
        && quotes.quoted_clean_price.is_none()
        && quotes.quoted_ytm.is_none()
        && quotes.quoted_ytw.is_none()
        && quotes.quoted_z_spread.is_none()
        && quotes.quoted_oas.is_none()
        && quotes.quoted_discount_margin.is_none()
        && quotes.quoted_i_spread.is_none()
        && quotes.quoted_asw_market.is_none()
    {
        return Ok(None);
    }

    // Dirty-price override: short-circuit, no accrued-interest conversion needed.
    if let Some(dirty) = quotes.quoted_dirty_price_ccy {
        return Ok(Some(dirty));
    }

    // All remaining inversions settle the quote at the bond's quote date.
    let quote_ctx = QuoteDateContext::new(bond, curves, as_of)?;
    let accrued_ccy = quote_ctx.accrued_at_quote_date;
    let notional = bond.notional.amount();

    if let Some(clean_pct) = quotes.quoted_clean_price {
        return Ok(Some(quote_ctx.dirty_from_clean_pct(clean_pct, notional)));
    }
    if let Some(ytm) = quotes.quoted_ytm {
        let flows = bond.pricing_dated_cashflows(curves, as_of)?;
        return Ok(Some(price_from_ytm(
            bond,
            &flows,
            quote_ctx.quote_date,
            ytm,
        )?));
    }
    if let Some(ytw) = quotes.quoted_ytw {
        // For non-callable bonds, YTW == YTM, and the inversion is identical.
        // For callable bonds, the quote-override path uses maturity flows
        // (consistent with `quoted_ytm`); users who need exercise-aware
        // pricing should use `quoted_oas` instead.
        let flows = bond.pricing_dated_cashflows(curves, as_of)?;
        return Ok(Some(price_from_ytm(
            bond,
            &flows,
            quote_ctx.quote_date,
            ytw,
        )?));
    }
    if let Some(z) = quotes.quoted_z_spread {
        // `price_from_z_spread` derives the settlement (`quote_date`) origin
        // internally, so it takes the valuation `as_of` here.
        return Ok(Some(price_from_z_spread(bond, curves, as_of, z)?));
    }
    if let Some(oas) = quotes.quoted_oas {
        return Ok(Some(price_from_oas(
            bond,
            curves,
            quote_ctx.quote_date,
            oas,
        )?));
    }
    if let Some(dm) = quotes.quoted_discount_margin {
        return Ok(Some(price_from_dm(bond, curves, quote_ctx.quote_date, dm)?));
    }
    if let Some(i_spread) = quotes.quoted_i_spread {
        let par_swap_rate = par_swap_rate_from_discount(bond, curves, quote_ctx.quote_date)?;
        let ytm = i_spread + par_swap_rate;
        let flows = bond.pricing_dated_cashflows(curves, as_of)?;
        return Ok(Some(price_from_ytm(
            bond,
            &flows,
            quote_ctx.quote_date,
            ytm,
        )?));
    }
    if let Some(asw) = quotes.quoted_asw_market {
        return Ok(Some(price_from_asw_market(
            bond,
            curves,
            quote_ctx.quote_date,
            asw,
        )?));
    }

    // Unreachable: the early-return above guarantees at least one override is set.
    let _ = accrued_ccy;
    Ok(None)
}

/// Compute the par swap fixed rate used in the I-Spread definition
/// (`ISpread = YTM - par_swap_rate`) using the same convention as the
/// `ISpreadCalculator` (annual Act/Act proxy fixed leg by default).
fn par_swap_rate_from_discount(
    bond: &Bond,
    curves: &MarketContext,
    quote_date: Date,
) -> Result<f64> {
    use finstack_quant_core::dates::{ScheduleBuilder, StubKind};

    let disc = curves.get_discount(&bond.discount_curve_id)?;
    if let Some(par_swap_rate) =
        crate::instruments::fixed_income::bond::metrics::price_yield_spread::i_spread::interpolated_swap_quote_rate(
            disc.as_ref(),
            quote_date,
            bond.maturity,
        )?
    {
        return Ok(par_swap_rate);
    }
    let ispread_cfg =
        crate::instruments::fixed_income::bond::metrics::price_yield_spread::i_spread::ISpreadConfig::default();

    // Mirror the fallback logic in `ISpreadCalculator`:
    // when using the default (annual Act/Act) proxy-leg config, use the bond's
    // fixed-coupon conventions for the proxy fixed leg.
    let mut fixed_leg_day_count = ispread_cfg.fixed_leg_day_count;
    let mut fixed_leg_frequency = ispread_cfg.fixed_leg_frequency;
    if matches!(
        ispread_cfg.fixed_leg_day_count,
        finstack_quant_core::dates::DayCount::ActAct
    ) && ispread_cfg.fixed_leg_frequency == finstack_quant_core::dates::Tenor::annual()
    {
        if let crate::instruments::fixed_income::bond::CashflowSpec::Fixed(spec) =
            &bond.cashflow_spec
        {
            fixed_leg_day_count = spec.schedule.dc;
            fixed_leg_frequency = spec.schedule.freq;
        }
    }

    // Mirror the schedule and fixed-leg conventions used in ISpreadCalculator defaults.
    let dates: Vec<Date> = ScheduleBuilder::new(quote_date, bond.maturity)?
        .frequency(fixed_leg_frequency)
        .stub_rule(StubKind::ShortFront)
        .build()?
        .into_iter()
        .collect();

    if dates.len() < 2 {
        return Err(finstack_quant_core::Error::Validation(
            "I-spread proxy par-swap calculation requires at least two schedule dates".to_string(),
        ));
    }

    let (par_rate, annuity) = par_rate_and_annuity_from_discount(
        disc.as_ref(),
        fixed_leg_day_count,
        Some(fixed_leg_frequency),
        &dates,
    )?;
    if annuity.abs() < 1e-12 {
        return Err(finstack_quant_core::Error::Validation(
            "I-spread proxy par-swap calculation is undefined for near-zero annuity".to_string(),
        ));
    }
    Ok(par_rate)
}

/// Price from market asset swap spread (decimal) using the same
/// approximation as `AssetSwapMarketCalculator` for non-custom,
/// fixed-rate bonds:
///
/// `ASW_mkt = (coupon - par_rate) + (1.0 - price_pct) / annuity`
///
/// where `price_pct = dirty / notional`. Inverting:
///
/// `price_pct = 1.0 - (ASW_mkt - (coupon - par_rate)) * annuity`.
fn price_from_asw_market(
    bond: &Bond,
    curves: &MarketContext,
    as_of: Date,
    asw_market: f64,
) -> Result<f64> {
    use crate::instruments::fixed_income::bond::CashflowSpec;
    use finstack_quant_core::dates::calendar::calendar_by_id;
    use finstack_quant_core::dates::ScheduleBuilder;

    // Only well-defined for fixed-rate, non-custom bonds in this helper.
    if bond.custom_cashflows.is_some() {
        return Err(finstack_quant_core::InputError::Invalid.into());
    }
    let (coupon, freq, stub, bdc, calendar_id) = match &bond.cashflow_spec {
        CashflowSpec::Fixed(spec) => (
            spec.rate.to_f64().unwrap_or(0.0),
            spec.schedule.freq,
            spec.schedule.stub,
            spec.schedule.bdc,
            Some(spec.schedule.calendar_id.as_str()),
        ),
        _ => return Err(finstack_quant_core::InputError::Invalid.into()),
    };

    let disc = curves.get_discount(&bond.discount_curve_id)?;

    // Mirror the schedule and annuity definition used by AssetSwapMarketCalculator
    // (discount-ratio approximation on the fixed-leg schedule).
    if as_of >= bond.maturity {
        return Err(finstack_quant_core::Error::Validation(
            "ASW market price inversion requires at least two fixed-leg schedule dates".to_string(),
        ));
    }
    let mut builder = ScheduleBuilder::new(as_of, bond.maturity)?
        .frequency(freq)
        .stub_rule(stub);

    if let Some(id) = calendar_id {
        if let Some(cal) = calendar_by_id(id) {
            builder = builder.adjust_with(bdc, cal);
        }
    }

    let sched: Vec<Date> = builder.build()?.into_iter().collect();
    if sched.len() < 2 {
        return Err(finstack_quant_core::Error::Validation(
            "ASW market price inversion requires at least two fixed-leg schedule dates".to_string(),
        ));
    }

    let dc = bond.cashflow_spec.day_count();
    let forward_components = if let Some(fwd_id) = resolved_asw_forward_curve_id(bond) {
        let fwd = curves.get_forward(fwd_id.as_str())?;
        Some(asset_swap_forward_components(
            disc.as_ref(),
            fwd.as_ref(),
            dc,
            Some(freq),
            &sched,
            0.0,
        )?)
    } else {
        None
    };
    let (par_rate, ann) = if let Some((float_pv, fixed_ann, float_ann)) = forward_components {
        if fixed_ann.abs() < 1e-12 {
            (0.0, 0.0)
        } else {
            (float_pv / fixed_ann, float_ann)
        }
    } else {
        par_rate_and_annuity_from_discount(disc.as_ref(), dc, Some(freq), &sched)?
    };
    if bond.notional.amount().abs() < 1e-12 {
        return Err(finstack_quant_core::Error::Validation(
            "ASW market price inversion is undefined for near-zero notional".to_string(),
        ));
    }
    // Use epsilon check to avoid unstable inversion when annuity is degenerate.
    if ann.abs() < 1e-12 {
        return Err(finstack_quant_core::Error::Validation(
            "ASW market price inversion is undefined for near-zero fixed-leg annuity".to_string(),
        ));
    }

    let price_pct = if let Some((float_pv, fixed_ann, float_ann)) = forward_components {
        1.0 + coupon * fixed_ann - float_pv - asw_market * float_ann
    } else {
        let par_asw = coupon - par_rate;
        1.0 - (asw_market - par_asw) * ann
    };
    Ok(price_pct * bond.notional.amount())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::bond::Bond;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
    use finstack_quant_core::money::Money;
    use time::macros::date;

    #[test]
    fn asset_swap_forward_paths_use_discount_factor_implied_rates() {
        let base = date!(2025 - 01 - 01);
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .knots([(0.0, 1.0), (1.0, 0.95)])
            .build()
            .expect("discount curve should build");
        let fwd = ForwardCurve::builder("USD-3M", 0.25)
            .base_date(base)
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .knots([(0.0, 0.01), (1.0, 0.21)])
            .build()
            .expect("forward curve should build");
        let schedule = [base, date!(2025 - 07 - 01), date!(2026 - 01 - 01)];
        let mut expected_float_pv = 0.0;
        let mut integrated_float_pv = 0.0;
        for dates in schedule.windows(2) {
            let t1 = fwd
                .day_count()
                .year_fraction(base, dates[0], DayCountContext::default())
                .expect("valid start time");
            let t2 = fwd
                .day_count()
                .year_fraction(base, dates[1], DayCountContext::default())
                .expect("valid end time");
            let yf = fwd
                .day_count()
                .year_fraction(dates[0], dates[1], DayCountContext::default())
                .expect("valid accrual fraction");
            let df = disc
                .df_on_date_curve(dates[1])
                .expect("valid discount factor");
            expected_float_pv += fwd.rate_between(t1, t2).expect("valid term forward") * yf * df;
            integrated_float_pv += fwd.rate_period(t1, t2) * yf * df;
        }

        let (float_pv, fixed_ann, _) = asset_swap_forward_components(
            &disc,
            &fwd,
            finstack_quant_core::dates::DayCount::Act360,
            None,
            &schedule,
            0.0,
        )
        .expect("asset-swap components should succeed");
        let (par_rate, par_ann) = par_rate_and_annuity_from_forward(
            &disc,
            &fwd,
            finstack_quant_core::dates::DayCount::Act360,
            None,
            &schedule,
            0.0,
        )
        .expect("forward par rate should succeed");

        assert!((expected_float_pv - integrated_float_pv).abs() > 1e-6);
        assert!((float_pv - expected_float_pv).abs() < 1e-14);
        assert!((par_ann - fixed_ann).abs() < 1e-14);
        assert!((par_rate - expected_float_pv / fixed_ann).abs() < 1e-14);
    }

    #[test]
    fn overnight_asset_swap_forward_paths_use_observation_average() {
        let base = date!(2025 - 01 - 01);
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .knots([(0.0, 1.0), (1.0, 0.95)])
            .build()
            .expect("discount curve should build");
        let fwd = ForwardCurve::builder("USD-SOFR", 1.0 / 360.0)
            .base_date(base)
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .knots([(0.0, 0.01), (1.0, 0.21)])
            .build()
            .expect("forward curve should build");
        let schedule = [base, date!(2026 - 01 - 01)];
        let t2 = fwd
            .day_count()
            .year_fraction(base, schedule[1], DayCountContext::default())
            .expect("valid end time");
        let yf = t2;
        let df = disc
            .df_on_date_curve(schedule[1])
            .expect("valid discount factor");
        let expected_float_pv = fwd.rate_period(0.0, t2) * yf * df;
        let term_float_pv = fwd.rate_between(0.0, t2).expect("valid term forward") * yf * df;

        let (float_pv, _, _) = asset_swap_forward_components(
            &disc,
            &fwd,
            finstack_quant_core::dates::DayCount::Act360,
            None,
            &schedule,
            0.0,
        )
        .expect("asset-swap components should succeed");

        assert!((expected_float_pv - term_float_pv).abs() > 1e-6);
        assert!((float_pv - expected_float_pv).abs() < 1e-14);
    }

    /// W-30: a Treasury with a LONG first coupon period (8 months on a
    /// semi-annual bond) must have its first-period stub flagged from the
    /// cashflow schedule. The price returned by `price_from_ytm_compounded_params`
    /// for `TreasuryActual` must apply simple interest over the actual 8-month
    /// first period — not the time-based `t <= 1/m` heuristic which would
    /// split it into a regular 6-month period plus a 2-month stub.
    #[test]
    fn treasury_actual_long_first_coupon_uses_schedule_stub() {
        use finstack_quant_core::dates::{DayCount, Tenor};

        let as_of = date!(2025 - 01 - 01);
        let day_count = DayCount::Act365F;
        let freq = Tenor::semi_annual();
        let ytm = 0.05;

        // Long first coupon: ~8 months to first flow, then semi-annual.
        let flows: Vec<(finstack_quant_core::dates::Date, Money)> = vec![
            (date!(2025 - 09 - 01), Money::new(3.0, Currency::USD)),
            (date!(2026 - 03 - 01), Money::new(3.0, Currency::USD)),
            (date!(2026 - 09 - 01), Money::new(103.0, Currency::USD)),
        ];

        let price_actual = price_from_ytm_compounded_params(
            day_count,
            freq,
            &flows,
            as_of,
            ytm,
            YieldCompounding::TreasuryActual,
        )
        .expect("treasury-actual price");

        // First-period length flagged from the schedule (yf to first flow).
        let first_period_len = day_count
            .year_fraction(as_of, flows[0].0, DayCountContext::default())
            .expect("first period yf");
        assert!(
            first_period_len > 0.5,
            "test precondition: first coupon must be a LONG stub (>1 regular period), got {first_period_len}"
        );
        let m = periods_per_year(freq).expect("m").max(1.0);

        // Schedule-aware reference: simple interest over the actual first period.
        let mut expected_schedule = 0.0;
        // Buggy time-based reference: the old `df_from_yield` heuristic.
        let mut buggy_time_based = 0.0;
        for &(date, amount) in &flows {
            let t = day_count
                .year_fraction(as_of, date, DayCountContext::default())
                .expect("yf");
            expected_schedule += amount.amount()
                * df_treasury_actual_with_first_period(ytm, t, m, first_period_len)
                    .expect("schedule df");
            buggy_time_based += amount.amount()
                * df_from_yield(ytm, t, YieldCompounding::TreasuryActual, freq)
                    .expect("time-based df");
        }

        assert!(
            (price_actual - expected_schedule).abs() < 1e-9,
            "price {price_actual} must use the schedule-flagged stub {expected_schedule}"
        );
        assert!(
            (price_actual - buggy_time_based).abs() > 1e-4,
            "price {price_actual} must NOT match the time-based heuristic {buggy_time_based}"
        );
    }

    #[test]
    fn compute_quotes_returns_zeroes_for_effectively_zero_notional() {
        let as_of = date!(2025 - 01 - 01);
        let bond = Bond::fixed(
            "QE-NEAR-ZERO-NOTIONAL",
            Money::new(1e-12, Currency::USD),
            0.05,
            as_of,
            date!(2030 - 01 - 01),
            "USD-OIS",
        )
        .expect("bond");
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (5.0, 0.8)])
            .build()
            .expect("curve");

        let quotes = compute_quotes(
            &bond,
            &MarketContext::new().insert(curve),
            as_of,
            BondQuoteInput::CleanPricePct(99.0),
        )
        .expect("quote conversion");

        assert_eq!(quotes.clean_price_ccy, 0.0);
        assert_eq!(quotes.clean_price_pct, 0.0);
        assert_eq!(quotes.dirty_price_ccy, 0.0);
        assert!(quotes.ytm.is_none());
    }
}
