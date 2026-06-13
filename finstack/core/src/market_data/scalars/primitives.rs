//! Generic market primitives used by valuations: scalars and ad-hoc time series.
//!
//! This module provides two minimal building blocks that are not modeled as
//! classic term structures but are still required by pricing and risk engines:
//!
//! - `MarketScalar`: single numeric value (unitless or price in a currency)
//! - `ScalarTimeSeries`: generic date → value series with step/linear interp
//!
//! Both are integrated into the [`crate::market_data::context::MarketContext`]
//! so downstream code can reference them by `CurveId` alongside other curves.

use super::storage::TimeSeriesStorage;
use crate::currency::Currency;
use crate::dates::Date;
use crate::error::InputError;
use crate::types::CurveId;
use crate::Result;
#[cfg(test)]
use time::Duration as TimeDuration;

/// Interpolation strategy for [`ScalarTimeSeries`].
///
/// # Examples
/// ```rust
/// use finstack_core::market_data::scalars::{ScalarTimeSeries, SeriesInterpolation};
/// use finstack_core::dates::Date;
/// use time::Month;
///
/// let series = ScalarTimeSeries::new(
///     "TS",
///     vec![
///         (Date::from_calendar_date(2024, Month::January, 1).expect("Valid date"), 100.0),
///         (Date::from_calendar_date(2024, Month::February, 1).expect("Valid date"), 105.0),
///     ],
///     None,
/// )
/// .expect("Series creation should succeed");
/// let stepped = series.clone().with_interpolation(SeriesInterpolation::Step);
/// let mid_date = Date::from_calendar_date(2024, Month::January, 15).expect("Valid date");
/// assert_eq!(stepped.value_on(mid_date).expect("Value lookup should succeed"), 100.0);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeriesInterpolation {
    /// Last observation carried forward
    #[default]
    Step,
    /// Linear interpolation between observed points
    Linear,
}

impl std::fmt::Display for SeriesInterpolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Step => write!(f, "step"),
            Self::Linear => write!(f, "linear"),
        }
    }
}

impl crate::parse::NormalizedEnum for SeriesInterpolation {
    const VARIANTS: &'static [(&'static str, Self)] =
        &[("step", Self::Step), ("linear", Self::Linear)];
}

impl std::str::FromStr for SeriesInterpolation {
    type Err = crate::error::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        crate::parse::parse_normalized_enum(s).map_err(|_| crate::error::InputError::Invalid.into())
    }
}

/// Single market observable that doesn't require a full curve.
///
/// Represents point-in-time market data like spot prices, spreads, or unitless
/// parameters. Stored in [`MarketContext`](crate::market_data::context::MarketContext)
/// alongside curves for simple lookups.
///
/// # Use Cases
///
/// - **Spot prices**: Equity spots, commodity prices, FX spots
/// - **Recovery rates**: Credit recovery assumptions (unitless)
/// - **Correlation parameters**: Equity-FX correlation, basis correlations
/// - **Spreads**: Credit spreads, basis spreads (unitless or monetary)
/// - **Multipliers**: Beta, vega notionals, adjustment factors
///
/// # Examples
///
/// ```rust
/// use finstack_core::market_data::scalars::MarketScalar;
/// use finstack_core::money::Money;
/// use finstack_core::currency::Currency;
///
/// // Equity beta (unitless)
/// let beta = MarketScalar::Unitless(1.2);
///
/// // Spot price (with currency)
/// let spot = MarketScalar::Price(Money::new(152.75, Currency::USD));
///
/// assert!(matches!(beta, MarketScalar::Unitless(_)));
/// if let MarketScalar::Price(m) = spot {
///     assert_eq!(m.currency(), Currency::USD);
/// }
/// ```
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketScalar {
    /// Unitless numeric (e.g., equity beta, recovery rate assumption)
    Unitless(f64),
    /// Monetary price or amount with currency
    Price(crate::money::Money),
}

