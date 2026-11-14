"""Tests for ScheduleBuilder bindings.

NOTE: requires a rebuilt wheel (the binding default frequency changed from
quarterly to monthly to match Rust ``Schedule::builder``).
"""

from datetime import date

import pytest

from finstack_quant.core.dates import (
    BusinessDayConvention,
    DayCountContext,
    FiscalConfig,
    PeriodId,
    Schedule,
    StubKind,
    available_calendars,
)


def test_default_frequency_is_monthly() -> None:
    """An unspecified frequency defaults to monthly, matching Rust."""
    builder = (
        Schedule
        .builder(date(2025, 1, 15), date(2026, 1, 15))
        .frequency("1M")
        .stub_rule("none")
        .grid("natural")
        .unadjusted()
    )
    schedule = builder.build()
    # 12 monthly periods over one year -> 13 dates (start + 12 period ends).
    assert len(schedule) == 13


def test_eom_regular_schedule_uses_the_month_end_roll_grid() -> None:
    schedule = (
        Schedule
        .builder(date(2025, 2, 28), date(2025, 8, 31))
        .stub_rule("none")
        .unadjusted()
        .frequency("3M")
        .grid("end_of_month")
        .build()
    )
    assert schedule.dates == [date(2025, 2, 28), date(2025, 5, 31), date(2025, 8, 31)]
    # Without a stub both endpoints must be on the month-end grid.
    with pytest.raises(ValueError, match="month-end"):
        Schedule.builder(date(2025, 2, 28), date(2025, 8, 28)).stub_rule("none").unadjusted().frequency("3M").grid(
            "end_of_month"
        ).build()


def test_nl365_uses_exclusive_start_inclusive_end_leap_days() -> None:
    from finstack_quant.core.dates import DayCount

    assert DayCount.NL_365.year_fraction("2024-02-28", "2024-02-29") == 0.0
    assert DayCount.NL_365.year_fraction("2024-02-29", "2024-03-01") == pytest.approx(1.0 / 365.0)


def test_fiscal_boundaries_and_actuals_cutoffs_are_consistent() -> None:
    from finstack_quant.core.dates import build_fiscal_periods, build_periods, fiscal_year

    january = FiscalConfig(1, 2)
    year = fiscal_year(date(2025, 1, 2), january)
    period = build_fiscal_periods(f"FY{year}..FY{year}", january).periods[0]
    assert period.start == date(2025, 1, 2)
    assert period.end == date(2026, 1, 2)
    leap = build_fiscal_periods("FY2024M12..FY2025M01", FiscalConfig(2, 29)).periods
    assert leap[0].end == leap[1].start == date(2024, 2, 29)
    with pytest.raises(ValueError, match="same period kind and calendar"):
        build_periods("2025M01..M04", "2025Q1")


@pytest.mark.parametrize("stub", ["short_back", "long_back"])
@pytest.mark.parametrize("frequency", ["1M", "1Y", "1W", "2D"])
def test_back_stub_terminates_at_python_date_max(stub: str, frequency: str) -> None:
    start = date(9999, 12, 30)
    schedule = (
        Schedule.builder(start, date.max).grid("natural").unadjusted().frequency(frequency).stub_rule(stub).build()
    )
    assert schedule.dates == [start, date.max]


@pytest.mark.parametrize("stub", ["long_back", "long_front"])
@pytest.mark.parametrize(
    "expected",
    [
        [date(9999, 10, 31), date(9999, 11, 30), date.max],
        [date.min, date(1, 2, 1), date(1, 3, 1)],
    ],
)
def test_aligned_long_stubs_preserve_boundary_anchors(stub: str, expected: list[date]) -> None:
    schedule = (
        Schedule.builder(expected[0], expected[-1]).grid("natural").unadjusted().frequency("1M").stub_rule(stub).build()
    )
    assert schedule.dates == expected


def test_schedule_builder_has_no_cds_mode() -> None:
    # Record CORE-CONVENTIONS-008: no generic CDS mode or start substitution.
    builder = Schedule.builder(date(2025, 1, 15), date(2026, 1, 15))
    assert not hasattr(builder, "cds_imm")


