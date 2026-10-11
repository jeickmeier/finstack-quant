from __future__ import annotations

import datetime as dt
from pathlib import Path

import pytest

from finstack_quant.reporting.document import KPI, Section, TearSheet, _resolve_sections
from finstack_quant.reporting.theme import INSTITUTIONAL


def _sheet() -> TearSheet:
    return TearSheet(
        theme=INSTITUTIONAL,
        eyebrow="Performance Review",
        title="Global Macro Composite",
        subtitle="USD · Daily",
        meta_lines=["Benchmark: 60/40", "Decimal mode"],
        kpis=[KPI("Total Return", "+84.7%", "pos"), KPI("Sharpe", "1.42", "")],
        sections=[Section("Cumulative Return", "<svg></svg>")],
        generated=dt.date(2026, 6, 19),
    )


def test_repr_html_is_scoped_fragment() -> None:
    html = _sheet()._repr_html_()
    assert "<style>" in html
    assert 'class="fq-ts"' in html
    assert "Global Macro Composite" in html
    assert "Total Return" in html
    assert "+84.7%" in html


def test_to_html_is_standalone_document() -> None:
    html = _sheet().to_html()
    assert html.lstrip().startswith("<!DOCTYPE html>")
    assert "</html>" in html


def test_save_writes_file(tmp_path: Path) -> None:
    out = tmp_path / "ts.html"
    _sheet().save(out)
    assert out.read_text().lstrip().startswith("<!DOCTYPE html>")


def test_output_is_deterministic() -> None:
    first, second = _sheet(), _sheet()
    assert first.to_html() == second.to_html()


def test_tooltip_assets_present() -> None:
    html = _sheet()._repr_html_()
    assert 'class="fq-tip"' in html
    assert "<script>" in html
    assert "__fqWired" in html  # the wiring guard


def test_to_html_includes_tooltip_script() -> None:
    html = _sheet().to_html()
    assert 'class="fq-tip"' in html
    assert "addEventListener" in html


def test_resolve_sections_preserves_default_and_requested_sequences() -> None:
    valid = ["second", "first"]
    assert _resolve_sections(None, valid) is valid

    requested = ["first", "first", "second"]
    resolved = _resolve_sections(requested, valid)
    assert resolved is requested
    assert resolved == ["first", "first", "second"]

    empty: list[str] = []
    assert _resolve_sections(empty, valid) is empty


def test_resolve_sections_sorts_and_deduplicates_unknowns() -> None:
    with pytest.raises(ValueError, match="unknown section") as exc_info:
        _resolve_sections(["zzz", "first", "aaa", "zzz"], ["first", "second"])

    assert str(exc_info.value) == ("unknown section(s): ['aaa', 'zzz']; valid sections: ['first', 'second']")


def test_resolve_sections_preserves_short_valid_label() -> None:
    with pytest.raises(ValueError, match="unknown section") as exc_info:
        _resolve_sections(["zzz"], ["first", "second"], valid_label="valid")

    assert str(exc_info.value) == "unknown section(s): ['zzz']; valid: ['first', 'second']"
