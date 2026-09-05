//! Schedule parameter types for cashflow generation.

use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use rust_decimal::Decimal;

/// Roll-date rule applied when generating schedule anchors.
///
/// Selects the core `ScheduleBuilder` date-generation mode. The IMM modes
/// force a quarterly grid: the schedule frequency and stub rule configured on
/// [`ScheduleParams`] are overridden with quarterly / short-back, matching
/// `ScheduleBuilder::imm` and `ScheduleBuilder::cds_imm`.
///
/// # Variants
///
/// - **`None`**: plain tenor stepping from the schedule boundaries (default).
/// - **`Imm`**: quarterly third Wednesdays of Mar/Jun/Sep/Dec (CME IMM dates
///   for rate, currency, and equity index futures).
/// - **`CdsImm`**: 20th of Mar/Jun/Sep/Dec (post-Big-Bang standard CDS roll
///   dates). When the start date is not itself a roll date, the first period
///   accrues from the roll date immediately **preceding** the start (standard
///   front accrual per the ISDA Big Bang Protocol, April 2009).
///
/// # Examples
///
/// ```rust
/// use finstack_quant_cashflows::builder::specs::RollRule;
///
/// let rule = RollRule::default();
/// assert_eq!(rule, RollRule::None);
/// ```
///
/// # References
///
/// - `docs/REFERENCES.md#isda-cds-standard-model`
/// - CME IMM date rules (third Wednesday of the contract month)
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RollRule {
    /// Plain tenor stepping from the schedule boundaries (no roll-date grid).
    #[default]
    None,
    /// Standard IMM dates: third Wednesday of Mar/Jun/Sep/Dec.
    Imm,
    /// CDS IMM dates: 20th of Mar/Jun/Sep/Dec with post-Big-Bang front accrual.
    CdsImm,
}

impl RollRule {
    /// Returns `true` for [`RollRule::None`].
    ///
    /// Used by serde to keep the default rule off the wire so existing
    /// payloads are unchanged.
    #[must_use]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

/// Canonical schedule-generation parameters for coupons and periodic fees.
///
/// This type controls how accrual boundaries and payment dates are generated.
/// The fields describe schedule construction conventions, not discounting or
/// valuation conventions.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ScheduleParams {
    /// Accrual and payment frequency used to generate the schedule boundaries.
    pub frequency: Tenor,
    /// Day-count convention used to convert each generated accrual period into a
    /// year fraction.
    pub day_count: DayCount,
    /// Business-day convention applied when rolling **payment dates** onto
    /// valid business days.
    ///
    /// Accrual boundaries are left unadjusted (bond/ICMA convention) unless
    /// [`Self::adjust_accrual_dates`] is `true`, in which case the same
    /// convention also rolls both accrual-period boundaries (swap/ISDA 2006
    /// §4.10 convention).
    #[serde(default = "crate::serde_defaults::bdc_modified_following")]
    pub business_day_convention: BusinessDayConvention,
    /// Holiday calendar identifier used together with `business_day_convention`.
    ///
    /// Use `"weekends_only"` when only Saturday/Sunday adjustment is needed.
    pub calendar_id: String,
    /// Stub-handling rule used when the start/end dates do not fit an exact
    /// whole number of periods.
    #[serde(default = "crate::serde_defaults::stub_short_front")]
    pub stub: StubKind,
    /// Whether end-of-month rolling should be preserved when generating the
    /// schedule.
    #[serde(default)]
    pub end_of_month: bool,
    /// Payment lag in business days after the adjusted accrual end date.
    #[serde(default)]
    pub payment_lag_days: i32,
    /// Whether accrual-period boundaries are business-day adjusted with
    /// `business_day_convention`.
    ///
    /// - `false` (default, bond convention): accrual periods run between the
    ///   unadjusted schedule anchors; only payment dates roll.
    /// - `true` (swap convention, ISDA 2006 §4.10 / ARRC SOFR conventions):
    ///   both accrual-period boundaries are rolled with `business_day_convention` before year
    ///   fractions and overnight observation windows are computed. The swap
    ///   presets ([`Self::usd_sofr_swap`], [`Self::eur_estr_swap`],
    ///   [`Self::gbp_sonia_swap`], [`Self::jpy_tona_swap`]) set this to
    ///   `true`.
    ///
    /// Serialized only when `true`, so existing wire payloads are unchanged.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub adjust_accrual_dates: bool,
    /// Roll-date rule for schedule anchors (standard IMM or CDS IMM grids).
    ///
    /// [`RollRule::None`] (default) keeps plain tenor stepping. The IMM modes
    /// override `frequency`/`stub` with quarterly / short-back; see [`RollRule`].
    ///
    /// Serialized only when not `None`, so existing wire payloads are
    /// unchanged.
    #[serde(default, skip_serializing_if = "RollRule::is_none")]
    pub roll_rule: RollRule,
}