def test_schedule_builder_setters_are_fluent_and_mutate_in_place() -> None:
    builder = (
        Schedule
        .builder(date(2025, 1, 15), date(2026, 1, 15))
        .frequency("1M")
        .stub_rule("none")
        .grid("natural")
        .unadjusted()
    )

    result = (
        builder
        .frequency("3M")
        .stub_rule(StubKind.SHORT_BACK)
        .adjust_with(BusinessDayConvention.MODIFIED_FOLLOWING, "usny")
        .grid("natural")
    )

    assert result is builder
    assert date(2025, 4, 15) in builder.build().dates


def test_schedule_payment_and_fixing_dates() -> None:
    schedule = (
        Schedule
        .builder(date(2025, 1, 2), date(2025, 1, 9))
        .frequency("1W")
        .stub_rule("none")
        .grid("natural")
        .adjust_with(BusinessDayConvention.FOLLOWING, "weekends_only")
        .payment_lag_days(2)
        .fixing_lag_business_days(2)
        .build()
    )
    assert schedule.dates == [date(2025, 1, 2), date(2025, 1, 9)]
    assert schedule.payment_dates == [date(2025, 1, 13)]
    assert schedule.fixing_dates == [date(2024, 12, 31)]


def test_schedule_builder_fluent_setter_preserves_exceptions() -> None:
    builder = (
        Schedule
        .builder(date(2025, 1, 15), date(2026, 1, 15))
        .frequency("1M")
        .stub_rule("none")
        .grid("natural")
        .unadjusted()
    )
    with pytest.raises(ValueError, match=r"(?i)(tenor|parse|invalid)"):
        builder.frequency("not-a-tenor")


def test_fiscal_week_stepping_includes_week_53() -> None:
    fiscal = {"fiscal": {"start_month": 10, "start_day": 1}}
    week_52 = PeriodId.parse("FY2025W52")

    week_53 = week_52.shift(1, fiscal)
    assert str(week_53) == "FY2025W53"
    assert week_53.is_fiscal
    assert PeriodId.parse(str(week_53)) == week_53
    assert str(week_53.shift(1, fiscal)) == "FY2026W01"
    assert PeriodId.parse("FY2026W01").shift(-1, fiscal) == week_53

    assert str(PeriodId.parse("2025W52").shift(1, "gregorian")) == "2026W01"
    assert str(PeriodId.parse("FY2025D365").shift(1, fiscal)) == "FY2026D001"


def test_shift_rejects_a_mismatched_calendar() -> None:
    with pytest.raises(ValueError, match="calendar"):
        PeriodId.parse("FY2025Q1").shift(1, "gregorian")
    with pytest.raises(ValueError, match="calendar"):
        PeriodId.quarter(2025, 1).shift(1, {"fiscal": {"start_month": 10, "start_day": 1}})


def test_schedule_builder_from_spec_and_dataframe() -> None:
    schedule = (
        Schedule
        .builder("2025-01-15", "2026-01-15")
        .stub_rule("none")
        .grid("natural")
        .frequency("6M")
        .adjust_with("modified_following", "usny")
        .build()
    )
    assert [d.isoformat() for d in schedule] == ["2025-01-15", "2025-07-15", "2026-01-15"]
    assert schedule.payment_dates == [date(2025, 7, 15), date(2026, 1, 15)]
    assert Schedule.from_json(schedule.to_json()) == schedule

    spec = (
        Schedule
        .builder("2025-01-15", "2025-07-15")
        .stub_rule("none")
        .grid("natural")
        .unadjusted()
        .frequency("3M")
        .to_spec()
    )
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
        .grid("natural")
        .adjust_with("modified_following", HolidayCalendar("nyse+gblo"))
        .build()
    )
    assert len(schedule) == 3
    with pytest.raises(KeyError):
        Schedule.builder("2025-01-15", "2026-01-15").frequency("1M").stub_rule("none").grid("natural").adjust_with(
            "modified_following", "no_such_calendar"
        ).build()
    with pytest.raises(ValueError, match=r"start must be before end"):
        Schedule.builder("2026-01-15", "2025-01-15").frequency("1M").stub_rule("none").grid("natural").unadjusted()


def test_schedule_has_no_lenient_policy_or_warnings() -> None:
    # A build returns a complete schedule or raises (CORE-CONVENTIONS-008 #12).
    builder = (
        Schedule.builder("2025-01-15", "2026-01-15").frequency("1M").stub_rule("none").grid("natural").unadjusted()
    )
    assert not hasattr(builder, "error_policy")
    with pytest.raises(KeyError):
        builder.adjust_with("following", "no_such_calendar").build()
    schedule = (
        Schedule
        .builder("2025-01-15", "2026-01-15")
        .stub_rule("none")
        .grid("natural")
        .unadjusted()
        .frequency("6M")
        .build()
    )
    assert not hasattr(schedule, "warnings")


