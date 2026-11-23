use super::convention::*;
use super::*;
use crate::dates::Tenor;

mod parse_tests {
    use super::DayCount;

    /// Every canonical label, in declaration order.
    const NAMES: &[&str] = &[
        "one_one",
        "act_360",
        "act_365f",
        "act_365l",
        "30_360",
        "30_360_isda",
        "30e_360",
        "30e_360_isda",
        "30_360_it",
        "nl_365",
        "act_act",
        "act_act_isma",
        "act_act_afb",
        "bus_252",
    ];

    #[test]
    fn canonical_parse_normalizes_only_case_and_surrounding_whitespace() {
        for name in NAMES {
            let normalized = format!("  {}  ", name.to_ascii_uppercase());
            assert_eq!(
                normalized.parse::<DayCount>().expect(name).to_string(),
                *name
            );
        }
    }

    #[test]
    fn from_str_lists_names_for_unknown_spellings() {
        let err = "nope".parse::<DayCount>().expect_err("unknown").to_string();
        assert!(err.contains("'nope'") && err.contains("act_360") && err.contains("bus_252"));
        for name in NAMES {
            assert_eq!(&name.parse::<DayCount>().expect(name).to_string(), name);
        }
    }
}
mod unit_tests {
    use super::DayCount;
    use crate::InputError;
    use crate::dates::{DayCountContext, Tenor, TenorUnit};
    use time::Date;
    use time::macros::date;

    #[test]
    fn equal_dates_validate_context_before_returning_zero() {
        let day = Date::from_calendar_date(2025, time::Month::March, 3).expect("valid date");
        let zero_basis = DayCountContext {
            bus_basis: Some(0),
            ..DayCountContext::default()
        };
        let inverted = DayCountContext {
            coupon_period: Some((day, day)),
            ..DayCountContext::default()
        };
        for convention in [DayCount::Act360, DayCount::OneOne, DayCount::ActAct] {
            assert!(
                convention.year_fraction(day, day, zero_basis).is_err(),
                "{convention:?}"
            );
            assert!(
                convention.signed_year_fraction(day, day, inverted).is_err(),
                "{convention:?}"
            );
            assert_eq!(
                convention
                    .year_fraction(day, day, DayCountContext::default())
                    .expect("context-free"),
                0.0
            );
        }
        assert!(matches!(
            DayCount::Bus252.year_fraction(day, day, DayCountContext::default()),
            Err(crate::Error::Input(InputError::MissingCalendarForBus252))
        ));
        assert!(
            DayCount::Act365L
                .signed_year_fraction(day, day, DayCountContext::default())
                .is_err()
        );
    }

    #[test]
    fn bus252_rejects_a_count_beyond_elapsed_days() {
        struct OverCounting;
        impl crate::dates::HolidayCalendar for OverCounting {
            fn is_holiday(&self, _date: Date) -> crate::Result<bool> {
                Ok(false)
            }

            fn is_business_day(&self, _date: Date) -> crate::Result<bool> {
                Ok(true)
            }

            fn count_business_days(&self, _start: Date, _end: Date) -> crate::Result<i32> {
                Ok(10)
            }
        }
        let start = Date::from_calendar_date(2025, time::Month::March, 3).expect("valid date");
        let end = Date::from_calendar_date(2025, time::Month::March, 5).expect("valid date");
        let ctx = DayCountContext {
            calendar: Some(&OverCounting),
            ..DayCountContext::default()
        };
        assert!(DayCount::Bus252.year_fraction(start, end, ctx).is_err());
    }

    #[test]
    fn act_act_isda_spans_the_supported_date_range() {
        let start = Date::from_calendar_date(-9999, time::Month::January, 1).expect("min year");
        let end = Date::from_calendar_date(9999, time::Month::December, 31).expect("max year");
        let yf = DayCount::ActAct
            .year_fraction(start, end, DayCountContext::default())
            .expect("whole range");
        // 19998 complete years plus 364 of 365 days in 9999.
        assert_eq!(yf, 19_998.0 + 364.0 / 365.0);
    }

