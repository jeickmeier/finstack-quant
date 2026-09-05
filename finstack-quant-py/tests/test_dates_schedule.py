"""Tests for ScheduleBuilder bindings.

NOTE: requires a rebuilt wheel (the binding default frequency changed from
quarterly to monthly to match Rust ``ScheduleBuilder::new``).
"""

from datetime import date

import pytest

from finstack_quant.core.dates import (
    BusinessDayConvention,
    DayCountContext,
    FiscalConfig,
    PeriodId,
    Schedule,
    ScheduleErrorPolicy,
    StubKind,
    available_calendars,
)


def test_default_frequency_is_monthly() -> None:
    """An unspecified frequency defaults to monthly, matching Rust."""
    builder = Schedule.builder(date(2025, 1, 15), date(2026, 1, 15))
    schedule = builder.build()
    # 12 monthly periods over one year -> 13 dates (start + 12 period ends).
    assert len(schedule) == 13


def test_imm_modes_use_last_call_wins() -> None:
    start = date(2025, 1, 15)
    end = date(2025, 9, 30)

    cds_builder = Schedule.builder(start, end)
    cds_builder.imm()
    cds_builder.cds_imm()
    cds_dates = cds_builder.build().dates

    imm_builder = Schedule.builder(start, end)
    imm_builder.cds_imm()
    imm_builder.imm()
    imm_dates = imm_builder.build().dates

    assert date(2025, 3, 20) in cds_dates
    assert date(2025, 3, 19) not in cds_dates
    assert date(2025, 3, 19) in imm_dates
    assert date(2025, 3, 20) not in imm_dates


def test_schedule_builder_setters_are_fluent_and_mutate_in_place() -> None:
    builder = Schedule.builder(date(2025, 1, 15), date(2026, 1, 15))

    result = (
        builder
        .frequency("3M")
        .stub_rule(StubKind.SHORT_BACK)
        .adjust_with(BusinessDayConvention.MODIFIED_FOLLOWING, "usny")
        .end_of_month(False)
        .imm()
        .cds_imm()
        .error_policy(ScheduleErrorPolicy.STRICT)
    )

    assert result is builder
    assert date(2025, 3, 20) in builder.build().dates


def test_schedule_payment_and_fixing_dates() -> None:
    schedule = (
        Schedule
        .builder(date(2025, 1, 2), date(2025, 1, 9))
        .frequency("1W")
        .adjust_with(BusinessDayConvention.FOLLOWING, "weekends_only")
        .payment_lag_business_days(2)
        .fixing_lag_business_days(2)
        .build()
    )
    assert schedule.dates == [date(2025, 1, 2), date(2025, 1, 9)]
    assert schedule.payment_dates == [date(2025, 1, 13)]
    assert schedule.fixing_dates == [date(2024, 12, 31)]


def test_schedule_builder_fluent_setter_preserves_exceptions() -> None:
    builder = Schedule.builder(date(2025, 1, 15), date(2026, 1, 15))
    with pytest.raises(ValueError, match=r"(?i)(tenor|parse|invalid)"):
        builder.frequency("not-a-tenor")


def test_fiscal_week_stepping_includes_week_53() -> None:
    fiscal = FiscalConfig.us_federal()
    week_52 = PeriodId.parse("FY2025W52")

    week_53 = week_52.next_fiscal(fiscal)
    assert week_53.code == "FY2025W53"
    assert week_53.is_fiscal
    assert PeriodId.parse(week_53.code) == week_53
    assert week_53.next_fiscal(fiscal).code == "FY2026W01"
    assert week_53.prev_fiscal(fiscal) == week_52

    assert PeriodId.parse("2025W52").next().code == "2026W01"

    with pytest.raises(ValueError, match="next_fiscal"):
        week_52.next()
    with pytest.raises(ValueError, match="prev_fiscal"):
        week_52.prev()


