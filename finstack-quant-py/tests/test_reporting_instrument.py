"""Tests for the instrument tear sheet."""

from __future__ import annotations

import json

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
    assert ins._metric_cell("clean_price", 101.96)[1] == "101.96"
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
