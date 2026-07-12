"""Instrument pricing and risk metrics.

Bindings for the ``finstack-quant-valuations`` Rust crate.
"""

import json as _json
from typing import TYPE_CHECKING as _TYPE_CHECKING, Any as _Any

from finstack_quant.finstack_quant import valuations as _valuations
from finstack_quant.valuations import (
    correlation as correlation,
    instruments as instruments,
    models as models,
)
from finstack_quant.valuations.envelope import CalibrationEnvelope as CalibrationEnvelope

if _TYPE_CHECKING:
    import pandas as pd

ValuationResult = _valuations.ValuationResult
CalibrationResult = _valuations.CalibrationResult
CalibrationEnvelopeError = _valuations.CalibrationEnvelopeError
validate_calibration_json = _valuations.validate_calibration_json
calibrate = _valuations.calibrate
dry_run = _valuations.dry_run
dependency_graph_json = _valuations.dependency_graph_json
tarn_coupon_profile = _valuations.tarn_coupon_profile
snowball_coupon_profile = _valuations.snowball_coupon_profile
inverse_floater_coupon_profile = _valuations.inverse_floater_coupon_profile
cms_spread_option_intrinsic = _valuations.cms_spread_option_intrinsic
callable_range_accrual_accrued = _valuations.callable_range_accrual_accrued
bs_cos_price = _valuations.bs_cos_price
vg_cos_price = _valuations.vg_cos_price
merton_jump_cos_price = _valuations.merton_jump_cos_price
bs_price = _valuations.bs_price
bs_greeks = _valuations.bs_greeks
bs_implied_vol = _valuations.bs_implied_vol
black76_implied_vol = _valuations.black76_implied_vol
barrier_call = _valuations.barrier_call
asian_option_price = _valuations.asian_option_price
lookback_option_price = _valuations.lookback_option_price
quanto_option_price = _valuations.quanto_option_price
SabrParameters = _valuations.SabrParameters
SabrModel = _valuations.SabrModel
SabrSmile = _valuations.SabrSmile
SabrCalibrator = _valuations.SabrCalibrator


def instrument_cashflows(
    instrument_json: str,
    market: _Any,
    as_of: str,
    *,
    model: str = "discounting",
) -> tuple[dict, "pd.DataFrame"]:
    """Per-flow DF / survival / PV DataFrame for a discountable instrument.

    Supports ``model in {"discounting", "hazard_rate"}``. The returned
    ``envelope["total_pv"]`` reconciles with the instrument's ``base_value``
    for the supported model-instrument pairs.

    Args:
        instrument_json: Tagged instrument JSON.
        market: ``MarketContext`` instance or JSON string.
        as_of: ISO 8601 valuation date.
        model: ``"discounting"`` (DF only) or ``"hazard_rate"`` (adds survival
            probability, conditional default probability, and recovery-adjusted
            principal PV).

    Returns:
        ``(envelope, df)`` where ``envelope`` is the parsed JSON dict and
        ``df`` is a ``pandas.DataFrame`` of the per-flow rows with ``date``
        / ``reset_date`` parsed as ``datetime64``.

    Raises:
        ValueError: If ``model`` is unsupported or the instrument type isn't
            priced under that model.
    """
    import pandas as pd

    payload = instruments.instrument_cashflows_json(instrument_json, market, as_of, model)
    envelope = _json.loads(payload)
    df = pd.DataFrame(envelope["flows"])
    if not df.empty:
        df["date"] = pd.to_datetime(df["date"])
        if "reset_date" in df.columns:
            df["reset_date"] = pd.to_datetime(df["reset_date"])
    return envelope, df


__all__: list[str] = [
    "CalibrationEnvelope",
    "CalibrationEnvelopeError",
    "CalibrationResult",
    "SabrCalibrator",
    "SabrModel",
    "SabrParameters",
    "SabrSmile",
    "ValuationResult",
    "asian_option_price",
    "barrier_call",
    "black76_implied_vol",
    "bs_cos_price",
    "bs_greeks",
    "bs_implied_vol",
    "bs_price",
    "calibrate",
    "callable_range_accrual_accrued",
    "cms_spread_option_intrinsic",
    "correlation",
    "dependency_graph_json",
    "dry_run",
    "instrument_cashflows",
    "instruments",
    "inverse_floater_coupon_profile",
    "lookback_option_price",
    "merton_jump_cos_price",
    "models",
    "quanto_option_price",
    "snowball_coupon_profile",
    "tarn_coupon_profile",
    "validate_calibration_json",
    "vg_cos_price",
]
