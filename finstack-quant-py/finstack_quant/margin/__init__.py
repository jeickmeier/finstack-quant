"""Margin and collateral: VM/IM calculators, CSA specifications, XVA, metrics.

Bindings for the ``finstack-quant-margin`` Rust crate.

Examples:
--------
>>> from finstack_quant.margin import CollateralAssetClass
>>> CollateralAssetClass.cash().standard_haircut()
0.0
"""

import sys as _sys

from finstack_quant.finstack_quant import margin as _margin

ImMethodology = _margin.ImMethodology
MarginTenor = _margin.MarginTenor
MarginCallType = _margin.MarginCallType
ClearingStatus = _margin.ClearingStatus
CollateralAssetClass = _margin.CollateralAssetClass
NettingSetId = _margin.NettingSetId
CsaSpec = _margin.CsaSpec
EligibleCollateralSchedule = _margin.EligibleCollateralSchedule
CONSTANTS = _margin.CONSTANTS
VmResult = _margin.VmResult
VmCalculator = _margin.VmCalculator
ImCollateralResult = _margin.ImCollateralResult
ImResult = _margin.ImResult
SimmSensitivities = _margin.SimmSensitivities
SimmCurvatureSensitivity = _margin.SimmCurvatureSensitivity
SimmCalculator = _margin.SimmCalculator
ScheduleImCalculator = _margin.ScheduleImCalculator
HaircutImCalculator = _margin.HaircutImCalculator
FundingConfig = _margin.FundingConfig
ExposureDiagnostics = _margin.ExposureDiagnostics
ExposureProfile = _margin.ExposureProfile
XvaResult = _margin.XvaResult
ImDecayProfile = _margin.ImDecayProfile
ImProfile = _margin.ImProfile
MvaResult = _margin.MvaResult
compute_mva = _margin.compute_mva
compute_bilateral_xva = _margin.compute_bilateral_xva
im_profile_from_simm = _margin.im_profile_from_simm
MarginUtilization = _margin.MarginUtilization
ExcessCollateral = _margin.ExcessCollateral
MarginFundingCost = _margin.MarginFundingCost
Haircut01 = _margin.Haircut01
FrtbSensitivities = _margin.FrtbSensitivities
FrtbSbaEngine = _margin.FrtbSbaEngine
FrtbSbaResult = _margin.FrtbSbaResult
EadResult = _margin.EadResult
SaCcrTrade = _margin.SaCcrTrade
SaCcrNettingSetConfig = _margin.SaCcrNettingSetConfig
SaCcrEngine = _margin.SaCcrEngine
frtb_sba_charge = _margin.frtb_sba_charge
saccr_ead = _margin.saccr_ead
schema = _margin.schema

# `schema` is a real submodule, so `import finstack_quant.margin.schema`
# must work as well as attribute access.
if "finstack_quant.margin.schema" not in _sys.modules:
    _sys.modules["finstack_quant.margin.schema"] = schema

__all__: list[str] = [
    "CONSTANTS",
    "ClearingStatus",
    "CollateralAssetClass",
    "CsaSpec",
    "EadResult",
    "EligibleCollateralSchedule",
    "ExcessCollateral",
    "ExposureDiagnostics",
    "ExposureProfile",
    "FrtbSbaEngine",
    "FrtbSbaResult",
    "FrtbSensitivities",
    "FundingConfig",
    "Haircut01",
    "HaircutImCalculator",
    "ImCollateralResult",
    "ImDecayProfile",
    "ImMethodology",
    "ImProfile",
    "ImResult",
    "MarginCallType",
    "MarginFundingCost",
    "MarginTenor",
    "MarginUtilization",
    "MvaResult",
    "NettingSetId",
    "SaCcrEngine",
    "SaCcrNettingSetConfig",
    "SaCcrTrade",
    "ScheduleImCalculator",
    "SimmCalculator",
    "SimmCurvatureSensitivity",
    "SimmSensitivities",
    "VmCalculator",
    "VmResult",
    "XvaResult",
    "compute_bilateral_xva",
    "compute_mva",
    "frtb_sba_charge",
    "im_profile_from_simm",
    "saccr_ead",
    "schema",
]
