# finstack-quant-py/finstack_quant/reporting/charts.py
"""Hand-built inline-SVG chart primitives (no matplotlib).

Every chart returns a self-contained ``<svg>`` string sized via ``viewBox`` with
``width:100%; height:auto`` so it scales uniformly inside the report container.
Values are plotted in their display units; ``y_pct=True`` only appends ``%`` to
axis tick labels. ``None``/``NaN`` values are skipped.
"""

from __future__ import annotations

import math
from typing import Any

from . import format as fmt
from .theme import Theme

_W = 620
_MINUS = chr(0x2212)  # U+2212 MINUS SIGN (used in value labels)


def _xml_attr(s: str) -> str:
    """Escape a string for use as an XML attribute value or element text.

    Escapes ``&`` first, then ``<``/``>``/``"`` so the result is safe both as a
    double-quoted attribute value and as ``<title>`` text content.
    """
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace('"', "&quot;")


def _missing(v: Any) -> bool:
    return v is None or (isinstance(v, float) and math.isnan(v))


def rgba(hex_color: str, alpha: float) -> str:
    """Convert an RGB hex color to a CSS ``rgba(...)`` string.

    Parameters
    ----------
    hex_color : str
        Six-digit RGB color, with or without a leading ``#``.
    alpha : float
        Opacity from ``0.0`` (transparent) through ``1.0`` (opaque).
    """
    h = hex_color.lstrip("#")
    r, g, b = int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)
    return f"rgba({r},{g},{b},{alpha})"


def nice_ticks(vmin: float, vmax: float, target: int = 4) -> list[float]:
    """Return evenly spaced, human-friendly tick values spanning a range.

    Parameters
    ----------
    vmin : float
        Minimum displayed axis value before tick rounding.
    vmax : float
        Maximum displayed axis value before tick rounding.
    target : int
        Approximate desired number of intervals; defaults to four.
    """
    if vmax <= vmin:
        vmax = vmin + 1.0
    raw = (vmax - vmin) / target
    mag = 10 ** math.floor(math.log10(raw)) if raw > 0 else 1.0
    n = raw / mag
    step = (1 if n < 1.5 else 2 if n < 3 else 5 if n < 7 else 10) * mag
    start = math.floor(vmin / step) * step
    end = math.ceil(vmax / step) * step
    out: list[float] = []
    v = start
    while v <= end + 1e-9:
        out.append(0.0 if abs(v) < 1e-9 else v)
        v += step
    return out


def _year(d: Any) -> int:
    if hasattr(d, "year"):
        return int(d.year)
    return int(str(d)[:4])


def color_scale(v: Any, theme: Theme, cap: float = 8.0) -> tuple[str, str]:
    """Return background and foreground colors for a heatmap cell.

    Parameters
    ----------
    v : Any
        Value in percentage points; missing values receive a transparent cell.
    theme : Theme
        Report palette providing positive, negative, grid, and text colors.
    cap : float
        Absolute percentage-point value that maps to maximum shading intensity.
    """
    if _missing(v):
        return ("transparent", theme.grid)
    mag = min(abs(v) / cap, 1.0)
    alpha = mag * 0.85 + 0.05
    base = theme.pos if v >= 0 else theme.neg
    bg = rgba(base, round(alpha, 3))
    fg = "#ffffff" if mag > 0.55 else "#23303f"
    return (bg, fg)


def _tick_label(t: float, y_pct: bool) -> str:
    return f"{t:.0f}%" if y_pct else f"{t:.1f}"


def _tip_val(v: float, y_pct: bool) -> str:
    """Tooltip value string (2 dp, % suffix when y_pct)."""
    return f"{v:.2f}%" if y_pct else f"{v:.2f}"


def _x_label_numeric(v: float) -> str:
    return f"{v:.0f}" if abs(v - round(v)) < 1e-9 else f"{v:.1f}"