    #[test]
    fn act_act_isma_requires_contained_interval_including_equal_dates() {
        let period = (date!(2025 - 01 - 15), date!(2025 - 07 - 15));
        let ctx = DayCountContext {
            frequency: Some(Tenor::semi_annual()),
            coupon_period: Some(period),
            ..DayCountContext::default()
        };
        assert_eq!(
            DayCount::ActActIsma
                .year_fraction(period.0, period.1, ctx)
                .expect("contained full period"),
            0.5
        );
        assert_eq!(
            DayCount::ActActIsma
                .year_fraction(period.0, period.0, ctx)
                .expect("equal date inside period"),
            0.0
        );
        for (start, end) in [
            (date!(2025 - 01 - 14), period.1),
            (period.0, date!(2025 - 07 - 16)),
            (date!(2025 - 07 - 16), date!(2025 - 07 - 16)),
            (date!(1700 - 01 - 01), date!(1700 - 01 - 02)),
        ] {
            assert!(
                DayCount::ActActIsma.year_fraction(start, end, ctx).is_err(),
                "accrual [{start}, {end}] outside coupon_period must fail"
            );
        }
    }

    #[test]
    fn act_act_isma_accepts_eom_and_clamped_nominal_periods() {
        let feb_period = (date!(2024 - 08 - 31), date!(2025 - 02 - 28));
        let aug_period = (date!(2025 - 02 - 28), date!(2025 - 08 - 31));
        let feb_ctx = DayCountContext {
            frequency: Some(Tenor::semi_annual()),
            coupon_period: Some(feb_period),
            ..DayCountContext::default()
        };
        let aug_ctx = DayCountContext {
            frequency: Some(Tenor::semi_annual()),
            coupon_period: Some(aug_period),
            ..DayCountContext::default()
        };

        let front_stub = DayCount::ActActIsma
            .year_fraction(date!(2024 - 10 - 15), date!(2025 - 02 - 28), feb_ctx)
            .expect("contained front stub");
        let back_stub = DayCount::ActActIsma
            .year_fraction(date!(2025 - 02 - 28), date!(2025 - 05 - 15), aug_ctx)
            .expect("contained back stub");

        let expected_front = (date!(2025 - 02 - 28) - date!(2024 - 10 - 15)).whole_days() as f64
            / (2.0 * (feb_period.1 - feb_period.0).whole_days() as f64);
        let expected_back = (date!(2025 - 05 - 15) - date!(2025 - 02 - 28)).whole_days() as f64
            / (2.0 * (aug_period.1 - aug_period.0).whole_days() as f64);

        assert!((front_stub - expected_front).abs() < 1e-14);
        assert!((back_stub - expected_back).abs() < 1e-14);
    }

    #[test]
    fn act_act_isma_rejects_unsupported_frequencies() {
        for bad in [
            Tenor::daily(),
            Tenor::weekly(),
            Tenor::new(5, TenorUnit::Months).expect("5M"),
            Tenor::new(2, TenorUnit::Years).expect("2Y"),
        ] {
            let ctx = DayCountContext {
                frequency: Some(bad),
                coupon_period: Some((date!(2025 - 01 - 15), date!(2025 - 07 - 15))),
                ..DayCountContext::default()
            };
            assert!(matches!(
                DayCount::ActActIsma.validate_context(ctx),
                Err(crate::Error::Input(
                    InputError::ActActIsmaUnsupportedFrequency { .. }
                ))
            ));
        }
    }

    // FromStr / Display roundtrip tests

    #[test]
    fn daycount_display_roundtrip() {
        let all = [
            super::DayCount::Act360,
            super::DayCount::Act365F,
            super::DayCount::Act365L,
            super::DayCount::Nl365,
            super::DayCount::Thirty360,
            super::DayCount::Thirty360Isda,
            super::DayCount::ThirtyE360,
            super::DayCount::ThirtyE360Isda,
            super::DayCount::Thirty360It,
            super::DayCount::ActAct,
            super::DayCount::ActActIsma,
            super::DayCount::ActActAfb,
            super::DayCount::Bus252,
        ];

        for day_count in &all {
            let label = day_count.to_string();
            assert!(
                matches!(label.parse::<super::DayCount>(), Ok(value) if value == *day_count),
                "roundtrip failed for {label}"
            );
        }
    }

    #[test]
    fn daycount_from_str_rejects_retired_spellings() {
        // schema-rejection-test: retired spellings must never regain parser support.
        for rejected in [
            "1/1",
            "ACT/360",
            "act360",
            "actual_360",
            "act_365",
            "act_365_fixed",
            "actual_365",
            "act365f",
            "actual_365l",
            "act_365_nl",
            "30/360",
            "30_360_us",
            "30_360_bond",
            "30u_360",
            "30_360_eurobond",
            "30_360_italian",
            "30E/360",
            "thirty360",
            "bond_basis",
            "eurobond_basis",
            "act_365afb",
            "isda",
            "icma",
            "act_act_icma",
            "act_act_isda",
            "actual_actual",
            "bus252",
            "business_252",
            "act 360",
            "act-360",
            "act__360",
        ] {
            assert!(rejected.parse::<DayCount>().is_err(), "{rejected}");
        }
    }

