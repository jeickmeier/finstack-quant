"""Visual themes for reporting output.

A :class:`Theme` is a frozen bundle of design tokens (fonts, colors, rules)
plus a :meth:`Theme.to_css` method that emits a scoped ``<style>`` block. The
default :data:`INSTITUTIONAL` theme is the approved "Institutional Research
Note" house style.
"""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class Theme:
    """Immutable bundle of design tokens for rendered reports.

    Override with :func:`dataclasses.replace` to create variants.
    """

    name: str
    font_head: str
    font_num: str
    font_sans: str
    ink: str
    muted: str
    pos: str
    neg: str
    accent: str
    canvas: str
    rule: str
    faint: str
    grid: str

    def to_css(self, scope: str) -> str:
        """Return a ``<style>`` block with every rule scoped to one class.

        Parameters
        ----------
        scope : str
            CSS class name without the leading dot that namespaces all rules.
        """
        s = f".{scope}"
        return f"""<style>
{s} {{ background:{self.canvas}; color:#1c1c1e; font-family:{self.font_sans};
  border-radius:12px; padding:30px 38px 40px; }}
{s} .head {{ display:flex; justify-content:space-between; align-items:flex-end;
  border-bottom:1px solid {self.ink}; padding-bottom:13px; margin-bottom:18px; }}
{s} .head .eyebrow {{ font-size:11px; letter-spacing:.15em; text-transform:uppercase;
  color:{self.muted}; margin-bottom:4px; }}
{s} .head .title {{ font-family:{self.font_head}; font-size:25px; font-weight:600;
  color:{self.ink}; letter-spacing:-.01em; }}
{s} .head .subtitle {{ font-size:12.5px; color:{self.muted}; margin-top:4px; }}
{s} .head .meta {{ text-align:right; font-size:11px; color:{self.muted}; line-height:1.6; }}
{s} .kpis {{ display:grid; grid-template-columns:repeat(4,1fr);
  border-top:1px solid {self.faint}; border-bottom:1px solid {self.faint};
  padding:15px 0; margin-bottom:22px; }}
{s} .kpi {{ padding:0 16px; border-right:1px solid {self.faint}; }}
{s} .kpi:last-child {{ border-right:none; }}
{s} .kpi .lbl {{ font-size:10.5px; letter-spacing:.08em; text-transform:uppercase;
  color:{self.muted}; margin-bottom:6px; }}
{s} .kpi .val {{ font-family:{self.font_num}; font-size:25px; font-weight:600;
  color:{self.ink}; line-height:1; }}
{s} .kpi .val.pos {{ color:{self.pos}; }} {s} .kpi .val.neg {{ color:{self.neg}; }}
{s} .secttl {{ font-family:{self.font_head}; font-size:14px; font-weight:600;
  color:{self.ink}; margin:26px 0 4px; border-bottom:1px solid {self.faint}; padding-bottom:6px; }}
{s} .sub {{ font-size:11px; color:{self.muted}; margin:0 0 10px; }}
{s} .grid2 {{ display:grid; grid-template-columns:1fr 1fr; gap:26px; }}
{s} .statgrid {{ display:grid; grid-template-columns:repeat(3,1fr); gap:0 30px; margin-top:6px; }}
{s} table.kv {{ width:100%; border-collapse:collapse; font-size:12.5px; }}
{s} table.kv td {{ padding:6px 0; border-bottom:1px solid {self.faint}; }}
{s} table.kv td.k {{ color:{self.muted}; }}
{s} table.kv td.v {{ text-align:right; font-family:{self.font_num}; font-weight:600; color:{self.ink}; }}
{s} table.kv td.v.pos {{ color:{self.pos}; }} {s} table.kv td.v.neg {{ color:{self.neg}; }}
{s} table.hm {{ width:100%; border-collapse:collapse; font-family:{self.font_num}; font-size:11.5px; }}
{s} table.hm th {{ font-family:{self.font_sans}; font-size:10px; letter-spacing:.04em;
  text-transform:uppercase; color:{self.muted}; font-weight:600; padding:5px 0; text-align:center; }}
{s} table.hm th.yr, {s} table.hm td.yr {{ text-align:left; color:{self.muted};
  font-family:{self.font_sans}; font-size:11px; width:46px; }}
{s} table.hm td {{ text-align:center; padding:7px 2px; color:#23303f;
  border:2px solid {self.canvas}; }}
{s} table.hm td.ytd {{ font-weight:700; border-left:2px solid {self.grid}; }}
{s} table.dd {{ width:100%; border-collapse:collapse; font-size:12px; }}
{s} table.dd th {{ font-size:10px; letter-spacing:.05em; text-transform:uppercase;
  color:{self.muted}; font-weight:600; text-align:right; padding:6px 4px; border-bottom:1px solid {self.ink}; }}
{s} table.dd th:first-child {{ text-align:left; }}
{s} table.dd td {{ padding:6px 4px; border-bottom:1px solid {self.faint}; text-align:right;
  font-family:{self.font_num}; }}
{s} table.dd td:first-child {{ text-align:left; font-family:{self.font_sans}; color:{self.muted}; }}
{s} table.dd td.neg {{ color:{self.neg}; }}
{s} svg {{ display:block; width:100%; height:auto; }}
{s} .foot {{ margin-top:30px; border-top:1px solid {self.ink}; padding-top:10px;
  font-size:10.5px; color:{self.muted}; display:flex; justify-content:space-between; }}
{s} .fq-tip {{ position:fixed; pointer-events:none; z-index:9999; opacity:0; transition:opacity .08s;
  background:{self.ink}; color:#fff; font:11px/1.4 {self.font_sans}; padding:4px 8px;
  border-radius:4px; white-space:nowrap; }}
{s} svg .fq-hb {{ cursor:crosshair; }}
{s} svg rect.fq-hb:not([fill]) {{ fill:transparent; }}
{s} svg .fq-cross {{ stroke:{self.muted}; stroke-width:1; stroke-dasharray:3 3; pointer-events:none; }}
{s} svg .fq-mk {{ fill:{self.accent}; stroke:#fff; stroke-width:1; pointer-events:none; }}
{s} .fq-scroll {{ max-height:240px; overflow:auto; border:1px solid {self.faint}; border-radius:4px; }}
{s} .fq-scroll table.dd th {{ position:sticky; top:0; background:#f3f1ea; }}
</style>"""


INSTITUTIONAL = Theme(
    name="institutional",
    font_head='Georgia,"Times New Roman",serif',
    font_num='"Iowan Old Style",Georgia,serif',
    font_sans="-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif",
    ink="#10243f",
    muted="#7a8190",
    pos="#1b6b4f",
    neg="#9b2335",
    accent="#7c2230",
    canvas="#fbfaf6",
    rule="#10243f",
    faint="#e4e1d8",
    grid="#cfc8b8",
)
