"""Dynamic term-structure models: Diebold-Li and yield-curve PCA.

Examples:
--------
>>> from finstack_quant.models.rates.dtsm import nelson_siegel_yields
>>> len(nelson_siegel_yields(0.7308, (0.03, -0.01, 0.005), [1.0, 5.0, 10.0]))
3
"""

from finstack_quant.finstack_quant import models as _models

_dtsm = _models.rates.dtsm

DieboldLi = _dtsm.DieboldLi
FactorTimeSeries = _dtsm.FactorTimeSeries
YieldForecast = _dtsm.YieldForecast
YieldPanel = _dtsm.YieldPanel
YieldPca = _dtsm.YieldPca
YieldPcaView = _dtsm.YieldPcaView
diebold_li_fit_factors = _dtsm.diebold_li_fit_factors
diebold_li_forecast = _dtsm.diebold_li_forecast
nelson_siegel_yields = _dtsm.nelson_siegel_yields

__all__: list[str] = [
    "DieboldLi",
    "FactorTimeSeries",
    "YieldForecast",
    "YieldPanel",
    "YieldPca",
    "YieldPcaView",
    "diebold_li_fit_factors",
    "diebold_li_forecast",
    "nelson_siegel_yields",
]
