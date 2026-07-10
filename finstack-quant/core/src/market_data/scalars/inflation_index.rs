//! Inflation index time series with lagging and seasonality support.
//!
//! Provides specialized handling for Consumer Price Index (CPI) and Retail Price
//! Index (RPI) data used in pricing inflation-linked securities. Handles publication
//! lags, seasonal adjustments, and ratio calculations required by TIPS, linkers,
//! and inflation derivatives.
//!
//! # Financial Context
//!
//! Inflation indices (CPI, RPI, HICP) are published monthly by government agencies
//! with specific conventions:
//!
//! ## Publication Lag
//!
//! - **US CPI-U**: Published ~2 weeks after month-end, 3-month lag common for TIPS
//! - **UK RPI**: Published mid-month for previous month, 3-month lag standard
//! - **Eurozone HICP**: Published end of month, 3-month lag typical
//!
//! ## Reference Index Calculation
//!
//! TIPS and inflation swaps use lagged interpolated indices:
//! ```text
//! Ref_Index(t) = CPI(t - lag)  with daily interpolation
//!
//! For 3-month lag on Jan 15, 2025:
//! Reference date = Oct 15, 2024
//! Index = interpolate between Oct and Nov published values
//! ```
//!
//! ## Seasonality
//!
//! Some indices exhibit monthly patterns (e.g., January sales, summer energy costs).
//! Optional seasonal factors can adjust historical data for forecasting.
//!
//! # Use Cases
//!
//! - **TIPS pricing**: Inflation-adjusted principal calculation
//! - **Inflation swaps**: Zero-coupon and year-on-year index ratios
//! - **Linkers (UK/Europe)**: RPI/HICP indexed bonds
//! - **CPI floors/caps**: Payoff based on index levels
//!
//! # Examples
//! ```rust
//! use finstack_quant_core::market_data::scalars::{
//!     InflationIndex, InflationInterpolation, InflationLag,
//! };
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::dates::Date;
//! use time::Month;
//!
//! let observations = vec![
//!     (Date::from_calendar_date(2023, Month::September, 30).expect("Valid date"), 296.8),
//!     (Date::from_calendar_date(2023, Month::October, 31).expect("Valid date"), 297.4),
//!     (Date::from_calendar_date(2023, Month::November, 30).expect("Valid date"), 298.1),
//!     (Date::from_calendar_date(2023, Month::December, 31).expect("Valid date"), 299.0),
//!     (Date::from_calendar_date(2024, Month::January, 31).expect("Valid date"), 300.0),
//!     (Date::from_calendar_date(2024, Month::February, 29).expect("Valid date"), 302.0),
//! ];
//! let index = InflationIndex::new("US-CPI", observations, Currency::USD)
//!     .expect("Index creation should succeed")
//!     .with_interpolation(InflationInterpolation::Linear)
//!     .with_lag(InflationLag::Months(3));
//! let settle = Date::from_calendar_date(2024, Month::June, 30).expect("Valid date");
//! let base = Date::from_calendar_date(2024, Month::March, 31).expect("Valid date");
//! let ratio = index.ratio(base, settle).expect("Ratio calculation should succeed");
//! assert!(ratio > 1.0);
//! ```

use super::primitives::{ScalarTimeSeries, SeriesInterpolation};
use crate::currency::Currency;
use crate::dates::{Date, DateExt};
use crate::{Error, Result};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Interpolation method for CPI/RPI values between monthly observations.
///
/// Determines how index values are computed for dates between published
/// monthly levels. Different markets use different conventions.
///
/// # Market Conventions
///
/// - **US TIPS**: Linear interpolation (daily pro-rata)
/// - **UK Index-Linked Gilts**: Linear interpolation with 3-month lag
/// - **Euro inflation bonds**: Varies by issuer (typically linear)
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::market_data::scalars::InflationInterpolation;
///
/// let linear = InflationInterpolation::Linear; // TIPS standard
/// let step = InflationInterpolation::Step;     // Conservative approach
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum InflationInterpolation {
    /// Last observation carried forward until next publication.
    ///
    /// Conservative approach: assumes no intra-month inflation.
    #[default]
    Step,

    /// Linear interpolation between monthly observations.
    ///
    /// Standard for TIPS and most inflation-linked bonds.
    /// Assumes constant daily inflation rate within each month.
    Linear,
}

