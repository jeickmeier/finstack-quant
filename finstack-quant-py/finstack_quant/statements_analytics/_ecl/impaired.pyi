"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements_analytics/ecl/impaired.rs``."""

from __future__ import annotations

from collections.abc import Sequence
from decimal import Decimal
from datetime import date
from finstack_quant.core.money import Money
from finstack_quant.core.table import ArrowTable
from typing import Any, Literal, ClassVar, NotRequired, TypedDict
import pandas as pd
import polars as pl
from finstack_quant.statements_analytics._ecl.staging import Stage

class ImpairedExposure:
    """Carry the sourced and dated exposure for an impaired recovery assessment.

    Examples
    --------
    >>> from decimal import Decimal
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.statements_analytics import ImpairedExposure
    >>> exposure = ImpairedExposure(
    ...     exposure_id="loan",
    ...     carrying_amount_source="ledger",
    ...     carrying_amount=Money(100, "USD"),
    ...     reporting_date="2025-01-01",
    ...     effective_interest_rate=Decimal("0.10"),
    ...     recovery_day_count="act_365f",
    ... )
    """

    def __init__(
        self,
        *,
        exposure_id: str,
        carrying_amount_source: str,
        carrying_amount: Money,
        reporting_date: date | str,
        effective_interest_rate: Decimal | str,
        recovery_day_count: str,
    ) -> None:
        """Record the approved carrying basis and original EIR policy.

        Parameters
        ----------
        exposure_id : str
            Reporting population identifier for the credit-impaired exposure.
        carrying_amount_source : str
            Approved ledger or carrying-value source identifier.
        carrying_amount : Money
            Nonnegative carrying amount in reporting currency.
        reporting_date : date or str
            Dated measurement and discount base, or ISO YYYY-MM-DD.
        effective_interest_rate : Decimal or str
            Exact annual effective original discount rate in decimal units, above -1.
        recovery_day_count : str
            Canonical wire day count, such as act_365f, for the recovery horizon.

        Raises
        ------
        ValueError
            If the input cannot be decoded as the native exposure.
        """
        ...

class RecoveryEvidence:
    """Sourced cash recovery with one expected receipt date.

    Examples
    --------
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.statements_analytics import RecoveryEvidence
    >>> recovery = RecoveryEvidence(source_id="workout", amount=Money(88, "USD"), date="2026-01-01")
    """

    def __init__(self, *, source_id: str, amount: Money, date: date | str) -> None:
        """Record projected recovery cash on the carrying exposure's basis.

        Parameters
        ----------
        source_id : str
            Approved workout or recovery estimate identifier.
        amount : Money
            Nonnegative recovery in the carrying amount's currency.
        date : date or str
            Expected receipt date, or ISO YYYY-MM-DD.

        Raises
        ------
        ValueError
            If the input cannot be decoded as the native recovery.
        """
        ...

