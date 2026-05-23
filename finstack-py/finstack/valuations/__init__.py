"""Instrument pricing and risk metrics.

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
    BaseCorrelationCurvePrior as BaseCorrelationCurvePrior,
    BaseCorrelationStep as BaseCorrelationStep,
    BasisSpreadCurvePrior as BasisSpreadCurvePrior,
    BondCleanPriceDatum as BondCleanPriceDatum,
    BondFixedRateBulletCleanPrice as BondFixedRateBulletCleanPrice,
    BondFixedRateBulletOas as BondFixedRateBulletOas,
    BondFixedRateBulletYtm as BondFixedRateBulletYtm,
    BondFixedRateBulletZSpread as BondFixedRateBulletZSpread,
    BondOasDatum as BondOasDatum,
    BondYtmDatum as BondYtmDatum,
    BondZSpreadDatum as BondZSpreadDatum,
    CalibrationEnvelope as CalibrationEnvelope,
    CalibrationPlan as CalibrationPlan,
    CalibrationStep as CalibrationStep,
    CapFloorHullWhiteStep as CapFloorHullWhiteStep,
    CapFloorVolDatum as CapFloorVolDatum,
    CapFloorVolPayload as CapFloorVolPayload,
    CapFloorVolQuote as CapFloorVolQuote,
    CdsConventionKey as CdsConventionKey,
    CdsParSpread as CdsParSpread,
    CdsParSpreadDatum as CdsParSpreadDatum,
    CdsTrancheDatum as CdsTrancheDatum,
    CdsTrancheQuote as CdsTrancheQuote,
    CdsUpfront as CdsUpfront,
    CdsUpfrontDatum as CdsUpfrontDatum,
    CollateralDatum as CollateralDatum,
    CreditIndexDatum as CreditIndexDatum,
    DatePillar as DatePillar,
    DiscountCurvePrior as DiscountCurvePrior,
    DiscountStep as DiscountStep,
    DividendScheduleDatum as DividendScheduleDatum,
    FixingSeriesDatum as FixingSeriesDatum,
    ForwardCurvePrior as ForwardCurvePrior,
    ForwardStep as ForwardStep,
    FxForwardOutright as FxForwardOutright,
    FxForwardOutrightDatum as FxForwardOutrightDatum,
    FxOptionVanilla as FxOptionVanilla,
    FxOptionVanillaDatum as FxOptionVanillaDatum,
    FxSpotDatum as FxSpotDatum,
    FxSwapOutright as FxSwapOutright,
    FxSwapOutrightDatum as FxSwapOutrightDatum,
    FxVolSurfaceDatum as FxVolSurfaceDatum,
    HazardCurvePrior as HazardCurvePrior,
    HazardStep as HazardStep,
    HullWhiteStep as HullWhiteStep,
    InflationCurvePrior as InflationCurvePrior,
    InflationFixingsDatum as InflationFixingsDatum,
    InflationStep as InflationStep,
    InflationSwapDatum as InflationSwapDatum,
    InflationSwapPayload as InflationSwapPayload,
    InflationSwapQuote as InflationSwapQuote,
    MarketDatum as MarketDatum,
    MarketQuote as MarketQuote,
    OptionVolDatum as OptionVolDatum,
    OptionVolPayload as OptionVolPayload,
    OptionVolQuote as OptionVolQuote,
    ParametricCurvePrior as ParametricCurvePrior,
    ParametricStep as ParametricStep,
    Pillar as Pillar,
    PriceCurvePrior as PriceCurvePrior,
    PriceDatum as PriceDatum,
    PriorMarketObject as PriorMarketObject,
    RateDeposit as RateDeposit,
    RateFra as RateFra,
    RateFutures as RateFutures,
    RateQuoteDepositDatum as RateQuoteDepositDatum,
    RateQuoteFraDatum as RateQuoteFraDatum,
    RateQuoteFuturesDatum as RateQuoteFuturesDatum,
    RateQuoteSwapDatum as RateQuoteSwapDatum,
    RateSwap as RateSwap,
    StudentTStep as StudentTStep,
    SviSurfaceStep as SviSurfaceStep,
    SwaptionVolDatum as SwaptionVolDatum,
    SwaptionVolPayload as SwaptionVolPayload,
    SwaptionVolQuote as SwaptionVolQuote,
    SwaptionVolStep as SwaptionVolStep,
    Tenor as Tenor,
    TenorPillar as TenorPillar,
    VolatilityIndexCurvePrior as VolatilityIndexCurvePrior,
    VolCubeDatum as VolCubeDatum,
    VolSurfacePrior as VolSurfacePrior,
    VolSurfaceStep as VolSurfaceStep,
    XccyBasisStep as XccyBasisStep,
    XccyBasisSwapDatum as XccyBasisSwapDatum,
    XccyBasisSwapQuote as XccyBasisSwapQuote,
    YoyInflationSwapDatum as YoyInflationSwapDatum,
    YoyInflationSwapPayload as YoyInflationSwapPayload,
    YoyInflationSwapQuote as YoyInflationSwapQuote,
)

if _TYPE_CHECKING:
    import pandas as pd

ValuationResult = _valuations.ValuationResult
validate_instrument_json = _valuations.validate_instrument_json
price_instrument = _valuations.price_instrument
price_instrument_with_metrics = _valuations.price_instrument_with_metrics
list_standard_metrics = _valuations.list_standard_metrics
list_standard_metrics_grouped = _valuations.list_standard_metrics_grouped
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

# Canonical structural-credit exports live at `finstack.valuations.*`; the
# `finstack.valuations.credit` module remains only as an import-compatibility
# namespace.
MertonModel = _valuations.credit.MertonModel
DynamicRecoverySpec = _valuations.credit.DynamicRecoverySpec
EndogenousHazardSpec = _valuations.credit.EndogenousHazardSpec
CreditState = _valuations.credit.CreditState
ToggleExerciseModel = _valuations.credit.ToggleExerciseModel


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
    "BaseCorrelationCurvePrior",
    "BaseCorrelationStep",
    "BasisSpreadCurvePrior",
    "BondCleanPriceDatum",
    "BondFixedRateBulletCleanPrice",
    "BondFixedRateBulletOas",
    "BondFixedRateBulletYtm",
    "BondFixedRateBulletZSpread",
    "BondOasDatum",
    "BondYtmDatum",
    "BondZSpreadDatum",
    "CalibrationEnvelope",
    "CalibrationEnvelopeError",
    "CalibrationPlan",
    "CalibrationResult",
    "CalibrationStep",
    "CapFloorHullWhiteStep",
    "CapFloorVolDatum",
    "CapFloorVolPayload",
    "CapFloorVolQuote",
    "CdsConventionKey",
    "CdsParSpread",
    "CdsParSpreadDatum",
    "CdsTrancheDatum",
    "CdsTrancheQuote",
    "CdsUpfront",
    "CdsUpfrontDatum",
    "CollateralDatum",
    "CreditCalibrator",
    "CreditFactorModel",
    "CreditIndexDatum",
    "CreditState",
    "DatePillar",
    "DiscountCurvePrior",
    "DiscountStep",
    "DividendScheduleDatum",
    "DynamicRecoverySpec",
    "EndogenousHazardSpec",
    "FactorCovarianceForecast",
    "FixingSeriesDatum",
    "ForwardCurvePrior",
    "ForwardStep",
    "FxForwardOutright",
    "FxForwardOutrightDatum",
    "FxOptionVanilla",
    "FxOptionVanillaDatum",
    "FxSpotDatum",
    "FxSwapOutright",
    "FxSwapOutrightDatum",
    "FxVolSurfaceDatum",
    "HazardCurvePrior",
    "HazardStep",
    "HullWhiteStep",
    "InflationCurvePrior",
    "InflationFixingsDatum",
    "InflationStep",
    "InflationSwapDatum",
    "InflationSwapPayload",
    "InflationSwapQuote",
    "LevelsAtDate",
    "MarketDatum",
    "MarketQuote",
    "MertonModel",
    "OptionVolDatum",
    "OptionVolPayload",
    "OptionVolQuote",
    "ParametricCurvePrior",
    "ParametricStep",
    "PeriodDecomposition",
    "Pillar",
    "PriceCurvePrior",
    "PriceDatum",
    "PriorMarketObject",
    "RateDeposit",
    "RateFra",
    "RateFutures",
    "RateQuoteDepositDatum",
    "RateQuoteFraDatum",
    "RateQuoteFuturesDatum",
    "RateQuoteSwapDatum",
    "RateSwap",
    "SabrCalibrator",
    "SabrModel",
    "SabrParameters",
    "SabrSmile",
    "StudentTStep",
    "SviSurfaceStep",
    "SwaptionVolDatum",
    "SwaptionVolPayload",
    "SwaptionVolQuote",
    "SwaptionVolStep",
    "Tenor",
    "TenorPillar",
    "ToggleExerciseModel",
    "ValuationResult",
    "VolCubeDatum",
    "VolSurfacePrior",
    "VolSurfaceStep",
    "VolatilityIndexCurvePrior",
    "XccyBasisStep",
    "XccyBasisSwapDatum",
    "XccyBasisSwapQuote",
    "YoyInflationSwapDatum",
    "YoyInflationSwapPayload",
    "YoyInflationSwapQuote",
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
    "credit",
    "credit_derivatives",
    "decompose_levels",
    "decompose_period",
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
    "validate_calibration_json",
    "validate_instrument_json",
    "vg_cos_price",
]