impl core::fmt::Display for InflationInterpolation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            InflationInterpolation::Step => write!(f, "step"),
            InflationInterpolation::Linear => write!(f, "linear"),
        }
    }
}

impl core::str::FromStr for InflationInterpolation {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "step" => Ok(InflationInterpolation::Step),
            "linear" => Ok(InflationInterpolation::Linear),
            other => Err(format!("Unknown inflation interpolation: {}", other)),
        }
    }
}

/// Publication lag for inflation index reference dates.
///
/// Inflation indices are published with a delay (typically 2-4 weeks). Securities
/// using these indices incorporate a lag to ensure the reference index is published
/// by the settlement date.
///
/// # Standard Lags by Market
///
/// - **US TIPS**: 3-month lag (reference index from 3 months prior)
/// - **UK Index-Linked Gilts**: 3-month lag (8-month for older issues)
/// - **French OATi**: 3-month lag
/// - **German index-linked**: 3-month lag
///
/// # Rationale
///
/// The lag ensures:
/// 1. Reference index is published before settlement
/// 2. Index value is known at coupon payment date
/// 3. No estimation or forecasting required for payment calculation
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::market_data::scalars::InflationLag;
///
/// let tips_lag = InflationLag::Months(3);  // US TIPS standard
/// let gilt_lag = InflationLag::Months(3);  // UK modern gilts
/// let no_lag = InflationLag::None;         // Inflation swaps (forecast-based)
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum InflationLag {
    /// Lag by specified number of months.
    ///
    /// Standard: 3 months for TIPS and most inflation-linked bonds.
    Months(u8),

    /// Lag by specified number of calendar days.
    ///
    /// Alternative specification for non-standard contracts.
    Days(u16),

    /// No lag applied.
    ///
    /// Used for inflation swaps where forecast indices are used.
    #[default]
    None,
}

