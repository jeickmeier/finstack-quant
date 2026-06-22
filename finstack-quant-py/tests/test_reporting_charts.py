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


def test_line_chart_has_native_tooltips_and_hover_bands() -> None:
    from xml.dom import minidom

    svg = charts.line_chart(_dates(4), [1.0, 5.0, 3.0, 8.0], theme=INSTITUTIONAL, y_pct=True)
    doc = minidom.parseString(svg)  # noqa: S318  still well-formed XML
    bands = [r for r in doc.getElementsByTagName("rect") if r.getAttribute("class") == "fq-hb"]
    assert len(bands) == 4
    # each band carries data + a native <title>
    for b in bands:
        assert b.getAttribute("data-label")
        assert b.getAttribute("data-val")
        assert b.getElementsByTagName("title")
    # hidden crosshair + marker present for the JS layer
    assert "fq-cross" in svg
    assert "fq-mk" in svg


def test_bar_chart_bars_have_titles() -> None:
    svg = charts.bar_chart(["2021", "2022"], [12.0, -8.0], theme=INSTITUTIONAL, y_pct=True)
    assert svg.count('class="fq-hb"') == 2
    assert "<title>2021" in svg
    assert "fq-cross" in svg
    assert "fq-mk" in svg


def test_line_chart_caps_hover_bands() -> None:
    n = 1500
    svg = charts.line_chart(_dates(n), [float(i % 7) for i in range(n)], theme=INSTITUTIONAL)
    band_count = svg.count('class="fq-hb"')
    assert 0 < band_count <= 500
    # polyline keeps full resolution: it references all points, so the svg is large
    assert "<polyline" in svg


def test_bar_chart_value_label_respects_y_pct() -> None:
    # dollar bars must NOT be labelled with "%"
    svg = charts.bar_chart(["5y", "10y"], [1350.0, 2620.0], theme=INSTITUTIONAL, y_pct=False)
    assert "1350%" not in svg
    assert "2620%" not in svg
    assert ">1350<" in svg or ">1,350<" in svg or "1350" in svg
    # percent bars still get "%"
    svg2 = charts.bar_chart(["2021"], [12.0], theme=INSTITUTIONAL, y_pct=True)
    assert "12%" in svg2


def test_line_chart_numeric_x_axis() -> None:
    from xml.dom import minidom

    spots = [4000.0, 4500.0, 5000.0, 5500.0, 6000.0]
    payoff = [0.0, 0.0, 0.0, 500.0, 1000.0]
    svg = charts.line_chart(spots, payoff, theme=INSTITUTIONAL, x_numeric=True, zero=True)
    minidom.parseString(svg)  # noqa: S318
    # numeric x tick labels appear (e.g. 5000), and no year label like "2026"
    assert "5000" in svg
    # hover band labels use the numeric x value, not a date
    assert 'data-label="5000"' in svg or 'data-label="5000.0"' in svg


def test_cashflow_ladder_wellformed() -> None:
    from xml.dom import minidom

    periods = ["'27", "'28", "'29"]
    coupon = [0.85, 0.85, 0.85]
    principal = [0.0, 0.0, 10.0]
    pv = [0.80, 0.77, 8.9]
    svg = charts.cashflow_ladder(periods, coupon, principal, theme=INSTITUTIONAL, pv=pv)
    minidom.parseString(svg)  # noqa: S318
    assert svg.count('class="fq-hb"') == 3  # one hover band per period
    assert "<polyline" in svg  # the PV overlay line
    assert "<title>" in svg


def test_tornado_chart_is_wellformed_svg() -> None:
    svg = charts.tornado_chart(
        [("Revenue", -25.0, 35.0), ("WACC", -18.0, 12.0)],
        theme=INSTITUTIONAL,
    )
    assert svg.startswith("<svg")
    minidom.parseString(svg)  # noqa: S318
    # 2 entries x (downside + upside) = 4 value bars
    assert svg.count('class="fq-hb"') == 4
    assert INSTITUTIONAL.neg in svg
    assert INSTITUTIONAL.pos in svg
    assert "Revenue" in svg
    assert "WACC" in svg


def test_tornado_chart_empty_is_wellformed() -> None:
    svg = charts.tornado_chart([], theme=INSTITUTIONAL)
    assert svg.startswith("<svg")
    minidom.parseString(svg)  # noqa: S318
