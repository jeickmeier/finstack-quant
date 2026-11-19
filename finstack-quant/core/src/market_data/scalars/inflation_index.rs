//! Observed monthly CPI/RPI fixing snapshots and contractual observation lags.
//!
//! Implements the [`CORE-MARKET-11`] separation between:
//! - Immutable, canonical first-of-month fixing snapshots ([`InflationIndex`])
//!   at an explicit snapshot cutoff `knowledge_date`, carrying per-observation
//!   `available_on` dates and distinct [`InflationExpectedRelease`] facts.
//! - Contractual date-shift specifications ([`InflationLag`]), consumed by
//!   instruments and shared reference-index resolution rather than baked into
//!   the observed history.
//!
//! # References
//!
//! - **TIPS / Index-Linked Bond Conventions**:
//!   - Deacon, M., Derry, A., & Mirfendereski, D. (2004). *Inflation-Indexed Securities*
//!     (2nd ed.). Wiley Finance. `docs/REFERENCES.md#deacon-derry-mirfendereski-2004`
//!   - Kerkhof, J. (2005). "Inflation Derivatives Explained." *Journal of Derivatives
//!     Accounting*, 2(1), 1-19. `docs/REFERENCES.md#kerkhof-2005`

use crate::currency::Currency;
use crate::dates::Date;
use crate::types::CurveId;
use crate::{Error, InputError, Result};

use serde::{Deserialize, Serialize};
use std::num::{NonZeroU8, NonZeroU16};

/// Publication identity and units of an observed or projected inflation level.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct InflationSeries {
    /// Publisher-qualified identifier of the statistical series.
    pub series_id: String,
    /// Published reference period and index scale, for example `1982-84=100`.
    pub reference_base: String,
    /// True for seasonally adjusted levels and false for unadjusted levels.
    pub seasonally_adjusted: bool,
}

impl InflationSeries {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.series_id.trim().is_empty() || self.reference_base.trim().is_empty() {
            return Err(Error::Validation(
                "inflation series identity and reference base must be nonempty".into(),
            ));
        }
        Ok(())
    }
}

/// Contractual observation lag for inflation index reference dates.
///
/// Specifies how far before a contractual reference date the historical or
/// projected CPI anchor is sampled. Publication and knowledge cutoffs are
/// tracked separately on [`InflationIndex`].
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::market_data::scalars::InflationLag;
///
/// let tips_lag = InflationLag::months(3);
/// let day_lag = InflationLag::days(90);
/// let no_lag = InflationLag::None;
/// assert_eq!(tips_lag.to_string(), "3M");
/// assert_eq!(day_lag.to_string(), "90D");
/// assert_eq!(no_lag.to_string(), "none");
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum InflationLag {
    /// Lag by a strictly positive number of whole calendar months.
    Months(NonZeroU8),

    /// Lag by a strictly positive number of literal calendar days.
    Days(NonZeroU16),

    /// Zero contractual shift.
    #[default]
    None,
}

impl InflationLag {
    /// Construct a month lag (`0` normalizes to [`InflationLag::None`]).
    ///
    /// # Arguments
    ///
    /// * `months` - Whole-month lag count (`0` produces [`Self::None`]).
    #[inline]
    #[must_use]
    pub fn months(months: u8) -> Self {
        NonZeroU8::new(months).map_or(Self::None, Self::Months)
    }

    /// Construct a calendar-day lag (`0` normalizes to [`InflationLag::None`]).
    ///
    /// # Arguments
    ///
    /// * `days` - Literal calendar-day lag count (`0` produces [`Self::None`]).
    #[inline]
    #[must_use]
    pub fn days(days: u16) -> Self {
        NonZeroU16::new(days).map_or(Self::None, Self::Days)
    }
}

impl From<u8> for InflationLag {
    #[inline]
    fn from(months: u8) -> Self {
        Self::months(months)
    }
}

impl std::fmt::Display for InflationLag {
    /// Render in the same form accepted by [`FromStr`](std::str::FromStr):
    /// `"none"`, `"<n>M"`, or `"<n>D"`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Months(n) => write!(f, "{}M", n.get()),
            Self::Days(n) => write!(f, "{}D", n.get()),
            Self::None => write!(f, "none"),
        }
    }
}