/// Inflation index time series with lagging and seasonality.
///
/// Wraps historical CPI/RPI observations with market-standard conventions for
/// lag application and interpolation. Used for pricing TIPS, linkers, and
/// inflation derivatives.
///
/// # Components
///
/// - **Observations**: Historical index levels by publication date
/// - **Interpolation**: Daily interpolation between monthly observations
/// - **Lag**: Publication lag (typically 3 months for TIPS)
/// - **Seasonality**: Optional monthly adjustment factors
///
/// # Interpolation Methods
///
/// - **Step**: Last observation carried forward (conservative)
/// - **Linear**: Daily interpolation between months (TIPS standard)
///
/// # Lag Application
///
/// Reference index calculation applies lag before interpolation:
/// ```text
/// For settlement date T with 3-month lag:
/// 1. Reference date = T - 3 months
/// 2. Find bracketing CPI observations
/// 3. Interpolate linearly between them
/// ```
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::market_data::scalars::{
///     InflationIndex, InflationInterpolation, InflationLag,
/// };
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::dates::Date;
/// use time::Month;
///
/// // US CPI-U observations
/// let observations = vec![
///     (Date::from_calendar_date(2023, Month::September, 30).expect("Valid date"), 296.8),
///     (Date::from_calendar_date(2023, Month::October, 31).expect("Valid date"), 297.4),
///     (Date::from_calendar_date(2023, Month::November, 30).expect("Valid date"), 298.1),
///     (Date::from_calendar_date(2023, Month::December, 31).expect("Valid date"), 299.0),
///     (Date::from_calendar_date(2024, Month::January, 31).expect("Valid date"), 300.5),
///     (Date::from_calendar_date(2024, Month::February, 29).expect("Valid date"), 302.1),
///     (Date::from_calendar_date(2024, Month::March, 31).expect("Valid date"), 303.8),
/// ];
///
/// let index = InflationIndex::new("US-CPI-U", observations, Currency::USD)
///     .expect("Index creation should succeed")
///     .with_interpolation(InflationInterpolation::Linear)
///     .with_lag(InflationLag::Months(3)); // TIPS standard
///
/// // Calculate inflation ratio for TIPS coupon indexation
/// let base_date = Date::from_calendar_date(2024, Month::January, 15).expect("Valid date");
/// let settle_date = Date::from_calendar_date(2024, Month::June, 15).expect("Valid date");
/// let ratio = index.ratio(base_date, settle_date).expect("Ratio calculation should succeed");
/// assert!(ratio >= 1.0); // Inflation adjustment factor
/// ```
///
/// # Thread Safety
///
/// Immutable after construction; safe to share via `Arc<InflationIndex>`.
///
/// # References
///
/// - **TIPS Mechanics**:
///   - US Treasury (2024). "TIPS In Depth." treasurydirect.gov.
///   - Deacon, M., Derry, A., & Mirfendereski, D. (2004). *Inflation-Indexed Securities*
///     (2nd ed.). Wiley Finance. Chapter 2 (Index-linked bond mechanics).
///
/// - **Index Lagging**:
///   - Kerkhof, J. (2005). "Inflation Derivatives Explained." *Journal of Derivatives
///     Accounting*, 2(1), 1-19.
///   - Hurd, M., & Relleen, J. (2006). "Estimating the Inflation Risk Premium."
///     Bank of England Quarterly Bulletin, Q2 2006.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "RawInflationIndex", into = "RawInflationIndex")]
pub struct InflationIndex {
    /// Unique identifier for this index (e.g., "US-CPI-U", "UK-RPI")
    pub id: String,
    /// Underlying time series (raw, without interpolation applied)
    series: ScalarTimeSeries,
    /// Pre-built series with the active interpolation applied, used by `value_on`
    /// to avoid a clone on every call. Not part of the serialized representation
    /// (the struct uses try_from/into via RawInflationIndex).
    series_interp: ScalarTimeSeries,
    /// Interpolation method between observations
    pub interpolation: InflationInterpolation,
    /// Lag policy for index application
    pub lag: InflationLag,
    /// Currency of the index
    pub currency: Currency,
    /// Optional monthly seasonality factors (index by month-1)
    seasonality: Option<[f64; 12]>,
}

impl InflationIndex {
    /// Create an [`InflationIndexBuilder`] for constructing a new index.
    ///
    /// This is the preferred entry point, consistent with other curve builders.
    #[must_use]
    pub fn builder(id: impl Into<String>, currency: Currency) -> InflationIndexBuilder {
        InflationIndexBuilder::new(id, currency)
    }

    /// Create a new inflation index from observations.
    ///
    /// # Parameters
    /// - `id`: stable identifier, e.g. `"US-CPI-U"`
    /// - `observations`: `(Date, value)` pairs in chronological order
    /// - `currency`: reporting currency
    pub fn new(
        id: impl Into<String>,
        observations: Vec<(Date, f64)>,
        currency: Currency,
    ) -> Result<Self> {
        if observations.is_empty() {
            return Err(Error::Input(crate::error::InputError::TooFewPoints));
        }

        // Use a placeholder internal id; external id is stored separately.
        let series = ScalarTimeSeries::new("inflation-index", observations, Some(currency))?;

        // Pre-build the interpolated series with the default (Step) interpolation
        // so that value_on does not need to clone on every call.
        let interp = SeriesInterpolation::default(); // matches InflationInterpolation::default() == Step
        let series_interp = series.clone().with_interpolation(interp);

        Ok(Self {
            id: id.into(),
            series,
            series_interp,
            interpolation: InflationInterpolation::default(),
            lag: InflationLag::default(),
            currency,
            seasonality: None,
        })
    }

    /// Set the interpolation method between observations.
    pub fn with_interpolation(mut self, interpolation: InflationInterpolation) -> Self {
        self.interpolation = interpolation;
        let interp = match interpolation {
            InflationInterpolation::Step => SeriesInterpolation::Step,
            InflationInterpolation::Linear => SeriesInterpolation::Linear,
        };
        self.series_interp = self.series.clone().with_interpolation(interp);
        self
    }

    /// Set the lag policy applied before lookups.
    pub fn with_lag(mut self, lag: InflationLag) -> Self {
        self.lag = lag;
        self
    }

    /// Add seasonal adjustment factors (one per calendar month).
    pub fn with_seasonality(mut self, factors: [f64; 12]) -> Result<Self> {
        self.seasonality = Some(factors);
        Ok(self)
    }

    /// Get the index value on a given date with interpolation and adjustments.
    pub fn value_on(&self, date: Date) -> Result<f64> {
        // Apply lag to get the effective date
        let effective_date = self.apply_lag(date)?;
        // Use the pre-built series (interpolation applied at construction/configuration time)
        let base_value = self.series_interp.value_on(effective_date)?;

        // Apply seasonality if present
        let adjusted_value = self.apply_seasonality(base_value, effective_date)?;

        Ok(adjusted_value)
    }

    /// Reference CPI for a date under the official months-lag daily
    /// interpolation rule (US TIPS, Canadian RRB, French OAT€i):
    ///
    /// ```text
    /// RefCPI(d) = CPI(m−L) + (day(d) − 1) / D(m) × [CPI(m−L+1) − CPI(m−L)]
    /// ```
    ///
    /// where `m` is the calendar month of `d`, `L = lag_months`, `D(m)` is the
    /// number of days in month `m`, and `CPI(·)` are the first-of-month index
    /// observations. Note the divisor is the length of the **settlement
    /// month**, not the distance between the lagged anchor observations; a
    /// generic calendar-time interpolation at `d − L months` gets both the
    /// weight and the day-clamping behaviour wrong at month ends.
    ///
    /// The index's own `lag` is **not** applied here — callers (e.g.
    /// inflation-linked bonds) own the lag and must use an index with
    /// `lag == None`.
    ///
    /// # Errors
    ///
    /// Returns an error when the anchor observations are unavailable or the
    /// anchor dates cannot be constructed.
    pub fn ref_cpi_months_lag(&self, date: Date, lag_months: u32) -> Result<f64> {
        let first_of_month = Date::from_calendar_date(date.year(), date.month(), 1)
            .map_err(|_| Error::Input(crate::error::InputError::InvalidDateRange))?;
        let anchor0 = first_of_month.add_months(-(lag_months as i32));
        let anchor1 = anchor0.add_months(1);

        let cpi0 = self.apply_seasonality(self.series_interp.value_on(anchor0)?, anchor0)?;
        let cpi1 = self.apply_seasonality(self.series_interp.value_on(anchor1)?, anchor1)?;

        let days_in_month = f64::from(date.month().length(date.year()));
        let weight = (f64::from(date.day()) - 1.0) / days_in_month;
        Ok(cpi0 + weight * (cpi1 - cpi0))
    }

    /// Calculate the index ratio `I(settle_date) / I(base_date)`.
    pub fn ratio(&self, base_date: Date, settle_date: Date) -> Result<f64> {
        let base_value = self.value_on(base_date)?;
        let settle_value = self.value_on(settle_date)?;

        if base_value == 0.0 {
            return Err(Error::Input(crate::error::InputError::NonPositiveValue));
        }

        Ok(settle_value / base_value)
    }

    /// Get the date range covered by observations
    pub fn date_range(&self) -> Result<(Date, Date)> {
        let observations = self.series.observations();

        if observations.is_empty() {
            return Err(Error::internal(
                "inflation index date_range requires at least one observation",
            ));
        }

        let start_date = observations
            .first()
            .map(|(d, _)| *d)
            .ok_or_else(|| Error::internal("inflation index observations missing first date"))?;
        let end_date = observations
            .last()
            .map(|(d, _)| *d)
            .ok_or_else(|| Error::internal("inflation index observations missing last date"))?;

        Ok((start_date, end_date))
    }

    /// Get all observations as (Date, value) pairs.
    ///
    /// Returns observations in chronological order.
    pub fn observations(&self) -> Vec<(Date, f64)> {
        self.series.observations()
    }

    /// Rebuild this index with replacement observations while preserving its
    /// interpolation, lag, currency, and seasonality conventions.
    pub(crate) fn with_replaced_observations(
        &self,
        observations: Vec<(Date, f64)>,
    ) -> Result<Self> {
        let mut rebuilt = Self::new(self.id.clone(), observations, self.currency)?
            .with_interpolation(self.interpolation)
            .with_lag(self.lag);
        if let Some(seasonality) = self.seasonality {
            rebuilt = rebuilt.with_seasonality(seasonality)?;
        }
        Ok(rebuilt)
    }

    /// Get the number of observations in the index.
    pub fn len(&self) -> usize {
        self.series.len()
    }

    /// Check if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.series.is_empty()
    }

    /// Expose current interpolation setting for bump/rebuild helpers.
    pub fn interpolation(&self) -> InflationInterpolation {
        self.interpolation
    }
    /// Expose current lag setting for bump/rebuild helpers.
    pub fn lag(&self) -> InflationLag {
        self.lag
    }

    // Private helper methods

    fn apply_lag(&self, date: Date) -> Result<Date> {
        match self.lag {
            InflationLag::None => Ok(date),
            InflationLag::Days(days) => date
                .checked_sub(time::Duration::days(days as i64))
                .ok_or(Error::Input(crate::error::InputError::InvalidDateRange)),
            InflationLag::Months(months) => {
                // Proper month arithmetic using shared helper
                Ok(date.add_months(-(months as i32)))
            }
        }
    }

    fn apply_seasonality(&self, base_value: f64, date: Date) -> Result<f64> {
        if let Some(factors) = &self.seasonality {
            let month_idx = (date.month() as usize) - 1;
            Ok(base_value * factors[month_idx])
        } else {
            Ok(base_value)
        }
    }
}

