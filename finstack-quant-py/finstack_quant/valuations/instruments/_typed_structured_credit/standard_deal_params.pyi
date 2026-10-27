"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_structured_credit/standard_deal_params.rs``."""

from __future__ import annotations

import datetime
from typing import Any
from finstack_quant.valuations.instruments._typed_structured_credit.asset_pool import AssetPool
from finstack_quant.valuations.instruments._typed_structured_credit.tranche_structure import TrancheStructure

class StandardDealParams:
    """Input record for a standard structured-credit registry profile.

    Calendar omission is accepted during construction; deterministic simulation
    needs an explicit payment calendar. Balances retain their pool currency,
    rates retain decimal units, and dates are contractual calendar dates.

    Parameters
    ----------
    id : str
        Stable instrument identifier used in pricing results.
    pool : AssetPool
        Collateral balances and terms in the pool currency.
    tranches : TrancheStructure
        Liability capital structure and coupon terms.
    closing_date : datetime.date | datetime.datetime | pandas.Timestamp | str
        Contractual inception date or ISO-8601 YYYY-MM-DD string. A timezone-aware
        timestamp contributes its wall-clock date without timezone conversion.
    maturity : datetime.date | datetime.datetime | pandas.Timestamp | str
        Legal final date or ISO-8601 YYYY-MM-DD string, validated by ``new_standard``
        to follow closing. Timezone-aware inputs retain their wall-clock date.
    discount_curve_id : str
        Identifier of the market discount curve used for tranche valuation.
    calendar_id : str | None
        Payment holiday-calendar identifier; defaults to None and must be
        supplied before simulation.

    Raises
    ------
    ValueError
        If a date string is malformed or its calendar components are invalid.
    TypeError
        If either date is not a string or an object with integer year/month/day attributes.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import StandardDealParams, StructuredCredit
    >>> deal = StructuredCredit.example()
    >>> params = StandardDealParams(
    ...     deal.id, deal.pool, deal.tranches, deal.closing_date, deal.maturity, deal.discount_curve_id
    ... )
    >>> params.calendar_id is None
    True
    """
    def __init__(
        self,
        id: str,
        pool: AssetPool,
        tranches: TrancheStructure,
        closing_date: datetime.date | str,
        maturity: datetime.date | str,
        discount_curve_id: str,
        calendar_id: str | None = None,
    ) -> None:
        """Store standard-deal inputs without selecting assumptions.

        Parameters
        ----------
        id : str
            Stable instrument identifier used in pricing results.
        pool : AssetPool
            Collateral balances and terms in the pool currency.
        tranches : TrancheStructure
            Liability capital structure and coupon terms.
        closing_date : datetime.date | datetime.datetime | pandas.Timestamp | str
            Contractual inception date or ISO-8601 YYYY-MM-DD string. A timezone-aware
            timestamp contributes its wall-clock date without timezone conversion.
        maturity : datetime.date | datetime.datetime | pandas.Timestamp | str
            Legal final date or ISO-8601 YYYY-MM-DD string, validated by ``new_standard``
            to follow closing. Timezone-aware inputs retain their wall-clock date.
        discount_curve_id : str
            Identifier of the market discount curve used for tranche valuation.
        calendar_id : str | None
            Payment holiday-calendar identifier; defaults to None and must be
            supplied before simulation.

        Raises
        ------
        ValueError
            If a date string is malformed or its calendar components are invalid.
        TypeError
            If either date is not a string or an object with integer year/month/day attributes.
        """
        ...
    @property
    def id(self) -> str:
        """Stable deal identifier.

        Returns
        -------
        str
            Stable deal identifier.

        Notes
        -----
        This accessor does not raise; it returns an owned view of the stored input.
        """
        ...
    @property
    def pool(self) -> AssetPool:
        """Collateral pool in its stated currency.

        Returns
        -------
        AssetPool
            Collateral pool in its stated currency.

        Notes
        -----
        This accessor does not raise; it returns an owned view of the stored input.
        """
        ...
    @property
    def tranches(self) -> TrancheStructure:
        """Liability capital structure.

        Returns
        -------
        TrancheStructure
            Liability capital structure.

        Notes
        -----
        This accessor does not raise; it returns an owned view of the stored input.
        """
        ...
    @property
    def closing_date(self) -> datetime.date:
        """Contractual inception date.

        Returns
        -------
        datetime.date
            Contractual inception date.

        Raises
        ------
        ValueError
            If the stored date is outside the Python ``datetime.date`` year range
            (1 through 9999).
        """
        ...
    @property
    def maturity(self) -> datetime.date:
        """Legal final maturity date.

        Returns
        -------
        datetime.date
            Legal final maturity date.

        Raises
        ------
        ValueError
            If the stored date is outside the Python ``datetime.date`` year range
            (1 through 9999).
        """
        ...
    @property
    def discount_curve_id(self) -> str:
        """Market discount-curve lookup identifier.

        Returns
        -------
        str
            Market discount-curve lookup identifier.

        Notes
        -----
        This accessor does not raise; it returns an owned view of the stored input.
        """
        ...
    @property
    def calendar_id(self) -> str | None:
        """Optional payment holiday-calendar identifier.

        Returns
        -------
        str | None
            Optional payment holiday-calendar identifier.

        Notes
        -----
        This accessor does not raise; it returns an owned view of the stored input.
        """
        ...
    def to_dict(self) -> dict[str, Any]:
        """Serialize inputs to their canonical Rust field mapping.

        Returns
        -------
        dict[str, Any]
            Canonical fields, including null calendar when absent.

        Raises
        ------
        ValueError
            If the input record cannot be serialized to a Python mapping.
        """
        ...
    def to_json(self) -> str:
        """Serialize canonical inputs as JSON.

        Returns
        -------
        str
            Canonical JSON record.

        Raises
        ------
        ValueError
            If the input record cannot be serialized to JSON.
        """
        ...
    @staticmethod
    def from_json(json: str) -> StandardDealParams:
        """Deserialize the canonical Rust input record.

        Parameters
        ----------
        json : str
            Canonical input JSON, with ISO calendar dates and nested pool/tranches.

        Returns
        -------
        StandardDealParams
            Decoded inputs, without selecting registry assumptions.

        Raises
        ------
        ValueError
            If JSON is malformed or does not match the input schema.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import StandardDealParams
        >>> try:
        ...     StandardDealParams.from_json("{}")
        ... except ValueError:
        ...     print("invalid inputs")
        invalid inputs
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable and canonical JSON arguments.

        Raises
        ------
        ValueError
            If serialization of the stored value fails.
        """
        ...
