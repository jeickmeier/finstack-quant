"""
Market data bindings from ``finstack-quant-core``: curves, FX, and market context.

Provides term-structure curve types (discount, forward, hazard, price incl.
volatility-index, inflation), volatility surfaces and cubes, the FX rate matrix,
scalar series and the unified :class:`MarketContext` container.

Every curve query (``df``, ``zero_rate``, ``rate``, ``sp``,
``hazard_rate``, ``price``, ``cpi``) accepts either a year fraction (``float``)
or a date (``datetime.date`` or ISO ``"YYYY-MM-DD"`` string). Rust converts
dates with the curve's own time axis. ``ForwardCurve`` takes one options
dictionary.

Examples
--------
>>> import datetime
>>> from finstack_quant.core.market_data import DiscountCurve
>>> round(DiscountCurve.flat("USD-OIS", datetime.date(2025, 1, 1), 0.05, "act_365f").df(1.0), 6)
0.951229

"""

from __future__ import annotations

import datetime
from typing import Union
from finstack_quant.core.market_data._curves.discount import DiscountCurve as DiscountCurve
from finstack_quant.core.market_data._curves.forward import ForwardCurve as ForwardCurve
from finstack_quant.core.market_data._curves.hazard import HazardCurve as HazardCurve
from finstack_quant.core.market_data._curves.credit import (
    BaseCorrelationCurve as BaseCorrelationCurve,
    CreditIndexData as CreditIndexData,
)
from finstack_quant.core.market_data._curves.price import PriceCurve as PriceCurve
from finstack_quant.core.market_data._curves.inflation import InflationCurve as InflationCurve
from finstack_quant.core.market_data._curves.fx_vol_surface import FxDeltaVolSurface as FxDeltaVolSurface
from finstack_quant.core.market_data._curves.vol_cube import (
    SabrParameterData as SabrParameterData,
    VolCube as VolCube,
)
from finstack_quant.core.market_data._curves.vol_surface import VolSurface as VolSurface
from finstack_quant.core.market_data._curves.cube_expiry_slice import VolCubeExpirySlice as VolCubeExpirySlice
from finstack_quant.core.market_data._fx import (
    FxConversionPolicy as FxConversionPolicy,
    FxMatrix as FxMatrix,
    FxPairConvention as FxPairConvention,
    FxRateResult as FxRateResult,
    invert_fx_rate as invert_fx_rate,
)
from finstack_quant.core.market_data._context import ConvertedMoney as ConvertedMoney, MarketContext as MarketContext
from finstack_quant.core.market_data._scalars import (
    InflationExpectedRelease as InflationExpectedRelease,
    InflationIndex as InflationIndex,
    InflationObservation as InflationObservation,
    ScalarTimeSeries as ScalarTimeSeries,
)

__all__ = [
    "BaseCorrelationCurve",
    "ConvertedMoney",
    "CreditIndexData",
    "DiscountCurve",
    "ForwardCurve",
    "FxConversionPolicy",
    "FxDeltaVolSurface",
    "FxMatrix",
    "FxPairConvention",
    "FxRateResult",
    "HazardCurve",
    "InflationCurve",
    "InflationExpectedRelease",
    "InflationIndex",
    "InflationObservation",
    "MarketContext",
    "PriceCurve",
    "SabrParameterData",
    "ScalarTimeSeries",
    "VolCube",
    "VolCubeExpirySlice",
    "VolSurface",
    "invert_fx_rate",
]

DateLike = Union[datetime.date, str]
"""A ``datetime.date`` (or ``datetime``/``pandas.Timestamp``) or an ISO ``"YYYY-MM-DD"`` string."""

TimeOrDate = Union[float, datetime.date, str]
"""A year fraction from the curve base date, or a :data:`DateLike` converted with the curve day count."""
