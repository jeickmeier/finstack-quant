"""Public feature contracts: chronology, numerical invariants, and wire parity."""

import datetime as dt
import json

import pandas as pd
import pytest

from finstack_quant import features as f
from finstack_quant.features import dataframe as fd


def test_datetime_keys_preserve_utc_chronology_across_dst_and_all_entrypoints() -> None:
    dates = [
        dt.datetime.fromisoformat("2026-11-01T01:50:00-04:00"),
        dt.datetime.fromisoformat("2026-11-01T01:10:00-05:00"),
    ]
    operations = [{"name": "ret", "family": "timeseries", "op": "returns"}]
    direct = f.transform_timeseries([100.0, 110.0], ["A"] * 2, dates, "returns")
    assert direct[0] is None
    assert direct[1] == pytest.approx(0.1)
    spec = {"values": [100.0, 110.0], "entity": ["A"] * 2, "order": dates, "operations": operations}
    assert f.transform_panel(spec).get_column("ret") == direct
    typed = f.PanelTransformSpec([100.0, 110.0], operations, entity=["A"] * 2, order=dates)
    assert f.transform_panel(typed).get_column("ret") == direct
    index = pd.to_datetime(["2026-11-01T05:50Z", "2026-11-01T06:10Z"], utc=True).tz_convert("America/New_York")
    frame = pd.DataFrame({"v": [100.0, 110.0], "asset": ["A"] * 2}, index=index)
    assert fd.timeseries(frame, "v", "asset", op="returns").iloc[1] == pytest.approx(0.1)
    assert fd.panel(frame, "v", operations, entity="asset")["ret"].iloc[1] == pytest.approx(0.1)


def test_equal_instants_share_a_partition_without_losing_nanosecond_precision() -> None:
    same_time = [dt.datetime.fromisoformat("2026-01-01T10:00:00-05:00"), pd.Timestamp("2026-01-01T15:00:00Z")]
    assert f.transform_cross_sectional([1.0, 3.0], same_time, "rank") == [0.0, 1.0]
    keys = [pd.Timestamp("2026-01-01T00:00:00.000000002Z"), pd.Timestamp("2026-01-01T00:00:00.000000001Z")]
    assert f.transform_timeseries([2.0, 1.0], ["A"] * 2, keys, "diff") == [1.0, None]
    naive = dt.datetime(2026, 1, 1)
    with pytest.raises(ValueError, match="aware and naive"):
        f.transform_timeseries([1.0, 2.0], ["A"] * 2, [naive, same_time[0]], "returns")


def test_panel_nonfinite_input_and_output_policies_match_direct_calls() -> None:
    values = [1.0, float("inf"), float("nan"), None, 3.0]
    operations = [{"name": "rank", "family": "cross_sectional", "op": "rank"}]
    expected = [0.0, None, None, None, 1.0]
    assert f.transform_cross_sectional(values, ["d"] * 5, "rank") == expected
    typed = f.PanelTransformSpec(values, operations, time_key=["d"] * 5)
    assert f.transform_panel(typed).get_column("rank") == expected
    assert (
        f.transform_panel({"values": values, "operations": operations, "time_key": ["d"] * 5}).get_column("rank")
        == expected
    )
    assert json.loads(typed.to_json())["values"] == [1.0, None, None, None, 3.0]
    frame = pd.DataFrame({"v": values, "t": ["d"] * 5})
    pd.testing.assert_series_equal(
        fd.panel(frame, "v", operations, time_key="t")["rank"],
        fd.cross_sectional(frame, "v", "t", "rank"),
        check_names=False,
    )
    spec = {
        "values": [1e308, 1e308],
        "entity": ["A"] * 2,
        "order": ["1", "2"],
        "operations": [{"name": "m", "family": "timeseries", "op": "rolling_mean", "params": {"window": 2}}],
    }
    assert f.transform_panel(spec).get_column("m") == [None, 1e308]
    assert json.loads(f.transform_panel_json(json.dumps(spec)))["columns"][0]["values"] == [None, 1e308]
    spec["operations"][0]["op"] = "rolling_sum"
    with pytest.raises(ValueError, match="non-finite"):
        f.transform_panel(spec)
    with pytest.raises(ValueError, match="non-finite"):
        f.transform_panel_json(json.dumps(spec))


def test_scaled_statistics_and_exact_fit_neutralization() -> None:
    x = [0.0, 1e-7, 2e-7]
    assert f.transform_timeseries_pairwise(x, x, ["A"] * 3, ["1", "2", "3"], "rolling_corr", {"window": 3}) == [
        None,
        None,
        1.0,
    ]
    for scale in (1e-7, 1.0, 1e7):
        assert f.neutralize([1.0, 0.0, 0.0], ["d"] * 3, [[scale, 2 * scale, 3 * scale]]) == pytest.approx([
            1 / 6,
            -1 / 3,
            1 / 6,
        ])
    assert f.neutralize_and_zscore([3.0, 5.0, 7.0], ["d"] * 3, [[1.0, 2.0, 3.0]]) == [0.0] * 3
    with pytest.raises(ValueError, match="fit_intercept"):
        f.neutralize_and_zscore([1.0, 0.0, 0.0], ["d"] * 3, [[1.0, 2.0, 3.0]], {"fit_intercept": False})


def test_risk_weights_and_caps_enforce_their_economic_constraints() -> None:
    assert f.rank_to_weights([-0.0, 0.0], ["d"] * 2) == [0.0, 0.0]
    with pytest.raises(ValueError, match="negative"):
        f.risk_scaled_weights([1.0, 2.0], ["d"] * 2, [1.0, -1.0])
    capped = f.transform_cross_sectional([-10.0, -1.0, 1.0, 10.0], ["d"] * 4, "cap_weights", {"max_abs": 0.3})
    assert capped == pytest.approx([-0.3, -0.2, 0.2, 0.3])
    assert sum(capped) == pytest.approx(0)
    assert sum(abs(w) for w in capped) == pytest.approx(1)
    with pytest.raises(ValueError, match="infeasible"):
        f.transform_cross_sectional([-1.0, 1.0], ["d"] * 2, "cap_weights", {"max_abs": 0.3})


def test_ewma_and_window_boundaries() -> None:
    with pytest.raises(ValueError, match="span"):
        f.transform_timeseries([], [], [], "ewma_vol", {"span": 0.5})
    with pytest.raises(ValueError, match="quantile"):
        f.transform_timeseries([1.0], ["A"], ["1"], "rolling_quantile", {"window": 3, "quantile": 2})
    assert f.transform_timeseries([0.01, 0.01], ["A"] * 2, ["1", "2"], "ewma_vol", {"span": 3}) == [None, 0.0]
    assert f.transform_timeseries([1.0, 1.0, 100.0], ["A"] * 3, ["1", "2", "3"], "hampel_filter", {"window": 3}) == [
        None,
        None,
        1.0,
    ]
