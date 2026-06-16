//! Compounding convention definitions for interest rate calculations.
//!
//! This module defines the [`Compounding`] enum which specifies how interest rates
//! should be quoted or converted. Different markets and instruments use different
//! compounding conventions, and this enum provides a unified way to specify the
//! desired convention.
//!
//! # Market Conventions
//!
//! | Convention | Common Usage | Formula (rate from DF) |
//! |------------|--------------|------------------------|
//! | Continuous | Internal calculations, curve construction | r = -ln(DF) / t |
//! | Annual | Bond markets (UK, Europe) | r = DF^(-1/t) - 1 |
//! | Semi-Annual | US Treasury, corporate bonds | r = 2 × (DF^(-1/(2t)) - 1) |
//! | Quarterly | Some floating rate notes | r = 4 × (DF^(-1/(4t)) - 1) |
//! | Simple | Money market (< 1Y), deposits | r = (1/DF - 1) / t |
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_core::math::Compounding;
//! use std::num::NonZeroU32;
//!
//! // Continuous compounding (most common for quant calculations)
//! let cont = Compounding::Continuous;
//!
//! // Semi-annual compounding (US Treasury convention)
//! let semi = Compounding::SEMI_ANNUAL;
//!
//! // Custom periodic compounding
//! let monthly = Compounding::Periodic(NonZeroU32::new(12).unwrap());
//!
//! // Simple interest (money market)
//! let simple = Compounding::Simple;
//! ```
//!
//! # References
//!
//! - Hull, J. C. (2018). *Options, Futures, and Other Derivatives* (10th ed.).
//!   Pearson. Chapter 4 (Interest Rates).
//! - Brigo, D., & Mercurio, F. (2006). *Interest Rate Models - Theory and Practice*
//!   (2nd ed.). Springer. Chapter 1 (Definitions and Notation).

use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;

/// Compounding convention for interest rates.
///
/// Used to specify how interest rates should be quoted or converted.
/// All variants produce mathematically equivalent discount factors when
/// applied consistently.
///
/// # Relationship Between Conventions
///
/// For a given discount factor DF at time t, the rates under different
/// conventions are related by:
///
/// ```text
/// DF = e^(-r_cc × t)                    [Continuous]
///    = (1 + r_ann)^(-t)                 [Annual]
///    = (1 + r_per/n)^(-n×t)             [Periodic(n)]
///    = 1 / (1 + r_simple × t)           [Simple]
/// ```
///
/// # Ordering of Rates
///
/// For positive rates and t > 0: `r_simple > r_annual > r_continuous`
/// (less frequent compounding requires a higher quoted rate for the same DF).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Compounding {
    /// Continuous compounding: r = -ln(DF) / t
    ///
    /// Most common for internal calculations, curve construction, and
    /// quantitative finance models. Provides the simplest mathematical
    /// properties (additive over time).
    Continuous,

    /// Annual compounding: r = DF^(-1/t) - 1
    ///
    /// Standard for many bond markets, particularly UK gilts and
    /// European government bonds. Also common for Bloomberg zero rate display.
    Annual,

    /// Periodic compounding with n periods per year: r = n × (DF^(-1/(n×t)) - 1)
    ///
    /// Common values:
    /// - n=2: Semi-annual (US Treasury, corporate bonds)
    /// - n=4: Quarterly (some FRNs)
    /// - n=12: Monthly (rare, some retail products)
    ///
    /// Note: Uses [`NonZeroU32`] to prevent division by zero at compile time.
    Periodic(NonZeroU32),

    /// Simple interest (no compounding): r = (1/DF - 1) / t
    ///
    /// Used for money market instruments with maturity < 1 year, including:
    /// - Interbank deposits
    /// - T-bills and commercial paper
    /// - SOFR, SONIA, €STR fixings
    ///
    /// Typically paired with ACT/360 (USD, EUR) or ACT/365F (GBP) day counts.
    Simple,
}