def _x_ticks_numeric(dates: list[Any], n: int, height: int, mb: int, theme: Theme, x: Any) -> list[str]:
    """Return SVG tick + label elements for a numeric x-axis (5 evenly-spaced ticks)."""
    parts: list[str] = []
    n_ticks = 5
    seen: set[int] = set()
    for k in range(n_ticks):
        i = round(k * (n - 1) / (n_ticks - 1)) if n > 1 else 0
        if i in seen:
            continue
        seen.add(i)
        xx = x(i)
        lab = _x_label_numeric(float(dates[i]))
        parts.append(
            f'<line x1="{xx:.1f}" y1="{height - mb}" x2="{xx:.1f}" y2="{height - mb + 4}" stroke="{theme.muted}"/>'
        )
        parts.append(
            f'<text x="{xx:.1f}" y="{height - mb + 15}" text-anchor="middle" font-size="10" '
            f'fill="{theme.muted}" font-family="{_xml_attr(theme.font_num)}">{lab}</text>'
        )
    return parts


def _x_ticks_date(dates: list[Any], height: int, mb: int, theme: Theme, x: Any) -> list[str]:
    """Return SVG tick + label elements for a date x-axis (one tick per year change)."""
    parts: list[str] = []
    prev_year = None
    for i, d in enumerate(dates):
        yr = _year(d)
        if yr != prev_year:
            xx = x(i)
            parts.append(
                f'<line x1="{xx:.1f}" y1="{height - mb}" x2="{xx:.1f}" y2="{height - mb + 4}" stroke="{theme.muted}"/>'
            )
            parts.append(
                f'<text x="{xx:.1f}" y="{height - mb + 15}" text-anchor="middle" font-size="10" '
                f'fill="{theme.muted}" font-family="{_xml_attr(theme.font_num)}">{yr}</text>'
            )
            prev_year = yr
    return parts


def line_chart(
    dates: list[Any],
    values: list[Any],
    *,
    theme: Theme,
    area: bool = False,
    y_pct: bool = False,
    zero: bool = False,
    ymin: float | None = None,
    ymax: float | None = None,
    color: str | None = None,
    fill: str | None = None,
    height: int = 190,
    x_numeric: bool = False,
) -> str:
    """Render a line or area SVG chart with date or numeric x-axis.

    Parameters
    ----------
    dates : list[Any]
        X-axis values aligned one-for-one with ``values``; dates may be date-like
        objects or strings, while numeric axes use numeric values.
    values : list[Any]
        Y-axis values aligned with ``dates``; ``None`` and ``NaN`` are skipped.
    theme : Theme
        Report palette and typography used for SVG elements.
    area : bool
        Whether to fill the area beneath the line; defaults to ``False``.
    y_pct : bool
        Whether y-axis labels append ``%`` to values already in percentage points.
    zero : bool
        Whether to include zero in the automatically chosen y-axis range.
    ymin : float or None
        Optional explicit lower y-axis bound before tick rounding.
    ymax : float or None
        Optional explicit upper y-axis bound before tick rounding.
    color : str or None
        Optional CSS stroke color; ``None`` uses the theme ink color.
    fill : str or None
        Optional CSS area-fill color; ``None`` derives a translucent stroke color.
    height : int
        SVG viewbox height in pixels; defaults to ``190``.
    x_numeric : bool
        Whether to render ``dates`` as numeric rather than calendar x-axis labels.
    """
    color = color or theme.ink
    ml, mr, mt, mb = 48, 14, 12, 24
    pw, ph = _W - ml - mr, height - mt - mb
    n = len(values)
    valid = [(i, float(v)) for i, v in enumerate(values) if not _missing(v)]
    if not valid:
        return f'<svg viewBox="0 0 {_W} {height}" xmlns="http://www.w3.org/2000/svg"></svg>'

    lo = ymin if ymin is not None else min(v for _, v in valid)
    hi = ymax if ymax is not None else max(v for _, v in valid)
    if zero:
        lo, hi = min(lo, 0.0), max(hi, 0.0)
    if lo == hi:
        hi = lo + 1.0

    ticks = nice_ticks(lo, hi, 4)
    lo, hi = ticks[0], ticks[-1]
    if lo == hi:
        hi = lo + 1.0

    def x(i: int) -> float:
        return ml + (i / (n - 1) if n > 1 else 0) * pw

    def y(v: float) -> float:
        return mt + (1 - (v - lo) / (hi - lo)) * ph

    parts: list[str] = [f'<svg viewBox="0 0 {_W} {height}" xmlns="http://www.w3.org/2000/svg">']
    # y gridlines + labels
    for t in ticks:
        yy = y(t)
        stroke = theme.grid if abs(t) < 1e-9 else theme.faint
        parts.append(f'<line x1="{ml}" y1="{yy:.1f}" x2="{_W - mr}" y2="{yy:.1f}" stroke="{stroke}"/>')
        parts.append(
            f'<text x="{ml - 6}" y="{yy + 3:.1f}" text-anchor="end" font-size="10" '
            f'fill="{theme.muted}" font-family="{_xml_attr(theme.font_num)}">{_tick_label(t, y_pct)}</text>'
        )
    # x ticks
    if x_numeric:
        parts.extend(_x_ticks_numeric(dates, n, height, mb, theme, x))
    else:
        parts.extend(_x_ticks_date(dates, height, mb, theme, x))
    pts = " ".join(f"{x(i):.1f},{y(v):.1f}" for i, v in valid)
    if area:
        base = y(0.0 if zero else lo)
        parts.append(
            f'<polygon points="{x(valid[0][0]):.1f},{base:.1f} {pts} {x(valid[-1][0]):.1f},{base:.1f}" '
            f'fill="{fill or rgba(color, 0.12)}"/>'
        )
    parts.append(f'<polyline points="{pts}" fill="none" stroke="{color}" stroke-width="1.6" stroke-linejoin="round"/>')
    # interactive overlay: transparent hover bands (native <title> + JS hook)
    band_w = pw / (len(valid) - 1) if len(valid) > 1 else pw
    max_bands = 500
    valid_for_bands = valid
    if len(valid) > max_bands:
        stride = max(1, math.ceil(len(valid) / max_bands))
        valid_for_bands = valid[::stride]
    for i, v in valid_for_bands:
        cx, cy = x(i), y(v)
        bx = max(ml, cx - band_w / 2)
        label = _x_label_numeric(float(dates[i])) if x_numeric else fmt.fmt_date(dates[i])
        val = _tip_val(v, y_pct)
        parts.append(
            f'<rect class="fq-hb" x="{bx:.1f}" y="{mt}" width="{band_w:.1f}" height="{ph}" '
            f'data-cx="{cx:.1f}" data-cy="{cy:.1f}" data-label="{label}" data-val="{val}">'
            f"<title>{label} · {val}</title></rect>"
        )
    parts.append(
        f'<line class="fq-cross" x1="0" x2="0" y1="{mt}" y2="{mt + ph}" '
        f'style="visibility:hidden" pointer-events="none"/>'
    )
    parts.append('<circle class="fq-mk" r="3.5" cx="0" cy="0" style="visibility:hidden" pointer-events="none"/>')
    parts.append("</svg>")
    return "".join(parts)