def test_dates_pickle_round_trips() -> None:
    import pickle

    from finstack_quant.core.dates import (
        DayCount,
        HolidayCalendar,
        Tenor,
        TenorUnit,
        build_periods,
    )

    values = [
        Tenor.parse("3M"),
        TenorUnit.MONTHS,
        DayCount.NL_365,
        PeriodId.parse("2025Q1"),
        HolidayCalendar("usny"),
        DayCountContext("usny", "6M", coupon_period=("2025-01-01", "2025-07-01")),
        DayCountContext(bus_basis=250),
        StubKind.LONG_BACK,
        BusinessDayConvention.MODIFIED_PRECEDING,
        DayCount.THIRTY_360_ISDA,
        FiscalConfig(4, 6),
        build_periods("2024Q1..Q4", "2024Q2"),
        Schedule
        .builder("2025-01-15", "2026-01-15")
        .stub_rule("none")
        .grid("natural")
        .unadjusted()
        .frequency("6M")
        .build(),
    ]
    for value in values:
        assert pickle.loads(pickle.dumps(value)) == value  # noqa: S301


def test_period_plan_iteration_dataframe_and_ordering() -> None:
    from finstack_quant.core.dates import PeriodKind, build_periods

    plan = build_periods("2024Q1..Q4", "2024Q2")
    assert [str(p.id) for p in plan] == ["2024Q1", "2024Q2", "2024Q3", "2024Q4"]
    frame = plan.to_dataframe()
    assert list(frame.columns) == ["id", "start", "end", "is_actual"]
    assert frame["is_actual"].tolist() == [True, True, False, False]
    assert PeriodId.parse("2024Q1") < PeriodId.parse("2024Q2") <= PeriodId.parse("2024Q2")
    assert str(sorted([PeriodId.parse("2025M02"), PeriodId.parse("2024M12")])[0]) == "2024M12"
    with pytest.raises(ValueError, match=r"'2024Q1\.\.2024M06'.*2024Q1\.\.Q4"):
        build_periods("2024Q1..2024M06")
    with pytest.raises(ValueError, match="13"):
        PeriodId.month(2025, month=13)
    assert str(PeriodId.half(2025, half=2)) == "2025H2"
    assert str(PeriodId.week(2025, week=2)) == "2025W02"
    assert PeriodKind.DAILY.annualization_factor == 252
    assert PeriodKind.QUARTERLY.slots_per_year == 4
    assert PeriodKind.WEEKLY.slots_per_year is None


def test_tenor_constructor_forms_and_context_helpers() -> None:
    from finstack_quant.core.dates import DayCount, HolidayCalendar, Tenor, TenorUnit

    assert Tenor.parse("3M") == Tenor(3, "M") == Tenor(3, TenorUnit.MONTHS)
    assert Tenor.parse(" 1d ") == Tenor.daily()
    assert Tenor.parse("3M").payments_per_year() == 4.0
    assert str(Tenor.from_years(0.5, "act_365f")) == "6M"
    assert Tenor.parse("1M").add_to_date("2025-01-31", business_day_convention="unadjusted") == date(2025, 2, 28)
    assert Tenor.parse("1M").add_to_date("2025-01-31", calendar="usny", business_day_convention="following") == date(
        2025, 2, 28
    )
    assert Tenor.parse("1Y").to_years_with_context(
        "2025-01-15", day_count=DayCount.ACT_ACT, business_day_convention="unadjusted"
    ) == pytest.approx(1.0)
    with pytest.raises(TypeError):
        Tenor(3)

    assert DayCount.from_name(" ACT_ACT_ISMA ") == DayCount.ACT_ACT_ISMA
    assert DayCount.from_name(" ACT_360 ") == DayCount.ACT_360
    assert DayCount.ACT_ACT_ISMA.year_fraction(
        "2025-01-15",
        "2025-07-15",
        DayCountContext(frequency="6M", coupon_period=("2025-01-15", "2025-07-15")),
    ) == pytest.approx(0.5)

    joint = HolidayCalendar("GBLO + nyse")
    assert str(joint) == "gblo+nyse"
    assert joint.metadata is None
    assert not joint.is_business_day(date(2025, 7, 4))
    assert str(HolidayCalendar("USNY")) == "usny"
    assert HolidayCalendar("usny").count_business_days("2025-01-01", "2025-01-08") == 4
    assert str(BusinessDayConvention.from_name(" MODIFIED_FOLLOWING ")) == "modified_following"
    assert "weekends_only" in available_calendars()