    #[test]
    fn thirty_360_isda_selects_bond_basis_not_30e_360_isda() {
        let start = time::macros::date!(2025 - 02 - 28);
        let end = time::macros::date!(2025 - 03 - 31);
        let bond_basis: DayCount = "30_360_isda".parse().expect("canonical");
        assert_eq!(bond_basis, DayCount::Thirty360Isda);
        let ordinary = DayCountContext {
            end_is_termination_date: Some(false),
            ..DayCountContext::default()
        };
        assert_eq!(
            bond_basis
                .year_fraction(start, end, DayCountContext::default())
                .expect("bond basis needs no termination status"),
            33.0 / 360.0
        );
        let european_isda: DayCount = "30e_360_isda".parse().expect("canonical");
        assert_eq!(
            european_isda
                .year_fraction(start, end, ordinary)
                .expect("fraction"),
            30.0 / 360.0
        );
    }

    #[test]
    fn daycount_from_str_unknown() {
        assert!("garbage".parse::<super::DayCount>().is_err());
    }

    // ACT/365L (ISDA coupon-end-year rule, record 002 #5) boundary tests

    fn act365l(start: Date, end: Date, period: (Date, Date)) -> crate::Result<f64> {
        DayCount::Act365L.year_fraction(
            start,
            end,
            DayCountContext {
                coupon_period: Some(period),
                ..DayCountContext::default()
            },
        )
    }

    #[test]
    fn act365l_whole_annual_periods_use_the_end_year() {
        // 2023 has no Feb 29 but the period ends in leap year 2024.
        let (s, e) = (date!(2023 - 01 - 01), date!(2024 - 01 - 01));
        assert_eq!(act365l(s, e, (s, e)).expect("annual"), 365.0 / 366.0);
        // 2024 contains Feb 29 but the period ends in 2025.
        let (s, e) = (date!(2024 - 01 - 01), date!(2025 - 01 - 01));
        assert_eq!(act365l(s, e, (s, e)).expect("annual"), 366.0 / 365.0);
    }

    #[test]
    fn act365l_partial_slice_keeps_the_period_denominator() {
        let period = (date!(2023 - 07 - 01), date!(2024 - 07 - 01));
        let yf = act365l(date!(2023 - 07 - 01), date!(2023 - 12 - 01), period).expect("partial");
        assert_eq!(yf, 153.0 / 366.0);
    }

    #[test]
    fn act365l_needs_no_frequency_and_rejects_slices_outside_the_period() {
        let (s, e) = (date!(2024 - 02 - 29), date!(2024 - 03 - 15));
        assert_eq!(act365l(s, e, (s, e)).expect("no frequency"), 15.0 / 366.0);
        let outside = date!(2024 - 04 - 01);
        assert!(act365l(e, outside, (s, e)).is_err());
        // Containment is checked before the equal-date zero.
        assert!(act365l(outside, outside, (s, e)).is_err());
        assert_eq!(act365l(e, e, (s, e)).expect("inside"), 0.0);
        assert!(
            DayCount::Act365L
                .year_fraction(e, e, DayCountContext::default())
                .is_err()
        );
    }

    // 30E/360 (ISDA) — ISDA 2006 §4.16(h) examples

    #[test]
    fn thirty_e_360_isda_last_day_of_month_rules() {
        use super::thirty360::thirty_e_360_isda_days as days_30e_360_isda;
        use super::{DayCount, DayCountContext};

        // Aug 31 → 30 (last day of month), Feb 29 → 30 (last day of Feb,
        // not termination): 360 + 30·(2-8) + (30-30) = 180.
        let start = date!(2011 - 08 - 31);
        let end = date!(2012 - 02 - 29);
        assert_eq!(days_30e_360_isda(start, end, false), 180);

        // Same period as the final period to maturity: Feb 29 kept → 179.
        assert_eq!(days_30e_360_isda(start, end, true), 179);

        // Feb 29 2012 → 30 (last day of Feb as D1), Aug 31 → 30:
        // 30·(8-2) + (30-30) = 180.
        let start = date!(2012 - 02 - 29);
        let end = date!(2012 - 08 - 31);
        assert_eq!(days_30e_360_isda(start, end, false), 180);

        // Non-leap end-of-Feb: Jan 28 (not last day) kept, Feb 28 → 30
        // (intermediate): 30 + (30-28) = 32; termination: 30 + (28-28) = 30.
        let start = date!(2011 - 01 - 28);
        let end = date!(2011 - 02 - 28);
        assert_eq!(days_30e_360_isda(start, end, false), 32);
        assert_eq!(days_30e_360_isda(start, end, true), 30);

        // The enum variant routes through context for the termination form,
        // which must be explicit, including for equal dates.
        let start = date!(2011 - 08 - 31);
        let end = date!(2012 - 02 - 29);
        let status = |terminal| DayCountContext {
            end_is_termination_date: Some(terminal),
            ..DayCountContext::default()
        };
        let yf = DayCount::ThirtyE360Isda
            .year_fraction(start, end, status(false))
            .expect("should succeed");
        assert_eq!(yf, 180.0 / 360.0);
        let terminal_yf = DayCount::ThirtyE360Isda
            .year_fraction(start, end, status(true))
            .expect("termination-period day count should succeed");
        assert_eq!(terminal_yf, 179.0 / 360.0);
        for (a, b) in [(start, end), (end, end)] {
            assert!(
                DayCount::ThirtyE360Isda
                    .year_fraction(a, b, DayCountContext::default())
                    .is_err()
            );
        }
        // Signed reversal keeps the later endpoint's termination status.
        assert_eq!(
            DayCount::ThirtyE360Isda
                .signed_year_fraction(end, start, status(true))
                .expect("signed"),
            -179.0 / 360.0
        );
    }

