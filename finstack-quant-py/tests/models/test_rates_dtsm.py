"""Typed dynamic term-structure bindings (Diebold-Li, PCA)."""

import datetime
import pickle

import pandas as pd
import pytest

from finstack_quant.models.rates.dtsm import (
    DieboldLi,
    FactorTimeSeries,
    YieldForecast,
    YieldPanel,
    YieldPca,
    YieldPcaView,
    diebold_li_fit_factors,
    diebold_li_forecast,
    yield_pca_fit,
)

TENORS = [1.0, 2.0, 5.0, 10.0]
YIELDS = [
    [0.020, 0.025, 0.030, 0.035],
    [0.021, 0.024, 0.031, 0.034],
    [0.019, 0.026, 0.029, 0.036],
    [0.022, 0.025, 0.032, 0.033],
    [0.020, 0.027, 0.030, 0.037],
    [0.023, 0.026, 0.033, 0.035],
]
DATES = [datetime.date(2025, 1, d) for d in range(1, 7)]


def test_yield_panel_dates_and_dataframe_round_trip() -> None:
    panel = YieldPanel(
        TENORS, YIELDS, ["2025-01-01", "2025-01-02", "2025-01-03", "2025-01-04", "2025-01-05", "2025-01-06"]
    )
    assert panel.dates == DATES
    frame = panel.to_dataframe()
    assert isinstance(frame.index, pd.DatetimeIndex)
    assert frame.columns.tolist() == TENORS
    rebuilt = YieldPanel.from_dataframe(frame)
    assert rebuilt.dates == DATES
    assert rebuilt.yields == panel.yields
    assert YieldPanel.from_json(panel.to_json()).num_dates == 6
    assert pickle.loads(pickle.dumps(panel)).tenors == TENORS  # noqa: S301
    with pytest.raises(ValueError, match=r"strictly ascending"):
        YieldPanel([2.0, 1.0], YIELDS)


def test_diebold_li_typed_pipeline() -> None:
    panel = YieldPanel(TENORS, YIELDS, DATES)
    model = DieboldLi().fit(panel)
    assert model.lambda_ == pytest.approx(0.7308)
    assert model.phi is not None
    assert len(model.phi) == 3
    factors = model.factors
    assert isinstance(factors, FactorTimeSeries)
    frame = factors.to_dataframe()
    assert frame.columns.tolist() == ["level", "slope", "curvature"]
    assert isinstance(frame.index, pd.DatetimeIndex)
    assert factors.dates == DATES
    forecast = model.forecast(2)
    assert isinstance(forecast, YieldForecast)
    assert forecast.to_dataframe().columns.tolist() == ["tenor", "yield", "lower_95", "upper_95"]
    assert DieboldLi.from_json(model.to_json()).forecast(2).yields == forecast.yields
    with pytest.raises(ValueError, match="fit_var"):
        DieboldLi().extract_factors(panel).forecast(1)
    with pytest.raises(ValueError, match=r"Lambda must be positive"):
        DieboldLi(-1.0)


def test_thin_twins_match_typed_api() -> None:
    panel = YieldPanel(TENORS, YIELDS)
    typed = DieboldLi().fit(panel)
    assert diebold_li_fit_factors(TENORS, YIELDS).level == typed.factors.level
    assert diebold_li_forecast(TENORS, YIELDS, 3).yields == typed.forecast(3).yields
    view = yield_pca_fit(panel.yield_changes(), 2)
    assert isinstance(view, YieldPcaView)
    assert view.tenors == [1.0, 2.0, 3.0, 4.0]
    assert view.num_components == 2
    assert YieldPcaView.from_json(view.to_json()) == view


def test_yield_pca_typed_api() -> None:
    panel = YieldPanel(TENORS, YIELDS)
    pca = YieldPca.fit(panel)
    assert pca.tenors == TENORS
    assert pca.to_dataframe().columns.tolist()[:2] == ["PC1", "PC2"]
    assert len(pca.scenario([2.0])) == 4
    assert pca.apply_scenario(YIELDS[-1], [0.0]) == pytest.approx(YIELDS[-1])
    assert pca.truncated(1).explained_variance_ratio == pca.variance_explained[:1]
    with pytest.raises(ValueError, match=r"n_components must be in"):
        pca.truncated(0)


def test_reextracting_factors_clears_stale_var_state() -> None:
    original = DieboldLi().fit(YieldPanel(TENORS, YIELDS))
    replacement_yields = [[value + 0.01 for value in row] for row in YIELDS]
    replacement = original.extract_factors(YieldPanel(TENORS, replacement_yields))
    assert original.phi is not None
    assert replacement.phi is None
    assert replacement.mu is None
    assert replacement.q_cov is None
    with pytest.raises(ValueError, match="fit_var"):
        replacement.forecast(1)
