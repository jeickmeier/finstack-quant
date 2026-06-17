from __future__ import annotations

import json

import pytest

from finstack_quant.features import (
    clean_signal,
    neutralize,
    neutralize_and_zscore,
    normalize_signal,
    rank_to_weights,
    risk_scaled_weights,
    rolling_regression_residual,
    transform_cross_sectional,
    transform_cross_sectional_grouped,
    transform_panel,
    transform_timeseries,
    transform_timeseries_pairwise,
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


def test_transform_entrypoints_accept_expanded_feature_ops() -> None:
    values = [1.0, 2.0, 2.0, 4.0]
    time_key = ["2026-01-01"] * 4

    percentile_rank = transform_cross_sectional(values, time_key, "percentile_rank")
    assert percentile_rank == pytest.approx([0.2, 0.5, 0.5, 0.8])

    buckets = transform_cross_sectional(
        values,
        time_key,
        "quantile_bucket",
        {"buckets": 4},
    )
    assert buckets == [0.0, 1.0, 1.0, 3.0]

    clipped = transform_cross_sectional(
        values,
        time_key,
        "clip_by_quantile",
        {"lower": 0.25, "upper": 0.75},
    )
    assert clipped == [1.75, 2.0, 2.0, 2.5]

    weights = transform_cross_sectional(values, time_key, "long_short_weights")
    assert weights == pytest.approx([-0.35714285714285715, -0.07142857142857142, -0.07142857142857142, 0.5])

    filled = transform_cross_sectional(
        [1.0, None, float("nan")],
        ["2026-01-01"] * 3,
        "fill_missing",
        {"value": 7.0},
    )
    assert filled == [1.0, 7.0, 7.0]

    entity = ["A", "A", "A"]
    order = ["2026-01-01", "2026-01-02", "2026-01-03"]
    diff = transform_timeseries([1.0, 3.0, 6.0], entity, order, "diff")
    assert diff == [None, 2.0, 3.0]

    ewma_mean = transform_timeseries(
        [1.0, 3.0, 5.0],
        entity,
        order,
        "ewma_mean",
        {"span": 3.0},
    )
    assert ewma_mean == pytest.approx([1.0, 2.0, 3.5])


def test_finance_specific_transform_entrypoints() -> None:
    time_key = ["2026-01-01"] * 4
    grouped = transform_cross_sectional_grouped(
        [1.0, 3.0, 10.0, 14.0],
        time_key,
        ["tech", "tech", "fin", "fin"],
        "zscore",
    )
    assert grouped == pytest.approx([-1.0, 1.0, -1.0, 1.0])

    residual = neutralize(
        [1.0, 2.0, 2.0, 4.0],
        time_key,
        [[0.0, 1.0, 0.0, 1.0]],
    )
    assert residual == pytest.approx([-0.5, -1.0, 0.5, 1.0])

    weights = risk_scaled_weights(
        [1.0, 2.0, 2.0, 4.0],
        time_key,
        [1.0, 2.0, 1.0, 2.0],
    )
    assert weights == pytest.approx([1.0 / 6.0, 1.0 / 6.0, 1.0 / 3.0, 1.0 / 3.0])

    entity = ["A", "A", "A"]
    order = ["2026-01-01", "2026-01-02", "2026-01-03"]
    beta = transform_timeseries_pairwise(
        [1.0, 2.0, 3.0],
        [1.0, 2.0, 4.0],
        entity,
        order,
        "rolling_beta",
        {"window": 3, "min_periods": 3},
    )
    assert beta[:2] == [None, None]
    assert beta[2] == pytest.approx(9.0 / 14.0)

    rolling_residual = rolling_regression_residual(
        [1.0, 2.0, 5.0],
        [[0.0, 1.0, 2.0]],
        entity,
        order,
        {"window": 3, "min_periods": 3},
    )
    assert rolling_residual[:2] == [None, None]
    assert rolling_residual[2] == pytest.approx(1.0 / 3.0)


def test_pipeline_helper_entrypoints() -> None:
    time_key = ["2026-01-01"] * 3
    cleaned = clean_signal(
        [1.0, 2.0, 100.0],
        time_key,
        {"lower": 0.0, "upper": 0.5},
    )
    assert cleaned == [1.0, 2.0, 2.0]

    normalized = normalize_signal(
        [1.0, 2.0, 100.0],
        time_key,
        {"method": "rank"},
    )
    assert normalized == [0.0, 0.5, 1.0]

    weights = rank_to_weights([1.0, 2.0, 100.0], time_key)
    assert weights == pytest.approx([-0.5, 0.0, 0.5])

    neutralized = neutralize_and_zscore(
        [1.0, 2.0, 2.0, 4.0],
        ["2026-01-01"] * 4,
        [[0.0, 1.0, 0.0, 1.0]],
    )
    assert neutralized == pytest.approx([
        -0.6324555320336759,
        -1.2649110640673518,
        0.6324555320336759,
        1.2649110640673518,
    ])


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