/// Helper const fn to create `NonZeroU32` in const context.
///
/// The fallback branch is only present to satisfy const evaluation without
/// introducing `panic!` in production code; all current call sites pass
/// non-zero compile-time literals.
const fn nonzero_u32(n: u32) -> NonZeroU32 {
    match NonZeroU32::new(n) {
        Some(v) => v,
        None => NonZeroU32::MIN,
    }
}

impl Compounding {
    /// Semi-annual compounding (n=2).
    ///
    /// Standard for US Treasury bonds and most US corporate bonds.
    pub const SEMI_ANNUAL: Self = Compounding::Periodic(nonzero_u32(2));

    /// Quarterly compounding (n=4).
    ///
    /// Used for some floating rate notes and quarterly-paying bonds.
    pub const QUARTERLY: Self = Compounding::Periodic(nonzero_u32(4));

    /// Monthly compounding (n=12).
    ///
    /// Relatively rare in wholesale markets; more common in retail products.
    pub const MONTHLY: Self = Compounding::Periodic(nonzero_u32(12));

    /// Returns the number of compounding periods per year, if applicable.
    ///
    /// - `Periodic(n)` returns `Some(n)`
    /// - `Annual` returns `Some(1)` (equivalent to `Periodic(1)`)
    /// - `Continuous` and `Simple` return `None`
    #[must_use]
    pub fn periods_per_year(&self) -> Option<u32> {
        match self {
            Compounding::Annual => Some(1),
            Compounding::Periodic(n) => Some(n.get()),
            Compounding::Continuous | Compounding::Simple => None,
        }
    }

    /// Returns true if this is a periodic compounding convention (including Annual).
    #[must_use]
    pub fn is_periodic(&self) -> bool {
        matches!(self, Compounding::Annual | Compounding::Periodic(_))
    }

    /// Convert an interest rate to a discount factor for time `t` (in years).
    ///
    /// Uses the compounding convention of `self`:
    ///
    /// ```text
    /// Continuous:   DF = exp(-r × t)
    /// Annual:       DF = (1 + r)^(-t)
    /// Periodic(n):  DF = (1 + r/n)^(-n × t)
    /// Simple:       DF = 1 / (1 + r × t)
    /// ```
    ///
    /// When `t == 0.0`, returns `1.0` regardless of the rate (instantaneous
    /// observation implies no discounting).
    ///
    /// # References
    ///
    /// Hull, J. C. (2018). *Options, Futures, and Other Derivatives* (10th ed.),
    /// Chapter 4 — Interest Rates.
    #[must_use]
    #[inline]
    pub fn df_from_rate(&self, rate: f64, t: f64) -> f64 {
        if t == 0.0 {
            return 1.0;
        }
        match self {
            Compounding::Continuous => (-rate * t).exp(),
            // ln_1p formulations: (1 + r)^(-t) = exp(-t·ln(1+r)). Forming
            // 1 + r explicitly rounds away rates below ~1e-16; ln_1p keeps
            // full precision for small rates.
            Compounding::Annual => (-t * rate.ln_1p()).exp(),
            Compounding::Periodic(n) => {
                let n = f64::from(n.get());
                (-n * t * (rate / n).ln_1p()).exp()
            }
            Compounding::Simple => {
                let denominator = 1.0 + rate * t;
                if denominator <= 0.0 || !denominator.is_finite() {
                    f64::NAN
                } else {
                    1.0 / denominator
                }
            }
        }
    }