/// Raw serializable state of an InflationIndex
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RawInflationIndex {
    /// Unique identifier
    pub id: String,
    /// Currency
    pub currency: Currency,
    /// Observations as (date, value) pairs
    #[schemars(with = "Vec<(String, f64)>")]
    pub observations: Vec<(Date, f64)>,
    /// Interpolation method
    pub interpolation: InflationInterpolation,
    /// Lag policy
    pub lag: InflationLag,
    /// Optional seasonality factors
    pub seasonality: Option<[f64; 12]>,
}

impl JsonSchema for InflationIndex {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "InflationIndex".into()
    }

    fn json_schema(gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
        RawInflationIndex::json_schema(gen)
    }
}

impl From<InflationIndex> for RawInflationIndex {
    fn from(index: InflationIndex) -> Self {
        let observations = index.observations();

        RawInflationIndex {
            id: index.id.to_owned(),
            currency: index.currency,
            observations,
            interpolation: index.interpolation,
            lag: index.lag,
            seasonality: index.seasonality,
        }
    }
}

impl TryFrom<RawInflationIndex> for InflationIndex {
    type Error = crate::Error;

    fn try_from(state: RawInflationIndex) -> crate::Result<Self> {
        let mut index = Self::new(state.id, state.observations, state.currency)?
            .with_interpolation(state.interpolation)
            .with_lag(state.lag);

        if let Some(factors) = state.seasonality {
            index = index.with_seasonality(factors)?;
        }

        Ok(index)
    }
}

