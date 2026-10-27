"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_structured_credit/borrowing_base.rs``."""

from __future__ import annotations

import datetime
from typing import Any

class EligibilityRule:
    """
    Which collateral an advance rate applies to.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.valuations.instruments import EligibilityRule
    >>> rule = EligibilityRule(max_days_past_due=60, max_maturity=datetime.date(2030, 1, 1))
    >>> rule.exclude_defaulted, rule.max_days_past_due
    (True, 60)
    """

    def __init__(
        self,
        *,
        exclude_defaulted: bool = True,
        max_maturity: datetime.date | None = None,
        max_days_past_due: int | None = None,
        exclude_non_performing: bool = True,
    ) -> None:
        """
        Construct an eligibility rule.

        Parameters
        ----------
        exclude_defaulted : bool, optional
            Exclude defaulted rows; ``True`` by default.
        max_maturity : datetime.date, optional
            Exclude rows maturing after this date; ``None`` for no limit.
        max_days_past_due : int, optional
            Exclude balances more than this many days delinquent (delinquency
            buckets are 30 days each); ``None`` for no limit.
        exclude_non_performing : bool, optional
            Exclude unresolved non-performing loans; ``True`` by default.

        Raises
        ------
        ValueError
            If ``max_maturity`` is not a valid date.
        """
        ...

    @staticmethod
    def from_json(json: str) -> EligibilityRule:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``EligibilityRule`` (the shape ``to_json`` writes).

        Returns
        -------
        EligibilityRule
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import EligibilityRule
        >>> try:
        ...     EligibilityRule.from_json('{"unknown": 1}')
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
            Canonical JSON.

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
            ``(EligibilityRule.from_json, (json,))``.

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
    def exclude_defaulted(self) -> bool:
        """
        Whether defaulted rows are excluded.

        Returns
        -------
        bool
            ``True`` when excluded.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def max_maturity(self) -> datetime.date | None:
        """
        Latest eligible maturity.

        Returns
        -------
        datetime.date | None
            ``None`` for no limit.

        Raises
        ------
        ValueError
            If the date cannot be converted.
        """
        ...

    @property
    def max_days_past_due(self) -> int | None:
        """
        Maximum days past due of an eligible balance.

        Returns
        -------
        int | None
            ``None`` for no limit.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def exclude_non_performing(self) -> bool:
        """
        Whether unresolved non-performing loans are excluded.

        Returns
        -------
        bool
            ``True`` when excluded.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class AdvanceRate:
    """
    Advance rate on one asset class of the collateral.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import AdvanceRate, EligibilityRule
    >>> rate = AdvanceRate("commercial_mortgage", 0.8, eligibility=EligibilityRule(max_days_past_due=60))
    >>> rate.asset_class, rate.rate
    ('commercial_mortgage', 0.8)
    """

    def __init__(self, asset_class: str, rate: float, *, eligibility: EligibilityRule | None = None) -> None:
        """
        Construct an advance rate.

        Parameters
        ----------
        asset_class : str
            ``AssetType`` wire name the rate applies to (for example
            ``"commercial_mortgage"`` or ``"leveraged_loan"``).
        rate : float
            Advance rate as a decimal in ``[0, 1]`` (``0.8`` lends 80% of
            eligible balance).
        eligibility : EligibilityRule, optional
            Which rows of the class are eligible; the default rule excludes
            defaulted and non-performing rows.

        Raises
        ------
        ValueError
            If ``asset_class`` is empty or ``rate`` is not a finite decimal in
            ``[0, 1]`` (Rust ``AdvanceRate::validate``).
        """
        ...

    @staticmethod
    def from_json(json: str) -> AdvanceRate:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``AdvanceRate`` (the shape ``to_json`` writes).

        Returns
        -------
        AdvanceRate
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed, carries unknown fields, or fails the same
            canonical Rust validation as construction (invalid ranges, dates or rules).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import AdvanceRate
        >>> try:
        ...     AdvanceRate.from_json("{}")
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
            Canonical JSON.

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
            ``(AdvanceRate.from_json, (json,))``.

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
    def asset_class(self) -> str:
        """
        Asset class the rate applies to.

        Returns
        -------
        str
            ``AssetType`` wire name.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rate(self) -> float:
        """
        Advance rate lent against the eligible balance of the class.

        Returns
        -------
        float
            Decimal in ``[0, 1]`` (``0.8`` lends 80% of eligible balance).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def eligibility(self) -> EligibilityRule:
        """
        Eligibility rule of the class.

        Returns
        -------
        EligibilityRule
            The rule.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class ConcentrationLimit:
    """
    Cap on the eligible collateral one obligor, industry or asset class may
    contribute; balance above the cap is excluded from the borrowing base.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import ConcentrationLimit
    >>> ConcentrationLimit("obligor", 5.0).scope
    'obligor'
    """

    def __init__(self, scope: str, max_pct: float) -> None:
        """
        Construct a concentration limit.

        Parameters
        ----------
        scope : str
            ``"obligor"`` (per ``obligor_id``), ``"industry"`` (per
            ``industry``) or ``"asset_class"`` (per asset type).
        max_pct : float
            Maximum share of the eligible collateral in percent (``20.0`` =
            20%).

        Raises
        ------
        ValueError
            If ``scope`` is not one of the three names or ``max_pct`` is not a
            finite percent in ``(0, 100]`` (Rust ``ConcentrationLimit::validate``).
        """
        ...

    @staticmethod
    def from_json(json: str) -> ConcentrationLimit:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``ConcentrationLimit`` (the shape ``to_json`` writes).

        Returns
        -------
        ConcentrationLimit
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed, carries unknown fields, or fails the same
            canonical Rust validation as construction (invalid ranges, dates or rules).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import ConcentrationLimit
        >>> try:
        ...     ConcentrationLimit.from_json("{}")
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
            Canonical JSON.

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
            ``(ConcentrationLimit.from_json, (json,))``.

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
    def scope(self) -> str:
        """
        Dimension the cap is measured on.

        Returns
        -------
        str
            ``"obligor"``, ``"industry"`` or ``"asset_class"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def max_pct(self) -> float:
        """
        Maximum share of the eligible collateral one bucket may contribute.

        Returns
        -------
        float
            Percent of the eligible collateral (``20.0`` = 20%).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class BorrowingBaseRules:
    """
    Advance rates and concentration limits that size a borrowing base.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import AdvanceRate, BorrowingBaseRules, ConcentrationLimit
    >>> rules = BorrowingBaseRules(
    ...     [AdvanceRate("commercial_mortgage", 0.8)],
    ...     concentration_limits=[ConcentrationLimit("obligor", 20.0)],
    ... )
    >>> len(rules.advance_rates), rules.concentration_limits[0].max_pct
    (1, 20.0)
    """

    def __init__(
        self,
        advance_rates: list[AdvanceRate],
        *,
        concentration_limits: list[ConcentrationLimit] | None = None,
    ) -> None:
        """
        Construct borrowing-base rules.

        Parameters
        ----------
        advance_rates : list[AdvanceRate]
            One advance rate per eligible asset class (at least one).
        concentration_limits : list[ConcentrationLimit], optional
            Caps on obligor, industry or asset-class shares; none by default.

        Raises
        ------
        ValueError
            If no advance rate is given, a class is repeated or a rate or
            limit is invalid.
        """
        ...

    @staticmethod
    def from_json(json: str) -> BorrowingBaseRules:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``BorrowingBaseRules`` (the shape ``to_json`` writes).

        Returns
        -------
        BorrowingBaseRules
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed, carries unknown fields, or fails the same
            canonical Rust validation as construction (invalid ranges, dates or rules).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import BorrowingBaseRules
        >>> try:
        ...     BorrowingBaseRules.from_json("{}")
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
            Canonical JSON.

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
            ``(BorrowingBaseRules.from_json, (json,))``.

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
    def advance_rates(self) -> list[AdvanceRate]:
        """
        Advance rates by asset class.

        Returns
        -------
        list[AdvanceRate]
            One entry per class.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def concentration_limits(self) -> list[ConcentrationLimit]:
        """
        Concentration limits.

        Returns
        -------
        list[ConcentrationLimit]
            Possibly empty.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