impl std::str::FromStr for InflationLag {
    type Err = Error;

    /// Parse a market-convention observation lag string.
    ///
    /// Accepted forms (case-insensitive, surrounding whitespace trimmed):
    /// - `"NONE"`, `"0"`, `"0M"`, `"00M"`, `"0D"`, `"00D"` → [`InflationLag::None`]
    /// - `"<n>M"` (positive `u8`) → [`InflationLag::Months`]
    /// - `"<n>D"` (positive `u16`) → [`InflationLag::Days`]
    ///
    /// # Arguments
    ///
    /// * `s` - Observation lag label such as `"3M"`, `"90D"`, or `"none"`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `s` cannot be parsed or overflows `u8`/`u16`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::market_data::scalars::InflationLag;
    ///
    /// let tips: InflationLag = "3M".parse().expect("valid lag");
    /// assert_eq!(tips, InflationLag::months(3));
    /// let zero_m: InflationLag = "00M".parse().expect("valid zero lag");
    /// assert_eq!(zero_m, InflationLag::None);
    /// ```
    fn from_str(s: &str) -> Result<Self> {
        let upper = s.trim().to_ascii_uppercase();
        if upper == "NONE" || upper == "0" {
            return Ok(Self::None);
        }
        if let Some(num) = upper.strip_suffix('M') {
            let months: u8 = num.trim().parse().map_err(|_| {
                Error::Validation(format!("Invalid inflation lag '{s}': expected like '3M'"))
            })?;
            return Ok(Self::months(months));
        }
        if let Some(num) = upper.strip_suffix('D') {
            let days: u16 = num.trim().parse().map_err(|_| {
                Error::Validation(format!("Invalid inflation lag '{s}': expected like '90D'"))
            })?;
            return Ok(Self::days(days));
        }
        Err(Error::Validation(format!(
            "Invalid inflation lag '{s}': expected like '3M' or '90D'"
        )))
    }
}

/// One published monthly CPI/RPI fixing record inside an [`InflationIndex`] snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct InflationObservation {
    /// Canonical first calendar day of the reference inflation month (`day() == 1`).
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pub reference_month: Date,
    /// Published finite, strictly positive index level.
    pub value: f64,
    /// Inclusive end-of-day availability date for this selected observation version.
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pub available_on: Date,
}

/// One expected-release schedule assumption for a monthly CPI/RPI reference period.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct InflationExpectedRelease {
    /// Canonical first calendar day of the reference inflation month (`day() == 1`).
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pub reference_month: Date,
    /// Scheduled or expected release date on or after `reference_month`.
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pub expected_release: Date,
    /// Inclusive date on or before the snapshot `knowledge_date` when this release assumption was known.
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pub known_on: Date,
}

/// Immutable monthly CPI/RPI observation snapshot at an explicit knowledge date `K`.
///
/// Stores strictly ordered, unique first-of-month [`InflationObservation`] records
/// whose `available_on` satisfies `reference_month <= available_on <= knowledge_date`,
/// together with distinct [`InflationExpectedRelease`] assumptions (`known_on <= knowledge_date`).
/// Empty histories are valid (allowing projection-only markets with release metadata).
///
/// Lookups ([`get_observation`](Self::get_observation) and
/// [`qualified_monthly_observation`](Self::qualified_monthly_observation)) are exact
/// by canonical first-of-month key and perform no lag, interpolation, carry, or
/// seasonal adjustment.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::market_data::scalars::{InflationIndex, InflationObservation};
/// use time::macros::date;
///
/// let obs = vec![
///     InflationObservation {
///         reference_month: date!(2024 - 01 - 01),
///         value: 300.0,
///         available_on: date!(2024 - 02 - 13),
///     },
///     InflationObservation {
///         reference_month: date!(2024 - 02 - 01),
///         value: 302.0,
///         available_on: date!(2024 - 03 - 12),
///     },
/// ];
/// let index = InflationIndex::new("US-CPI-U", obs, Currency::USD, date!(2024 - 04 - 01))
///     .expect("valid fixing snapshot");
/// assert_eq!(
///     index
///         .get_observation(date!(2024 - 02 - 01))
///         .expect("observation exists")
///         .value,
///     302.0
/// );
/// ```
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "InflationIndexWire", into = "InflationIndexWire")]
#[cfg_attr(feature = "json-schema", schemars(try_from = "InflationIndexWire"))]
pub struct InflationIndex {
    series: Option<InflationSeries>,
    id: CurveId,
    currency: Currency,
    knowledge_date: Date,
    observations: Vec<InflationObservation>,
    expected_releases: Vec<InflationExpectedRelease>,
}