/// Date-indexed time series with flexible interpolation.
///
/// Provides lightweight storage for historical or forecast data with step or
/// linear interpolation. Unlike full term structures, this is optimized for
/// sparse, irregularly-spaced observations.
///
/// # Storage
///
/// Uses columnar format with parallel arrays:
/// - Dates stored as i32 (days since Unix epoch) for compact size
/// - Values stored as f64
/// - Binary search for O(log n) lookup
///
/// # Interpolation
///
/// - **Step**: Last observation carried forward (LOCF)
/// - **Linear**: Linear interpolation between observations
///
/// # Use Cases
///
/// - **Economic indicators**: GDP, unemployment rate, PMI
/// - **Credit metrics**: Historical credit spreads, CDS levels
/// - **Commodity fundamentals**: Inventory levels, production data
/// - **Any sparse time series**: Where full curve infrastructure is overkill
///
/// # Examples
///
/// ```rust
/// use finstack_core::market_data::scalars::{ScalarTimeSeries, SeriesInterpolation};
/// use finstack_core::dates::Date;
/// use time::Month;
///
/// let series = ScalarTimeSeries::new(
///     "US-UNEMPLOYMENT",
///     vec![
///         (Date::from_calendar_date(2024, Month::January, 31).expect("Valid date"), 3.7),
///         (Date::from_calendar_date(2024, Month::February, 29).expect("Valid date"), 3.9),
///     ],
///     None,
/// )
/// .expect("Series creation should succeed")
/// .with_interpolation(SeriesInterpolation::Linear);
///
/// let mid = Date::from_calendar_date(2024, Month::February, 14).expect("Valid date");
/// let interpolated = series.value_on(mid).expect("Value lookup should succeed");
/// assert!(interpolated > 3.7 && interpolated < 3.9);
/// ```
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "RawScalarTimeSeries", into = "RawScalarTimeSeries")]
pub struct ScalarTimeSeries {
    id: CurveId,
    currency: Option<Currency>,
    /// Lightweight storage: parallel arrays of dates (days since epoch) and values
    data: TimeSeriesStorage,
    interpolation: SeriesInterpolation,
}

impl ScalarTimeSeries {
    /// Create a new time series from `(Date, value)` observations.
    ///
    /// # Parameters
    /// - `id`: identifier used when storing the series inside [`MarketContext`](crate::market_data::context::MarketContext)
    /// - `observations`: sorted or unsorted list of observations (duplicates not allowed)
    /// - `currency`: optional currency tag when the series represents a monetary amount
    ///
    /// # Examples
    /// ```rust
    /// use finstack_core::market_data::scalars::ScalarTimeSeries;
    /// use finstack_core::dates::Date;
    /// use time::Month;
    ///
    /// let series = ScalarTimeSeries::new(
    ///     "TREASURY",
    ///     vec![
    ///         (Date::from_calendar_date(2024, Month::January, 1).expect("Valid date"), 4.5),
    ///         (Date::from_calendar_date(2024, Month::February, 1).expect("Valid date"), 4.7),
    ///     ],
    ///     None,
    /// )
    /// .expect("Series creation should succeed");
    /// assert_eq!(series.id().as_str(), "TREASURY");
    /// ```
    pub fn new(
        id: impl AsRef<str>,
        observations: Vec<(Date, f64)>,
        currency: Option<Currency>,
    ) -> Result<Self> {
        if observations.is_empty() {
            return Err(crate::Error::Input(InputError::TooFewPoints));
        }

        // Convert dates to days since epoch
        let observations_i32: Vec<(i32, f64)> = observations
            .into_iter()
            .map(|(d, v)| (to_days(d), v))
            .collect();

        // Create storage (handles sorting and duplicate detection)
        let data = TimeSeriesStorage::new(observations_i32)?;

        Ok(Self {
            id: CurveId::from(id.as_ref()),
            currency,
            data,
            interpolation: SeriesInterpolation::default(),
        })
    }

    /// Override the interpolation method used for lookups.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_core::market_data::scalars::{ScalarTimeSeries, SeriesInterpolation};
    /// use time::macros::date;
    /// # fn main() -> finstack_core::Result<()> {
    ///
    /// let series = ScalarTimeSeries::new(
    ///     "TS",
    ///     vec![
    ///         (date!(2024 - 01 - 01), 10.0),
    ///         (date!(2024 - 02 - 01), 20.0),
    ///     ],
    ///     None,
    /// )
    /// ?
    /// .with_interpolation(SeriesInterpolation::Linear);
    /// assert!(matches!(series.interpolation(), SeriesInterpolation::Linear));
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_interpolation(mut self, interpolation: SeriesInterpolation) -> Self {
        self.interpolation = interpolation;
        self
    }

