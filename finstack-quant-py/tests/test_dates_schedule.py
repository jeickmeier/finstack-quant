"""Tests for ScheduleBuilder bindings.

NOTE: requires a rebuilt wheel (the binding default frequency changed from
quarterly to monthly to match Rust ``ScheduleBuilder::new``).
"""

from datetime import date

from finstack_quant.core.dates import ScheduleBuilder


def test_default_frequency_is_monthly() -> None:
    """An unspecified frequency defaults to monthly, matching Rust."""
    builder = ScheduleBuilder(date(2025, 1, 15), date(2026, 1, 15))
    schedule = builder.build()
    # 12 monthly periods over one year -> 13 dates (start + 12 period ends).
    assert len(schedule) == 13