const WK: &str = crate::builder::calendar::WEEKENDS_ONLY_ID;

impl ScheduleParams {
    fn preset(
        frequency: Tenor,
        day_count: DayCount,
        business_day_convention: BusinessDayConvention,
        calendar_id: &str,
    ) -> Self {
        Self {
            frequency,
            day_count,
            business_day_convention,
            calendar_id: calendar_id.to_string(),
            stub: StubKind::ShortFront,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
            roll_rule: RollRule::None,
        }
    }

    /// Swap-style preset: accrual periods are business-day adjusted
    /// (ISDA 2006 §4.10; ARRC SOFR conventions).
    fn swap_preset(
        frequency: Tenor,
        day_count: DayCount,
        business_day_convention: BusinessDayConvention,
        calendar_id: &str,
        payment_lag_days: i32,
    ) -> Self {
        Self {
            payment_lag_days,
            adjust_accrual_dates: true,
            ..Self::preset(frequency, day_count, business_day_convention, calendar_id)
        }
    }

    /// Quarterly payments with Act/360 day count and Modified Following BDC.
    ///
    /// # Returns
    ///
    /// Schedule parameters using a weekends-only calendar, short-front stubs,
    /// no end-of-month rolling, and no payment lag.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::ScheduleParams;
    /// use finstack_quant_core::dates::{BusinessDayConvention, DayCount, Tenor};
    ///
    /// let params = ScheduleParams::quarterly_act360();
    /// assert_eq!(params.frequency, Tenor::quarterly());
    /// assert_eq!(params.day_count, DayCount::Act360);
    /// assert_eq!(params.business_day_convention, BusinessDayConvention::ModifiedFollowing);
    /// assert_eq!(params.calendar_id, "weekends_only");
    /// ```
    ///
    /// # References
    ///
    /// - `docs/REFERENCES.md#isda-2006-definitions`
    pub fn quarterly_act360() -> Self {
        Self::preset(
            Tenor::quarterly(),
            DayCount::Act360,
            BusinessDayConvention::ModifiedFollowing,
            WK,
        )
    }

    /// Semi-annual payments with 30/360 day count and Modified Following BDC.
    ///
    /// # Returns
    ///
    /// Schedule parameters using a weekends-only calendar, short-front stubs,
    /// no end-of-month rolling, and no payment lag.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::ScheduleParams;
    /// use finstack_quant_core::dates::{BusinessDayConvention, DayCount, Tenor};
    ///
    /// let params = ScheduleParams::semiannual_30360();
    /// assert_eq!(params.frequency, Tenor::semi_annual());
    /// assert_eq!(params.day_count, DayCount::Thirty360);
    /// assert_eq!(params.business_day_convention, BusinessDayConvention::ModifiedFollowing);
    /// ```
    ///
    /// # References
    ///
    /// - `docs/REFERENCES.md#isda-2006-definitions`
    pub fn semiannual_30360() -> Self {
        Self::preset(
            Tenor::semi_annual(),
            DayCount::Thirty360,
            BusinessDayConvention::ModifiedFollowing,
            WK,
        )
    }

    /// Annual payments with ISDA Act/Act day count and Following BDC.
    ///
    /// This is the ISDA Actual/Actual convention (`DayCount::ActAct`), not
    /// ICMA Act/Act. Government-bond schedules use [`Self::eur_gov_bond`] or
    /// [`Self::usd_treasury`].
    ///
    /// # Returns
    ///
    /// Schedule parameters using a weekends-only calendar, short-front stubs,
    /// no end-of-month rolling, and no payment lag.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::ScheduleParams;
    /// use finstack_quant_core::dates::{BusinessDayConvention, DayCount, Tenor};
    ///
    /// let params = ScheduleParams::annual_actact();
    /// assert_eq!(params.frequency, Tenor::annual());
    /// assert_eq!(params.day_count, DayCount::ActAct);
    /// assert_eq!(params.business_day_convention, BusinessDayConvention::Following);
    /// ```
    ///
    /// # References
    ///
    /// - `docs/REFERENCES.md#isda-2006-definitions`
    pub fn annual_actact() -> Self {
        Self::preset(
            Tenor::annual(),
            DayCount::ActAct,
            BusinessDayConvention::Following,
            WK,
        )
    }

