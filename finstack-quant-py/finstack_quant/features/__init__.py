"""Vectorized panel feature transforms from ``finstack-quant-features``.

Re-exports the compiled bindings and the optional
:mod:`finstack_quant.features.dataframe` pandas helpers. Symbol documentation
lives in the ``.pyi`` stubs and in ``dataframe.py``.

Examples:
--------
>>> from finstack_quant.features import transform_cross_sectional
>>> transform_cross_sectional([1.0, 3.0], ["2026-01-01"] * 2, "rank")
[0.0, 1.0]
"""

from importlib import import_module as _import_module

from finstack_quant.finstack_quant import features as _features

CrossSectionalOp = _features.CrossSectionalOp
PairwiseOp = _features.PairwiseOp
PanelTransformResult = _features.PanelTransformResult
PanelTransformSpec = _features.PanelTransformSpec
TimeSeriesOp = _features.TimeSeriesOp
neutralize = _features.neutralize
neutralize_and_zscore = _features.neutralize_and_zscore
rank_to_weights = _features.rank_to_weights
risk_scaled_weights = _features.risk_scaled_weights
rolling_regression_residual = _features.rolling_regression_residual
transform_cross_sectional = _features.transform_cross_sectional
transform_cross_sectional_grouped = _features.transform_cross_sectional_grouped
transform_panel = _features.transform_panel
transform_panel_json = _features.transform_panel_json
transform_timeseries = _features.transform_timeseries
transform_timeseries_pairwise = _features.transform_timeseries_pairwise

dataframe = _import_module(__name__ + ".dataframe")

__all__: list[str] = [
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
