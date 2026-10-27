"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_structured_credit/hedge_swap.rs``."""

from __future__ import annotations

from typing import Any, Literal
from finstack_quant.valuations.instruments._typed_rates import InterestRateSwap

class HedgeSwap:
    """
    An interest-rate swap settled through the deal waterfall: net receipts
    join interest collections (and the IC test), net payments rank as a
    senior or junior fee.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import HedgeSwap, InterestRateSwap
    >>> swap = InterestRateSwap.example()
    >>> hedge = HedgeSwap(swap, notional="pool_par", priority="senior_fee")
    >>> hedge.priority, hedge.notional
    ('senior_fee', 'pool_par')
    >>> HedgeSwap(swap, notional={"tranche_par": "A"}).notional
    {'tranche_par': 'A'}
    """

    def __init__(
        self,
        swap: InterestRateSwap,
        notional: str | dict[str, Any] | None = None,
        priority: Literal["senior_fee", "junior_fee"] | None = None,
    ) -> None:
        """
        Construct a hedge from a typed swap.

        Parameters
        ----------
        swap : InterestRateSwap
            The swap; its side, legs and market dependencies drive the projected
            net settlements.
        notional : str | dict[str, Any], optional
            ``SwapNotional`` serde value: ``"contractual"`` (the swap's own
            notional, the default), ``"pool_par"`` (balance-tracking on the
            pool) or ``{"tranche_par": "<tranche id>"}`` (balance-tracking on
            one class).
        priority : {"senior_fee", "junior_fee"}, optional
            ``"senior_fee"`` (default: net payments rank with the senior fees)
            or ``"junior_fee"`` (after every note coupon).

        Raises
        ------
        ValueError
            If ``notional`` or ``priority`` is not a recognized value.
        """
        ...

    @staticmethod
    def from_json(json: str) -> HedgeSwap:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``HedgeSwap`` (the shape ``to_json`` writes).

        Returns
        -------
        HedgeSwap
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import HedgeSwap
        >>> try:
        ...     HedgeSwap.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            JSON-encoded ``HedgeSwap``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust value.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(HedgeSwap.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the value.
        """
        ...

    @property
    def swap(self) -> InterestRateSwap:
        """
        The hedged swap.

        Returns
        -------
        InterestRateSwap
            A typed copy of the swap.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def notional(self) -> Any:
        """
        ``SwapNotional`` serde value.

        Returns
        -------
        Any
            ``"contractual"``, ``"pool_par"`` or ``{"tranche_par": id}``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def priority(self) -> str:
        """
        Fee rank of the net payments.

        Returns
        -------
        str
            ``"senior_fee"`` or ``"junior_fee"``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
