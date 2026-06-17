"""Vectorized panel feature transforms."""

from importlib import import_module as _import_module

from finstack_quant.finstack_quant import features as _features

clean_signal = _features.clean_signal
neutralize = _features.neutralize
neutralize_and_zscore = _features.neutralize_and_zscore
normalize_signal = _features.normalize_signal
rank_to_weights = _features.rank_to_weights
risk_scaled_weights = _features.risk_scaled_weights
rolling_regression_residual = _features.rolling_regression_residual
transform_cross_sectional = _features.transform_cross_sectional
transform_cross_sectional_grouped = _features.transform_cross_sectional_grouped
transform_panel = _features.transform_panel
transform_timeseries = _features.transform_timeseries
transform_timeseries_pairwise = _features.transform_timeseries_pairwise

dataframe = _import_module(__name__ + ".dataframe")

__all__: list[str] = [
    "clean_signal",
    "dataframe",
    "neutralize",
    "neutralize_and_zscore",
    "normalize_signal",
    "rank_to_weights",
    "risk_scaled_weights",
    "rolling_regression_residual",
    "transform_cross_sectional",
    "transform_cross_sectional_grouped",
    "transform_panel",
    "transform_timeseries",
    "transform_timeseries_pairwise",
]
