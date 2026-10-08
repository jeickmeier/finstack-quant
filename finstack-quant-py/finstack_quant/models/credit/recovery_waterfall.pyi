"""
Absolute-priority recovery allocation with estate-inclusive collateral.

Examples
--------
>>> from finstack_quant.models.credit import recovery_waterfall
>>> claim = recovery_waterfall.RecoveryClaim("SEN", "secured", 1, 100.0, 0.0, 0.0, 0.0)
>>> result = recovery_waterfall.allocate_recovery(40.0, [claim])
>>> (result.total_distributed, result.undistributed_estate, result.apr_satisfied)
(40.0, 0.0, True)

"""

from __future__ import annotations

from typing import Any

import pandas

__all__ = [
    "RecoveryAllocation",
    "RecoveryClaim",
    "RecoveryWaterfallResult",
    "allocate_recovery",
]

class RecoveryClaim:
    """
    A claim participating in an absolute-priority recovery waterfall.

    Examples
    --------
    >>> from finstack_quant.models.credit import recovery_waterfall
    >>> claim = recovery_waterfall.RecoveryClaim("SEN", "secured", 1, 100.0, 5.0, 0.0, 0.0)
    >>> (claim.id, claim.total_claim)
    ('SEN', 105.0)
    """

    def __init__(
        self,
        id: str,
        seniority: str,
        priority: int,
        principal: float,
        accrued: float,
        penalties: float,
        collateral_haircut: float,
        collateral_value: float | None = None,
    ) -> None:
        """
        Create a claim for absolute-priority recovery allocation.

        Parameters
        ----------
        id : str
            Stable claim identifier retained on the resulting allocation.
        seniority : str
            Human-readable seniority label used in recovery reporting.
        priority : int
            Absolute-priority rank; lower values receive estate proceeds
            before higher values.
        principal : float
            Outstanding principal claim in the estate's monetary units.
        accrued : float
            Unpaid accrued interest added to the claim amount; pass
            ``0.0`` when none has accrued.
        penalties : float
            Contractual penalty or default-interest claim added to the
            total; pass ``0.0`` when none applies.
        collateral_haircut : float
            Decimal fraction of ``collateral_value`` deducted before
            estate allocation; must lie in ``[0, 1]``. Required so a
            secured claim never receives full collateral credit by
            omission; pass ``0.0`` for an unsecured claim.
        collateral_value : float or None, default None
            Gross market value of collateral pledged to this claim, or
            ``None`` for an unsecured claim (matching the Rust wire form,
            where the field may be omitted).

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...
    @property
    def id(self) -> str:
        """
        Stable identifier for this claim.

        Returns
        -------
        str
            Stable identifier retained on the resulting allocation.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def seniority(self) -> str:
        """
        Seniority class the claim sits in.

        Returns
        -------
        str
            Human-readable seniority label used in recovery reporting.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def priority(self) -> int:
        """
        Absolute-priority rank; lower ranks are paid first.

        Returns
        -------
        int
            Absolute-priority rank; lower values receive estate proceeds first.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def principal(self) -> float:
        """
        Principal outstanding, before accrued interest and penalties.

        Returns
        -------
        float
            Outstanding principal claim in the estate's monetary units.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def accrued(self) -> float:
        """
        Accrued but unpaid interest included in the claim.

        Returns
        -------
        float
            Unpaid accrued interest added to the claim amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def penalties(self) -> float:
        """
        Penalties and fees included in the claim.

        Returns
        -------
        float
            Contractual penalty or default-interest claim added to the total.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def collateral_value(self) -> float | None:
        """
        Gross value of collateral pledged to this claim.

        Returns
        -------
        float | None
            Pledged collateral market value, or ``None`` when the claim is unsecured.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def collateral_haircut(self) -> float:
        """
        Haircut applied to pledged collateral, as a fraction in ``[0, 1]``.

        Returns
        -------
        float
            Decimal haircut deducted from collateral value before estate allocation.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_claim(self) -> float:
        """
        Principal plus accrued interest and penalties.

        Returns
        -------
        float
            Sum of principal, accrued interest, and penalties in estate units.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @staticmethod
    def from_json(json: str) -> RecoveryClaim:
        """
        Deserialize a ``RecoveryClaim`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON text produced by :meth:`to_json` (strict serde; unknown or
            invalid fields are rejected).

        Returns
        -------
        RecoveryClaim
            Reconstructed value.

        Raises
        ------
        ValueError
            If the payload is not valid ``RecoveryClaim`` JSON.

        Examples
        --------
        >>> from finstack_quant.models.credit import recovery_waterfall
        >>> value = recovery_waterfall.RecoveryClaim("SEN", "secured", 1, 100.0, 0.0, 0.0, 0.0)
        >>> recovery_waterfall.RecoveryClaim.from_json(value.to_json()) == value
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact canonical JSON.

        Returns
        -------
        str
            JSON text accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the canonical JSON representation.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(from_json, (json,))`` so unpickling rebuilds the value.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

class RecoveryAllocation:
    """
    Recovery allocated to one claim under absolute priority.

    Examples
    --------
    >>> from finstack_quant.models.credit import recovery_waterfall
    >>> claim = recovery_waterfall.RecoveryClaim("SEN", "secured", 1, 100.0, 0.0, 0.0, 0.0)
    >>> allocation = recovery_waterfall.allocate_recovery(40.0, [claim]).allocations[0]
    >>> (allocation.id, allocation.total_recovery, allocation.recovery_rate)
    ('SEN', 40.0, 0.4)
    """

    @property
    def id(self) -> str:
        """
        Stable identifier for this claim.

        Returns
        -------
        str
            Claim identifier copied from the source ``RecoveryClaim``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def seniority(self) -> str:
        """
        Seniority class the claim sits in.

        Returns
        -------
        str
            Human-readable seniority label copied from the source claim.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def priority(self) -> int:
        """
        Absolute-priority rank; lower ranks are paid first.

        Returns
        -------
        int
            Absolute-priority rank copied from the source claim.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_claim(self) -> float:
        """
        Principal plus accrued interest and penalties.

        Returns
        -------
        float
            Total admitted claim amount in estate units.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def collateral_recovery(self) -> float:
        """
        Amount recovered from pledged collateral.

        Returns
        -------
        float
            Recovery attributed to pledged collateral, in estate units.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def general_recovery(self) -> float:
        """
        Amount recovered from the general estate.

        Returns
        -------
        float
            Recovery attributed to the unsecured estate, in estate units.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_recovery(self) -> float:
        """
        Collateral plus general recovery.

        Returns
        -------
        float
            Sum of collateral and general recovery, in estate units.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def recovery_rate(self) -> float:
        """
        Total recovery divided by total claim, as a fraction in ``[0, 1]``.

        Returns
        -------
        float
            ``total_recovery / total_claim``, floored at zero when the claim is zero.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def deficiency(self) -> float:
        """
        Unrecovered claim after collateral and general recovery.

        Returns
        -------
        float
            ``total_claim - total_recovery``, floored at zero.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @staticmethod
    def from_json(json: str) -> RecoveryAllocation:
        """
        Deserialize a ``RecoveryAllocation`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON text produced by :meth:`to_json` (strict serde; unknown or
            invalid fields are rejected).

        Returns
        -------
        RecoveryAllocation
            Reconstructed value.

        Raises
        ------
        ValueError
            If the payload is not valid ``RecoveryAllocation`` JSON.

        Examples
        --------
        >>> from finstack_quant.models.credit import recovery_waterfall
        >>> value = recovery_waterfall.allocate_recovery(
        ...     40.0, [recovery_waterfall.RecoveryClaim("SEN", "secured", 1, 100.0, 0.0, 0.0, 0.0)]
        ... ).allocations[0]
        >>> recovery_waterfall.RecoveryAllocation.from_json(value.to_json()) == value
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact canonical JSON.

        Returns
        -------
        str
            JSON text accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the canonical JSON representation.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(from_json, (json,))`` so unpickling rebuilds the value.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...
    def to_dataframe(self) -> pandas.DataFrame:
        """
        Single-row frame with the allocation columns (``id``, ``seniority``, ``priority``, ``total_claim``, ``collateral_recovery``, ``general_recovery``, ``total_recovery``, ``recovery_rate``, ``deficiency``).

        Returns
        -------
        pandas.DataFrame
            Single-row frame with the allocation columns (``id``, ``seniority``, ``priority``, ``total_claim``, ``collateral_recovery``, ``general_recovery``, ``total_recovery``, ``recovery_rate``, ``deficiency``).

        Raises
        ------
        ValueError
            If the value cannot be serialized into a pandas object.
        """
        ...

class RecoveryWaterfallResult:
    """
    Result of allocating a distributable estate across claims.

    Examples
    --------
    >>> from finstack_quant.models.credit import recovery_waterfall
    >>> claim = recovery_waterfall.RecoveryClaim("SEN", "secured", 1, 100.0, 0.0, 0.0, 0.0)
    >>> result = recovery_waterfall.allocate_recovery(40.0, [claim])
    >>> (result.total_distributed, result.undistributed_estate, result.apr_satisfied)
    (40.0, 0.0, True)
    """

    @property
    def total_distributed(self) -> float:
        """
        Sum of every claim's total recovery.

        Returns
        -------
        float
            Aggregate recovery paid across all claims, in estate units.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def undistributed_estate(self) -> float:
        """
        Estate value left after all claims are satisfied.

        Returns
        -------
        float
            Residual estate after absolute-priority allocation, in estate units.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def apr_satisfied(self) -> bool:
        """
        Whether the run respected absolute priority end to end.

        Returns
        -------
        bool
            ``True`` when every senior claim was paid before any junior claim.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def allocations(self) -> list[RecoveryAllocation]:
        """
        Per-claim allocations, in absolute-priority order.

        Returns
        -------
        list[recovery_waterfall.RecoveryAllocation]
            One ``RecoveryAllocation`` per claim, ordered by increasing priority.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pandas.DataFrame:
        """
        Export the per-claim allocations as a pandas DataFrame.

        Columns: ``id``, ``seniority``, ``priority``, ``total_claim``,
        ``collateral_recovery``, ``general_recovery``, ``total_recovery``,
        ``recovery_rate``, ``deficiency``.

        One row per claim — the natural grain of a waterfall. Rows keep the
        Rust ordering (ascending ``priority``, then original claim order),
        so repeated exports of the same result are byte-identical. The
        estate-level fields (:attr:`total_distributed`,
        :attr:`undistributed_estate`, :attr:`apr_satisfied`) are
        deliberately not repeated on every row; read them from the result
        object.

        A waterfall with no claims yields a zero-row frame that still
        carries the columns above.

        Returns
        -------
        pandas.DataFrame
            One row per claim, in absolute-priority order.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

