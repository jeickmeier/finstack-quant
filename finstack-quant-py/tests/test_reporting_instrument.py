"""Tests for the instrument tear sheet."""

from __future__ import annotations

import json
from pathlib import Path

from finstack_quant.reporting import instrument as ins
from finstack_quant.valuations import list_standard_metrics


def test_recommended_metrics_known_types() -> None:
    for t in ("bond", "credit_default_swap", "equity_option", "interest_rate_swap"):
        ids = ins.recommended_metrics(t)
        assert isinstance(ids, list), t
        assert ids, t
    assert "dv01" in ins.recommended_metrics("bond")
    assert "par_spread" in ins.recommended_metrics("credit_default_swap")
    assert "delta" in ins.recommended_metrics("equity_option")


def test_recommended_metrics_unknown_type_is_empty_or_minimal() -> None:
    assert isinstance(ins.recommended_metrics("nonesuch"), list)


def test_recommended_metrics_ids_are_in_catalog() -> None:
    catalog = set(list_standard_metrics())
    for t in ("bond", "credit_default_swap", "equity_option", "interest_rate_swap"):
        for mid in ins.recommended_metrics(t):
            assert mid in catalog, f"{t}:{mid} not in metric catalog"


class _FakeResult:
    """Stand-in for ValuationResult exposing the methods instrument.py uses."""

    def __init__(self, payload: dict) -> None:
        self._p = payload

    @property
    def instrument_id(self) -> str:
        return self._p["instrument_id"]

    @property
    def currency(self) -> str:
        return self._p["value"]["currency"]

    @property
    def price(self) -> float:
        return float(self._p["value"]["amount"])

    def metric_keys(self) -> list[str]:
        return list(self._p["measures"].keys())

    def get_metric(self, key: str) -> float | None:
        return self._p["measures"].get(key)

    def all_covenants_passed(self) -> bool:
        return not self._p.get("covenants")

    def failed_covenants(self) -> list[str]:
        return []

    def to_json(self) -> str:
        return json.dumps(self._p)


def _fake_bond_result() -> _FakeResult:
    return _FakeResult({
        "schema_version": 1,
        "instrument_id": "TEST-BOND",
        "as_of": "2026-06-19",
        "value": {"amount": "10283500.0", "currency": "USD"},
        "measures": {
            "dv01": 6420.0,
            "ytm": 0.0394,
            "duration_mod": 6.42,
            "bucketed_dv01::USD-OIS::5y": 1350.0,
            "bucketed_dv01::USD-OIS::10y": 2620.0,
            "bucketed_dv01::USD-OIS::2y": 230.0,
        },
        "meta": {"numeric_mode": "Decimal", "fx_policy_applied": None, "version": "0.1.0"},
        "details": None,
        "covenants": None,
    })


def test_parse_result_meta() -> None:
    m = ins._parse_result(_fake_bond_result())
    assert m["as_of"] == "2026-06-19"
    assert m["numeric_mode"] == "Decimal"


def test_bucketed_series_orders_by_tenor() -> None:
    series = ins._bucketed_series(_fake_bond_result(), "bucketed_dv01")
    # ordered by the standard tenor grid: 2y, 5y, 10y (3m/6m/1y absent)
    assert [t for t, _ in series] == ["2y", "5y", "10y"]
    assert dict(series)["10y"] == 2620.0


def test_metric_cell_formats_by_unit() -> None:
    assert ins._metric_cell("ytm", 0.0394) == ("Yield to Maturity", "3.94%", "")
    assert ins._metric_cell("z_spread", 0.0078)[1] == "78 bp"
    assert ins._metric_cell("dv01", 6420.0)[1] in ("6,420", "6,420.00")
    # clean_price is a full dollar amount (not per-100) — formatted as money (0dp)
    assert ins._metric_cell("clean_price", 9725674.0)[1] in ("9,725,674", "$9,725,674")
    _lbl, _val, cls = ins._metric_cell("jump_to_default", -5816000.0)
    assert cls == "neg"


def test_definition_terms_bond() -> None:
    defn = {
        "type": "bond",
        "spec": {
            "id": "ACME-34",
            "notional": {"amount": "10000000", "currency": "USD"},
            "issue_date": "2024-03-15",
            "maturity": "2034-03-15",
            "cashflow_spec": {"Fixed": {"rate": 0.0425, "freq": {"count": 6, "unit": "months"}, "dc": "Thirty360"}},
            "discount_curve_id": "USD-OIS",
        },
    }
    cols = ins._definition_terms(defn)
    flat = [kv for col in cols for kv in col]
    keys = {k for k, _ in flat}
    assert "Notional" in keys
    assert "Maturity" in keys
    assert "Coupon" in keys


