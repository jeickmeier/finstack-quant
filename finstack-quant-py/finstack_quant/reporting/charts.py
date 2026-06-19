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

from .theme import Theme

_W = 620


def _xml_attr(s: str) -> str:
    """Escape a string for use as an XML attribute value (double-quoted)."""
    return s.replace("&", "&amp;").replace('"', "&quot;")


def _missing(v: Any) -> bool:
    return v is None or (isinstance(v, float) and math.isnan(v))


def rgba(hex_color: str, alpha: float) -> str:
    """Convert ``#rrggbb`` to an ``rgba(...)`` string."""
    h = hex_color.lstrip("#")
    r, g, b = int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)
    return f"rgba({r},{g},{b},{alpha})"


def nice_ticks(vmin: float, vmax: float, target: int = 4) -> list[float]:
    """Return evenly spaced, human-friendly tick values spanning ``[vmin, vmax]``."""
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
    """Background/foreground colors for a heatmap cell (value in percent units)."""
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
) -> str:
    """Render a line (optionally area-filled) chart with date x-axis and value y-axis."""
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
    # x ticks at year changes
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
    pts = " ".join(f"{x(i):.1f},{y(v):.1f}" for i, v in valid)
    if area:
        base = y(0.0 if zero else lo)
        parts.append(
            f'<polygon points="{x(valid[0][0]):.1f},{base:.1f} {pts} {x(valid[-1][0]):.1f},{base:.1f}" '
            f'fill="{fill or rgba(color, 0.12)}"/>'
        )
    parts.append(f'<polyline points="{pts}" fill="none" stroke="{color}" stroke-width="1.6" stroke-linejoin="round"/>')
    parts.append("</svg>")
    return "".join(parts)


def bar_chart(labels: list[str], values: list[Any], *, theme: Theme, y_pct: bool = False, height: int = 190) -> str:
    """Render a bar chart with category x-axis and value y-axis (value labels on bars)."""
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
        parts.append(
            f'<rect x="{cx - bw / 2:.1f}" y="{min(y0, y1):.1f}" width="{bw:.1f}" '
            f'height="{abs(y1 - y0):.1f}" fill="{col}" fill-opacity="0.82"/>'
        )
        parts.append(
            f'<text x="{cx:.1f}" y="{height - mb + 15}" text-anchor="middle" font-size="10" '
            f'fill="{theme.muted}" font-family="{_xml_attr(theme.font_num)}">{lab}</text>'
        )
        vy = y1 - 4 if v >= 0 else y1 + 12
        parts.append(
            f'<text x="{cx:.1f}" y="{vy:.1f}" text-anchor="middle" font-size="9.5" '
            f'fill="#23303f" font-family="{_xml_attr(theme.font_num)}">{"+" if v >= 0 else ""}{v:.0f}%</text>'
        )
    parts.append("</svg>")
    return "".join(parts)
