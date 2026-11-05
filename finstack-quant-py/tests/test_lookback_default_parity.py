"""Cross-host golden for ``Performance.lookback_returns`` with no date.

The WASM twin is ``finstack-quant-wasm/tests/facade/lookback_default_parity.test.mjs``:
both hosts build the same pinned panel and compare to the same result.

Regenerate with ``UPDATE_LOOKBACK_DEFAULT_PARITY=1 uv run --no-sync pytest <this file>``.
"""

from __future__ import annotations

from datetime import date
import json
import os
from pathlib import Path

import pytest

from finstack_quant.analytics import Performance, PerformancePanel

FIXTURE = Path(__file__).parent / "fixtures" / "lookback_default.json"
DATES = ["2025-01-30", "2025-01-31", "2025-02-03", "2025-02-04", "2025-02-05"]
RETURNS = [[0.01, -0.02, 0.03, 0.005, 0.0]]


def _panel() -> Performance:
    return Performance.from_panel(
        PerformancePanel(dates=DATES, series=RETURNS, kind="returns", ticker_names=["asset"], frequency="daily")
    )


def test_default_matches_the_result_pinned_for_both_hosts() -> None:
    actual = json.loads(_panel().lookback_returns().to_json())
    if os.environ.get("UPDATE_LOOKBACK_DEFAULT_PARITY"):
        FIXTURE.write_text(json.dumps({"dates": DATES, "returns": RETURNS, "lookback": actual}, indent=2) + "\n")
    assert actual == json.loads(FIXTURE.read_text())["lookback"]


def test_default_is_the_last_active_date() -> None:
    panel = _panel()
    omitted = panel.lookback_returns()
    assert omitted.ref_date == date(2025, 2, 5)
    assert omitted.to_json() == panel.lookback_returns(date(2025, 2, 5)).to_json()
    assert panel.lookback_returns(date(2025, 1, 31)).ref_date == date(2025, 1, 31)


def test_no_date_over_an_empty_active_range_is_rejected() -> None:
    panel = _panel()
    panel.reset_date_range(date(2025, 2, 5), date(2025, 1, 30))
    with pytest.raises(ValueError, match="ref_date"):
        panel.lookback_returns()
