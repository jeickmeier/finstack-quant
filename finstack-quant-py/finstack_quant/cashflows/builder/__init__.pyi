"""
Composable cashflow builder: coupon/fee/amortization specs, CashFlowBuilder, CashFlowSchedule.

Typed bindings for the spec types under ``finstack_quant_cashflows::builder``:
schedule generation parameters, fixed/floating/step-up coupon specs,
amortization and notional rules, fee specs, and the credit behavior models
(prepayment, default, recovery) used by structured/private-credit cashflow
programs. ``CashFlowSchedule.builder()`` is the fluent entry point into
``CashFlowBuilder``, whose chained methods assemble a principal, coupon
legs, fees, and principal events into a canonical ``CashFlowSchedule``.
Schedule accessors (``get_flows``, notional, day count, metadata,
validation) are available on ``CashFlowSchedule``. The canonical JSON
program bridge on ``finstack_quant.cashflows`` remains available for the
full build pipeline as well.

Examples
--------
>>> from finstack_quant.cashflows.builder import Notional
>>> Notional.par(1_000_000.0, "USD").initial.amount
1000000.0

"""

from __future__ import annotations
from finstack_quant.cashflows.builder._specs.amortization import (
    AmortizationSpec as AmortizationSpec,
    Notional as Notional,
)
from finstack_quant.cashflows.builder._orchestrator import CashFlowBuilder as CashFlowBuilder
from finstack_quant.cashflows.builder._schedule import (
    CashFlowMeta as CashFlowMeta,
    CashFlowSchedule as CashFlowSchedule,
    merge_cashflow_schedules as merge_cashflow_schedules,
)
from finstack_quant.cashflows.builder._specs.coupon import (
    CouponType as CouponType,
    FixedCouponSpec as FixedCouponSpec,
    FloatingCouponSpec as FloatingCouponSpec,
    FloatingLegCompounding as FloatingLegCompounding,
    FloatingRateFallback as FloatingRateFallback,
    FloatingRateSpec as FloatingRateSpec,
    OvernightIndexConstraintApplication as OvernightIndexConstraintApplication,
    StepUpCouponSpec as StepUpCouponSpec,
)
from finstack_quant.cashflows.builder._specs.default import DefaultModelSpec as DefaultModelSpec
from finstack_quant.cashflows.builder._specs.fees import (
    FeeAccrualBasis as FeeAccrualBasis,
    FeeBase as FeeBase,
    FeeSpec as FeeSpec,
)
from finstack_quant.cashflows.builder._specs.prepayment import PrepaymentModelSpec as PrepaymentModelSpec
from finstack_quant.cashflows.builder._specs.recovery import RecoveryModelSpec as RecoveryModelSpec
from finstack_quant.cashflows.builder._specs.schedule import (
    PrincipalExchange as PrincipalExchange,
    RollRule as RollRule,
    ScheduleParams as ScheduleParams,
)

__all__ = [
    "AmortizationSpec",
    "CashFlowBuilder",
    "CashFlowMeta",
    "CashFlowSchedule",
    "CouponType",
    "DefaultModelSpec",
    "FeeAccrualBasis",
    "FeeBase",
    "FeeSpec",
    "FixedCouponSpec",
    "FloatingCouponSpec",
    "FloatingLegCompounding",
    "FloatingRateFallback",
    "FloatingRateSpec",
    "Notional",
    "OvernightIndexConstraintApplication",
    "PrepaymentModelSpec",
    "PrincipalExchange",
    "RecoveryModelSpec",
    "RollRule",
    "ScheduleParams",
    "StepUpCouponSpec",
    "abs_to_smm",
    "annual_to_monthly",
    "monthly_to_annual",
    "merge_cashflow_schedules",
]

def abs_to_smm(speed: float, month: int) -> float:
    """
    Convert an ABS speed to the single-month mortality for a seasoning month.

    Mirrors Rust ``builder::abs_to_smm`` (the kernel behind
    ``PrepaymentModelSpec.abs``). The ABS convention prepays a
    constant share of the *original* balance each month:
    ``SMM_t = speed / (1 - speed * (t - 1))``, capped at ``1.0`` once the
    original balance is exhausted; month 0 is treated as month 1.

    Parameters
    ----------
    speed : float
        Monthly prepayment as a decimal fraction of the original balance
        (``0.015`` = 1.5% ABS), in ``[0, 1]``.
    month : int
        Seasoning month counted from origination (non-negative).

    Returns
    -------
    float
        Single-month mortality as a decimal in ``[0, 1]``.

    Raises
    ------
    ValueError
        If ``speed`` is non-finite or outside ``[0, 1]``.
    OverflowError
        If ``month`` is negative.

    Examples
    --------
    >>> from finstack_quant.cashflows.builder import abs_to_smm
    >>> round(abs_to_smm(0.015, 11), 6)
    0.017647
    """
    ...

def annual_to_monthly(annual: float) -> float:
    """Convert annual conditional prepayment or default mortality to monthly mortality.

    Uses ``monthly = 1 - (1 - annual) ** (1 / 12)`` for both CPR/SMM and CDR/MDR.

    Parameters
    ----------
    annual : float
        Finite annual conditional mortality as a decimal in ``[0, 1]``;
        ``0.06`` represents 6% CPR or CDR. No calendar day-count adjustment applies.

    Returns
    -------
    float
        Monthly conditional mortality as a decimal. Zero and one map to themselves.

    Raises
    ------
    ValueError
        If ``annual`` is non-finite or outside ``[0, 1]``.

    Examples
    --------
    >>> from finstack_quant.cashflows.builder import annual_to_monthly
    >>> round(annual_to_monthly(annual=0.06), 6)
    0.005143
    """
    ...

def monthly_to_annual(monthly: float) -> float:
    """Convert monthly conditional prepayment or default mortality to annual mortality.

    Uses ``annual = 1 - (1 - monthly) ** 12`` for both SMM/CPR and MDR/CDR.

    Parameters
    ----------
    monthly : float
        Finite monthly conditional mortality as a decimal in ``[0, 1]``;
        ``0.01`` represents 1% SMM or MDR. Annualization uses twelve monthly periods.

    Returns
    -------
    float
        Annual conditional mortality as a decimal. Zero and one map to themselves.

    Raises
    ------
    ValueError
        If ``monthly`` is non-finite or outside ``[0, 1]``.

    Examples
    --------
    >>> from finstack_quant.cashflows.builder import annual_to_monthly, monthly_to_annual
    >>> round(monthly_to_annual(monthly=annual_to_monthly(0.06)), 10)
    0.06
    """
    ...
