"""Direct fixed-income valuation instrument wrappers.

Mirrors ``finstack_quant_valuations::instruments::fixed_income``.
"""

from __future__ import annotations

from finstack_quant.finstack_quant import valuations as _valuations

Bond = _valuations.instruments.fixed_income.Bond
ConvertibleBond = _valuations.instruments.fixed_income.ConvertibleBond
InflationLinkedBond = _valuations.instruments.fixed_income.InflationLinkedBond
TermLoan = _valuations.instruments.fixed_income.TermLoan
RevolvingCredit = _valuations.instruments.fixed_income.RevolvingCredit
BondFuture = _valuations.instruments.fixed_income.BondFuture
AgencyMbsPassthrough = _valuations.instruments.fixed_income.AgencyMbsPassthrough
AgencyTba = _valuations.instruments.fixed_income.AgencyTba
AgencyCmo = _valuations.instruments.fixed_income.AgencyCmo
DollarRoll = _valuations.instruments.fixed_income.DollarRoll
FIIndexTotalReturnSwap = _valuations.instruments.fixed_income.FIIndexTotalReturnSwap
StructuredCredit = _valuations.instruments.fixed_income.StructuredCredit

__all__: list[str] = [
    "AgencyCmo",
    "AgencyMbsPassthrough",
    "AgencyTba",
    "Bond",
    "BondFuture",
    "ConvertibleBond",
    "DollarRoll",
    "FIIndexTotalReturnSwap",
    "InflationLinkedBond",
    "RevolvingCredit",
    "StructuredCredit",
    "TermLoan",
]
