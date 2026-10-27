"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_structured_credit/waterfall.rs``."""

from __future__ import annotations

from typing import Any
from finstack_quant.valuations.instruments._typed_structured_credit.coverage_rules import CoverageRules

class Waterfall:
    """
    The deal's priority of payments: ordered tiers (fees, interest, coverage
    tests, principal, residual) with their recipients, allocation mode,
    funding source and the coverage rules the tests use.

    Obtain one from :meth:`StructuredCredit.create_waterfall` (the effective
    template or custom waterfall) or :meth:`Waterfall.from_json`; pass a
    ``Waterfall`` (or its dict) to :meth:`StructuredCreditBuilder.waterfall`
    to run a custom priority of payments.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.dates import DayCount
    >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import (
    ...     AssetPool,
    ...     PoolAsset,
    ...     StandardDealParams,
    ...     StructuredCredit,
    ...     Tranche,
    ...     TrancheStructure,
    ... )
    >>> as_of, maturity = datetime.date(2024, 1, 15), datetime.date(2031, 1, 15)
    >>> pool = AssetPool("POOL-1", "clo", Currency("USD")).with_assets([
    ...     PoolAsset.fixed_rate_bond("LOAN-1", Money(80_000_000.0, Currency("USD")), 0.07, maturity, DayCount.ACT_360)
    ... ])
    >>> note = (
    ...     Tranche
    ...     .builder()
    ...     .id("A")
    ...     .attach_pct(0.0)
    ...     .detach_pct(100.0)
    ...     .seniority("senior")
    ...     .original_balance(Money(80_000_000.0, Currency("USD")))
    ...     .coupon_fixed(0.05)
    ...     .maturity(maturity)
    ...     .build()
    ... )
    >>> deal = StructuredCredit.new_standard(
    ...     "clo",
    ...     StandardDealParams(
    ...         "CLO-1",
    ...         pool,
    ...         TrancheStructure([note]),
    ...         as_of,
    ...         maturity,
    ...         "USD-SOFR-DISC",
    ...         calendar_id="nyse",
    ...     ),
    ... )
    >>> market = MarketContext().insert(DiscountCurve.flat("USD-SOFR-DISC", as_of, 0.03))
    >>> waterfall = deal.create_waterfall()
    >>> waterfall.currency, len(waterfall.tiers) > 0
    ('USD', True)
    """

    @staticmethod
    def from_json(json: str) -> Waterfall:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``Waterfall`` (the shape ``to_json`` writes).

        Returns
        -------
        Waterfall
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Waterfall
        >>> try:
        ...     Waterfall.from_json("{}")
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
            JSON-encoded ``Waterfall``.

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
            ``(Waterfall.from_json, (json,))``.

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

    def coverage_tests(self) -> list[dict[str, Any]]:
        """
        Every coverage test placed in the waterfall as ``CoverageTestSpec``
        serde dicts, in tier order.

        Returns
        -------
        list[dict[str, Any]]
            One dict per test (``id``, ``tranche_id``, ``kind``,
            ``trigger_level``, ``action`` ...).

        Raises
        ------
        ValueError
            If the specs cannot be serialized.
        """
        ...

    @property
    def currency(self) -> str:
        """
        ISO-4217 currency code every waterfall amount is denominated in.

        Returns
        -------
        str
            The currency code.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def tiers(self) -> list[dict[str, Any]]:
        """
        Ordered tiers as ``WaterfallTier`` serde dicts.

        Returns
        -------
        list[dict[str, Any]]
            ``id``, ``priority``, ``payment_type``, ``allocation_mode``, ``recipients``, ``tests`` and ``funding`` per tier.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def coverage_rules(self) -> CoverageRules | None:
        """
        Collateral valuation rules attached to the coverage tests.

        Returns
        -------
        CoverageRules | None
            ``None`` when collateral is carried at par.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
