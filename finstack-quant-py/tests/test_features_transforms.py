from __future__ import annotations

import json

import pytest

from finstack_quant.features import (
    neutralize,
    neutralize_and_zscore,
    rank_to_weights,
    risk_scaled_weights,
    rolling_regression_residual,
    transform_cross_sectional,
    transform_cross_sectional_grouped,
    transform_panel_json,
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
        "winsorize",
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


@pytest.mark.parametrize("alias", ["clip_by_quantile", "dollar_neutral_weights"])
def test_transform_cross_sectional_rejects_removed_aliases(alias: str) -> None:
    with pytest.raises(ValueError, match=alias):
        transform_cross_sectional([1.0, 2.0], ["2026-01-01"] * 2, alias)


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
    assert weights == pytest.approx([-0.25, -0.25, 0.25, 0.25])

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
    cleaned = transform_cross_sectional(
        [1.0, 2.0, 100.0],
        time_key,
        "winsorize",
        {"lower": 0.0, "upper": 0.5},
    )
    assert cleaned == [1.0, 2.0, 2.0]

    normalized = transform_cross_sectional(
        [1.0, 2.0, 100.0],
        time_key,
        "rank",
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
            {"name": "rank", "family": "cross_sectional", "op": "rank", "input": "values"},
        ],
    }

    result = json.loads(transform_panel_json(json.dumps(spec)))
    assert result["columns"][0]["name"] == "ret1"
    assert result["columns"][0]["values"][1] == pytest.approx(0.2)
    assert result["columns"][1]["name"] == "rank"
    assert result["columns"][1]["values"][2] == 1.0


def test_periods_count_finite_observations() -> None:
    entity = ["A"] * 5
    order = ["1", "2", "3", "4", "5"]
    diff = transform_timeseries([10.0, 12.0, 13.0, None, 17.0], entity, order, "diff", {"periods": 2})
    # The None row does not advance the lag: 17 is differenced against 12.
    assert diff == [None, None, 3.0, None, 5.0]
    lag = transform_timeseries([10.0, None, 13.0], entity[:3], order[:3], "lag")
    assert lag == [None, None, 10.0]


def test_unknown_params_and_ops_are_rejected_with_listings() -> None:
    with pytest.raises(ValueError, match="unknown panel transform parameter 'windows'"):
        transform_timeseries([1.0, 2.0], ["A", "A"], ["1", "2"], "rolling_mean", {"windows": 2})
    with pytest.raises(ValueError, match="accepted ops: returns, log_returns"):
        transform_timeseries([1.0], ["A"], ["1"], "not_an_op")
    with pytest.raises(ValueError, match="accepted ops: zscore"):
        transform_cross_sectional([1.0], ["d"], "nope")


def test_op_enums_and_key_coercion() -> None:
    import datetime

    from finstack_quant.features import CrossSectionalOp, PairwiseOp, TimeSeriesOp

    assert TimeSeriesOp("returns") == TimeSeriesOp["RETURNS"]
    assert "RETURNS" in TimeSeriesOp.__members__
    assert TimeSeriesOp.values()[0] == "returns"
    assert CrossSectionalOp("winsorize").param_keys == ["lower", "upper"]
    assert PairwiseOp.values() == ["rolling_cov", "rolling_corr", "rolling_beta"]
    assert repr(TimeSeriesOp("ewma_vol")) == "TimeSeriesOp.EWMA_VOL"
    with pytest.raises(ValueError, match="accepted ops"):
        TimeSeriesOp("bogus")
    out = transform_timeseries(
        [100.0, 102.0],
        [1, 1],
        [datetime.date(2026, 1, 1), datetime.date(2026, 1, 2)],
        TimeSeriesOp("returns"),
    )
    assert out[0] is None
    assert out[1] == pytest.approx(0.02)


def test_transform_panel_typed_twin() -> None:
    import pickle

    from finstack_quant.features import PanelTransformResult, PanelTransformSpec, transform_panel

    spec = PanelTransformSpec(
        [10.0, 12.0, 20.0, 21.0],
        [{"name": "rank", "family": "cross_sectional", "op": "rank"}],
        time_key=["1", "2", "1", "2"],
    )
    result = transform_panel(spec)
    assert isinstance(result, PanelTransformResult)
    assert result.columns == ["rank"]
    assert result.get_column("rank") == [0.0, 0.0, 1.0, 1.0]
    assert transform_panel(spec.to_json()).to_json() == result.to_json()
    assert pickle.loads(pickle.dumps(result)).columns == ["rank"]  # noqa: S301 - trusted in-process round trip
    with pytest.raises(KeyError):
        result.get_column("missing")
    pd = pytest.importorskip("pandas")
    frame = result.to_dataframe(index=pd.Index([10, 11, 12, 13]))
    assert list(frame.index) == [10, 11, 12, 13]
