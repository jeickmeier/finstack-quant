# finstack-quant-py/tests/test_reporting_charts.py
from __future__ import annotations

import datetime as dt
from xml.dom import minidom

from finstack_quant.reporting import charts
from finstack_quant.reporting.theme import INSTITUTIONAL


def _dates(n: int) -> list[dt.date]:
    return [dt.date(2021, 1, 1) + dt.timedelta(days=30 * i) for i in range(n)]


def test_nice_ticks_spans_range_with_zero() -> None:
    ticks = charts.nice_ticks(-14.0, 84.0, 4)
    assert ticks[0] <= -14.0 and ticks[-1] >= 84.0  # noqa: PT018
    assert 0 in ticks or 0.0 in ticks


def test_rgba_converts_hex() -> None:
    assert charts.rgba("#10243f", 0.12) == "rgba(16,36,63,0.12)"


def test_line_chart_is_wellformed_svg() -> None:
    svg = charts.line_chart(
        _dates(6),
        [0.0, 5.0, 3.0, 12.0, 20.0, 30.0],
        theme=INSTITUTIONAL,
        area=True,
        y_pct=True,
        zero=True,
    )
    assert svg.startswith("<svg")
    minidom.parseString(svg)  # raises if not well-formed XML  # noqa: S318
    assert "<polyline" in svg and "<polygon" in svg  # noqa: PT018


def test_bar_chart_is_wellformed_svg() -> None:
    svg = charts.bar_chart(["2021", "2022", "2023"], [12.0, -8.0, 25.0], theme=INSTITUTIONAL, y_pct=True)
    minidom.parseString(svg)  # noqa: S318
    assert svg.count("<rect") == 3


def test_color_scale_signs() -> None:
    bg_pos, _ = charts.color_scale(5.0, INSTITUTIONAL)
    bg_neg, _ = charts.color_scale(-5.0, INSTITUTIONAL)
    assert "rgba" in bg_pos and "rgba" in bg_neg  # noqa: PT018
    assert charts.color_scale(None, INSTITUTIONAL) == ("transparent", INSTITUTIONAL.grid)


def test_line_chart_gridlines_within_plot_area() -> None:
    # Non-boundary-aligned data range [3, 27] -> ticks [0..30] -> domain must
    # widen to the tick extremes so no gridline renders outside the plot box.
    svg = charts.line_chart(_dates(6), [3.0, 9.0, 15.0, 21.0, 24.0, 27.0], theme=INSTITUTIONAL, y_pct=True)
    doc = minidom.parseString(svg)  # noqa: S318
    # Plot area for default height=190: top=12, bottom=mt+ph=166.
    for ln in doc.getElementsByTagName("line"):
        if abs(float(ln.getAttribute("x1")) - 48.0) < 0.5:  # horizontal gridlines start at left margin 48
            y1 = float(ln.getAttribute("y1"))
            assert 11.5 <= y1 <= 166.5
