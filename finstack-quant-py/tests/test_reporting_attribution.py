"""Tests for the attribution tear sheet and its waterfall chart."""

from __future__ import annotations

from pathlib import Path

DATA = Path(__file__).parent / "data"


def test_waterfall_chart_totals_signs_and_colors() -> None:
    from finstack_quant.reporting import charts
    from finstack_quant.reporting.theme import INSTITUTIONAL

    svg = charts.waterfall_chart(["Carry", "Rates"], [25000.0, -2400.0], theme=INSTITUTIONAL, total_label="Total P&L")
    assert svg.startswith("<svg")
    # all three bar labels present
    for lab in ("Carry", "Rates", "Total P&L"):
        assert lab in svg
    # positive step uses pos color, negative uses neg, total uses ink
    assert INSTITUTIONAL.pos in svg
    assert INSTITUTIONAL.neg in svg
    assert INSTITUTIONAL.ink in svg
    # signed value labels (minus is U+2212)
    assert "+25,000" in svg
    assert "−2,400" in svg
    # total = 25000 - 2400 = 22,600
    assert "+22,600" in svg
    # hover bands present
    assert 'class="fq-hb"' in svg
