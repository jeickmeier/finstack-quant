"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_structured_credit/call_assumption.rs``."""

from __future__ import annotations

import datetime
from typing import Any

class CallAssumption:
    """
    Assumed optional redemption for price-to-call analytics.

    A deal-scope call liquidates the collateral on the first payment date at
    or after ``date`` and redeems every note at ``price_pct`` of its balance;
    a tranche-scope call (``tranche_id`` given) leaves the deal's cashflows
    unchanged and only truncates that class's ``*_to_call`` metrics.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.valuations.instruments import CallAssumption
    >>> call = CallAssumption(datetime.date(2027, 1, 15), 100.0)
    >>> call.scope, call.tranche_id
    ('deal', None)
    >>> CallAssumption(datetime.date(2027, 1, 15), 101.0, tranche_id="B").tranche_id
    'B'
    """

    def __init__(self, date: datetime.date, price_pct: float, tranche_id: str | None = None) -> None:
        """
        Construct a call assumption.

        Parameters
        ----------
        date : datetime.date
            Assumed call date; the redemption happens on the first payment date
            at or after it.
        price_pct : float
            Redemption price as a percent of the note balance (``100.0`` = par;
            a premium is paid as interest, a discount is a write-down).
        tranche_id : str, optional
            Restrict the call to one class (tranche-scope). ``None`` calls the
            whole deal.

        Raises
        ------
        ValueError
            If ``date`` is not a valid date.
        """
        ...

    @staticmethod
    def from_json(json: str) -> CallAssumption:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``CallAssumption`` (the shape ``to_json`` writes).

        Returns
        -------
        CallAssumption
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CallAssumption
        >>> try:
        ...     CallAssumption.from_json("{}")
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
            JSON-encoded ``CallAssumption``.

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
            ``(CallAssumption.from_json, (json,))``.

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
    def date(self) -> datetime.date:
        """
        Assumed call date.

        Returns
        -------
        datetime.date
            The call date.

        Raises
        ------
        ValueError
            If the date cannot be converted.
        """
        ...

    @property
    def price_pct(self) -> float:
        """
        Redemption price as a percent of balance.

        Returns
        -------
        float
            The call price.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def scope(self) -> str:
        """
        Whether the call covers the whole deal or one class.

        Returns
        -------
        str
            ``"deal"`` or ``"tranche"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def tranche_id(self) -> str | None:
        """
        The called class for a tranche-scope call.

        Returns
        -------
        str | None
            ``None`` for a deal-scope call.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def scope_spec(self) -> Any:
        """
        ``CallScope`` serde value.

        Returns
        -------
        Any
            ``"deal"`` or ``{"tranche": id}``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