/// Builder for creating inflation indices from various sources
pub struct InflationIndexBuilder {
    id: String,
    currency: Currency,
    observations: Vec<(Date, f64)>,
    interpolation: InflationInterpolation,
    lag: InflationLag,
    seasonality: Option<[f64; 12]>,
}

impl InflationIndexBuilder {
    /// Create a new inflation index builder.
    pub fn new(id: impl Into<String>, currency: Currency) -> Self {
        Self {
            id: id.into(),
            currency,
            observations: Vec::new(),
            interpolation: InflationInterpolation::default(),
            lag: InflationLag::default(),
            seasonality: None,
        }
    }

    /// Add a single observation to the index.
    pub fn add_observation(mut self, date: Date, value: f64) -> Self {
        self.observations.push((date, value));
        self
    }

    /// Set all observations at once.
    pub fn with_observations(mut self, observations: Vec<(Date, f64)>) -> Self {
        self.observations = observations;
        self
    }

    /// Set the interpolation method.
    pub fn with_interpolation(mut self, interpolation: InflationInterpolation) -> Self {
        self.interpolation = interpolation;
        self
    }

    /// Set the lag policy.
    pub fn with_lag(mut self, lag: InflationLag) -> Self {
        self.lag = lag;
        self
    }

    /// Set seasonal adjustment factors (one per month).
    pub fn with_seasonality(mut self, factors: [f64; 12]) -> Self {
        self.seasonality = Some(factors);
        self
    }

