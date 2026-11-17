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

use crate::InputError;
use crate::Result;
use crate::currency::Currency;
use crate::dates::Date;
use crate::market_data::bumps::{BumpMode, BumpSpec, BumpUnits, Bumpable};
use crate::types::CurveId;
#[cfg(test)]
use time::Duration as TimeDuration;

/// Interpolation strategy for [`ScalarTimeSeries`].
///
/// # Examples
/// ```rust
/// use finstack_quant_core::market_data::scalars::{
///     ScalarTimeSeries, SeriesExtrapolation, SeriesInterpolation,
/// };
/// use finstack_quant_core::dates::Date;
/// use time::Month;
///
/// let series = ScalarTimeSeries::new(
///     "TS",
///     vec![
///         (Date::from_calendar_date(2024, Month::January, 1).expect("Valid date"), 100.0),
///         (Date::from_calendar_date(2024, Month::February, 1).expect("Valid date"), 105.0),
///     ],
///     None,
///     SeriesInterpolation::Step,
///     SeriesExtrapolation::Error,
/// )
/// .expect("Series creation should succeed");
/// let mid_date = Date::from_calendar_date(2024, Month::January, 15).expect("Valid date");
/// assert_eq!(series.get_value(mid_date).expect("Value lookup should succeed"), 100.0);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SeriesInterpolation {
    /// Last observation carried forward
    Step,
    /// Linear interpolation between observed points
    Linear,
}

/// Right-tail extrapolation policy for [`ScalarTimeSeries`].
///
/// Applies only to dates strictly after the latest observation; queries on an
/// empty series or strictly before the earliest observation always fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SeriesExtrapolation {
    /// Hold the latest observed value flat after the final observation date.
    Flat,
    /// Return an error when querying strictly after the final observation date.
    Error,
}

impl std::fmt::Display for SeriesExtrapolation {
    /// Writes the serde label (`flat` or `error`).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&crate::wire::serde_label(self).map_err(|_| std::fmt::Error)?)
    }
}

impl std::str::FromStr for SeriesExtrapolation {
    type Err = crate::Error;

    /// Parses the serde label; the error names the accepted labels.
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        crate::wire::serde_parse(s)
    }
}

impl std::fmt::Display for SeriesInterpolation {
    /// Writes the serde label (`step` or `linear`).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&crate::wire::serde_label(self).map_err(|_| std::fmt::Error)?)
    }
}

impl std::str::FromStr for SeriesInterpolation {
    type Err = crate::Error;

    /// Parses the serde label; the error names the accepted labels.
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        crate::wire::serde_parse(s)
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
/// use finstack_quant_core::market_data::scalars::MarketScalar;
/// use finstack_quant_core::money::Money;
/// use finstack_quant_core::currency::Currency;
/// # fn main() -> finstack_quant_core::Result<()> {
///
/// // Equity beta (unitless)
/// let beta = MarketScalar::unitless(1.2)?;
///
/// // Spot price (with currency)
/// let spot = MarketScalar::price(Money::new(152.75, Currency::USD).expect("valid money fixture"));
///
/// assert!(beta.is_unitless());
/// if let Some(m) = spot.get_price() {
///     assert_eq!(m.currency(), Currency::USD);
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "MarketScalarWire", into = "MarketScalarWire")]
#[cfg_attr(feature = "json-schema", schemars(try_from = "MarketScalarWire"))]
pub struct MarketScalar(MarketScalarKind);

#[derive(Clone, Debug, PartialEq)]
enum MarketScalarKind {
    Unitless(f64),
    Price(crate::money::Money),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum MarketScalarWire {
    Unitless(f64),
    Price(crate::money::Money),
}

impl MarketScalar {
    /// Build a unitless scalar, rejecting non-finite values and normalizing `-0.0` to `0.0`.
    ///
    /// This is the validating constructor behind host `insert_price` calls
    /// without a currency; it applies the same rule as deserialization.
    ///
    /// # Arguments
    ///
    /// * `value` - Dimensionless observation (for example an equity beta or a
    ///   recovery-rate assumption as a decimal); must be finite.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] when `value` is NaN or infinite.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::market_data::scalars::MarketScalar;
    ///
    /// assert_eq!(MarketScalar::unitless(1.2)?.get_unitless(), Some(1.2));
    /// assert!(MarketScalar::unitless(f64::NAN).is_err());
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn unitless(value: f64) -> crate::Result<Self> {
        if value.is_finite() {
            let normalized = if value == 0.0 { 0.0 } else { value };
            Ok(Self(MarketScalarKind::Unitless(normalized)))
        } else {
            Err(crate::Error::Validation(
                "MarketScalar unitless value must be finite".into(),
            ))
        }
    }

