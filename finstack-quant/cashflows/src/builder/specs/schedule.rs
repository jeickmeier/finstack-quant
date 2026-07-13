//! Schedule parameter types for cashflow generation.

use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use rust_decimal::Decimal;

/// Canonical schedule-generation parameters for coupons and periodic fees.
///
/// This type controls how accrual boundaries and payment dates are generated.
/// The fields describe schedule construction conventions, not discounting or
/// valuation conventions.
#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ScheduleParams {
    /// Accrual and payment frequency used to generate the schedule boundaries.
    pub freq: Tenor,
    /// Day-count convention used to convert each generated accrual period into a
    /// year fraction.
    pub dc: DayCount,
    /// Business-day convention applied when rolling **payment dates** onto
    /// valid business days.
    ///
    /// Accrual boundaries are left unadjusted (bond/ICMA convention) unless
    /// [`Self::adjust_accrual_dates`] is `true`, in which case the same
    /// convention also rolls both accrual-period boundaries (swap/ISDA 2006
    /// §4.10 convention).
    #[serde(default = "crate::serde_defaults::bdc_modified_following")]
    pub bdc: BusinessDayConvention,
    /// Holiday calendar identifier used together with `bdc`.
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
    /// Whether accrual-period boundaries are business-day adjusted with `bdc`.
    ///
    /// - `false` (default, bond convention): accrual periods run between the
    ///   unadjusted schedule anchors; only payment dates roll.
    /// - `true` (swap convention, ISDA 2006 §4.10 / ARRC SOFR conventions):
    ///   both accrual-period boundaries are rolled with `bdc` before year
    ///   fractions and overnight observation windows are computed. The swap
    ///   presets ([`Self::usd_sofr_swap`], [`Self::eur_estr_swap`],
    ///   [`Self::gbp_sonia_swap`], [`Self::jpy_tona_swap`]) set this to
    ///   `true`.
    ///
    /// Serialized only when `true`, so existing wire payloads are unchanged.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub adjust_accrual_dates: bool,
}

const WK: &str = crate::builder::calendar::WEEKENDS_ONLY_ID;

impl ScheduleParams {
    fn preset(freq: Tenor, dc: DayCount, bdc: BusinessDayConvention, calendar_id: &str) -> Self {
        Self {
            freq,
            dc,
            bdc,
            calendar_id: calendar_id.to_string(),
            stub: StubKind::ShortFront,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
        }
    }

    /// Swap-style preset: accrual periods are business-day adjusted
    /// (ISDA 2006 §4.10; ARRC SOFR conventions).
    fn swap_preset(
        freq: Tenor,
        dc: DayCount,
        bdc: BusinessDayConvention,
        calendar_id: &str,
        payment_lag_days: i32,
    ) -> Self {
        Self {
            payment_lag_days,
            adjust_accrual_dates: true,
            ..Self::preset(freq, dc, bdc, calendar_id)
        }
    }

    // ── Generic presets (calendar-agnostic) ──────────────────────────────────

    /// Quarterly payments with Act/360 day count and Modified Following BDC.
    ///
    /// # Arguments
    ///
    /// None.
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
    /// assert_eq!(params.freq, Tenor::quarterly());
    /// assert_eq!(params.dc, DayCount::Act360);
    /// assert_eq!(params.bdc, BusinessDayConvention::ModifiedFollowing);
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
    /// # Arguments
    ///
    /// None.
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
    /// assert_eq!(params.freq, Tenor::semi_annual());
    /// assert_eq!(params.dc, DayCount::Thirty360);
    /// assert_eq!(params.bdc, BusinessDayConvention::ModifiedFollowing);
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

    /// Annual payments with Act/Act day count and Following BDC.
    ///
    /// # Arguments
    ///
    /// None.
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
    /// assert_eq!(params.freq, Tenor::annual());
    /// assert_eq!(params.dc, DayCount::ActAct);
    /// assert_eq!(params.bdc, BusinessDayConvention::Following);
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

    // ── USD ──────────────────────────────────────────────────────────────────

    /// USD SOFR swap (quarterly, Act/360, Modified Following, USNY, T+2 payment lag).
    ///
    /// Follows ARRC SOFR conventions and ISDA 2021 definitions.
    ///
    /// # Arguments
    ///
    /// None.
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
    /// assert_eq!(params.freq, Tenor::quarterly());
    /// assert_eq!(params.dc, DayCount::Act360);
    /// assert_eq!(params.bdc, BusinessDayConvention::ModifiedFollowing);
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
    /// # Arguments
    ///
    /// None.
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
    /// assert_eq!(params.freq, Tenor::semi_annual());
    /// assert_eq!(params.dc, DayCount::Thirty360);
    /// assert_eq!(params.bdc, BusinessDayConvention::Following);
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
    /// # Arguments
    ///
    /// None.
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
    /// assert_eq!(params.freq, Tenor::semi_annual());
    /// assert_eq!(params.dc, DayCount::ActActIsma);
    /// assert_eq!(params.bdc, BusinessDayConvention::Following);
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

    // ── EUR ──────────────────────────────────────────────────────────────────

    /// EUR ESTR swap (annual, Act/360, Modified Following, TARGET2, T+2 payment lag).
    ///
    /// Follows ECB €STR conventions.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// Schedule parameters for a EUR €STR-style floating leg with TARGET2
    /// calendar and two-business-day payment lag.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::ScheduleParams;
    /// use finstack_quant_core::dates::{BusinessDayConvention, DayCount, Tenor};
    ///
    /// let params = ScheduleParams::eur_estr_swap();
    /// assert_eq!(params.freq, Tenor::annual());
    /// assert_eq!(params.dc, DayCount::Act360);
    /// assert_eq!(params.bdc, BusinessDayConvention::ModifiedFollowing);
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
    /// # Arguments
    ///
    /// None.
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
    /// assert_eq!(params.freq, Tenor::annual());
    /// assert_eq!(params.dc, DayCount::ActActIsma);
    /// assert_eq!(params.bdc, BusinessDayConvention::Following);
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

    // ── GBP ──────────────────────────────────────────────────────────────────

    /// GBP SONIA swap (annual, Act/365F, Modified Following, GBLO, no payment lag).
    ///
    /// Follows BoE SONIA conventions.
    ///
    /// # Arguments
    ///
    /// None.
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
    /// assert_eq!(params.freq, Tenor::annual());
    /// assert_eq!(params.dc, DayCount::Act365F);
    /// assert_eq!(params.bdc, BusinessDayConvention::ModifiedFollowing);
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

    // ── JPY ──────────────────────────────────────────────────────────────────

    /// JPY TONA swap (annual, Act/365F, Modified Following, JPTO, T+2 payment lag).
    ///
    /// Follows BoJ TONA conventions.
    ///
    /// # Arguments
    ///
    /// None.
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
    /// assert_eq!(params.freq, Tenor::annual());
    /// assert_eq!(params.dc, DayCount::Act365F);
    /// assert_eq!(params.bdc, BusinessDayConvention::ModifiedFollowing);
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
}

/// Fixed-rate coupon window with a shared schedule.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FixedWindow {
    /// Annual coupon rate as a decimal, for example `0.05` for 5%.
    pub rate: Decimal,
    /// Schedule-generation parameters for this fixed-rate window.
    pub schedule: ScheduleParams,
}