    /// USD SOFR swap (quarterly, Act/360, Modified Following, USNY, T+2 payment lag).
    ///
    /// Follows ARRC SOFR conventions and ISDA 2021 definitions.
    ///
    /// # Returns
    ///
    /// Schedule parameters for a USD SOFR-style floating leg with USNY
    /// calendar and two-business-day payment lag.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::ScheduleParams;
    /// use finstack_quant_core::dates::{BusinessDayConvention, DayCount, Tenor};
    ///
    /// let params = ScheduleParams::usd_sofr_swap();
    /// assert_eq!(params.frequency, Tenor::quarterly());
    /// assert_eq!(params.day_count, DayCount::Act360);
    /// assert_eq!(params.business_day_convention, BusinessDayConvention::ModifiedFollowing);
    /// assert_eq!(params.calendar_id, "usny");
    /// assert_eq!(params.payment_lag_days, 2);
    /// assert!(params.adjust_accrual_dates);
    /// ```
    ///
    /// # References
    ///
    /// - `docs/REFERENCES.md#arrc-sofr-users-guide`
    /// - `docs/REFERENCES.md#isda-2006-definitions`
    pub fn usd_sofr_swap() -> Self {
        Self::swap_preset(
            Tenor::quarterly(),
            DayCount::Act360,
            BusinessDayConvention::ModifiedFollowing,
            "usny",
            2,
        )
    }

    /// USD corporate bond (semi-annual, 30/360, Following, USNY).
    ///
    /// # Returns
    ///
    /// Schedule parameters for a plain USD corporate bond coupon schedule.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::ScheduleParams;
    /// use finstack_quant_core::dates::{BusinessDayConvention, DayCount, Tenor};
    ///
    /// let params = ScheduleParams::usd_corporate_bond();
    /// assert_eq!(params.frequency, Tenor::semi_annual());
    /// assert_eq!(params.day_count, DayCount::Thirty360);
    /// assert_eq!(params.business_day_convention, BusinessDayConvention::Following);
    /// assert_eq!(params.calendar_id, "usny");
    /// ```
    ///
    /// # References
    ///
    /// - `docs/REFERENCES.md#icma-rule-book`
    /// - `docs/REFERENCES.md#isda-2006-definitions`
    pub fn usd_corporate_bond() -> Self {
        Self::preset(
            Tenor::semi_annual(),
            DayCount::Thirty360,
            BusinessDayConvention::Following,
            "usny",
        )
    }

    /// USD Treasury bond (semi-annual, Act/Act, Following, USNY).
    ///
    /// # Returns
    ///
    /// Schedule parameters for a USD Treasury-style coupon schedule.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::ScheduleParams;
    /// use finstack_quant_core::dates::{BusinessDayConvention, DayCount, Tenor};
    ///
    /// let params = ScheduleParams::usd_treasury();
    /// assert_eq!(params.frequency, Tenor::semi_annual());
    /// assert_eq!(params.day_count, DayCount::ActActIsma);
    /// assert_eq!(params.business_day_convention, BusinessDayConvention::Following);
    /// assert_eq!(params.calendar_id, "usny");
    /// ```
    ///
    /// # References
    ///
    /// - `docs/REFERENCES.md#icma-rule-book`
    /// - `docs/REFERENCES.md#isda-2006-definitions`
    pub fn usd_treasury() -> Self {
        Self::preset(
            Tenor::semi_annual(),
            DayCount::ActActIsma,
            BusinessDayConvention::Following,
            "usny",
        )
    }

    /// EUR ESTR swap (annual, Act/360, Modified Following, TARGET2, T+2 payment lag).
    ///
    /// Payment lag is the **LCH** €STR OIS template (T+2), not a universal CCP
    /// default. Follows ECB €STR compounding conventions for the day count.
    ///
    /// # Returns
    ///
    /// Schedule parameters for a EUR €STR-style floating leg with TARGET2
    /// calendar and the LCH two-business-day payment lag.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::ScheduleParams;
    /// use finstack_quant_core::dates::{BusinessDayConvention, DayCount, Tenor};
    ///
    /// let params = ScheduleParams::eur_estr_swap();
    /// assert_eq!(params.frequency, Tenor::annual());
    /// assert_eq!(params.day_count, DayCount::Act360);
    /// assert_eq!(params.business_day_convention, BusinessDayConvention::ModifiedFollowing);
    /// assert_eq!(params.calendar_id, "target2");
    /// assert_eq!(params.payment_lag_days, 2);
    /// ```
    ///
    /// # References
    ///
    /// - `docs/REFERENCES.md#ecb-estr-methodology`
    /// - `docs/REFERENCES.md#isda-2006-definitions`
    pub fn eur_estr_swap() -> Self {
        Self::swap_preset(
            Tenor::annual(),
            DayCount::Act360,
            BusinessDayConvention::ModifiedFollowing,
            "target2",
            2,
        )
    }