    #[test]
    fn thirty_e_360_isda_differs_from_european_on_end_of_february() {
        use super::thirty360::{
            Thirty360Variant, thirty_360_days as days_30_360,
            thirty_e_360_isda_days as days_30e_360_isda,
        };

        // 30E/360 (European, §4.16(g)) has NO end-of-February rule;
        // 30E/360 (ISDA, §4.16(h)) adjusts end-of-Feb to 30.
        let start = date!(2012 - 02 - 29);
        let end = date!(2012 - 03 - 31);
        // European: D1=29 kept, D2=31→30: 30 + (30-29) = 31.
        assert_eq!(days_30_360(start, end, Thirty360Variant::European), 31);
        // ISDA: D1=29→30 (last day of Feb), D2=31→30: 30 + (30-30) = 30.
        assert_eq!(days_30e_360_isda(start, end, false), 30);
    }

    #[test]
    fn thirty_360_italian_feb_after_27_and_day_31() {
        use super::thirty360::{Thirty360Variant, thirty_360_days as days_30_360};

        // Jan 31 → Feb 28 2025: Italian D1=30, D2=30 → 30.
        // European D2 stays 28; US SIA D2 stays 28 (not both Feb EOM).
        let start = date!(2025 - 01 - 31);
        let end = date!(2025 - 02 - 28);
        assert_eq!(days_30_360(start, end, Thirty360Variant::Italian), 30);
        assert_eq!(days_30_360(start, end, Thirty360Variant::European), 28);
        assert_eq!(days_30_360(start, end, Thirty360Variant::UsEom), 28);

        // Leap: Feb 29 → Mar 31 2024: Italian D1=30 (Feb>27), D2=30 → 30.
        let start = date!(2024 - 02 - 29);
        let end = date!(2024 - 03 - 31);
        assert_eq!(days_30_360(start, end, Thirty360Variant::Italian), 30);
    }

    // NL/365

    #[test]
    fn nl365_excludes_end_leap_day_but_includes_start_leap_day() {
        use super::{DayCount, DayCountContext};

        let year_fraction = |start, end| {
            DayCount::Nl365
                .year_fraction(start, end, DayCountContext::default())
                .expect("valid interval")
        };
        assert_eq!(
            year_fraction(date!(2024 - 02 - 28), date!(2024 - 02 - 29)),
            0.0
        );
        assert_eq!(
            year_fraction(date!(2024 - 02 - 29), date!(2024 - 03 - 01)),
            1.0 / 365.0
        );
        // Adjacent subperiods must sum to the unsplit accrual.
        assert_eq!(
            year_fraction(date!(2024 - 02 - 28), date!(2024 - 02 - 29))
                + year_fraction(date!(2024 - 02 - 29), date!(2024 - 03 - 01)),
            year_fraction(date!(2024 - 02 - 28), date!(2024 - 03 - 01))
        );
    }