def cashflow_ladder(
    periods: list[str],
    coupon: list[float],
    principal: list[float],
    *,
    theme: Theme,
    pv: list[float] | None = None,
    height: int = 210,
) -> str:
    """Stacked coupon+principal bars per period, with an optional dashed PV-overlay line.

    Values are in display units (e.g. $ millions). Each period gets a transparent
    full-height hover band with a native ``<title>`` summarising the flow.

    Parameters
    ----------
    periods : list[str]
        Payment-period labels aligned with every cashflow series.
    coupon : list[float]
        Coupon cashflows in displayed currency units, aligned with ``periods``.
    principal : list[float]
        Principal cashflows in displayed currency units, aligned with ``periods``.
    theme : Theme
        Report palette and typography used for SVG elements.
    pv : list[float] or None
        Optional present-value overlay in the same units and order as cashflows.
    height : int
        SVG viewbox height in pixels; defaults to ``210``.
    """
    ml, mr, mt, mb = 52, 14, 12, 24
    pw, ph = _W - ml - mr, height - mt - mb
    totals = [coupon[i] + principal[i] for i in range(len(periods))]
    hi = max([*totals, *(pv or [0.0]), 0.0])
    ticks = nice_ticks(0.0, hi, 4)
    hi = ticks[-1] if ticks[-1] > 0 else 1.0

    def y(v: float) -> float:
        return mt + (1 - v / hi) * ph

    gap = pw / max(len(periods), 1)
    bw = gap * 0.55
    parts: list[str] = [f'<svg viewBox="0 0 {_W} {height}" xmlns="http://www.w3.org/2000/svg">']
    for t in ticks:
        yy = y(t)
        stroke = theme.grid if abs(t) < 1e-9 else theme.faint
        parts.append(f'<line x1="{ml}" y1="{yy:.1f}" x2="{_W - mr}" y2="{yy:.1f}" stroke="{stroke}"/>')
        parts.append(
            f'<text x="{ml - 6}" y="{yy + 3:.1f}" text-anchor="end" font-size="9.5" '
            f'fill="{theme.muted}" font-family="{_xml_attr(theme.font_num)}">{t:,.0f}</text>'
        )
    for i, lab in enumerate(periods):
        cx = ml + i * gap + gap / 2
        yc0, yc1 = y(0.0), y(coupon[i])
        parts.append(
            f'<rect x="{cx - bw / 2:.1f}" y="{yc1:.1f}" width="{bw:.1f}" height="{yc0 - yc1:.1f}" '
            f'fill="{theme.ink}" fill-opacity="0.85"/>'
        )
        if principal[i] > 0:
            yp1 = y(coupon[i] + principal[i])
            parts.append(
                f'<rect x="{cx - bw / 2:.1f}" y="{yp1:.1f}" width="{bw:.1f}" height="{yc1 - yp1:.1f}" '
                f'fill="{theme.accent}" fill-opacity="0.85"/>'
            )
        parts.append(
            f'<text x="{cx:.1f}" y="{height - mb + 15}" text-anchor="middle" font-size="9.5" '
            f'fill="{theme.muted}" font-family="{_xml_attr(theme.font_num)}">{_xml_attr(lab)}</text>'
        )
        pvtxt = f" · pv {pv[i]:,.2f}" if pv is not None else ""
        title = f"{lab} · coupon {coupon[i]:,.2f} · principal {principal[i]:,.2f}{pvtxt}"
        parts.append(
            f'<rect class="fq-hb" x="{cx - gap / 2:.1f}" y="{mt}" width="{gap:.1f}" height="{ph}" '
            f'data-cx="{cx:.1f}" data-cy="{y(totals[i]):.1f}" data-label="{_xml_attr(lab)}" '
            f'data-val="{coupon[i] + principal[i]:,.2f}"><title>{_xml_attr(title)}</title></rect>'
        )
    if pv is not None:
        pts = " ".join(f"{ml + i * gap + gap / 2:.1f},{y(pv[i]):.1f}" for i in range(len(periods)))
        parts.append(
            f'<polyline points="{pts}" fill="none" stroke="{theme.pos}" stroke-width="1.6" stroke-dasharray="4 3"/>'
        )
    parts.append(
        f'<line class="fq-cross" x1="0" x2="0" y1="{mt}" y2="{mt + ph}" style="visibility:hidden" pointer-events="none"/>'
    )
    parts.append('<circle class="fq-mk" r="3.5" cx="0" cy="0" style="visibility:hidden" pointer-events="none"/>')
    parts.append("</svg>")
    return "".join(parts)


