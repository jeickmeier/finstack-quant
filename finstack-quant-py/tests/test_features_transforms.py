from __future__ import annotations

import json

import pytest

from finstack_quant.features import (
    transform_cross_sectional,
    transform_panel,
    transform_timeseries,
)


def test_transform_timeseries_and_cross_sectional_entrypoints() -> None:
    values = [12.0, 10.0, 21.0, 20.0]
    entity = ["A", "A", "B", "B"]
    order = ["2026-01-02", "2026-01-01", "2026-01-02", "2026-01-01"]

    returns = transform_timeseries(values, entity, order, "returns", {"periods": 1})
    assert returns[0] == pytest.approx(0.2)
    assert returns[1] is None
    assert returns[2] == pytest.approx(0.05)

    ranks = transform_cross_sectional(
        [1.0, 2.0, 100.0, 5.0],
        ["2026-01-01", "2026-01-01", "2026-01-01", "2026-01-02"],
        "rank",
    )
    assert ranks == [0.0, 0.5, 1.0, 0.0]


def test_transform_panel_json_entrypoint() -> None:
    spec = {
        "values": [10.0, 12.0, 20.0, 21.0],
        "entity": ["A", "A", "B", "B"],
        "order": ["2026-01-01", "2026-01-02", "2026-01-01", "2026-01-02"],
        "time_key": ["2026-01-01", "2026-01-02", "2026-01-01", "2026-01-02"],
        "operations": [
            {"name": "ret1", "family": "timeseries", "op": "returns", "params": {"periods": 1}},
            {"name": "rank", "family": "cross_sectional", "op": "rank"},
        ],
    }

    result = json.loads(transform_panel(json.dumps(spec)))
    assert result["columns"]["ret1"][1] == pytest.approx(0.2)
    assert result["columns"]["rank"][2] == 1.0
