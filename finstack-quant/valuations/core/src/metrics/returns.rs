//! Annual-effective return quotes on an explicit basis (records
//! CORE-CONVENTIONS-017 #12 and resolution O11).
//!
//! Core [`xirr`] returns one canonical rate: effective annual growth on a
//! fixed ACT/365F clock. A product that quotes its return on a different
//! annual basis selects a [`YieldBasis`]; the quote is the mathematically
//! equivalent growth conversion, not a re-solve on another clock. Only bases
//! whose year fraction is calendar days over a constant admit an exact
//! conversion, so the basis is a closed set.

use finstack_quant_core::cashflow::xirr;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::{Error, Result};

/// Annual basis of an effective-rate quote.
///
/// A rate `q` on a basis with `B` days per year grows one unit to
/// `(1 + q)^(days / B)` over `days` calendar days. ACT/365F and ACT/360 are
/// the supported bases because each is exactly days over a constant, which
/// makes the conversion from the canonical ACT/365F rate exact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum YieldBasis {
    /// Actual days over a 365-day year (the canonical XIRR clock).
    #[serde(rename = "act_365f")]
    Act365F,
    /// Actual days over a 360-day year.
    #[serde(rename = "act_360")]
    Act360,
}

impl YieldBasis {
    /// Days in this basis's year.
    pub const fn days_per_year(self) -> f64 {
        match self {
            YieldBasis::Act365F => 365.0,
            YieldBasis::Act360 => 360.0,
        }
    }

    /// The day count whose year fraction is this basis's clock.
    pub const fn day_count(self) -> DayCount {
        match self {
            YieldBasis::Act365F => DayCount::Act365F,
            YieldBasis::Act360 => DayCount::Act360,
        }
    }

    /// Year fraction on this basis: actual calendar days over its year.
    ///
    /// # Arguments
    ///
    /// * `start` - Period start date.
    /// * `end` - Period end date; earlier than `start` gives a negative fraction.
    pub fn year_fraction(self, start: Date, end: Date) -> f64 {
        (end - start).whole_days() as f64 / self.days_per_year()
    }

    /// Convert the canonical ACT/365F effective annual rate to this basis.
    ///
    /// Equal growth over any day count requires
    /// `log1p(q) = log1p(rate) * days_per_year / 365`.
    ///
    /// # Arguments
    ///
    /// * `rate` - Effective annual decimal rate on ACT/365F, greater than -1.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when `rate` is not finite and above -1, or
    /// the converted rate is not finite and above -1.
    pub fn from_act365f(self, rate: f64) -> Result<f64> {
        if !rate.is_finite() || rate <= -1.0 {
            return Err(Error::Validation(format!(
                "return rate must be finite and greater than -1, got {rate}"
            )));
        }
        let quoted = match self {
            YieldBasis::Act365F => rate,
            YieldBasis::Act360 => (rate.ln_1p() * (360.0 / 365.0)).exp_m1(),
        };
        if !quoted.is_finite() || quoted <= -1.0 {
            return Err(Error::Validation(format!(
                "return rate {rate} is not representable on {self}"
            )));
        }
        Ok(quoted)
    }
}

impl std::fmt::Display for YieldBasis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            YieldBasis::Act365F => "act_365f",
            YieldBasis::Act360 => "act_360",
        })
    }
}

/// The supported basis for a configured day count.
///
/// # Errors
///
/// Returns `Error::Validation` naming the convention when it is not ACT/365F
/// or ACT/360: no exact growth conversion exists for it, and it is never
/// silently replaced.
impl TryFrom<DayCount> for YieldBasis {
    type Error = Error;

    fn try_from(day_count: DayCount) -> Result<Self> {
        match day_count {
            DayCount::Act365F => Ok(YieldBasis::Act365F),
            DayCount::Act360 => Ok(YieldBasis::Act360),
            other => Err(Error::Validation(format!(
                "annual-effective return basis must be act_365f or act_360, got {other}"
            ))),
        }
    }
}

/// XIRR quoted on `basis`: the canonical ACT/365F XIRR converted by equal
/// growth.
///
/// # Arguments
///
/// * `flows` - `(date, amount)` signed cash in one currency, as for core
///   [`xirr`].
/// * `basis` - Annual basis of the returned quote.
///
/// # Errors
///
/// Returns core [`xirr`]'s errors, or the conversion error of
/// [`YieldBasis::from_act365f`].
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::create_date;
/// use finstack_quant_valuations_core::metrics::returns::{xirr_on_basis, YieldBasis};
/// use time::Month;
///
/// let flows = [
///     (create_date(2025, Month::January, 1)?, -100.0),
///     (create_date(2026, Month::January, 1)?, 110.0),
/// ];
/// let act360 = xirr_on_basis(&flows, YieldBasis::Act360)?;
/// // Same growth over 365 days: (1 + q)^(365/360) = 1.1.
/// assert!(((1.0 + act360).powf(365.0 / 360.0) - 1.1).abs() < 1e-12);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn xirr_on_basis(flows: &[(Date, f64)], basis: YieldBasis) -> Result<f64> {
    basis.from_act365f(xirr(flows)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::create_date;
    use time::Month;

    #[test]
    fn act360_quote_preserves_growth_over_any_period() {
        let start = create_date(2024, Month::January, 1).expect("date");
        let end = create_date(2024, Month::July, 1).expect("date");
        let flows = [(start, -100_000.0), (end, 102_500.0)];
        let q = xirr_on_basis(&flows, YieldBasis::Act360).expect("ACT/360 quote");
        let growth = (1.0 + q).powf(YieldBasis::Act360.year_fraction(start, end));
        assert!((growth - 1.025).abs() < 1e-12);
        let canonical = xirr_on_basis(&flows, YieldBasis::Act365F).expect("ACT/365F");
        assert_eq!(canonical, xirr(&flows).expect("core"));
        assert!(q < canonical);
    }

    #[test]
    fn unsupported_day_counts_are_named_errors() {
        assert_eq!(
            YieldBasis::try_from(DayCount::Act360).ok(),
            Some(YieldBasis::Act360)
        );
        let err = YieldBasis::try_from(DayCount::Thirty360).expect_err("no exact conversion");
        assert!(err.to_string().contains("30"), "{err}");
        assert!(YieldBasis::Act360.from_act365f(f64::NAN).is_err());
        assert!(YieldBasis::Act360.from_act365f(-1.0).is_err());
    }

    #[test]
    fn wire_labels_match_day_count_labels() {
        for basis in [YieldBasis::Act365F, YieldBasis::Act360] {
            let json = serde_json::to_string(&basis).expect("serialize");
            assert_eq!(
                json,
                serde_json::to_string(&basis.day_count()).expect("day count")
            );
            assert_eq!(json.trim_matches('"'), basis.to_string());
        }
    }
}
