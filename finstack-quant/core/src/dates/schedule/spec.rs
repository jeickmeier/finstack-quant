//! Serializable schedule specification.

use super::*;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "ScheduleSpecWire")]
/// Complete, validated specification of a schedule (record
/// CORE-CONVENTIONS-008 #2).
///
/// Every financial choice is explicit: there is no default frequency, stub,
/// grid or payment convention. Only the operation defaults remain: zero
/// extra payment lag and no fixing series. A specification is validated when
/// it is deserialized and again before generation, so a value assembled or
/// mutated directly cannot bypass the rules. Build it with
/// `ScheduleBuilder::from(spec).build()`.
///
/// # Examples
/// ```rust
/// use finstack_quant_core::dates::{
///     BusinessDayConvention, GridMode, ScheduleBuilder, ScheduleSpec, StubKind, Tenor,
/// };
/// use time::macros::date;
///
/// let spec = ScheduleSpec {
///     start: date!(2025 - 01 - 15),
///     end: date!(2026 - 01 - 15),
///     frequency: Tenor::quarterly(),
///     stub: StubKind::None,
///     grid: GridMode::Natural,
///     business_day_convention: BusinessDayConvention::Unadjusted,
///     calendar_id: None,
///     payment_lag_days: 0,
///     fixing_lag_business_days: None,
/// };
/// let schedule = ScheduleBuilder::from(spec).build()?;
/// assert_eq!(schedule.get_dates().len(), 5);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub struct ScheduleSpec {
    /// First unadjusted grid date; must precede `end`.
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pub start: Date,
    /// Last unadjusted grid date (maturity).
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pub end: Date,
    /// Regular period length.
    pub frequency: Tenor,
    /// Stub policy, which also selects the regular anchor side.
    pub stub: StubKind,
    /// Regular-grid selection.
    pub grid: GridMode,
    /// Payment-date business-day convention, including a deliberate
    /// `Unadjusted`.
    pub business_day_convention: BusinessDayConvention,
    /// Calendar for payment adjustment and both lags. Required unless the
    /// convention is `Unadjusted` and no positive lag is set; always
    /// resolved strictly when given.
    pub calendar_id: Option<String>,
    /// Business days after each adjusted period end for the payment date.
    pub payment_lag_days: i32,
    /// T-minus business days from each period's raw start for the fixing
    /// date, or `None` for no fixing series.
    pub fixing_lag_business_days: Option<i32>,
}

#[derive(serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct ScheduleSpecWire {
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    start: Date,
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    end: Date,
    frequency: Tenor,
    stub: StubKind,
    grid: GridMode,
    business_day_convention: BusinessDayConvention,
    calendar_id: Option<String>,
    #[serde(default)]
    payment_lag_days: i32,
    #[serde(default)]
    fixing_lag_business_days: Option<i32>,
}

impl TryFrom<ScheduleSpecWire> for ScheduleSpec {
    type Error = crate::Error;

    fn try_from(wire: ScheduleSpecWire) -> Result<Self, Self::Error> {
        let spec = Self {
            start: wire.start,
            end: wire.end,
            frequency: wire.frequency,
            stub: wire.stub,
            grid: wire.grid,
            business_day_convention: wire.business_day_convention,
            calendar_id: wire.calendar_id,
            payment_lag_days: wire.payment_lag_days,
            fixing_lag_business_days: wire.fixing_lag_business_days,
        };
        spec.validate()?;
        Ok(spec)
    }
}

impl ScheduleSpec {
    /// Check the specification's own consistency (not its dates' grid
    /// alignment, which generation checks).
    ///
    /// # Errors
    ///
    /// Returns an error when `start` is not before `end`, the frequency does
    /// not suit the grid, a lag is negative, or a calendar is required but
    /// absent.
    pub fn validate(&self) -> crate::Result<()> {
        if self.start >= self.end {
            return Err(crate::InputError::InvalidDateRange.into());
        }
        self.grid.check_frequency(self.frequency)?;
        for lag in [self.payment_lag_days]
            .into_iter()
            .chain(self.fixing_lag_business_days)
        {
            if lag < 0 {
                return Err(InputError::NegativeScheduleLag { lag }.into());
            }
        }
        if self.calendar_id.is_none() {
            if self.payment_lag_days > 0 || self.fixing_lag_business_days.is_some_and(|lag| lag > 0)
            {
                return Err(InputError::ScheduleLagRequiresCalendar.into());
            }
            if self.business_day_convention != BusinessDayConvention::Unadjusted {
                return Err(crate::Error::Validation(format!(
                    "payment convention {} needs a calendar; use Unadjusted for none",
                    self.business_day_convention
                )));
            }
        }
        Ok(())
    }

    /// The regular grid's nominal reference (record CORE-CONVENTIONS-008
    /// #14): the original anchor, frequency and grid from which every
    /// regular roll is computed, including boundaries removed by a long stub
    /// or lying outside the schedule.
    ///
    /// # Errors
    ///
    /// Returns an error when the specification is invalid.
    pub fn nominal_reference(&self) -> crate::Result<NominalReference> {
        self.validate()?;
        let front = matches!(self.stub, StubKind::ShortFront | StubKind::LongFront);
        let anchor = if front { self.end } else { self.start };
        Ok(NominalReference {
            anchor,
            frequency: self.frequency,
            grid: self.grid,
        })
    }
}

/// Original anchor, frequency and grid of a schedule's regular periods
/// (record CORE-CONVENTIONS-008 #14), for consumers that need full nominal
/// reference periods (for example ICMA accrual).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NominalReference {
    /// Date every regular roll is computed from.
    pub anchor: Date,
    /// Regular period length.
    pub frequency: Tenor,
    /// Grid each computed roll is snapped to.
    pub grid: GridMode,
}

impl NominalReference {
    /// The `n`-th regular roll from the anchor (`n` negative counts
    /// backward), computed directly from the anchor and snapped to the grid.
    ///
    /// # Arguments
    ///
    /// * `n` - Signed roll index; `0` is the anchor itself.
    ///
    /// # Errors
    ///
    /// Returns an error when the roll is outside the supported date range or
    /// the tenor arithmetic overflows; the reference is then unavailable even
    /// if the schedule itself ends before it.
    pub fn roll_date(&self, n: i32) -> crate::Result<Date> {
        let generator = BuilderInternal {
            start: self.anchor,
            end: self.anchor,
            frequency: self.frequency,
            stub: StubKind::None,
            roll: self.grid,
        };
        generator.nth_rolled(self.anchor, n)?.ok_or_else(|| {
            crate::Error::Validation(format!(
                "nominal roll {n} from {} at {} is outside the supported date range",
                self.anchor, self.frequency
            ))
        })
    }
}
