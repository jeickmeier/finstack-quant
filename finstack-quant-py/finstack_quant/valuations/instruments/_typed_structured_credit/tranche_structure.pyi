"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_structured_credit/tranche_structure.rs``."""

from __future__ import annotations

from typing import Any
from finstack_quant.core.money import Money
from finstack_quant.valuations.instruments._typed_structured_credit.tranche import Tranche

class TrancheStructure:
    """
    Capital structure formed from a list of tranches.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import Tranche
    >>> tranche = (
    ...     Tranche
    ...     .builder()
    ...     .id("A")
    ...     .attach_pct(0.0)
    ...     .detach_pct(100.0)
    ...     .seniority("senior")
    ...     .original_balance(Money(100.0, Currency("USD")))
    ...     .coupon_fixed(0.05)
    ...     .maturity(datetime.date(2029, 1, 1))
    ...     .build()
    ... )
    >>> from finstack_quant.valuations.instruments import TrancheStructure
    >>> "tranches=1" in repr(TrancheStructure([tranche]))
    True

    """

    def __init__(self, tranches: list[Tranche]) -> None:
        """
        Capital structure formed from a list of tranches.

        Validates that attachment/detachment points tile ``[0, 100]`` without
        gaps or overlaps, that every tranche shares one currency, and assigns
        each tranche a distinct, strictly-increasing ``payment_priority``
        ranked by seniority.

        Parameters
        ----------
        tranches : list[Tranche]
            Tranches forming the capital structure.

        Returns
        -------
        TrancheStructure
            The validated tranche structure.

        Raises
        ------
        ValueError
            If ``tranches`` is empty, has non-finite attachment/detachment
            points, leaves a gap/overlap, doesn't tile to 100%, or mixes
            currencies.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import Tranche, TrancheStructure
        >>> senior = (
        ...     Tranche
        ...     .builder()
        ...     .id("A")
        ...     .attach_pct(10.0)
        ...     .detach_pct(100.0)
        ...     .seniority("senior")
        ...     .original_balance(Money(72_000_000.0, Currency("USD")))
        ...     .coupon_fixed(0.05)
        ...     .maturity(datetime.date(2031, 1, 15))
        ...     .build()
        ... )
        >>> equity = (
        ...     Tranche
        ...     .builder()
        ...     .id("E")
        ...     .attach_pct(0.0)
        ...     .detach_pct(10.0)
        ...     .seniority("equity")
        ...     .original_balance(Money(8_000_000.0, Currency("USD")))
        ...     .coupon_fixed(0.0)
        ...     .maturity(datetime.date(2031, 1, 15))
        ...     .build()
        ... )
        >>> structure = TrancheStructure([senior, equity])
        >>> "tranches=2" in repr(structure)
        True
        """
        ...

    @staticmethod
    def from_balances(tranches: list[Tranche]) -> TrancheStructure:
        """
        Build a structure whose attachment and detachment points come from the
        balance shares alone (mirrors Rust ``TrancheStructure::from_balances``).

        Any points declared on the tranches are discarded: the first-loss class
        attaches at 0, each senior class stacks on top in payment-priority order
        and the most senior class detaches at 100.

        Parameters
        ----------
        tranches : list[Tranche]
            Notes of the capital structure; ``seniority`` and
            ``original_balance`` decide the boundaries.

        Returns
        -------
        TrancheStructure
            The validated structure with derived points.

        Raises
        ------
        ValueError
            If the list is empty or the notes mix currencies.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import Tranche, TrancheStructure
        >>> def note(id_, seniority, balance):
        ...     return (
        ...         Tranche
        ...         .builder()
        ...         .id(id_)
        ...         .seniority(seniority)
        ...         .original_balance(Money(balance, Currency("USD")))
        ...         .coupon_fixed(0.05)
        ...         .maturity(datetime.date(2031, 1, 15))
        ...         .build()
        ...     )
        >>> structure = TrancheStructure.from_balances([
        ...     note("A", "senior", 60.0),
        ...     note("B", "mezzanine", 30.0),
        ...     note("E", "equity", 10.0),
        ... ])
        >>> [(t.id, t.attach_pct, t.detach_pct) for t in structure.tranches]
        [('A', 40.0, 100.0), ('B', 10.0, 40.0), ('E', 0.0, 10.0)]
        """
        ...

    @staticmethod
    def from_json(json: str) -> TrancheStructure:
        """
        Deserialize from the JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            Strict JSON object with exactly the fields ``to_json`` writes.

        Returns
        -------
        TrancheStructure
            The reconstructed value.

        Raises
        ------
        ValueError
            If the JSON is malformed or has the wrong shape.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import Tranche
        >>> tranche = (
        ...     Tranche
        ...     .builder()
        ...     .id("A")
        ...     .attach_pct(0.0)
        ...     .detach_pct(100.0)
        ...     .seniority("senior")
        ...     .original_balance(Money(100.0, Currency("USD")))
        ...     .coupon_fixed(0.05)
        ...     .maturity(datetime.date(2029, 1, 1))
        ...     .build()
        ... )
        >>> from finstack_quant.valuations.instruments import TrancheStructure
        >>> structure = TrancheStructure([tranche])
        >>> restored = TrancheStructure.from_json(structure.to_json())
        >>> restored.to_json() == structure.to_json()
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form (also used by ``pickle``).

        Returns
        -------
        str
            Strict JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Same content as ``json.loads(self.to_json())``.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(from_json, (payload,))``.

        Raises
        ------
        ValueError
            If the value cannot be reconstructed from its JSON form.
        """
        ...

    @property
    def tranches(self) -> list[Tranche]:
        """
        Tranches in payment-priority order.

        Returns
        -------
        list[Tranche]
            Independent copies of the tranches.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_size(self) -> Money:
        """
        Total original size of the capital structure.

        Returns
        -------
        Money
            Currency-tagged amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