    /// Identifier accessor.
    pub fn id(&self) -> &CurveId {
        &self.id
    }

    /// Optional currency accessor.
    pub fn currency(&self) -> Option<Currency> {
        self.currency
    }

    /// Current interpolation method accessor.
    pub fn interpolation(&self) -> SeriesInterpolation {
        self.interpolation
    }

    /// Retrieve the value on a given date according to the active interpolation.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_core::market_data::scalars::{ScalarTimeSeries, SeriesInterpolation};
    /// use finstack_core::dates::Date;
    /// use time::Month;
    ///
    /// let series = ScalarTimeSeries::new(
    ///     "TS",
    ///     vec![
    ///         (Date::from_calendar_date(2024, Month::January, 1).expect("Valid date"), 10.0),
    ///         (Date::from_calendar_date(2024, Month::February, 1).expect("Valid date"), 20.0),
    ///     ],
    ///     None,
    /// )
    /// .expect("Series creation should succeed");
    /// let jan = Date::from_calendar_date(2024, Month::January, 15).expect("Valid date");
    /// assert_eq!(series.value_on(jan).expect("Value lookup should succeed"), 10.0);
    ///
    /// let linear = series.clone().with_interpolation(SeriesInterpolation::Linear);
    /// assert!(linear.value_on(jan).expect("Value lookup should succeed") > 10.0);
    /// ```
    pub fn value_on(&self, date: Date) -> Result<f64> {
        let days = to_days(date);
        self.values_on_days(&[days]).map(|v| v[0])
    }

    /// Retrieve the value for an **exactly observed** date.
    ///
    /// Unlike [`Self::value_on`], this does **not** interpolate or carry values forward.
    /// It is intended for market fixings where missing observations must be treated
    /// as an error (e.g., RFR overnight fixings).
    pub fn value_on_exact(&self, date: Date) -> Result<f64> {
        use crate::error::InputError;

        let q = to_days(date);
        let date_vec = self.data.dates();
        let value_vec = self.data.values();
        match date_vec.binary_search(&q) {
            Ok(idx) => Ok(value_vec[idx]),
            Err(_) => Err(crate::Error::Input(InputError::NotFound {
                id: format!("series '{}' observation on {}", self.id.as_str(), date),
            })),
        }
    }

    /// Retrieve the value for the most recent observation on or before `date`.
    ///
    /// If an exact observation exists on `date`, it is returned. Otherwise the
    /// most recent prior observation within `max_staleness_days` calendar days
    /// is returned. If no observation falls within the staleness window, an
    /// error is returned.
    ///
    /// This is intended for production fixing lookups where data feeds may
    /// occasionally miss a single business day. For strict exact-date lookups
    /// (e.g., unit tests), use [`Self::value_on_exact`].
    pub fn value_on_or_before(&self, date: Date, max_staleness_days: u32) -> Result<f64> {
        let q = to_days(date);
        let date_vec = self.data.dates();
        let value_vec = self.data.values();

        match date_vec.binary_search(&q) {
            Ok(idx) => Ok(value_vec[idx]),
            Err(insert_pos) => {
                if insert_pos == 0 {
                    return Err(crate::Error::Input(InputError::NotFound {
                        id: format!(
                            "series '{}': no observation on or before {}",
                            self.id.as_str(),
                            date
                        ),
                    }));
                }
                let prev_idx = insert_pos - 1;
                let stale_days = (q - date_vec[prev_idx]) as u32;
                if stale_days > max_staleness_days {
                    return Err(crate::Error::Input(InputError::NotFound {
                        id: format!(
                            "series '{}': nearest observation before {} is {} days stale (limit: {})",
                            self.id.as_str(),
                            date,
                            stale_days,
                            max_staleness_days,
                        ),
                    }));
                }
                Ok(value_vec[prev_idx])
            }
        }
    }

