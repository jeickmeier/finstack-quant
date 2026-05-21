"""Instrument pricing, risk metrics, and P&L attribution.

Bindings for the ``finstack-valuations`` Rust crate.
"""

import json as _json
from typing import TYPE_CHECKING as _TYPE_CHECKING, Any as _Any

from finstack.finstack import valuations as _valuations
from finstack.valuations import (
    correlation as correlation,
    credit as credit,
    credit_derivatives as credit_derivatives,
    exotics as exotics,
    fx as fx,
    instruments as instruments,
)
from finstack.valuations.envelope import (
    CalibrationEnvelope as CalibrationEnvelope,
    CalibrationPlan as CalibrationPlan,
    CalibrationStep as CalibrationStep,
    DiscountStep as DiscountStep,
    ForwardStep as ForwardStep,
    HazardStep as HazardStep,
    MarketDatum as MarketDatum,
    MarketQuote as MarketQuote,
    Pillar as Pillar,
    PriorMarketObject as PriorMarketObject,
    RateDeposit as RateDeposit,
    RateSwap as RateSwap,
    Tenor as Tenor,
    VolSurfaceStep as VolSurfaceStep,
)

if _TYPE_CHECKING:
    import pandas as pd

ValuationResult = _valuations.ValuationResult
validate_instrument_json = _valuations.validate_instrument_json
price_instrument = _valuations.price_instrument
price_instrument_with_metrics = _valuations.price_instrument_with_metrics
list_standard_metrics = _valuations.list_standard_metrics
list_standard_metrics_grouped = _valuations.list_standard_metrics_grouped
PnlAttribution = _valuations.PnlAttribution
attribute_pnl = _valuations.attribute_pnl
attribute_pnl_from_spec = _valuations.attribute_pnl_from_spec
validate_attribution_json = _valuations.validate_attribution_json
default_waterfall_order = _valuations.default_waterfall_order
default_attribution_metrics = _valuations.default_attribution_metrics
SensitivityMatrix = _valuations.SensitivityMatrix
FactorPnlProfile = _valuations.FactorPnlProfile
compute_factor_sensitivities = _valuations.compute_factor_sensitivities
compute_pnl_profiles = _valuations.compute_pnl_profiles
RiskDecomposition = _valuations.RiskDecomposition
decompose_factor_risk = _valuations.decompose_factor_risk
CalibrationResult = _valuations.CalibrationResult
CalibrationEnvelopeError = _valuations.CalibrationEnvelopeError
validate_calibration_json = _valuations.validate_calibration_json
calibrate = _valuations.calibrate
dry_run = _valuations.dry_run
dependency_graph_json = _valuations.dependency_graph_json
tarn_coupon_profile = _valuations.tarn_coupon_profile
snowball_coupon_profile = _valuations.snowball_coupon_profile
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
instrument_cashflows_json = _valuations.instrument_cashflows_json
CreditFactorModel = _valuations.CreditFactorModel
CreditCalibrator = _valuations.CreditCalibrator
LevelsAtDate = _valuations.LevelsAtDate
PeriodDecomposition = _valuations.PeriodDecomposition
FactorCovarianceForecast = _valuations.FactorCovarianceForecast
decompose_levels = _valuations.decompose_levels
decompose_period = _valuations.decompose_period


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

    payload = instrument_cashflows_json(instrument_json, market, as_of, model)
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
    "CalibrationPlan",
    "CalibrationResult",
    "CalibrationStep",
    "CreditCalibrator",
    "CreditFactorModel",
    "DiscountStep",
    "FactorCovarianceForecast",
    "FactorPnlProfile",
    "ForwardStep",
    "HazardStep",
    "LevelsAtDate",
    "MarketDatum",
    "MarketQuote",
    "PeriodDecomposition",
    "Pillar",
    "PnlAttribution",
    "PriorMarketObject",
    "RateDeposit",
    "RateSwap",
    "RiskDecomposition",
    "SabrCalibrator",
    "SabrModel",
    "SabrParameters",
    "SabrSmile",
    "SensitivityMatrix",
    "Tenor",
    "ValuationResult",
    "VolSurfaceStep",
    "asian_option_price",
    "attribute_pnl",
    "attribute_pnl_from_spec",
    "barrier_call",
    "black76_implied_vol",
    "bs_cos_price",
    "bs_greeks",
    "bs_implied_vol",
    "bs_price",
    "calibrate",
    "callable_range_accrual_accrued",
    "cms_spread_option_intrinsic",
    "compute_factor_sensitivities",
    "compute_pnl_profiles",
    "correlation",
    "credit",
    "credit_derivatives",
    "decompose_factor_risk",
    "decompose_levels",
    "decompose_period",
    "default_attribution_metrics",
    "default_waterfall_order",
    "dependency_graph_json",
    "dry_run",
    "exotics",
    "fx",
    "instrument_cashflows",
    "instrument_cashflows_json",
    "instruments",
    "list_standard_metrics",
    "list_standard_metrics_grouped",
    "lookback_option_price",
    "merton_jump_cos_price",
    "price_instrument",
    "price_instrument_with_metrics",
    "quanto_option_price",
    "snowball_coupon_profile",
    "tarn_coupon_profile",
    "validate_attribution_json",
    "validate_calibration_json",
    "validate_instrument_json",
    "vg_cos_price",
]