fn validate_canonical_month(context: &str, id: &str, reference_month: Date) -> Result<()> {
    if reference_month.day() != 1 {
        return Err(Error::Validation(format!(
            "inflation index '{id}' {context} must use canonical first-of-month date, got {reference_month}"
        )));
    }
    Ok(())
}

impl InflationIndex {
    /// Construct an immutable monthly CPI fixing snapshot at `knowledge_date`.
    ///
    /// # Arguments
    ///
    /// * `id` - Stable non-blank series identifier (for example `"US-CPI-U"`).
    /// * `observations` - Strictly ascending first-of-month [`InflationObservation`]
    ///   records (`day() == 1`), each with finite strictly positive `value` and
    ///   `reference_month <= available_on <= knowledge_date`. Empty slices are accepted.
    /// * `currency` - Currency association of the index series.
    /// * `knowledge_date` - Inclusive end-of-day snapshot cutoff date `K`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when `id` is blank, any observation month
    /// does not start on day `1`, months are unsorted or duplicated, any value is
    /// non-finite or `<= 0.0`, or `available_on` lies outside `[reference_month, knowledge_date]`.
    pub fn new(
        id: impl Into<CurveId>,
        observations: Vec<InflationObservation>,
        currency: Currency,
        knowledge_date: Date,
    ) -> Result<Self> {
        let id = id.into();
        let id_str = id.as_str();
        if id_str.trim().is_empty() {
            return Err(Error::Validation(
                "inflation index id cannot be blank".into(),
            ));
        }
        let mut prev_month: Option<Date> = None;
        for obs in &observations {
            validate_canonical_month("observation", id_str, obs.reference_month)?;
            if let Some(prev) = prev_month
                && obs.reference_month <= prev
            {
                return Err(Error::Validation(format!(
                    "inflation index '{id_str}' observations must be strictly increasing by reference_month ({} after {prev})",
                    obs.reference_month
                )));
            }
            prev_month = Some(obs.reference_month);
            if !obs.value.is_finite() || obs.value <= 0.0 {
                return Err(Error::Validation(format!(
                    "inflation index '{id_str}' observation for {} must be finite and strictly positive, got {}",
                    obs.reference_month, obs.value
                )));
            }
            if obs.available_on < obs.reference_month || obs.available_on > knowledge_date {
                return Err(Error::Validation(format!(
                    "inflation index '{id_str}' observation for {} has available_on={} outside [{}, {}]",
                    obs.reference_month, obs.available_on, obs.reference_month, knowledge_date
                )));
            }
        }
        Ok(Self {
            id,
            currency,
            knowledge_date,
            observations,
            series: None,
            expected_releases: Vec::new(),
        })
    }

    /// Attach the publisher, reference-base and seasonal identity of these fixings.
    ///
    /// # Arguments
    ///
    /// * `series` - Publication metadata for the levels as supplied, including their reference scale.
    pub fn with_series(mut self, series: InflationSeries) -> Result<Self> {
        series.validate()?;
        self.series = Some(series);
        Ok(self)
    }

    /// Return publication metadata used to qualify comparisons between snapshots.
    pub fn get_series(&self) -> Option<&InflationSeries> {
        self.series.as_ref()
    }