    /// Retrieve values for multiple dates at once.
    ///
    /// The returned vector is aligned with the input order. Step interpolation
    /// carries the last observation forward while Linear blends between
    /// neighboring observations.
    pub fn values_on(&self, dates: &[Date]) -> Result<Vec<f64>> {
        let query_days: Vec<i32> = dates.iter().map(|&d| to_days(d)).collect();
        self.values_on_days(&query_days)
    }

    /// Internal vectorized lookup using days since epoch.
    fn values_on_days(&self, query_days: &[i32]) -> Result<Vec<f64>> {
        if query_days.is_empty() {
            return Ok(Vec::new());
        }

        // Access storage arrays directly
        let date_vec = self.data.dates();
        let value_vec = self.data.values();

        // Vectorized interpolation with optimized branch prediction
        match self.interpolation {
            SeriesInterpolation::Step => {
                self.vectorized_step_interpolation(date_vec, value_vec, query_days)
            }
            SeriesInterpolation::Linear => {
                self.vectorized_linear_interpolation(date_vec, value_vec, query_days)
            }
        }
    }

    /// Optimized step interpolation for multiple query points.
    fn vectorized_step_interpolation(
        &self,
        date_vec: &[i32],
        value_vec: &[f64],
        query_days: &[i32],
    ) -> Result<Vec<f64>> {
        let mut result = Vec::with_capacity(query_days.len());

        for &query_day in query_days {
            let value = match date_vec.binary_search(&query_day) {
                Ok(idx) => value_vec[idx],
                Err(idx) => {
                    if idx == 0 {
                        return Err(crate::Error::Input(InputError::NotFound {
                            id: format!(
                                "series '{}': no observation on or before {}",
                                self.id.as_str(),
                                from_days(query_day),
                            ),
                        }));
                    } else {
                        value_vec[idx - 1] // Last observation carried forward
                    }
                }
            };
            result.push(value);
        }

        Ok(result)
    }

    /// Optimized linear interpolation for multiple query points.
    fn vectorized_linear_interpolation(
        &self,
        date_vec: &[i32],
        value_vec: &[f64],
        query_days: &[i32],
    ) -> Result<Vec<f64>> {
        let mut result = Vec::with_capacity(query_days.len());

        for &query_day in query_days {
            let value = match date_vec.binary_search(&query_day) {
                Ok(idx) => value_vec[idx],
                Err(idx) => {
                    if idx == 0 {
                        return Err(crate::Error::Input(InputError::NotFound {
                            id: format!(
                                "series '{}': no observation on or before {}",
                                self.id.as_str(),
                                from_days(query_day),
                            ),
                        }));
                    } else if idx >= date_vec.len() {
                        *value_vec.last().ok_or(InputError::TooFewPoints)? // Use last value for dates after series
                    } else {
                        // Linear interpolation between adjacent points
                        let x0 = date_vec[idx - 1] as f64;
                        let x1 = date_vec[idx] as f64;
                        let y0 = value_vec[idx - 1];
                        let y1 = value_vec[idx];
                        let weight = (query_day as f64 - x0) / (x1 - x0);
                        y0 + weight * (y1 - y0)
                    }
                }
            };
            result.push(value);
        }

        Ok(result)
    }

    /// Get observations as (Date, value) pairs.
    ///
    /// Returns all stored observations in chronological order.
    pub fn observations(&self) -> Vec<(Date, f64)> {
        self.data
            .iter()
            .map(|(days, value)| {
                let date = from_days(days);
                (date, value)
            })
            .collect()
    }

    /// Get the number of observations in the series.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if the series is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

fn to_days(date: Date) -> i32 {
    // Epoch date - unwrap_or provides defensive fallback for infallible operation
    let epoch = Date::from_calendar_date(1970, time::Month::January, 1).unwrap_or(time::Date::MIN);
    (date - epoch).whole_days() as i32
}

fn from_days(days: i32) -> Date {
    // Epoch date - unwrap_or provides defensive fallback for infallible operation
    let epoch = Date::from_calendar_date(1970, time::Month::January, 1).unwrap_or(time::Date::MIN);
    epoch + time::Duration::days(days as i64)
}

/// Raw serializable state of a ScalarTimeSeries
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawScalarTimeSeries {
    /// Series identifier
    pub id: String,
    /// Optional currency
    pub currency: Option<Currency>,
    /// Observations as (date, value) pairs
    pub observations: Vec<(Date, f64)>,
    /// Interpolation method
    pub interpolation: SeriesInterpolation,
}

impl From<ScalarTimeSeries> for RawScalarTimeSeries {
    fn from(series: ScalarTimeSeries) -> Self {
        let observations = series.observations();

        RawScalarTimeSeries {
            id: series.id.to_string(),
            currency: series.currency,
            observations,
            interpolation: series.interpolation,
        }
    }
}

impl TryFrom<RawScalarTimeSeries> for ScalarTimeSeries {
    type Error = crate::Error;