    /// Build a monetary scalar with currency from a validated [`crate::money::Money`] value.
    ///
    /// # Arguments
    ///
    /// * `money` - Validated monetary price or amount with ISO-4217 currency.
    #[must_use]
    pub fn price(money: crate::money::Money) -> Self {
        Self(MarketScalarKind::Price(money))
    }

    /// Build a scalar from a value and an optional currency: a monetary
    /// price when `currency` is given, otherwise a unitless scalar.
    ///
    /// This is the construction behind host `insert_price(id, value, currency?)`.
    ///
    /// # Arguments
    ///
    /// * `value` - Finite observation: a price in major units of `currency`,
    ///   or a dimensionless value when `currency` is `None`.
    /// * `currency` - ISO-4217 currency of a monetary price; `None` for a
    ///   unitless scalar.
    ///
    /// # Errors
    ///
    /// Returns an error when `value` is NaN or infinite (or, for a price, not
    /// representable as a `Money` amount).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::market_data::scalars::MarketScalar;
    ///
    /// assert!(MarketScalar::from_parts(1.2, None)?.is_unitless());
    /// assert!(MarketScalar::from_parts(101.5, Some(Currency::USD))?.is_price());
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn from_parts(
        value: f64,
        currency: Option<crate::currency::Currency>,
    ) -> crate::Result<Self> {
        match currency {
            Some(currency) => crate::money::Money::new(value, currency).map(Self::price),
            None => Self::unitless(value),
        }
    }

    /// Return the underlying value when this scalar is unitless, or `None` for a monetary price.
    #[must_use]
    pub fn get_unitless(&self) -> Option<f64> {
        match self.0 {
            MarketScalarKind::Unitless(v) => Some(v),
            MarketScalarKind::Price(_) => None,
        }
    }

    /// Return the underlying [`crate::money::Money`] when this scalar is a monetary price, or `None` when unitless.
    #[must_use]
    pub fn get_price(&self) -> Option<crate::money::Money> {
        match self.0 {
            MarketScalarKind::Price(m) => Some(m),
            MarketScalarKind::Unitless(_) => None,
        }
    }

    /// Return `true` if this scalar holds a unitless observation.
    #[must_use]
    pub fn is_unitless(&self) -> bool {
        matches!(self.0, MarketScalarKind::Unitless(_))
    }

    /// Return `true` if this scalar holds a monetary price with currency.
    #[must_use]
    pub fn is_price(&self) -> bool {
        matches!(self.0, MarketScalarKind::Price(_))
    }
}

impl TryFrom<MarketScalarWire> for MarketScalar {
    type Error = crate::Error;

    fn try_from(raw: MarketScalarWire) -> std::result::Result<Self, Self::Error> {
        match raw {
            MarketScalarWire::Unitless(value) => Self::unitless(value),
            MarketScalarWire::Price(value) => Ok(Self::price(value)),
        }
    }
}

impl From<MarketScalar> for MarketScalarWire {
    fn from(value: MarketScalar) -> Self {
        match value.0 {
            MarketScalarKind::Unitless(value) => Self::Unitless(value),
            MarketScalarKind::Price(value) => Self::Price(value),
        }
    }
}

/// Columnar date-indexed storage behind [`ScalarTimeSeries`].
///
/// Stores dates (days since 1970-01-01) and values in parallel arrays,
/// sorted ascending with no duplicate dates.
#[derive(Clone, Debug)]
struct TimeSeriesStorage {
    /// Dates as days since epoch (1970-01-01), sorted ascending, no duplicates
    dates: Box<[i32]>,
    /// Values aligned with dates
    values: Box<[f64]>,
}

impl TimeSeriesStorage {
    /// Create from strictly ascending `(days_since_epoch, value)` observations,
    /// rejecting non-finite values, unsorted dates, and duplicate dates.
    /// Empty and single-point observations are admitted.
    fn new(observations: Vec<(i32, f64)>) -> Result<Self> {
        for window in observations.windows(2) {
            if window[0].0 >= window[1].0 {
                return Err(crate::Error::Input(InputError::NonMonotonicKnots));
            }
        }

        let mut dates = Vec::with_capacity(observations.len());
        let mut values = Vec::with_capacity(observations.len());
        for (date, value) in observations {
            if !value.is_finite() {
                return Err(crate::Error::Validation(
                    "time-series observation values must be finite".to_string(),
                ));
            }
            let normalized = if value == 0.0 { 0.0 } else { value };
            dates.push(date);
            values.push(normalized);
        }

        Ok(Self {
            dates: dates.into_boxed_slice(),
            values: values.into_boxed_slice(),
        })
    }