def test_definition_terms_generic_fallback() -> None:
    defn = {"type": "mystery", "spec": {"id": "X", "notional": {"amount": "5", "currency": "USD"}, "rate": 0.01}}
    cols = ins._definition_terms(defn)
    flat = [kv for col in cols for kv in col]
    assert flat  # produced something from spec scalars


def test_instrument_tearsheet_generic_from_fake_result() -> None:
    import datetime as dt

    from finstack_quant.reporting import instrument_tearsheet
    from finstack_quant.reporting.document import TearSheet

    ts = instrument_tearsheet(_fake_bond_result(), generated=dt.date(2026, 6, 19))
    assert isinstance(ts, TearSheet)
    html = ts.to_html()
    assert "TEST-BOND" in html
    assert "DV01" in html
    assert "Yield to Maturity" in html
    # key-rate bar present (bucketed_dv01 keys exist on the fake result)
    assert "Key-Rate" in html


def test_instrument_tearsheet_bond_with_definition_and_cashflows() -> None:
    import datetime as dt

    import pandas as pd

    from finstack_quant.reporting import instrument_tearsheet

    defn = {
        "type": "bond",
        "spec": {
            "id": "TEST-BOND",
            "notional": {"amount": "10000000", "currency": "USD"},
            "issue_date": "2024-03-15",
            "maturity": "2034-03-15",
            "cashflow_spec": {"Fixed": {"rate": 0.0425, "freq": {"count": 6, "unit": "months"}, "dc": "Thirty360"}},
            "discount_curve_id": "USD-OIS",
        },
    }
    cf = pd.DataFrame({
        "date": [dt.date(2027, 3, 15), dt.date(2034, 3, 15)],
        "kind": ["coupon", "principal"],
        "amount": [212500.0, 10000000.0],
        "rate": [0.0425, None],
        "discount_factor": [0.98, 0.71],
        "pv": [208000.0, 7100000.0],
    })
    html = instrument_tearsheet(
        _fake_bond_result(), definition=defn, cashflows=cf, generated=dt.date(2026, 6, 19)
    ).to_html()
    assert "Cashflow Ladder" in html
    assert "Cashflow Schedule" in html
    assert "Maturity" in html  # definition table


def test_instrument_tearsheet_unknown_section_raises() -> None:
    import datetime as dt

    import pytest

    from finstack_quant.reporting import instrument_tearsheet

    with pytest.raises(ValueError, match=r"unknown section"):
        instrument_tearsheet(_fake_bond_result(), sections=["typo"], generated=dt.date(2026, 6, 19))


def test_generic_kpis_skip_composite_keys() -> None:
    r = _FakeResult({
        "schema_version": 1,
        "instrument_id": "X",
        "as_of": "2026-06-19",
        "value": {"amount": "100.0", "currency": "USD"},
        "measures": {
            "bucketed_dv01::C::5y": 1.0,
            "bucketed_dv01::C::10y": 2.0,  # composites FIRST
            "ytm": 0.04,
            "dv01": 500.0,
            "duration_mod": 6.0,
        },
        "meta": {"numeric_mode": "Decimal"},
        "details": None,
        "covenants": None,
    })
    kpis = ins._kpis(r, "")
    labels = [k.label for k in kpis]
    assert "PV" in labels
    # the 3 non-composite metrics are included despite composites appearing first
    assert len([k for k in kpis if k.label != "PV"]) == 3


def test_cashflow_blocks_from_dataframe() -> None:
    import datetime as dt

    import pandas as pd

    df = pd.DataFrame({
        "date": [dt.date(2027, 3, 15), dt.date(2027, 9, 15), dt.date(2034, 3, 15)],
        "kind": ["coupon", "coupon", "principal"],
        "amount": [212500.0, 212500.0, 10000000.0],
        "rate": [0.0425, 0.0425, None],
        "discount_factor": [0.98, 0.97, 0.71],
        "pv": [208000.0, 206000.0, 7100000.0],
    })
    ladder, schedule = ins._cashflow_blocks(df)
    # ladder grouped by year: 2027 has coupons; 2034 has the principal
    years = [p for p, _, _, _ in ladder]
    assert "2034" in years
    by_year = {p: (c, pr) for p, c, pr, _ in ladder}
    assert by_year["2034"][1] > 0  # principal present in 2034
    # schedule rows carry the original columns
    assert schedule
    assert "Date" in schedule[0]