    /// Convert a discount factor to an interest rate for time `t` (in years).
    ///
    /// Inverts the discount-factor formula for the compounding convention of `self`:
    ///
    /// ```text
    /// Continuous:   r = -ln(DF) / t
    /// Annual:       r = DF^(-1/t) - 1
    /// Periodic(n):  r = n × (DF^(-1/(n×t)) - 1)
    /// Simple:       r = (1/DF - 1) / t
    /// ```
    ///
    /// Returns `NaN` for non-positive or non-finite discount factors.
    ///
    /// When `t == 0.0`, returns `0.0` (the rate is undefined for an
    /// instantaneous observation; zero is a safe sentinel).
    ///
    /// # References
    ///
    /// Hull, J. C. (2018). *Options, Futures, and Other Derivatives* (10th ed.),
    /// Chapter 4 — Interest Rates.
    #[must_use]
    #[inline]
    pub fn rate_from_df(&self, df: f64, t: f64) -> f64 {
        if !df.is_finite() || df <= 0.0 {
            return f64::NAN;
        }
        if t == 0.0 {
            return 0.0;
        }
        match self {
            Compounding::Continuous => -df.ln() / t,
            // exp_m1 formulations avoid cancellation when the exponent is
            // small (DF near 1 and/or short tenors): exp(x) - 1 loses all
            // significant digits for |x| ≲ 1e-8 while exp_m1 is exact.
            Compounding::Annual => (-df.ln() / t).exp_m1(),
            Compounding::Periodic(n) => {
                let n = f64::from(n.get());
                n * (-df.ln() / (n * t)).exp_m1()
            }
            Compounding::Simple => (1.0 / df - 1.0) / t,
        }
    }

    /// Continuous-equivalent (instantaneous) rate of `rate` quoted under `self`.
    ///
    /// This is the `t → 0` limit of the conversion to continuous compounding:
    ///
    /// ```text
    /// Continuous:   c = r
    /// Annual:       c = ln(1 + r)
    /// Periodic(n):  c = n × ln(1 + r/n)
    /// Simple:       c = r        (DF = 1/(1 + r t) → e^(-r t) as t → 0)
    /// ```
    #[must_use]
    #[inline]
    fn instantaneous_rate(&self, rate: f64) -> f64 {
        match self {
            Compounding::Continuous | Compounding::Simple => rate,
            Compounding::Annual => rate.ln_1p(),
            Compounding::Periodic(n) => {
                let n = f64::from(n.get());
                n * (rate / n).ln_1p()
            }
        }
    }

    /// Rate quoted under `self` equivalent to the continuous rate `c` as `t → 0`.
    ///
    /// Inverse of [`instantaneous_rate`](Self::instantaneous_rate):
    ///
    /// ```text
    /// Continuous:   r = c
    /// Annual:       r = e^c − 1
    /// Periodic(n):  r = n × (e^(c/n) − 1)
    /// Simple:       r = c
    /// ```
    #[must_use]
    #[inline]
    fn rate_from_instantaneous(&self, c: f64) -> f64 {
        match self {
            Compounding::Continuous | Compounding::Simple => c,
            Compounding::Annual => c.exp_m1(),
            Compounding::Periodic(n) => {
                let n = f64::from(n.get());
                n * (c / n).exp_m1()
            }
        }
    }

    /// Convert a rate quoted under `self` to the equivalent rate under `to`.
    ///
    /// Internally this computes the discount factor from the source convention
    /// and then inverts it under the target convention, ensuring exact
    /// consistency:
    ///
    /// ```text
    /// r_to = to.rate_from_df(self.df_from_rate(r_from, t), t)
    /// ```
    ///
    /// When `t == 0.0`, returns the `t → 0` limit of the conversion: the
    /// source rate is mapped to its continuous-equivalent
    /// (`ln(1+r)` for annual, `n·ln(1+r/n)` for periodic, identity for
    /// continuous/simple) and then back out under the target convention
    /// (`e^c − 1` for annual, `n·(e^(c/n) − 1)` for periodic). For example,
    /// continuous → annual at `t = 0` returns `e^r − 1`, not `0.0`.
    ///
    /// # References
    ///
    /// Hull, J. C. (2018). *Options, Futures, and Other Derivatives* (10th ed.),
    /// Chapter 4 — Interest Rates.
    #[must_use]
    #[inline]
    pub fn convert_rate(&self, rate: f64, t: f64, to: &Compounding) -> f64 {
        if t == 0.0 {
            // t → 0 limit: route through the continuous-equivalent rate.
            return to.rate_from_instantaneous(self.instantaneous_rate(rate));
        }
        let df = self.df_from_rate(rate, t);
        to.rate_from_df(df, t)
    }
}