    fn try_from(state: RawScalarTimeSeries) -> Result<Self> {
        Self::new(state.id, state.observations, state.currency)
            .map(|s| s.with_interpolation(state.interpolation))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn series_step_and_linear() {
        let d0 =
            time::Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date");
        let d1 = time::Date::from_calendar_date(2025, time::Month::February, 1)
            .expect("Valid test date");
        let d2 =
            time::Date::from_calendar_date(2025, time::Month::March, 1).expect("Valid test date");
        let s = ScalarTimeSeries::new("US-UNEMP", vec![(d0, 3.0), (d1, 4.0), (d2, 5.0)], None)
            .expect("ScalarTimeSeries creation should succeed in test");

        // Midpoint between d0 and d1
        let mid = d0 + TimeDuration::days(15);
        let step_v = s
            .clone()
            .with_interpolation(SeriesInterpolation::Step)
            .value_on(mid)
            .expect("Value lookup should succeed in test");
        assert!((step_v - 3.0).abs() < 1e-12);

        let lin_v = s
            .with_interpolation(SeriesInterpolation::Linear)
            .value_on(mid)
            .expect("Value lookup should succeed in test");
        assert!(lin_v > 3.0 && lin_v < 4.0);
    }

    #[test]
    fn linear_interpolation_after_last_date_uses_last_value() {
        let d0 = time::Date::from_calendar_date(2025, time::Month::January, 1).expect("date");
        let d1 = time::Date::from_calendar_date(2025, time::Month::February, 1).expect("date");
        let series = ScalarTimeSeries::new("TS", vec![(d0, 10.0), (d1, 20.0)], None)
            .expect("series")
            .with_interpolation(SeriesInterpolation::Linear);

        let after = time::Date::from_calendar_date(2025, time::Month::March, 1).expect("date");
        assert_eq!(series.value_on(after).expect("past-end lookup"), 20.0);
    }

    #[test]
    fn scalar_time_series_rejects_unknown_serde_field() {
        let json = r#"{
            "id":"TS",
            "currency":null,
            "observations":[["2025-01-01",10.0],["2025-02-01",20.0]],
            "interpolation":"step",
            "extra":true
        }"#;

        assert!(serde_json::from_str::<ScalarTimeSeries>(json).is_err());
    }

    #[test]
    fn test_scalar_time_series_empty_error() {
        // Test that empty series returns proper error
        let result = ScalarTimeSeries::new("TEST", vec![], None);
        assert!(result.is_err());

        match result.expect_err("Should fail with too few points") {
            crate::Error::Input(crate::error::InputError::TooFewPoints) => {
                // Expected error type
            }
            _ => panic!("Expected TooFewPoints error for empty series"),
        }
    }

    #[test]
    fn test_scalar_time_series_single_point_error() {
        let single_obs = vec![(
            time::Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date"),
            100.0,
        )];

        let series = ScalarTimeSeries::new("TEST", single_obs, None)
            .expect("single-point series should be accepted");
        assert_eq!(series.observations().len(), 1);
    }

    #[test]
    fn test_scalar_time_series_valid_cases() {
        // Test valid series creation
        let observations = vec![
            (
                time::Date::from_calendar_date(2025, time::Month::January, 1)
                    .expect("Valid test date"),
                100.0,
            ),
            (
                time::Date::from_calendar_date(2025, time::Month::February, 1)
                    .expect("Valid test date"),
                200.0,
            ),
            (
                time::Date::from_calendar_date(2025, time::Month::March, 1)
                    .expect("Valid test date"),
                300.0,
            ),
        ];

        let result = ScalarTimeSeries::new("TEST", observations, None);
        assert!(result.is_ok());

        let series = result.expect("ScalarTimeSeries creation should succeed in test");
        assert_eq!(series.observations().len(), 3);
        assert_eq!(series.id.as_str(), "TEST");
    }

