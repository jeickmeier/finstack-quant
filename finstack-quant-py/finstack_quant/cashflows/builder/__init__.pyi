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

from finstack_quant.cashflows.builder._rates import (
    abs_to_smm as abs_to_smm,
    annual_to_monthly as annual_to_monthly,
    monthly_to_annual as monthly_to_annual,
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