    /// Build the inflation index.
    pub fn build(self) -> Result<InflationIndex> {
        let mut index = InflationIndex::new(self.id, self.observations, self.currency)?
            .with_interpolation(self.interpolation)
            .with_lag(self.lag);

        if let Some(factors) = self.seasonality {
            index = index.with_seasonality(factors)?;
        }

        Ok(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    fn make_date(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(
            year,
            Month::try_from(month).expect("Valid month (1-12)"),
            day,
        )
        .expect("Valid test date")
    }

    fn sample_cpi() -> InflationIndex {
        let observations = vec![
            (make_date(2023, 1, 31), 100.0),
            (make_date(2023, 2, 28), 101.0),
            (make_date(2023, 3, 31), 102.0),
            (make_date(2023, 4, 30), 102.5),
            (make_date(2023, 5, 31), 103.0),
        ];

        InflationIndex::new("US-CPI", observations, Currency::USD)
            .expect("InflationIndex creation should succeed in test")
    }

    #[test]
    fn test_inflation_creation() {
        let index = sample_cpi();
        assert_eq!(index.id, "US-CPI");
        assert_eq!(index.currency, Currency::USD);

        let (start, end) = index
            .date_range()
            .expect("Date range should exist for non-empty index");
        assert_eq!(start, make_date(2023, 1, 31));
        assert_eq!(end, make_date(2023, 5, 31));
    }

    #[test]
    fn test_step_interpolation() {
        let index = sample_cpi();

        // Exact date match
        let value = index
            .value_on(make_date(2023, 2, 28))
            .expect("Value lookup should succeed in test");
        assert_eq!(value, 101.0);

        // Between dates - should use previous value
        let value = index
            .value_on(make_date(2023, 3, 15))
            .expect("Value lookup should succeed in test");
        assert_eq!(value, 101.0);
    }

    #[test]
    fn test_linear_interpolation() {
        let index = sample_cpi().with_interpolation(InflationInterpolation::Linear);

        // Exact date
        let value = index
            .value_on(make_date(2023, 2, 28))
            .expect("Value lookup should succeed in test");
        assert_eq!(value, 101.0);

        // Interpolated value
        let value = index
            .value_on(make_date(2023, 3, 15))
            .expect("Value lookup should succeed in test");
        assert!(value > 101.0 && value < 102.0);
    }

    #[test]
    fn test_ratio_calculation() {
        let index = sample_cpi();

        let ratio = index
            .ratio(make_date(2023, 1, 31), make_date(2023, 5, 31))
            .expect("Ratio calculation should succeed in test");
        assert_eq!(ratio, 103.0 / 100.0);
    }

    #[test]
    fn test_with_lag() {
        let index = sample_cpi().with_lag(InflationLag::Months(1));

        // Value on Apr 30 with 1-month lag should give Mar 31 value (102.0)
        // However, with step interpolation (default), we get the previous value (101.0)
        // since March 30 (Apr 30 - 1 month) is between Feb 28 and Mar 31
        let value = index
            .value_on(make_date(2023, 4, 30))
            .expect("Value lookup should succeed in test");
        assert_eq!(value, 101.0); // Feb value due to step interpolation
    }

    #[test]
    fn test_with_days_lag() {
        let index = sample_cpi().with_lag(InflationLag::Days(31));

        let value = index
            .value_on(make_date(2023, 3, 31))
            .expect("days-lagged lookup should succeed");

        assert_eq!(value, 101.0);
    }

    #[test]
    fn ref_cpi_months_lag_uses_settlement_month_day_count() {
        let observations = vec![
            (make_date(2025, 1, 1), 300.0),
            (make_date(2025, 2, 1), 302.0),
            (make_date(2025, 3, 1), 305.1),
        ];
        let index = InflationIndex::new("US-CPI", observations, Currency::USD)
            .expect("index creation should succeed");

        let apr_16 = index
            .ref_cpi_months_lag(make_date(2025, 4, 16), 3)
            .expect("reference CPI should have January/February anchors");
        assert!((apr_16 - 301.0).abs() < 1e-12);

        let may_31 = index
            .ref_cpi_months_lag(make_date(2025, 5, 31), 3)
            .expect("reference CPI should have February/March anchors");
        let expected_may_31 = 302.0 + (30.0 / 31.0) * (305.1 - 302.0);
        assert!((may_31 - expected_may_31).abs() < 1e-12);

        let may_1 = index
            .ref_cpi_months_lag(make_date(2025, 5, 1), 3)
            .expect("first of settlement month should use the first lagged anchor");
        assert_eq!(may_1, 302.0);
    }

    #[test]
    fn seasonality_adjusts_values_and_survives_builder_path() {
        let mut seasonality = [1.0; 12];
        seasonality[2] = 1.10;

        let index = InflationIndex::builder("US-CPI", Currency::USD)
            .with_observations(vec![
                (make_date(2025, 2, 1), 200.0),
                (make_date(2025, 3, 1), 210.0),
            ])
            .with_interpolation(InflationInterpolation::Linear)
            .with_seasonality(seasonality)
            .build()
            .expect("builder should apply seasonality");

        let feb = index.value_on(make_date(2025, 2, 1)).unwrap();
        let mar = index.value_on(make_date(2025, 3, 1)).unwrap();
        assert_eq!(feb, 200.0);
        assert!((mar - 231.0).abs() < 1e-12);

        let json = serde_json::to_string(&index).unwrap();
        let roundtrip: InflationIndex = serde_json::from_str(&json).unwrap();
        assert!((roundtrip.value_on(make_date(2025, 3, 1)).unwrap() - 231.0).abs() < 1e-12);
    }

    #[test]
    fn test_builder_pattern() {
        let index = InflationIndexBuilder::new("UK-RPI", Currency::GBP)
            .add_observation(make_date(2023, 1, 31), 300.0)
            .add_observation(make_date(2023, 2, 28), 303.0)
            .with_interpolation(InflationInterpolation::Linear)
            .with_lag(InflationLag::Days(90))
            .build()
            .expect("Ratio calculation should succeed in test");

        assert_eq!(index.id, "UK-RPI");
        assert_eq!(index.currency, Currency::GBP);
        assert_eq!(index.interpolation, InflationInterpolation::Linear);
    }
}
