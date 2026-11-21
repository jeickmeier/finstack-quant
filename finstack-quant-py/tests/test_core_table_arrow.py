"""Structural and behavioral tests for ``finstack_quant.core.table``.

Verifies ``ArrowTable`` and ``LabeledSeries``: Arrow PyCapsule stream and
schema exports (``__arrow_c_stream__``, ``__arrow_c_schema__``), zero-copy
``pyarrow`` / ``polars`` interoperability, pure-Rust ``pandas`` fallback when
``pyarrow`` is unavailable, IPC/pickle persistence, dict-like indexing on
``LabeledSeries``, and HTML notebook rendering.
"""

from __future__ import annotations

import math
import pickle
import sys

import numpy as np
import pandas as pd
import polars as pl
import pyarrow as pa
import pytest

import finstack_quant.core.table as table_module
from finstack_quant.core.table import ArrowTable, LabeledSeries


def test_table_module_exports_expected_classes() -> None:
    """The ``finstack_quant.core.table`` module exports ``ArrowTable`` and ``LabeledSeries``."""
    assert table_module.__all__ == ["ArrowTable", "LabeledSeries"]
    assert table_module.ArrowTable is ArrowTable
    assert table_module.LabeledSeries is LabeledSeries


def test_table_module_reports_qualified_package() -> None:
    """The submodule is registered under the ``finstack_quant.core`` package."""
    assert table_module.__package__ == "finstack_quant.core.table"
    assert table_module.__name__ == "finstack_quant.core.table"


def test_arrow_table_has_expected_methods() -> None:
    """``ArrowTable`` exposes row/column/schema/dict/records/capsule accessors."""
    for name in (
        "num_rows",
        "num_columns",
        "shape",
        "column_names",
        "schema",
        "to_dict",
        "to_records",
        "to_pandas",
        "to_polars",
        "to_pyarrow",
        "__arrow_c_stream__",
        "__arrow_c_schema__",
        "_repr_html_",
        "__repr__",
    ):
        assert hasattr(ArrowTable, name), f"ArrowTable missing `{name}`"


def test_arrow_table_module_and_name() -> None:
    """``ArrowTable`` and ``LabeledSeries`` report their canonical module path."""
    assert ArrowTable.__module__ == "finstack_quant.core.table"
    assert ArrowTable.__name__ == "ArrowTable"
    assert LabeledSeries.__module__ == "finstack_quant.core.table"
    assert LabeledSeries.__name__ == "LabeledSeries"


def test_arrow_table_class_docstring_cites_pycapsule_spec() -> None:
    """The class docs point readers at the Arrow PyCapsule interface spec."""
    doc = ArrowTable.__doc__ or ""
    assert "PyCapsule" in doc


def test_arrow_c_stream_docstring_mentions_capsule_name_and_schema_arg() -> None:
    """``__arrow_c_stream__`` documents the capsule name and ``requested_schema``."""
    doc = ArrowTable.__arrow_c_stream__.__doc__ or ""
    assert "arrow_array_stream" in doc
    assert "requested_schema" in doc


def test_arrow_table_has_no_python_constructor() -> None:
    """``ArrowTable`` is only producible from Rust or :meth:`from_ipc`."""
    with pytest.raises(TypeError):
        ArrowTable()


def test_arrow_table_shape_schema_dict_records_and_html() -> None:
    """``ArrowTable`` supports ``shape``, ``__arrow_c_schema__``, ``to_dict``, ``to_records``, and ``_repr_html_``."""
    s = LabeledSeries(["A&B", "<C>"], [1.25, 2.5], name="val<x>", index_name="key&1")
    at = s.to_arrow()
    assert at.shape == (2, 2)
    assert (at.num_rows, at.num_columns) == (2, 2)
    assert at.column_names() == ["key&1", "val<x>"]

    pa_schema = pa.schema(at)
    assert pa_schema.names == ["key&1", "val<x>"]
    assert at.to_dict() == {"key&1": ["A&B", "<C>"], "val<x>": [1.25, 2.5]}
    assert at.to_records() == [
        {"key&1": "A&B", "val<x>": 1.25},
        {"key&1": "<C>", "val<x>": 2.5},
    ]

    html = at._repr_html_()
    assert "&amp;" in html
    assert "&lt;C&gt;" in html
    assert "val&lt;x&gt;" in html
    assert "2 rows &times; 2 columns" in html


def test_arrow_table_html_truncates_over_25_rows() -> None:
    """``_repr_html_`` truncates large tables to head + ellipsis + tail."""
    labels = [f"k{i:02d}" for i in range(30)]
    values = [float(i) for i in range(30)]
    at = LabeledSeries(labels, values, name="metric").to_arrow()
    html = at._repr_html_()
    assert "<td>k00</td>" in html
    assert "<td>k09</td>" in html
    assert "<td>...</td>" in html
    assert "<td>k29</td>" in html
    assert "<td>k15</td>" not in html
    assert "30 rows &times; 2 columns" in html


def test_arrow_table_to_pandas_fallback_without_pyarrow(monkeypatch: pytest.MonkeyPatch) -> None:
    """``to_pandas`` falls back cleanly when ``pyarrow`` is not importable."""
    series_pop = LabeledSeries(["FUND", "BENCH"], [1.25, 0.75], name="sharpe", index_name="ticker")
    series_null = LabeledSeries(["FUND", "BENCH"], [1.25, None], name="sharpe", index_name="ticker")
    series_empty = LabeledSeries([], [], name="sharpe", index_name="ticker")

    for src in (series_pop, series_null, series_empty):
        at = src.to_arrow()
        expected = at.to_pandas()
        monkeypatch.setitem(sys.modules, "pyarrow", None)
        fallback = at.to_pandas()
        monkeypatch.delitem(sys.modules, "pyarrow")
        pd.testing.assert_frame_equal(fallback, expected, check_dtype=True)