def waterfall_chart(
    labels: list[str],
    deltas: list[Any],
    *,
    theme: Theme,
    total_label: str = "Total",
    height: int = 210,
) -> str:
    """Render a contribution waterfall (bridge).

    Each ``deltas[i]`` is a signed contribution; bar ``i`` floats from the running
    cumulative ``cum_i`` to ``cum_i + deltas[i]``, and a final anchored bar spans
    ``[0, Σ deltas]`` labelled ``total_label``. Positive steps use ``theme.pos``,
    negative ``theme.neg``, and the total bar ``theme.ink``. Reuses the axis,
    gridline, value-label, and hover-band (``fq-hb``/``fq-cross``/``fq-mk``)
    conventions of :func:`bar_chart`. Deterministic.

    Parameters
    ----------
    labels : list[str]
        Contribution labels aligned one-for-one with ``deltas``.
    deltas : list[Any]
        Signed contribution values in display units; missing values become zero.
    theme : Theme
        Report palette and typography used for SVG elements.
    total_label : str
        Label shown on the final cumulative-total bar.
    height : int
        SVG viewbox height in pixels; defaults to ``210``.
    """
    ml, mr, mt, mb = 56, 14, 12, 40
    pw, ph = _W - ml - mr, height - mt - mb
    ds = [0.0 if _missing(v) else float(v) for v in deltas]
    n = len(ds)

    cum = [0.0]
    for d in ds:
        cum.append(cum[-1] + d)
    total = cum[-1]

    levels = [*cum, 0.0, total]
    lo, hi = min(0.0, *levels), max(0.0, *levels)
    if lo == hi:
        hi = lo + 1.0
    ticks = nice_ticks(lo, hi, 4)
    lo, hi = ticks[0], ticks[-1]
    if lo == hi:
        hi = lo + 1.0

    def y(v: float) -> float:
        return mt + (1 - (v - lo) / (hi - lo)) * ph

    parts: list[str] = [f'<svg viewBox="0 0 {_W} {height}" xmlns="http://www.w3.org/2000/svg">']

    for t in ticks:
        yy = y(t)
        stroke = theme.grid if abs(t) < 1e-9 else theme.faint
        parts.append(f'<line x1="{ml}" y1="{yy:.1f}" x2="{_W - mr}" y2="{yy:.1f}" stroke="{stroke}"/>')
        parts.append(
            f'<text x="{ml - 6}" y="{yy + 3:.1f}" text-anchor="end" font-size="10" '
            f'fill="{theme.muted}" font-family="{_xml_attr(theme.font_num)}">{t:,.0f}</text>'
        )

    nbars = n + 1
    gap = pw / max(nbars, 1)
    bw = gap * 0.62

    def _bar(i: int, level_top: float, level_bot: float, value: float, lab: str, col: str, anchored: bool) -> None:
        cx = ml + i * gap + gap / 2
        y_top, y_bot = y(level_top), y(level_bot)
        sign = "+" if value >= 0 else _MINUS
        valstr = f"{sign}{abs(value):,.0f}"
        parts.append(
            f'<rect class="fq-hb" x="{cx - bw / 2:.1f}" y="{min(y_top, y_bot):.1f}" width="{bw:.1f}" '
            f'height="{max(abs(y_bot - y_top), 1.0):.1f}" fill="{col}" '
            f'fill-opacity="{0.9 if anchored else 0.82}" '
            f'data-cx="{cx:.1f}" data-cy="{min(y_top, y_bot):.1f}" data-label="{_xml_attr(lab)}" data-val="{valstr}">'
            f"<title>{_xml_attr(lab)} · {valstr}</title></rect>"
        )
        parts.append(
            f'<text x="{cx:.1f}" y="{height - mb + 14}" text-anchor="middle" font-size="9.5" '
            f'fill="{theme.muted}" font-family="{_xml_attr(theme.font_sans)}">{_xml_attr(lab)}</text>'
        )
        vy = min(y_top, y_bot) - 4 if value >= 0 else max(y_top, y_bot) + 12
        parts.append(
            f'<text x="{cx:.1f}" y="{vy:.1f}" text-anchor="middle" font-size="9.5" '
            f'fill="#23303f" font-family="{_xml_attr(theme.font_num)}">{valstr}</text>'
        )

    for i, d in enumerate(ds):
        col = theme.pos if d >= 0 else theme.neg
        _bar(i, cum[i + 1], cum[i], d, labels[i], col, anchored=False)
        cx = ml + i * gap + gap / 2
        x_from = cx + bw / 2
        x_to = ml + (i + 1) * gap + gap / 2 - bw / 2
        yy = y(cum[i + 1])
        parts.append(
            f'<line x1="{x_from:.1f}" y1="{yy:.1f}" x2="{x_to:.1f}" y2="{yy:.1f}" '
            f'stroke="{theme.faint}" stroke-dasharray="2 2"/>'
        )

    _bar(n, total, 0.0, total, total_label, theme.ink, anchored=True)

    parts.append(
        f'<line class="fq-cross" x1="0" x2="0" y1="{mt}" y2="{mt + ph}" '
        f'style="visibility:hidden" pointer-events="none"/>'
    )
    parts.append('<circle class="fq-mk" r="3.5" cx="0" cy="0" style="visibility:hidden" pointer-events="none"/>')
    parts.append("</svg>")
    return "".join(parts)


