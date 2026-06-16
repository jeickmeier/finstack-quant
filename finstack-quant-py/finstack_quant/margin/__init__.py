"""Margin and collateral: VM/IM calculators, CSA specifications, XVA, metrics.

Bindings for the ``finstack-quant-margin`` Rust crate.
"""

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
ImResult = _margin.ImResult
FundingConfig = _margin.FundingConfig
XvaConfig = _margin.XvaConfig
ExposureDiagnostics = _margin.ExposureDiagnostics
ExposureProfile = _margin.ExposureProfile
XvaResult = _margin.XvaResult
CsaTerms = _margin.CsaTerms
XvaNettingSet = _margin.XvaNettingSet
MarginUtilization = _margin.MarginUtilization
ExcessCollateral = _margin.ExcessCollateral
MarginFundingCost = _margin.MarginFundingCost
Haircut01 = _margin.Haircut01
FrtbSensitivities = _margin.FrtbSensitivities
FrtbSbaEngine = _margin.FrtbSbaEngine
SaCcrTrade = _margin.SaCcrTrade
SaCcrNettingSetConfig = _margin.SaCcrNettingSetConfig
SaCcrEngine = _margin.SaCcrEngine
frtb_sba_charge = _margin.frtb_sba_charge
saccr_ead = _margin.saccr_ead

__all__: list[str] = [
    "CONSTANTS",
    "ClearingStatus",
    "CollateralAssetClass",
    "CsaSpec",
    "CsaTerms",
    "EligibleCollateralSchedule",
    "ExcessCollateral",
    "ExposureDiagnostics",
    "ExposureProfile",
    "FrtbSbaEngine",
    "FrtbSensitivities",
    "FundingConfig",
    "Haircut01",
    "ImMethodology",
    "ImResult",
    "MarginCallType",
    "MarginFundingCost",
    "MarginTenor",
    "MarginUtilization",
    "NettingSetId",
    "SaCcrEngine",
    "SaCcrNettingSetConfig",
    "SaCcrTrade",
    "VmCalculator",
    "VmResult",
    "XvaConfig",
    "XvaNettingSet",
    "XvaResult",
    "frtb_sba_charge",
    "saccr_ead",
]
