//! Volatility clock tying option expiry year fractions to a valuation reference date and day-count convention.

use crate::dates::{Date, DayCount, DayCountContextState};

/// Explicit day-count clock translating calendar expiry dates into year fractions
/// relative to a surface or cube reference date.
///
/// Market data objects such as [`FxDeltaVolSurface`](crate::market_data::surfaces::FxDeltaVolSurface)
/// attach an optional `VolatilityClock` so downstream pricers can validate that
/// the surface anchor matches `as_of` and bridge between instrument day-counts
/// and surface variance time without silently assuming a day-count convention.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::{DayCount, DayCountContextState, VolatilityClock};
/// use time::macros::date;
///
/// let clock = VolatilityClock::try_new(
///     date!(2025 - 01 - 15),
///     DayCount::Act365F,
///     DayCountContextState::default(),
/// )?;
/// let t = clock.expiry_time(date!(2026 - 01 - 15))?;
/// assert!((t - 1.0).abs() < 1e-12);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, try_from = "VolatilityClockFields")]
#[cfg_attr(
    feature = "json-schema",
    derive(schemars::JsonSchema),
    schemars(!try_from, deny_unknown_fields)
)]
pub struct VolatilityClock {
    /// Valuation reference date anchored to year fraction `0.0`.
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    reference_date: Date,
    /// Day-count convention used to measure expiry times from `reference_date`.
    day_count: DayCount,
    /// Owned, canonicalized day-count context parameters.
    #[serde(default)]
    context: DayCountContextState,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct VolatilityClockFields {
    reference_date: Date,
    day_count: DayCount,
    #[serde(default)]
    context: DayCountContextState,
}

impl TryFrom<VolatilityClockFields> for VolatilityClock {
    type Error = crate::Error;

    fn try_from(fields: VolatilityClockFields) -> Result<Self, Self::Error> {
        Self::try_new(fields.reference_date, fields.day_count, fields.context)
    }
}

impl VolatilityClock {
    /// Construct and validate a [`VolatilityClock`].
    ///
    /// Resolves `context` against the calendar registry, verifies that `context`
    /// satisfies `day_count` requirements, and stores the re-canonicalized context
    /// (for example canonical composite calendar ordering such as `"TARGET2+USNY"`).
    ///
    /// # Arguments
    ///
    /// * `reference_date` - Valuation anchor date corresponding to expiry time `0.0`.
    /// * `day_count` - Day-count convention used to translate expiry dates into
    ///   annualized year fractions.
    /// * `context` - Owned day-count context parameters such as calendar identifier,
    ///   frequency, or business-day basis required by `day_count`.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error`] when `context` references an unknown calendar,
    /// fails structural validation, or violates `day_count.validate_context(...)`.
    pub fn try_new(
        reference_date: Date,
        day_count: DayCount,
        context: DayCountContextState,
    ) -> crate::Result<Self> {
        let ctx = context.to_ctx()?;
        day_count.validate_context(ctx)?;
        let canonical_context = DayCountContextState::try_from(ctx)?;
        Ok(Self {
            reference_date,
            day_count,
            context: canonical_context,
        })
    }

    /// Return the clock's valuation reference date (`t = 0.0`).
    #[must_use]
    pub const fn get_reference_date(&self) -> Date {
        self.reference_date
    }

    /// Return the day-count convention used by this clock.
    #[must_use]
    pub const fn get_day_count(&self) -> DayCount {
        self.day_count
    }

    /// Return the canonicalized day-count context state stored by this clock.
    #[must_use]
    pub const fn get_context(&self) -> &DayCountContextState {
        &self.context
    }

    /// Compute the annualized year fraction from `reference_date` to `expiry_date`.
    ///
    /// # Arguments
    ///
    /// * `expiry_date` - Option expiration calendar date on or after `reference_date`.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Input`] with [`crate::InputError::InvalidDateRange`]
    /// when `expiry_date < self.get_reference_date()`, or a validation error when
    /// the stored context cannot evaluate `day_count.year_fraction`.
    pub fn expiry_time(&self, expiry_date: Date) -> crate::Result<f64> {
        let ctx = self.context.to_ctx()?;
        self.day_count
            .year_fraction(self.reference_date, expiry_date, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::VolatilityClock;
    use crate::dates::{DayCount, DayCountContextState};
    use time::macros::date;

    #[test]
    fn try_new_canonicalizes_composite_calendar_and_validates_day_count() {
        let state = DayCountContextState::try_new(
            Some(" usny + TARGET2+usny ".to_string()),
            None,
            None,
            None,
            None,
        )
        .expect("valid composite calendar");
        let clock = VolatilityClock::try_new(date!(2025 - 01 - 15), DayCount::Bus252, state)
            .expect("valid Bus252 clock");
        assert_eq!(clock.get_reference_date(), date!(2025 - 01 - 15));
        assert_eq!(clock.get_day_count(), DayCount::Bus252);
        assert_eq!(clock.get_context().get_calendar_id(), Some("target2+usny"));
        assert!(
            clock
                .expiry_time(date!(2025 - 07 - 15))
                .expect("expiry_time")
                > 0.0
        );
        assert!(clock.expiry_time(date!(2025 - 01 - 14)).is_err());

        let missing_cal = VolatilityClock::try_new(
            date!(2025 - 01 - 15),
            DayCount::Bus252,
            DayCountContextState::default(),
        );
        assert!(missing_cal.is_err());
    }

    #[test]
    fn serde_round_trip_and_rejects_invalid_or_unknown_fields() {
        let clock = VolatilityClock::try_new(
            date!(2025 - 01 - 15),
            DayCount::Act365F,
            DayCountContextState::default(),
        )
        .expect("valid clock");
        let json = serde_json::to_value(&clock).expect("serialize");
        let roundtrip: VolatilityClock = serde_json::from_value(json).expect("deserialize");
        assert_eq!(roundtrip, clock);

        let minimal = serde_json::json!({
            "reference_date": "2025-01-15",
            "day_count": "act_365f"
        });
        assert!(serde_json::from_value::<VolatilityClock>(minimal).is_ok());

        let invalid_bus252 = serde_json::json!({
            "reference_date": "2025-01-15",
            "day_count": "bus_252"
        });
        assert!(serde_json::from_value::<VolatilityClock>(invalid_bus252).is_err());

        let unknown_field = serde_json::json!({
            "reference_date": "2025-01-15",
            "day_count": "act_365f",
            "extra": true
        });
        assert!(serde_json::from_value::<VolatilityClock>(unknown_field).is_err());
    }
}