def bar_chart(labels: list[str], values: list[Any], *, theme: Theme, y_pct: bool = False, height: int = 190) -> str:
    """Render a categorical bar-chart SVG with value labels.

    Parameters
    ----------
    labels : list[str]
        Category labels aligned one-for-one with ``values``.
    values : list[Any]
        Signed values in display units; missing values are rendered as zero.
    theme : Theme
        Report palette and typography used for SVG elements.
    y_pct : bool
        Whether y-axis and bar labels append ``%`` to percentage-point values.
    height : int
        SVG viewbox height in pixels; defaults to ``190``.
    """
    ml, mr, mt, mb = 48, 14, 12, 24
    pw, ph = _W - ml - mr, height - mt - mb
    nums = [0.0 if _missing(v) else float(v) for v in values]
    lo, hi = min(0.0, *nums), max(0.0, *nums)
    if lo == hi:
        hi = lo + 1.0

    ticks = nice_ticks(lo, hi, 4)
    lo, hi = ticks[0], ticks[-1]
    if lo == hi:
        hi = lo + 1.0

    def y(v: float) -> float:
        return mt + (1 - (v - lo) / (hi - lo)) * ph

    parts: list[str] = [f'<svg viewBox="0 0 {_W} {height}" xmlns="http://www.w3.org/2000/svg">']
    for t in ticks:
        yy = y(t)
        stroke = theme.grid if abs(t) < 1e-9 else theme.faint
        parts.append(f'<line x1="{ml}" y1="{yy:.1f}" x2="{_W - mr}" y2="{yy:.1f}" stroke="{stroke}"/>')
        parts.append(
            f'<text x="{ml - 6}" y="{yy + 3:.1f}" text-anchor="end" font-size="10" '
            f'fill="{theme.muted}" font-family="{_xml_attr(theme.font_num)}">{t:.0f}{"%" if y_pct else ""}</text>'
        )
    gap = pw / max(len(labels), 1)
    bw = gap * 0.62
    for i, lab in enumerate(labels):
        cx = ml + i * gap + gap / 2
        v = nums[i]
        y0, y1 = y(0.0), y(v)
        col = theme.pos if v >= 0 else theme.neg
        label = _xml_attr(lab)
        valstr = _tip_val(v, y_pct)
        parts.append(
            f'<rect class="fq-hb" x="{cx - bw / 2:.1f}" y="{min(y0, y1):.1f}" width="{bw:.1f}" '
            f'height="{abs(y1 - y0):.1f}" fill="{col}" fill-opacity="0.82" '
            f'data-cx="{cx:.1f}" data-cy="{min(y0, y1):.1f}" data-label="{label}" data-val="{valstr}">'
            f"<title>{label} · {valstr}</title></rect>"
        )
        parts.append(
            f'<text x="{cx:.1f}" y="{height - mb + 15}" text-anchor="middle" font-size="10" '
            f'fill="{theme.muted}" font-family="{_xml_attr(theme.font_num)}">{_xml_attr(lab)}</text>'
        )
        vy = y1 - 4 if v >= 0 else y1 + 12
        vlabel = f"{'+' if v >= 0 else ''}{v:.0f}%" if y_pct else f"{v:,.0f}"
        parts.append(
            f'<text x="{cx:.1f}" y="{vy:.1f}" text-anchor="middle" font-size="9.5" '
            f'fill="#23303f" font-family="{_xml_attr(theme.font_num)}">{vlabel}</text>'
        )
    parts.append(
        f'<line class="fq-cross" x1="0" x2="0" y1="{mt}" y2="{mt + ph}" '
        f'style="visibility:hidden" pointer-events="none"/>'
    )
    parts.append('<circle class="fq-mk" r="3.5" cx="0" cy="0" style="visibility:hidden" pointer-events="none"/>')
    parts.append("</svg>")
    return "".join(parts)