    /// Attach strictly ordered expected-release schedule assumptions to this snapshot.
    ///
    /// # Arguments
    ///
    /// * `expected_releases` - Strictly ascending first-of-month
    ///   [`InflationExpectedRelease`] entries, each with
    ///   `expected_release >= reference_month` and `known_on <= self.knowledge_date()`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if any `reference_month` is not day `1`,
    /// entries are unsorted or duplicated, `expected_release < reference_month`,
    /// or `known_on > self.knowledge_date()`.
    pub fn with_expected_releases(
        mut self,
        expected_releases: Vec<InflationExpectedRelease>,
    ) -> Result<Self> {
        let id_str = self.id.as_str();
        let mut prev_month: Option<Date> = None;
        for rel in &expected_releases {
            validate_canonical_month("expected release", id_str, rel.reference_month)?;
            if let Some(prev) = prev_month
                && rel.reference_month <= prev
            {
                return Err(Error::Validation(format!(
                    "inflation index '{id_str}' expected_releases must be strictly increasing by reference_month ({} after {prev})",
                    rel.reference_month
                )));
            }
            prev_month = Some(rel.reference_month);
            if rel.expected_release < rel.reference_month {
                return Err(Error::Validation(format!(
                    "inflation index '{id_str}' expected_release {} precedes reference_month {}",
                    rel.expected_release, rel.reference_month
                )));
            }
            if rel.known_on > self.knowledge_date {
                return Err(Error::Validation(format!(
                    "inflation index '{id_str}' expected release assumption known_on={} exceeds snapshot knowledge_date={}",
                    rel.known_on, self.knowledge_date
                )));
            }
        }
        self.expected_releases = expected_releases;
        Ok(self)
    }

    /// Unique identifier of this inflation index snapshot.
    #[inline]
    pub fn id(&self) -> &str {
        self.id.as_str()
    }

    /// Currency association of the index series.
    #[inline]
    pub const fn currency(&self) -> Currency {
        self.currency
    }

    /// Snapshot knowledge cutoff date `K`.
    #[inline]
    pub const fn knowledge_date(&self) -> Date {
        self.knowledge_date
    }

    /// Number of stored monthly fixing observations.
    #[inline]
    pub fn len(&self) -> usize {
        self.observations.len()
    }

    /// Returns `true` if no historical observations are stored in this snapshot.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.observations.is_empty()
    }

    /// Inclusive `(first, last)` canonical reference months stored, or `None` when empty.
    #[must_use]
    pub fn date_range(&self) -> Option<(Date, Date)> {
        let first = self.observations.first()?.reference_month;
        let last = self.observations.last()?.reference_month;
        Some((first, last))
    }

    /// Chronological slice of all stored [`InflationObservation`] records.
    #[inline]
    pub fn observations(&self) -> &[InflationObservation] {
        &self.observations
    }

    /// Chronological slice of configured [`InflationExpectedRelease`] facts.
    #[inline]
    pub fn get_expected_releases(&self) -> &[InflationExpectedRelease] {
        &self.expected_releases
    }

    /// Look up the exact stored observation record for canonical `reference_month` (`day() == 1`).
    ///
    /// # Arguments
    ///
    /// * `reference_month` - First calendar day of the requested reference month (`day() == 1`).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `reference_month.day() != 1`, or
    /// [`InputError::NotFound`] if the month has no observation in this snapshot.
    pub fn get_observation(&self, reference_month: Date) -> Result<InflationObservation> {
        validate_canonical_month("get_observation", self.id(), reference_month)?;
        self.observations
            .binary_search_by_key(&reference_month, |obs| obs.reference_month)
            .map(|idx| self.observations[idx])
            .map_err(|_| {
                InputError::NotFound {
                    id: format!(
                        "inflation index '{}' observation for {}",
                        self.id(),
                        reference_month
                    ),
                }
                .into()
            })
    }

    /// Look up the expected release assumption for canonical `reference_month` (`day() == 1`).
    ///
    /// # Arguments
    ///
    /// * `reference_month` - First calendar day of the reference month (`day() == 1`).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `reference_month.day() != 1`.
    pub fn get_expected_release(
        &self,
        reference_month: Date,
    ) -> Result<Option<InflationExpectedRelease>> {
        validate_canonical_month("get_expected_release", self.id(), reference_month)?;
        Ok(self
            .expected_releases
            .binary_search_by_key(&reference_month, |rel| rel.reference_month)
            .ok()
            .map(|idx| self.expected_releases[idx]))
    }

    /// Resolve the monthly CPI observation for `(year, month)` eligible on `knowledge_date`.
    ///
    /// # Arguments
    ///
    /// * `year` - Calendar year of the reference month.
    /// * `month` - Calendar month of the reference print.
    /// * `knowledge_date` - Query cutoff date (must satisfy `knowledge_date <= self.knowledge_date()`).
    ///
    /// # Errors
    ///
    /// Returns [`InputError::InvalidDateRange`] for an invalid calendar date,
    /// [`InputError::NotFound`] if `reference_month` is absent, or
    /// [`Error::Validation`] if `knowledge_date > self.knowledge_date()` or
    /// `obs.available_on > knowledge_date`.
    pub fn qualified_monthly_observation(
        &self,
        year: i32,
        month: time::Month,
        knowledge_date: Date,
    ) -> Result<f64> {
        let anchor =
            Date::from_calendar_date(year, month, 1).map_err(|_| InputError::InvalidDateRange)?;
        if knowledge_date > self.knowledge_date {
            return Err(Error::Validation(format!(
                "inflation index '{}' query knowledge_date {} exceeds snapshot knowledge_date {}",
                self.id(),
                knowledge_date,
                self.knowledge_date
            )));
        }
        let obs = self.get_observation(anchor)?;
        if obs.available_on > knowledge_date {
            return Err(Error::Validation(format!(
                "inflation index '{}' observation for {} (available {}) is unavailable on knowledge date {}",
                self.id(),
                anchor,
                obs.available_on,
                knowledge_date
            )));
        }
        Ok(obs.value)
    }
}

