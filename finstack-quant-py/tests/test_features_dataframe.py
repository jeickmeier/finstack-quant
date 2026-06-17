from __future__ import annotations

import pandas as pd
import pytest

from finstack_quant.features import dataframe as fdf


def test_dataframe_helpers_return_aligned_series_and_frames() -> None:
    df = pd.DataFrame(
        {
            "date": ["2026-01-01", "2026-01-01", "2026-01-02", "2026-01-02"],
            "asset": ["A", "B", "A", "B"],
            "sector": ["tech", "tech", "fin", "fin"],
            "signal": [1.0, 3.0, 2.0, 4.0],
            "beta": [0.0, 1.0, 0.0, 1.0],
            "vol": [1.0, 2.0, 1.0, 2.0],
        },
        index=["r0", "r1", "r2", "r3"],
    )

    cross = fdf.cross_sectional(df, "signal", "date", "rank")
    assert isinstance(cross, pd.Series)
    assert list(cross.index) == list(df.index)
    assert cross.tolist() == [0.0, 1.0, 0.0, 1.0]

    ts = fdf.timeseries(df, "signal", "asset", "date", "diff")
    assert ts.iloc[0:2].isna().all()
    assert ts.iloc[2:].tolist() == [1.0, 1.0]

    grouped = fdf.grouped(df, "signal", "date", "sector", "zscore")
    assert grouped.tolist() == pytest.approx([-1.0, 1.0, -1.0, 1.0])

    residual = fdf.neutralize(df, "signal", "date", ["beta"])
    assert residual.tolist() == pytest.approx([0.0, 0.0, 0.0, 0.0])

    weights = fdf.risk_scaled_weights(df, "signal", "date", "vol")
    assert weights.tolist() == pytest.approx([0.4, 0.6, 0.5, 0.5])

    panel = fdf.panel(
        df,
        "signal",
        [
            {"name": "rank", "family": "cross_sectional", "op": "rank"},
            {"name": "diff", "family": "timeseries", "op": "diff"},
        ],
        entity="asset",
        order="date",
        time_key="date",
    )
    assert isinstance(panel, pd.DataFrame)
    assert list(panel.index) == list(df.index)
    assert panel["rank"].tolist() == [0.0, 1.0, 0.0, 1.0]
    assert panel["diff"].iloc[0:2].isna().all()
    assert panel["diff"].iloc[2:].tolist() == [1.0, 1.0]


def test_dataframe_pipeline_helpers_delegate_to_feature_transforms() -> None:
    df = pd.DataFrame({
        "date": ["2026-01-01"] * 4,
        "signal": [1.0, 2.0, 2.0, 4.0],
        "beta": [0.0, 1.0, 0.0, 1.0],
    })

    cleaned = fdf.clean_signal(df, "signal", "date", {"lower": 0.0, "upper": 0.5})
    assert cleaned.tolist() == [1.0, 2.0, 2.0, 2.0]

    normalized = fdf.normalize_signal(df, "signal", "date", {"method": "rank"})
    assert normalized.tolist() == [0.0, 1.0 / 3.0, 1.0 / 3.0, 1.0]

    weights = fdf.rank_to_weights(df, "signal", "date")
    assert weights.tolist() == pytest.approx([
        -0.35714285714285715,
        -0.07142857142857142,
        -0.07142857142857142,
        0.5000000000000001,
    ])

    scored = fdf.neutralize_and_zscore(df, "signal", "date", ["beta"])
    assert scored.tolist() == pytest.approx([
        -0.6324555320336759,
        -1.2649110640673518,
        0.6324555320336759,
        1.2649110640673518,
    ])


def test_dataframe_helpers_normalize_pandas_missing_values() -> None:
    df = pd.DataFrame({
        "date": ["2026-01-01", "2026-01-01", "2026-01-01"],
        "signal": pd.Series([1.0, pd.NA, float("inf")], dtype="Float64"),
    })

    filled = fdf.cross_sectional(
        df,
        "signal",
        "date",
        "fill_missing",
        {"value": 7.0},
    )
    assert filled.tolist() == [1.0, 7.0, 7.0]


def test_dataframe_cross_sectional_uses_datetime_index_when_time_key_omitted() -> None:
    df = pd.DataFrame(
        {"signal": [1.0, 3.0, 2.0, 4.0]},
        index=pd.to_datetime([
            "2026-01-01",
            "2026-01-01",
            "2026-01-02",
            "2026-01-02",
        ]),
    )

    ranked = fdf.cross_sectional(df, "signal", op="rank")

    assert ranked.tolist() == [0.0, 1.0, 0.0, 1.0]
    assert ranked.index.equals(df.index)


def test_dataframe_helpers_resolve_explicit_multiindex_levels() -> None:
    index = pd.MultiIndex.from_arrays(
        [
            pd.to_datetime(["2026-01-01", "2026-01-01", "2026-01-02", "2026-01-02"]),
            ["A", "B", "A", "B"],
            ["tech", "tech", "fin", "fin"],
        ],
        names=["date", "asset", "sector"],
    )
    df = pd.DataFrame({"signal": [1.0, 3.0, 2.0, 4.0]}, index=index)

    with pytest.raises(KeyError, match="time_key is required"):
        fdf.cross_sectional(df, "signal", op="rank")

    cross = fdf.cross_sectional(df, "signal", "date", "rank")
    cross_by_position = fdf.cross_sectional(df, "signal", 0, "rank")
    ts = fdf.timeseries(df, "signal", "asset", "date", "diff")
    grouped = fdf.grouped(df, "signal", "date", "sector", "zscore")

    assert cross.tolist() == [0.0, 1.0, 0.0, 1.0]
    assert cross_by_position.tolist() == cross.tolist()
    assert ts.iloc[0:2].isna().all()
    assert ts.iloc[2:].tolist() == [1.0, 1.0]
    assert grouped.tolist() == pytest.approx([-1.0, 1.0, -1.0, 1.0])


def test_dataframe_key_resolution_rejects_column_index_ambiguity() -> None:
    df = pd.DataFrame(
        {
            "date": ["2026-01-01", "2026-01-01"],
            "signal": [1.0, 2.0],
        },
        index=pd.Index(["2026-01-02", "2026-01-02"], name="date"),
    )

    with pytest.raises(ValueError, match="ambiguous key"):
        fdf.cross_sectional(df, "signal", "date", "rank")