def tornado_chart(
    entries: list[tuple[str, Any, Any]],
    *,
    theme: Theme,
    height: int | None = None,
) -> str:
    """Render a horizontal tornado (diverging bars) for sensitivity entries.

    ``entries`` is ``[(label, downside, upside)]`` (caller sorts by magnitude).
    Each row draws a downside bar (left of the zero baseline, ``theme.neg``) and
    an upside bar (right, ``theme.pos``). Reuses the value-axis, gridline, and
    hover-band (``fq-hb``/``fq-cross``/``fq-mk``) conventions of the other
    charts. Deterministic.

    Parameters
    ----------
    entries : list[tuple[str, Any, Any]]
        ``(label, downside, upside)`` sensitivity rows, normally pre-sorted by
        magnitude; values use shared display units.
    theme : Theme
        Report palette and typography used for SVG elements.
    height : int or None
        Optional SVG viewbox height; ``None`` scales automatically by row count.
    """
    n = len(entries)
    row_h = 26
    ml, mr, mt, mb = 132, 16, 14, 26  # wide left margin for category labels
    height = height if height is not None else mt + mb + max(n, 1) * row_h
    pw, ph = _W - ml - mr, height - mt - mb

    downs = [0.0 if _missing(d) else float(d) for _, d, _ in entries]
    ups = [0.0 if _missing(u) else float(u) for _, _, u in entries]
    lo = min(0.0, *downs, *ups) if entries else 0.0
    hi = max(0.0, *downs, *ups) if entries else 1.0
    if lo == hi:
        hi = lo + 1.0
    ticks = nice_ticks(lo, hi, 4)
    lo, hi = ticks[0], ticks[-1]
    if lo == hi:
        hi = lo + 1.0

    def x(v: float) -> float:
        return ml + (v - lo) / (hi - lo) * pw

    x0 = x(0.0)
    parts: list[str] = [f'<svg viewBox="0 0 {_W} {height}" xmlns="http://www.w3.org/2000/svg">']
    for t in ticks:
        xx = x(t)
        stroke = theme.grid if abs(t) < 1e-9 else theme.faint
        parts.append(f'<line x1="{xx:.1f}" y1="{mt}" x2="{xx:.1f}" y2="{mt + ph}" stroke="{stroke}"/>')
        parts.append(
            f'<text x="{xx:.1f}" y="{mt + ph + 16}" text-anchor="middle" font-size="10" '
            f'fill="{theme.muted}" font-family="{_xml_attr(theme.font_num)}">{t:,.0f}</text>'
        )

    bh = row_h * 0.6
    for i, (label, _, _) in enumerate(entries):
        cy = mt + i * row_h + row_h / 2
        parts.append(
            f'<text x="{ml - 8}" y="{cy + 3:.1f}" text-anchor="end" font-size="10.5" '
            f'fill="{theme.ink}" font-family="{_xml_attr(theme.font_sans)}">{_xml_attr(str(label))}</text>'
        )
        for value, col in ((downs[i], theme.neg), (ups[i], theme.pos)):
            xv = x(value)
            xa, xb = min(x0, xv), max(x0, xv)
            sign = "+" if value >= 0 else _MINUS
            valstr = f"{sign}{abs(value):,.0f}"
            parts.append(
                f'<rect class="fq-hb" x="{xa:.1f}" y="{cy - bh / 2:.1f}" width="{max(xb - xa, 0.5):.1f}" '
                f'height="{bh:.1f}" fill="{col}" fill-opacity="0.82" '
                f'data-cx="{xv:.1f}" data-cy="{cy:.1f}" data-label="{_xml_attr(str(label))}" data-val="{valstr}">'
                f"<title>{_xml_attr(str(label))} · {valstr}</title></rect>"
            )

    parts.append(f'<line x1="{x0:.1f}" y1="{mt}" x2="{x0:.1f}" y2="{mt + ph}" stroke="{theme.ink}"/>')
    parts.append(
        f'<line class="fq-cross" x1="0" x2="0" y1="{mt}" y2="{mt + ph}" '
        f'style="visibility:hidden" pointer-events="none"/>'
    )
    parts.append('<circle class="fq-mk" r="3.5" cx="0" cy="0" style="visibility:hidden" pointer-events="none"/>')
    parts.append("</svg>")
    return "".join(parts)


