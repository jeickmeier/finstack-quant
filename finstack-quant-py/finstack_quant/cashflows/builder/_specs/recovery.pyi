"""Stub declarations mirroring ``finstack-quant-py/src/bindings/cashflows/builder/specs/recovery.rs``."""

from __future__ import annotations

from typing import Any

class RecoveryModelSpec:
    """
    Recovery model specification for defaulted credit exposures.

    Combines a recovery rate fraction with the lag between default and the
    recovery cashflow.

    Examples
    --------
    >>> from finstack_quant.cashflows.builder import RecoveryModelSpec
    >>> spec = RecoveryModelSpec(rate=0.40, recovery_lag=12)
    >>> spec.recovery_lag
    12
    """

    def __init__(self, rate: float, recovery_lag: int) -> None:
        """
        Construct a recovery model with a rate and recovery lag.

        Parameters
        ----------
        rate : float
            Recovery rate as a fraction in ``[0.0, 1.0]`` (e.g. ``0.40``
            for 40%).
        recovery_lag : int
            Number of months between default and the recovery cashflow.

        Raises
        ------
        ValueError
            If *rate* is not finite; range validation happens in
            :meth:`validate`.
        """
        ...

    @property
    def rate(self) -> float:
        """
        Recovery rate as a fraction.

        Returns
        -------
        float
            The configured recovery rate, expected in ``[0.0, 1.0]``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def recovery_lag(self) -> int:
        """
        Months between default and recovery cash receipt.

        Returns
        -------
        int
            The number of months between default and recovery cashflow.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def with_severity_vector(self, severity_vector: list[float]) -> RecoveryModelSpec:
        """
        Return a copy carrying a seasoning-indexed severity vector.

        Parameters
        ----------
        severity_vector : list[float]
            Loss given default as decimals in ``[0, 1]``, one entry per
            seasoning month (the last entry extends); overrides ``1 - rate``
            where present.

        Returns
        -------
        RecoveryModelSpec
            A new spec with the vector attached (this spec is unchanged).

        Raises
        ------
        TypeError
            If ``severity_vector`` is not a list of floats.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import RecoveryModelSpec
        >>> RecoveryModelSpec(0.4, 12).with_severity_vector([0.5, 0.6]).recovery_rate(1)
        0.5
        """
        ...

    @property
    def severity_vector(self) -> list[float] | None:
        """
        Severity vector by seasoning month.

        Returns
        -------
        list[float] | None
            ``None`` when only ``rate`` applies.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def recovery_rate(self, seasoning_months: int) -> float:
        """
        Recovery rate for the supplied seasoning.

        Parameters
        ----------
        seasoning_months : int
            Months since origination of the defaulting month, one-based:
            months 0 and 1 read the first vector entry, and months past the
            end hold the last one.

        Returns
        -------
        float
            ``1 - severity`` from the vector when present, else ``rate``,
            clamped to ``[0, 1]``.

        Raises
        ------
        OverflowError
            If ``seasoning_months`` is outside the unsigned 32-bit range.
        """
        ...

    def validate(self) -> None:
        """
        Validate that the rate is finite and in ``[0.0, 1.0]``.

        Raises
        ------
        ValueError
            If the rate is not finite or falls outside ``[0.0, 1.0]``.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            Strict-serde JSON document; round-trips through :meth:`from_json`.

        Raises
        ------
        ValueError
            If a field cannot be represented in JSON (non-finite float).

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import RecoveryModelSpec
        >>> isinstance(RecoveryModelSpec(0.4, 12).to_json(), str)
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> RecoveryModelSpec:
        """
        Deserialize from the canonical JSON wire form (strict field names).

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        RecoveryModelSpec
            Reconstructed value.

        Raises
        ------
        ValueError
            If the JSON is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import RecoveryModelSpec
        >>> value = RecoveryModelSpec(0.4, 12)
        >>> RecoveryModelSpec.from_json(value.to_json()).to_json() == value.to_json()
        True
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Pickle support via the JSON wire form (``from_json``, ``(to_json(),)``).

        Returns
        -------
        tuple
            ``(from_json, (json,))`` reconstructor pair.

        Raises
        ------
        ValueError
            If the value holds a non-finite float that JSON cannot carry.
        """
        ...