    #[test]
    fn nl365_excludes_feb_29() {
        use super::{DayCount, DayCountContext};

        // Full leap year: 366 actual days − 1 leap day = 365 → exactly 1.0.
        let start = date!(2024 - 01 - 01);
        let end = date!(2025 - 01 - 01);
        let yf = DayCount::Nl365
            .year_fraction(start, end, DayCountContext::default())
            .expect("should succeed");
        assert_eq!(yf, 1.0);

        // Feb 28 → Mar 1 in a leap year: 2 actual days − Feb 29 = 1 day.
        let start = date!(2024 - 02 - 28);
        let end = date!(2024 - 03 - 01);
        let yf = DayCount::Nl365
            .year_fraction(start, end, DayCountContext::default())
            .expect("should succeed");
        assert_eq!(yf, 1.0 / 365.0);

        // Non-leap year: identical to Act/365F.
        let start = date!(2025 - 01 - 01);
        let end = date!(2025 - 07 - 01);
        let nl = DayCount::Nl365
            .year_fraction(start, end, DayCountContext::default())
            .expect("should succeed");
        let act365f = DayCount::Act365F
            .year_fraction(start, end, DayCountContext::default())
            .expect("should succeed");
        assert_eq!(nl, act365f);
    }

    // DayCountContextState coupon_period round-trip

    #[test]
    fn daycount_context_state_roundtrips_coupon_period() {
        use super::{DayCountContext, DayCountContextState};
        use crate::dates::Tenor;

        let coupon = (date!(2025 - 01 - 15), date!(2025 - 07 - 15));
        let ctx = DayCountContext {
            frequency: Some(Tenor::semi_annual()),
            coupon_period: Some(coupon),
            ..Default::default()
        };

        // Context → state → JSON → state → context preserves coupon_period.
        let state = DayCountContextState::try_from(ctx).expect("snapshot context");
        assert_eq!(state.get_coupon_period(), Some(coupon));

        let json = serde_json::to_string(&state).expect("serialize");
        let restored: DayCountContextState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.get_coupon_period(), Some(coupon));

        let restored_ctx = restored.to_ctx().expect("calendar state hydrates");
        assert_eq!(restored_ctx.coupon_period, Some(coupon));

        // An omitted termination status is unspecified, not false: 30E/360
        // ISDA rejects it, including for equal dates.
        let omitted =
            r#"{"calendar_id":null,"frequency":null,"bus_basis":null,"coupon_period":null}"#;
        let state: DayCountContextState = serde_json::from_str(omitted).expect("omission is valid");
        assert_eq!(state.get_end_is_termination_date(), None);
        let day = date!(2025 - 02 - 28);
        assert!(
            super::DayCount::ThirtyE360Isda
                .year_fraction(day, day, state.to_ctx().expect("context"))
                .is_err()
        );
    }

    #[test]
    fn daycount_context_state_rejects_unknown_calendar() {
        let state = super::DayCountContextState::try_new(
            Some("not_a_calendar".to_string()),
            None,
            None,
            None,
            None,
        )
        .expect("calendar lookup is deferred to to_ctx");

        let error = state
            .to_ctx()
            .expect_err("unknown persisted calendar must fail hydration");
        assert!(error.to_string().contains("not_a_calendar"));
    }

    // Act/Act ISMA coupon_period routing tests

    #[test]
    fn act_act_isma_coupon_period_mid_coupon_accrual() {
        use super::{DayCount, DayCountContext, Tenor};

        let coupon_start = date!(2025 - 01 - 15);
        let coupon_end = date!(2025 - 07 - 15);

        // Mid-coupon accrual: settlement to next coupon
        let settlement = date!(2025 - 03 - 15);
        let frequency = Tenor::semi_annual();

        // With coupon_period: uses the explicit reference period
        let ctx_with = DayCountContext {
            frequency: Some(frequency),
            coupon_period: Some((coupon_start, coupon_end)),
            ..Default::default()
        };
        let yf_with = DayCount::ActActIsma
            .year_fraction(settlement, coupon_end, ctx_with)
            .expect("should succeed with coupon_period");

        // Without coupon_period an irregular span must error rather than
        // re-anchoring a quasi-coupon grid on settlement.
        let ctx_without = DayCountContext {
            frequency: Some(frequency),
            ..Default::default()
        };
        let err = DayCount::ActActIsma
            .year_fraction(settlement, coupon_end, ctx_without)
            .expect_err("irregular Act/Act ICMA without coupon_period must fail");
        assert!(
            err.to_string().contains("coupon_period"),
            "unexpected error: {err}"
        );

        // With reference period: 122 days / 181 days × 0.5 ≈ 0.33702
        let expected_days = (coupon_end - settlement).whole_days() as f64;
        let ref_days = (coupon_end - coupon_start).whole_days() as f64;
        let expected = (expected_days / ref_days) * 0.5;
        assert!(
            (yf_with - expected).abs() < 1e-10,
            "coupon_period path: {yf_with} vs expected {expected}"
        );
    }
}