class ImpairedEclResult:
    """Dated credit-impaired discount and signed loss bridge.

    Examples
    --------
    >>> from decimal import Decimal
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.statements_analytics import ImpairedExposure, RecoveryEvidence, compute_impaired_ecl
    >>> exposure = ImpairedExposure(
    ...     exposure_id="loan",
    ...     carrying_amount_source="ledger",
    ...     carrying_amount=Money(100, "USD"),
    ...     reporting_date="2025-01-01",
    ...     effective_interest_rate=Decimal("0.10"),
    ...     recovery_day_count="act_365f",
    ... )
    >>> recovery = RecoveryEvidence(source_id="workout", amount=Money(88, "USD"), date="2026-01-01")
    >>> round(compute_impaired_ecl(exposure, recovery).allowance.amount, 2)
    20.0
    """

    @property
    def present_value_recovery(self) -> Money:
        """Return recovery cash discounted once to the reporting date.

        This property does not raise.

        Returns
        -------
        Money
            Sourced recovery proceeds discounted to the reporting date in the carrying currency.
        """
        ...

    @property
    def signed_shortfall(self) -> Money:
        """Return carrying value less discounted recovery before the zero floor.

        This property does not raise.

        Returns
        -------
        Money
            Carrying value less the present value of recovery, in reporting currency.
        """
        ...

    @property
    def allowance(self) -> Money:
        """Return the nonnegative signed loss portion in reporting currency.

        This property does not raise.

        Returns
        -------
        Money
            Positive portion of the signed recovery shortfall, in reporting currency.
        """
        ...

    @property
    def time_to_recovery_years(self) -> float:
        """Return the fraction of a year to receipt on the declared discount clock.

        This property does not raise.

        Returns
        -------
        float
            Nonnegative year fraction from report date until expected receipt.
        """
        ...

    @property
    def discount_factor(self) -> float:
        """Return the annual effective original EIR factor applied to recovery.

        This property does not raise.

        Returns
        -------
        float
            Annual effective EIR factor at the specified recovery horizon.
        """
        ...

    def to_json(self) -> str:
        """Return the allowance and its full source evidence as native JSON.

        Returns
        -------
        str
            Serialized credit-impaired evidence and the reconciled recovery bridge.

        Raises
        ------
        ValueError
            If the value cannot be represented as JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> ImpairedEclResult:
        """Rebuild the native ImpairedEclResult from its JSON record.

        Parameters
        ----------
        json : str
            JSON object with the dated evidence and fields of this record.

        Returns
        -------
        ImpairedEclResult
            Decoded native value.

        Raises
        ------
        ValueError
            If the JSON syntax or field values are invalid.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import ImpairedEclResult
        >>> try:
        ...     ImpairedEclResult.from_json("null")
        ... except ValueError:
        ...     print("A record is required")
        A record is required
        """
        ...

    def __reduce__(self) -> tuple[object, tuple[str]]:
        """Return the reconstruction callable and native JSON arguments for pickle."""
        ...

def compute_impaired_ecl(exposure: ImpairedExposure, recovery: RecoveryEvidence) -> ImpairedEclResult:
    """Discount an approved Stage 3 recovery and reconcile the credit allowance.

    Parameters
    ----------
    exposure : ImpairedExposure
        Sourced same-basis carrying amount and reporting date with original effective rate and clock.
    recovery : RecoveryEvidence
        Approved cash and projected receipt date in the exposure currency.

    Returns
    -------
    ImpairedEclResult
        Recovery provenance, discount, signed shortfall and nonnegative allowance.

    Raises
    ------
    ValueError
        If dates, source names, currency, amounts, discount rate or discount method are invalid.

    Examples
    --------
    >>> from decimal import Decimal
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.statements_analytics import ImpairedExposure, RecoveryEvidence, compute_impaired_ecl
    >>> exposure = ImpairedExposure(
    ...     exposure_id="loan",
    ...     carrying_amount_source="ledger",
    ...     carrying_amount=Money(100, "USD"),
    ...     reporting_date="2025-01-01",
    ...     effective_interest_rate=Decimal("0.10"),
    ...     recovery_day_count="act_365f",
    ... )
    >>> recovery = RecoveryEvidence(source_id="workout", amount=Money(88, "USD"), date="2026-01-01")
    >>> round(compute_impaired_ecl(exposure, recovery).allowance.amount, 2)
    20.0
    """
    ...

class _ImpairedEclOutcome(TypedDict):
    id: str
    weight: str
    status: Literal["measured", "unavailable", "zero_weight"]
    result: NotRequired[dict[str, Any]]
    weighted_allowance: NotRequired[dict[str, str]]
    reason: NotRequired[str]

class ImpairedEclScenario:
    """A named recovery scenario with an exact probability.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import ImpairedEclScenario
    >>> pending = ImpairedEclScenario(id="pending workout", weight="0.30")
    """

    def __init__(
        self,
        *,
        id: str,
        weight: Decimal | str,
        recovery: RecoveryEvidence | None = None,
    ) -> None:
        """Record the probability and available evidence for one recovery.

        Parameters
        ----------
        id : str
            Stable unique scenario name used in the result.
        weight : Decimal or str
            Exact probability in [0, 1], with no normalization.
        recovery : RecoveryEvidence or None
            Expected cash and receipt date on the exposure basis. None leaves
            a positive-weight scenario unavailable.

        Raises
        ------
        ValueError
            If weight cannot be read as an exact decimal probability.
        """
        ...

class WeightedImpairedEclResult:
    """Stage 3 allowance and standalone evidence for each recovery scenario.

    Examples
    --------
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.statements_analytics import ImpairedEclScenario, ImpairedExposure
    >>> from finstack_quant.statements_analytics import RecoveryEvidence, compute_impaired_ecl_weighted
    >>> exposure = ImpairedExposure(
    ...     exposure_id="loan",
    ...     carrying_amount_source="ledger",
    ...     carrying_amount=Money(100, "USD"),
    ...     reporting_date="2025-01-01",
    ...     effective_interest_rate="0",
    ...     recovery_day_count="act_365f",
    ... )
    >>> base = RecoveryEvidence(source_id="workout-base", amount=Money(80, "USD"), date="2026-01-01")
    >>> stress = RecoveryEvidence(source_id="workout-stress", amount=Money(20, "USD"), date="2026-01-01")
    >>> result = compute_impaired_ecl_weighted(
    ...     exposure,
    ...     [
    ...         ImpairedEclScenario(id="base", weight="0.70", recovery=base),
    ...         ImpairedEclScenario(id="stress", weight="0.30", recovery=stress),
    ...     ],
    ... )
    >>> assert result.allowance is not None
    >>> result.allowance.amount
    38.0
    """

    @property
    def allowance(self) -> Money | None:
        """Get the weighted Stage 3 allowance in the carrying currency.

        This property does not raise.

        Returns
        -------
        Money or None
            Total allowance if every positive-weight row was measured.
        """
        ...

    @property
    def outcomes(self) -> list[_ImpairedEclOutcome]:
        """Get measured recoveries and unavailable scenarios in request order.

        Returns
        -------
        list[_ImpairedEclOutcome]
            Scenario labels, exact decimal probabilities and status, with
            measured evidence and currency amounts when available.

        Raises
        ------
        ValueError
            If scenario evidence cannot be converted to native Python values.
        """
        ...

def compute_impaired_ecl_weighted(
    exposure: ImpairedExposure, scenarios: Sequence[ImpairedEclScenario]
) -> WeightedImpairedEclResult:
    """Measure and weight Stage 3 under a single carrying and discount policy.

    Parameters
    ----------
    exposure : ImpairedExposure
        Verified carrying amount, currency, reporting date and original EIR convention.
    scenarios : Sequence[ImpairedEclScenario]
        Unique economic scenarios with exact probabilities summing to one.
        Positive-weight inputs require dated recovery evidence for a full total.

    Returns
    -------
    WeightedImpairedEclResult
        Same-currency allowance and all priced or unavailable scenario rows.

    Raises
    ------
    ValueError
        If scenario IDs, weights, source, dates, currency or amounts are invalid.
    RuntimeError
        If checked monetary arithmetic overflows.

    Examples
    --------
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.statements_analytics import ImpairedEclScenario, ImpairedExposure
    >>> from finstack_quant.statements_analytics import RecoveryEvidence, compute_impaired_ecl_weighted
    >>> exposure = ImpairedExposure(
    ...     exposure_id="loan",
    ...     carrying_amount_source="ledger",
    ...     carrying_amount=Money(100, "USD"),
    ...     reporting_date="2025-01-01",
    ...     effective_interest_rate="0",
    ...     recovery_day_count="act_365f",
    ... )
    >>> base = RecoveryEvidence(source_id="workout-base", amount=Money(80, "USD"), date="2026-01-01")
    >>> stress = RecoveryEvidence(source_id="workout-stress", amount=Money(20, "USD"), date="2026-01-01")
    >>> result = compute_impaired_ecl_weighted(
    ...     exposure,
    ...     [
    ...         ImpairedEclScenario(id="base", weight="0.70", recovery=base),
    ...         ImpairedEclScenario(id="stress", weight="0.30", recovery=stress),
    ...     ],
    ... )
    >>> assert result.allowance is not None
    >>> result.allowance.amount
    38.0
    """
    ...