    /// Length of series.
    #[inline]
    fn len(&self) -> usize {
        self.dates.len()
    }

    /// Check if empty.
    #[inline]
    fn is_empty(&self) -> bool {
        self.dates.is_empty()
    }

    /// Get all dates as a slice.
    #[inline]
    fn dates(&self) -> &[i32] {
        &self.dates
    }

    /// Get all values as a slice.
    #[inline]
    fn values(&self) -> &[f64] {
        &self.values
    }

    /// Iterator over (date, value) pairs.
    fn iter(&self) -> impl Iterator<Item = (i32, f64)> + '_ {
        self.dates
            .iter()
            .zip(self.values.iter())
            .map(|(&d, &v)| (d, v))
    }
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
/// use finstack_quant_core::market_data::scalars::{
///     ScalarTimeSeries, SeriesExtrapolation, SeriesInterpolation,
/// };
/// use finstack_quant_core::dates::Date;
/// use time::Month;
///
/// let series = ScalarTimeSeries::new(
///     "US-UNEMPLOYMENT",
///     vec![
///         (Date::from_calendar_date(2024, Month::January, 31).expect("Valid date"), 3.7),
///         (Date::from_calendar_date(2024, Month::February, 29).expect("Valid date"), 3.9),
///     ],
///     None,
///     SeriesInterpolation::Linear,
///     SeriesExtrapolation::Error,
/// )
/// .expect("Series creation should succeed");
///
/// let mid = Date::from_calendar_date(2024, Month::February, 14).expect("Valid date");
/// let interpolated = series.get_value(mid).expect("Value lookup should succeed");
/// assert!(interpolated > 3.7 && interpolated < 3.9);
/// ```
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "ScalarTimeSeriesWire", into = "ScalarTimeSeriesWire")]
#[cfg_attr(feature = "json-schema", schemars(try_from = "ScalarTimeSeriesWire"))]
pub struct ScalarTimeSeries {
    id: CurveId,
    currency: Option<Currency>,
    /// Lightweight storage: parallel arrays of dates (days since epoch) and values
    data: TimeSeriesStorage,
    interpolation: SeriesInterpolation,
    extrapolation: SeriesExtrapolation,
}