def test_thirty360_and_date_extension_free_functions() -> None:
    from finstack_quant.core.dates import (
        DayCount,
        DayCountContext,
        add_business_days,
        add_months,
        add_weekdays,
        end_of_month,
        fiscal_year,
        is_weekend,
        months_until,
        quarter,
    )

    # The 30/360 family is selected through DayCount; 30E/360 ISDA needs an
    # explicit termination status for its later endpoint.
    assert DayCount.from_name("30_360_isda") == DayCount.THIRTY_360_ISDA
    assert DayCount.THIRTY_360_ISDA.year_fraction("2025-02-28", "2025-03-31") * 360 == pytest.approx(33)
    ordinary = DayCountContext(end_is_termination_date=False)
    assert DayCount.THIRTY_E_360_ISDA.year_fraction("2024-01-31", "2024-02-29", ordinary) * 360 == pytest.approx(30)
    with pytest.raises(ValueError, match="end_is_termination_date"):
        DayCount.THIRTY_E_360_ISDA.year_fraction("2024-01-31", "2024-01-31")
    assert DayCountContext().end_is_termination_date is None
    assert add_business_days(date(2025, 6, 27), 3, "target2") == date(2025, 7, 2)
    assert add_weekdays("2025-01-03", 1) == date(2025, 1, 6)
    assert add_months("2024-01-31", 1) == date(2024, 2, 29)
    assert end_of_month("2024-02-15") == date(2024, 2, 29)
    assert is_weekend("2025-01-04")
    assert quarter("2025-08-15") == 3
    assert fiscal_year("2024-10-15", FiscalConfig(10, 1)) == 2025
    assert months_until("2020-01-15", "2022-03-10") == 25


def test_schedule_restore_validates_shape() -> None:
    # Record CORE-CONVENTIONS-008 #13: no empty or malformed restored schedule.
    for payload, message in [
        ('{"dates":[],"payment_dates":[],"fixing_dates":[]}', "at least two grid dates"),
        ('{"dates":["2025-01-15","2025-07-15"],"payment_dates":[],"fixing_dates":[]}', "payment dates"),
        (
            '{"dates":["2025-07-15","2025-01-15"],"payment_dates":["2025-07-15"],"fixing_dates":[]}',
            "strictly increasing",
        ),
        ('{"dates":["2025-01-15","2025-07-15"],"payment_dates":["2025-07-15"]}', "fixing_dates"),
    ]:
        with pytest.raises(ValueError, match=message):
            Schedule.from_json(payload)
    schedule = Schedule.from_json(
        '{"dates":["2025-01-15","2025-07-15"],"payment_dates":["2025-07-15"],"fixing_dates":[]}'
    )
    frame = schedule.to_dataframe()
    assert len(frame) == 1
    assert list(frame.columns) == ["period_start", "period_end", "payment_date", "fixing_date"]


@pytest.mark.parametrize("offset", [-(2**31), 2**31 - 1])
def test_date_extensions_reject_extreme_offsets(offset: int) -> None:
    from finstack_quant.core.dates import add_months, add_weekdays

    for shift in (add_months, add_weekdays):
        with pytest.raises(ValueError, match="supported date range"):
            shift("2025-01-01", offset)


def test_business_day_shift_rejects_calendar_boundary() -> None:
    from finstack_quant.core.dates import add_business_days

    with pytest.raises(ValueError, match="supported date range"):
        add_business_days("9999-12-31", 1, "weekends_only")


@pytest.mark.parametrize(
    ("days", "match"),
    [
        (-(2**63), "non-negative days"),
        (2**63 - 1, "supported date range"),
    ],
)
def test_market_roll_rejects_extreme_day_offsets(days: int, match: str) -> None:
    from finstack_quant.core.market_data import DiscountCurve, MarketContext

    context = MarketContext().insert(DiscountCurve.flat("USD-OIS", "2025-01-01", 0.05))
    with pytest.raises(ValueError, match=match):
        context.roll_forward(days)
