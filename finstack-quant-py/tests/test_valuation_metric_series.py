from __future__ import annotations

import json
from pathlib import Path

from finstack_quant.portfolio import PortfolioMetrics
from finstack_quant.valuations import ValuationResult

DATA = Path(__file__).parent / "data"


def test_metric_series_decodes_components_and_preserves_measure_order() -> None:
    payload = json.loads((DATA / "instrument_bond_result.json").read_text())
    payload["measures"] = {
        "bucketed_dv01": -3.0,
        "bucketed_dv01::USD_x2dOIS::10y": -1.0,
        "bucketed_cs01::ACME": 8.0,
        "bucketed_dv01::EUR_x2fUSD::_empty": -2.0,
        "bucketed_dv01::curve-ray": -4.0,
        "bucketed_dv01::curve_x2dray": -5.0,
        "bucketed_dv01::curve_xray": -6.0,
        "bucketed_dv01::curve_x5fx2dray": -7.0,
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
                "bucketed_dv01::USD_x2dOIS::10y": {
                    "metric_id": "bucketed_dv01::USD_x2dOIS::10y",
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
