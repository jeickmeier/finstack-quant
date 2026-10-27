"""
Margin and collateral: VM/IM calculators, CSA specifications, XVA, metrics.

This module exposes variation and initial margin types, netting-set identifiers,
credit support annex (CSA) specs, eligible collateral schedules, XVA configuration
and results, and margin analytics helpers.

Examples
--------
>>> from finstack_quant.margin import CollateralAssetClass
>>> CollateralAssetClass.cash().standard_haircut()
0.0
"""

from __future__ import annotations

from typing import Any, Final
from finstack_quant.margin import schema as schema
from finstack_quant.margin._types import (
    ClearingStatus as ClearingStatus,
    CollateralAssetClass as CollateralAssetClass,
    CsaSpec as CsaSpec,
    EligibleCollateralSchedule as EligibleCollateralSchedule,
    ImCollateralResult as ImCollateralResult,
    ImMethodology as ImMethodology,
    MarginCallType as MarginCallType,
    MarginTenor as MarginTenor,
    NettingSetId as NettingSetId,
)
from finstack_quant.margin._calculators import ImResult as ImResult, VmCalculator as VmCalculator, VmResult as VmResult
from finstack_quant.margin._im_curvature import SimmCurvatureSensitivity as SimmCurvatureSensitivity
from finstack_quant.margin._im import (
    HaircutImCalculator as HaircutImCalculator,
    ScheduleImCalculator as ScheduleImCalculator,
    SimmCalculator as SimmCalculator,
    SimmSensitivities as SimmSensitivities,
)
from finstack_quant.margin._xva import (
    ExposureDiagnostics as ExposureDiagnostics,
    ExposureProfile as ExposureProfile,
    FundingConfig as FundingConfig,
    ImDecayProfile as ImDecayProfile,
    ImProfile as ImProfile,
    MvaResult as MvaResult,
    XvaResult as XvaResult,
    compute_bilateral_xva as compute_bilateral_xva,
    compute_mva as compute_mva,
    im_profile_from_simm as im_profile_from_simm,
)
from finstack_quant.margin._metrics import (
    ExcessCollateral as ExcessCollateral,
    Haircut01 as Haircut01,
    MarginFundingCost as MarginFundingCost,
    MarginUtilization as MarginUtilization,
)
from finstack_quant.margin._regulatory import (
    EadResult as EadResult,
    FrtbSbaEngine as FrtbSbaEngine,
    FrtbSbaResult as FrtbSbaResult,
    FrtbSensitivities as FrtbSensitivities,
    SaCcrEngine as SaCcrEngine,
    SaCcrNettingSetConfig as SaCcrNettingSetConfig,
    SaCcrTrade as SaCcrTrade,
    frtb_sba_charge as frtb_sba_charge,
    saccr_ead as saccr_ead,
)

__all__ = [
    "ImMethodology",
    "MarginTenor",
    "MarginCallType",
    "ClearingStatus",
    "CollateralAssetClass",
    "NettingSetId",
    "CsaSpec",
    "EligibleCollateralSchedule",
    "CONSTANTS",
    "VmResult",
    "VmCalculator",
    "ImResult",
    "ImCollateralResult",
    "SimmCurvatureSensitivity",
    "SimmSensitivities",
    "SimmCalculator",
    "ScheduleImCalculator",
    "HaircutImCalculator",
    "FundingConfig",
    "ExposureDiagnostics",
    "ExposureProfile",
    "XvaResult",
    "ImDecayProfile",
    "ImProfile",
    "MvaResult",
    "compute_bilateral_xva",
    "compute_mva",
    "im_profile_from_simm",
    "MarginUtilization",
    "ExcessCollateral",
    "MarginFundingCost",
    "Haircut01",
    "FrtbSbaResult",
    "EadResult",
    "FrtbSensitivities",
    "FrtbSbaEngine",
    "SaCcrTrade",
    "SaCcrNettingSetConfig",
    "SaCcrEngine",
    "frtb_sba_charge",
    "saccr_ead",
    "schema",
]

# Rust ``finstack_quant_margin::constants`` plus the registry/calculator
# constants needed to interpret results. Keys: ``CALENDAR_DAYS_PER_YEAR``,
# ``DURATION_APPROXIMATION_FACTOR``, ``ONE_BP``, ``STANDARD_CDS_MATURITY_YEARS``,
# ``tenor_buckets`` (dict of
# ``BUCKET_3M`` .. ``BUCKET_20Y`` -> years), ``BCBS_IOSCO_SCHEDULE_ID`` (str),
# ``HAIRCUT_MPOR_DAYS`` (business days), ``SIMM_TENORS`` (the valid SIMM tenor
# labels, ``"2W"`` .. ``"30Y"``) and ``SIMM_COMMODITY_BUCKET_COUNT`` (17).
CONSTANTS: Final[dict[str, Any]] = ...
