"""Tests for ScheduleBuilder bindings.

NOTE: requires a rebuilt wheel (the binding default frequency changed from
quarterly to monthly to match Rust ``ScheduleBuilder::new``).
"""

from datetime import date

import pytest

from finstack_quant.core.dates import (
    BusinessDayConvention,
    FiscalConfig,
    PeriodId,
    ScheduleBuilder,
    ScheduleErrorPolicy,
    StubKind,
)


def test_default_frequency_is_monthly() -> None:
    """An unspecified frequency defaults to monthly, matching Rust."""
    builder = ScheduleBuilder(date(2025, 1, 15), date(2026, 1, 15))
    schedule = builder.build()
    # 12 monthly periods over one year -> 13 dates (start + 12 period ends).
    assert len(schedule) == 13


def test_imm_modes_use_last_call_wins() -> None:
    start = date(2025, 1, 15)
    end = date(2025, 9, 30)

    cds_builder = ScheduleBuilder(start, end)
    cds_builder.imm()
    cds_builder.cds_imm()
    cds_dates = cds_builder.build().dates

    imm_builder = ScheduleBuilder(start, end)
    imm_builder.cds_imm()
    imm_builder.imm()
    imm_dates = imm_builder.build().dates

    assert date(2025, 3, 20) in cds_dates
    assert date(2025, 3, 19) not in cds_dates
    assert date(2025, 3, 19) in imm_dates
    assert date(2025, 3, 20) not in imm_dates


def test_schedule_builder_setters_are_fluent_and_mutate_in_place() -> None:
    builder = ScheduleBuilder(date(2025, 1, 15), date(2026, 1, 15))

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


def test_schedule_builder_fluent_setter_preserves_exceptions() -> None:
    builder = ScheduleBuilder(date(2025, 1, 15), date(2026, 1, 15))
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