impl ScalarTimeSeries {
    /// Create a new time series from strictly increasing `(Date, value)` observations.
    ///
    /// # Arguments
    ///
    /// * `id` - Non-empty trimmed identifier used when storing the series inside
    ///   [`MarketContext`](crate::market_data::context::MarketContext).
    /// * `observations` - Chronologically strictly increasing `(Date, f64)` pairs;
    ///   empty and single-point series are accepted, unsorted or duplicate dates
    ///   and non-finite values are rejected, and `-0.0` is normalized to `0.0`.
    /// * `currency` - Optional ISO-4217 currency tag when the series represents a
    ///   monetary amount, or `None` for dimensionless observations or rate fixings.
    /// * `interpolation` - Strategy used for in-range dates between adjacent observations.
    /// * `extrapolation` - Policy used for dates strictly after the latest observation.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::market_data::scalars::{
    ///     ScalarTimeSeries, SeriesExtrapolation, SeriesInterpolation,
    /// };
    /// use finstack_quant_core::dates::Date;
    /// use time::Month;
    ///
    /// let series = ScalarTimeSeries::new(
    ///     "TREASURY",
    ///     vec![
    ///         (Date::from_calendar_date(2024, Month::January, 1).expect("Valid date"), 4.5),
    ///         (Date::from_calendar_date(2024, Month::February, 1).expect("Valid date"), 4.7),
    ///     ],
    ///     None,
    ///     SeriesInterpolation::Step,
    ///     SeriesExtrapolation::Error,
    /// )
    /// .expect("Series creation should succeed");
    /// assert_eq!(series.id().as_str(), "TREASURY");
    /// ```
    pub fn new(
        id: impl AsRef<str>,
        observations: Vec<(Date, f64)>,
        currency: Option<Currency>,
        interpolation: SeriesInterpolation,
        extrapolation: SeriesExtrapolation,
    ) -> Result<Self> {
        let raw_id = id.as_ref();
        if raw_id.trim().is_empty() || raw_id.trim() != raw_id {
            return Err(crate::Error::Validation(
                "time series identifier must be non-empty and trimmed".into(),
            ));
        }

        let mut observations_i32 = Vec::with_capacity(observations.len());
        for (d, v) in observations {
            observations_i32.push((to_days_checked(d)?, v));
        }

        let data = TimeSeriesStorage::new(observations_i32)?;

        Ok(Self {
            id: CurveId::from(raw_id),
            currency,
            data,
            interpolation,
            extrapolation,
        })
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

    /// Current right-tail extrapolation policy accessor.
    pub fn extrapolation(&self) -> SeriesExtrapolation {
        self.extrapolation
    }

    /// Retrieve the value on a given date according to the active interpolation
    /// and right-tail extrapolation policies.
    ///
    /// Queries on an empty series or strictly prior to the earliest observation
    /// always return an error.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::market_data::scalars::{
    ///     ScalarTimeSeries, SeriesExtrapolation, SeriesInterpolation,
    /// };
    /// use finstack_quant_core::dates::Date;
    /// use time::Month;
    ///
    /// let obs = vec![
    ///     (Date::from_calendar_date(2024, Month::January, 1).expect("Valid date"), 10.0),
    ///     (Date::from_calendar_date(2024, Month::February, 1).expect("Valid date"), 20.0),
    /// ];
    /// let step = ScalarTimeSeries::new(
    ///     "TS",
    ///     obs.clone(),
    ///     None,
    ///     SeriesInterpolation::Step,
    ///     SeriesExtrapolation::Error,
    /// )
    /// .expect("Series creation should succeed");
    /// let jan = Date::from_calendar_date(2024, Month::January, 15).expect("Valid date");
    /// assert_eq!(step.get_value(jan).expect("Value lookup should succeed"), 10.0);
    ///
    /// let linear = ScalarTimeSeries::new(
    ///     "TS",
    ///     obs,
    ///     None,
    ///     SeriesInterpolation::Linear,
    ///     SeriesExtrapolation::Error,
    /// )
    /// .expect("Series creation should succeed");
    /// assert!(linear.get_value(jan).expect("Value lookup should succeed") > 10.0);
    /// ```
    ///
    /// # Arguments
    ///
    /// * `date` - Calendar date in the instrument or market convention for the operation.
    pub fn get_value(&self, date: Date) -> Result<f64> {
        self.value_at_day(to_days_checked(date)?)
    }

    /// Resolve a single query day under this series' interpolation and extrapolation policies.
    ///
    /// This is the allocation-free single-point path.
    fn value_at_day(&self, query_day: i32) -> Result<f64> {
        let date_vec = self.data.dates();
        let value_vec = self.data.values();
        if date_vec.is_empty() {
            return Err(crate::Error::Input(InputError::NotFound {
                id: format!("series '{}': no observations", self.id.as_str()),
            }));
        }
        match date_vec.binary_search(&query_day) {
            Ok(idx) => Ok(value_vec[idx]),
            Err(idx) => {
                if idx == 0 {
                    Err(self.no_prior_observation(query_day))
                } else if idx >= date_vec.len() {
                    match self.extrapolation {
                        SeriesExtrapolation::Flat => Ok(value_vec[date_vec.len() - 1]),
                        SeriesExtrapolation::Error => {
                            Err(crate::Error::Input(InputError::NotFound {
                                id: format!(
                                    "series '{}': date {} is after latest observation {}",
                                    self.id.as_str(),
                                    from_days(query_day),
                                    from_days(date_vec[date_vec.len() - 1]),
                                ),
                            }))
                        }
                    }
                } else {
                    match self.interpolation {
                        SeriesInterpolation::Step => Ok(value_vec[idx - 1]),
                        SeriesInterpolation::Linear => {
                            let d0 = i64::from(date_vec[idx - 1]);
                            let d1 = i64::from(date_vec[idx]);
                            let dq = i64::from(query_day);
                            let span = d1.checked_sub(d0).ok_or(InputError::InvalidDateRange)?;
                            let offset = dq.checked_sub(d0).ok_or(InputError::InvalidDateRange)?;
                            let weight = (offset as f64) / (span as f64);
                            let y0 = value_vec[idx - 1];
                            let y1 = value_vec[idx];
                            let interpolated = if y0.to_bits() == y1.to_bits() {
                                y0
                            } else {
                                let v = (1.0 - weight) * y0 + weight * y1;
                                if v == 0.0 { 0.0 } else { v }
                            };
                            if !interpolated.is_finite() {
                                return Err(crate::Error::Validation(format!(
                                    "series '{}': linear interpolation overflow on {}",
                                    self.id.as_str(),
                                    from_days(query_day),
                                )));
                            }
                            Ok(interpolated)
                        }
                    }
                }
            }
        }
    }

    /// Error for a lookup before the first observation of the series.
    fn no_prior_observation(&self, query_day: i32) -> crate::Error {
        crate::Error::Input(InputError::NotFound {
            id: format!(
                "series '{}': no observation on or before {}",
                self.id.as_str(),
                from_days(query_day),
            ),
        })
    }

    /// Retrieve the value for an **exactly observed** date.
    ///
    /// Unlike [`Self::get_value`], this does **not** interpolate or carry values forward.
    /// It is intended for market fixings where missing observations must be treated
    /// as an error (e.g., RFR overnight fixings).
    ///
    /// # Arguments
    ///
    /// * `date` - Calendar date in the instrument or market convention for the operation.
    pub fn get_value_exact(&self, date: Date) -> Result<f64> {
        let q = to_days_checked(date)?;
        let date_vec = self.data.dates();
        let value_vec = self.data.values();
        match date_vec.binary_search(&q) {
            Ok(idx) => Ok(value_vec[idx]),
            Err(_) => Err(crate::Error::Input(InputError::NotFound {
                id: format!("series '{}' observation on {}", self.id.as_str(), date),
            })),
        }
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

    /// Get the earliest observation date, or `None` when the series is empty.
    ///
    /// O(1). Prefer this over `observations().first()`, which materializes the
    /// entire series just to read one element.
    #[inline]
    pub fn first_date(&self) -> Option<Date> {
        self.data.dates().first().copied().map(from_days)
    }

    /// Get the latest observation date, or `None` when the series is empty.
    ///
    /// O(1). Prefer this over `observations().last()`, which materializes the
    /// entire series just to read one element.
    #[inline]
    pub fn last_date(&self) -> Option<Date> {
        self.data.dates().last().copied().map(from_days)
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

/// The 1970-01-01 epoch these series index from.
///
/// Built once rather than reconstructed per call: `to_days_checked`/`from_days` sit on
/// the per-fixing lookup path and `from_days` is also called once per element by
/// [`ScalarTimeSeries::observations`].
const EPOCH: Date = time::macros::date!(1970 - 01 - 01);

fn to_days_checked(date: Date) -> Result<i32> {
    i32::try_from((date - EPOCH).whole_days())
        .map_err(|_| crate::Error::Input(InputError::InvalidDateRange))
}

fn from_days(days: i32) -> Date {
    EPOCH + time::Duration::days(i64::from(days))
}

/// Raw serializable state of a ScalarTimeSeries
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct ScalarTimeSeriesWire {
    /// Series identifier
    pub id: String,
    /// Optional currency
    pub currency: Option<Currency>,
    /// Observations as (date, value) pairs
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<(crate::wire::DateWire, f64)>")
    )]
    pub observations: Vec<(Date, f64)>,
    /// Interpolation method
    pub interpolation: SeriesInterpolation,
    /// Right-tail extrapolation policy
    pub extrapolation: SeriesExtrapolation,
}

impl From<ScalarTimeSeries> for ScalarTimeSeriesWire {
    fn from(series: ScalarTimeSeries) -> Self {
        let observations = series.observations();

        ScalarTimeSeriesWire {
            id: series.id.to_string(),
            currency: series.currency,
            observations,
            interpolation: series.interpolation,
            extrapolation: series.extrapolation,
        }
    }
}

impl TryFrom<ScalarTimeSeriesWire> for ScalarTimeSeries {
    type Error = crate::Error;

    fn try_from(state: ScalarTimeSeriesWire) -> Result<Self> {
        Self::new(
            state.id,
            state.observations,
            state.currency,
            state.interpolation,
            state.extrapolation,
        )
    }
}

impl Bumpable for MarketScalar {
    /// Apply a scalar shock with checked monetary arithmetic and finite results.
    fn apply_bump(&self, spec: BumpSpec) -> crate::Result<Self> {
        if !spec.value.is_finite() {
            return Err(crate::Error::Validation(
                "market scalar bump value must be finite".into(),
            ));
        }
        spec.validate_parallel("MarketScalar")?;

        match &self.0 {
            MarketScalarKind::Unitless(v) => {
                let new_val = match (spec.mode, spec.units) {
                    (BumpMode::Additive, BumpUnits::Fraction) => v + spec.value,
                    (BumpMode::Multiplicative, BumpUnits::Factor) => {
                        let scaled = v * spec.value;
                        if *v != 0.0 && spec.value != 0.0 && scaled == 0.0 {
                            return Err(crate::Error::Input(InputError::DecimalUnderflow));
                        }
                        scaled
                    }
                    _ => {
                        return Err(crate::InputError::UnsupportedBump {
                            reason: format!(
                                "MarketScalar only supports Additive/Fraction or Multiplicative/Factor, got {:?}/{:?}",
                                spec.mode, spec.units
                            ),
                        }
                        .into());
                    }
                };
                Self::unitless(new_val)
            }
            MarketScalarKind::Price(m) => match (spec.mode, spec.units) {
                (BumpMode::Additive, BumpUnits::Fraction) => {
                    let bump = crate::money::Money::new(spec.value, m.currency())?;
                    m.checked_add(bump).map(Self::price)
                }
                (BumpMode::Multiplicative, BumpUnits::Factor) => {
                    m.checked_mul_f64(spec.value).map(Self::price)
                }
                _ => Err(crate::InputError::UnsupportedBump {
                    reason: format!(
                        "MarketScalar only supports Additive/Fraction or Multiplicative/Factor, got {:?}/{:?}",
                        spec.mode, spec.units
                    ),
                }
                .into()),
            },
        }
    }
}

impl Bumpable for ScalarTimeSeries {
    fn apply_bump(&self, spec: BumpSpec) -> crate::Result<Self> {
        if self
            .id
            .as_str()
            .starts_with(crate::market_data::fixings::FIXING_PREFIX)
        {
            return Err(crate::InputError::UnsupportedBump {
                reason: format!(
                    "ScalarTimeSeries '{}': historical fixing series cannot be bumped",
                    self.id.as_str()
                ),
            }
            .into());
        }
        if !spec.value.is_finite() {
            return Err(crate::Error::Validation(
                "time series bump value must be finite".into(),
            ));
        }
        spec.validate_parallel("ScalarTimeSeries")?;

        let is_multiplicative = match (spec.mode, spec.units) {
            (BumpMode::Additive, BumpUnits::Fraction) => false,
            (BumpMode::Multiplicative, BumpUnits::Factor) => true,
            _ => {
                return Err(crate::InputError::UnsupportedBump {
                    reason: format!(
                        "ScalarTimeSeries only supports Additive/Fraction or Multiplicative/Factor, got {:?}/{:?}",
                        spec.mode, spec.units
                    ),
                }
                .into());
            }
        };

        let observations = self.observations();
        let mut bumped_obs = Vec::with_capacity(observations.len());
        for (d, v) in observations {
            let bumped = if is_multiplicative {
                let scaled = v * spec.value;
                if v != 0.0 && spec.value != 0.0 && scaled == 0.0 {
                    return Err(crate::Error::Input(InputError::DecimalUnderflow));
                }
                scaled
            } else {
                v + spec.value
            };
            if !bumped.is_finite() {
                return Err(crate::Error::Validation(
                    "time series bump must produce finite values".into(),
                ));
            }
            bumped_obs.push((d, if bumped == 0.0 { 0.0 } else { bumped }));
        }

        ScalarTimeSeries::new(
            self.id().as_str(),
            bumped_obs,
            self.currency(),
            self.interpolation(),
            self.extrapolation(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_rejects_unsorted_and_duplicate_dates() {
        assert!(matches!(
            TimeSeriesStorage::new(vec![(2, 20.0), (0, 10.0), (1, 15.0)]),
            Err(crate::Error::Input(InputError::NonMonotonicKnots))
        ));
        assert!(matches!(
            TimeSeriesStorage::new(vec![(0, 10.0), (1, 15.0), (1, 20.0)]),
            Err(crate::Error::Input(InputError::NonMonotonicKnots))
        ));
    }

    #[test]
    fn unitless_constructor_rejects_non_finite_and_normalizes_negative_zero() {
        assert_eq!(
            MarketScalar::unitless(0.1).expect("finite").get_unitless(),
            Some(0.1)
        );
        let neg_zero = MarketScalar::unitless(-0.0).expect("negative zero is finite");
        let val = neg_zero.get_unitless().expect("unitless");
        assert!(val.is_sign_positive() && val == 0.0);
        assert_eq!(neg_zero, MarketScalar::unitless(0.0).expect("zero"));
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(matches!(
                MarketScalar::unitless(bad),
                Err(crate::Error::Validation(_))
            ));
        }
    }

    #[test]
    fn market_scalar_shadow_rejects_non_finite_values() {
        assert!(MarketScalar::try_from(MarketScalarWire::Unitless(f64::NAN)).is_err());
        assert!(MarketScalar::try_from(MarketScalarWire::Unitless(f64::INFINITY)).is_err());
        assert!(crate::money::Money::new(f64::NEG_INFINITY, Currency::USD).is_err());
        let deserialized: MarketScalar =
            serde_json::from_str(r#"{"unitless":-0.0}"#).expect("valid wire scalar");
        let val = deserialized.get_unitless().expect("unitless");
        assert!(val.is_sign_positive());
    }

    #[test]
    fn scalar_bumps_report_overflow_underflow_and_reject_unsupported_units() {
        let money = MarketScalar::price(
            crate::money::Money::from_decimal_str("10000000000000000000000000000", Currency::USD)
                .expect("representable money"),
        );
        assert!(money.apply_bump(BumpSpec::multiplier(10.0)).is_err());
        assert!(
            money
                .apply_bump(BumpSpec::inflation_shift_pct(10.0))
                .is_err()
        );
        assert!(money.apply_bump(BumpSpec::parallel_bp(10.0)).is_err());

        for spec in [
            BumpSpec::multiplier(10.0),
            BumpSpec {
                value: f64::MAX,
                units: BumpUnits::Fraction,
                ..BumpSpec::inflation_shift_pct(900.0)
            },
        ] {
            assert!(
                MarketScalar::unitless(f64::MAX)
                    .expect("finite")
                    .apply_bump(spec)
                    .is_err()
            );
        }
        assert!(
            MarketScalar::unitless(1.0)
                .expect("finite")
                .apply_bump(BumpSpec::multiplier(f64::NAN))
                .is_err()
        );
        // Nonzero underflow to zero must error
        assert!(matches!(
            MarketScalar::unitless(1e-300)
                .expect("finite")
                .apply_bump(BumpSpec::multiplier(1e-30)),
            Err(crate::Error::Input(InputError::DecimalUnderflow))
        ));
        assert!(matches!(
            MarketScalar::unitless(1.0)
                .expect("finite")
                .apply_bump(BumpSpec::inflation_shift_pct(10.0)),
            Err(crate::Error::Input(InputError::UnsupportedBump { .. }))
        ));
    }

    #[test]
    fn scalar_bumps_reject_bucket_coordinates_without_a_time_axis() {
        assert!(
            MarketScalar::unitless(1.0)
                .expect("finite")
                .apply_bump(BumpSpec::triangular_key_rate_bp(0.0, 1.0, 2.0, 1.0))
                .is_err()
        );
    }

    #[test]
    fn fixing_series_rejects_even_zero_or_identity_bumps() {
        let d0 =
            time::Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date");
        let empty_fixing = ScalarTimeSeries::new(
            "FIXING:USD-SOFR",
            vec![],
            None,
            SeriesInterpolation::Step,
            SeriesExtrapolation::Error,
        )
        .expect("empty fixing series");
        assert!(empty_fixing.apply_bump(BumpSpec::multiplier(1.0)).is_err());
        assert!(
            empty_fixing
                .apply_bump(BumpSpec {
                    value: 0.0,
                    units: BumpUnits::Fraction,
                    ..BumpSpec::multiplier(1.0)
                })
                .is_err()
        );

        let fixing = ScalarTimeSeries::new(
            "FIXING:USD-SOFR",
            vec![(d0, 0.05)],
            None,
            SeriesInterpolation::Step,
            SeriesExtrapolation::Error,
        )
        .expect("fixing series");
        assert!(fixing.apply_bump(BumpSpec::multiplier(1.0)).is_err());
    }

    #[test]
    fn series_step_and_linear() {
        let d0 =
            time::Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date");
        let d1 = time::Date::from_calendar_date(2025, time::Month::February, 1)
            .expect("Valid test date");
        let d2 =
            time::Date::from_calendar_date(2025, time::Month::March, 1).expect("Valid test date");
        let obs = vec![(d0, 3.0), (d1, 4.0), (d2, 5.0)];
        let mid = d0 + TimeDuration::days(15);

        let step_s = ScalarTimeSeries::new(
            "US-UNEMP",
            obs.clone(),
            None,
            SeriesInterpolation::Step,
            SeriesExtrapolation::Error,
        )
        .expect("ScalarTimeSeries creation should succeed in test");
        let step_v = step_s
            .get_value(mid)
            .expect("Value lookup should succeed in test");
        assert!((step_v - 3.0).abs() < 1e-12);

        let lin_s = ScalarTimeSeries::new(
            "US-UNEMP",
            obs,
            None,
            SeriesInterpolation::Linear,
            SeriesExtrapolation::Error,
        )
        .expect("ScalarTimeSeries creation should succeed in test");
        let lin_v = lin_s
            .get_value(mid)
            .expect("Value lookup should succeed in test");
        assert!(lin_v > 3.0 && lin_v < 4.0);
    }

    #[test]
    fn series_right_tail_extrapolation_and_empty_or_prefirst_errors() {
        let d0 = time::Date::from_calendar_date(2025, time::Month::January, 1).expect("date");
        let d1 = time::Date::from_calendar_date(2025, time::Month::February, 1).expect("date");
        let before = time::Date::from_calendar_date(2024, time::Month::December, 31).expect("date");
        let after = time::Date::from_calendar_date(2025, time::Month::March, 1).expect("date");

        let flat = ScalarTimeSeries::new(
            "TS",
            vec![(d0, 10.0), (d1, 20.0)],
            None,
            SeriesInterpolation::Linear,
            SeriesExtrapolation::Flat,
        )
        .expect("series");
        assert!(flat.get_value(before).is_err());
        assert_eq!(flat.get_value(after).expect("flat extrapolation"), 20.0);

        let strict = ScalarTimeSeries::new(
            "TS",
            vec![(d0, 10.0), (d1, 20.0)],
            None,
            SeriesInterpolation::Step,
            SeriesExtrapolation::Error,
        )
        .expect("series");
        assert!(strict.get_value(before).is_err());
        assert!(strict.get_value(after).is_err());

        let empty = ScalarTimeSeries::new(
            "EMPTY",
            vec![],
            None,
            SeriesInterpolation::Step,
            SeriesExtrapolation::Flat,
        )
        .expect("empty series is admitted");
        assert!(empty.is_empty());
        assert!(empty.get_value(d0).is_err());
        assert!(empty.get_value_exact(d0).is_err());
    }

    #[test]
    fn scalar_time_series_rejects_unknown_missing_and_invalid_ids() {
        let extra = r#"{
            "id":"TS",
            "currency":null,
            "observations":[["2025-01-01",10.0],["2025-02-01",20.0]],
            "interpolation":"step",
            "extrapolation":"error",
            "extra":true
        }"#;
        assert!(serde_json::from_str::<ScalarTimeSeries>(extra).is_err());

        let missing_extrapolation = r#"{
            "id":"TS",
            "currency":null,
            "observations":[["2025-01-01",10.0]],
            "interpolation":"step"
        }"#;
        assert!(serde_json::from_str::<ScalarTimeSeries>(missing_extrapolation).is_err());

        for bad_id in ["", "   ", " TS", "TS "] {
            assert!(
                ScalarTimeSeries::new(
                    bad_id,
                    vec![],
                    None,
                    SeriesInterpolation::Step,
                    SeriesExtrapolation::Error,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn test_scalar_time_series_single_point_accepted() {
        let single_obs = vec![(
            time::Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date"),
            -0.0,
        )];

        let series = ScalarTimeSeries::new(
            "TEST",
            single_obs,
            None,
            SeriesInterpolation::Step,
            SeriesExtrapolation::Error,
        )
        .expect("single-point series should be accepted");
        assert_eq!(series.observations().len(), 1);
        assert!(series.observations()[0].1.is_sign_positive());
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

        let result = ScalarTimeSeries::new(
            "TEST",
            observations,
            None,
            SeriesInterpolation::Linear,
            SeriesExtrapolation::Flat,
        );
        assert!(result.is_ok());

        let series = result.expect("ScalarTimeSeries creation should succeed in test");
        assert_eq!(series.observations().len(), 3);
        assert_eq!(series.id.as_str(), "TEST");
        assert_eq!(series.interpolation(), SeriesInterpolation::Linear);
        assert_eq!(series.extrapolation(), SeriesExtrapolation::Flat);
    }

    #[test]
    fn test_scalar_time_series_large_dataset() {
        let mut observations = Vec::new();

        for i in 0..30 {
            let day = 1 + i as u8;
            if day <= 31 {
                let date = time::Date::from_calendar_date(2025, time::Month::January, day)
                    .expect("Valid test date");
                observations.push((date, i as f64));
            }
        }

        let result = ScalarTimeSeries::new(
            "TEST",
            observations,
            None,
            SeriesInterpolation::Step,
            SeriesExtrapolation::Error,
        );
        assert!(result.is_ok());

        let series = result.expect("ScalarTimeSeries creation should succeed in test");
        assert!(series.observations().len() >= 25);
    }

    #[test]
    fn series_interpolation_and_extrapolation_roundtrip() {
        use std::str::FromStr;
        for v in [SeriesInterpolation::Step, SeriesInterpolation::Linear] {
            let s = v.to_string();
            let parsed = SeriesInterpolation::from_str(&s).expect("roundtrip parse should succeed");
            assert_eq!(v, parsed, "roundtrip failed for {s}");
        }
        assert!(SeriesInterpolation::from_str("invalid").is_err());

        for v in [SeriesExtrapolation::Flat, SeriesExtrapolation::Error] {
            let s = v.to_string();
            let parsed = SeriesExtrapolation::from_str(&s).expect("roundtrip parse should succeed");
            assert_eq!(v, parsed, "roundtrip failed for {s}");
        }
        assert!(SeriesExtrapolation::from_str("invalid").is_err());
    }
}
