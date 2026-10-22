"""Canonical parser contracts shared with the WASM facade."""

from __future__ import annotations

import pytest

from finstack_quant.core.cashflow import CFKind
from finstack_quant.core.dates import BusinessDayConvention, DayCount, PeriodKind, Tenor


def test_canonical_parsers_normalize_case_and_outer_whitespace() -> None:
    assert str(DayCount.from_name(" ACT_360 ")) == "act_360"
    assert str(BusinessDayConvention.from_name(" MODIFIED_FOLLOWING ")) == "modified_following"
    assert str(PeriodKind.from_name("quarterly")) == "quarterly"
    assert CFKind.parse(" FIXED ") == CFKind.FIXED
    assert str(Tenor.parse(" 3m ")) == "3M"


@pytest.mark.parametrize("name", ["ACT/360", "act-360", "act_365", "act_act_icma", "30_360_isda"])
def test_daycount_rejects_retired_labels(name: str) -> None:
    with pytest.raises(ValueError, match="day-count"):
        DayCount.from_name(name)


@pytest.mark.parametrize("name", ["MF", "F", "P", "MP", "NONE", "modfollowing"])
def test_business_day_rejects_short_codes(name: str) -> None:
    with pytest.raises(ValueError, match="business day"):
        BusinessDayConvention.from_name(name)


@pytest.mark.parametrize(
    "name", ["D", "B", "W", "M", "ME", "Q", "QE", "A", "Y", "YE", "semiannual", " QUARTERLY ", "DAILY", "monthly "]
)
def test_period_kind_rejects_frequency_aliases(name: str) -> None:
    with pytest.raises(ValueError, match="period kind"):
        PeriodKind.from_name(name)


@pytest.mark.parametrize("name", ["ON", "TN", "SN", "O/N", "+1D", "0D", "1 M"])
def test_tenor_rejects_noncanonical_tokens(name: str) -> None:
    with pytest.raises(ValueError, match="tenor"):
        Tenor.parse(name)


@pytest.mark.parametrize("name", ["coupon", "principal", "float-reset"])
def test_cashflow_kind_rejects_aliases(name: str) -> None:
    with pytest.raises(ValueError, match="cashflow kind"):
        CFKind.parse(name)


def _period_stats_payload() -> dict[str, object]:
    import datetime as dt
    import json

    from finstack_quant.analytics import Performance

    performance = Performance.from_returns_arrays(
        [dt.date(2025, 1, 1), dt.date(2025, 2, 1)], [[0.01, 0.02]], ["fund"], frequency="monthly"
    )
    return json.loads(performance.period_stats(0, aggregation_frequency="monthly").to_json())


def test_nonfinite_sentinel_parser_normalizes_canonical_token() -> None:
    import json
    import math

    from finstack_quant.analytics import PeriodStats

    payload = _period_stats_payload()
    payload["profit_factor"] = " INF "
    assert math.isinf(PeriodStats.from_json(json.dumps(payload)).profit_factor)


@pytest.mark.parametrize("token", ["+inf", "infinity", "+infinity", "-infinity"])
def test_nonfinite_sentinel_parser_rejects_aliases(token: str) -> None:
    import json

    from finstack_quant.analytics import PeriodStats

    payload = _period_stats_payload()
    payload["profit_factor"] = token
    with pytest.raises(ValueError, match="expected a number"):
        PeriodStats.from_json(json.dumps(payload))