    #[test]
    fn test_scalar_time_series_large_dataset() {
        // Test with larger dataset to ensure performance and correctness
        let mut observations = Vec::new();

        for i in 0..30 {
            // Use a smaller range to stay within January
            let day = 1 + i as u8;
            if day <= 31 {
                // Make sure we don't exceed January 31
                let date = time::Date::from_calendar_date(2025, time::Month::January, day)
                    .expect("Valid test date");
                observations.push((date, i as f64));
            }
        }

        let result = ScalarTimeSeries::new("TEST", observations, None);
        assert!(result.is_ok());

        let series = result.expect("ScalarTimeSeries creation should succeed in test");
        assert!(series.observations().len() >= 25); // At least 25 valid days
    }

    #[test]
    fn value_on_or_before_exact_match() {
        let d0 = time::Date::from_calendar_date(2025, time::Month::January, 6).expect("date");
        let d1 = time::Date::from_calendar_date(2025, time::Month::January, 7).expect("date");
        let d2 = time::Date::from_calendar_date(2025, time::Month::January, 8).expect("date");
        let s = ScalarTimeSeries::new("FIX", vec![(d0, 0.05), (d1, 0.051), (d2, 0.052)], None)
            .expect("series");

        let v = s.value_on_or_before(d1, 3).expect("exact match");
        assert!((v - 0.051).abs() < 1e-12);
    }

    #[test]
    fn value_on_or_before_uses_prior_within_window() {
        let d0 = time::Date::from_calendar_date(2025, time::Month::January, 3).expect("date"); // Friday
        let d2 = time::Date::from_calendar_date(2025, time::Month::January, 6).expect("date"); // Monday (3 days gap)
        let s = ScalarTimeSeries::new("FIX", vec![(d0, 0.05), (d2, 0.052)], None).expect("series");

        // Saturday (1 day after Friday) with 3-day staleness window -> should return Friday
        let sat = time::Date::from_calendar_date(2025, time::Month::January, 4).expect("date");
        let v = s.value_on_or_before(sat, 3).expect("prior within window");
        assert!((v - 0.05).abs() < 1e-12);
    }

    #[test]
    fn value_on_or_before_rejects_stale() {
        let d0 = time::Date::from_calendar_date(2025, time::Month::January, 1).expect("date");
        let d1 = time::Date::from_calendar_date(2025, time::Month::January, 10).expect("date");
        let s = ScalarTimeSeries::new("FIX", vec![(d0, 0.05), (d1, 0.06)], None).expect("series");

        // 5 days after d0, with 3-day staleness limit -> should fail
        let query = time::Date::from_calendar_date(2025, time::Month::January, 6).expect("date");
        let result = s.value_on_or_before(query, 3);
        assert!(result.is_err(), "Should reject observations > 3 days stale");
    }

    #[test]
    fn value_on_or_before_no_prior_observation() {
        let d0 = time::Date::from_calendar_date(2025, time::Month::January, 10).expect("date");
        let d1 = time::Date::from_calendar_date(2025, time::Month::January, 20).expect("date");
        let s = ScalarTimeSeries::new("FIX", vec![(d0, 0.05), (d1, 0.06)], None).expect("series");

        let query = time::Date::from_calendar_date(2025, time::Month::January, 5).expect("date");
        let result = s.value_on_or_before(query, 10);
        assert!(
            result.is_err(),
            "Should fail when no prior observation exists"
        );
    }

    #[test]
    fn series_interpolation_fromstr_display_roundtrip() {
        use std::str::FromStr;
        let variants = [SeriesInterpolation::Step, SeriesInterpolation::Linear];
        for v in variants {
            let s = v.to_string();
            let parsed = SeriesInterpolation::from_str(&s).expect("roundtrip parse should succeed");
            assert_eq!(v, parsed, "roundtrip failed for {s}");
        }
        assert!(SeriesInterpolation::from_str("invalid").is_err());
    }
}