def test_labeled_series_dtypes_and_empty() -> None:
    """``LabeledSeries`` infers and honors ``float64``, nullable ``float64``, ``int64``, and empty inputs."""
    f_series = LabeledSeries(["a", "b"], [1.0, 2.5], name="x", index_name="id")
    assert f_series.name == "x"
    assert f_series.index_name == "id"
    assert f_series.index == ["a", "b"]
    assert f_series.to_arrow().schema() == [("id", "Utf8", False), ("x", "Float64", False)]

    nf_series = LabeledSeries(["a", "b", "c"], [1.0, None, float("nan")], name="x")
    assert nf_series.to_arrow().schema() == [("label", "Utf8", False), ("x", "Float64", True)]

    i_series = LabeledSeries(["a", "b", "c"], [10, None, -3], name="days")
    assert i_series.to_arrow().schema() == [("label", "Utf8", False), ("days", "Int64", True)]
    assert i_series.to_pandas().dtype == pd.Int64Dtype()
    assert i_series["a"] == 10
    assert i_series["b"] is None

    i_non_null = LabeledSeries(["a", "b"], [1, 2], name="days")
    np_int = i_non_null.to_numpy()
    assert np_int.dtype == np.int64
    assert list(np_int) == [1, 2]

    np_obj = i_series.to_numpy()
    assert np_obj.dtype == object
    assert list(np_obj) == [10, None, -3]

    f_forced = LabeledSeries(["a", "b"], [1, 2], dtype="float64")
    assert f_forced.to_arrow().schema() == [("label", "Utf8", False), ("value", "Float64", False)]

    empty = LabeledSeries([], [], name="m")
    assert len(empty) == 0
    assert empty.to_list() == []
    assert empty.to_pandas().empty


def test_labeled_series_validation_errors() -> None:
    """``LabeledSeries`` rejects booleans, mismatched lengths, and invalid dtypes cleanly."""
    with pytest.raises(ValueError, match="equal length"):
        LabeledSeries(["a"], [1.0, 2.0])
    with pytest.raises(ValueError, match="unsupported dtype"):
        LabeledSeries(["a"], [1.0], dtype="decimal")
    with pytest.raises(TypeError, match="booleans"):
        LabeledSeries(["a"], [True])
    with pytest.raises(TypeError, match="booleans"):
        LabeledSeries(["a"], [np.bool_(False)])
    with pytest.raises(TypeError, match="int64"):
        LabeledSeries(["a"], [1.5], dtype="int64")
    with pytest.raises(TypeError):
        LabeledSeries(["a"], "not-numbers")


def test_labeled_series_mapping_protocol_and_exports() -> None:
    """``LabeledSeries`` implements dict-like indexing and host conversions."""
    s = LabeledSeries(["FUND", "BENCH"], [1.5, None], name="calmar", index_name="ticker")

    assert len(s) == 2
    assert "FUND" in s
    assert "BENCH" in s
    assert "MISSING" not in s
    assert 123 not in s
    assert s["FUND"] == 1.5
    assert s["BENCH"] is None
    with pytest.raises(KeyError):
        _ = s["MISSING"]
    assert s.get("FUND") == 1.5
    assert s.get("BENCH", -1.0) is None
    assert s.get("MISSING") is None
    assert s.get("MISSING", -1.0) == -1.0

    assert s.keys() == ["FUND", "BENCH"]
    assert s.values() == [1.5, None]
    assert s.to_list() == [1.5, None]
    assert s.items() == [("FUND", 1.5), ("BENCH", None)]
    assert list(s) == ["FUND", "BENCH"]
    assert s.to_dict() == {"FUND": 1.5, "BENCH": None}

    np_arr = s.to_numpy()
    assert np_arr[0] == 1.5
    assert math.isnan(np_arr[1])

    pd_s = s.to_pandas()
    assert isinstance(pd_s, pd.Series)
    assert pd_s.name == "calmar"
    assert list(pd_s.index) == ["FUND", "BENCH"]
    assert pd_s["FUND"] == 1.5
    assert math.isnan(pd_s["BENCH"])

    pl_df = s.to_polars()
    assert isinstance(pl_df, pl.DataFrame)
    assert pl_df.columns == ["ticker", "calmar"]
    assert pl_df.equals(pl.DataFrame(s))

    pa_tbl = s.to_pyarrow()
    assert pa_tbl.equals(pa.table(s))
    assert pa.schema(s).names == ["ticker", "calmar"]
    assert "calmar" in (s._repr_html_() or "")


def test_labeled_series_ipc_and_pickle_roundtrip() -> None:
    """``LabeledSeries`` round-trips through Arrow IPC bytes and pickle (including ``NaN`` and ``None``)."""
    for orig in (
        LabeledSeries(["A", "B"], [1.25, 2.5], name="sharpe", index_name="ticker"),
        LabeledSeries(["A", "B", "C"], [1.0, None, float("nan")], name="sortino", index_name="ticker"),
        LabeledSeries(["A", "B"], [42, None], name="duration", index_name="ticker"),
    ):
        rebuilt_ipc = LabeledSeries.from_ipc(orig.to_ipc())
        assert rebuilt_ipc == orig
        rebuilt_pickle = pickle.loads(pickle.dumps(orig))  # noqa: S301
        assert rebuilt_pickle == orig