def test_schedule_builder_from_spec_and_dataframe() -> None:
    schedule = (
        Schedule.builder("2025-01-15", "2026-01-15").frequency("6M").adjust_with("modified_following", "usny").build()
    )
    assert [d.isoformat() for d in schedule] == ["2025-01-15", "2025-07-15", "2026-01-15"]
    assert schedule.payment_dates == [date(2025, 7, 15), date(2026, 1, 15)]
    assert Schedule.from_json(schedule.to_json()) == schedule

    spec = Schedule.builder("2025-01-15", "2025-07-15").frequency("3M").to_spec()
    assert spec["frequency"] == {"count": 3, "unit": "months"}
    assert len(Schedule.from_spec(spec)) == 3

    frame = schedule.to_dataframe()
    assert list(frame.columns) == ["period_start", "period_end", "payment_date", "fixing_date"]
    assert len(frame) == 2
    assert frame["fixing_date"].isna().all()


def test_schedule_builder_accepts_strings_and_calendar_objects() -> None:
    from finstack_quant.core.dates import HolidayCalendar

    schedule = (
        Schedule
        .builder("2025-01-15", "2026-01-15")
        .frequency("6M")
        .stub_rule("short_front")
        .adjust_with("MF", HolidayCalendar("nyse+gblo"))
        .error_policy("strict")
        .build()
    )
    assert len(schedule) == 3
    with pytest.raises(KeyError):
        Schedule.builder("2025-01-15", "2026-01-15").adjust_with("MF", "no_such_calendar").build()
    with pytest.raises(ValueError, match=r"start must be before end"):
        Schedule.builder("2026-01-15", "2025-01-15")


def test_schedule_warnings_are_typed_dicts() -> None:
    schedule = (
        Schedule
        .builder("2025-01-15", "2026-01-15")
        .adjust_with("following", "no_such_calendar")
        .error_policy("missing_calendar_warning")
        .build()
    )
    assert schedule.has_warnings()
    assert not schedule.used_graceful_fallback()
    (warning,) = schedule.warnings
    assert warning["kind"] == "missing_calendar_id"
    assert warning["calendar_id"] == "no_such_calendar"
    assert "no_such_calendar" in warning["message"]


def test_dates_pickle_round_trips() -> None:
    import pickle

    from finstack_quant.core.dates import (
        DayCount,
        DayCountContext,
        HolidayCalendar,
        Tenor,
        TenorUnit,
        Thirty360Convention,
        build_periods,
    )

    values = [
        Tenor("3M"),
        TenorUnit.MONTHS,
        DayCount.NL_365,
        PeriodId.parse("2025Q1"),
        HolidayCalendar("usny"),
        DayCountContext("usny", "6M", coupon_period=("2025-01-01", "2025-07-01")),
        DayCountContext(bus_basis=250),
        StubKind.LONG_BACK,
        BusinessDayConvention.MODIFIED_PRECEDING,
        ScheduleErrorPolicy.GRACEFUL_EMPTY,
        Thirty360Convention.ITALIAN,
        FiscalConfig.uk(),
        build_periods("2024Q1..Q4", "2024Q2"),
        Schedule.builder("2025-01-15", "2026-01-15").frequency("6M").build(),
    ]
    for value in values:
        assert pickle.loads(pickle.dumps(value)) == value  # noqa: S301


def test_period_plan_iteration_dataframe_and_ordering() -> None:
    from finstack_quant.core.dates import PeriodKind, build_periods

    plan = build_periods("2024Q1..Q4", "2024Q2")
    assert [p.id.code for p in plan] == ["2024Q1", "2024Q2", "2024Q3", "2024Q4"]
    frame = plan.to_dataframe()
    assert list(frame.columns) == ["id", "start", "end", "is_actual"]
    assert frame["is_actual"].tolist() == [True, True, False, False]
    assert PeriodId.parse("2024Q1") < PeriodId.parse("2024Q2") <= PeriodId.parse("2024Q2")
    assert sorted([PeriodId.parse("2025M02"), PeriodId.parse("2024M12")])[0].code == "2024M12"
    assert PeriodKind.QUARTERLY.prior_observation_date("2025-03-31") == date(2024, 12, 31)
    with pytest.raises(ValueError, match=r"'2024Q1\.\.2024M06'.*2024Q1\.\.Q4"):
        build_periods("2024Q1..2024M06")
    with pytest.raises(ValueError, match="13"):
        PeriodId.month(2025, month=13)
    assert PeriodId.half(2025, half=2).code == "2025H2"
    assert PeriodId.week(2025, week=2).code == "2025W02"


