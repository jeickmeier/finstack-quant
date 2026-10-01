"""Tests for the attribution tear sheet and its waterfall chart."""

from __future__ import annotations

import datetime as dt
import json
from pathlib import Path

import pytest

DATA = Path(__file__).parent / "data"


def test_waterfall_chart_totals_signs_and_colors() -> None:
    from finstack_quant.reporting import charts
    from finstack_quant.reporting.theme import INSTITUTIONAL

    svg = charts.waterfall_chart(["Carry", "Rates"], [25000.0, -2400.0], theme=INSTITUTIONAL, total_label="Total P&L")
    assert svg.startswith("<svg")
    # all three bar labels present
    assert "Carry" in svg
    assert "Rates" in svg
    assert "Total P&amp;L" in svg  # & is XML-escaped in SVG titles/labels
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


def _load_attr() -> object:
    from finstack_quant.attribution import PnlAttribution

    return PnlAttribution.from_json((DATA / "attribution_bond.json").read_text())


def test_waterfall_field_order_matches_engine() -> None:
    # Drift guard: the static factor order must match the engine's canonical order.
    from finstack_quant.attribution import default_waterfall_order
    from finstack_quant.reporting.attribution import _WF_FIELD

    assert list(_WF_FIELD.keys()) == list(default_waterfall_order())


def test_attribution_tearsheet_from_object_has_sections_and_kpis() -> None:
    from finstack_quant.reporting import attribution_tearsheet

    ts = attribution_tearsheet(_load_attr(), generated=dt.date(2026, 6, 21))
    titles = [s.title for s in ts.sections]
    assert "P&L Attribution" in titles  # waterfall
    assert "Factor Contributions" in titles  # factor table
    assert "Carry & Roll-Down" in titles  # carry detail (bond has carry)
    assert "Credit Factor Detail" not in titles  # no credit P&L -> omitted (adaptive)
    kpi_labels = [k.label for k in ts.kpis]
    assert kpi_labels == ["Total P&L", "Mark-to-Market", "Carry", "Residual", "Repricings"]
    html = ts.to_html()
    assert "ATTR-BOND-001" in html
    assert "Total P&L" in html
    assert "Carry" in html


def test_attribution_tearsheet_section_selection() -> None:
    from finstack_quant.reporting import attribution_tearsheet

    ts = attribution_tearsheet(_load_attr(), sections=["waterfall"], generated=dt.date(2026, 6, 21))
    assert [s.title for s in ts.sections] == ["P&L Attribution"]


def test_attribution_tearsheet_json_and_object_match() -> None:
    from finstack_quant.reporting import attribution_tearsheet

    raw = (DATA / "attribution_bond.json").read_text()
    from_obj = attribution_tearsheet(_load_attr(), generated=dt.date(2026, 6, 21)).to_html()
    from_json = attribution_tearsheet(raw, generated=dt.date(2026, 6, 21)).to_html()
    assert from_obj == from_json


def test_attribution_tearsheet_requires_inputs() -> None:
    import pytest

    from finstack_quant.reporting import attribution_tearsheet

    with pytest.raises(ValueError, match="requires a precomputed PnlAttribution"):
        attribution_tearsheet(None)


def test_reporting_import_is_engine_light() -> None:
    # Importing reporting and rendering from a PnlAttribution OBJECT must not import
    # the attribution engine module.
    import subprocess
    import sys

    code = (
        "import sys, finstack_quant.reporting as r;"
        "assert 'finstack_quant.attribution' not in sys.modules, "
        "'reporting import pulled the attribution engine'"
    )
    out = subprocess.run([sys.executable, "-c", code], capture_output=True, text=True, check=False)  # noqa: S603
    assert out.returncode == 0, out.stderr


def test_attribution_tearsheet_rejects_unknown_section() -> None:
    import pytest

    from finstack_quant.reporting import attribution_tearsheet

    with pytest.raises(ValueError, match="unknown section") as exc_info:
        attribution_tearsheet(_load_attr(), sections=["zzz", "waterfall", "aaa", "zzz"])

    assert str(exc_info.value) == (
        "unknown section(s): ['aaa', 'zzz']; valid: ['waterfall', 'factors', 'carry', 'credit']"
    )


def _attribution_golden_html() -> str:
    from finstack_quant.reporting import attribution_tearsheet

    return attribution_tearsheet(_load_attr(), generated=dt.date(2026, 6, 21)).to_html()


def test_attribution_tearsheet_matches_golden() -> None:
    golden = DATA / "attribution_tearsheet_golden.html"
    assert golden.exists(), "golden missing — regenerate (Task 3 Step 2)"
    assert _attribution_golden_html() == golden.read_text(encoding="utf-8")