# ---------------------------------------------------------------------------
# Task 8: Bond golden test + CDS/option smoke tests
# ---------------------------------------------------------------------------

DATA = Path(__file__).parent / "data"


def _bond_definition() -> dict:
    return {
        "type": "bond",
        "spec": {
            "id": "ACME-4.25-2034",
            "notional": {"amount": "10000000", "currency": "USD"},
            "issue_date": "2024-03-15",
            "maturity": "2034-03-15",
            "cashflow_spec": {"Fixed": {"rate": 0.0425, "freq": {"count": 6, "unit": "months"}, "dc": "Thirty360"}},
            "discount_curve_id": "USD-OIS",
        },
    }


def _bond_golden_html() -> str:
    import datetime as dt

    import pandas as pd

    from finstack_quant.reporting import instrument_tearsheet
    from finstack_quant.valuations import ValuationResult

    result = ValuationResult.from_json((DATA / "instrument_bond_result.json").read_text())
    records = json.loads((DATA / "instrument_bond_cashflows.json").read_text())
    cf = pd.DataFrame(records)
    cf["date"] = pd.to_datetime(cf["date"]).dt.date
    ts = instrument_tearsheet(result, definition=_bond_definition(), cashflows=cf, generated=dt.date(2026, 6, 19))
    return ts.to_html()


def test_instrument_bond_matches_golden() -> None:
    golden = DATA / "instrument_bond_tearsheet_golden.html"
    assert golden.exists(), "golden missing — regenerate (Task 8 Step 4)"
    assert _bond_golden_html() == golden.read_text(encoding="utf-8")


def test_instrument_cds_renders_credit_blocks() -> None:
    import datetime as dt

    from finstack_quant.reporting import instrument_tearsheet

    cds = _FakeResult({
        "schema_version": 1,
        "instrument_id": "ACME-5Y-CDS",
        "as_of": "2026-06-19",
        "value": {"amount": "184200.0", "currency": "USD"},
        "measures": {
            "par_spread": 0.0137,
            "cs01": 4930.0,
            "jump_to_default": -5816000.0,
            "bucketed_cs01::ACME-SR::5y": 2510.0,
            "bucketed_cs01::ACME-SR::3y": 940.0,
            "default01": 9690.0,
        },
        "meta": {"numeric_mode": "Decimal"},
        "details": None,
        "covenants": None,
    })
    defn = {
        "type": "credit_default_swap",
        "spec": {
            "id": "ACME",
            "notional": {"amount": "10000000", "currency": "USD"},
            "side": "Buy",
            "premium": {
                "start": "2024-06-20",
                "end": "2029-06-20",
                "spread_bp": 100,
                "frequency": {"count": 3, "unit": "months"},
            },
            "protection": {"credit_curve_id": "ACME-SR", "recovery_rate": 0.4},
            "doc_clause": "XR14",
        },
    }
    html = instrument_tearsheet(cds, definition=defn, generated=dt.date(2026, 6, 19)).to_html()
    assert "Bucketed CS01" in html
    assert "Par Spread" in html


def test_instrument_option_renders_payoff() -> None:
    import datetime as dt

    from finstack_quant.reporting import instrument_tearsheet

    opt = _FakeResult({
        "schema_version": 1,
        "instrument_id": "SPX-5000-C",
        "as_of": "2026-06-19",
        "value": {"amount": "218.40", "currency": "USD"},
        "measures": {
            "delta": 0.512,
            "vega": 9.83,
            "implied_vol": 0.176,
            "theta": -1.42,
            "gamma": 0.0021,
        },
        "meta": {"numeric_mode": "Decimal"},
        "details": None,
        "covenants": None,
    })
    defn = {
        "type": "equity_option",
        "spec": {
            "id": "SPX-5000-C",
            "underlying_ticker": "SPX",
            "strike": 5000.0,
            "option_type": "Call",
            "exercise_style": "European",
            "expiry": "2026-12-18",
            "discount_curve_id": "USD-OIS",
            "vol_surface_id": "SPX-VOL",
        },
    }
    html = instrument_tearsheet(opt, definition=defn, generated=dt.date(2026, 6, 19)).to_html()
    assert "Payoff at Expiry" in html
    assert "Delta" in html
