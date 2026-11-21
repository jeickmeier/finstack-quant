"""Composition layer: assemble a header, KPI strip, and sections into HTML.

A :class:`TearSheet` renders a scoped fragment for Jupyter (``_repr_html_``) and a
standalone document (``to_html`` / ``save``). Output is fully deterministic: the
CSS scope class is constant and the ``generated`` stamp is caller-injectable.

Examples:
--------
>>> from finstack_quant.reporting.document import TearSheet
>>> from finstack_quant.reporting.theme import INSTITUTIONAL
>>> TearSheet(INSTITUTIONAL, "Demo", []).to_html().startswith("<!DOCTYPE html>")
True
"""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass, field
import datetime as dt
import os
from pathlib import Path
from typing import Any

from . import format as fmt
from .tables import data_table
from .theme import Theme

__all__ = [
    "KPI",
    "Section",
    "TearSheet",
]


def _resolve_sections(
    sections: list[str] | None,
    valid_sections: list[str],
    *,
    valid_label: str = "valid sections",
) -> list[str]:
    wanted = sections if sections is not None else valid_sections
    unknown = set(wanted) - set(valid_sections)
    if unknown:
        raise ValueError(f"unknown section(s): {sorted(unknown)}; {valid_label}: {valid_sections}")
    return wanted


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
    >>> KPI("PV", "1.2m USD").label
    'PV'
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
    >>> Section("Summary", "<p>Ready</p>").title
    'Summary'
    """

    title: str
    body: str
    subtitle: str | None = None

    @classmethod
    def from_dataframe(
        cls,
        title: str,
        df: Any,
        *,
        columns: list[str] | None = None,
        formats: dict[str, Callable[[Any], str]] | None = None,
        neg_columns: set[str] | None = None,
        subtitle: str | None = None,
    ) -> Section:
        """Render a tabular object (DataFrame, Polars DataFrame, ArrowTable, etc.) as a report section.

        Parameters
        ----------
        title : str
            Heading displayed above the table.
        df : pandas.DataFrame | polars.DataFrame | pyarrow.Table | ArrowTable | Any
            Rows to display in their current order. Accepts ``pandas.DataFrame``,
            ``polars.DataFrame``, ``pyarrow.Table``, ``ArrowTable``, or any
            finstack tabular result exposing ``to_pandas()``. Reset the index on a
            pandas DataFrame to display it.
        columns : list[str] or None
            Column names in display order. Omit to use the frame's column order.
        formats : dict[str, Callable[[Any], str]] or None
            Per-column formatters that return display-ready HTML.
        neg_columns : set[str] or None
            Columns where negative numbers receive the negative-value style.
        subtitle : str or None
            Optional note associated with the section.

        Returns:
        -------
        Section
            Section containing the formatted data table.

        Raises:
        ------
        KeyError
            If a requested column is absent.
        ValueError
            If selected columns contain duplicate labels.

        Examples:
        --------
        >>> import pandas as pd
        >>> from finstack_quant.reporting import Section
        >>> section = Section.from_dataframe("Positions", pd.DataFrame({"PnL": [-5]}), neg_columns={"PnL"})
        >>> 'class="neg"' in section.body
        True
        """
        if hasattr(df, "to_pandas") and not hasattr(df, "iterrows"):
            df = df.to_pandas()
        selected = df if columns is None else df.loc[:, columns]
        if not selected.columns.is_unique:
            raise ValueError("Report table columns must have unique labels")
        body = data_table(
            selected.to_dict("records"), columns=list(selected.columns), formats=formats, neg_columns=neg_columns
        )
        return cls(title, body, subtitle)


@dataclass
class TearSheet:
    """A composed report. Renders to scoped-fragment or standalone HTML.

    Examples:
    --------
    >>> from finstack_quant.reporting.document import TearSheet
    >>> from finstack_quant.reporting.theme import INSTITUTIONAL
    >>> TearSheet(INSTITUTIONAL, "Demo", []).to_html().startswith("<!DOCTYPE html>")
    True
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

    def add_section(self, section: Section) -> TearSheet:
        """Append a section to this report in display order.

        Parameters
        ----------
        section : Section
            The prepared report section to append.

        Returns:
        -------
        TearSheet
            This report, with the new section at the end.

        Notes:
        -----
        Appending a prepared section does not raise.

        Examples:
        --------
        >>> from finstack_quant.reporting import INSTITUTIONAL, Section, TearSheet
        >>> report = TearSheet(INSTITUTIONAL, "Demo", [])
        >>> report.add_section(Section("Summary", "<p>Ready</p>")) is report
        True
        """
        self.sections.append(section)
        return self

    def _header_html(self) -> str:
        meta = list(self.meta_lines)
        if self.generated is not None:
            meta = [*meta, f"Generated {fmt.fmt_date(self.generated)}"]
        meta_html = "<br>".join(fmt._escape_html(m) for m in meta)
        sub = f'<div class="subtitle">{fmt._escape_html(self.subtitle)}</div>' if self.subtitle else ""
        return (
            '<div class="head"><div>'
            f'<div class="eyebrow">{fmt._escape_html(self.eyebrow)}</div>'
            f'<div class="title">{fmt._escape_html(self.title)}</div>{sub}</div>'
            f'<div class="meta">{meta_html}</div></div>'
        )

    def _kpis_html(self) -> str:
        if not self.kpis:
            return ""
        cells = "".join(
            f'<div class="kpi"><div class="lbl">{fmt._escape_html(k.label)}</div>'
            f'<div class="val {k.cls}">{fmt._escape_html(k.value)}</div></div>'
            for k in self.kpis
        )
        return f'<div class="kpis">{cells}</div>'

    def _sections_html(self) -> str:
        out = []
        for sec in self.sections:
            out.append(f'<div class="secttl">{fmt._escape_html(sec.title)}</div>')
            if sec.subtitle:
                out.append(f'<p class="sub">{fmt._escape_html(sec.subtitle)}</p>')
            out.append(sec.body)
        return "".join(out)

    def _footer_html(self) -> str:
        return (
            f'<div class="foot"><span>{fmt._escape_html(self.footer_left)}</span>'
            f"<span>{fmt._escape_html(self.footer_right)}</span></div>"
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
            Complete standalone HTML document containing the configured report.

        Notes:
            This method does not raise; it returns the stored or derived value.
        """
        return (
            "<!DOCTYPE html>\n"
            '<html lang="en"><head><meta charset="utf-8">'
            '<meta name="viewport" content="width=device-width, initial-scale=1">'
            f"<title>{fmt._escape_html(self.title)}</title>{self.theme.to_css(_SCOPE)}</head>"
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
        OSError
            If the destination cannot be opened or the HTML cannot be written.

        """
        with Path(path).open("w", encoding="utf-8") as fh:
            fh.write(self.to_html())