def test_material_residual_is_visible_when_total_pnl_is_zero() -> None:
    import json

    from finstack_quant.attribution import PnlAttribution
    from finstack_quant.reporting import attribution_tearsheet

    raw = json.loads((DATA / "attribution_bond.json").read_text())
    raw["total_pnl"]["amount"] = "0"
    raw["residual"]["amount"] = "1000"
    raw["meta"]["residual_pct"] = 0.0
    raw["result_invalid"] = False
    result = PnlAttribution.from_json(json.dumps(raw))
    sheet = attribution_tearsheet(result)
    residual = next(k for k in sheet.kpis if k.label == "Residual")
    assert "1,000" in residual.value
    assert "%" not in residual.value
    html = sheet.to_html()
    assert "residual outside tolerance" in html
    assert "calculation flagged invalid" not in html

    raw["result_invalid"] = True
    invalid = attribution_tearsheet(PnlAttribution.from_json(json.dumps(raw))).to_html()
    assert "calculation flagged invalid" in invalid


@pytest.mark.parametrize("field", ["carry", "residual", "mark_to_market_pnl", "coupon_income"])
def test_attribution_tearsheet_rejects_mixed_currencies(field: str) -> None:
    from finstack_quant.attribution import PnlAttribution
    from finstack_quant.reporting import attribution_tearsheet

    raw = json.loads((DATA / "attribution_bond.json").read_text())
    amount = raw["carry_detail"]["coupon_income"]["total"] if field == "coupon_income" else raw[field]
    amount["currency"] = "EUR"
    for supplied in (raw, json.dumps(raw), PnlAttribution.from_json(json.dumps(raw))):
        with pytest.raises(ValueError, match="Currency mismatch"):
            attribution_tearsheet(supplied)


def test_credit_detail_preserves_identifiers_and_marks_breakdowns() -> None:
    from finstack_quant.reporting import attribution_tearsheet

    raw = json.loads((DATA / "attribution_bond.json").read_text())

    def usd(value: int) -> dict[str, str]:
        return {"amount": str(value), "currency": "USD"}

    raw["credit_curves_pnl"] = usd(90)
    raw["credit_factor_detail"] = {
        "model_id": "DETAIL-MODEL",
        "generic_pnl": usd(10),
        "levels": [
            {"level_name": "rating", "total": usd(20), "by_bucket": {"IG": usd(20)}},
            {"level_name": "sector", "total": usd(30), "by_bucket": {"TECH": usd(30)}},
        ],
        "adder_pnl_total": usd(15),
        "curve_shape_pnl": usd(15),
        "adder_pnl_by_issuer": {"ACME": usd(15)},
        "adder_magnitude": usd(15),
    }
    sheet = attribution_tearsheet(raw, sections=["credit"])
    html = sheet.sections[0].body
    assert "Level Total · rating" in html
    assert "Level Total · sector" in html
    assert "Level / By Bucket (Breakdown) · rating / IG" in html
    assert "Level / By Bucket (Breakdown) · sector / TECH" in html
    assert "Adder By Issuer (Breakdown) · adder / ACME" in html


def test_carry_detail_marks_rates_and_credit_as_subdivisions() -> None:
    from finstack_quant.reporting import attribution_tearsheet

    raw = json.loads((DATA / "attribution_bond.json").read_text())
    raw["carry_detail"]["coupon_income"]["rates_part"] = {"amount": "100", "currency": "USD"}
    raw["carry_detail"]["coupon_income"]["credit_part"] = {"amount": "50", "currency": "USD"}
    html = attribution_tearsheet(raw, sections=["carry"]).sections[0].body
    assert "Coupon Income / Rates (Breakdown)" in html
    assert "Coupon Income / Credit (Breakdown)" in html


def test_credit_detail_escapes_issuer_identifiers() -> None:
    from finstack_quant.reporting import attribution_tearsheet

    raw = json.loads((DATA / "attribution_bond.json").read_text())
    zero = {"amount": "0", "currency": "USD"}
    one = {"amount": "1", "currency": "USD"}
    issuer = "<img src=x onerror=alert(1)>"
    raw["credit_curves_pnl"] = one
    raw["credit_factor_detail"] = {
        "model_id": "DETAIL-MODEL",
        "generic_pnl": zero,
        "levels": [],
        "adder_pnl_total": one,
        "curve_shape_pnl": zero,
        "adder_pnl_by_issuer": {issuer: one},
        "adder_magnitude": one,
    }
    html = attribution_tearsheet(raw, sections=["credit"]).sections[0].body
    assert "Adder By Issuer (Breakdown)" in html
    assert "&lt;img src=x onerror=alert(1)&gt;" in html
    assert issuer not in html