def allocate_recovery(
    estate_value: float,
    claims: list[RecoveryClaim],
) -> RecoveryWaterfallResult:
    """
    Allocate an insolvent estate under absolute priority.

    Parameters
    ----------
    estate_value : float
        Cash estate available for distribution after any external costs,
        expressed in the same monetary units as each claim.
    claims : list[RecoveryClaim]
        Claims to rank by ``priority``. Collateral recovery is applied to
        each claim before general estate proceeds are distributed.

    Returns
    -------
    RecoveryWaterfallResult
        Per-claim recoveries, undistributed estate, and APR satisfaction.

    Raises
    ------
    ValueError
        If the estate or claim amounts are negative or non-finite, a claim
        identifier or seniority is blank, identifiers are duplicated,
        a haircut is outside ``[0, 1]``, a claim total overflows, or net
        collateral exceeds the estate.
    RuntimeError
        If the allocator cannot reserve its claim-index storage or a
        recovery-conservation invariant fails.


    Examples
    --------
    >>> from finstack_quant.models.credit import recovery_waterfall
    >>> claims = [recovery_waterfall.RecoveryClaim("SEN", "secured", 1, 100.0, 0.0, 0.0, 0.0)]
    >>> recovery_waterfall.allocate_recovery(40.0, claims).allocations[0].recovery_rate
    0.4

    """
    ...
