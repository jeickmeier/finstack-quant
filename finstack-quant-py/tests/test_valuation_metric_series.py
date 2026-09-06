from __future__ import annotations

import json
from pathlib import Path

import pytest

from finstack_quant.portfolio import PortfolioMetrics
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.composite import WeightingMethod

DATA = Path(__file__).parent / "data"


def test_metric_series_decodes_components_and_preserves_measure_order() -> None:
    payload = json.loads((DATA / "instrument_bond_result.json").read_text())
    payload["measures"] = {
        "bucketed_dv01": -3.0,
        "bucketed_dv01::USD-OIS::10y": -1.0,
        "bucketed_cs01::ACME": 8.0,
        "bucketed_dv01::EUR/USD::_empty": -2.0,
        "bucketed_dv01::curve-ray": -4.0,
        "bucketed_dv01::curve_x5fx2dray": -5.0,
        "bucketed_dv01::curve_x5fxray": -6.0,
        "bucketed_dv01::curve_x5fx5fx2dray": -7.0,
    }
    result = ValuationResult.from_json(json.dumps(payload))

    assert result.metric_series("bucketed_dv01") == [
        (["USD-OIS", "10y"], -1.0),
        (["EUR/USD", ""], -2.0),
        (["curve-ray"], -4.0),
        (["curve_x2dray"], -5.0),
        (["curve_xray"], -6.0),
        (["curve_x5fx2dray"], -7.0),
    ]
    assert result.metric_series("dv01") == []


def test_portfolio_metrics_series_preserves_aggregated_metric_payloads() -> None:
    metrics = PortfolioMetrics.from_json(
        json.dumps({
            "aggregated": {
                "bucketed_dv01": {
                    "metric_id": "bucketed_dv01",
                    "total": -3.0,
                    "by_entity": {},
                },
                "bucketed_dv01::USD-OIS::10y": {
                    "metric_id": "bucketed_dv01::USD-OIS::10y",
                    "total": -1.0,
                    "by_entity": {"FUND_Z": -0.25, "FUND_A": -0.75},
                },
            },
            "by_position": {},
        })
    )

    series = metrics.metric_series("bucketed_dv01")
    assert series == [(["USD-OIS", "10y"], -1.0, {"FUND_Z": -0.25, "FUND_A": -0.75})]
    assert list(series[0][2]) == ["FUND_Z", "FUND_A"]


@pytest.mark.parametrize("location", ["aggregated", "by_position"])
def test_portfolio_metric_maps_reject_noncanonical_keys(location: str) -> None:
    key = "bucketed_dv01::USD_x2dOIS::10y"
    payload = {"aggregated": {}, "by_position": {}}
    if location == "aggregated":
        payload[location] = {key: {"metric_id": key, "total": 1.0, "by_entity": {}}}
    else:
        payload[location] = {"POS": {"currency": "USD", "metrics": {key: 1.0}}}
    with pytest.raises(ValueError, match="noncanonical"):
        PortfolioMetrics.from_json(json.dumps(payload))


def test_metric_weighted_parses_wire_keys() -> None:
    assert isinstance(WeightingMethod.metric_weighted("pv01::USD-OIS", "A", 1.0), WeightingMethod)
    with pytest.raises(ValueError, match="noncanonical"):
        WeightingMethod.metric_weighted("pv01::USD_x2dOIS", "A", 1.0)


def test_metric_series_uses_canonical_custom_base_keys() -> None:
    base = "custom_x5fxray"
    key = f"{base}::USD-OIS"
    payload = json.loads((DATA / "instrument_bond_result.json").read_text())
    payload["measures"] = {key: 1.0}
    result = ValuationResult.from_json(json.dumps(payload))
    assert result.metric_series(base) == [(["USD-OIS"], 1.0)]
    metrics = PortfolioMetrics.from_json(
        json.dumps({
            "aggregated": {key: {"metric_id": key, "total": 1.0, "by_entity": {}}},
            "by_position": {},
        })
    )
    assert metrics.metric_series(base) == [(["USD-OIS"], 1.0, {})]
    for values in [result, metrics]:
        with pytest.raises(ValueError, match="noncanonical"):
            values.metric_series("custom_xray")