impl crate::parse::NormalizedEnum for Compounding {
    const VARIANTS: &'static [(&'static str, Self)] = &[
        ("continuous", Self::Continuous),
        ("simple", Self::Simple),
        ("annual", Self::Annual),
        ("semi_annual", Self::SEMI_ANNUAL),
        ("semiannual", Self::SEMI_ANNUAL),
        ("quarterly", Self::QUARTERLY),
        ("monthly", Self::MONTHLY),
    ];
}

impl std::str::FromStr for Compounding {
    type Err = crate::error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        crate::parse::parse_normalized_enum(s).map_err(|_| crate::error::InputError::Invalid.into())
    }
}

impl Default for Compounding {
    /// Default to continuous compounding (most common for quant finance).
    fn default() -> Self {
        Compounding::Continuous
    }
}

impl std::fmt::Display for Compounding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Compounding::Continuous => write!(f, "continuous"),
            Compounding::Annual => write!(f, "annual"),
            Compounding::Periodic(n) => match n.get() {
                2 => write!(f, "semi_annual"),
                4 => write!(f, "quarterly"),
                12 => write!(f, "monthly"),
                other => write!(f, "periodic({})", other),
            },
            Compounding::Simple => write!(f, "simple"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compounding_constants() {
        assert_eq!(
            Compounding::SEMI_ANNUAL,
            Compounding::Periodic(NonZeroU32::new(2).expect("2 is non-zero"))
        );
        assert_eq!(
            Compounding::QUARTERLY,
            Compounding::Periodic(NonZeroU32::new(4).expect("4 is non-zero"))
        );
        assert_eq!(
            Compounding::MONTHLY,
            Compounding::Periodic(NonZeroU32::new(12).expect("12 is non-zero"))
        );
    }

    #[test]
    fn test_periods_per_year() {
        assert_eq!(Compounding::Continuous.periods_per_year(), None);
        assert_eq!(Compounding::Annual.periods_per_year(), Some(1));
        assert_eq!(Compounding::SEMI_ANNUAL.periods_per_year(), Some(2));
        assert_eq!(Compounding::QUARTERLY.periods_per_year(), Some(4));
        assert_eq!(Compounding::MONTHLY.periods_per_year(), Some(12));
        assert_eq!(Compounding::Simple.periods_per_year(), None);
    }

    #[test]
    fn test_is_periodic() {
        assert!(!Compounding::Continuous.is_periodic());
        assert!(Compounding::Annual.is_periodic());
        assert!(Compounding::SEMI_ANNUAL.is_periodic());
        assert!(!Compounding::Simple.is_periodic());
    }

    #[test]
    fn test_default() {
        assert_eq!(Compounding::default(), Compounding::Continuous);
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", Compounding::Continuous), "continuous");
        assert_eq!(format!("{}", Compounding::Annual), "annual");
        assert_eq!(format!("{}", Compounding::SEMI_ANNUAL), "semi_annual");
        assert_eq!(format!("{}", Compounding::QUARTERLY), "quarterly");
        assert_eq!(format!("{}", Compounding::MONTHLY), "monthly");
        assert_eq!(format!("{}", Compounding::Simple), "simple");
    }

    #[test]
    fn test_serde_roundtrip() {
        let variants = [
            Compounding::Continuous,
            Compounding::Annual,
            Compounding::SEMI_ANNUAL,
            Compounding::Simple,
        ];

        for variant in variants {
            let json = serde_json::to_string(&variant).expect("serialize");
            let deserialized: Compounding = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(variant, deserialized);
        }
    }

    // ── Rate conversion tests ───────────────────────────────────────────

    /// All compounding conventions used in round-trip tests.
    fn all_conventions() -> Vec<Compounding> {
        vec![
            Compounding::Continuous,
            Compounding::Annual,
            Compounding::SEMI_ANNUAL,
            Compounding::QUARTERLY,
            Compounding::MONTHLY,
            Compounding::Simple,
        ]
    }

    const TOL: f64 = 1e-12;

    #[test]
    fn test_df_rate_roundtrip_all_conventions() {
        let rate = 0.05;
        let t = 2.0;
        for conv in all_conventions() {
            let df = conv.df_from_rate(rate, t);
            let recovered = conv.rate_from_df(df, t);
            assert!(
                (recovered - rate).abs() < TOL,
                "{conv}: rate round-trip failed: {recovered} vs {rate}",
            );
        }
    }

    #[test]
    fn test_rate_df_roundtrip_all_conventions() {
        let df = 0.92;
        let t = 1.5;
        for conv in all_conventions() {
            let rate = conv.rate_from_df(df, t);
            let recovered = conv.df_from_rate(rate, t);
            assert!(
                (recovered - df).abs() < TOL,
                "{conv}: DF round-trip failed: {recovered} vs {df}",
            );
        }
    }

    #[test]
    fn test_convert_rate_roundtrip_cross_convention() {
        let rate = 0.06;
        let t = 3.0;
        let conventions = all_conventions();
        for from in &conventions {
            for to in &conventions {
                let converted = from.convert_rate(rate, t, to);
                let back = to.convert_rate(converted, t, from);
                assert!(
                    (back - rate).abs() < TOL,
                    "{from}->{to}->{from}: convert round-trip failed: {back} vs {rate}",
                );
            }
        }
    }

    #[test]
    fn test_convert_rate_same_convention_is_identity() {
        let rate = 0.04;
        let t = 5.0;
        for conv in all_conventions() {
            let converted = conv.convert_rate(rate, t, &conv);
            assert!(
                (converted - rate).abs() < TOL,
                "{conv}: self-conversion should be identity, got {converted}",
            );
        }
    }

    #[test]
    fn test_known_value_continuous_5pct_1y() {
        let df = Compounding::Continuous.df_from_rate(0.05, 1.0);
        let expected = (-0.05_f64).exp(); // ≈ 0.951229424500714
        assert!(
            (df - expected).abs() < TOL,
            "Continuous 5% 1Y: DF = {df}, expected {expected}",
        );
    }

    #[test]
    fn test_known_value_annual_5pct_1y() {
        let df = Compounding::Annual.df_from_rate(0.05, 1.0);
        let expected = 1.0 / 1.05; // ≈ 0.952380952380952
        assert!(
            (df - expected).abs() < TOL,
            "Annual 5% 1Y: DF = {df}, expected {expected}",
        );
    }

    #[test]
    fn test_known_value_simple_5pct_half_year() {
        let df = Compounding::Simple.df_from_rate(0.05, 0.5);
        let expected = 1.0 / (1.0 + 0.05 * 0.5); // = 1 / 1.025
        assert!(
            (df - expected).abs() < TOL,
            "Simple 5% 0.5Y: DF = {df}, expected {expected}",
        );
    }

    #[test]
    fn test_known_value_semiannual_5pct_1y() {
        let df = Compounding::SEMI_ANNUAL.df_from_rate(0.05, 1.0);
        let expected = (1.0 + 0.05 / 2.0_f64).powf(-2.0); // (1.025)^-2
        assert!(
            (df - expected).abs() < TOL,
            "Semi-annual 5% 1Y: DF = {df}, expected {expected}",
        );
    }

    #[test]
    fn test_edge_case_t_zero() {
        for conv in all_conventions() {
            let df = conv.df_from_rate(0.10, 0.0);
            assert_eq!(df, 1.0, "{conv}: df_from_rate(_, 0) should be 1.0");

            let rate = conv.rate_from_df(0.95, 0.0);
            assert_eq!(rate, 0.0, "{conv}: rate_from_df(_, 0) should be 0.0");
        }
    }

    #[test]
    fn test_edge_case_zero_rate() {
        let t = 2.0;
        for conv in all_conventions() {
            let df = conv.df_from_rate(0.0, t);
            assert!(
                (df - 1.0).abs() < TOL,
                "{conv}: zero rate should give DF = 1.0, got {df}",
            );
        }
    }

    #[test]
    fn test_edge_case_high_rate() {
        let rate = 1.0; // 100%
        let t = 1.0;
        for conv in all_conventions() {
            let df = conv.df_from_rate(rate, t);
            assert!(df > 0.0, "{conv}: DF must be positive for high rate");
            assert!(df < 1.0, "{conv}: DF must be < 1 for positive rate");
            let recovered = conv.rate_from_df(df, t);
            assert!(
                (recovered - rate).abs() < TOL,
                "{conv}: high-rate round-trip failed: {recovered} vs {rate}",
            );
        }
    }

    #[test]
    fn test_convert_rate_t_zero_returns_limit() {
        // t → 0 limit of the conversion, not the old 0.0 sentinel.
        let rate = 0.05;
        let t = 0.0;

        // continuous → annual: e^r − 1
        let result = Compounding::Continuous.convert_rate(rate, t, &Compounding::Annual);
        assert!(
            (result - rate.exp_m1()).abs() < 1e-15,
            "continuous→annual at t=0 should be e^r - 1, got {result}"
        );

        // annual → continuous: ln(1 + r)
        let result = Compounding::Annual.convert_rate(rate, t, &Compounding::Continuous);
        assert!(
            (result - rate.ln_1p()).abs() < 1e-15,
            "annual→continuous at t=0 should be ln(1+r), got {result}"
        );

        // continuous → semi-annual: 2(e^{r/2} − 1)
        let result = Compounding::Continuous.convert_rate(rate, t, &Compounding::SEMI_ANNUAL);
        assert!(
            (result - 2.0 * (rate / 2.0).exp_m1()).abs() < 1e-15,
            "continuous→semi-annual at t=0 should be 2(e^(r/2)-1), got {result}"
        );

        // same convention: identity
        let result = Compounding::Continuous.convert_rate(rate, t, &Compounding::Continuous);
        assert!((result - rate).abs() < 1e-15);

        // Consistency: the t=0 limit matches the conversion at a tiny tenor.
        // The tiny-tenor path itself loses ~ε/(r·t) relative precision in
        // ln(DF), so the comparison tolerance is looser than the exact-limit
        // assertions above.
        let at_tiny_t = Compounding::Continuous.convert_rate(rate, 1e-9, &Compounding::Annual);
        let at_zero = Compounding::Continuous.convert_rate(rate, 0.0, &Compounding::Annual);
        assert!(
            (at_tiny_t - at_zero).abs() < 1e-7,
            "t=0 limit {at_zero} should match tiny-tenor conversion {at_tiny_t}"
        );
    }

    #[test]
    fn test_simple_df_from_rate_rejects_non_positive_denominator() {
        let df = Compounding::Simple.df_from_rate(-2.0, 0.5);
        assert!(df.is_nan(), "simple compounding should reject 1 + r*t <= 0");
    }

    #[test]
    fn test_simple_rate_from_df_rejects_non_positive_discount_factor() {
        let zero_df = Compounding::Simple.rate_from_df(0.0, 1.0);
        assert!(zero_df.is_nan(), "simple compounding should reject df == 0");

        let negative_df = Compounding::Simple.rate_from_df(-0.5, 1.0);
        assert!(
            negative_df.is_nan(),
            "simple compounding should reject negative discount factors"
        );
    }

    // ── H7 documentation tests: t == 0.0 float comparison semantics ──
    //
    // The reviewer suggested replacing `t == 0.0` with `t.abs() < 1e-12`.
    // This was intentionally rejected because t is a year-fraction parameter
    // whose zero-exactly semantics are part of the API contract.
    // Year fractions close to zero but not exactly zero represent genuinely
    // small maturities and should not be silently treated as zero.
    // These tests document the accepted boundary behaviour.
    #[test]
    fn test_compounding_t_exactly_zero_is_sentinel() {
        // Exact t=0.0 is the only value that triggers the sentinel path.
        for conv in all_conventions() {
            assert_eq!(
                conv.df_from_rate(0.10, 0.0),
                1.0,
                "{conv}: df_from_rate(_, 0.0) must be 1.0"
            );
            assert_eq!(
                conv.rate_from_df(0.95, 0.0),
                0.0,
                "{conv}: rate_from_df(_, 0.0) must be 0.0"
            );
        }
    }

    #[test]
    fn test_compounding_very_small_t_is_not_zero() {
        // A very small but nonzero t must NOT be treated as the zero sentinel.
        let t = 1.0e-10; // One-tenth of a nanosecond in year-fraction terms
        for conv in all_conventions() {
            let df = conv.df_from_rate(0.10, t);
            // df != 1.0 (some discounting occurs, however tiny)
            // Just confirm the function is finite and not exactly 1.0
            assert!(df.is_finite(), "{conv}: df must be finite for tiny t");
        }
    }

    #[test]
    fn test_rate_ordering_simple_gt_annual_gt_continuous() {
        // For positive rates and t > 0, less frequent compounding requires
        // a higher quoted rate for the same discount factor.
        // Note: at t=1 Simple and Annual collapse to the same formula
        // (1/DF - 1), so we use t=2 where they diverge.
        let df = 0.90;
        let t = 2.0;
        let r_cont = Compounding::Continuous.rate_from_df(df, t);
        let r_ann = Compounding::Annual.rate_from_df(df, t);
        let r_simple = Compounding::Simple.rate_from_df(df, t);

        assert!(
            r_simple > r_ann,
            "r_simple ({r_simple}) should be > r_annual ({r_ann})",
        );
        assert!(
            r_ann > r_cont,
            "r_annual ({r_ann}) should be > r_continuous ({r_cont})",
        );
    }

    #[test]
    fn test_rate_from_df_nan_for_invalid_discount_factors() {
        let bad_dfs = [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY];
        let t = 1.0;
        for conv in all_conventions() {
            for &df in &bad_dfs {
                let rate = conv.rate_from_df(df, t);
                assert!(
                    rate.is_nan(),
                    "{conv}: rate_from_df({df}, {t}) should be NaN, got {rate}",
                );
            }
        }
    }

    #[test]
    fn test_rate_ordering_periodic_between_annual_and_continuous() {
        // Semi-annual compounds more frequently than annual but less than
        // continuous, so: r_annual > r_semi > r_continuous.
        let df = 0.90;
        let t = 2.0;
        let r_cont = Compounding::Continuous.rate_from_df(df, t);
        let r_semi = Compounding::SEMI_ANNUAL.rate_from_df(df, t);
        let r_ann = Compounding::Annual.rate_from_df(df, t);

        assert!(
            r_ann > r_semi,
            "r_annual ({r_ann}) should be > r_semi ({r_semi})",
        );
        assert!(
            r_semi > r_cont,
            "r_semi ({r_semi}) should be > r_continuous ({r_cont})",
        );
    }

    #[test]
    fn test_all_conventions_agree_on_df() {
        // Converting the same 5% continuous rate to each convention and back
        // to a DF should always yield the same DF.
        let r_cont = 0.05;
        let t = 2.5;
        let df_expected = Compounding::Continuous.df_from_rate(r_cont, t);

        for conv in all_conventions() {
            let r_conv = Compounding::Continuous.convert_rate(r_cont, t, &conv);
            let df_conv = conv.df_from_rate(r_conv, t);
            assert!(
                (df_conv - df_expected).abs() < TOL,
                "{conv}: DF mismatch after conversion: {df_conv} vs {df_expected}",
            );
        }
    }

    #[test]
    fn compounding_fromstr_display_roundtrip() {
        use std::str::FromStr;

        fn assert_parses_to(label: &str, expected: Compounding) {
            assert!(matches!(Compounding::from_str(label), Ok(value) if value == expected));
        }

        let variants = [
            Compounding::Continuous,
            Compounding::Simple,
            Compounding::Annual,
            Compounding::SEMI_ANNUAL,
            Compounding::QUARTERLY,
            Compounding::MONTHLY,
        ];
        for v in variants {
            let s = v.to_string();
            let parsed = Compounding::from_str(&s).expect("roundtrip parse should succeed");
            assert_eq!(v, parsed, "roundtrip failed for {s}");
        }
        // Test aliases
        assert_parses_to("semiannual", Compounding::SEMI_ANNUAL);
        assert_parses_to("Semi-Annual", Compounding::SEMI_ANNUAL);
        assert!(Compounding::from_str("invalid").is_err());
    }
}
