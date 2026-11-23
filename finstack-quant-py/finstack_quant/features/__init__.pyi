"""
Feature engineering: panel-data transformations for signal research.

Bindings for the ``finstack-quant-features`` crate
(``finstack_quant.features``). Provides time-series, cross-sectional, and
pairwise transforms (z-score, rank, rolling mean, neutralization, risk-scaled
weights) plus a general panel dispatcher :func:`transform_panel` (typed) /
:func:`transform_panel_json` (JSON), and the operation selectors
:class:`TimeSeriesOp`, :class:`CrossSectionalOp`, :class:`PairwiseOp`.

Key columns (``entity``, ``order``, ``time_key``, ``groups``) accept strings
and date-like objects. Datetimes are formatted by the Rust key policy
(``datetime_order_key``: aware values normalize to UTC, naive values keep
wall time, fixed nanosecond precision); dates use ``isoformat()``. Ints,
floats, bools, durations (``pd.Timedelta``), times and other objects raise
``TypeError``, matching the WASM
string-only key columns; convert integer period keys to zero-padded strings.

Examples
--------
>>> from finstack_quant.features import transform_cross_sectional
>>> transform_cross_sectional([1.0, 3.0], ["2026-01-01"] * 2, "rank")
[0.0, 1.0]
"""

from __future__ import annotations

import datetime
from collections.abc import Sequence
from types import ModuleType
from typing import Any

import numpy as np
import numpy.typing as npt
import pandas as pd
import polars as pl
from finstack_quant.core.table import ArrowTable

TransformParams = dict[str, Any]
KeyColumn = Sequence[str | datetime.date] | pd.Series | pl.Series | npt.NDArray[Any] | Any
ValueColumn = Sequence[float | None] | npt.NDArray[np.float64] | pd.Series | pl.Series | Any
ExposureColumns = Sequence[ValueColumn] | pd.DataFrame | pl.DataFrame | Any

from finstack_quant.features._ops import (
    TimeSeriesOp as TimeSeriesOp,
    CrossSectionalOp as CrossSectionalOp,
    PairwiseOp as PairwiseOp,
)
from finstack_quant.features._transforms import (
    transform_timeseries as transform_timeseries,
    transform_cross_sectional as transform_cross_sectional,
    transform_cross_sectional_grouped as transform_cross_sectional_grouped,
    neutralize as neutralize,
    transform_timeseries_pairwise as transform_timeseries_pairwise,
    rolling_regression_residual as rolling_regression_residual,
    risk_scaled_weights as risk_scaled_weights,
    rank_to_weights as rank_to_weights,
    neutralize_and_zscore as neutralize_and_zscore,
)
from finstack_quant.features._panel import (
    PanelTransformSpec as PanelTransformSpec,
    PanelTransformResult as PanelTransformResult,
    transform_panel as transform_panel,
    transform_panel_json as transform_panel_json,
)

__all__ = [
    "CrossSectionalOp",
    "PairwiseOp",
    "PanelTransformResult",
    "PanelTransformSpec",
    "TimeSeriesOp",
    "dataframe",
    "neutralize",
    "neutralize_and_zscore",
    "rank_to_weights",
    "risk_scaled_weights",
    "rolling_regression_residual",
    "transform_cross_sectional",
    "transform_cross_sectional_grouped",
    "transform_panel",
    "transform_panel_json",
    "transform_timeseries",
    "transform_timeseries_pairwise",
]

dataframe: ModuleType