    /// EUR government bond (annual, Act/Act, Following, TARGET2).
    ///
    /// # Returns
    ///
    /// Schedule parameters for an annual EUR government bond coupon schedule.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::ScheduleParams;
    /// use finstack_quant_core::dates::{BusinessDayConvention, DayCount, Tenor};
    ///
    /// let params = ScheduleParams::eur_gov_bond();
    /// assert_eq!(params.frequency, Tenor::annual());
    /// assert_eq!(params.day_count, DayCount::ActActIsma);
    /// assert_eq!(params.business_day_convention, BusinessDayConvention::Following);
    /// assert_eq!(params.calendar_id, "target2");
    /// ```
    ///
    /// # References
    ///
    /// - `docs/REFERENCES.md#icma-rule-book`
    /// - `docs/REFERENCES.md#isda-2006-definitions`
    pub fn eur_gov_bond() -> Self {
        Self::preset(
            Tenor::annual(),
            DayCount::ActActIsma,
            BusinessDayConvention::Following,
            "target2",
        )
    }

    /// GBP SONIA swap (annual, Act/365F, Modified Following, GBLO, no payment lag).
    ///
    /// Follows BoE SONIA conventions.
    ///
    /// # Returns
    ///
    /// Schedule parameters for a GBP SONIA-style floating leg with GBLO
    /// calendar and no payment lag.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::ScheduleParams;
    /// use finstack_quant_core::dates::{BusinessDayConvention, DayCount, Tenor};
    ///
    /// let params = ScheduleParams::gbp_sonia_swap();
    /// assert_eq!(params.frequency, Tenor::annual());
    /// assert_eq!(params.day_count, DayCount::Act365F);
    /// assert_eq!(params.business_day_convention, BusinessDayConvention::ModifiedFollowing);
    /// assert_eq!(params.calendar_id, "gblo");
    /// assert_eq!(params.payment_lag_days, 0);
    /// ```
    ///
    /// # References
    ///
    /// - `docs/REFERENCES.md#boe-sonia-key-features`
    /// - `docs/REFERENCES.md#isda-2006-definitions`
    pub fn gbp_sonia_swap() -> Self {
        Self::swap_preset(
            Tenor::annual(),
            DayCount::Act365F,
            BusinessDayConvention::ModifiedFollowing,
            "gblo",
            0,
        )
    }

    /// JPY TONA swap (annual, Act/365F, Modified Following, JPTO, T+2 payment lag).
    ///
    /// Follows BoJ TONA conventions.
    ///
    /// # Returns
    ///
    /// Schedule parameters for a JPY TONA-style floating leg with JPTO
    /// calendar and two-business-day payment lag.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::ScheduleParams;
    /// use finstack_quant_core::dates::{BusinessDayConvention, DayCount, Tenor};
    ///
    /// let params = ScheduleParams::jpy_tona_swap();
    /// assert_eq!(params.frequency, Tenor::annual());
    /// assert_eq!(params.day_count, DayCount::Act365F);
    /// assert_eq!(params.business_day_convention, BusinessDayConvention::ModifiedFollowing);
    /// assert_eq!(params.calendar_id, "jpto");
    /// assert_eq!(params.payment_lag_days, 2);
    /// ```
    ///
    /// # References
    ///
    /// - `docs/REFERENCES.md#boj-tona`
    /// - `docs/REFERENCES.md#isda-2006-definitions`
    pub fn jpy_tona_swap() -> Self {
        Self::swap_preset(
            Tenor::annual(),
            DayCount::Act365F,
            BusinessDayConvention::ModifiedFollowing,
            "jpto",
            2,
        )
    }

    /// Fail-fast validation of the schedule conventions.
    ///
    /// Checks that `calendar_id` resolves to a registered holiday calendar
    /// (or `"weekends_only"`) and that `payment_lag_days` is non-negative, so
    /// a typo in a calendar id surfaces at construction time instead of deep
    /// inside a schedule build.
    ///
    /// # Arguments
    ///
    /// None beyond `self`; every field of the parameters is inspected.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] when the calendar id
    /// is unknown or the payment lag is negative.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::ScheduleParams;
    ///
    /// assert!(ScheduleParams::usd_sofr_swap().validate().is_ok());
    /// let mut bad = ScheduleParams::usd_sofr_swap();
    /// bad.calendar_id = "not-a-calendar".to_string();
    /// assert!(bad.validate().is_err());
    /// ```
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        crate::builder::calendar::resolve_calendar_strict(&self.calendar_id).map_err(|err| {
            finstack_quant_core::Error::Validation(format!(
                "unknown calendar_id '{}': {err}",
                self.calendar_id
            ))
        })?;
        if self.payment_lag_days < 0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "payment_lag_days must be non-negative; got {}",
                self.payment_lag_days
            )));
        }
        Ok(())
    }
}

/// Fixed-rate coupon window with a shared schedule.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FixedWindow {
    /// Annual coupon rate as a decimal, for example `0.05` for 5%.
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
    pub rate: Decimal,
    /// Schedule-generation parameters for this fixed-rate window.
    pub schedule: ScheduleParams,
}