def fan_chart(
    periods: list[str],
    p_low: list[Any],
    p_mid: list[Any],
    p_high: list[Any],
    *,
    theme: Theme,
    y_pct: bool = False,
    height: int = 200,
) -> str:
    """Render a Monte-Carlo fan: a shaded band between ``p_low``/``p_high`` plus a median line.

    The median (``p_mid``) line is drawn over evenly spaced ``periods`` (category x-axis).
    The three series align by index; ``None``/``NaN`` entries are skipped for the
    band/line.

    The three percentile series must align with ``periods`` by index. An interior
    gap (a period missing from the band) is bridged by a straight segment rather
    than splitting the band into separate polygons.

    Reuses the gridline + hover-band conventions of :func:`line_chart`. Deterministic.

    Parameters
    ----------
    periods : list[str]
        Category labels aligned one-for-one with all percentile series.
    p_low : list[Any]
        Lower-percentile series in display units, aligned with ``periods``.
    p_mid : list[Any]
        Median-percentile series in display units, aligned with ``periods``.
    p_high : list[Any]
        Upper-percentile series in display units, aligned with ``periods``.
    theme : Theme
        Report palette and typography used for SVG elements.
    y_pct : bool
        Whether y-axis labels append ``%`` to percentage-point values.
    height : int
        SVG viewbox height in pixels; defaults to ``200``.
    """
    ml, mr, mt, mb = 48, 14, 12, 26
    pw, ph = _W - ml - mr, height - mt - mb
    n = len(periods)
    vals = [float(v) for v in (*p_low, *p_mid, *p_high) if not _missing(v)]
    if not vals or n == 0:
        return f'<svg viewBox="0 0 {_W} {height}" xmlns="http://www.w3.org/2000/svg"></svg>'

    lo, hi = min(vals), max(vals)
    if lo == hi:
        hi = lo + 1.0
    ticks = nice_ticks(lo, hi, 4)
    lo, hi = ticks[0], ticks[-1]
    if lo == hi:
        hi = lo + 1.0

    def x(i: int) -> float:
        return ml + (i / (n - 1) if n > 1 else 0) * pw

    def y(v: float) -> float:
        return mt + (1 - (v - lo) / (hi - lo)) * ph

    parts: list[str] = [f'<svg viewBox="0 0 {_W} {height}" xmlns="http://www.w3.org/2000/svg">']
    for t in ticks:
        yy = y(t)
        stroke = theme.grid if abs(t) < 1e-9 else theme.faint
        parts.append(f'<line x1="{ml}" y1="{yy:.1f}" x2="{_W - mr}" y2="{yy:.1f}" stroke="{stroke}"/>')
        parts.append(
            f'<text x="{ml - 6}" y="{yy + 3:.1f}" text-anchor="end" font-size="10" '
            f'fill="{theme.muted}" font-family="{_xml_attr(theme.font_num)}">{_tick_label(t, y_pct)}</text>'
        )
    for i, lab in enumerate(periods):
        parts.append(
            f'<text x="{x(i):.1f}" y="{height - mb + 15}" text-anchor="middle" font-size="9.5" '
            f'fill="{theme.muted}" font-family="{_xml_attr(theme.font_num)}">{_xml_attr(str(lab))}</text>'
        )

    band_idx = [i for i in range(n) if not _missing(p_low[i]) and not _missing(p_high[i])]
    if band_idx:
        top = " ".join(f"{x(i):.1f},{y(float(p_high[i])):.1f}" for i in band_idx)
        bot = " ".join(f"{x(i):.1f},{y(float(p_low[i])):.1f}" for i in reversed(band_idx))
        parts.append(f'<polygon points="{top} {bot}" fill="{rgba(theme.accent, 0.15)}"/>')

    mid = [(i, float(p_mid[i])) for i in range(n) if not _missing(p_mid[i])]
    if mid:
        pts = " ".join(f"{x(i):.1f},{y(v):.1f}" for i, v in mid)
        parts.append(
            f'<polyline points="{pts}" fill="none" stroke="{theme.ink}" stroke-width="1.6" stroke-linejoin="round"/>'
        )

    band_w = pw / (n - 1) if n > 1 else pw
    for i, lab in enumerate(periods):
        cx = x(i)
        bx = max(ml, cx - band_w / 2)
        lo_v = _tip_val(float(p_low[i]), y_pct) if not _missing(p_low[i]) else "·"
        mid_v = _tip_val(float(p_mid[i]), y_pct) if not _missing(p_mid[i]) else "·"
        hi_v = _tip_val(float(p_high[i]), y_pct) if not _missing(p_high[i]) else "·"
        cy = y(float(p_mid[i])) if not _missing(p_mid[i]) else mt
        title = f"{lab} · p50 {mid_v} · [{lo_v}, {hi_v}]"
        parts.append(
            f'<rect class="fq-hb" x="{bx:.1f}" y="{mt}" width="{band_w:.1f}" height="{ph}" '
            f'data-cx="{cx:.1f}" data-cy="{cy:.1f}" data-label="{_xml_attr(str(lab))}" data-val="{_xml_attr(mid_v)}">'
            f"<title>{_xml_attr(title)}</title></rect>"
        )

    parts.append(
        f'<line class="fq-cross" x1="0" x2="0" y1="{mt}" y2="{mt + ph}" '
        f'style="visibility:hidden" pointer-events="none"/>'
    )
    parts.append('<circle class="fq-mk" r="3.5" cx="0" cy="0" style="visibility:hidden" pointer-events="none"/>')
    parts.append("</svg>")
    return "".join(parts)
