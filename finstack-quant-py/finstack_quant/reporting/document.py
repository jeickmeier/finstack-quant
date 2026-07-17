# finstack-quant-py/finstack_quant/reporting/document.py
"""Composition layer: assemble a header, KPI strip, and sections into HTML.

A :class:`TearSheet` renders a scoped fragment for Jupyter (``_repr_html_``) and a
standalone document (``to_html`` / ``save``). Output is fully deterministic: the
CSS scope class is constant and the ``generated`` stamp is caller-injectable.

Examples:
--------
>>> import finstack_quant.reporting.document as document
>>> document.__name__
'finstack_quant.reporting.document'
"""

from __future__ import annotations

from dataclasses import dataclass, field
import datetime as dt
import html
import os
from pathlib import Path
from typing import Any

from . import format as fmt
from .theme import Theme

_SCOPE = "fq-ts"

_TOOLTIP_JS = """<script>
(function(){
  function init(){
    document.querySelectorAll('.fq-ts svg .fq-hb').forEach(function(band){
      if(band.__fqWired){return;} band.__fqWired=true;
      var root=band.closest('.fq-ts'); if(!root){return;}
      var tip=root.querySelector('.fq-tip');
      var svg=band.ownerSVGElement;
      var cross=svg&&svg.querySelector('.fq-cross');
      var mk=svg&&svg.querySelector('.fq-mk');
      function show(e){
        if(tip){tip.textContent=band.getAttribute('data-label')+' · '+band.getAttribute('data-val');
          tip.style.left=(e.clientX+12)+'px'; tip.style.top=(e.clientY+12)+'px'; tip.style.opacity='1';}
        var cx=band.getAttribute('data-cx'), cy=band.getAttribute('data-cy');
        if(cross){cross.setAttribute('x1',cx); cross.setAttribute('x2',cx); cross.style.visibility='visible';}
        if(mk){mk.setAttribute('cx',cx); mk.setAttribute('cy',cy); mk.style.visibility='visible';}
      }
      function hide(){ if(tip){tip.style.opacity='0';} if(cross){cross.style.visibility='hidden';} if(mk){mk.style.visibility='hidden';} }
      band.addEventListener('mousemove',show);
      band.addEventListener('mouseenter',show);
      band.addEventListener('mouseleave',hide);
    });
  }
  if(document.readyState==='loading'){document.addEventListener('DOMContentLoaded',init);}else{init();}
})();
</script>"""


@dataclass
class KPI:
    """A single headline statistic in the KPI strip.

    Examples:
    --------
    >>> from finstack_quant.reporting.document import KPI
    >>> KPI.__name__
    'KPI'
    """

    label: str
    value: str
    cls: str = ""  # "pos" | "neg" | ""


@dataclass
class Section:
    """A titled block of body HTML, optionally with a subtitle line.

    Examples:
    --------
    >>> from finstack_quant.reporting.document import Section
    >>> Section.__name__
    'Section'
    """

    title: str
    body: str
    subtitle: str | None = None


@dataclass
class TearSheet:
    """A composed report. Renders to scoped-fragment or standalone HTML.

    Examples:
    --------
    >>> from finstack_quant.reporting.document import TearSheet
    >>> TearSheet.__name__
    'TearSheet'
    """

    theme: Theme
    title: str
    sections: list[Section]
    eyebrow: str = ""
    subtitle: str | None = None
    meta_lines: list[str] = field(default_factory=list)
    kpis: list[KPI] = field(default_factory=list)
    generated: dt.date | None = None
    footer_left: str = ""
    footer_right: str = "finstack-quant"

    def _esc(self, x: Any) -> str:
        return html.escape(str(x))

    def _header_html(self) -> str:
        meta = list(self.meta_lines)
        if self.generated is not None:
            meta = [*meta, f"Generated {fmt.fmt_date(self.generated)}"]
        meta_html = "<br>".join(self._esc(m) for m in meta)
        sub = f'<div class="subtitle">{self._esc(self.subtitle)}</div>' if self.subtitle else ""
        return (
            '<div class="head"><div>'
            f'<div class="eyebrow">{self._esc(self.eyebrow)}</div>'
            f'<div class="title">{self._esc(self.title)}</div>{sub}</div>'
            f'<div class="meta">{meta_html}</div></div>'
        )

    def _kpis_html(self) -> str:
        if not self.kpis:
            return ""
        cells = "".join(
            f'<div class="kpi"><div class="lbl">{self._esc(k.label)}</div>'
            f'<div class="val {k.cls}">{self._esc(k.value)}</div></div>'
            for k in self.kpis
        )
        return f'<div class="kpis">{cells}</div>'

    def _sections_html(self) -> str:
        out = []
        for sec in self.sections:
            out.append(f'<div class="secttl">{self._esc(sec.title)}</div>')
            if sec.subtitle:
                out.append(f'<p class="sub">{self._esc(sec.subtitle)}</p>')
            out.append(sec.body)
        return "".join(out)

    def _footer_html(self) -> str:
        return (
            f'<div class="foot"><span>{self._esc(self.footer_left)}</span>'
            f"<span>{self._esc(self.footer_right)}</span></div>"
        )

    def _body_fragment(self) -> str:
        return (
            f'<div class="{_SCOPE}">'
            f"{self._header_html()}{self._kpis_html()}{self._sections_html()}{self._footer_html()}"
            '<div class="fq-tip" aria-hidden="true"></div>'
            "</div>"
        )

    def _repr_html_(self) -> str:
        """Scoped fragment for inline Jupyter display."""
        return self.theme.to_css(_SCOPE) + self._body_fragment() + _TOOLTIP_JS

    def to_html(self) -> str:
        """Full standalone HTML document.

        Returns:
        -------
        str
            Result of to html for this `TearSheet` in the annotated representation.
        """
        return (
            "<!DOCTYPE html>\n"
            '<html lang="en"><head><meta charset="utf-8">'
            '<meta name="viewport" content="width=device-width, initial-scale=1">'
            f"<title>{self._esc(self.title)}</title>{self.theme.to_css(_SCOPE)}</head>"
            '<body style="margin:0;padding:24px;background:#e8e9ec;">'
            f"{self._body_fragment()}{_TOOLTIP_JS}</body></html>\n"
        )

    def save(self, path: str | os.PathLike[str]) -> None:
        """Write the standalone document to a UTF-8 file.

        Parameters
        ----------
        path : str or os.PathLike[str]
            Destination HTML file path; existing content is replaced.

        Raises:
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        with Path(path).open("w", encoding="utf-8") as fh:
            fh.write(self.to_html())