/// Canonical serializable wire state of an [`InflationIndex`] snapshot.
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct InflationIndexWire {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    series: Option<InflationSeries>,
    /// Unique identifier.
    pub id: String,
    /// Reporting currency association.
    pub currency: Currency,
    /// Inclusive end-of-day snapshot knowledge cutoff date `K`.
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pub knowledge_date: Date,
    /// Strictly ascending canonical monthly fixing observations.
    pub observations: Vec<InflationObservation>,
    /// Distinct strictly ascending monthly expected-release schedule assumptions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expected_releases: Vec<InflationExpectedRelease>,
}

impl From<InflationIndex> for InflationIndexWire {
    fn from(index: InflationIndex) -> Self {
        Self {
            id: index.id.to_string(),
            series: index.series,
            currency: index.currency,
            knowledge_date: index.knowledge_date,
            observations: index.observations,
            expected_releases: index.expected_releases,
        }
    }
}

impl TryFrom<InflationIndexWire> for InflationIndex {
    type Error = crate::Error;

    fn try_from(state: InflationIndexWire) -> Result<Self> {
        let index = Self::new(
            state.id,
            state.observations,
            state.currency,
            state.knowledge_date,
        )?
        .with_expected_releases(state.expected_releases)?;
        match state.series {
            Some(series) => index.with_series(series),
            None => Ok(index),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    #[test]
    fn zero_lags_normalize_to_none() {
        assert_eq!(InflationLag::months(0), InflationLag::None);
        assert_eq!(InflationLag::days(0), InflationLag::None);
        assert_eq!(InflationLag::from(0_u8), InflationLag::None);
        for spec in ["none", "NONE", "0", "0M", "00M", "000M", "0D", "00D"] {
            assert_eq!(spec.parse::<InflationLag>().unwrap(), InflationLag::None);
        }
        assert_eq!(
            "3M".parse::<InflationLag>().unwrap(),
            InflationLag::months(3)
        );
        assert_eq!(
            "90D".parse::<InflationLag>().unwrap(),
            InflationLag::days(90)
        );
    }

    #[test]
    fn empty_history_with_expected_releases_is_valid_and_roundtrips() {
        let index = InflationIndex::new("US-CPI", vec![], Currency::USD, date!(2026 - 02 - 20))
            .expect("empty history allowed")
            .with_expected_releases(vec![InflationExpectedRelease {
                reference_month: date!(2026 - 02 - 01),
                expected_release: date!(2026 - 03 - 12),
                known_on: date!(2026 - 02 - 01),
            }])
            .expect("valid release fact");
        assert!(index.is_empty());
        assert_eq!(index.date_range(), None);

        let json = serde_json::to_string(&index).expect("serialize");
        let restored: InflationIndex = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.knowledge_date(), date!(2026 - 02 - 20));
        assert_eq!(restored.get_expected_releases().len(), 1);
        assert_eq!(
            restored
                .get_expected_release(date!(2026 - 02 - 01))
                .expect("lookup"),
            Some(InflationExpectedRelease {
                reference_month: date!(2026 - 02 - 01),
                expected_release: date!(2026 - 03 - 12),
                known_on: date!(2026 - 02 - 01),
            })
        );
    }

    #[test]
    fn rejects_non_canonical_months_duplicates_and_future_availability() {
        let cutoff = date!(2026 - 02 - 20);
        // Non-first-of-month label
        assert!(
            InflationIndex::new(
                "CPI",
                vec![InflationObservation {
                    reference_month: date!(2026 - 01 - 31),
                    value: 300.0,
                    available_on: date!(2026 - 02 - 12),
                }],
                Currency::USD,
                cutoff,
            )
            .is_err()
        );
        // Non-positive value
        assert!(
            InflationIndex::new(
                "CPI",
                vec![InflationObservation {
                    reference_month: date!(2026 - 01 - 01),
                    value: 0.0,
                    available_on: date!(2026 - 02 - 12),
                }],
                Currency::USD,
                cutoff,
            )
            .is_err()
        );
        // available_on > knowledge_date
        assert!(
            InflationIndex::new(
                "CPI",
                vec![InflationObservation {
                    reference_month: date!(2026 - 01 - 01),
                    value: 300.0,
                    available_on: date!(2026 - 02 - 21),
                }],
                Currency::USD,
                cutoff,
            )
            .is_err()
        );
        // available_on < reference_month
        assert!(
            InflationIndex::new(
                "CPI",
                vec![InflationObservation {
                    reference_month: date!(2026 - 01 - 01),
                    value: 300.0,
                    available_on: date!(2025 - 12 - 31),
                }],
                Currency::USD,
                cutoff,
            )
            .is_err()
        );
        // expected release schedule known_on > cutoff rejected
        let base_idx =
            InflationIndex::new("CPI", vec![], Currency::USD, cutoff).expect("valid empty");
        assert!(
            base_idx
                .clone()
                .with_expected_releases(vec![InflationExpectedRelease {
                    reference_month: date!(2026 - 02 - 01),
                    expected_release: date!(2026 - 03 - 12),
                    known_on: date!(2026 - 02 - 21),
                }])
                .is_err()
        );
        // Non-canonical month in get_observation / get_expected_release rejected before NotFound
        assert!(base_idx.get_observation(date!(2026 - 01 - 15)).is_err());
        assert!(
            base_idx
                .get_expected_release(date!(2026 - 01 - 15))
                .is_err()
        );
    }

    #[test]
    fn subnormals_and_knowledge_cutoff_work_without_sorting_or_gaps_error() {
        let index = InflationIndex::new(
            "SUB",
            vec![
                InflationObservation {
                    reference_month: date!(2025 - 01 - 01),
                    value: f64::MIN_POSITIVE / 2.0,
                    available_on: date!(2025 - 02 - 12),
                },
                InflationObservation {
                    reference_month: date!(2025 - 03 - 01),
                    value: 305.0,
                    available_on: date!(2025 - 04 - 10),
                },
            ],
            Currency::USD,
            date!(2025 - 04 - 15),
        )
        .expect("subnormals and gaps allowed");

        assert_eq!(
            index
                .qualified_monthly_observation(2025, time::Month::January, date!(2025 - 02 - 12))
                .unwrap(),
            f64::MIN_POSITIVE / 2.0
        );
        // Query before available_on fails
        assert!(
            index
                .qualified_monthly_observation(2025, time::Month::March, date!(2025 - 04 - 09))
                .is_err()
        );
        // Query past snapshot knowledge_date fails
        assert!(
            index
                .qualified_monthly_observation(2025, time::Month::January, date!(2025 - 04 - 16))
                .is_err()
        );
        // Gap month fails NotFound
        assert!(matches!(
            index.get_observation(date!(2025 - 02 - 01)),
            Err(Error::Input(InputError::NotFound { .. }))
        ));
    }
}