def test_tenor_constructor_forms_and_context_helpers() -> None:
    from finstack_quant.core.dates import DayCount, HolidayCalendar, Tenor, TenorUnit

    assert Tenor("3M") == Tenor(3, "M") == Tenor(3, TenorUnit.MONTHS)
    assert Tenor("ON") == Tenor.daily()
    assert Tenor("3M").payments_per_year() == 4.0
    assert str(Tenor.from_years(0.5, "act_365f")) == "6M"
    assert Tenor("1M").add_to_date("2025-01-31") == date(2025, 2, 28)
    assert Tenor("1M").add_to_date("2025-01-31", calendar="usny", business_day_convention="F") == date(2025, 2, 28)
    assert Tenor("1Y").to_years_with_context("2025-01-15", day_count=DayCount.ACT_ACT) == pytest.approx(1.0)
    with pytest.raises(TypeError):
        Tenor(3)

    assert DayCount.parse("ACT/ACT ICMA") == DayCount.ACT_ACT_ISMA
    with pytest.raises(ValueError, match="act_360"):
        DayCount.from_name("ACT/360")
    assert DayCount.ACT_ACT_ISMA.year_fraction("2025-01-15", "2025-07-15", frequency="6M") == pytest.approx(0.5)
    with pytest.raises(ValueError, match="not both"):
        DayCount.ACT_360.year_fraction("2025-01-15", "2025-07-15", DayCountContext(), frequency="6M")

    joint = HolidayCalendar("GBLO + nyse")
    assert joint.code == "gblo+nyse"
    assert joint.metadata is None
    assert not joint.is_business_day(date(2025, 7, 4))
    assert HolidayCalendar("USNY").code == "usny"
    assert HolidayCalendar("usny").count_business_days("2025-01-01", "2025-01-08") == 4
    assert str(BusinessDayConvention.from_name("MF")) == "modified_following"
    assert "weekends_only" in available_calendars()


def test_thirty360_and_date_extension_free_functions() -> None:
    from finstack_quant.core.dates import (
        Thirty360Convention,
        add_business_days,
        add_months,
        add_weekdays,
        days_30_360,
        days_30e_360_isda,
        end_of_month,
        fiscal_year,
        is_weekend,
        months_until,
        quarter,
    )

    assert days_30_360("2025-01-31", "2025-03-31", "isda") == 60
    assert days_30_360("2025-01-31", "2025-03-31", Thirty360Convention.US_SIA) == 60
    assert days_30e_360_isda("2024-01-31", "2024-02-29", False) == 30
    assert Thirty360Convention.from_name("ISDA") == Thirty360Convention.ISDA
    assert add_business_days(date(2025, 6, 27), 3, "target2") == date(2025, 7, 2)
    assert add_weekdays("2025-01-03", 1) == date(2025, 1, 6)
    assert add_months("2024-01-31", 1) == date(2024, 2, 29)
    assert end_of_month("2024-02-15") == date(2024, 2, 29)
    assert is_weekend("2025-01-04")
    assert quarter("2025-08-15") == 3
    assert fiscal_year("2024-10-15", FiscalConfig.us_federal()) == 2025
    assert months_until("2020-01-15", "2022-03-10") == 25


def test_empty_schedule_dataframe() -> None:
    schedule = Schedule.from_json('{"dates":[],"payment_dates":[],"fixing_dates":[],"warnings":[]}')
    frame = schedule.to_dataframe()
    assert frame.empty
    assert list(frame.columns) == ["period_start", "period_end", "payment_date", "fixing_date"]
